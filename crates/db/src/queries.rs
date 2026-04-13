use sqlx::PgPool;

use crate::error::DbError;
use crate::models::{
    Block, FailedTransaction, LiquidityEvent, Pool, PriceSnapshot, SwapEvent, Token, TokenTransfer,
    TraceLog, Transaction, UserProfile,
};

/// 블록을 배치 INSERT한다.
#[tracing::instrument(skip(pool, blocks))]
pub async fn insert_blocks(pool: &PgPool, blocks: &[Block]) -> Result<u64, DbError> {
    let mut tx = pool.begin().await?;
    let mut count = 0u64;

    for block in blocks {
        sqlx::query(
            "INSERT INTO block (block_number, timestamp, gas_used)
             VALUES ($1, $2, $3)
             ON CONFLICT (block_number) DO NOTHING",
        )
        .bind(block.block_number)
        .bind(block.timestamp)
        .bind(block.gas_used)
        .execute(&mut *tx)
        .await?;
        count += 1;
    }

    tx.commit().await?;
    Ok(count)
}

/// 트랜잭션을 배치 INSERT한다.
#[tracing::instrument(skip(pool, transactions))]
pub async fn insert_transactions(
    pool: &PgPool,
    transactions: &[Transaction],
) -> Result<u64, DbError> {
    let mut tx = pool.begin().await?;
    let mut count = 0u64;

    for t in transactions {
        sqlx::query(
            "INSERT INTO transaction (tx_hash, from_addr, to_addr, block_number, gas_used, gas_price, value, status, input_data)
             VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9)
             ON CONFLICT (tx_hash) DO NOTHING",
        )
        .bind(&t.tx_hash)
        .bind(&t.from_addr)
        .bind(&t.to_addr)
        .bind(t.block_number)
        .bind(t.gas_used)
        .bind(&t.gas_price)
        .bind(&t.value)
        .bind(t.status)
        .bind(&t.input_data)
        .execute(&mut *tx)
        .await?;
        count += 1;
    }

    tx.commit().await?;
    Ok(count)
}

/// 토큰 메타데이터를 INSERT한다.
#[tracing::instrument(skip(pool, tokens))]
pub async fn insert_tokens(pool: &PgPool, tokens: &[Token]) -> Result<u64, DbError> {
    let mut tx = pool.begin().await?;
    let mut count = 0u64;

    for token in tokens {
        sqlx::query(
            "INSERT INTO token (token_address, symbol, name, decimals)
             VALUES ($1, $2, $3, $4)
             ON CONFLICT (token_address) DO NOTHING",
        )
        .bind(&token.token_address)
        .bind(&token.symbol)
        .bind(&token.name)
        .bind(token.decimals)
        .execute(&mut *tx)
        .await?;
        count += 1;
    }

    tx.commit().await?;
    Ok(count)
}

/// 풀 정보를 INSERT한다.
#[tracing::instrument(skip(pool_conn, pools))]
pub async fn insert_pools(pool_conn: &PgPool, pools: &[Pool]) -> Result<u64, DbError> {
    let mut tx = pool_conn.begin().await?;
    let mut count = 0u64;

    for p in pools {
        sqlx::query(
            "INSERT INTO pool (pool_address, pair_name, token0_address, token1_address, fee_tier, created_at)
             VALUES ($1, $2, $3, $4, $5, $6)
             ON CONFLICT (pool_address) DO NOTHING",
        )
        .bind(&p.pool_address)
        .bind(&p.pair_name)
        .bind(&p.token0_address)
        .bind(&p.token1_address)
        .bind(p.fee_tier)
        .bind(p.created_at)
        .execute(&mut *tx)
        .await?;
        count += 1;
    }

    tx.commit().await?;
    Ok(count)
}

/// 스왑 이벤트를 배치 INSERT한다.
#[tracing::instrument(skip(pool, events))]
pub async fn insert_swap_events(pool: &PgPool, events: &[SwapEvent]) -> Result<u64, DbError> {
    let mut tx = pool.begin().await?;
    let mut count = 0u64;

    for e in events {
        sqlx::query(
            "INSERT INTO swap_event (pool_address, tx_hash, sender, recipient, amount0, amount1, amount_in, amount_out, sqrt_price_x96, liquidity, tick, log_index, timestamp)
             VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10, $11, $12, $13)
             ON CONFLICT (tx_hash, log_index) DO NOTHING",
        )
        .bind(&e.pool_address)
        .bind(&e.tx_hash)
        .bind(&e.sender)
        .bind(&e.recipient)
        .bind(&e.amount0)
        .bind(&e.amount1)
        .bind(&e.amount_in)
        .bind(&e.amount_out)
        .bind(&e.sqrt_price_x96)
        .bind(&e.liquidity)
        .bind(e.tick)
        .bind(e.log_index)
        .bind(e.timestamp)
        .execute(&mut *tx)
        .await?;
        count += 1;
    }

    tx.commit().await?;
    Ok(count)
}

/// 유동성 이벤트를 배치 INSERT한다.
#[tracing::instrument(skip(pool, events))]
pub async fn insert_liquidity_events(
    pool: &PgPool,
    events: &[LiquidityEvent],
) -> Result<u64, DbError> {
    let mut tx = pool.begin().await?;
    let mut count = 0u64;

    for e in events {
        sqlx::query(
            "INSERT INTO liquidity_event (event_type, pool_address, tx_hash, provider, token0_amount, token1_amount, tick_lower, tick_upper, liquidity, log_index, timestamp)
             VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10, $11)
             ON CONFLICT (tx_hash, log_index) DO NOTHING",
        )
        .bind(&e.event_type)
        .bind(&e.pool_address)
        .bind(&e.tx_hash)
        .bind(&e.provider)
        .bind(&e.token0_amount)
        .bind(&e.token1_amount)
        .bind(e.tick_lower)
        .bind(e.tick_upper)
        .bind(&e.liquidity)
        .bind(e.log_index)
        .bind(e.timestamp)
        .execute(&mut *tx)
        .await?;
        count += 1;
    }

    tx.commit().await?;
    Ok(count)
}

/// 토큰 전송을 배치 INSERT한다.
#[tracing::instrument(skip(pool, transfers))]
pub async fn insert_token_transfers(
    pool: &PgPool,
    transfers: &[TokenTransfer],
) -> Result<u64, DbError> {
    let mut tx = pool.begin().await?;
    let mut count = 0u64;

    for t in transfers {
        sqlx::query(
            "INSERT INTO token_transfer (tx_hash, token_address, from_addr, to_addr, amount, log_index, timestamp)
             VALUES ($1, $2, $3, $4, $5, $6, $7)
             ON CONFLICT (tx_hash, log_index) DO NOTHING",
        )
        .bind(&t.tx_hash)
        .bind(&t.token_address)
        .bind(&t.from_addr)
        .bind(&t.to_addr)
        .bind(&t.amount)
        .bind(t.log_index)
        .bind(t.timestamp)
        .execute(&mut *tx)
        .await?;
        count += 1;
    }

    tx.commit().await?;
    Ok(count)
}

/// 실패한 트랜잭션을 INSERT한다.
#[tracing::instrument(skip(pool, failed))]
pub async fn insert_failed_transactions(
    pool: &PgPool,
    failed: &[FailedTransaction],
) -> Result<u64, DbError> {
    let mut tx = pool.begin().await?;
    let mut count = 0u64;

    for f in failed {
        sqlx::query(
            "INSERT INTO failed_transaction (tx_hash, error_category, revert_reason, failing_function, gas_used, timestamp)
             VALUES ($1, $2, $3, $4, $5, $6)
             ON CONFLICT (tx_hash) DO NOTHING",
        )
        .bind(&f.tx_hash)
        .bind(&f.error_category)
        .bind(&f.revert_reason)
        .bind(&f.failing_function)
        .bind(f.gas_used)
        .bind(f.timestamp)
        .execute(&mut *tx)
        .await?;
        count += 1;
    }

    tx.commit().await?;
    Ok(count)
}

/// 가격 스냅샷을 INSERT한다.
#[tracing::instrument(skip(pool, snapshots))]
pub async fn insert_price_snapshots(
    pool: &PgPool,
    snapshots: &[PriceSnapshot],
) -> Result<u64, DbError> {
    let mut tx = pool.begin().await?;
    let mut count = 0u64;

    for s in snapshots {
        sqlx::query(
            "INSERT INTO price_snapshot (pool_address, price, tick, liquidity, snapshot_ts, interval_type)
             VALUES ($1, $2, $3, $4, $5, $6)
             ON CONFLICT (pool_address, snapshot_ts, interval_type) DO NOTHING",
        )
        .bind(&s.pool_address)
        .bind(&s.price)
        .bind(s.tick)
        .bind(&s.liquidity)
        .bind(s.snapshot_ts)
        .bind(&s.interval_type)
        .execute(&mut *tx)
        .await?;
        count += 1;
    }

    tx.commit().await?;
    Ok(count)
}

/// 유저 프로필을 UPSERT한다.
#[tracing::instrument(skip(pool, profiles))]
pub async fn upsert_user_profiles(pool: &PgPool, profiles: &[UserProfile]) -> Result<u64, DbError> {
    let mut tx = pool.begin().await?;
    let mut count = 0u64;

    for u in profiles {
        sqlx::query(
            "INSERT INTO user_profile (user_address, label, first_seen, last_seen, total_swaps, total_volume_usd)
             VALUES ($1, $2, $3, $4, $5, $6)
             ON CONFLICT (user_address) DO UPDATE SET
                 last_seen = EXCLUDED.last_seen,
                 total_swaps = user_profile.total_swaps + EXCLUDED.total_swaps,
                 total_volume_usd = user_profile.total_volume_usd + EXCLUDED.total_volume_usd",
        )
        .bind(&u.user_address)
        .bind(&u.label)
        .bind(u.first_seen)
        .bind(u.last_seen)
        .bind(u.total_swaps)
        .bind(&u.total_volume_usd)
        .execute(&mut *tx)
        .await?;
        count += 1;
    }

    tx.commit().await?;
    Ok(count)
}

/// 트레이스 로그를 배치 INSERT한다.
#[tracing::instrument(skip(pool, traces))]
pub async fn insert_trace_logs(pool: &PgPool, traces: &[TraceLog]) -> Result<u64, DbError> {
    let mut tx = pool.begin().await?;
    let mut count = 0u64;

    for t in traces {
        sqlx::query(
            "INSERT INTO trace_log (tx_hash, call_depth, call_type, from_addr, to_addr, value, gas_used, input, output, error)
             VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10)",
        )
        .bind(&t.tx_hash)
        .bind(t.call_depth)
        .bind(&t.call_type)
        .bind(&t.from_addr)
        .bind(&t.to_addr)
        .bind(&t.value)
        .bind(t.gas_used)
        .bind(&t.input)
        .bind(&t.output)
        .bind(&t.error)
        .execute(&mut *tx)
        .await?;
        count += 1;
    }

    tx.commit().await?;
    Ok(count)
}

/// 특정 체인의 마지막 체크포인트를 조회한다.
#[tracing::instrument(skip(pool))]
pub async fn get_last_checkpoint(pool: &PgPool, chain_id: i32) -> Result<Option<i64>, DbError> {
    let row: Option<(i64,)> =
        sqlx::query_as("SELECT last_processed_block FROM indexer_checkpoint WHERE chain_id = $1")
            .bind(chain_id)
            .fetch_optional(pool)
            .await?;
    Ok(row.map(|r| r.0))
}

/// 체크포인트를 갱신한다 (없으면 INSERT, 있으면 UPDATE).
#[tracing::instrument(skip(pool))]
pub async fn update_checkpoint(
    pool: &PgPool,
    chain_id: i32,
    last_processed_block: i64,
) -> Result<(), DbError> {
    sqlx::query(
        "INSERT INTO indexer_checkpoint (chain_id, last_processed_block, updated_at)
         VALUES ($1, $2, NOW())
         ON CONFLICT (chain_id) DO UPDATE
         SET last_processed_block = EXCLUDED.last_processed_block,
             updated_at = NOW()",
    )
    .bind(chain_id)
    .bind(last_processed_block)
    .execute(pool)
    .await?;
    Ok(())
}

/// 블록 번호로 블록을 조회한다.
#[tracing::instrument(skip(pool))]
pub async fn get_block_by_number(pool: &PgPool, block_number: i64) -> Result<Block, DbError> {
    sqlx::query_as::<_, Block>("SELECT * FROM block WHERE block_number = $1")
        .bind(block_number)
        .fetch_optional(pool)
        .await?
        .ok_or_else(|| DbError::NotFound(format!("block {block_number}")))
}

/// DB에 저장된 가장 최근 블록 번호를 반환한다.
#[tracing::instrument(skip(pool))]
pub async fn get_latest_block_number(pool: &PgPool) -> Result<Option<i64>, DbError> {
    let row: Option<(i64,)> = sqlx::query_as("SELECT MAX(block_number) FROM block")
        .fetch_optional(pool)
        .await?;
    Ok(row.map(|r| r.0))
}

/// 특정 블록의 스왑 이벤트를 조회한다.
#[tracing::instrument(skip(pool))]
pub async fn get_swap_events_by_block(
    pool: &PgPool,
    block_number: i64,
) -> Result<Vec<SwapEvent>, DbError> {
    let events = sqlx::query_as::<_, SwapEvent>(
        "SELECT se.* FROM swap_event se
         JOIN transaction t ON se.tx_hash = t.tx_hash
         WHERE t.block_number = $1
         ORDER BY se.event_id",
    )
    .bind(block_number)
    .fetch_all(pool)
    .await?;
    Ok(events)
}
