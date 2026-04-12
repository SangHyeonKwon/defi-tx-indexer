-- ============================================
-- 6. 풀별 최신 가격 스냅샷 (1m 기준)
-- Latest price snapshots per pool (1m interval)
-- ============================================

SELECT
    p.pair_name,
    ps.price,
    ps.tick,
    ps.liquidity,
    ps.snapshot_ts
FROM price_snapshot ps
JOIN pool p ON ps.pool_address = p.pool_address
WHERE ps.interval_type = '1m'
ORDER BY ps.snapshot_ts DESC;
