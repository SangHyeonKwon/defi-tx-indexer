-- ============================================
-- 7. 평균 이상의 가스를 사용한 트랜잭션
-- Transactions with above-average gas usage
-- ============================================

SELECT
    tx_hash,
    from_addr,
    gas_used,
    status
FROM transaction
WHERE gas_used > (SELECT AVG(gas_used) FROM transaction)
ORDER BY gas_used DESC;
