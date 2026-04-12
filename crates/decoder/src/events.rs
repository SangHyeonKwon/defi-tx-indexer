use alloy::primitives::B256;
use alloy::sol;
use alloy::sol_types::SolEvent;
use bigdecimal::BigDecimal;
use chrono::{DateTime, Utc};
use std::str::FromStr;

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
    log_topics: &[B256],
    log_data: &[u8],
    pool_address: &str,
    tx_hash: &str,
    log_index: i32,
    timestamp: DateTime<Utc>,
) -> Result<DecodedEvent, DecodeError> {
    let topic0 = log_topics
        .first()
        .ok_or_else(|| DecodeError::MissingField("topic0".to_string()))?;

    if *topic0 == Swap::SIGNATURE_HASH {
        decode_swap(
            log_topics,
            log_data,
            pool_address,
            tx_hash,
            log_index,
            timestamp,
        )
    } else if *topic0 == Mint::SIGNATURE_HASH {
        decode_mint(
            log_topics,
            log_data,
            pool_address,
            tx_hash,
            log_index,
            timestamp,
        )
    } else if *topic0 == Burn::SIGNATURE_HASH {
        decode_burn(
            log_topics,
            log_data,
            pool_address,
            tx_hash,
            log_index,
            timestamp,
        )
    } else if *topic0 == Transfer::SIGNATURE_HASH {
        decode_transfer(
            log_topics,
            log_data,
            pool_address,
            tx_hash,
            log_index,
            timestamp,
        )
    } else {
        Err(DecodeError::UnknownTopic(format!("{topic0}")))
    }
}

/// Swap 이벤트를 디코딩한다.
fn decode_swap(
    topics: &[B256],
    data: &[u8],
    pool_address: &str,
    tx_hash: &str,
    log_index: i32,
    timestamp: DateTime<Utc>,
) -> Result<DecodedEvent, DecodeError> {
    let decoded = Swap::decode_raw_log(topics.iter().copied(), data)
        .map_err(|e| DecodeError::AbiDecode(e.to_string()))?;

    let amount0 = bd_from_str(&decoded.amount0.to_string())?;
    let amount1 = bd_from_str(&decoded.amount1.to_string())?;

    let zero = BigDecimal::from(0);
    let (amount_in, amount_out) = if amount0 >= zero {
        (amount0.clone(), amount1.clone().abs())
    } else {
        (amount1.clone(), amount0.clone().abs())
    };

    Ok(DecodedEvent::Swap(DecodedSwap {
        pool_address: pool_address.to_string(),
        tx_hash: tx_hash.to_string(),
        sender: format!("{}", decoded.sender).to_lowercase(),
        recipient: format!("{}", decoded.recipient).to_lowercase(),
        amount0,
        amount1,
        amount_in,
        amount_out,
        sqrt_price_x96: bd_from_str(&decoded.sqrtPriceX96.to_string())?,
        liquidity: bd_from_str(&decoded.liquidity.to_string())?,
        tick: i32_from_str(&decoded.tick.to_string())?,
        log_index,
        timestamp,
    }))
}

/// Mint 이벤트를 디코딩한다.
fn decode_mint(
    topics: &[B256],
    data: &[u8],
    pool_address: &str,
    tx_hash: &str,
    log_index: i32,
    timestamp: DateTime<Utc>,
) -> Result<DecodedEvent, DecodeError> {
    let decoded = Mint::decode_raw_log(topics.iter().copied(), data)
        .map_err(|e| DecodeError::AbiDecode(e.to_string()))?;

    Ok(DecodedEvent::Liquidity(DecodedLiquidity {
        event_type: "MINT".to_string(),
        pool_address: pool_address.to_string(),
        tx_hash: tx_hash.to_string(),
        provider: format!("{}", decoded.owner).to_lowercase(),
        token0_amount: bd_from_str(&decoded.amount0.to_string())?,
        token1_amount: bd_from_str(&decoded.amount1.to_string())?,
        tick_lower: i32_from_str(&decoded.tickLower.to_string())?,
        tick_upper: i32_from_str(&decoded.tickUpper.to_string())?,
        liquidity: bd_from_str(&decoded.amount.to_string())?,
        log_index,
        timestamp,
    }))
}

/// Burn 이벤트를 디코딩한다.
fn decode_burn(
    topics: &[B256],
    data: &[u8],
    pool_address: &str,
    tx_hash: &str,
    log_index: i32,
    timestamp: DateTime<Utc>,
) -> Result<DecodedEvent, DecodeError> {
    let decoded = Burn::decode_raw_log(topics.iter().copied(), data)
        .map_err(|e| DecodeError::AbiDecode(e.to_string()))?;

    Ok(DecodedEvent::Liquidity(DecodedLiquidity {
        event_type: "BURN".to_string(),
        pool_address: pool_address.to_string(),
        tx_hash: tx_hash.to_string(),
        provider: format!("{}", decoded.owner).to_lowercase(),
        token0_amount: bd_from_str(&decoded.amount0.to_string())?,
        token1_amount: bd_from_str(&decoded.amount1.to_string())?,
        tick_lower: i32_from_str(&decoded.tickLower.to_string())?,
        tick_upper: i32_from_str(&decoded.tickUpper.to_string())?,
        liquidity: bd_from_str(&decoded.amount.to_string())?,
        log_index,
        timestamp,
    }))
}

/// ERC-20 Transfer 이벤트를 디코딩한다.
fn decode_transfer(
    topics: &[B256],
    data: &[u8],
    token_address: &str,
    tx_hash: &str,
    log_index: i32,
    timestamp: DateTime<Utc>,
) -> Result<DecodedEvent, DecodeError> {
    let decoded = Transfer::decode_raw_log(topics.iter().copied(), data)
        .map_err(|e| DecodeError::AbiDecode(e.to_string()))?;

    Ok(DecodedEvent::Transfer(DecodedTransfer {
        tx_hash: tx_hash.to_string(),
        token_address: token_address.to_string(),
        from_addr: format!("{}", decoded.from).to_lowercase(),
        to_addr: format!("{}", decoded.to).to_lowercase(),
        amount: bd_from_str(&decoded.value.to_string())?,
        log_index,
        timestamp,
    }))
}

/// 문자열을 BigDecimal로 변환하는 헬퍼.
fn bd_from_str(s: &str) -> Result<BigDecimal, DecodeError> {
    BigDecimal::from_str(s).map_err(|e| DecodeError::AbiDecode(e.to_string()))
}

/// 문자열을 i32로 변환하는 헬퍼.
fn i32_from_str(s: &str) -> Result<i32, DecodeError> {
    s.parse::<i32>()
        .map_err(|e| DecodeError::AbiDecode(e.to_string()))
}

#[cfg(test)]
mod tests {
    use super::*;
    use alloy::primitives::{Address, Signed, Uint, I256};

    fn make_swap_log() -> (Vec<B256>, Vec<u8>) {
        let swap = Swap {
            sender: Address::repeat_byte(0x01),
            recipient: Address::repeat_byte(0x02),
            amount0: I256::try_from(-1_000_000i64).expect("valid i256"),
            amount1: I256::try_from(500_000i64).expect("valid i256"),
            sqrtPriceX96: Uint::from(79228162514264337593543950336u128),
            liquidity: 1_000_000u128,
            tick: Signed::<24, 1>::try_from(100i32).expect("valid i24"),
        };
        let encoded = swap.encode_log_data();
        let topics = encoded.topics().to_vec();
        let data = encoded.data.to_vec();
        (topics, data)
    }

    #[test]
    fn test_decode_swap_event() {
        let (topics, data) = make_swap_log();
        let result = decode_log(&topics, &data, "0xpool", "0xtx", 0, Utc::now());

        let event = result.expect("should decode swap");
        match event {
            DecodedEvent::Swap(s) => {
                assert_eq!(s.pool_address, "0xpool");
                assert_eq!(s.tx_hash, "0xtx");
                assert_eq!(s.tick, 100);
                assert!(s.amount_in > BigDecimal::from(0));
                assert!(s.amount_out > BigDecimal::from(0));
            }
            _ => panic!("expected Swap variant"),
        }
    }

    #[test]
    fn test_decode_transfer_event() {
        let transfer = Transfer {
            from: Address::repeat_byte(0x0a),
            to: Address::repeat_byte(0x0b),
            value: Uint::from(999_999u64),
        };
        let encoded = transfer.encode_log_data();
        let topics = encoded.topics().to_vec();
        let data = encoded.data.to_vec();

        let result = decode_log(&topics, &data, "0xtoken", "0xtx", 5, Utc::now());
        let event = result.expect("should decode transfer");
        match event {
            DecodedEvent::Transfer(t) => {
                assert_eq!(t.token_address, "0xtoken");
                assert_eq!(t.amount, BigDecimal::from(999_999));
                assert_eq!(t.log_index, 5);
            }
            _ => panic!("expected Transfer variant"),
        }
    }

    #[test]
    fn test_decode_mint_event() {
        let mint = Mint {
            sender: Address::repeat_byte(0x01),
            owner: Address::repeat_byte(0x02),
            tickLower: Signed::<24, 1>::try_from(-887220i32).expect("valid i24"),
            tickUpper: Signed::<24, 1>::try_from(887220i32).expect("valid i24"),
            amount: 5_000_000u128,
            amount0: Uint::from(1_000_000u64),
            amount1: Uint::from(2_000_000u64),
        };
        let encoded = mint.encode_log_data();
        let topics = encoded.topics().to_vec();
        let data = encoded.data.to_vec();

        let result = decode_log(&topics, &data, "0xpool", "0xtx", 1, Utc::now());
        let event = result.expect("should decode mint");
        match event {
            DecodedEvent::Liquidity(l) => {
                assert_eq!(l.event_type, "MINT");
                assert_eq!(l.tick_lower, -887220);
                assert_eq!(l.tick_upper, 887220);
            }
            _ => panic!("expected Liquidity variant"),
        }
    }

    #[test]
    fn test_decode_unknown_topic() {
        let result = decode_log(
            &[B256::repeat_byte(0xff)],
            &[],
            "0xaddr",
            "0xtx",
            0,
            Utc::now(),
        );
        assert!(matches!(result, Err(DecodeError::UnknownTopic(_))));
    }

    #[test]
    fn test_decode_empty_topics() {
        let result = decode_log(&[], &[], "0xaddr", "0xtx", 0, Utc::now());
        assert!(matches!(result, Err(DecodeError::MissingField(_))));
    }
}
