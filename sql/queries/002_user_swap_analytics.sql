-- ============================================
-- 2. 유저별 스왑 횟수 (라벨 포함)
-- User swap analytics with labels
-- ============================================

SELECT
    u.user_address,
    u.label,
    u.total_swaps,
    u.total_volume_usd
FROM user_profile u
ORDER BY u.total_volume_usd DESC;
