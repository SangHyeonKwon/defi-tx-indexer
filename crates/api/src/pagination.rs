use serde::Deserialize;

/// 페이지네이션 쿼리 파라미터.
///
/// `?limit=20&offset=0` 형태로 URL에서 추출된다.
/// limit 기본값 20, 최대 100. offset 기본값 0.
#[derive(Debug, Deserialize)]
pub struct PaginationParams {
    /// 최대 반환 아이템 수 (기본 20, 최대 100)
    pub limit: Option<i64>,
    /// 건너뛸 아이템 수 (기본 0)
    pub offset: Option<i64>,
}

impl PaginationParams {
    /// 유효한 limit 값을 반환한다.
    pub fn limit(&self) -> i64 {
        self.limit.unwrap_or(20).clamp(1, 100)
    }

    /// 유효한 offset 값을 반환한다.
    pub fn offset(&self) -> i64 {
        self.offset.unwrap_or(0).max(0)
    }
}
