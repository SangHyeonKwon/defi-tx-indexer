use alloy::sol;
use bigdecimal::BigDecimal;
use chrono::{DateTime, Utc};

use crate::error::DecodeError;

// Uniswap V3 이벤트 ABI 정의
sol! {
    /// Uniswap V3 Pool Swap 이벤트
    #[derive(Debug)]
    event Swap(
        address indexed sender,
        address indexed recipient,
        int256 amount0,
        int256 amount1,
        uint160 sqrtPriceX96,
        uint128 liquidity,
        int24 tick
    );

    /// Uniswap V3 Pool Mint 이벤트 (유동성 추가)
    #[derive(Debug)]
    event Mint(
        address sender,
        address indexed owner,
        int24 indexed tickLower,
        int24 indexed tickUpper,
        uint128 amount,
        uint256 amount0,
        uint256 amount1
    );

    /// Uniswap V3 Pool Burn 이벤트 (유동성 제거)
    #[derive(Debug)]
    event Burn(
        address indexed owner,
        int24 indexed tickLower,
        int24 indexed tickUpper,
        uint128 amount,
        uint256 amount0,
        uint256 amount1
    );

    /// ERC-20 Transfer 이벤트
    #[derive(Debug)]
    event Transfer(
        address indexed from,
        address indexed to,
        uint256 value
    );
}

/// 디코딩된 스왑 이벤트 (DB INSERT용 중간 구조체).
#[derive(Debug, Clone)]
pub struct DecodedSwap {
    /// 풀 주소
    pub pool_address: String,
    /// 트랜잭션 해시
    pub tx_hash: String,
    /// 스왑 발신자
    pub sender: String,
    /// 스왑 수신자
    pub recipient: String,
    /// token0 변동량
    pub amount0: BigDecimal,
    /// token1 변동량
    pub amount1: BigDecimal,
    /// 유입량
    pub amount_in: BigDecimal,
    /// 유출량
    pub amount_out: BigDecimal,
    /// sqrtPriceX96
    pub sqrt_price_x96: BigDecimal,
    /// 유동성
    pub liquidity: BigDecimal,
    /// 틱
    pub tick: i32,
    /// 로그 인덱스
    pub log_index: i32,
    /// 타임스탬프
    pub timestamp: DateTime<Utc>,
}

/// 디코딩된 유동성 이벤트 (Mint/Burn).
#[derive(Debug, Clone)]
pub struct DecodedLiquidity {
    /// 이벤트 타입 ("MINT" 또는 "BURN")
    pub event_type: String,
    /// 풀 주소
    pub pool_address: String,
    /// 트랜잭션 해시
    pub tx_hash: String,
    /// 유동성 제공자
    pub provider: String,
    /// token0 수량
    pub token0_amount: BigDecimal,
    /// token1 수량
    pub token1_amount: BigDecimal,
    /// 하한 틱
    pub tick_lower: i32,
    /// 상한 틱
    pub tick_upper: i32,
    /// 유동성
    pub liquidity: BigDecimal,
    /// 로그 인덱스
    pub log_index: i32,
    /// 타임스탬프
    pub timestamp: DateTime<Utc>,
}

/// 디코딩된 ERC-20 전송.
#[derive(Debug, Clone)]
pub struct DecodedTransfer {
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
    /// 타임스탬프
    pub timestamp: DateTime<Utc>,
}

/// 디코딩 결과를 나타내는 통합 enum.
#[derive(Debug, Clone)]
pub enum DecodedEvent {
    /// 스왑 이벤트
    Swap(DecodedSwap),
    /// 유동성 이벤트
    Liquidity(DecodedLiquidity),
    /// 토큰 전송
    Transfer(DecodedTransfer),
}

/// raw 로그에서 이벤트를 디코딩한다.
///
/// 이벤트 토픽(topic0)을 기반으로 적절한 디코더를 선택한다.
/// 알 수 없는 토픽은 `DecodeError::UnknownTopic`을 반환한다.
pub fn decode_log(
    _log_topics: &[alloy::primitives::B256],
    _log_data: &[u8],
    _pool_address: &str,
    _tx_hash: &str,
    _log_index: i32,
    _timestamp: DateTime<Utc>,
) -> Result<DecodedEvent, DecodeError> {
    // Phase 3에서 구현
    // topic0으로 이벤트 시그니처 매칭 → 각 이벤트별 디코딩 로직
    Err(DecodeError::UnknownTopic("not yet implemented".to_string()))
}
