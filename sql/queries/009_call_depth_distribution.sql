-- ============================================
-- 9. 콜 깊이별 내부 호출 분포
-- Internal call distribution by call depth
-- ============================================

SELECT
    call_depth,
    call_type,
    COUNT(*) AS call_count
FROM trace_log
GROUP BY call_depth, call_type
ORDER BY call_depth;
