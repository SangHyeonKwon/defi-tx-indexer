use axum::extract::{Query, State};
use axum::Json;
use sqlx::PgPool;

use crate::error::ApiError;
use crate::response::{PaginatedResponse, PaginationInfo};

/// 트레이더 랭킹 쿼리 파라미터.
#[derive(serde::Deserialize)]
pub struct TraderQuery {
    /// 반환할 트레이더 수 (기본 10, 최대 100)
    pub limit: Option<i64>,
}

/// 거래량 기준 트레이더 랭킹을 조회한다.
pub async fn get_top_traders(
    State(pool): State<PgPool>,
    Query(params): Query<TraderQuery>,
) -> Result<Json<PaginatedResponse<db::models::TopTrader>>, ApiError> {
    let limit = params.limit.unwrap_or(10).clamp(1, 100);
    let traders = db::queries::get_top_traders(&pool, limit).await?;
    let count = traders.len() as i64;

    Ok(Json(PaginatedResponse {
        data: traders,
        pagination: PaginationInfo {
            limit,
            offset: 0,
            count,
        },
    }))
}
