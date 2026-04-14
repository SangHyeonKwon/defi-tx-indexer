use serde::Serialize;

/// 단일 리소스 API 응답 래퍼.
#[derive(Debug, Serialize)]
pub struct ApiResponse<T: Serialize> {
    /// 응답 데이터
    pub data: T,
}

/// 페이지네이션된 목록 API 응답 래퍼.
#[derive(Debug, Serialize)]
pub struct PaginatedResponse<T: Serialize> {
    /// 응답 데이터 목록
    pub data: Vec<T>,
    /// 페이지네이션 정보
    pub pagination: PaginationInfo,
}

/// 페이지네이션 메타데이터.
#[derive(Debug, Serialize)]
pub struct PaginationInfo {
    /// 요청된 최대 아이템 수
    pub limit: i64,
    /// 건너뛴 아이템 수
    pub offset: i64,
    /// 반환된 아이템 수
    pub count: i64,
}
