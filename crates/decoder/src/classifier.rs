//! 리버트 사유를 에러 카테고리로 분류한다.
//!
//! Uniswap V3 및 ERC-20에서 발생하는 일반적인 revert reason 패턴을
//! `ErrorCategory` 문자열로 매핑한다.

/// revert reason 문자열을 에러 카테고리로 분류한다.
///
/// 반환 값은 `db::models::ErrorCategory` enum과 매칭되는 문자열이다.
/// decoder 크레이트가 db에 의존하지 않으므로 문자열로 반환한다.
///
/// S12.1 룰 우선순위 (D030): *더 구체적인 세부 카테고리를 먼저 매칭*하고,
/// 미매칭 시 *기존 generic 카테고리로 fallback*. 세부 매칭이 *없을 때만*
/// fallback이 실행되도록 순서를 유지해야 한다. 신규 트랜잭션은 우선
/// 세부 카테고리로 분류된다.
pub fn classify_error(revert_reason: &str) -> &'static str {
    let lower = revert_reason.to_lowercase();

    // 잔액 부족 계열 — S12.1 세부 우선
    //   "allowance" → ERC-20 approve 부족 (진단 메시지 완전 다름: approve를 호출)
    if lower.contains("allowance") {
        return "INSUFFICIENT_ALLOWANCE";
    }

    // 애그리게이터 슬리피지 계열 — S12.1 세부 우선
    //   "return amount is not enough" / "min return" → 1inch 계열 애그리게이터의
    //   출력 최소치 미달 (의미상 "Too little received"와 동일한 amountOut 부족).
    //   generic "not enough" 키워드가 INSUFFICIENT_BALANCE로 삼키기 전에
    //   먼저 매칭해야 하므로 반드시 잔액 부족 블록 *앞*에 위치.
    if lower.contains("return amount is not enough") || lower.contains("min return") {
        return "SLIPPAGE_AMOUNT_OUT";
    }

    if lower.contains("stf")
        || lower.contains("insufficient")
        || lower.contains("balance")
        || lower.contains("exceeds balance")
        || lower.contains("not enough")
    {
        return "INSUFFICIENT_BALANCE";
    }

    // 슬리피지 계열 — S12.1 세부 우선
    //   "too little received" → 매수 슬리피지 (amountOut 부족)
    //   "too much requested"  → 매도 슬리피지 (amountIn 한도 초과)
    //   "price slipped" / "amount out" → 풀 가격 영향 한도 초과
    //   그 외 일반 "slippage" 키워드만 → generic SLIPPAGE_EXCEEDED fallback
    if lower.contains("too little received") {
        return "SLIPPAGE_AMOUNT_OUT";
    }
    if lower.contains("too much requested") {
        return "SLIPPAGE_AMOUNT_IN";
    }
    if lower.contains("price slipped") || lower.contains("amount out") {
        return "SLIPPAGE_PRICE_IMPACT";
    }
    if lower.contains("slippage") {
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
    //   "transfer_from_failed" — Uniswap V2 계열 TransferHelper의
    //   "TRANSFER_FROM_FAILED"는 "transfer_failed"의 부분 문자열 매칭에 걸리지
    //   않으므로 별도 키워드 필요. 공백 변형도 기존 spaced/underscored 쌍과
    //   대칭으로 함께 매칭.
    if lower.contains("transfer failed")
        || lower.contains("transfer_failed")
        || lower.contains("transfer_from_failed")
        || lower.contains("transfer from failed")
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

    // S12.1 — INSUFFICIENT_ALLOWANCE는 INSUFFICIENT_BALANCE의 *별개 카테고리*. approve를
    // 호출해야 한다는 진단 메시지가 완전 다름.
    #[test]
    fn test_classify_insufficient_allowance() {
        assert_eq!(
            classify_error("ERC20: insufficient allowance"),
            "INSUFFICIENT_ALLOWANCE"
        );
        assert_eq!(
            classify_error("transfer amount exceeds allowance"),
            "INSUFFICIENT_ALLOWANCE"
        );
        // "allowance" 키워드는 "balance"보다 더 *구체적*이라 우선 매칭 — 룰 순서가
        // 깨지면 INSUFFICIENT_BALANCE로 떨어질 위험. 회귀 가드.
        assert_eq!(
            classify_error("Insufficient allowance to transfer this amount"),
            "INSUFFICIENT_ALLOWANCE"
        );
    }

    // S12.1 — SLIPPAGE는 4개 세부 카테고리 + generic fallback. 컨트랙트가 공백을
    // 쓰는 형태("Too little received") 기준으로 매칭 — D015 자기시드 정신 일관이라
    // 운영자가 UPPER_SNAKE 변형(`TOO_LITTLE_RECEIVED`)을 만나면 룰을 추가하는 흐름.
    #[test]
    fn test_classify_slippage_amount_out() {
        assert_eq!(classify_error("Too little received"), "SLIPPAGE_AMOUNT_OUT");
        assert_eq!(
            classify_error("UniswapV2Router: too little received"),
            "SLIPPAGE_AMOUNT_OUT"
        );
    }

    #[test]
    fn test_classify_slippage_amount_in() {
        assert_eq!(classify_error("Too much requested"), "SLIPPAGE_AMOUNT_IN");
        assert_eq!(
            classify_error("router: too much requested"),
            "SLIPPAGE_AMOUNT_IN"
        );
    }

    #[test]
    fn test_classify_slippage_price_impact() {
        assert_eq!(
            classify_error("UniswapV3Pool: price slipped past the limit"),
            "SLIPPAGE_PRICE_IMPACT"
        );
        assert_eq!(
            classify_error("amount out is too low"),
            "SLIPPAGE_PRICE_IMPACT"
        );
    }

    // generic slippage fallback — 세부 키워드 없이 단순 "slippage"만 등장.
    #[test]
    fn test_classify_slippage_generic_fallback() {
        assert_eq!(
            classify_error("Generic slippage occurred"),
            "SLIPPAGE_EXCEEDED"
        );
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

    // S12.1 — 1inch 계열 애그리게이터 슬리피지 문자열은 "not enough"를 포함하지만
    // 의미상 amountOut 부족 → SLIPPAGE_AMOUNT_OUT. INSUFFICIENT_BALANCE 블록보다
    // 먼저 매칭되어야 하는 룰 순서 회귀 가드.
    #[test]
    fn test_classify_aggregator_slippage_amount_out() {
        assert_eq!(
            classify_error("Return amount is not enough"),
            "SLIPPAGE_AMOUNT_OUT"
        );
        assert_eq!(
            classify_error("Min return not reached"),
            "SLIPPAGE_AMOUNT_OUT"
        );
    }

    // 룰 순서 가드 — 애그리게이터 세부 룰 추가 후에도 generic "not enough" /
    // "insufficient"는 여전히 INSUFFICIENT_BALANCE로 fallback해야 한다.
    #[test]
    fn test_classify_generic_not_enough_still_balance() {
        assert_eq!(classify_error("not enough tokens"), "INSUFFICIENT_BALANCE");
        assert_eq!(classify_error("insufficient funds"), "INSUFFICIENT_BALANCE");
        // "allowance"는 여전히 balance보다 우선.
        assert_eq!(
            classify_error("insufficient allowance"),
            "INSUFFICIENT_ALLOWANCE"
        );
    }

    #[test]
    fn test_classify_transfer_failed() {
        assert_eq!(
            classify_error("TransferHelper: TRANSFER_FAILED"),
            "TRANSFER_FAILED"
        );
    }

    // Uniswap V2 계열 TransferHelper — "TRANSFER_FROM_FAILED"는 "transfer_failed"의
    // 부분 문자열이 아니라 UNKNOWN으로 떨어지던 룰 공백 회귀 가드.
    #[test]
    fn test_classify_transfer_from_failed() {
        assert_eq!(
            classify_error("TransferHelper: TRANSFER_FROM_FAILED"),
            "TRANSFER_FAILED"
        );
        assert_eq!(classify_error("transfer from failed"), "TRANSFER_FAILED");
    }

    #[test]
    fn test_classify_unknown() {
        assert_eq!(classify_error("0xdeadbeef"), "UNKNOWN");
        assert_eq!(classify_error("Panic(0x11)"), "UNKNOWN");
    }
}
