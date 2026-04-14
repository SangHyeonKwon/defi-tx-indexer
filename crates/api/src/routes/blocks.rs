use axum::extract::{Path, State};
use axum::Json;
use sqlx::PgPool;

use crate::error::ApiError;
use crate::response::ApiResponse;

/// 최신 인덱싱된 블록 번호를 반환한다.
pub async fn get_latest_block(
    State(pool): State<PgPool>,
) -> Result<Json<ApiResponse<Option<i64>>>, ApiError> {
    let latest = db::queries::get_latest_block_number(&pool).await?;
    Ok(Json(ApiResponse { data: latest }))
}

/// 블록 번호로 단일 블록을 조회한다.
pub async fn get_block(
    State(pool): State<PgPool>,
    Path(number): Path<i64>,
) -> Result<Json<ApiResponse<db::models::Block>>, ApiError> {
    let block = db::queries::get_block_by_number(&pool, number).await?;
    Ok(Json(ApiResponse { data: block }))
}
