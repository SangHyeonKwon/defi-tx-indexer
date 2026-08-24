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
    /// S12.1: ERC-20 allowance 부족 — `INSUFFICIENT_BALANCE`의 *진단 메시지가 다름*
    /// (approve를 호출해야 함). classifier가 `"allowance"` 패턴 매칭.
    InsufficientAllowance,
    SlippageExceeded,
    /// S12.1: 매수 슬리피지 — `"too little received"`. fallback은 `SLIPPAGE_EXCEEDED`.
    SlippageAmountOut,
    /// S12.1: 매도 슬리피지 — `"too much requested"`. fallback은 `SLIPPAGE_EXCEEDED`.
    SlippageAmountIn,
    /// S12.1: 풀 가격 영향 한도 초과 — `"price slipped"` / `"amount out"`. fallback은
    /// `SLIPPAGE_EXCEEDED`.
    SlippagePriceImpact,
    DeadlineExpired,
    Unauthorized,
    TransferFailed,
    Unknown,
}

impl std::str::FromStr for ErrorCategory {
    type Err = ();

    /// 와이어 표현(SCREAMING_SNAKE_CASE) 문자열을 파싱한다.
    ///
    /// API 쿼리 파라미터 → 타입 변환의 단일 출처. 알 수 없는 값은 `Err(())`.
    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s {
            "INSUFFICIENT_BALANCE" => Ok(Self::InsufficientBalance),
            "INSUFFICIENT_ALLOWANCE" => Ok(Self::InsufficientAllowance),
            "SLIPPAGE_EXCEEDED" => Ok(Self::SlippageExceeded),
            "SLIPPAGE_AMOUNT_OUT" => Ok(Self::SlippageAmountOut),
            "SLIPPAGE_AMOUNT_IN" => Ok(Self::SlippageAmountIn),
            "SLIPPAGE_PRICE_IMPACT" => Ok(Self::SlippagePriceImpact),
            "DEADLINE_EXPIRED" => Ok(Self::DeadlineExpired),
            "UNAUTHORIZED" => Ok(Self::Unauthorized),
            "TRANSFER_FAILED" => Ok(Self::TransferFailed),
            "UNKNOWN" => Ok(Self::Unknown),
            _ => Err(()),
        }
    }
}

impl ErrorCategory {
    /// SCREAMING_SNAKE_CASE 와이어 표현 문자열을 반환한다.
    ///
    /// DB enum 컬럼 / API 쿼리 파라미터 / `category_diagnosis` 시드 키 모두 같은
    /// 와이어 형태를 쓴다 — 호출 측이 그 변환을 자체 구현하지 않도록 단일 출처.
    pub fn as_wire(&self) -> &'static str {
        match self {
            Self::InsufficientBalance => "INSUFFICIENT_BALANCE",
            Self::InsufficientAllowance => "INSUFFICIENT_ALLOWANCE",
            Self::SlippageExceeded => "SLIPPAGE_EXCEEDED",
            Self::SlippageAmountOut => "SLIPPAGE_AMOUNT_OUT",
            Self::SlippageAmountIn => "SLIPPAGE_AMOUNT_IN",
            Self::SlippagePriceImpact => "SLIPPAGE_PRICE_IMPACT",
            Self::DeadlineExpired => "DEADLINE_EXPIRED",
            Self::Unauthorized => "UNAUTHORIZED",
            Self::TransferFailed => "TRANSFER_FAILED",
            Self::Unknown => "UNKNOWN",
        }
    }
}

/// 시계열 버킷 단위 — `date_trunc`에 쓰일 화이트리스트(인젝션 방지).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum TimeBucket {
    Hour,
    Day,
    Week,
}

impl TimeBucket {
    /// `date_trunc` 1번째 인자에 **바인딩**할 고정 텍스트 (사용자 입력 보간 아님).
    pub fn as_pg(&self) -> &'static str {
        match self {
            Self::Hour => "hour",
            Self::Day => "day",
            Self::Week => "week",
        }
    }
}

impl std::str::FromStr for TimeBucket {
    type Err = ();

    /// `hour|day|week` 파싱. 알 수 없는 값은 `Err(())` (API에서 400).
    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s {
            "hour" => Ok(Self::Hour),
            "day" => Ok(Self::Day),
            "week" => Ok(Self::Week),
            _ => Err(()),
        }
    }
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
    /// 블록 해시 (reorg 감지용; S06 이전 행은 NULL)
    pub block_hash: Option<String>,
    /// 부모 블록 해시 (fork point 탐지용; S06 이전 행은 NULL)
    pub parent_hash: Option<String>,
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

/// 인덱서 체크포인트 — 크래시 복구용.
#[derive(Debug, Clone, Serialize, sqlx::FromRow)]
pub struct IndexerCheckpoint {
    /// 자동 생성 ID
    pub checkpoint_id: i32,
    /// 체인 ID (1 = Ethereum mainnet)
    pub chain_id: i32,
    /// 마지막으로 완전히 처리된 블록 번호
    pub last_processed_block: i64,
    /// 마지막 갱신 시각
    pub updated_at: DateTime<Utc>,
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

// ============================================
// 뷰 기반 모델 (API 응답용)
// ============================================

/// 일별 풀별 스왑 볼륨 (vw_daily_swap_volume).
#[derive(Debug, Clone, Serialize, sqlx::FromRow)]
pub struct DailySwapVolume {
    /// 풀 주소
    pub pool_address: String,
    /// 풀 페어 이름
    pub pair_name: String,
    /// 스왑 날짜
    pub swap_date: chrono::NaiveDate,
    /// 스왑 건수
    pub swap_count: i64,
    /// 총 유입량
    pub total_amount_in: BigDecimal,
    /// 총 유출량
    pub total_amount_out: BigDecimal,
}

/// 트레이더 랭킹 (vw_top_traders).
#[derive(Debug, Clone, Serialize, sqlx::FromRow)]
pub struct TopTrader {
    /// 트레이더 주소
    pub user_address: String,
    /// 사용자 라벨 (whale, bot, retail)
    pub label: Option<String>,
    /// 총 스왑 횟수
    pub total_swaps: i32,
    /// 총 거래량 (USD)
    pub total_volume_usd: BigDecimal,
    /// 거래량 기준 순위
    pub volume_rank: i64,
}

/// 실패 TX 카테고리별 분석 (vw_failed_tx_analysis).
#[derive(Debug, Clone, Serialize, sqlx::FromRow)]
pub struct FailedTxAnalysis {
    /// 에러 카테고리
    pub error_category: ErrorCategory,
    /// 실패 건수
    pub failure_count: i64,
    /// 평균 낭비 가스
    pub avg_gas_wasted: BigDecimal,
    /// 전체 대비 비율 (%)
    pub pct_of_total: BigDecimal,
    /// 가장 최근 실패 시각
    pub most_recent_failure: DateTime<Utc>,
}

/// UNKNOWN 실패의 revert 사유 클러스터 한 행 (vw_unknown_revert_clusters).
///
/// classifier가 분류하지 못한 실패를 `fn_revert_template` 정규화 키로 묶어
/// 빈도·낭비 가스 순으로 랭킹한다 — 상위 클러스터가 classifier 신규 룰 후보.
/// `sample_revert_reason` / `sample_tx_hash`는 각각 독립적인 `MIN`이라 같은
/// 행에서 나온 값이 아닐 수 있다(대표 예시 용도).
#[derive(Debug, Clone, Serialize, sqlx::FromRow)]
pub struct UnknownRevertCluster {
    /// 정규화된 revert 템플릿 (클러스터 키)
    pub template: String,
    /// 클러스터 종류: NO_DATA | CUSTOM_ERROR | PANIC | TEXT
    pub cluster_kind: String,
    /// 발생 건수
    pub occurrences: i64,
    /// 전체 UNKNOWN 대비 비율 (%)
    pub pct_of_unknown: BigDecimal,
    /// 총 낭비 가스 (`SUM(BIGINT)` → NUMERIC)
    pub total_gas_wasted: BigDecimal,
    /// 평균 낭비 가스
    pub avg_gas_wasted: BigDecimal,
    /// 고유 발신자 수
    pub distinct_senders: i64,
    /// 고유 함수 셀렉터 수
    pub distinct_selectors: i64,
    /// 대표 revert 사유 (독립 MIN — 예시 용도)
    pub sample_revert_reason: Option<String>,
    /// 대표 tx 해시 (독립 MIN — 예시 용도)
    pub sample_tx_hash: Option<String>,
    /// 최초 발생 시각
    pub first_seen: DateTime<Utc>,
    /// 최근 발생 시각
    pub last_seen: DateTime<Utc>,
}

/// 풀 종합 통계 (fn_get_pool_stats).
#[derive(Debug, Clone, Serialize, sqlx::FromRow)]
pub struct PoolStats {
    /// 페어 이름
    pub pair_name: String,
    /// 스왑 건수
    pub swap_count: i64,
    /// 고유 트레이더 수
    pub unique_traders: i64,
    /// 총 유입 볼륨
    pub total_volume_in: BigDecimal,
    /// 평균 거래 크기
    pub avg_trade_size: BigDecimal,
    /// 유동성 이벤트 수
    pub liquidity_events: i64,
    /// 추정 수수료 수익
    pub estimated_fees: BigDecimal,
}

/// 단건 실패 트랜잭션 진단 결과.
///
/// API 조립용 합성 구조체 — 테이블/뷰가 아니다. `failed_transaction` 1행과
/// 해당 tx의 평탄화된 `trace_log` 콜트리(1:N), `root_cause`(첫 error frame),
/// 그리고 `failing_function_decoded`(selector → 이름/시그니처)를 함께 담는다.
#[derive(Debug, Clone, Serialize)]
pub struct FailedTxDetail {
    /// 실패 트랜잭션 메타 + 분류 결과
    pub failed: FailedTransaction,
    /// 평탄화된 콜 프레임 (`trace_id` 오름차순 = pre-order DFS)
    pub call_tree: Vec<TraceLog>,
    /// `call_tree`가 상한에서 잘렸으면 `true` (부분 응답 신호)
    pub call_tree_truncated: bool,
    /// 실제 revert가 발생한 trace frame — `trace_log`에서 `error IS NOT NULL`인
    /// 가장 빠른(`trace_id ASC`) 1행 (= pre-order DFS 첫 error). 매칭 frame이
    /// 없으면 `null` (S10 / M004; silent default 금지 — 명시 `null` 노출).
    pub root_cause: Option<TraceLog>,
    /// `failed.failing_function` selector를 자기소유 ABI 시드(`function_signature`)
    /// 와 lookup해 사람이 읽는 함수명/시그니처로 가산 (S11 / M004). 매칭이 없거나
    /// `failing_function` 자체가 `None`이면 `null` (silent default 금지 — D015/D014).
    /// S11.1: `args`까지 디코드 — `call_tree` root frame(`call_depth = 0`)의 input
    /// bytes를 signature와 함께 ABI 디코드.
    pub failing_function_decoded: Option<DecodedFunction>,
    /// `root_cause` frame의 `input` bytes를 자기소유 ABI 시드로 디코드 (S11.1).
    /// 첫 4바이트(selector) lookup → `DecodedFunction` 합성 + 나머지 args 디코드.
    /// `root_cause`가 `null`, `input`이 `null`, selector 시드 미존재 모두 명시
    /// `null` (silent default 금지 — D027/D014). 정직히: 실패 frame이 *서로 다른*
    /// 함수를 호출했을 때(예: top-level은 `swap`, revert는 내부 `transfer`)에
    /// 가장 유용 — `failing_function_decoded`와 *다를 수 있다*.
    pub root_cause_decoded: Option<DecodedFunction>,
    /// `failed.error_category` wire form으로 `category_diagnosis` 시드 lookup
    /// 결과 — 사람이 읽는 진단 메시지 + 추천 액션 (S12 / M004). 시드되지 않은
    /// 카테고리는 `null` (silent default 금지 — D016/D014). enum 자체 세분화는
    /// 별 슬라이스(S12.1 sketch).
    pub diagnosis: Option<Diagnosis>,
}

/// 실패 추이 시계열의 한 점 (failed_tx_timeseries).
#[derive(Debug, Clone, Serialize, sqlx::FromRow)]
pub struct FailedTxTrendPoint {
    /// 버킷 시작 시각 (`date_trunc` 결과)
    pub bucket: DateTime<Utc>,
    /// 에러 카테고리
    pub error_category: ErrorCategory,
    /// 해당 버킷·카테고리의 실패 건수
    pub failure_count: i64,
}

/// 실패 패턴 알림 구독 (alert_subscription).
///
/// `signing_secret`은 생성 응답([`AlertSubscriptionCreated`])에서 **1회만** 노출
/// 하고, 목록 직렬화에선 `#[serde(skip_serializing)]`로 제외한다(시크릿).
///
/// `sub_type='per_event'` (default, S08) — 매칭 실패 tx 1건당 1회 webhook.
/// `sub_type='rate_threshold'` (S14/M005) — 윈도우 내 매칭 카운트가 임계 이상이면
/// 1회 webhook, 발송 후 `debounce_secs` 동안 같은 sub 무시. CHECK 제약으로 rate
/// 모드면 3 컬럼 모두 NOT NULL.
#[derive(Debug, Clone, Serialize, sqlx::FromRow)]
pub struct AlertSubscription {
    /// 구독 ID (PK)
    pub subscription_id: i64,
    /// 매칭할 에러 카테고리 (None = 모든 카테고리)
    pub error_category: Option<ErrorCategory>,
    /// 매칭할 컨트랙트 주소(소문자) (None = 모든 주소)
    pub to_addr: Option<String>,
    /// 알림을 POST 할 webhook URL
    pub webhook_url: String,
    /// per-구독 HMAC-SHA256 키 — 로그/목록 응답에 노출 금지
    #[serde(skip_serializing)]
    pub signing_secret: String,
    /// 활성 여부
    pub active: bool,
    /// 생성 시각
    pub created_at: DateTime<Utc>,
    /// `per_event` | `rate_threshold` (S14/M005, default per_event)
    pub sub_type: String,
    /// rate_threshold: 임계 카운트 (per_event는 NULL)
    pub threshold_count: Option<i32>,
    /// rate_threshold: 카운트 윈도우 (초)
    pub threshold_window_secs: Option<i32>,
    /// rate_threshold: 발송 후 디바운스 시간 (초)
    pub debounce_secs: Option<i32>,
}

/// `AlertSubscription` → `AlertSubscriptionCreated` 변환 (S14 / M005).
///
/// 한 곳에서 매핑 — sub_type 등 rate 필드가 늘어나도 호출처(api 핸들러)는
/// `row.into()` 한 줄. 단방향(생성/회전 응답에서 `signing_secret`을 *그대로*
/// 노출), 일반 list 응답에는 사용 금지(시크릿 노출 위험).
impl From<AlertSubscription> for AlertSubscriptionCreated {
    fn from(s: AlertSubscription) -> Self {
        Self {
            subscription_id: s.subscription_id,
            error_category: s.error_category,
            to_addr: s.to_addr,
            webhook_url: s.webhook_url,
            signing_secret: s.signing_secret,
            active: s.active,
            created_at: s.created_at,
            sub_type: s.sub_type,
            threshold_count: s.threshold_count,
            threshold_window_secs: s.threshold_window_secs,
            debounce_secs: s.debounce_secs,
        }
    }
}

/// 구독 생성(POST) 응답 — `signing_secret`을 **이때 한 번만** 반환한다.
#[derive(Debug, Clone, Serialize)]
pub struct AlertSubscriptionCreated {
    /// 구독 ID (PK)
    pub subscription_id: i64,
    /// 매칭 에러 카테고리 (None = 모든 카테고리)
    pub error_category: Option<ErrorCategory>,
    /// 매칭 컨트랙트 주소(소문자) (None = 모든 주소)
    pub to_addr: Option<String>,
    /// 알림 webhook URL
    pub webhook_url: String,
    /// HMAC 서명 키 — 생성 직후 **1회만** 노출(이후 조회 불가)
    pub signing_secret: String,
    /// 활성 여부
    pub active: bool,
    /// 생성 시각
    pub created_at: DateTime<Utc>,
    /// `per_event` | `rate_threshold` (S14/M005)
    pub sub_type: String,
    /// rate_threshold: 임계 카운트 (per_event는 None)
    pub threshold_count: Option<i32>,
    /// rate_threshold: 카운트 윈도우 (초)
    pub threshold_window_secs: Option<i32>,
    /// rate_threshold: 디바운스 시간 (초)
    pub debounce_secs: Option<i32>,
}

/// 디스패처가 전송할 (구독 × 미전송 실패 tx) 매칭 1건. 내부용(직렬화 안 함).
#[derive(Debug, Clone, sqlx::FromRow)]
pub struct AlertMatch {
    /// 대상 구독 ID
    pub subscription_id: i64,
    /// 매칭된 실패 tx 해시
    pub tx_hash: String,
    /// 전송 대상 webhook URL
    pub webhook_url: String,
    /// 본문 서명용 HMAC 키
    pub signing_secret: String,
}

/// rate_threshold 디스패처가 발송할 (구독 × 윈도우 카운트) 매칭 1건 (S14/M005).
///
/// `match_count`는 윈도우 내 매칭된 실패 tx 수, `threshold_count`/
/// `threshold_window_secs`는 발송 payload에 포함하기 위한 sub 메타. 디바운스
/// 검증은 SQL 측에서 이미 수행됨 — 본 행은 *발송 자격 있음*을 의미.
#[derive(Debug, Clone, sqlx::FromRow)]
pub struct RateAlertMatch {
    pub subscription_id: i64,
    pub webhook_url: String,
    pub signing_secret: String,
    pub match_count: i64,
    pub threshold_count: i32,
    pub threshold_window_secs: i32,
}

/// Off-chain `address → human label` 매핑 (S09 / M003).
///
/// Dune이 구조적으로 못 하는 "비공개 라벨 × 온체인 데이터" 조인의 기반. 라벨은
/// 공개(`owner_id = NULL`)이거나 테넌트별(`owner_id`)이 될 수 있고, 분석 엔드포인트
/// 는 `transaction.to_addr` 를 키로 JOIN 한다.
#[derive(Debug, Clone, Serialize, sqlx::FromRow)]
pub struct ContractLabel {
    /// 소문자 0x + 40 hex (`transaction.to_addr`와 매칭)
    pub address: String,
    /// 사람이 읽는 라벨
    pub label: String,
    /// 테넌시 힌트 — `None` = 공개/전역 라벨
    pub owner_id: Option<String>,
    /// 등록 시각
    pub created_at: DateTime<Utc>,
}

/// 라벨된 컨트랙트 1개의 실패 분포(`failed_tx_by_label_aggregate`).
///
/// `by_category`는 `{ "SLIPPAGE_EXCEEDED": 3, "UNKNOWN": 1, ... }` 형태의 카테고리
/// 별 카운트. SQL 측에서 한 행씩 (label, address, category) 그루핑 결과를 받아
/// 호출자가 Rust에서 피벗한다 — `sqlx`의 `json` 피처 무도입(스코프 단속).
#[derive(Debug, Clone, Serialize)]
pub struct FailedTxByLabelPoint {
    pub label: String,
    pub address: String,
    pub total_failures: i64,
    pub by_category: std::collections::HashMap<String, i64>,
}

/// 4-byte function selector → 사람이 읽는 이름/시그니처 매핑 (S11 / M004).
///
/// 자기소유 ABI 시드(`migrations/20240106000001_add_function_signature.sql`)에서
/// 채워진다. `/v1/failed-tx/{tx_hash}` 응답의 `failing_function`(selector)을
/// 즉시 사람이 읽는 함수명/시그니처로 식별 가능하게 — args 디코딩은 별 슬라이스
/// (S11.1 sketch, D015 일관).
#[derive(Debug, Clone, Serialize, sqlx::FromRow)]
pub struct FunctionSignature {
    /// 4-byte selector — lowercased hex (`0x` + 8 hex chars).
    pub selector: String,
    /// 함수명 (예: `transfer`).
    pub name: String,
    /// ABI signature (예: `transfer(address,uint256)`).
    pub signature: String,
    /// 시드 출처 (`erc20` | `uniswap-v3-router` | ...).
    pub source: Option<String>,
    /// 시드/등록 시각.
    pub created_at: DateTime<Utc>,
}

/// `FailedTxDetail.failing_function_decoded`용 합성 — 응답 직렬화 전용 (S11 / M004).
///
/// `failing_function`(4-byte selector) lookup 성공 시 [`FunctionSignature`]에서
/// 발췌(생성 시각 등 내부 컬럼 제외). 매칭 없음은 `None` (silent default 금지;
/// S10/D014 일관).
///
/// S11.1: `args`는 input bytes를 signature와 함께 ABI 디코드한 결과 — *시도하지
/// 않음/실패* 모두 `None` (D027). 정직히: name/signature는 lookup만으로도 박히지만
/// args는 input이 있어야 하고 디코드도 성공해야 채워짐.
#[derive(Debug, Clone, Serialize)]
pub struct DecodedFunction {
    pub selector: String,
    pub name: String,
    pub signature: String,
    pub source: Option<String>,
    /// S11.1 — 타입된 인자값(`Option<Vec<DecodedArg>>`). `None`은 *디코드 시도하지
    /// 않음 또는 실패* (D027). 클라이언트는 `null`을 *조용히 fallback*이 아닌
    /// *명시 신호*로 해석해야 함.
    pub args: Option<Vec<decoder::abi::DecodedArg>>,
}

impl From<FunctionSignature> for DecodedFunction {
    fn from(fs: FunctionSignature) -> Self {
        Self {
            selector: fs.selector,
            name: fs.name,
            signature: fs.signature,
            source: fs.source,
            args: None,
        }
    }
}

/// 카테고리별 진단 메시지 + 추천 액션 (S12 / M004).
///
/// 자기소유 시드(`migrations/20240107000001_add_category_diagnosis.sql`)에서
/// 채워진다. `/v1/failed-tx/{tx_hash}` 응답에서 dApp 개발자가 "왜 실패했나 +
/// 어떻게 고치나"를 한 호출에 받게 한다. enum 자체 세분화는 별 슬라이스
/// (S12.1 sketch, D016 일관).
#[derive(Debug, Clone, Serialize, sqlx::FromRow)]
pub struct CategoryDiagnosis {
    /// ErrorCategory enum wire form (예: `SLIPPAGE_EXCEEDED`).
    pub error_category: String,
    /// 진단 메시지 (왜 실패했나).
    pub message: String,
    /// 추천 액션 (어떻게 고치나) — 선택.
    pub recommended_action: Option<String>,
    /// 시드 출처 (예: `builtin`) — 운영자 후속 시드 구분.
    pub source: Option<String>,
    /// 시드/등록 시각.
    pub created_at: DateTime<Utc>,
}

/// `FailedTxDetail.diagnosis`용 합성 — 응답 직렬화 전용 (S12 / M004).
///
/// `error_category`로 [`CategoryDiagnosis`] lookup 성공 시 발췌. 응답 컨텍스트가
/// 이미 카테고리를 보유하므로 `error_category`/`created_at`은 제외. 매칭 없음은
/// `None` (silent default 금지; S10/S11/D014 일관).
#[derive(Debug, Clone, Serialize)]
pub struct Diagnosis {
    pub message: String,
    pub recommended_action: Option<String>,
    pub source: Option<String>,
}

impl From<CategoryDiagnosis> for Diagnosis {
    fn from(cd: CategoryDiagnosis) -> Self {
        Self {
            message: cd.message,
            recommended_action: cd.recommended_action,
            source: cd.source,
        }
    }
}
