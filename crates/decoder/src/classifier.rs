//! 리버트 사유를 에러 카테고리로 분류한다.
//!
//! Uniswap V3 및 ERC-20에서 발생하는 일반적인 revert reason 패턴을
//! `ErrorCategory` 문자열로 매핑한다.

/// revert reason 문자열을 에러 카테고리로 분류한다.
///
/// 반환 값은 `db::models::ErrorCategory` enum과 매칭되는 문자열이다.
/// decoder 크레이트가 db에 의존하지 않으므로 문자열로 반환한다.
pub fn classify_error(revert_reason: &str) -> &'static str {
    let lower = revert_reason.to_lowercase();

    // 잔액 부족
    if lower.contains("stf")
        || lower.contains("insufficient")
        || lower.contains("balance")
        || lower.contains("exceeds balance")
        || lower.contains("not enough")
    {
        return "INSUFFICIENT_BALANCE";
    }

    // 슬리피지 초과
    if lower.contains("too little received")
        || lower.contains("too much requested")
        || lower.contains("slippage")
        || lower.contains("price slipped")
        || lower.contains("amount out")
    {
        return "SLIPPAGE_EXCEEDED";
    }

    // 기한 만료
    if lower.contains("deadline")
        || lower.contains("too old")
        || lower.contains("expired")
        || lower.contains("transaction too old")
    {
        return "DEADLINE_EXPIRED";
    }

    // 권한 없음
    if lower.contains("unauthorized")
        || lower.contains("ownable")
        || lower.contains("not owner")
        || lower.contains("forbidden")
        || lower.contains("access denied")
    {
        return "UNAUTHORIZED";
    }

    // 전송 실패
    if lower.contains("transfer failed")
        || lower.contains("transfer_failed")
        || lower.contains("safe transfer")
        || lower.contains("safetransferfrom")
    {
        return "TRANSFER_FAILED";
    }

    "UNKNOWN"
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_classify_insufficient_balance() {
        assert_eq!(classify_error("STF"), "INSUFFICIENT_BALANCE");
        assert_eq!(
            classify_error("Insufficient balance for transfer"),
            "INSUFFICIENT_BALANCE"
        );
        assert_eq!(
            classify_error("ERC20: transfer amount exceeds balance"),
            "INSUFFICIENT_BALANCE"
        );
    }

    #[test]
    fn test_classify_slippage() {
        assert_eq!(classify_error("Too little received"), "SLIPPAGE_EXCEEDED");
        assert_eq!(classify_error("Too much requested"), "SLIPPAGE_EXCEEDED");
    }

    #[test]
    fn test_classify_deadline() {
        assert_eq!(classify_error("Transaction too old"), "DEADLINE_EXPIRED");
        assert_eq!(classify_error("Deadline expired"), "DEADLINE_EXPIRED");
    }

    #[test]
    fn test_classify_unauthorized() {
        assert_eq!(
            classify_error("Ownable: caller is not the owner"),
            "UNAUTHORIZED"
        );
    }

    #[test]
    fn test_classify_transfer_failed() {
        assert_eq!(
            classify_error("TransferHelper: TRANSFER_FAILED"),
            "TRANSFER_FAILED"
        );
    }

    #[test]
    fn test_classify_unknown() {
        assert_eq!(classify_error("0xdeadbeef"), "UNKNOWN");
        assert_eq!(classify_error("Panic(0x11)"), "UNKNOWN");
    }
}
