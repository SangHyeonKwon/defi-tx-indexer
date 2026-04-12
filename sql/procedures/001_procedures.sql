-- ============================================
-- DeFi Analytics Database (Uniswap V3)
-- Procedures: 저장 프로시저 5개
-- ============================================

-- ────────────────────────────────────────────
-- 1. sp_register_pool — 새 Uniswap V3 풀 등록
-- ────────────────────────────────────────────
CREATE OR REPLACE PROCEDURE sp_register_pool(
    p_pool_address  VARCHAR(42),
    p_pair_name     VARCHAR(50),
    p_token0_addr   VARCHAR(42),
    p_token0_symbol VARCHAR(20),
    p_token0_name   VARCHAR(100),
    p_token0_dec    SMALLINT,
    p_token1_addr   VARCHAR(42),
    p_token1_symbol VARCHAR(20),
    p_token1_name   VARCHAR(100),
    p_token1_dec    SMALLINT,
    p_fee_tier      INTEGER
)
LANGUAGE plpgsql
AS $$
BEGIN
    -- token0이 없으면 먼저 등록
    INSERT INTO token (token_address, symbol, name, decimals)
    VALUES (p_token0_addr, p_token0_symbol, p_token0_name, p_token0_dec)
    ON CONFLICT (token_address) DO NOTHING;

    -- token1이 없으면 먼저 등록
    INSERT INTO token (token_address, symbol, name, decimals)
    VALUES (p_token1_addr, p_token1_symbol, p_token1_name, p_token1_dec)
    ON CONFLICT (token_address) DO NOTHING;

    -- 풀 등록
    INSERT INTO pool (pool_address, pair_name, token0_address, token1_address, fee_tier)
    VALUES (p_pool_address, p_pair_name, p_token0_addr, p_token1_addr, p_fee_tier)
    ON CONFLICT (pool_address) DO NOTHING;
END;
$$;

-- ────────────────────────────────────────────
-- 2. sp_update_user_profile_after_swap — 스왑 후 유저 프로필 갱신
-- ────────────────────────────────────────────
CREATE OR REPLACE PROCEDURE sp_update_user_profile_after_swap(
    p_user_address      VARCHAR(42),
    p_swap_volume_usd   NUMERIC(20,2),
    p_swap_timestamp    TIMESTAMPTZ
)
LANGUAGE plpgsql
AS $$
DECLARE
    v_total_swaps   INTEGER;
    v_total_volume  NUMERIC(20,2);
    v_label         VARCHAR(50);
BEGIN
    INSERT INTO user_profile (user_address, first_seen, last_seen, total_swaps, total_volume_usd)
    VALUES (p_user_address, p_swap_timestamp, p_swap_timestamp, 1, p_swap_volume_usd)
    ON CONFLICT (user_address) DO UPDATE SET
        last_seen        = GREATEST(user_profile.last_seen, EXCLUDED.last_seen),
        total_swaps      = user_profile.total_swaps + 1,
        total_volume_usd = user_profile.total_volume_usd + EXCLUDED.total_volume_usd;

    -- 갱신된 값 조회
    SELECT total_swaps, total_volume_usd
    INTO v_total_swaps, v_total_volume
    FROM user_profile
    WHERE user_address = p_user_address;

    -- 라벨 자동 분류
    IF v_total_volume > 1000000 THEN
        v_label := 'whale';
    ELSIF v_total_swaps > 100 THEN
        v_label := 'bot';
    ELSE
        v_label := 'retail';
    END IF;

    UPDATE user_profile
    SET label = v_label
    WHERE user_address = p_user_address;
END;
$$;

-- ────────────────────────────────────────────
-- 3. sp_record_failed_transaction — 실패 트랜잭션 기록
-- ────────────────────────────────────────────
CREATE OR REPLACE PROCEDURE sp_record_failed_transaction(
    p_tx_hash           VARCHAR(66),
    p_error_category    error_category,
    p_revert_reason     TEXT,
    p_failing_function  VARCHAR(100),
    p_gas_used          BIGINT,
    p_timestamp         TIMESTAMPTZ
)
LANGUAGE plpgsql
AS $$
BEGIN
    INSERT INTO failed_transaction (tx_hash, error_category, revert_reason, failing_function, gas_used, timestamp)
    VALUES (p_tx_hash, p_error_category, p_revert_reason, p_failing_function, p_gas_used, p_timestamp)
    ON CONFLICT (tx_hash) DO UPDATE SET
        error_category   = EXCLUDED.error_category,
        revert_reason    = EXCLUDED.revert_reason,
        failing_function = EXCLUDED.failing_function;
END;
$$;

-- ────────────────────────────────────────────
-- 4. sp_take_price_snapshot — 가격 스냅샷 캡처
-- ────────────────────────────────────────────
CREATE OR REPLACE PROCEDURE sp_take_price_snapshot(
    p_pool_address  VARCHAR(42),
    p_price         NUMERIC(30,18),
    p_tick          INTEGER,
    p_liquidity     NUMERIC(38,0),
    p_interval_type snapshot_interval
)
LANGUAGE plpgsql
AS $$
DECLARE
    v_snapshot_ts TIMESTAMPTZ;
BEGIN
    -- 현재 시각을 interval_type에 맞게 truncate
    v_snapshot_ts := CASE p_interval_type
        WHEN '1m'  THEN date_trunc('minute', NOW())
        WHEN '5m'  THEN date_trunc('hour', NOW())
                        + INTERVAL '1 minute' * (EXTRACT(MINUTE FROM NOW())::INT / 5 * 5)
        WHEN '15m' THEN date_trunc('hour', NOW())
                        + INTERVAL '1 minute' * (EXTRACT(MINUTE FROM NOW())::INT / 15 * 15)
        WHEN '1h'  THEN date_trunc('hour', NOW())
        WHEN '4h'  THEN date_trunc('day', NOW())
                        + INTERVAL '1 hour' * (EXTRACT(HOUR FROM NOW())::INT / 4 * 4)
        WHEN '1d'  THEN date_trunc('day', NOW())
    END;

    INSERT INTO price_snapshot (pool_address, price, tick, liquidity, snapshot_ts, interval_type)
    VALUES (p_pool_address, p_price, p_tick, p_liquidity, v_snapshot_ts, p_interval_type)
    ON CONFLICT (pool_address, snapshot_ts, interval_type) DO UPDATE SET
        price     = EXCLUDED.price,
        tick      = EXCLUDED.tick,
        liquidity = EXCLUDED.liquidity;
END;
$$;

-- ────────────────────────────────────────────
-- 5. sp_get_pool_stats — 풀 종합 통계 조회
--    (함수로 구현: 결과 테이블 반환)
-- ────────────────────────────────────────────
CREATE OR REPLACE FUNCTION fn_get_pool_stats(
    p_pool_address  VARCHAR(42),
    p_from_date     TIMESTAMPTZ,
    p_to_date       TIMESTAMPTZ
)
RETURNS TABLE (
    pair_name           VARCHAR(50),
    swap_count          BIGINT,
    unique_traders      BIGINT,
    total_volume_in     NUMERIC,
    avg_trade_size      NUMERIC,
    liquidity_events    BIGINT,
    estimated_fees      NUMERIC
)
LANGUAGE plpgsql
AS $$
BEGIN
    RETURN QUERY
    SELECT
        p.pair_name,
        COUNT(s.event_id)                                       AS swap_count,
        COUNT(DISTINCT s.sender)                                AS unique_traders,
        SUM(s.amount_in)                                        AS total_volume_in,
        CASE WHEN COUNT(s.event_id) > 0
             THEN SUM(s.amount_in) / COUNT(s.event_id)
             ELSE 0
        END                                                     AS avg_trade_size,
        (SELECT COUNT(*) FROM liquidity_event le
         WHERE le.pool_address = p_pool_address
           AND le.timestamp BETWEEN p_from_date AND p_to_date)  AS liquidity_events,
        SUM(s.amount_in) * p.fee_tier / 1000000                 AS estimated_fees
    FROM pool p
    LEFT JOIN swap_event s
        ON p.pool_address = s.pool_address
       AND s.timestamp BETWEEN p_from_date AND p_to_date
    WHERE p.pool_address = p_pool_address
    GROUP BY p.pair_name, p.fee_tier;
END;
$$;
