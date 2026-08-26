pub mod alerts;
pub mod analytics;
pub mod blocks;
pub mod contract_labels;
pub mod failed_tx;
pub mod health;
pub mod pools;
pub mod swaps;
pub mod tokens;
pub mod traders;

mod state;

pub use state::ApiState;

use axum::routing::{delete, get, post};
use axum::Router;
use tower_http::cors::CorsLayer;
use tower_http::trace::TraceLayer;

/// API 라우터를 조립한다.
///
/// `/health` 엔드포인트와 `/v1/` 하위 라우트를 포함한다.
///
/// **PROTECTED routes** (S16/M006/D021/D022) — 핸들러 시그니처의 `_: AdminAuth`
/// extractor가 `Authorization: Bearer <AMARILLO_ADMIN_API_KEY>`를 강제:
/// - `POST   /v1/contract-labels`
/// - `DELETE /v1/contract-labels/{address}`
/// - `POST   /v1/alert-subscriptions`
/// - `DELETE /v1/alert-subscriptions/{id}`
/// - `POST   /v1/alert-subscriptions/{id}/rotate-secret`
///
/// 그 외 `GET /v1/*`와 `/health`는 공개(임베드성 보존). 새 write/admin 라우트
/// 추가 시 핸들러에 `_: AdminAuth`를 박지 않으면 *컴파일은 되지만 인증 없이
/// 노출되는 회귀*이므로 — 본 표 + 통합테스트(crates/api/tests/auth.rs)가
/// 회귀 차단.
pub fn build_router(state: ApiState) -> Router {
    Router::new()
        .route("/health", get(health::health_check))
        .nest("/v1", v1_router())
        .layer(TraceLayer::new_for_http())
        .layer(CorsLayer::permissive())
        .with_state(state)
}

/// v1 API 라우트를 정의한다.
fn v1_router() -> Router<ApiState> {
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
        .route(
            "/analytics/failed-tx/timeseries",
            get(failed_tx::failed_tx_timeseries),
        )
        .route(
            "/analytics/failed-tx/unknown-clusters",
            get(analytics::unknown_revert_clusters),
        )
        .route(
            "/analytics/failed-tx/by-label",
            get(failed_tx::failed_tx_by_label),
        )
        // failed-tx
        .route("/failed-tx", get(failed_tx::list_failed_tx))
        .route("/failed-tx/{tx_hash}", get(failed_tx::get_failed_tx))
        // alert subscriptions (S08, HARDEN2)
        .route(
            "/alert-subscriptions",
            post(alerts::create_alert_subscription).get(alerts::list_alert_subscriptions),
        )
        .route(
            "/alert-subscriptions/{id}",
            delete(alerts::deactivate_alert_subscription),
        )
        .route(
            "/alert-subscriptions/{id}/rotate-secret",
            post(alerts::rotate_alert_subscription_secret),
        )
        // contract labels (S15 / M005) — admin, NO auth (D008/D019 demo scope)
        .route(
            "/contract-labels",
            post(contract_labels::create_contract_label),
        )
        .route(
            "/contract-labels/{address}",
            delete(contract_labels::delete_contract_label),
        )
}
