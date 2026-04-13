use std::str::FromStr;
use std::sync::Arc;

use alloy::consensus::Transaction as ConsensusTx;
use alloy::eips::BlockNumberOrTag;
use alloy::network::TransactionResponse;
use alloy::providers::{Provider, ProviderBuilder};
use bigdecimal::BigDecimal;
use chrono::DateTime;
use sqlx::PgPool;
use tokio::sync::Semaphore;

use db::models::{
    Block, LiquidityEvent, LiquidityEventType, SwapEvent, TokenTransfer, Transaction,
};
use decoder::events::{DecodedEvent, DecodedLiquidity, DecodedSwap, DecodedTransfer};

/// 블록 범위를 청크 단위로 분할하여 병렬 수집하는 워커 풀.
pub struct WorkerPool {
    /// DB 연결 풀
    db_pool: PgPool,
    /// RPC 엔드포인트
    rpc_url: String,
    /// 동시 실행 제한 세마포어
    semaphore: Arc<Semaphore>,
    /// 배치 INSERT 크기
    batch_size: usize,
}

impl WorkerPool {
    /// 새 워커 풀을 생성한다.
    pub fn new(db_pool: PgPool, rpc_url: String, max_concurrent: usize, batch_size: usize) -> Self {
        Self {
            db_pool,
            rpc_url,
            semaphore: Arc::new(Semaphore::new(max_concurrent)),
            batch_size,
        }
    }

    /// 지정된 블록 범위를 인덱싱한다.
    ///
    /// 블록 범위를 `batch_size` 단위 청크로 분할하고,
    /// `tokio::JoinSet`으로 병렬 수집한다.
    #[tracing::instrument(skip(self))]
    pub async fn index_range(&self, from_block: u64, to_block: u64) -> anyhow::Result<()> {
        tracing::info!(from_block, to_block, "starting block range indexing");

        let total = to_block.saturating_sub(from_block) + 1;
        let mut processed = 0u64;

        for chunk_start in (from_block..=to_block).step_by(self.batch_size) {
            let chunk_end = (chunk_start + self.batch_size as u64 - 1).min(to_block);
            self.process_chunk(chunk_start, chunk_end).await?;
            processed += chunk_end - chunk_start + 1;
            tracing::info!(processed, total, "progress");
        }

        tracing::info!(total_blocks = total, "indexing complete");
        Ok(())
    }

    /// 단일 블록 청크를 처리한다.
    #[tracing::instrument(skip(self))]
    async fn process_chunk(&self, from: u64, to: u64) -> anyhow::Result<()> {
        let provider = ProviderBuilder::new().connect_http(
            self.rpc_url
                .parse()
                .map_err(|e| anyhow::anyhow!("invalid RPC URL: {e}"))?,
        );

        let mut join_set = tokio::task::JoinSet::new();

        for block_num in from..=to {
            let permit = Arc::clone(&self.semaphore);
            let provider = provider.clone();
            let db_pool = self.db_pool.clone();

            join_set.spawn(async move {
                let _permit = permit.acquire().await?;
                Self::process_block(&db_pool, &provider, block_num).await
            });
        }

        while let Some(result) = join_set.join_next().await {
            result??;
        }

        Ok(())
    }

    /// 단일 블록을 수집·디코딩·저장한다.
    ///
    /// 1. RPC로 블록 + 영수증 fetch
    /// 2. 각 로그를 decoder로 디코딩
    /// 3. DB에 배치 INSERT
    async fn process_block(
        db_pool: &PgPool,
        provider: &impl Provider,
        block_number: u64,
    ) -> anyhow::Result<()> {
        tracing::debug!(block_number, "processing block");

        // 1. 블록 조회 (full transactions)
        let block = provider
            .get_block_by_number(BlockNumberOrTag::Number(block_number))
            .full()
            .await?
            .ok_or_else(|| anyhow::anyhow!("block {block_number} not found"))?;

        let timestamp =
            DateTime::from_timestamp(block.header.timestamp as i64, 0).unwrap_or_default();

        // 2. Block 모델 저장
        let block_model = Block {
            block_number: block_number as i64,
            timestamp,
            gas_used: block.header.gas_used as i64,
        };
        db::queries::insert_blocks(db_pool, &[block_model]).await?;

        // 3. 트랜잭션 영수증 조회
        let receipts = provider
            .get_block_receipts(BlockNumberOrTag::Number(block_number).into())
            .await?
            .unwrap_or_default();

        // 4. 트랜잭션 모델 빌드 + 로그 디코딩
        let block_txs: Vec<_> = block.transactions.into_transactions().collect();
        let mut transactions = Vec::with_capacity(block_txs.len());
        let mut swap_events: Vec<SwapEvent> = Vec::new();
        let mut liquidity_events: Vec<LiquidityEvent> = Vec::new();
        let mut token_transfers: Vec<TokenTransfer> = Vec::new();

        for (idx, receipt) in receipts.iter().enumerate() {
            let tx_hash_str = format!("0x{:x}", receipt.transaction_hash);

            // 트랜잭션 모델 빌드 (블록 TX + 영수증 매칭)
            if let Some(tx) = block_txs.get(idx) {
                let gas_price = tx.effective_gas_price.unwrap_or(0);
                let tx_model = Transaction {
                    tx_hash: tx_hash_str.clone(),
                    from_addr: format!("{}", tx.from()).to_lowercase(),
                    to_addr: ConsensusTx::to(tx).map(|a| format!("{a}").to_lowercase()),
                    block_number: block_number as i64,
                    gas_used: receipt.gas_used as i64,
                    gas_price: BigDecimal::from_str(&gas_price.to_string())
                        .unwrap_or_else(|_| BigDecimal::from(0)),
                    value: BigDecimal::from_str(&ConsensusTx::value(tx).to_string())
                        .unwrap_or_else(|_| BigDecimal::from(0)),
                    status: if receipt.status() { 1 } else { 0 },
                    input_data: if ConsensusTx::input(tx).is_empty() {
                        None
                    } else {
                        Some(format!("{}", ConsensusTx::input(tx)))
                    },
                };
                transactions.push(tx_model);
            }

            // 로그 디코딩
            for log in receipt.inner.logs() {
                let log_data = log.data();
                let topics = log_data.topics().to_vec();
                if topics.is_empty() {
                    continue;
                }

                let data = &log_data.data;
                let log_address = format!("{}", log.address()).to_lowercase();
                let log_idx = log.log_index.unwrap_or(0) as i32;

                match decoder::events::decode_log(
                    &topics,
                    data,
                    &log_address,
                    &tx_hash_str,
                    log_idx,
                    timestamp,
                ) {
                    Ok(DecodedEvent::Swap(s)) => swap_events.push(to_swap_model(s)),
                    Ok(DecodedEvent::Liquidity(l)) => {
                        liquidity_events.push(to_liquidity_model(l));
                    }
                    Ok(DecodedEvent::Transfer(t)) => {
                        token_transfers.push(to_transfer_model(t));
                    }
                    Err(decoder::error::DecodeError::UnknownTopic(_)) => {}
                    Err(e) => {
                        tracing::warn!(block_number, error = %e, "failed to decode log");
                    }
                }
            }
        }

        // 5. 배치 INSERT
        if !transactions.is_empty() {
            db::queries::insert_transactions(db_pool, &transactions).await?;
        }
        if !swap_events.is_empty() {
            db::queries::insert_swap_events(db_pool, &swap_events).await?;
        }
        if !liquidity_events.is_empty() {
            db::queries::insert_liquidity_events(db_pool, &liquidity_events).await?;
        }
        if !token_transfers.is_empty() {
            db::queries::insert_token_transfers(db_pool, &token_transfers).await?;
        }

        tracing::debug!(
            block_number,
            txs = transactions.len(),
            swaps = swap_events.len(),
            liq = liquidity_events.len(),
            transfers = token_transfers.len(),
            "block processed"
        );
        Ok(())
    }
}

/// `DecodedSwap` → DB `SwapEvent` 변환.
fn to_swap_model(s: DecodedSwap) -> SwapEvent {
    SwapEvent {
        pool_address: s.pool_address,
        tx_hash: s.tx_hash,
        sender: s.sender,
        recipient: s.recipient,
        amount0: s.amount0,
        amount1: s.amount1,
        amount_in: s.amount_in,
        amount_out: s.amount_out,
        sqrt_price_x96: s.sqrt_price_x96,
        liquidity: s.liquidity,
        tick: s.tick,
        log_index: s.log_index,
        timestamp: s.timestamp,
        event_id: 0, // DB에서 자동 생성
    }
}

/// `DecodedLiquidity` → DB `LiquidityEvent` 변환.
fn to_liquidity_model(l: DecodedLiquidity) -> LiquidityEvent {
    let event_type = match l.event_type.as_str() {
        "BURN" => LiquidityEventType::Burn,
        _ => LiquidityEventType::Mint,
    };
    LiquidityEvent {
        event_type,
        pool_address: l.pool_address,
        tx_hash: l.tx_hash,
        provider: l.provider,
        token0_amount: l.token0_amount,
        token1_amount: l.token1_amount,
        tick_lower: l.tick_lower,
        tick_upper: l.tick_upper,
        liquidity: l.liquidity,
        log_index: l.log_index,
        timestamp: l.timestamp,
        event_id: 0,
    }
}

/// `DecodedTransfer` → DB `TokenTransfer` 변환.
fn to_transfer_model(t: DecodedTransfer) -> TokenTransfer {
    TokenTransfer {
        tx_hash: t.tx_hash,
        token_address: t.token_address,
        from_addr: t.from_addr,
        to_addr: t.to_addr,
        amount: t.amount,
        log_index: t.log_index,
        timestamp: t.timestamp,
        transfer_id: 0,
    }
}
