-- ============================================
-- DeFi Analytics Database (Uniswap V3)
-- DDL: Table Definitions (11 tables)
-- 생성 순서: FK 의존성 기준
-- ============================================

-- ============================================
-- 1. block — 이더리움 블록
-- ============================================
CREATE TABLE IF NOT EXISTS block (
    block_number    BIGINT          NOT NULL,
    timestamp       TIMESTAMPTZ     NOT NULL,
    gas_used        BIGINT          NOT NULL,

    CONSTRAINT block_pkey PRIMARY KEY (block_number),
    CONSTRAINT block_gas_used_check CHECK (gas_used >= 0)
);

-- ============================================
-- 2. token — ERC-20 토큰 메타데이터
-- ============================================
CREATE TABLE IF NOT EXISTS token (
    token_address   VARCHAR(42)     NOT NULL,
    symbol          VARCHAR(20)     NOT NULL,
    name            VARCHAR(100)    NOT NULL,
    decimals        SMALLINT        NOT NULL DEFAULT 18,

    CONSTRAINT token_pkey PRIMARY KEY (token_address),
    CONSTRAINT token_decimals_check CHECK (decimals BETWEEN 0 AND 18),
    CONSTRAINT token_address_format_check CHECK (token_address ~ '^0x[a-fA-F0-9]{40}$')
);

-- ============================================
-- 3. transaction — 이더리움 트랜잭션
-- ============================================
CREATE TABLE IF NOT EXISTS transaction (
    tx_hash         VARCHAR(66)     NOT NULL,
    from_addr       VARCHAR(42)     NOT NULL,
    to_addr         VARCHAR(42),                    -- NULL: 컨트랙트 생성 트랜잭션
    block_number    BIGINT          NOT NULL,
    gas_used        BIGINT          NOT NULL,
    gas_price       NUMERIC(30,0)   NOT NULL,       -- wei 단위
    value           NUMERIC(38,0)   NOT NULL DEFAULT 0,  -- wei 단위
    status          SMALLINT        NOT NULL DEFAULT 1,  -- 1=성공, 0=실패
    input_data      TEXT,

    CONSTRAINT transaction_pkey PRIMARY KEY (tx_hash),
    CONSTRAINT transaction_block_number_fkey
        FOREIGN KEY (block_number) REFERENCES block (block_number),
    CONSTRAINT transaction_status_check CHECK (status IN (0, 1)),
    CONSTRAINT transaction_gas_used_check CHECK (gas_used >= 0)
);

-- ============================================
-- 4. pool — Uniswap V3 유동성 풀
-- ============================================
CREATE TABLE IF NOT EXISTS pool (
    pool_address        VARCHAR(42)     NOT NULL,
    pair_name           VARCHAR(50)     NOT NULL,       -- e.g. 'WETH/USDC'
    token0_address      VARCHAR(42)     NOT NULL,
    token1_address      VARCHAR(42)     NOT NULL,
    fee_tier            INTEGER         NOT NULL,       -- 100, 500, 3000, 10000 (bps)
    created_at          TIMESTAMPTZ     NOT NULL DEFAULT NOW(),

    CONSTRAINT pool_pkey PRIMARY KEY (pool_address),
    CONSTRAINT pool_token0_address_fkey
        FOREIGN KEY (token0_address) REFERENCES token (token_address),
    CONSTRAINT pool_token1_address_fkey
        FOREIGN KEY (token1_address) REFERENCES token (token_address),
    CONSTRAINT pool_fee_tier_check CHECK (fee_tier IN (100, 500, 3000, 10000)),
    CONSTRAINT pool_address_format_check CHECK (pool_address ~ '^0x[a-fA-F0-9]{40}$')
);

-- ============================================
-- 5. swap_event — Uniswap V3 스왑 이벤트
-- ============================================
CREATE TABLE IF NOT EXISTS swap_event (
    event_id        BIGINT GENERATED ALWAYS AS IDENTITY,
    pool_address    VARCHAR(42)     NOT NULL,
    tx_hash         VARCHAR(66)     NOT NULL,
    sender          VARCHAR(42)     NOT NULL,
    recipient       VARCHAR(42)     NOT NULL,
    amount0         NUMERIC(38,0)   NOT NULL,       -- 부호 있음 (음수 가능)
    amount1         NUMERIC(38,0)   NOT NULL,       -- 부호 있음 (음수 가능)
    amount_in       NUMERIC(38,0)   NOT NULL,
    amount_out      NUMERIC(38,0)   NOT NULL,
    sqrt_price_x96  NUMERIC(60,0)   NOT NULL,       -- Uniswap V3 가격 인코딩
    liquidity       NUMERIC(38,0)   NOT NULL,
    tick            INTEGER         NOT NULL,
    log_index       INTEGER         NOT NULL,
    timestamp       TIMESTAMPTZ     NOT NULL,

    CONSTRAINT swap_event_pkey PRIMARY KEY (event_id),
    CONSTRAINT swap_event_pool_address_fkey
        FOREIGN KEY (pool_address) REFERENCES pool (pool_address),
    CONSTRAINT swap_event_tx_hash_fkey
        FOREIGN KEY (tx_hash) REFERENCES transaction (tx_hash),
    CONSTRAINT swap_event_tx_log_unique UNIQUE (tx_hash, log_index)
);

-- ============================================
-- 6. liquidity_event — 유동성 공급/회수 이벤트 (Mint/Burn)
-- ============================================
CREATE TABLE IF NOT EXISTS liquidity_event (
    event_id        BIGINT GENERATED ALWAYS AS IDENTITY,
    event_type      liquidity_event_type NOT NULL,  -- 'MINT' or 'BURN'
    pool_address    VARCHAR(42)     NOT NULL,
    tx_hash         VARCHAR(66)     NOT NULL,
    provider        VARCHAR(42)     NOT NULL,
    token0_amount   NUMERIC(38,0)   NOT NULL,
    token1_amount   NUMERIC(38,0)   NOT NULL,
    tick_lower      INTEGER         NOT NULL,
    tick_upper      INTEGER         NOT NULL,
    liquidity       NUMERIC(38,0)   NOT NULL,
    log_index       INTEGER         NOT NULL,
    timestamp       TIMESTAMPTZ     NOT NULL,

    CONSTRAINT liquidity_event_pkey PRIMARY KEY (event_id),
    CONSTRAINT liquidity_event_pool_address_fkey
        FOREIGN KEY (pool_address) REFERENCES pool (pool_address),
    CONSTRAINT liquidity_event_tx_hash_fkey
        FOREIGN KEY (tx_hash) REFERENCES transaction (tx_hash),
    CONSTRAINT liquidity_event_tx_log_unique UNIQUE (tx_hash, log_index),
    CONSTRAINT liquidity_event_tick_range_check CHECK (tick_lower < tick_upper)
);

-- ============================================
-- 7. token_transfer — ERC-20 토큰 전송
-- ============================================
CREATE TABLE IF NOT EXISTS token_transfer (
    transfer_id     BIGINT GENERATED ALWAYS AS IDENTITY,
    tx_hash         VARCHAR(66)     NOT NULL,
    token_address   VARCHAR(42)     NOT NULL,
    from_addr       VARCHAR(42)     NOT NULL,
    to_addr         VARCHAR(42)     NOT NULL,
    amount          NUMERIC(38,0)   NOT NULL,
    log_index       INTEGER         NOT NULL,
    timestamp       TIMESTAMPTZ     NOT NULL,

    CONSTRAINT token_transfer_pkey PRIMARY KEY (transfer_id),
    CONSTRAINT token_transfer_tx_hash_fkey
        FOREIGN KEY (tx_hash) REFERENCES transaction (tx_hash),
    CONSTRAINT token_transfer_token_address_fkey
        FOREIGN KEY (token_address) REFERENCES token (token_address),
    CONSTRAINT token_transfer_tx_log_unique UNIQUE (tx_hash, log_index),
    CONSTRAINT token_transfer_amount_check CHECK (amount >= 0)
);

-- ============================================
-- 8. failed_transaction — 실패한 트랜잭션 상세
-- ============================================
CREATE TABLE IF NOT EXISTS failed_transaction (
    tx_hash             VARCHAR(66)     NOT NULL,
    error_category      error_category  NOT NULL DEFAULT 'UNKNOWN',
    revert_reason       TEXT,
    failing_function    VARCHAR(100),
    gas_used            BIGINT          NOT NULL,
    timestamp           TIMESTAMPTZ     NOT NULL,

    CONSTRAINT failed_transaction_pkey PRIMARY KEY (tx_hash),
    CONSTRAINT failed_transaction_tx_hash_fkey
        FOREIGN KEY (tx_hash) REFERENCES transaction (tx_hash),
    CONSTRAINT failed_transaction_gas_used_check CHECK (gas_used >= 0)
);

-- ============================================
-- 9. price_snapshot — 풀 가격 스냅샷
-- ============================================
CREATE TABLE IF NOT EXISTS price_snapshot (
    snapshot_id     BIGINT GENERATED ALWAYS AS IDENTITY,
    pool_address    VARCHAR(42)         NOT NULL,
    price           NUMERIC(30,18)      NOT NULL,   -- 고정밀 DeFi 가격
    tick            INTEGER             NOT NULL,
    liquidity       NUMERIC(38,0)       NOT NULL,
    snapshot_ts     TIMESTAMPTZ         NOT NULL,
    interval_type   snapshot_interval   NOT NULL,

    CONSTRAINT price_snapshot_pkey PRIMARY KEY (snapshot_id),
    CONSTRAINT price_snapshot_pool_address_fkey
        FOREIGN KEY (pool_address) REFERENCES pool (pool_address),
    CONSTRAINT price_snapshot_pool_ts_interval_unique
        UNIQUE (pool_address, snapshot_ts, interval_type)
);

-- ============================================
-- 10. user_profile — 유저 프로필 (집계 테이블)
-- ============================================
CREATE TABLE IF NOT EXISTS user_profile (
    user_address        VARCHAR(42)     NOT NULL,
    label               VARCHAR(50),                    -- 'whale', 'bot', 'retail' 등
    first_seen          TIMESTAMPTZ     NOT NULL,
    last_seen           TIMESTAMPTZ     NOT NULL,
    total_swaps         INTEGER         NOT NULL DEFAULT 0,
    total_volume_usd    NUMERIC(20,2)   NOT NULL DEFAULT 0,

    CONSTRAINT user_profile_pkey PRIMARY KEY (user_address),
    CONSTRAINT user_profile_total_swaps_check CHECK (total_swaps >= 0),
    CONSTRAINT user_profile_total_volume_check CHECK (total_volume_usd >= 0),
    CONSTRAINT user_profile_seen_order_check CHECK (last_seen >= first_seen)
);

-- ============================================
-- 11. trace_log — 트랜잭션 내부 호출 트레이스
-- ============================================
CREATE TABLE IF NOT EXISTS trace_log (
    trace_id        BIGINT GENERATED ALWAYS AS IDENTITY,
    tx_hash         VARCHAR(66)     NOT NULL,
    call_depth      INTEGER         NOT NULL,
    call_type       VARCHAR(20)     NOT NULL,           -- CALL, DELEGATECALL, STATICCALL, CREATE, CREATE2
    from_addr       VARCHAR(42)     NOT NULL,
    to_addr         VARCHAR(42),                        -- NULL: CREATE 트랜잭션
    value           NUMERIC(38,0)   NOT NULL DEFAULT 0, -- wei 단위
    gas_used        BIGINT          NOT NULL,
    input           TEXT,
    output          TEXT,
    error           TEXT,

    CONSTRAINT trace_log_pkey PRIMARY KEY (trace_id),
    CONSTRAINT trace_log_tx_hash_fkey
        FOREIGN KEY (tx_hash) REFERENCES transaction (tx_hash),
    CONSTRAINT trace_log_call_depth_check CHECK (call_depth >= 0)
);
