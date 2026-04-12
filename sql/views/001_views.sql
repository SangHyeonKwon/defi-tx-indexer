-- ============================================
-- DeFi Analytics Database (Uniswap V3)
-- Views: 분석용 뷰 7개
-- ============================================

-- ────────────────────────────────────────────
-- 1. vw_daily_swap_volume — 일별 풀별 스왑 볼륨
-- ────────────────────────────────────────────
CREATE OR REPLACE VIEW vw_daily_swap_volume AS
SELECT
    s.pool_address,
    p.pair_name,
    DATE(s.timestamp) AS swap_date,
    COUNT(*)          AS swap_count,
    SUM(s.amount_in)  AS total_amount_in,
    SUM(s.amount_out) AS total_amount_out
FROM swap_event s
JOIN pool p ON s.pool_address = p.pool_address
GROUP BY s.pool_address, p.pair_name, DATE(s.timestamp)
ORDER BY swap_date DESC, swap_count DESC;

-- ────────────────────────────────────────────
-- 2. vw_top_traders — 트레이더 랭킹 (거래량 기준)
-- ────────────────────────────────────────────
CREATE OR REPLACE VIEW vw_top_traders AS
SELECT
    user_address,
    label,
    total_swaps,
    total_volume_usd,
    DENSE_RANK() OVER (ORDER BY total_volume_usd DESC) AS volume_rank
FROM user_profile
WHERE total_swaps > 0
ORDER BY volume_rank;

-- ────────────────────────────────────────────
-- 3. vw_pool_liquidity_summary — 풀별 유동성 현황
-- ────────────────────────────────────────────
CREATE OR REPLACE VIEW vw_pool_liquidity_summary AS
WITH latest_price AS (
    SELECT DISTINCT ON (pool_address)
        pool_address,
        price       AS latest_price,
        tick        AS latest_tick,
        liquidity   AS current_liquidity,
        snapshot_ts AS last_snapshot
    FROM price_snapshot
    ORDER BY pool_address, snapshot_ts DESC
),
liquidity_stats AS (
    SELECT
        pool_address,
        COUNT(*) FILTER (WHERE event_type = 'MINT') AS total_mints,
        COUNT(*) FILTER (WHERE event_type = 'BURN') AS total_burns,
        SUM(CASE WHEN event_type = 'MINT' THEN liquidity ELSE -liquidity END) AS net_liquidity
    FROM liquidity_event
    GROUP BY pool_address
)
SELECT
    p.pool_address,
    p.pair_name,
    p.fee_tier,
    lp.latest_price,
    lp.latest_tick,
    lp.current_liquidity,
    lp.last_snapshot,
    COALESCE(ls.total_mints, 0) AS total_mints,
    COALESCE(ls.total_burns, 0) AS total_burns,
    COALESCE(ls.net_liquidity, 0) AS net_liquidity
FROM pool p
LEFT JOIN latest_price lp ON p.pool_address = lp.pool_address
LEFT JOIN liquidity_stats ls ON p.pool_address = ls.pool_address;

-- ────────────────────────────────────────────
-- 4. vw_failed_tx_analysis — 실패 TX 카테고리별 분석
-- ────────────────────────────────────────────
CREATE OR REPLACE VIEW vw_failed_tx_analysis AS
WITH total AS (
    SELECT COUNT(*) AS total_failures FROM failed_transaction
)
SELECT
    f.error_category,
    COUNT(*)                          AS failure_count,
    ROUND(AVG(f.gas_used))           AS avg_gas_wasted,
    ROUND(
        100.0 * COUNT(*) / GREATEST(t.total_failures, 1), 2
    )                                 AS pct_of_total,
    MAX(f.timestamp)                  AS most_recent_failure
FROM failed_transaction f
CROSS JOIN total t
GROUP BY f.error_category, t.total_failures
ORDER BY failure_count DESC;

-- ────────────────────────────────────────────
-- 5. vw_hourly_gas_stats — 시간대별 가스 통계
-- ────────────────────────────────────────────
CREATE OR REPLACE VIEW vw_hourly_gas_stats AS
SELECT
    date_trunc('hour', b.timestamp) AS hour_bucket,
    COUNT(t.tx_hash)                AS tx_count,
    ROUND(AVG(t.gas_used))         AS avg_gas_used,
    MAX(t.gas_used)                 AS max_gas_used,
    MIN(t.gas_used)                 AS min_gas_used,
    SUM(t.gas_used)                 AS total_gas_used
FROM transaction t
JOIN block b ON t.block_number = b.block_number
GROUP BY date_trunc('hour', b.timestamp)
ORDER BY hour_bucket DESC;

-- ────────────────────────────────────────────
-- 6. vw_token_activity — 토큰별 활동 요약
-- ────────────────────────────────────────────
CREATE OR REPLACE VIEW vw_token_activity AS
WITH transfer_stats AS (
    SELECT
        token_address,
        COUNT(*)     AS transfer_count,
        SUM(amount)  AS total_transferred
    FROM token_transfer
    GROUP BY token_address
),
pool_stats AS (
    SELECT token_address, COUNT(*) AS pool_count
    FROM (
        SELECT token0_address AS token_address FROM pool
        UNION ALL
        SELECT token1_address FROM pool
    ) sub
    GROUP BY token_address
)
SELECT
    tk.token_address,
    tk.symbol,
    tk.name,
    tk.decimals,
    COALESCE(ts.transfer_count, 0)  AS transfer_count,
    COALESCE(ts.total_transferred, 0) AS total_transferred,
    COALESCE(ps.pool_count, 0)      AS pool_count
FROM token tk
LEFT JOIN transfer_stats ts ON tk.token_address = ts.token_address
LEFT JOIN pool_stats ps ON tk.token_address = ps.token_address
ORDER BY transfer_count DESC;

-- ────────────────────────────────────────────
-- 7. vw_pool_fee_revenue — 풀별 추정 수수료 수익
-- ────────────────────────────────────────────
CREATE OR REPLACE VIEW vw_pool_fee_revenue AS
SELECT
    p.pool_address,
    p.pair_name,
    p.fee_tier,
    COUNT(s.event_id)           AS total_swaps,
    SUM(s.amount_in)            AS total_volume,
    SUM(s.amount_in) * p.fee_tier / 1000000 AS estimated_fee_revenue
FROM pool p
LEFT JOIN swap_event s ON p.pool_address = s.pool_address
GROUP BY p.pool_address, p.pair_name, p.fee_tier
ORDER BY estimated_fee_revenue DESC;
