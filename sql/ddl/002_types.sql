-- ============================================
-- DeFi Analytics Database (Uniswap V3)
-- DDL: Custom ENUM Types
-- ============================================

-- 실패한 트랜잭션 에러 카테고리
DO $$ BEGIN
    CREATE TYPE error_category AS ENUM (
        'INSUFFICIENT_BALANCE',
        'SLIPPAGE_EXCEEDED',
        'DEADLINE_EXPIRED',
        'UNAUTHORIZED',
        'TRANSFER_FAILED',
        'UNKNOWN'
    );
EXCEPTION
    WHEN duplicate_object THEN NULL;
END $$;

-- 유동성 이벤트 타입 (Mint/Burn)
DO $$ BEGIN
    CREATE TYPE liquidity_event_type AS ENUM (
        'MINT',
        'BURN'
    );
EXCEPTION
    WHEN duplicate_object THEN NULL;
END $$;

-- 가격 스냅샷 시간 간격
DO $$ BEGIN
    CREATE TYPE snapshot_interval AS ENUM (
        '1m',
        '5m',
        '15m',
        '1h',
        '4h',
        '1d'
    );
EXCEPTION
    WHEN duplicate_object THEN NULL;
END $$;
