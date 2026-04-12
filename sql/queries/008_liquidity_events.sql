-- ============================================
-- 8. 유동성 공급/회수 내역
-- Liquidity provision and withdrawal history
-- ============================================

SELECT
    le.event_type,
    p.pair_name,
    le.provider,
    le.token0_amount,
    le.token1_amount,
    le.timestamp
FROM liquidity_event le
JOIN pool p ON le.pool_address = p.pool_address
ORDER BY le.timestamp;
