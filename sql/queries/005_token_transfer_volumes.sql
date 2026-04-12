-- ============================================
-- 5. 토큰별 전송 횟수 및 총 전송량
-- Token transfer counts and total volumes
-- ============================================

SELECT
    tk.symbol,
    tk.name,
    COUNT(tt.transfer_id) AS transfer_count,
    SUM(tt.amount)        AS total_amount
FROM token_transfer tt
JOIN token tk ON tt.token_address = tk.token_address
GROUP BY tk.symbol, tk.name
ORDER BY transfer_count DESC;
