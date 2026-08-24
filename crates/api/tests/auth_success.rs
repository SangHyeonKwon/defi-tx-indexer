//! Admin API 성공 경로(2xx) 통합 테스트 — 실제 PostgreSQL이 필요하다.
//!
//! `auth.rs`가 401 게이트(잘못된/누락 키 거부)를 검증하는 반면, 본 파일은
//! *유효한* Bearer 키가 실제로 통과해 2xx와 올바른 본문에 도달하는지 HTTP
//! 레벨에서 검증한다 — 이전에는 `scripts/verify-*.sh`(compose)에만 위임되어
//! 유효 키가 거부되는 회귀를 어떤 자동화도 잡지 못했다.
//!
//! 전부 `#[ignore]` — CI의 Integration tests 스텝(마이그레이션+시드된 PG)에서
//! `cargo test --workspace -- --ignored`로 실행된다.
//! `DATABASE_URL` 미설정 시 docker-compose 기본값을 사용한다.

use axum::body::Body;
use axum::http::{Method, Request, StatusCode};
use tower::ServiceExt;

use api::routes::{build_router, ApiState};

const DEFAULT_URL: &str = "postgres://defi:defi@localhost:5432/defi_analytics";
const TEST_KEY: &str = "integration-test-key-32-bytes-aaaa";

/// 본 테스트 전용 컨트랙트 주소 — 시드/타 테스트 픽스처와 겹치지 않는 값.
const LABEL_ADDR_MIXED: &str = "0xA07A5CCE55a07a5cce55A07A5CCE55a07a5cce55";
const LABEL_ADDR_LOWER: &str = "0xa07a5cce55a07a5cce55a07a5cce55a07a5cce55";

fn db_url() -> String {
    std::env::var("DATABASE_URL").unwrap_or_else(|_| DEFAULT_URL.to_string())
}

async fn state() -> ApiState {
    let db_pool = db::create_pool(&db_url(), 2).await.expect("connect");
    ApiState {
        db_pool,
        admin_api_key: TEST_KEY.into(),
    }
}

fn authed_json(method: Method, uri: &str, body: &str) -> Request<Body> {
    Request::builder()
        .method(method)
        .uri(uri)
        .header(
            axum::http::header::AUTHORIZATION,
            format!("Bearer {TEST_KEY}"),
        )
        .header(axum::http::header::CONTENT_TYPE, "application/json")
        .body(Body::from(body.to_owned()))
        .expect("build request")
}

fn authed_empty(method: Method, uri: &str) -> Request<Body> {
    Request::builder()
        .method(method)
        .uri(uri)
        .header(
            axum::http::header::AUTHORIZATION,
            format!("Bearer {TEST_KEY}"),
        )
        .body(Body::empty())
        .expect("build request")
}

async fn body_json(resp: axum::response::Response) -> serde_json::Value {
    let bytes = axum::body::to_bytes(resp.into_body(), usize::MAX)
        .await
        .expect("read body");
    serde_json::from_slice(&bytes).expect("valid JSON body")
}

/// `POST /v1/contract-labels` 생성(201) → 같은 주소 UPSERT(201, 라벨 교체) →
/// `DELETE`(204) → 재-DELETE(404). 유효 키가 게이트를 통과해 핸들러·DB까지
/// 도달하는 전체 경로 + 주소 lowercase 정규화를 확인한다.
#[tokio::test]
#[ignore = "requires PostgreSQL: cargo test -p api -- --ignored"]
async fn contract_label_success_path_create_upsert_delete() {
    let st = state().await;

    // 생성 — 대소문자 섞인 입력이 소문자로 정규화되어 저장된다.
    let body = format!(r#"{{"address":"{LABEL_ADDR_MIXED}","label":"ci-auth-success"}}"#);
    let resp = build_router(st.clone())
        .oneshot(authed_json(Method::POST, "/v1/contract-labels", &body))
        .await
        .expect("router responded");
    assert_eq!(
        resp.status(),
        StatusCode::CREATED,
        "valid key must reach 201"
    );
    let v = body_json(resp).await;
    assert_eq!(v["data"]["address"], LABEL_ADDR_LOWER);
    assert_eq!(v["data"]["label"], "ci-auth-success");

    // UPSERT — 같은 주소 재-POST는 라벨을 덮어쓰고 다시 201.
    let body = format!(r#"{{"address":"{LABEL_ADDR_LOWER}","label":"ci-auth-updated"}}"#);
    let resp = build_router(st.clone())
        .oneshot(authed_json(Method::POST, "/v1/contract-labels", &body))
        .await
        .expect("router responded");
    assert_eq!(resp.status(), StatusCode::CREATED);
    let v = body_json(resp).await;
    assert_eq!(v["data"]["label"], "ci-auth-updated");

    // 삭제 — 204, 두번째는 404 (핸들러 계약).
    let uri = format!("/v1/contract-labels/{LABEL_ADDR_LOWER}");
    let resp = build_router(st.clone())
        .oneshot(authed_empty(Method::DELETE, &uri))
        .await
        .expect("router responded");
    assert_eq!(resp.status(), StatusCode::NO_CONTENT);

    let resp = build_router(st)
        .oneshot(authed_empty(Method::DELETE, &uri))
        .await
        .expect("router responded");
    assert_eq!(resp.status(), StatusCode::NOT_FOUND);
}

/// 알림 구독 성공 경로: 생성(201, 시크릿 1회 노출) → 시크릿 회전(200, 값 변경)
/// → 비활성화(204) → 비활성 구독 회전/재삭제(404). 종료 시 행을 hard-delete해
/// 재실행 가능하게 유지한다.
#[tokio::test]
#[ignore = "requires PostgreSQL: cargo test -p api -- --ignored"]
async fn alert_subscription_success_path_create_rotate_deactivate() {
    let st = state().await;

    let resp = build_router(st.clone())
        .oneshot(authed_json(
            Method::POST,
            "/v1/alert-subscriptions",
            r#"{"webhook_url":"https://example.com/ci-auth-hook"}"#,
        ))
        .await
        .expect("router responded");
    assert_eq!(
        resp.status(),
        StatusCode::CREATED,
        "valid key must reach 201"
    );
    let v = body_json(resp).await;
    let id = v["data"]["subscription_id"]
        .as_i64()
        .expect("subscription_id");
    let secret = v["data"]["signing_secret"]
        .as_str()
        .expect("signing_secret exposed once")
        .to_owned();
    assert!(!secret.is_empty());
    assert_eq!(v["data"]["active"], true);
    assert_eq!(v["data"]["sub_type"], "per_event");

    // 회전 — 200 + 새 시크릿 (기존 값과 달라야 한다).
    let rotate_uri = format!("/v1/alert-subscriptions/{id}/rotate-secret");
    let resp = build_router(st.clone())
        .oneshot(authed_empty(Method::POST, &rotate_uri))
        .await
        .expect("router responded");
    assert_eq!(resp.status(), StatusCode::OK);
    let v = body_json(resp).await;
    assert_eq!(v["data"]["subscription_id"], id);
    let rotated = v["data"]["signing_secret"].as_str().expect("new secret");
    assert_ne!(rotated, secret, "rotate must issue a different secret");

    // 비활성화 — 204. 이후 회전/재삭제는 404 (soft delete 계약).
    let del_uri = format!("/v1/alert-subscriptions/{id}");
    let resp = build_router(st.clone())
        .oneshot(authed_empty(Method::DELETE, &del_uri))
        .await
        .expect("router responded");
    assert_eq!(resp.status(), StatusCode::NO_CONTENT);

    let resp = build_router(st.clone())
        .oneshot(authed_empty(Method::POST, &rotate_uri))
        .await
        .expect("router responded");
    assert_eq!(
        resp.status(),
        StatusCode::NOT_FOUND,
        "inactive → rotate 404"
    );

    let resp = build_router(st.clone())
        .oneshot(authed_empty(Method::DELETE, &del_uri))
        .await
        .expect("router responded");
    assert_eq!(
        resp.status(),
        StatusCode::NOT_FOUND,
        "already inactive → 404"
    );

    // teardown — soft delete 행을 물리 삭제해 재실행 시 누적을 방지.
    sqlx::query("DELETE FROM alert_subscription WHERE subscription_id = $1")
        .bind(id)
        .execute(&st.db_pool)
        .await
        .expect("cleanup subscription row");
}
