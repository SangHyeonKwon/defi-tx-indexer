-- ============================================
-- 3. 실패한 트랜잭션 목록 (에러 카테고리 포함)
-- Failed transactions with error categories
-- ============================================

SELECT
    t.tx_hash,
    t.from_addr,
    f.error_category,
    f.revert_reason,
    f.failing_function
FROM failed_transaction f
JOIN transaction t ON f.tx_hash = t.tx_hash;
