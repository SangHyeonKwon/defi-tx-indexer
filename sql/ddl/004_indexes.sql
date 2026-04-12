-- ============================================
-- DeFi Analytics Database (Uniswap V3)
-- DDL: Performance Indexes
-- ============================================

-- block
CREATE INDEX IF NOT EXISTS idx_block_timestamp
    ON block (timestamp);

-- transaction
CREATE INDEX IF NOT EXISTS idx_transaction_block_number
    ON transaction (block_number);

CREATE INDEX IF NOT EXISTS idx_transaction_from_addr
    ON transaction (from_addr);

CREATE INDEX IF NOT EXISTS idx_transaction_to_addr
    ON transaction (to_addr);

CREATE INDEX IF NOT EXISTS idx_transaction_status_failed
    ON transaction (status) WHERE status = 0;

-- swap_event
CREATE INDEX IF NOT EXISTS idx_swap_event_pool_address
    ON swap_event (pool_address);

CREATE INDEX IF NOT EXISTS idx_swap_event_tx_hash
    ON swap_event (tx_hash);

CREATE INDEX IF NOT EXISTS idx_swap_event_sender
    ON swap_event (sender);

CREATE INDEX IF NOT EXISTS idx_swap_event_recipient
    ON swap_event (recipient);

CREATE INDEX IF NOT EXISTS idx_swap_event_timestamp
    ON swap_event (timestamp);

-- liquidity_event
CREATE INDEX IF NOT EXISTS idx_liquidity_event_pool_address
    ON liquidity_event (pool_address);

CREATE INDEX IF NOT EXISTS idx_liquidity_event_tx_hash
    ON liquidity_event (tx_hash);

CREATE INDEX IF NOT EXISTS idx_liquidity_event_provider
    ON liquidity_event (provider);

CREATE INDEX IF NOT EXISTS idx_liquidity_event_timestamp
    ON liquidity_event (timestamp);

CREATE INDEX IF NOT EXISTS idx_liquidity_event_event_type
    ON liquidity_event (event_type);

-- token_transfer
CREATE INDEX IF NOT EXISTS idx_token_transfer_tx_hash
    ON token_transfer (tx_hash);

CREATE INDEX IF NOT EXISTS idx_token_transfer_token_address
    ON token_transfer (token_address);

CREATE INDEX IF NOT EXISTS idx_token_transfer_from_addr
    ON token_transfer (from_addr);

CREATE INDEX IF NOT EXISTS idx_token_transfer_to_addr
    ON token_transfer (to_addr);

CREATE INDEX IF NOT EXISTS idx_token_transfer_timestamp
    ON token_transfer (timestamp);

-- failed_transaction
CREATE INDEX IF NOT EXISTS idx_failed_transaction_error_category
    ON failed_transaction (error_category);

CREATE INDEX IF NOT EXISTS idx_failed_transaction_timestamp
    ON failed_transaction (timestamp);

-- price_snapshot
CREATE INDEX IF NOT EXISTS idx_price_snapshot_pool_ts
    ON price_snapshot (pool_address, snapshot_ts);

CREATE INDEX IF NOT EXISTS idx_price_snapshot_interval_type
    ON price_snapshot (interval_type);

CREATE INDEX IF NOT EXISTS idx_price_snapshot_snapshot_ts
    ON price_snapshot (snapshot_ts);

-- user_profile
CREATE INDEX IF NOT EXISTS idx_user_profile_label
    ON user_profile (label);

CREATE INDEX IF NOT EXISTS idx_user_profile_total_volume_usd
    ON user_profile (total_volume_usd DESC);

-- trace_log
CREATE INDEX IF NOT EXISTS idx_trace_log_tx_hash
    ON trace_log (tx_hash);

CREATE INDEX IF NOT EXISTS idx_trace_log_call_depth
    ON trace_log (call_depth);

CREATE INDEX IF NOT EXISTS idx_trace_log_call_type
    ON trace_log (call_type);
