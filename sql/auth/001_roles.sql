-- ============================================
-- DeFi Analytics Database (Uniswap V3)
-- Auth: 역할, 권한, Row-Level Security
-- ============================================

-- ────────────────────────────────────────────
-- 1. 역할 생성 (멱등성 보장)
-- ────────────────────────────────────────────

-- defi_readonly: 분석가 역할 (SELECT만 허용)
DO $$ BEGIN
    CREATE ROLE defi_readonly NOLOGIN;
EXCEPTION
    WHEN duplicate_object THEN NULL;
END $$;

-- defi_indexer: 인덱서 역할 (INSERT/UPDATE 허용, DELETE 불가)
DO $$ BEGIN
    CREATE ROLE defi_indexer NOLOGIN;
EXCEPTION
    WHEN duplicate_object THEN NULL;
END $$;

-- defi_admin: 관리자 역할 (전체 권한)
DO $$ BEGIN
    CREATE ROLE defi_admin NOLOGIN;
EXCEPTION
    WHEN duplicate_object THEN NULL;
END $$;

-- ────────────────────────────────────────────
-- 2. 스키마 접근 권한
-- ────────────────────────────────────────────
GRANT USAGE ON SCHEMA public TO defi_readonly;
GRANT USAGE ON SCHEMA public TO defi_indexer;
GRANT USAGE ON SCHEMA public TO defi_admin;

-- ────────────────────────────────────────────
-- 3. defi_readonly 권한 — SELECT만
-- ────────────────────────────────────────────
GRANT SELECT ON ALL TABLES IN SCHEMA public TO defi_readonly;
ALTER DEFAULT PRIVILEGES IN SCHEMA public
    GRANT SELECT ON TABLES TO defi_readonly;

-- ────────────────────────────────────────────
-- 4. defi_indexer 권한 — SELECT + INSERT + UPDATE (DELETE 불가)
-- ────────────────────────────────────────────
GRANT SELECT, INSERT, UPDATE ON ALL TABLES IN SCHEMA public TO defi_indexer;
GRANT USAGE, SELECT ON ALL SEQUENCES IN SCHEMA public TO defi_indexer;
ALTER DEFAULT PRIVILEGES IN SCHEMA public
    GRANT SELECT, INSERT, UPDATE ON TABLES TO defi_indexer;
ALTER DEFAULT PRIVILEGES IN SCHEMA public
    GRANT USAGE, SELECT ON SEQUENCES TO defi_indexer;

-- ────────────────────────────────────────────
-- 5. defi_admin 권한 — 전체
-- ────────────────────────────────────────────
GRANT ALL PRIVILEGES ON ALL TABLES IN SCHEMA public TO defi_admin;
GRANT ALL PRIVILEGES ON ALL SEQUENCES IN SCHEMA public TO defi_admin;
ALTER DEFAULT PRIVILEGES IN SCHEMA public
    GRANT ALL PRIVILEGES ON TABLES TO defi_admin;
ALTER DEFAULT PRIVILEGES IN SCHEMA public
    GRANT ALL PRIVILEGES ON SEQUENCES TO defi_admin;

-- 프로시저/함수 실행 권한
GRANT EXECUTE ON ALL FUNCTIONS IN SCHEMA public TO defi_admin;
GRANT EXECUTE ON ALL PROCEDURES IN SCHEMA public TO defi_admin;
-- indexer도 프로시저 실행 가능
GRANT EXECUTE ON ALL FUNCTIONS IN SCHEMA public TO defi_indexer;
GRANT EXECUTE ON ALL PROCEDURES IN SCHEMA public TO defi_indexer;

-- ────────────────────────────────────────────
-- 6. Row-Level Security (user_profile 테이블)
-- ────────────────────────────────────────────
ALTER TABLE user_profile ENABLE ROW LEVEL SECURITY;

-- readonly: 활동 기록이 있는 유저만 조회 가능
DROP POLICY IF EXISTS user_profile_readonly_policy ON user_profile;
CREATE POLICY user_profile_readonly_policy ON user_profile
    FOR SELECT
    TO defi_readonly
    USING (total_swaps > 0);

-- indexer: 모든 행 접근 가능
DROP POLICY IF EXISTS user_profile_indexer_policy ON user_profile;
CREATE POLICY user_profile_indexer_policy ON user_profile
    FOR ALL
    TO defi_indexer
    USING (TRUE)
    WITH CHECK (TRUE);

-- admin: 모든 행 접근 가능
DROP POLICY IF EXISTS user_profile_admin_policy ON user_profile;
CREATE POLICY user_profile_admin_policy ON user_profile
    FOR ALL
    TO defi_admin
    USING (TRUE)
    WITH CHECK (TRUE);

-- ────────────────────────────────────────────
-- 7. 샘플 로그인 사용자
-- ────────────────────────────────────────────
DO $$ BEGIN
    CREATE ROLE analyst_alice LOGIN PASSWORD 'analyst_pass' IN ROLE defi_readonly;
EXCEPTION
    WHEN duplicate_object THEN NULL;
END $$;

DO $$ BEGIN
    CREATE ROLE indexer_bot LOGIN PASSWORD 'indexer_pass' IN ROLE defi_indexer;
EXCEPTION
    WHEN duplicate_object THEN NULL;
END $$;

DO $$ BEGIN
    CREATE ROLE admin_owner LOGIN PASSWORD 'admin_pass' IN ROLE defi_admin;
EXCEPTION
    WHEN duplicate_object THEN NULL;
END $$;
