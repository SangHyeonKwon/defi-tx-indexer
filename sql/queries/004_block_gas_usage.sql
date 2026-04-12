-- ============================================
-- 4. 블록별 가스 사용량 및 트랜잭션 수
-- Block gas usage and transaction counts
-- ============================================

SELECT
    b.block_number,
    b.timestamp,
    b.gas_used AS block_gas_used,
    COUNT(t.tx_hash) AS tx_count
FROM block b
LEFT JOIN transaction t ON b.block_number = t.block_number
GROUP BY b.block_number, b.timestamp, b.gas_used
ORDER BY b.block_number;
