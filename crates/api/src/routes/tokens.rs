use axum::extract::{Path, Query, State};
use axum::Json;
use sqlx::PgPool;

use crate::error::ApiError;
use crate::pagination::PaginationParams;
use crate::response::{ApiResponse, PaginatedResponse, PaginationInfo};

/// 토큰 목록을 페이지네이션하여 조회한다.
pub async fn list_tokens(
    State(pool): State<PgPool>,
    Query(params): Query<PaginationParams>,
) -> Result<Json<PaginatedResponse<db::models::Token>>, ApiError> {
    let limit = params.limit();
    let offset = params.offset();
    let tokens = db::queries::list_tokens(&pool, limit, offset).await?;
    let count = tokens.len() as i64;

    Ok(Json(PaginatedResponse {
        data: tokens,
        pagination: PaginationInfo {
            limit,
            offset,
            count,
        },
    }))
}

/// 주소로 단일 토큰을 조회한다.
pub async fn get_token(
    State(pool): State<PgPool>,
    Path(address): Path<String>,
) -> Result<Json<ApiResponse<db::models::Token>>, ApiError> {
    let token = db::queries::get_token_by_address(&pool, &address).await?;
    Ok(Json(ApiResponse { data: token }))
}
