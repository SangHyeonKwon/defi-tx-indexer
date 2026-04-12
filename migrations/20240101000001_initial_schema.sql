-- DeFi Analytics Database (Uniswap V3) — Initial Schema Migration
-- Extensions + Types + Tables + Indexes

-- ============================================
-- Extensions
-- ============================================
CREATE EXTENSION IF NOT EXISTS pgcrypto;

-- ============================================
-- ENUM Types
-- ============================================
DO $$ BEGIN
    CREATE TYPE error_category AS ENUM (
        'INSUFFICIENT_BALANCE',
        'SLIPPAGE_EXCEEDED',
        'DEADLINE_EXPIRED',
        'UNAUTHORIZED',
        'TRANSFER_FAILED',
        'UNKNOWN'
    );
EXCEPTION
    WHEN duplicate_object THEN NULL;
END $$;

DO $$ BEGIN
    CREATE TYPE liquidity_event_type AS ENUM (
        'MINT',
        'BURN'
    );
EXCEPTION
    WHEN duplicate_object THEN NULL;
END $$;

DO $$ BEGIN
    CREATE TYPE snapshot_interval AS ENUM (
        '1m',
        '5m',
        '15m',
        '1h',
        '4h',
        '1d'
    );
EXCEPTION
    WHEN duplicate_object THEN NULL;
END $$;

-- ============================================
-- Tables (FK dependency order)
-- ============================================

CREATE TABLE IF NOT EXISTS block (
    block_number    BIGINT          NOT NULL,
    timestamp       TIMESTAMPTZ     NOT NULL,
    gas_used        BIGINT          NOT NULL,

    CONSTRAINT block_pkey PRIMARY KEY (block_number),
    CONSTRAINT block_gas_used_check CHECK (gas_used >= 0)
);

CREATE TABLE IF NOT EXISTS token (
    token_address   VARCHAR(42)     NOT NULL,
    symbol          VARCHAR(20)     NOT NULL,
    name            VARCHAR(100)    NOT NULL,
    decimals        SMALLINT        NOT NULL DEFAULT 18,

    CONSTRAINT token_pkey PRIMARY KEY (token_address),
    CONSTRAINT token_decimals_check CHECK (decimals BETWEEN 0 AND 18),
    CONSTRAINT token_address_format_check CHECK (token_address ~ '^0x[a-fA-F0-9]{40}$')
);

CREATE TABLE IF NOT EXISTS transaction (
    tx_hash         VARCHAR(66)     NOT NULL,
    from_addr       VARCHAR(42)     NOT NULL,
    to_addr         VARCHAR(42),
    block_number    BIGINT          NOT NULL,
    gas_used        BIGINT          NOT NULL,
    gas_price       NUMERIC(30,0)   NOT NULL,
    value           NUMERIC(38,0)   NOT NULL DEFAULT 0,
    status          SMALLINT        NOT NULL DEFAULT 1,
    input_data      TEXT,

    CONSTRAINT transaction_pkey PRIMARY KEY (tx_hash),
    CONSTRAINT transaction_block_number_fkey
        FOREIGN KEY (block_number) REFERENCES block (block_number),
    CONSTRAINT transaction_status_check CHECK (status IN (0, 1)),
    CONSTRAINT transaction_gas_used_check CHECK (gas_used >= 0)
);

CREATE TABLE IF NOT EXISTS pool (
    pool_address        VARCHAR(42)     NOT NULL,
    pair_name           VARCHAR(50)     NOT NULL,
    token0_address      VARCHAR(42)     NOT NULL,
    token1_address      VARCHAR(42)     NOT NULL,
    fee_tier            INTEGER         NOT NULL,
    created_at          TIMESTAMPTZ     NOT NULL DEFAULT NOW(),

    CONSTRAINT pool_pkey PRIMARY KEY (pool_address),
    CONSTRAINT pool_token0_address_fkey
        FOREIGN KEY (token0_address) REFERENCES token (token_address),
    CONSTRAINT pool_token1_address_fkey
        FOREIGN KEY (token1_address) REFERENCES token (token_address),
    CONSTRAINT pool_fee_tier_check CHECK (fee_tier IN (100, 500, 3000, 10000)),
    CONSTRAINT pool_address_format_check CHECK (pool_address ~ '^0x[a-fA-F0-9]{40}$')
);

CREATE TABLE IF NOT EXISTS swap_event (
    event_id        BIGINT GENERATED ALWAYS AS IDENTITY,
    pool_address    VARCHAR(42)     NOT NULL,
    tx_hash         VARCHAR(66)     NOT NULL,
    sender          VARCHAR(42)     NOT NULL,
    recipient       VARCHAR(42)     NOT NULL,
    amount0         NUMERIC(38,0)   NOT NULL,
    amount1         NUMERIC(38,0)   NOT NULL,
    amount_in       NUMERIC(38,0)   NOT NULL,
    amount_out      NUMERIC(38,0)   NOT NULL,
    sqrt_price_x96  NUMERIC(60,0)   NOT NULL,
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

CREATE TABLE IF NOT EXISTS liquidity_event (
    event_id        BIGINT GENERATED ALWAYS AS IDENTITY,
    event_type      liquidity_event_type NOT NULL,
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

CREATE TABLE IF NOT EXISTS price_snapshot (
    snapshot_id     BIGINT GENERATED ALWAYS AS IDENTITY,
    pool_address    VARCHAR(42)         NOT NULL,
    price           NUMERIC(30,18)      NOT NULL,
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

CREATE TABLE IF NOT EXISTS user_profile (
    user_address        VARCHAR(42)     NOT NULL,
    label               VARCHAR(50),
    first_seen          TIMESTAMPTZ     NOT NULL,
    last_seen           TIMESTAMPTZ     NOT NULL,
    total_swaps         INTEGER         NOT NULL DEFAULT 0,
    total_volume_usd    NUMERIC(20,2)   NOT NULL DEFAULT 0,

    CONSTRAINT user_profile_pkey PRIMARY KEY (user_address),
    CONSTRAINT user_profile_total_swaps_check CHECK (total_swaps >= 0),
    CONSTRAINT user_profile_total_volume_check CHECK (total_volume_usd >= 0),
    CONSTRAINT user_profile_seen_order_check CHECK (last_seen >= first_seen)
);

CREATE TABLE IF NOT EXISTS trace_log (
    trace_id        BIGINT GENERATED ALWAYS AS IDENTITY,
    tx_hash         VARCHAR(66)     NOT NULL,
    call_depth      INTEGER         NOT NULL,
    call_type       VARCHAR(20)     NOT NULL,
    from_addr       VARCHAR(42)     NOT NULL,
    to_addr         VARCHAR(42),
    value           NUMERIC(38,0)   NOT NULL DEFAULT 0,
    gas_used        BIGINT          NOT NULL,
    input           TEXT,
    output          TEXT,
    error           TEXT,

    CONSTRAINT trace_log_pkey PRIMARY KEY (trace_id),
    CONSTRAINT trace_log_tx_hash_fkey
        FOREIGN KEY (tx_hash) REFERENCES transaction (tx_hash),
    CONSTRAINT trace_log_call_depth_check CHECK (call_depth >= 0)
);

-- ============================================
-- Performance Indexes
-- ============================================

CREATE INDEX IF NOT EXISTS idx_block_timestamp ON block (timestamp);

CREATE INDEX IF NOT EXISTS idx_transaction_block_number ON transaction (block_number);
CREATE INDEX IF NOT EXISTS idx_transaction_from_addr ON transaction (from_addr);
CREATE INDEX IF NOT EXISTS idx_transaction_to_addr ON transaction (to_addr);
CREATE INDEX IF NOT EXISTS idx_transaction_status_failed ON transaction (status) WHERE status = 0;

CREATE INDEX IF NOT EXISTS idx_swap_event_pool_address ON swap_event (pool_address);
CREATE INDEX IF NOT EXISTS idx_swap_event_tx_hash ON swap_event (tx_hash);
CREATE INDEX IF NOT EXISTS idx_swap_event_sender ON swap_event (sender);
CREATE INDEX IF NOT EXISTS idx_swap_event_recipient ON swap_event (recipient);
CREATE INDEX IF NOT EXISTS idx_swap_event_timestamp ON swap_event (timestamp);

CREATE INDEX IF NOT EXISTS idx_liquidity_event_pool_address ON liquidity_event (pool_address);
CREATE INDEX IF NOT EXISTS idx_liquidity_event_tx_hash ON liquidity_event (tx_hash);
CREATE INDEX IF NOT EXISTS idx_liquidity_event_provider ON liquidity_event (provider);
CREATE INDEX IF NOT EXISTS idx_liquidity_event_timestamp ON liquidity_event (timestamp);
CREATE INDEX IF NOT EXISTS idx_liquidity_event_event_type ON liquidity_event (event_type);

CREATE INDEX IF NOT EXISTS idx_token_transfer_tx_hash ON token_transfer (tx_hash);
CREATE INDEX IF NOT EXISTS idx_token_transfer_token_address ON token_transfer (token_address);
CREATE INDEX IF NOT EXISTS idx_token_transfer_from_addr ON token_transfer (from_addr);
CREATE INDEX IF NOT EXISTS idx_token_transfer_to_addr ON token_transfer (to_addr);
CREATE INDEX IF NOT EXISTS idx_token_transfer_timestamp ON token_transfer (timestamp);

CREATE INDEX IF NOT EXISTS idx_failed_transaction_error_category ON failed_transaction (error_category);
CREATE INDEX IF NOT EXISTS idx_failed_transaction_timestamp ON failed_transaction (timestamp);

CREATE INDEX IF NOT EXISTS idx_price_snapshot_pool_ts ON price_snapshot (pool_address, snapshot_ts);
CREATE INDEX IF NOT EXISTS idx_price_snapshot_interval_type ON price_snapshot (interval_type);
CREATE INDEX IF NOT EXISTS idx_price_snapshot_snapshot_ts ON price_snapshot (snapshot_ts);

CREATE INDEX IF NOT EXISTS idx_user_profile_label ON user_profile (label);
CREATE INDEX IF NOT EXISTS idx_user_profile_total_volume_usd ON user_profile (total_volume_usd DESC);

CREATE INDEX IF NOT EXISTS idx_trace_log_tx_hash ON trace_log (tx_hash);
CREATE INDEX IF NOT EXISTS idx_trace_log_call_depth ON trace_log (call_depth);
CREATE INDEX IF NOT EXISTS idx_trace_log_call_type ON trace_log (call_type);
