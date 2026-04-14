use axum::extract::State;
use axum::Json;
use sqlx::PgPool;

use crate::error::ApiError;

/// 헬스체크 응답.
#[derive(serde::Serialize)]
pub struct HealthResponse {
    /// 서비스 상태
    pub status: &'static str,
    /// DB 연결 상태
    pub database: &'static str,
}

/// 서버 헬스체크 엔드포인트.
///
/// DB 연결을 확인하고 상태를 반환한다.
pub async fn health_check(State(pool): State<PgPool>) -> Result<Json<HealthResponse>, ApiError> {
    sqlx::query("SELECT 1").execute(&pool).await.map_err(|e| {
        tracing::error!(error = %e, "health check failed");
        ApiError::Internal("database unreachable".into())
    })?;

    Ok(Json(HealthResponse {
        status: "ok",
        database: "connected",
    }))
}
