/// DB 크레이트 에러 타입.
///
/// 모든 데이터베이스 관련 에러를 단일 enum으로 집약한다.
#[derive(Debug, thiserror::Error)]
pub enum DbError {
    /// SQLx 쿼리 또는 연결 에러
    #[error("database error: {0}")]
    Sqlx(#[from] sqlx::Error),

    /// SQLx 마이그레이션 에러
    #[error("migration error: {0}")]
    Migration(#[from] sqlx::migrate::MigrateError),

    /// 레코드를 찾을 수 없음
    #[error("record not found: {0}")]
    NotFound(String),

    /// 중복 레코드 삽입 시도
    #[error("duplicate record: {0}")]
    Duplicate(String),
}
