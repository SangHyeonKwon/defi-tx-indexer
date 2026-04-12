-- ============================================
-- 1. 풀별 스왑 횟수 및 총 거래량
-- Pool swap counts and total volumes
-- ============================================

SELECT
    p.pair_name,
    COUNT(s.event_id) AS swap_count,
    SUM(s.amount_in)  AS total_amount_in
FROM swap_event s
JOIN pool p ON s.pool_address = p.pool_address
GROUP BY p.pair_name
ORDER BY swap_count DESC;
