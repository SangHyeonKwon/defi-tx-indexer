//! DeFi Analytics REST API 서버.
//!
//! 인덱싱된 Uniswap V3 데이터를 JSON REST API로 제공한다.
//!
//! ## 사용법
//! ```bash
//! # 환경변수 설정 후
//! cargo run -p api
//! ```

mod config;
mod error;
mod pagination;
mod response;
mod routes;

use tracing_subscriber::EnvFilter;

use crate::config::ApiConfig;

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    tracing_subscriber::fmt()
        .with_env_filter(EnvFilter::from_default_env())
        .init();

    let config = ApiConfig::from_env()?;
    tracing::info!(?config, "starting DeFi Analytics API");

    let db_pool = db::create_pool(&config.database_url, config.max_db_connections).await?;
    db::run_migrations(&db_pool).await?;
    tracing::info!("database connected and migrations applied");

    let app = routes::build_router(db_pool);
    let addr = format!("{}:{}", config.host, config.port);
    let listener = tokio::net::TcpListener::bind(&addr).await?;
    tracing::info!(addr, "API server listening");

    axum::serve(listener, app).await?;
    Ok(())
}
