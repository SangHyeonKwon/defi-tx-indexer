//! Admin API key authentication (S16/M006).
//!
//! `AdminAuth` extractor — `Authorization: Bearer <key>` 헤더가 [`ApiState::admin_api_key`]
//! 와 **상수시간** 일치하면 통과, 그 외 모두 401.
//!
//! ## Info-leak 방지 (D021)
//!
//! 헤더 누락 / `Bearer ` prefix 누락 / 키 불일치 / 길이 불일치 — *모두 동일 401*.
//! 키 존재 여부·길이·prefix 형식 어느 것도 응답에 노출되지 않음. 클라이언트는
//! "401" 메시지만 받음.
//!
//! ## 전역 layer 대신 핸들러별 extractor (D022)
//!
//! 보호 라우트의 핸들러 시그니처에 `_: AdminAuth`가 반드시 박혀야 빌드된다.
//! 새 보호 라우트 추가 시 layer 등록 깜빡 회귀를 **컴파일 시점에 차단**.
//!
//! [`ApiState::admin_api_key`]: crate::routes::ApiState::admin_api_key

use axum::extract::FromRequestParts;
use axum::http::header::AUTHORIZATION;
use axum::http::request::Parts;
use subtle::ConstantTimeEq;

use crate::error::ApiError;
use crate::routes::ApiState;

const BEARER_PREFIX: &str = "Bearer ";

/// API key 인증 게이트 — 보호 라우트 핸들러의 첫 파라미터로 추가한다.
///
/// 예 (의사코드 — `Json<...>`은 실제 타입이 아니므로 doctest 대상 아님):
/// ```text
/// use crate::auth::AdminAuth;
///
/// pub async fn protected_handler(
///     _: AdminAuth,
///     State(state): State<ApiState>,
///     // ... 그 외 extractors
/// ) -> Result<Json<...>, ApiError> { /* ... */ }
/// ```
#[derive(Debug, Clone, Copy)]
pub struct AdminAuth;

impl FromRequestParts<ApiState> for AdminAuth {
    type Rejection = ApiError;

    async fn from_request_parts(
        parts: &mut Parts,
        state: &ApiState,
    ) -> Result<Self, Self::Rejection> {
        let header = parts
            .headers
            .get(AUTHORIZATION)
            .ok_or(ApiError::Unauthorized)?;
        let value = header.to_str().map_err(|_| ApiError::Unauthorized)?;
        let token = value
            .strip_prefix(BEARER_PREFIX)
            .ok_or(ApiError::Unauthorized)?;

        // 상수시간 비교 — subtle::ConstantTimeEq는 슬라이스 길이 불일치 시
        // Choice(0)을 반환하므로 별도 길이 분기 불필요(또한 그 분기 자체가 timing
        // 노출이 될 수 있음).
        if bool::from(token.as_bytes().ct_eq(state.admin_api_key.as_bytes())) {
            Ok(AdminAuth)
        } else {
            Err(ApiError::Unauthorized)
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use axum::http::{HeaderMap, HeaderValue, Method, Request};
    use sqlx::PgPool;

    const TEST_KEY: &str = "test-key-32-bytes-long-aaaaaaaaaa";

    fn test_state() -> ApiState {
        // PgPool::connect_lazy는 실제 connect 시도하지 않음 — extractor가 pool을
        // touch하지 않으므로 단위테스트에서 안전.
        let db_pool =
            PgPool::connect_lazy("postgres://test:test@localhost:5432/test").expect("lazy pool");
        ApiState {
            db_pool,
            admin_api_key: TEST_KEY.into(),
        }
    }

    async fn extract(headers: HeaderMap, state: &ApiState) -> Result<AdminAuth, ApiError> {
        let req = Request::builder()
            .method(Method::GET)
            .uri("/")
            .body(())
            .expect("build request");
        let (mut parts, _) = req.into_parts();
        parts.headers = headers;
        AdminAuth::from_request_parts(&mut parts, state).await
    }

    #[tokio::test]
    async fn missing_header_returns_401() {
        let state = test_state();
        let result = extract(HeaderMap::new(), &state).await;
        assert!(matches!(result, Err(ApiError::Unauthorized)));
    }

    #[tokio::test]
    async fn basic_auth_returns_401() {
        let state = test_state();
        let mut headers = HeaderMap::new();
        headers.insert(AUTHORIZATION, HeaderValue::from_static("Basic xxx"));
        let result = extract(headers, &state).await;
        assert!(matches!(result, Err(ApiError::Unauthorized)));
    }

    #[tokio::test]
    async fn no_prefix_returns_401() {
        let state = test_state();
        let mut headers = HeaderMap::new();
        headers.insert(AUTHORIZATION, HeaderValue::from_str(TEST_KEY).unwrap());
        let result = extract(headers, &state).await;
        assert!(matches!(result, Err(ApiError::Unauthorized)));
    }

    #[tokio::test]
    async fn empty_bearer_token_returns_401() {
        let state = test_state();
        let mut headers = HeaderMap::new();
        headers.insert(AUTHORIZATION, HeaderValue::from_static("Bearer "));
        let result = extract(headers, &state).await;
        // 빈 토큰 → ct_eq("", TEST_KEY) → length mismatch → 401
        assert!(matches!(result, Err(ApiError::Unauthorized)));
    }

    #[tokio::test]
    async fn wrong_key_returns_401() {
        let state = test_state();
        let mut headers = HeaderMap::new();
        headers.insert(
            AUTHORIZATION,
            HeaderValue::from_static("Bearer wrong-key-value-here"),
        );
        let result = extract(headers, &state).await;
        assert!(matches!(result, Err(ApiError::Unauthorized)));
    }

    #[tokio::test]
    async fn correct_key_returns_ok() {
        let state = test_state();
        let mut headers = HeaderMap::new();
        let val = format!("Bearer {TEST_KEY}");
        headers.insert(AUTHORIZATION, HeaderValue::from_str(&val).unwrap());
        let result = extract(headers, &state).await;
        assert!(matches!(result, Ok(AdminAuth)));
    }

    #[tokio::test]
    async fn non_ascii_header_returns_401() {
        let state = test_state();
        let mut headers = HeaderMap::new();
        // 비-ASCII 바이트 — HeaderValue는 받지만 to_str()에서 실패
        headers.insert(
            AUTHORIZATION,
            HeaderValue::from_bytes(&[0xFF, 0xFE, 0xFD]).unwrap(),
        );
        let result = extract(headers, &state).await;
        assert!(matches!(result, Err(ApiError::Unauthorized)));
    }
}
