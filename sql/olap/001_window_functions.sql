-- ============================================
-- DeFi Analytics Database (Uniswap V3)
-- OLAP: 고급 분석 쿼리 8개
-- Window Functions, CTEs, Percentiles
-- ============================================

-- ────────────────────────────────────────────
-- 1. 7일 이동평균 스왑 볼륨 (ROWS BETWEEN)
-- ────────────────────────────────────────────
WITH daily_volumes AS (
    SELECT
        s.pool_address,
        p.pair_name,
        DATE(s.timestamp) AS swap_date,
        SUM(s.amount_in)  AS daily_volume,
        COUNT(*)          AS daily_swaps
    FROM swap_event s
    JOIN pool p ON s.pool_address = p.pool_address
    GROUP BY s.pool_address, p.pair_name, DATE(s.timestamp)
)
SELECT
    pool_address,
    pair_name,
    swap_date,
    daily_volume,
    daily_swaps,
    AVG(daily_volume) OVER (
        PARTITION BY pool_address
        ORDER BY swap_date
        ROWS BETWEEN 6 PRECEDING AND CURRENT ROW
    ) AS moving_avg_7d,
    SUM(daily_swaps) OVER (
        PARTITION BY pool_address
        ORDER BY swap_date
        ROWS BETWEEN 6 PRECEDING AND CURRENT ROW
    ) AS rolling_swaps_7d
FROM daily_volumes
ORDER BY pool_address, swap_date;

-- ────────────────────────────────────────────
-- 2. 풀별 누적 볼륨 (Running SUM)
-- ────────────────────────────────────────────
SELECT
    s.pool_address,
    p.pair_name,
    s.timestamp,
    s.amount_in,
    SUM(s.amount_in) OVER (
        PARTITION BY s.pool_address
        ORDER BY s.timestamp
    ) AS cumulative_volume,
    ROW_NUMBER() OVER (
        PARTITION BY s.pool_address
        ORDER BY s.timestamp
    ) AS trade_sequence
FROM swap_event s
JOIN pool p ON s.pool_address = p.pool_address
ORDER BY s.pool_address, s.timestamp;

-- ────────────────────────────────────────────
-- 3. 풀별 트레이더 랭킹 (DENSE_RANK)
-- ────────────────────────────────────────────
WITH trader_volumes AS (
    SELECT
        s.pool_address,
        p.pair_name,
        s.sender,
        COUNT(*)        AS trade_count,
        SUM(s.amount_in) AS total_volume
    FROM swap_event s
    JOIN pool p ON s.pool_address = p.pool_address
    GROUP BY s.pool_address, p.pair_name, s.sender
)
SELECT
    pool_address,
    pair_name,
    sender,
    trade_count,
    total_volume,
    DENSE_RANK() OVER (
        PARTITION BY pool_address
        ORDER BY total_volume DESC
    ) AS volume_rank,
    ROUND(
        100.0 * total_volume / SUM(total_volume) OVER (PARTITION BY pool_address),
        2
    ) AS volume_share_pct
FROM trader_volumes
ORDER BY pool_address, volume_rank;

-- ────────────────────────────────────────────
-- 4. 시간대별 트랜잭션 처리량 + 전시간 비교 (LAG/LEAD)
-- ────────────────────────────────────────────
WITH hourly_counts AS (
    SELECT
        date_trunc('hour', b.timestamp) AS hour_bucket,
        COUNT(*)                        AS tx_count,
        SUM(t.gas_used)                 AS total_gas
    FROM transaction t
    JOIN block b ON t.block_number = b.block_number
    GROUP BY date_trunc('hour', b.timestamp)
)
SELECT
    hour_bucket,
    tx_count,
    total_gas,
    LAG(tx_count) OVER (ORDER BY hour_bucket)   AS prev_hour_count,
    tx_count - LAG(tx_count) OVER (ORDER BY hour_bucket) AS count_delta,
    CASE
        WHEN LAG(tx_count) OVER (ORDER BY hour_bucket) > 0
        THEN ROUND(
            100.0 * (tx_count - LAG(tx_count) OVER (ORDER BY hour_bucket))
            / LAG(tx_count) OVER (ORDER BY hour_bucket),
            2
        )
        ELSE NULL
    END AS pct_change,
    LEAD(tx_count) OVER (ORDER BY hour_bucket)  AS next_hour_count
FROM hourly_counts
ORDER BY hour_bucket;

-- ────────────────────────────────────────────
-- 5. 블록별 가스 가격 백분위 (PERCENTILE_CONT)
-- ───────────────────────────��────────────────
SELECT
    t.block_number,
    COUNT(*)                                                    AS tx_count,
    MIN(t.gas_price)                                            AS min_gas_price,
    PERCENTILE_CONT(0.25) WITHIN GROUP (ORDER BY t.gas_price)  AS p25_gas_price,
    PERCENTILE_CONT(0.50) WITHIN GROUP (ORDER BY t.gas_price)  AS median_gas_price,
    PERCENTILE_CONT(0.75) WITHIN GROUP (ORDER BY t.gas_price)  AS p75_gas_price,
    PERCENTILE_CONT(0.95) WITHIN GROUP (ORDER BY t.gas_price)  AS p95_gas_price,
    MAX(t.gas_price)                                            AS max_gas_price,
    ROUND(AVG(t.gas_price))                                    AS avg_gas_price
FROM transaction t
GROUP BY t.block_number
ORDER BY t.block_number;

-- ────────────────────────────────────────────
-- 6. LP 리텐션 분석 (다중 CTE)
--    유동성 공급자가 MINT 후 얼마나 빨리 BURN하는지 분석
-- ────────────────────────────────────────────
WITH first_mint AS (
    SELECT
        provider,
        pool_address,
        MIN(timestamp) AS first_mint_ts
    FROM liquidity_event
    WHERE event_type = 'MINT'
    GROUP BY provider, pool_address
),
first_burn AS (
    SELECT
        le.provider,
        le.pool_address,
        MIN(le.timestamp) AS first_burn_ts
    FROM liquidity_event le
    WHERE le.event_type = 'BURN'
    GROUP BY le.provider, le.pool_address
),
retention AS (
    SELECT
        fm.provider,
        fm.pool_address,
        p.pair_name,
        fm.first_mint_ts,
        fb.first_burn_ts,
        fb.first_burn_ts - fm.first_mint_ts AS retention_duration,
        CASE
            WHEN fb.first_burn_ts IS NULL THEN 'STILL_ACTIVE'
            WHEN fb.first_burn_ts - fm.first_mint_ts < INTERVAL '1 hour' THEN 'SHORT_TERM'
            WHEN fb.first_burn_ts - fm.first_mint_ts < INTERVAL '1 day' THEN 'MEDIUM_TERM'
            ELSE 'LONG_TERM'
        END AS retention_category
    FROM first_mint fm
    LEFT JOIN first_burn fb
        ON fm.provider = fb.provider AND fm.pool_address = fb.pool_address
    JOIN pool p ON fm.pool_address = p.pool_address
)
SELECT
    retention_category,
    COUNT(*)                AS provider_count,
    AVG(EXTRACT(EPOCH FROM retention_duration) / 3600) AS avg_hours_retained
FROM retention
GROUP BY retention_category
ORDER BY provider_count DESC;

-- ────────────────────────────────────────────
-- 7. �� 시장점유율 추이 (일별 비율 계산)
-- ────────────────────────────────────────────
WITH daily_pool_volume AS (
    SELECT
        DATE(s.timestamp) AS swap_date,
        s.pool_address,
        p.pair_name,
        SUM(s.amount_in)  AS pool_volume
    FROM swap_event s
    JOIN pool p ON s.pool_address = p.pool_address
    GROUP BY DATE(s.timestamp), s.pool_address, p.pair_name
)
SELECT
    swap_date,
    pool_address,
    pair_name,
    pool_volume,
    SUM(pool_volume) OVER (PARTITION BY swap_date) AS total_daily_volume,
    ROUND(
        100.0 * pool_volume / NULLIF(SUM(pool_volume) OVER (PARTITION BY swap_date), 0),
        2
    ) AS market_share_pct,
    RANK() OVER (
        PARTITION BY swap_date
        ORDER BY pool_volume DESC
    ) AS daily_rank
FROM daily_pool_volume
ORDER BY swap_date, daily_rank;

-- ────────────────────────────────────────────
-- 8. 에러 카테고리별 트렌드 피벗 (FILTER 절)
-- ───────────────────────��────────────────────
SELECT
    DATE(f.timestamp)       AS failure_date,
    COUNT(*)                AS total_failures,
    COUNT(*) FILTER (WHERE f.error_category = 'INSUFFICIENT_BALANCE') AS insufficient_balance,
    COUNT(*) FILTER (WHERE f.error_category = 'SLIPPAGE_EXCEEDED')    AS slippage_exceeded,
    COUNT(*) FILTER (WHERE f.error_category = 'DEADLINE_EXPIRED')     AS deadline_expired,
    COUNT(*) FILTER (WHERE f.error_category = 'UNAUTHORIZED')         AS unauthorized,
    COUNT(*) FILTER (WHERE f.error_category = 'TRANSFER_FAILED')      AS transfer_failed,
    COUNT(*) FILTER (WHERE f.error_category = 'UNKNOWN')              AS unknown_error,
    ROUND(AVG(f.gas_used))  AS avg_gas_wasted
FROM failed_transaction f
GROUP BY DATE(f.timestamp)
ORDER BY failure_date;
