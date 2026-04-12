use bigdecimal::BigDecimal;
use chrono::{DateTime, Utc};
use serde::Serialize;

// ============================================
// ENUM 타입 (PostgreSQL ENUM ↔ Rust)
// ============================================

/// 실패한 트랜잭션 에러 카테고리.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, sqlx::Type)]
#[sqlx(type_name = "error_category", rename_all = "SCREAMING_SNAKE_CASE")]
pub enum ErrorCategory {
    InsufficientBalance,
    SlippageExceeded,
    DeadlineExpired,
    Unauthorized,
    TransferFailed,
    Unknown,
}

/// 유동성 이벤트 타입 (Mint 또는 Burn).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, sqlx::Type)]
#[sqlx(
    type_name = "liquidity_event_type",
    rename_all = "SCREAMING_SNAKE_CASE"
)]
pub enum LiquidityEventType {
    Mint,
    Burn,
}

/// 가격 스냅샷 시간 간격.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, sqlx::Type)]
#[sqlx(type_name = "snapshot_interval")]
pub enum SnapshotInterval {
    #[sqlx(rename = "1m")]
    OneMinute,
    #[sqlx(rename = "5m")]
    FiveMinutes,
    #[sqlx(rename = "15m")]
    FifteenMinutes,
    #[sqlx(rename = "1h")]
    OneHour,
    #[sqlx(rename = "4h")]
    FourHours,
    #[sqlx(rename = "1d")]
    OneDay,
}

// ============================================
// 테이블 모델 (11개)
// ============================================

/// 이더리움 블록.
#[derive(Debug, Clone, Serialize, sqlx::FromRow)]
pub struct Block {
    /// 블록 번호 (PK)
    pub block_number: i64,
    /// 블록 타임스탬프
    pub timestamp: DateTime<Utc>,
    /// 블록에서 사용된 총 가스
    pub gas_used: i64,
}

/// ERC-20 토큰 메타데이터.
#[derive(Debug, Clone, Serialize, sqlx::FromRow)]
pub struct Token {
    /// 토큰 컨트랙트 주소 (0x..., 42자)
    pub token_address: String,
    /// 토큰 심볼 (e.g. "WETH")
    pub symbol: String,
    /// 토큰 이름
    pub name: String,
    /// 소수점 자릿수 (기본 18)
    pub decimals: i16,
}

/// 이더리움 트랜잭션.
#[derive(Debug, Clone, Serialize, sqlx::FromRow)]
pub struct Transaction {
    /// 트랜잭션 해시 (0x..., 66자)
    pub tx_hash: String,
    /// 송신자 주소
    pub from_addr: String,
    /// 수신자 주소 (None = 컨트랙트 생성)
    pub to_addr: Option<String>,
    /// 포함된 블록 번호
    pub block_number: i64,
    /// 사용된 가스
    pub gas_used: i64,
    /// 가스 가격 (wei)
    pub gas_price: BigDecimal,
    /// 전송 값 (wei)
    pub value: BigDecimal,
    /// 상태 (1=성공, 0=실패)
    pub status: i16,
    /// 입력 데이터
    pub input_data: Option<String>,
}

/// Uniswap V3 유동성 풀.
#[derive(Debug, Clone, Serialize, sqlx::FromRow)]
pub struct Pool {
    /// 풀 컨트랙트 주소
    pub pool_address: String,
    /// 토큰 쌍 이름 (e.g. "WETH/USDC")
    pub pair_name: String,
    /// token0 주소
    pub token0_address: String,
    /// token1 주소
    pub token1_address: String,
    /// 수수료 티어 (100, 500, 3000, 10000 bps)
    pub fee_tier: i32,
    /// 풀 생성 시각
    pub created_at: DateTime<Utc>,
}

/// Uniswap V3 스왑 이벤트.
#[derive(Debug, Clone, Serialize, sqlx::FromRow)]
pub struct SwapEvent {
    /// 풀 주소
    pub pool_address: String,
    /// 트랜잭션 해시
    pub tx_hash: String,
    /// 스왑 발신자
    pub sender: String,
    /// 스왑 수신자
    pub recipient: String,
    /// token0 변동량 (부호 있음)
    pub amount0: BigDecimal,
    /// token1 변동량 (부호 있음)
    pub amount1: BigDecimal,
    /// 유입 토큰량
    pub amount_in: BigDecimal,
    /// 유출 토큰량
    pub amount_out: BigDecimal,
    /// Uniswap V3 가격 인코딩
    pub sqrt_price_x96: BigDecimal,
    /// 풀 유동성
    pub liquidity: BigDecimal,
    /// 틱 값
    pub tick: i32,
    /// 로그 인덱스
    pub log_index: i32,
    /// 이벤트 타임스탬프
    pub timestamp: DateTime<Utc>,
    /// 자동 생성 ID
    pub event_id: i64,
}

/// 유동성 공급/회수 이벤트 (Mint/Burn).
#[derive(Debug, Clone, Serialize, sqlx::FromRow)]
pub struct LiquidityEvent {
    /// 이벤트 타입
    pub event_type: LiquidityEventType,
    /// 풀 주소
    pub pool_address: String,
    /// 트랜잭션 해시
    pub tx_hash: String,
    /// 유동성 공급자 주소
    pub provider: String,
    /// token0 수량
    pub token0_amount: BigDecimal,
    /// token1 수량
    pub token1_amount: BigDecimal,
    /// 하한 틱
    pub tick_lower: i32,
    /// 상한 틱
    pub tick_upper: i32,
    /// 유동성 양
    pub liquidity: BigDecimal,
    /// 로그 인덱스
    pub log_index: i32,
    /// 이벤트 타임스탬프
    pub timestamp: DateTime<Utc>,
    /// 자동 생성 ID
    pub event_id: i64,
}

/// ERC-20 토큰 전송.
#[derive(Debug, Clone, Serialize, sqlx::FromRow)]
pub struct TokenTransfer {
    /// 트랜잭션 해시
    pub tx_hash: String,
    /// 토큰 주소
    pub token_address: String,
    /// 송신자
    pub from_addr: String,
    /// 수신자
    pub to_addr: String,
    /// 전송량
    pub amount: BigDecimal,
    /// 로그 인덱스
    pub log_index: i32,
    /// 전송 타임스탬프
    pub timestamp: DateTime<Utc>,
    /// 자동 생성 ID
    pub transfer_id: i64,
}

/// 실패한 트랜잭션 상세.
#[derive(Debug, Clone, Serialize, sqlx::FromRow)]
pub struct FailedTransaction {
    /// 트랜잭션 해시 (PK, FK → transaction)
    pub tx_hash: String,
    /// 에러 카테고리
    pub error_category: ErrorCategory,
    /// 리버트 사유 (디코딩된 텍스트)
    pub revert_reason: Option<String>,
    /// 실패한 함수명
    pub failing_function: Option<String>,
    /// 사용된 가스
    pub gas_used: i64,
    /// 실패 타임스탬프
    pub timestamp: DateTime<Utc>,
}

/// 풀 가격 스냅샷.
#[derive(Debug, Clone, Serialize, sqlx::FromRow)]
pub struct PriceSnapshot {
    /// 풀 주소
    pub pool_address: String,
    /// 가격 (고정밀)
    pub price: BigDecimal,
    /// 틱 값
    pub tick: i32,
    /// 유동성
    pub liquidity: BigDecimal,
    /// 스냅샷 시각
    pub snapshot_ts: DateTime<Utc>,
    /// 스냅샷 간격
    pub interval_type: SnapshotInterval,
    /// 자동 생성 ID
    pub snapshot_id: i64,
}

/// 유저 프로필 (집계 테이블).
#[derive(Debug, Clone, Serialize, sqlx::FromRow)]
pub struct UserProfile {
    /// 유저 지갑 주소
    pub user_address: String,
    /// 라벨 (whale, bot, retail 등)
    pub label: Option<String>,
    /// 최초 활동 시각
    pub first_seen: DateTime<Utc>,
    /// 최근 활동 시각
    pub last_seen: DateTime<Utc>,
    /// 총 스왑 횟수
    pub total_swaps: i32,
    /// 총 거래량 (USD)
    pub total_volume_usd: BigDecimal,
}

/// 트랜잭션 내부 호출 트레이스.
#[derive(Debug, Clone, Serialize, sqlx::FromRow)]
pub struct TraceLog {
    /// 트랜잭션 해시
    pub tx_hash: String,
    /// 호출 깊이
    pub call_depth: i32,
    /// 호출 타입 (CALL, DELEGATECALL, STATICCALL, CREATE, CREATE2)
    pub call_type: String,
    /// 호출자 주소
    pub from_addr: String,
    /// 대상 주소 (None = CREATE)
    pub to_addr: Option<String>,
    /// 전송 값 (wei)
    pub value: BigDecimal,
    /// 사용된 가스
    pub gas_used: i64,
    /// 입력 데이터
    pub input: Option<String>,
    /// 출력 데이터
    pub output: Option<String>,
    /// 에러 메시지
    pub error: Option<String>,
    /// 자동 생성 ID
    pub trace_id: i64,
}
