pub mod analytics;
pub mod blocks;
pub mod health;
pub mod pools;
pub mod swaps;
pub mod tokens;
pub mod traders;

use axum::routing::get;
use axum::Router;
use sqlx::PgPool;
use tower_http::cors::CorsLayer;
use tower_http::trace::TraceLayer;

/// API 라우터를 조립한다.
///
/// `/health` 엔드포인트와 `/v1/` 하위 라우트를 포함한다.
pub fn build_router(db_pool: PgPool) -> Router {
    Router::new()
        .route("/health", get(health::health_check))
        .nest("/v1", v1_router())
        .layer(TraceLayer::new_for_http())
        .layer(CorsLayer::permissive())
        .with_state(db_pool)
}

/// v1 API 라우트를 정의한다.
fn v1_router() -> Router<PgPool> {
    Router::new()
        // blocks
        .route("/blocks/latest", get(blocks::get_latest_block))
        .route("/blocks/{number}", get(blocks::get_block))
        // pools
        .route("/pools", get(pools::list_pools))
        .route("/pools/{address}", get(pools::get_pool))
        .route("/pools/{address}/stats", get(pools::get_pool_stats))
        // tokens
        .route("/tokens", get(tokens::list_tokens))
        .route("/tokens/{address}", get(tokens::get_token))
        // swaps
        .route("/swaps", get(swaps::list_swaps))
        // traders
        .route("/traders/top", get(traders::get_top_traders))
        // analytics
        .route("/analytics/daily-volume", get(analytics::daily_volume))
        .route("/analytics/failed-tx", get(analytics::failed_tx_analysis))
}
