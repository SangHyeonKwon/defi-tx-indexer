use axum::extract::{Query, State};
use axum::Json;
use sqlx::PgPool;

use crate::error::ApiError;
use crate::pagination::PaginationParams;
use crate::response::{PaginatedResponse, PaginationInfo};

/// 스왑 이벤트 필터 쿼리 파라미터.
#[derive(serde::Deserialize)]
pub struct SwapFilter {
    /// 풀 주소 필터 (선택)
    pub pool_address: Option<String>,
    /// 페이지네이션 limit
    pub limit: Option<i64>,
    /// 페이지네이션 offset
    pub offset: Option<i64>,
}

/// 스왑 이벤트를 필터+페이지네이션으로 조회한다.
pub async fn list_swaps(
    State(pool): State<PgPool>,
    Query(filter): Query<SwapFilter>,
) -> Result<Json<PaginatedResponse<db::models::SwapEvent>>, ApiError> {
    let params = PaginationParams {
        limit: filter.limit,
        offset: filter.offset,
    };
    let limit = params.limit();
    let offset = params.offset();

    let swaps =
        db::queries::list_swap_events(&pool, filter.pool_address.as_deref(), limit, offset).await?;
    let count = swaps.len() as i64;

    Ok(Json(PaginatedResponse {
        data: swaps,
        pagination: PaginationInfo {
            limit,
            offset,
            count,
        },
    }))
}
