-- Relax event table FK constraints for lazy indexing.
-- Pool/token discovery is not yet implemented, so the indexer cannot
-- guarantee that pool and token rows exist before inserting events.
-- These soft references are still maintained via JOINs in analytical views.

BEGIN;

-- Drop FK constraints that block event insertion without pre-existing pool/token rows
ALTER TABLE swap_event DROP CONSTRAINT IF EXISTS swap_event_pool_address_fkey;
ALTER TABLE liquidity_event DROP CONSTRAINT IF EXISTS liquidity_event_pool_address_fkey;
ALTER TABLE token_transfer DROP CONSTRAINT IF EXISTS token_transfer_token_address_fkey;

COMMENT ON COLUMN swap_event.pool_address IS 'Soft reference to pool.pool_address — FK removed for lazy indexing';
COMMENT ON COLUMN liquidity_event.pool_address IS 'Soft reference to pool.pool_address — FK removed for lazy indexing';
COMMENT ON COLUMN token_transfer.token_address IS 'Soft reference to token.token_address — FK removed for lazy indexing';

-- Checkpoint table for crash recovery and resumption
CREATE TABLE IF NOT EXISTS indexer_checkpoint (
    checkpoint_id SERIAL PRIMARY KEY,
    chain_id      INT NOT NULL DEFAULT 1,
    last_processed_block BIGINT NOT NULL,
    updated_at    TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    CONSTRAINT indexer_checkpoint_chain_id_unique UNIQUE (chain_id)
);

COMMENT ON TABLE indexer_checkpoint IS 'Tracks the last successfully indexed block per chain for crash recovery';
COMMENT ON COLUMN indexer_checkpoint.chain_id IS '1 = Ethereum mainnet';
COMMENT ON COLUMN indexer_checkpoint.last_processed_block IS 'Last block number that was fully processed and committed';

COMMIT;
