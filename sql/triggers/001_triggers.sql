-- ============================================
-- DeFi Analytics Database (Uniswap V3)
-- Triggers: 트리거 5개
-- ============================================

-- ────────────────────────────────────────────
-- 감사 로그 테이블 (트리거 5번에서 사용)
-- ────────────────────────────────────────────
CREATE TABLE IF NOT EXISTS audit_log (
    audit_id    BIGINT GENERATED ALWAYS AS IDENTITY,
    table_name  VARCHAR(50)     NOT NULL,
    event_type  VARCHAR(20)     NOT NULL,
    record_id   TEXT            NOT NULL,
    details     JSONB,
    created_at  TIMESTAMPTZ     NOT NULL DEFAULT NOW(),

    CONSTRAINT audit_log_pkey PRIMARY KEY (audit_id)
);

COMMENT ON TABLE audit_log IS '대규모 이벤트 감사 로그 (트리거 자동 기록)';

-- ────────────────────────────────────────────
-- 1. trg_swap_update_user_profile
--    swap_event INSERT 시 sender/recipient의 user_profile 자동 갱신
-- ────────────────────────────────────────────
CREATE OR REPLACE FUNCTION fn_trg_swap_update_user_profile()
RETURNS TRIGGER
LANGUAGE plpgsql
AS $$
BEGIN
    -- sender 프로필 UPSERT
    INSERT INTO user_profile (user_address, first_seen, last_seen, total_swaps, total_volume_usd)
    VALUES (NEW.sender, NEW.timestamp, NEW.timestamp, 1, 0)
    ON CONFLICT (user_address) DO UPDATE SET
        last_seen   = GREATEST(user_profile.last_seen, EXCLUDED.last_seen),
        total_swaps = user_profile.total_swaps + 1;

    -- recipient가 sender와 다를 경우에만
    IF NEW.recipient <> NEW.sender THEN
        INSERT INTO user_profile (user_address, first_seen, last_seen, total_swaps, total_volume_usd)
        VALUES (NEW.recipient, NEW.timestamp, NEW.timestamp, 0, 0)
        ON CONFLICT (user_address) DO UPDATE SET
            last_seen = GREATEST(user_profile.last_seen, EXCLUDED.last_seen);
    END IF;

    RETURN NEW;
END;
$$;

DROP TRIGGER IF EXISTS trg_swap_update_user_profile ON swap_event;
CREATE TRIGGER trg_swap_update_user_profile
    AFTER INSERT ON swap_event
    FOR EACH ROW
    EXECUTE FUNCTION fn_trg_swap_update_user_profile();

-- ────────────────────────────────────────────
-- 2. trg_transaction_check_failed
--    status=0 transaction INSERT 시 failed_transaction 스텁 자동 생성
-- ────────────────────────────────────────────
CREATE OR REPLACE FUNCTION fn_trg_transaction_check_failed()
RETURNS TRIGGER
LANGUAGE plpgsql
AS $$
BEGIN
    IF NEW.status = 0 THEN
        INSERT INTO failed_transaction (tx_hash, error_category, gas_used, timestamp)
        SELECT NEW.tx_hash, 'UNKNOWN', NEW.gas_used, b.timestamp
        FROM block b WHERE b.block_number = NEW.block_number
        ON CONFLICT (tx_hash) DO NOTHING;
    END IF;
    RETURN NEW;
END;
$$;

DROP TRIGGER IF EXISTS trg_transaction_check_failed ON transaction;
CREATE TRIGGER trg_transaction_check_failed
    AFTER INSERT ON transaction
    FOR EACH ROW
    EXECUTE FUNCTION fn_trg_transaction_check_failed();

-- ────────────────────────────────────────────
-- 3. trg_price_snapshot_notify
--    price_snapshot INSERT 시 NOTIFY로 실시간 알림
-- ────────────────────────────────────────────
CREATE OR REPLACE FUNCTION fn_trg_price_snapshot_notify()
RETURNS TRIGGER
LANGUAGE plpgsql
AS $$
BEGIN
    PERFORM pg_notify(
        'price_update',
        json_build_object(
            'pool_address', NEW.pool_address,
            'price',        NEW.price,
            'tick',         NEW.tick,
            'interval',     NEW.interval_type,
            'timestamp',    NEW.snapshot_ts
        )::TEXT
    );
    RETURN NEW;
END;
$$;

DROP TRIGGER IF EXISTS trg_price_snapshot_notify ON price_snapshot;
CREATE TRIGGER trg_price_snapshot_notify
    AFTER INSERT ON price_snapshot
    FOR EACH ROW
    EXECUTE FUNCTION fn_trg_price_snapshot_notify();

-- ────────────────────────────────────────────
-- 4. trg_block_timestamp_validate
--    블록 타임스탬프가 미래인지 검증 (15초 허용)
-- ────────────────────────────────────────────
CREATE OR REPLACE FUNCTION fn_trg_block_timestamp_validate()
RETURNS TRIGGER
LANGUAGE plpgsql
AS $$
BEGIN
    IF NEW.timestamp > NOW() + INTERVAL '15 seconds' THEN
        RAISE EXCEPTION 'Block timestamp is in the future: % (current: %)',
            NEW.timestamp, NOW();
    END IF;
    RETURN NEW;
END;
$$;

DROP TRIGGER IF EXISTS trg_block_timestamp_validate ON block;
CREATE TRIGGER trg_block_timestamp_validate
    BEFORE INSERT ON block
    FOR EACH ROW
    EXECUTE FUNCTION fn_trg_block_timestamp_validate();

-- ────────────────────────────────────────────
-- 5. trg_liquidity_event_audit
--    대규모 유동성 이벤트 (liquidity > 10^18) 감사 로그 기록
-- ────────────────────────────────────────────
CREATE OR REPLACE FUNCTION fn_trg_liquidity_event_audit()
RETURNS TRIGGER
LANGUAGE plpgsql
AS $$
BEGIN
    IF NEW.liquidity > 1000000000000000000 THEN  -- 10^18
        INSERT INTO audit_log (table_name, event_type, record_id, details)
        VALUES (
            'liquidity_event',
            NEW.event_type::TEXT,
            NEW.event_id::TEXT,
            json_build_object(
                'pool_address',  NEW.pool_address,
                'provider',      NEW.provider,
                'liquidity',     NEW.liquidity,
                'token0_amount', NEW.token0_amount,
                'token1_amount', NEW.token1_amount,
                'timestamp',     NEW.timestamp
            )::JSONB
        );
    END IF;
    RETURN NEW;
END;
$$;

DROP TRIGGER IF EXISTS trg_liquidity_event_audit ON liquidity_event;
CREATE TRIGGER trg_liquidity_event_audit
    AFTER INSERT ON liquidity_event
    FOR EACH ROW
    EXECUTE FUNCTION fn_trg_liquidity_event_audit();
