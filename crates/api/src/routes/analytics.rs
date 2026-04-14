use axum::extract::{Query, State};
use axum::Json;
use sqlx::PgPool;

use crate::error::ApiError;
use crate::pagination::PaginationParams;
use crate::response::{ApiResponse, PaginatedResponse, PaginationInfo};

/// 일별 스왑 볼륨 필터 쿼리 파라미터.
#[derive(serde::Deserialize)]
pub struct DailyVolumeFilter {
    /// 풀 주소 필터 (선택)
    pub pool_address: Option<String>,
    /// 페이지네이션 limit
    pub limit: Option<i64>,
    /// 페이지네이션 offset
    pub offset: Option<i64>,
}

/// 일별 스왑 볼륨을 조회한다 (vw_daily_swap_volume).
pub async fn daily_volume(
    State(pool): State<PgPool>,
    Query(filter): Query<DailyVolumeFilter>,
) -> Result<Json<PaginatedResponse<db::models::DailySwapVolume>>, ApiError> {
    let params = PaginationParams {
        limit: filter.limit,
        offset: filter.offset,
    };
    let limit = params.limit();
    let offset = params.offset();

    let volumes =
        db::queries::get_daily_swap_volume(&pool, filter.pool_address.as_deref(), limit, offset)
            .await?;
    let count = volumes.len() as i64;

    Ok(Json(PaginatedResponse {
        data: volumes,
        pagination: PaginationInfo {
            limit,
            offset,
            count,
        },
    }))
}

/// 실패 TX 카테고리별 분석을 조회한다 (vw_failed_tx_analysis).
pub async fn failed_tx_analysis(
    State(pool): State<PgPool>,
) -> Result<Json<ApiResponse<Vec<db::models::FailedTxAnalysis>>>, ApiError> {
    let analysis = db::queries::get_failed_tx_analysis(&pool).await?;
    Ok(Json(ApiResponse { data: analysis }))
}
