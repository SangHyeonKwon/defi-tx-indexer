pub mod health;

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
        .layer(TraceLayer::new_for_http())
        .layer(CorsLayer::permissive())
        .with_state(db_pool)
}
