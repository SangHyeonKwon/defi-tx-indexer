use axum::extract::{Path, Query, State};
use axum::Json;
use chrono::{DateTime, Utc};
use sqlx::PgPool;

use crate::error::ApiError;
use crate::pagination::PaginationParams;
use crate::response::{ApiResponse, PaginatedResponse, PaginationInfo};

/// 풀 목록을 페이지네이션하여 조회한다.
pub async fn list_pools(
    State(pool): State<PgPool>,
    Query(params): Query<PaginationParams>,
) -> Result<Json<PaginatedResponse<db::models::Pool>>, ApiError> {
    let limit = params.limit();
    let offset = params.offset();
    let pools = db::queries::list_pools(&pool, limit, offset).await?;
    let count = pools.len() as i64;

    Ok(Json(PaginatedResponse {
        data: pools,
        pagination: PaginationInfo {
            limit,
            offset,
            count,
        },
    }))
}

/// 주소로 단일 풀을 조회한다.
pub async fn get_pool(
    State(pool): State<PgPool>,
    Path(address): Path<String>,
) -> Result<Json<ApiResponse<db::models::Pool>>, ApiError> {
    let p = db::queries::get_pool_by_address(&pool, &address).await?;
    Ok(Json(ApiResponse { data: p }))
}

/// 풀 종합 통계 쿼리 파라미터.
#[derive(serde::Deserialize)]
pub struct PoolStatsQuery {
    /// 시작 날짜 (ISO 8601)
    pub from_date: DateTime<Utc>,
    /// 종료 날짜 (ISO 8601)
    pub to_date: DateTime<Utc>,
}

/// 풀 종합 통계를 날짜 범위로 조회한다.
pub async fn get_pool_stats(
    State(pool): State<PgPool>,
    Path(address): Path<String>,
    Query(params): Query<PoolStatsQuery>,
) -> Result<Json<ApiResponse<db::models::PoolStats>>, ApiError> {
    let stats =
        db::queries::get_pool_stats(&pool, &address, params.from_date, params.to_date).await?;
    Ok(Json(ApiResponse { data: stats }))
}
