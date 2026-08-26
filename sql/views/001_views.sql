-- ============================================
-- DeFi Analytics Database (Uniswap V3)
-- Views: 분석용 뷰 8개 + 정규화 함수 1개
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

-- ────────────────────────────────────────────
-- fn_revert_template — revert reason 정규화 함수
--
-- decoder::classifier가 UNKNOWN으로 남긴 revert 사유를 클러스터링하기
-- 위한 템플릿 키를 만든다. revert 문자열은 기계 생성이라 가변부(주소,
-- 큰 숫자, hex 인자)만 치환하면 동일 원인끼리 정확히 묶인다.
--
-- 정규화 규칙 (순서 중요):
--   1) NULL / '' / '0x'            → '(no revert data)'
--        out-of-gas, bare revert() 등 출력 자체가 없는 실패
--   2) 'Panic(0xNN)'               → 원문 유지 (decoder::trace가 만든
--        정규형 — 코드별로 의미가 다르므로 코드를 보존: 0x11 산술
--        오버플로우, 0x12 0나눗셈, 0x32 배열 범위 초과 등)
--   3) '0x' + hex 8자 이상          → 'custom_error:0x셀렉터(8 hex)'
--        디코딩 안 된 커스텀 에러 — 4바이트 셀렉터로 클러스터링
--        (동일 에러의 인자만 다른 경우가 한 클러스터로 묶인다)
--   4) '0x' + hex 8자 미만          → '(undecodable output)'
--   5) 일반 텍스트: hex 41자 이상 → {hex}, 정확히 40자 → {addr},
--        나머지 hex → {hex}, 숫자 4자리 이상 → {n}, 공백 정리.
--        숫자는 4자리 이상만 치환 — 'M0', '51'(Aave), 'GS013' 같은
--        짧은 에러 코드는 그 자체가 식별자라 보존해야 한다.
--
-- 뷰보다 먼저 정의되어야 하므로 이 파일에 둔다 (seed.sh 적용 순서가
-- views → procedures라 procedures에 두면 뷰 생성 시점에 함수가 없다).
-- ────────────────────────────────────────────
CREATE OR REPLACE FUNCTION fn_revert_template(p_reason TEXT)
RETURNS TEXT
LANGUAGE sql
IMMUTABLE
AS $$
SELECT CASE
    WHEN p_reason IS NULL OR btrim(p_reason) = '' OR btrim(p_reason) = '0x'
        THEN '(no revert data)'
    WHEN p_reason ~ '^Panic\(0x[0-9a-fA-F]+\)$'
        THEN p_reason
    WHEN p_reason ~ '^0x[0-9a-fA-F]{8,}$'
        THEN 'custom_error:' || lower(left(p_reason, 10))
    WHEN p_reason ~ '^0x[0-9a-fA-F]+$'
        THEN '(undecodable output)'
    ELSE
        btrim(regexp_replace(regexp_replace(regexp_replace(regexp_replace(regexp_replace(
            p_reason,
            '0x[0-9a-fA-F]{41,}', '{hex}', 'g'),
            '0x[0-9a-fA-F]{40}',  '{addr}', 'g'),
            '0x[0-9a-fA-F]{1,39}', '{hex}', 'g'),
            '[0-9]{4,}',          '{n}',   'g'),
            '\s+',                ' ',     'g'))
END;
$$;

COMMENT ON FUNCTION fn_revert_template(TEXT) IS
    'revert reason 원문을 클러스터 템플릿으로 정규화 — 주소/긴 숫자/hex 인자를 플레이스홀더로 치환, 커스텀 에러는 4바이트 셀렉터로 축약';

-- ────────────────────────────────────────────
-- 8. vw_unknown_revert_clusters — UNKNOWN revert 사유 클러스터링
--
-- classifier가 분류하지 못한(UNKNOWN) 실패를 fn_revert_template 기준으로
-- 묶어 발생 빈도·낭비 가스·영향 범위 순으로 랭킹한다. 상위 클러스터가
-- 곧 classifier 신규 룰/카테고리 후보다.
--
-- cluster_kind:
--   NO_DATA      — 쓸 수 있는 revert 출력 없음 (out-of-gas, bare revert,
--                  또는 셀렉터조차 안 되는 4바이트 미만 출력)
--   CUSTOM_ERROR — 미디코딩 커스텀 에러 (셀렉터 단위)
--   PANIC        — Solidity Panic(uint256)
--   TEXT         — Error(string) 디코딩됐지만 룰 미매칭 (즉시 룰 추가 가능)
--
-- sample_revert_reason / sample_tx_hash는 각각 독립적인 MIN이라 서로
-- 같은 행에서 나온 값이 아닐 수 있다 — 대표 예시 용도로만 사용.
-- ────────────────────────────────────────────
CREATE OR REPLACE VIEW vw_unknown_revert_clusters AS
WITH unknown_tx AS (
    SELECT
        f.tx_hash,
        f.revert_reason,
        f.failing_function,
        f.gas_used,
        f.timestamp,
        fn_revert_template(f.revert_reason) AS template
    FROM failed_transaction f
    WHERE f.error_category = 'UNKNOWN'
),
total AS (
    SELECT COUNT(*) AS total_unknown FROM unknown_tx
)
SELECT
    u.template,
    CASE
        WHEN u.template IN ('(no revert data)', '(undecodable output)')
                                                  THEN 'NO_DATA'
        WHEN u.template LIKE 'custom_error:%'     THEN 'CUSTOM_ERROR'
        WHEN u.template ~ '^Panic\(0x'            THEN 'PANIC'
        ELSE 'TEXT'
    END                                            AS cluster_kind,
    COUNT(*)                                       AS occurrences,
    ROUND(100.0 * COUNT(*) / GREATEST(t.total_unknown, 1), 2) AS pct_of_unknown,
    SUM(u.gas_used)                                AS total_gas_wasted,
    ROUND(AVG(u.gas_used))                        AS avg_gas_wasted,
    COUNT(DISTINCT tx.from_addr)                   AS distinct_senders,
    COUNT(DISTINCT u.failing_function)             AS distinct_selectors,
    MIN(u.revert_reason)                           AS sample_revert_reason,
    MIN(u.tx_hash)                                 AS sample_tx_hash,
    MIN(u.timestamp)                               AS first_seen,
    MAX(u.timestamp)                               AS last_seen
FROM unknown_tx u
CROSS JOIN total t
LEFT JOIN transaction tx ON tx.tx_hash = u.tx_hash
GROUP BY u.template, t.total_unknown
ORDER BY occurrences DESC, total_gas_wasted DESC;

COMMENT ON VIEW vw_unknown_revert_clusters IS
    'UNKNOWN 실패의 revert 사유를 템플릿 단위로 클러스터링 — 상위 클러스터가 classifier 신규 룰 후보 (빈도·가스 낭비·영향 주소 수 포함)';
