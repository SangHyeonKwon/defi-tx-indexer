//! CI/로컬용 마이그레이션 러너.
//!
//! `DATABASE_URL`이 가리키는 PostgreSQL에 `db::run_migrations`(sqlx `migrate!`)를
//! 적용한다. psql로 마이그레이션 파일을 직접 돌리면 `_sqlx_migrations` 레저가
//! 남지 않아 이후 `run_migrations` 호출(통합 테스트 등)이 전체를 재적용하게 되므로,
//! CI에서도 프로덕션/테스트와 동일한 이 코드 경로를 사용한다.
//!
//! 사용법: `DATABASE_URL=postgres://... cargo run -p db --example migrate`

use anyhow::Context;

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let database_url =
        std::env::var("DATABASE_URL").context("DATABASE_URL environment variable is required")?;
    let pool = db::create_pool(&database_url, 1)
        .await
        .context("failed to connect to database")?;
    db::run_migrations(&pool)
        .await
        .context("failed to run migrations")?;
    Ok(())
}
