use std::env;

/// 인덱서 실행 설정.
///
/// 환경변수와 CLI 인자에서 설정값을 로드한다.
#[derive(Debug, Clone)]
pub struct Config {
    /// PostgreSQL 연결 문자열
    pub database_url: String,
    /// 이더리움 RPC 엔드포인트
    pub rpc_url: String,
    /// WebSocket 엔드포인트 (Phase 3에서 실시간 구독에 사용)
    #[allow(dead_code)]
    pub ws_url: Option<String>,
    /// 시작 블록 번호
    pub from_block: u64,
    /// 종료 블록 번호 (None이면 최신 블록까지)
    pub to_block: Option<u64>,
    /// 동시 처리할 최대 블록 수
    pub max_concurrent_blocks: usize,
    /// 배치 INSERT 크기
    pub batch_size: usize,
    /// DB 연결 풀 최대 크기
    pub max_db_connections: u32,
}

impl Config {
    /// 환경변수에서 설정을 로드한다.
    ///
    /// 필수 변수: `DATABASE_URL`, `RPC_URL`
    /// 선택 변수: `WS_URL`, `MAX_CONCURRENT_BLOCKS`, `BATCH_SIZE`
    pub fn from_env() -> anyhow::Result<Self> {
        let database_url = env::var("DATABASE_URL")
            .map_err(|_| anyhow::anyhow!("DATABASE_URL environment variable is required"))?;
        let rpc_url = env::var("RPC_URL")
            .map_err(|_| anyhow::anyhow!("RPC_URL environment variable is required"))?;
        let ws_url = env::var("WS_URL").ok();

        let max_concurrent_blocks = env::var("MAX_CONCURRENT_BLOCKS")
            .ok()
            .and_then(|v| v.parse().ok())
            .unwrap_or(10);

        let batch_size = env::var("BATCH_SIZE")
            .ok()
            .and_then(|v| v.parse().ok())
            .unwrap_or(100);

        Ok(Self {
            database_url,
            rpc_url,
            ws_url,
            from_block: 0,
            to_block: None,
            max_concurrent_blocks,
            batch_size,
            max_db_connections: env::var("MAX_DB_CONNECTIONS")
                .ok()
                .and_then(|v| v.parse().ok())
                .unwrap_or(10),
        })
    }

    /// CLI 인자로 블록 범위를 오버라이드한다.
    pub fn with_block_range(mut self, from: u64, to: Option<u64>) -> Self {
        self.from_block = from;
        self.to_block = to;
        self
    }
}
