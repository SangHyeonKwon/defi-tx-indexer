-- ============================================
-- 10. 스왑 이벤트와 해당 블록 정보 조회
-- Swap events with associated block information
-- ============================================

SELECT
    b.block_number,
    b.timestamp,
    p.pair_name,
    s.amount_in,
    s.amount_out
FROM swap_event s
JOIN transaction t  ON s.tx_hash       = t.tx_hash
JOIN block b        ON t.block_number   = b.block_number
JOIN pool p         ON s.pool_address   = p.pool_address
ORDER BY b.block_number;
