//! Admin API key integration tests (S16/M006).
//!
//! 라우터를 실제로 빌드한 뒤 `tower::ServiceExt::oneshot`으로 요청을 주입해
//! HTTP 레벨에서 인증 게이트를 검증한다. 핸들러까지 도달하지 않고 401이 반환됨
//! (extractor가 우선 거름)을 확인 → `PgPool::connect_lazy`로 DB connect 시도
//! 없이 안전하게 테스트 가능.
//!
//! 정상 200/201/204 시나리오는 verify 스크립트(S17, docker compose 환경)에서
//! 검증한다 — 본 파일은 *401 게이트*에 집중.

use std::time::Duration;

use axum::body::Body;
use axum::http::{Method, Request, StatusCode};
use sqlx::postgres::PgPoolOptions;
use tower::ServiceExt;

use api::routes::{build_router, ApiState};

const TEST_KEY: &str = "integration-test-key-32-bytes-aaaa";

fn test_state() -> ApiState {
    // acquire_timeout 기본값(30초)을 그대로 두면 /health 테스트가 DB 연결
    // 시도로 30초를 소모한다 — 어차피 실패가 전제이므로 즉시 포기.
    let db_pool = PgPoolOptions::new()
        .acquire_timeout(Duration::from_millis(100))
        .connect_lazy("postgres://test:test@localhost:5432/test")
        .expect("lazy pool");
    ApiState {
        db_pool,
        admin_api_key: TEST_KEY.into(),
    }
}

fn request_no_body(method: Method, uri: &str, auth: Option<&str>) -> Request<Body> {
    let mut builder = Request::builder().method(method).uri(uri);
    if let Some(value) = auth {
        builder = builder.header(axum::http::header::AUTHORIZATION, value);
    }
    builder.body(Body::empty()).expect("build request")
}

fn request_with_json(method: Method, uri: &str, auth: Option<&str>, body: &str) -> Request<Body> {
    let mut builder = Request::builder()
        .method(method)
        .uri(uri)
        .header(axum::http::header::CONTENT_TYPE, "application/json");
    if let Some(value) = auth {
        builder = builder.header(axum::http::header::AUTHORIZATION, value);
    }
    builder
        .body(Body::from(body.to_owned()))
        .expect("build request")
}

/// 보호 라우트 (`POST /v1/contract-labels`) — 헤더 없음 / Basic / Bearer wrong /
/// Bearer empty 모두 401, 또한 *동일한* 401 단일 응답(info-leak 방지).
#[tokio::test]
async fn contract_labels_post_rejects_unauthenticated() {
    let cases: &[(Option<&str>, &str)] = &[
        (None, "missing header"),
        (Some("Basic xxx"), "basic auth"),
        (Some("Bearer wrong-key-here"), "wrong key"),
        (Some("Bearer "), "empty bearer token"),
        (Some(TEST_KEY), "no Bearer prefix"),
    ];
    for (auth, label) in cases {
        let app = build_router(test_state());
        // 본문은 *형식상* 올바른 JSON — extractor가 먼저 거르므로 핸들러 단까지
        // 도달하지 않는다(즉 본문 검증·DB 호출 모두 발생 X).
        let body = r#"{"address":"0x0000000000000000000000000000000000000000","label":"test"}"#;
        let req = request_with_json(Method::POST, "/v1/contract-labels", *auth, body);
        let resp = app.oneshot(req).await.expect("router responded");
        assert_eq!(
            resp.status(),
            StatusCode::UNAUTHORIZED,
            "case `{label}` expected 401, got {}",
            resp.status()
        );
    }
}

/// 보호 라우트 (`DELETE /v1/contract-labels/{address}`) — 헤더 없으면 401.
#[tokio::test]
async fn contract_labels_delete_rejects_unauthenticated() {
    let app = build_router(test_state());
    let req = request_no_body(
        Method::DELETE,
        "/v1/contract-labels/0x0000000000000000000000000000000000000000",
        None,
    );
    let resp = app.oneshot(req).await.expect("router responded");
    assert_eq!(resp.status(), StatusCode::UNAUTHORIZED);
}

/// 보호 라우트 (`POST /v1/alert-subscriptions`) — 헤더 없으면 401.
#[tokio::test]
async fn alert_subscriptions_post_rejects_unauthenticated() {
    let app = build_router(test_state());
    let body = r#"{"webhook_url":"https://example.com/hook"}"#;
    let req = request_with_json(Method::POST, "/v1/alert-subscriptions", None, body);
    let resp = app.oneshot(req).await.expect("router responded");
    assert_eq!(resp.status(), StatusCode::UNAUTHORIZED);
}

/// 보호 라우트 (`DELETE /v1/alert-subscriptions/{id}`) — 헤더 없으면 401.
#[tokio::test]
async fn alert_subscriptions_delete_rejects_unauthenticated() {
    let app = build_router(test_state());
    let req = request_no_body(Method::DELETE, "/v1/alert-subscriptions/123", None);
    let resp = app.oneshot(req).await.expect("router responded");
    assert_eq!(resp.status(), StatusCode::UNAUTHORIZED);
}

/// 보호 라우트 (`POST /v1/alert-subscriptions/{id}/rotate-secret`) — 헤더 없으면 401.
#[tokio::test]
async fn alert_subscriptions_rotate_rejects_unauthenticated() {
    let app = build_router(test_state());
    let req = request_no_body(
        Method::POST,
        "/v1/alert-subscriptions/123/rotate-secret",
        None,
    );
    let resp = app.oneshot(req).await.expect("router responded");
    assert_eq!(resp.status(), StatusCode::UNAUTHORIZED);
}

/// 비보호 라우트 — `/health`에 헤더 *없이* 호출해도 *401이 아님*을 단언.
///
/// DB가 lazy pool이라 connect 실패로 500이 떨어지지만, *우리가 확인하려는 것은*
/// "인증이 강제되지 않는다"는 사실. 401이 아닌 어떤 응답이든 통과(보통 500).
#[tokio::test]
async fn health_endpoint_is_unauthenticated() {
    let app = build_router(test_state());
    let req = request_no_body(Method::GET, "/health", None);
    let resp = app.oneshot(req).await.expect("router responded");
    assert_ne!(
        resp.status(),
        StatusCode::UNAUTHORIZED,
        "/health should not require auth (got 401)"
    );
}

/// 비보호 라우트 — `/health`에 *잘못된 Bearer* 헤더가 있어도 401이 아님 — 인증
/// 자체가 *검증되지 않는* 라우트임을 강하게 확인.
#[tokio::test]
async fn health_endpoint_ignores_invalid_bearer() {
    let app = build_router(test_state());
    let req = request_no_body(Method::GET, "/health", Some("Bearer totally-wrong-key"));
    let resp = app.oneshot(req).await.expect("router responded");
    assert_ne!(
        resp.status(),
        StatusCode::UNAUTHORIZED,
        "/health is unauthenticated — invalid bearer should be ignored (got 401)"
    );
}
