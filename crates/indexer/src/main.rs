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

use clap::Parser;
use tracing_subscriber::EnvFilter;

use crate::config::Config;
use crate::worker::WorkerPool;

/// Uniswap V3 DeFi transaction indexer.
///
/// Collects, decodes, and stores Uniswap V3 events from the Ethereum
/// blockchain into PostgreSQL.
#[derive(Parser)]
#[command(name = "indexer", version, about)]
struct Cli {
    /// Start block number (inclusive)
    #[arg(long)]
    from_block: u64,

    /// End block number (inclusive, defaults to from_block)
    #[arg(long)]
    to_block: Option<u64>,
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    // 로깅 초기화
    tracing_subscriber::fmt()
        .with_env_filter(EnvFilter::from_default_env())
        .init();

    tracing::info!("starting DeFi Analytics indexer");

    // CLI + 환경변수 설정 로드
    let cli = Cli::parse();
    let config = Config::from_env()?.with_block_range(cli.from_block, cli.to_block);
    tracing::info!(?config, "configuration loaded");

    // DB 연결 + 마이그레이션
    let db_pool = db::create_pool(&config.database_url, config.max_db_connections).await?;
    db::run_migrations(&db_pool).await?;
    tracing::info!("database connected and migrations applied");

    // 체크포인트에서 재개 지점 결정
    let from_block = match db::queries::get_last_checkpoint(&db_pool, 1).await? {
        Some(last) if last >= config.from_block as i64 => {
            let resume = (last + 1) as u64;
            tracing::info!(
                checkpoint = last,
                resume_from = resume,
                "resuming from checkpoint"
            );
            resume
        }
        _ => config.from_block,
    };

    let to_block = config.to_block.unwrap_or(from_block);
    if from_block > to_block {
        tracing::info!("all blocks already indexed up to checkpoint");
        return Ok(());
    }

    // 워커 풀 생성 및 인덱싱 시작
    let worker_pool = WorkerPool::new(
        db_pool,
        config.rpc_url.clone(),
        config.max_concurrent_blocks,
        config.batch_size,
    );

    worker_pool.index_range(from_block, to_block).await?;

    tracing::info!("indexer finished successfully");
    Ok(())
}
