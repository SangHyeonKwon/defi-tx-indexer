use sqlx::PgPool;
use tokio::sync::Semaphore;

use std::sync::Arc;

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
        let mut join_set = tokio::task::JoinSet::new();

        for block_num in from..=to {
            let permit = Arc::clone(&self.semaphore);
            let rpc_url = self.rpc_url.clone();
            let db_pool = self.db_pool.clone();

            join_set.spawn(async move {
                let _permit = permit.acquire().await?;
                Self::process_block(&db_pool, &rpc_url, block_num).await
            });
        }

        while let Some(result) = join_set.join_next().await {
            result??;
        }

        Ok(())
    }

    /// 단일 블록을 수집·디코딩·저장한다.
    ///
    /// Phase 3에서 구현:
    /// 1. RPC로 블록 + 영수증 fetch
    /// 2. 각 로그를 decoder로 디코딩
    /// 3. DB에 배치 INSERT
    async fn process_block(
        _db_pool: &PgPool,
        _rpc_url: &str,
        block_number: u64,
    ) -> anyhow::Result<()> {
        tracing::debug!(block_number, "processing block (stub)");
        // Phase 3에서 구현
        Ok(())
    }
}
