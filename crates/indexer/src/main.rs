//! DeFi Analytics 인덱서 — Uniswap V3 이벤트 수집기.
//!
//! 이더리움 블록체인에서 Uniswap V3 이벤트를 수집·디코딩·저장한다.
//!
//! ## 사용법
//! ```bash
//! cargo run -p indexer -- --from-block 18000000 --to-block 18001000
//! ```

mod config;
mod worker;

use tracing_subscriber::EnvFilter;

use crate::config::Config;
use crate::worker::WorkerPool;

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    // 로깅 초기화
    tracing_subscriber::fmt()
        .with_env_filter(EnvFilter::from_default_env())
        .init();

    tracing::info!("starting DeFi Analytics indexer");

    // 설정 로드
    let config = parse_args()?;
    tracing::info!(?config, "configuration loaded");

    // DB 연결 + 마이그레이션
    let db_pool = db::create_pool(&config.database_url, config.max_db_connections).await?;
    db::run_migrations(&db_pool).await?;
    tracing::info!("database connected and migrations applied");

    // 워커 풀 생성 및 인덱싱 시작
    let worker_pool = WorkerPool::new(
        db_pool,
        config.rpc_url.clone(),
        config.max_concurrent_blocks,
        config.batch_size,
    );

    let to_block = config.to_block.unwrap_or(config.from_block);
    worker_pool.index_range(config.from_block, to_block).await?;

    tracing::info!("indexer finished successfully");
    Ok(())
}

/// CLI 인자를 파싱한다.
///
/// `--from-block <N>` 와 `--to-block <N>` 을 지원한다.
fn parse_args() -> anyhow::Result<Config> {
    let args: Vec<String> = std::env::args().collect();
    let config = Config::from_env()?;

    let mut from_block = None;
    let mut to_block = None;

    let mut i = 1;
    while i < args.len() {
        match args[i].as_str() {
            "--from-block" => {
                i += 1;
                from_block = Some(
                    args.get(i)
                        .ok_or_else(|| anyhow::anyhow!("--from-block requires a value"))?
                        .parse::<u64>()?,
                );
            }
            "--to-block" => {
                i += 1;
                to_block = Some(
                    args.get(i)
                        .ok_or_else(|| anyhow::anyhow!("--to-block requires a value"))?
                        .parse::<u64>()?,
                );
            }
            other => {
                anyhow::bail!("unknown argument: {other}");
            }
        }
        i += 1;
    }

    let from = from_block.ok_or_else(|| anyhow::anyhow!("--from-block is required"))?;
    Ok(config.with_block_range(from, to_block))
}
