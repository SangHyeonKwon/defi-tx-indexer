//! DeFi Analytics DB 크레이트.
//!
//! SQLx 기반으로 PostgreSQL과 상호작용하는 모든 로직을 제공한다.
//! 모델 정의, 배치 INSERT/쿼리 함수, 마이그레이션 실행을 포함한다.

pub mod error;
pub mod models;
pub mod queries;

use sqlx::postgres::PgPoolOptions;
use sqlx::PgPool;

use crate::error::DbError;

/// 데이터베이스 연결 풀을 생성한다.
///
/// `database_url`은 `postgres://user:pass@host:port/dbname` 형식이어야 한다.
pub async fn create_pool(database_url: &str, max_connections: u32) -> Result<PgPool, DbError> {
    let pool = PgPoolOptions::new()
        .max_connections(max_connections)
        .connect(database_url)
        .await?;
    Ok(pool)
}

/// 내장된 마이그레이션을 실행한다.
pub async fn run_migrations(pool: &PgPool) -> Result<(), DbError> {
    sqlx::migrate!("../../migrations").run(pool).await?;
    Ok(())
}
