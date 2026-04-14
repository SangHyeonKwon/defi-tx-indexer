use std::env;

/// API 서버 설정.
///
/// 환경변수에서 로드한다.
#[derive(Debug, Clone)]
pub struct ApiConfig {
    /// PostgreSQL 연결 문자열
    pub database_url: String,
    /// 서버 바인딩 호스트
    pub host: String,
    /// 서버 바인딩 포트
    pub port: u16,
    /// DB 연결 풀 최대 크기
    pub max_db_connections: u32,
}

impl ApiConfig {
    /// 환경변수에서 설정을 로드한다.
    ///
    /// 필수: `DATABASE_URL`
    /// 선택: `API_HOST` (기본 0.0.0.0), `API_PORT` (기본 3000), `MAX_DB_CONNECTIONS` (기본 10)
    pub fn from_env() -> anyhow::Result<Self> {
        let database_url = env::var("DATABASE_URL")
            .map_err(|_| anyhow::anyhow!("DATABASE_URL environment variable is required"))?;

        Ok(Self {
            database_url,
            host: env::var("API_HOST").unwrap_or_else(|_| "0.0.0.0".to_string()),
            port: env::var("API_PORT")
                .ok()
                .and_then(|v| v.parse().ok())
                .unwrap_or(3000),
            max_db_connections: env::var("MAX_DB_CONNECTIONS")
                .ok()
                .and_then(|v| v.parse().ok())
                .unwrap_or(10),
        })
    }
}
