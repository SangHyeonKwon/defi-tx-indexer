use axum::http::StatusCode;
use axum::response::{IntoResponse, Response};
use axum::Json;

/// API 에러 타입.
///
/// `db::error::DbError`를 HTTP 응답으로 변환한다.
#[derive(Debug, thiserror::Error)]
pub enum ApiError {
    /// 리소스를 찾을 수 없음 (404)
    #[error("not found: {0}")]
    NotFound(String),

    /// 잘못된 요청 (400)
    #[error("bad request: {0}")]
    BadRequest(String),

    /// 내부 서버 에러 (500)
    #[error("internal error: {0}")]
    Internal(String),
}

impl From<db::error::DbError> for ApiError {
    fn from(err: db::error::DbError) -> Self {
        match err {
            db::error::DbError::NotFound(msg) => ApiError::NotFound(msg),
            db::error::DbError::Duplicate(msg) => ApiError::BadRequest(msg),
            db::error::DbError::Sqlx(e) => {
                tracing::error!(error = %e, "database error");
                ApiError::Internal("database error".into())
            }
            db::error::DbError::Migration(e) => {
                tracing::error!(error = %e, "migration error");
                ApiError::Internal("internal error".into())
            }
        }
    }
}

impl IntoResponse for ApiError {
    fn into_response(self) -> Response {
        let (status, message) = match &self {
            ApiError::NotFound(msg) => (StatusCode::NOT_FOUND, msg.clone()),
            ApiError::BadRequest(msg) => (StatusCode::BAD_REQUEST, msg.clone()),
            ApiError::Internal(msg) => (StatusCode::INTERNAL_SERVER_ERROR, msg.clone()),
        };
        let body = Json(serde_json::json!({ "error": message }));
        (status, body).into_response()
    }
}
