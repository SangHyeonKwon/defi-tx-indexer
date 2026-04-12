-- ================================================================
-- DeFi Analytics Database (Uniswap V3)
-- 통합 제출 스크립트 (PostgreSQL 16+)
--
-- 실행: psql $DATABASE_URL -f sql/full_script.sql
--
-- 구성:
--   1. Extensions          7. Triggers
--   2. ENUM Types          8. Views
--   3. Tables (11)         9. Procedures & Functions
--   4. Indexes            10. Auth (Roles, RLS)
--   5. Comments           11. Queries (10 SELECT)
--   6. Seed Data (DML)    12. OLAP (8 Window Functions)
-- ================================================================

BEGIN;

-- ================================================================
-- 1. EXTENSIONS
-- ================================================================

CREATE EXTENSION IF NOT EXISTS pgcrypto;

-- ================================================================
-- 2. ENUM TYPES
-- ================================================================

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

-- ================================================================
-- 3. TABLES (11개, FK 의존성 순서)
-- ================================================================

-- 1. block — 이더리움 블록
CREATE TABLE IF NOT EXISTS block (
    block_number    BIGINT          NOT NULL,
    timestamp       TIMESTAMPTZ     NOT NULL,
    gas_used        BIGINT          NOT NULL,

    CONSTRAINT block_pkey PRIMARY KEY (block_number),
    CONSTRAINT block_gas_used_check CHECK (gas_used >= 0)
);

-- 2. token — ERC-20 토큰 메타데이터
CREATE TABLE IF NOT EXISTS token (
    token_address   VARCHAR(42)     NOT NULL,
    symbol          VARCHAR(20)     NOT NULL,
    name            VARCHAR(100)    NOT NULL,
    decimals        SMALLINT        NOT NULL DEFAULT 18,

    CONSTRAINT token_pkey PRIMARY KEY (token_address),
    CONSTRAINT token_decimals_check CHECK (decimals BETWEEN 0 AND 18),
    CONSTRAINT token_address_format_check CHECK (token_address ~ '^0x[a-fA-F0-9]{40}$')
);

-- 3. transaction — 이더리움 트랜잭션
CREATE TABLE IF NOT EXISTS transaction (
    tx_hash         VARCHAR(66)     NOT NULL,
    from_addr       VARCHAR(42)     NOT NULL,
    to_addr         VARCHAR(42),
    block_number    BIGINT          NOT NULL,
    gas_used        BIGINT          NOT NULL,
    gas_price       NUMERIC(30,0)   NOT NULL,
    value           NUMERIC(38,0)   NOT NULL DEFAULT 0,
    status          SMALLINT        NOT NULL DEFAULT 1,
    input_data      TEXT,

    CONSTRAINT transaction_pkey PRIMARY KEY (tx_hash),
    CONSTRAINT transaction_block_number_fkey
        FOREIGN KEY (block_number) REFERENCES block (block_number),
    CONSTRAINT transaction_status_check CHECK (status IN (0, 1)),
    CONSTRAINT transaction_gas_used_check CHECK (gas_used >= 0)
);

-- 4. pool — Uniswap V3 유동성 풀
CREATE TABLE IF NOT EXISTS pool (
    pool_address        VARCHAR(42)     NOT NULL,
    pair_name           VARCHAR(50)     NOT NULL,
    token0_address      VARCHAR(42)     NOT NULL,
    token1_address      VARCHAR(42)     NOT NULL,
    fee_tier            INTEGER         NOT NULL,
    created_at          TIMESTAMPTZ     NOT NULL DEFAULT NOW(),

    CONSTRAINT pool_pkey PRIMARY KEY (pool_address),
    CONSTRAINT pool_token0_address_fkey
        FOREIGN KEY (token0_address) REFERENCES token (token_address),
    CONSTRAINT pool_token1_address_fkey
        FOREIGN KEY (token1_address) REFERENCES token (token_address),
    CONSTRAINT pool_fee_tier_check CHECK (fee_tier IN (100, 500, 3000, 10000)),
    CONSTRAINT pool_address_format_check CHECK (pool_address ~ '^0x[a-fA-F0-9]{40}$')
);

-- 5. swap_event — Uniswap V3 스왑 이벤트
CREATE TABLE IF NOT EXISTS swap_event (
    event_id        BIGINT GENERATED ALWAYS AS IDENTITY,
    pool_address    VARCHAR(42)     NOT NULL,
    tx_hash         VARCHAR(66)     NOT NULL,
    sender          VARCHAR(42)     NOT NULL,
    recipient       VARCHAR(42)     NOT NULL,
    amount0         NUMERIC(38,0)   NOT NULL,
    amount1         NUMERIC(38,0)   NOT NULL,
    amount_in       NUMERIC(38,0)   NOT NULL,
    amount_out      NUMERIC(38,0)   NOT NULL,
    sqrt_price_x96  NUMERIC(60,0)   NOT NULL,
    liquidity       NUMERIC(38,0)   NOT NULL,
    tick            INTEGER         NOT NULL,
    log_index       INTEGER         NOT NULL,
    timestamp       TIMESTAMPTZ     NOT NULL,

    CONSTRAINT swap_event_pkey PRIMARY KEY (event_id),
    CONSTRAINT swap_event_pool_address_fkey
        FOREIGN KEY (pool_address) REFERENCES pool (pool_address),
    CONSTRAINT swap_event_tx_hash_fkey
        FOREIGN KEY (tx_hash) REFERENCES transaction (tx_hash),
    CONSTRAINT swap_event_tx_log_unique UNIQUE (tx_hash, log_index)
);

-- 6. liquidity_event — 유동성 공급/회수 이벤트 (Mint/Burn)
CREATE TABLE IF NOT EXISTS liquidity_event (
    event_id        BIGINT GENERATED ALWAYS AS IDENTITY,
    event_type      liquidity_event_type NOT NULL,
    pool_address    VARCHAR(42)     NOT NULL,
    tx_hash         VARCHAR(66)     NOT NULL,
    provider        VARCHAR(42)     NOT NULL,
    token0_amount   NUMERIC(38,0)   NOT NULL,
    token1_amount   NUMERIC(38,0)   NOT NULL,
    tick_lower      INTEGER         NOT NULL,
    tick_upper      INTEGER         NOT NULL,
    liquidity       NUMERIC(38,0)   NOT NULL,
    log_index       INTEGER         NOT NULL,
    timestamp       TIMESTAMPTZ     NOT NULL,

    CONSTRAINT liquidity_event_pkey PRIMARY KEY (event_id),
    CONSTRAINT liquidity_event_pool_address_fkey
        FOREIGN KEY (pool_address) REFERENCES pool (pool_address),
    CONSTRAINT liquidity_event_tx_hash_fkey
        FOREIGN KEY (tx_hash) REFERENCES transaction (tx_hash),
    CONSTRAINT liquidity_event_tx_log_unique UNIQUE (tx_hash, log_index),
    CONSTRAINT liquidity_event_tick_range_check CHECK (tick_lower < tick_upper)
);

-- 7. token_transfer — ERC-20 토큰 전송
CREATE TABLE IF NOT EXISTS token_transfer (
    transfer_id     BIGINT GENERATED ALWAYS AS IDENTITY,
    tx_hash         VARCHAR(66)     NOT NULL,
    token_address   VARCHAR(42)     NOT NULL,
    from_addr       VARCHAR(42)     NOT NULL,
    to_addr         VARCHAR(42)     NOT NULL,
    amount          NUMERIC(38,0)   NOT NULL,
    log_index       INTEGER         NOT NULL,
    timestamp       TIMESTAMPTZ     NOT NULL,

    CONSTRAINT token_transfer_pkey PRIMARY KEY (transfer_id),
    CONSTRAINT token_transfer_tx_hash_fkey
        FOREIGN KEY (tx_hash) REFERENCES transaction (tx_hash),
    CONSTRAINT token_transfer_token_address_fkey
        FOREIGN KEY (token_address) REFERENCES token (token_address),
    CONSTRAINT token_transfer_tx_log_unique UNIQUE (tx_hash, log_index),
    CONSTRAINT token_transfer_amount_check CHECK (amount >= 0)
);

-- 8. failed_transaction — 실패한 트랜잭션 상세
CREATE TABLE IF NOT EXISTS failed_transaction (
    tx_hash             VARCHAR(66)     NOT NULL,
    error_category      error_category  NOT NULL DEFAULT 'UNKNOWN',
    revert_reason       TEXT,
    failing_function    VARCHAR(100),
    gas_used            BIGINT          NOT NULL,
    timestamp           TIMESTAMPTZ     NOT NULL,

    CONSTRAINT failed_transaction_pkey PRIMARY KEY (tx_hash),
    CONSTRAINT failed_transaction_tx_hash_fkey
        FOREIGN KEY (tx_hash) REFERENCES transaction (tx_hash),
    CONSTRAINT failed_transaction_gas_used_check CHECK (gas_used >= 0)
);

-- 9. price_snapshot — 풀 가격 스냅샷
CREATE TABLE IF NOT EXISTS price_snapshot (
    snapshot_id     BIGINT GENERATED ALWAYS AS IDENTITY,
    pool_address    VARCHAR(42)         NOT NULL,
    price           NUMERIC(30,18)      NOT NULL,
    tick            INTEGER             NOT NULL,
    liquidity       NUMERIC(38,0)       NOT NULL,
    snapshot_ts     TIMESTAMPTZ         NOT NULL,
    interval_type   snapshot_interval   NOT NULL,

    CONSTRAINT price_snapshot_pkey PRIMARY KEY (snapshot_id),
    CONSTRAINT price_snapshot_pool_address_fkey
        FOREIGN KEY (pool_address) REFERENCES pool (pool_address),
    CONSTRAINT price_snapshot_pool_ts_interval_unique
        UNIQUE (pool_address, snapshot_ts, interval_type)
);

-- 10. user_profile — 유저 프로필 (집계 테이블)
CREATE TABLE IF NOT EXISTS user_profile (
    user_address        VARCHAR(42)     NOT NULL,
    label               VARCHAR(50),
    first_seen          TIMESTAMPTZ     NOT NULL,
    last_seen           TIMESTAMPTZ     NOT NULL,
    total_swaps         INTEGER         NOT NULL DEFAULT 0,
    total_volume_usd    NUMERIC(20,2)   NOT NULL DEFAULT 0,

    CONSTRAINT user_profile_pkey PRIMARY KEY (user_address),
    CONSTRAINT user_profile_total_swaps_check CHECK (total_swaps >= 0),
    CONSTRAINT user_profile_total_volume_check CHECK (total_volume_usd >= 0),
    CONSTRAINT user_profile_seen_order_check CHECK (last_seen >= first_seen)
);

-- 11. trace_log — 트���잭션 내부 호출 트레이스
CREATE TABLE IF NOT EXISTS trace_log (
    trace_id        BIGINT GENERATED ALWAYS AS IDENTITY,
    tx_hash         VARCHAR(66)     NOT NULL,
    call_depth      INTEGER         NOT NULL,
    call_type       VARCHAR(20)     NOT NULL,
    from_addr       VARCHAR(42)     NOT NULL,
    to_addr         VARCHAR(42),
    value           NUMERIC(38,0)   NOT NULL DEFAULT 0,
    gas_used        BIGINT          NOT NULL,
    input           TEXT,
    output          TEXT,
    error           TEXT,

    CONSTRAINT trace_log_pkey PRIMARY KEY (trace_id),
    CONSTRAINT trace_log_tx_hash_fkey
        FOREIGN KEY (tx_hash) REFERENCES transaction (tx_hash),
    CONSTRAINT trace_log_call_depth_check CHECK (call_depth >= 0)
);

-- 감사 로그 테이블 (트리거에서 사용)
CREATE TABLE IF NOT EXISTS audit_log (
    audit_id    BIGINT GENERATED ALWAYS AS IDENTITY,
    table_name  VARCHAR(50)     NOT NULL,
    event_type  VARCHAR(20)     NOT NULL,
    record_id   TEXT            NOT NULL,
    details     JSONB,
    created_at  TIMESTAMPTZ     NOT NULL DEFAULT NOW(),

    CONSTRAINT audit_log_pkey PRIMARY KEY (audit_id)
);

-- ================================================================
-- 4. INDEXES
-- ================================================================

-- block
CREATE INDEX IF NOT EXISTS idx_block_timestamp
    ON block (timestamp);

-- transaction
CREATE INDEX IF NOT EXISTS idx_transaction_block_number
    ON transaction (block_number);
CREATE INDEX IF NOT EXISTS idx_transaction_from_addr
    ON transaction (from_addr);
CREATE INDEX IF NOT EXISTS idx_transaction_to_addr
    ON transaction (to_addr);
CREATE INDEX IF NOT EXISTS idx_transaction_status_failed
    ON transaction (status) WHERE status = 0;

-- swap_event
CREATE INDEX IF NOT EXISTS idx_swap_event_pool_address
    ON swap_event (pool_address);
CREATE INDEX IF NOT EXISTS idx_swap_event_tx_hash
    ON swap_event (tx_hash);
CREATE INDEX IF NOT EXISTS idx_swap_event_sender
    ON swap_event (sender);
CREATE INDEX IF NOT EXISTS idx_swap_event_recipient
    ON swap_event (recipient);
CREATE INDEX IF NOT EXISTS idx_swap_event_timestamp
    ON swap_event (timestamp);

-- liquidity_event
CREATE INDEX IF NOT EXISTS idx_liquidity_event_pool_address
    ON liquidity_event (pool_address);
CREATE INDEX IF NOT EXISTS idx_liquidity_event_tx_hash
    ON liquidity_event (tx_hash);
CREATE INDEX IF NOT EXISTS idx_liquidity_event_provider
    ON liquidity_event (provider);
CREATE INDEX IF NOT EXISTS idx_liquidity_event_timestamp
    ON liquidity_event (timestamp);
CREATE INDEX IF NOT EXISTS idx_liquidity_event_event_type
    ON liquidity_event (event_type);

-- token_transfer
CREATE INDEX IF NOT EXISTS idx_token_transfer_tx_hash
    ON token_transfer (tx_hash);
CREATE INDEX IF NOT EXISTS idx_token_transfer_token_address
    ON token_transfer (token_address);
CREATE INDEX IF NOT EXISTS idx_token_transfer_from_addr
    ON token_transfer (from_addr);
CREATE INDEX IF NOT EXISTS idx_token_transfer_to_addr
    ON token_transfer (to_addr);
CREATE INDEX IF NOT EXISTS idx_token_transfer_timestamp
    ON token_transfer (timestamp);

-- failed_transaction
CREATE INDEX IF NOT EXISTS idx_failed_transaction_error_category
    ON failed_transaction (error_category);
CREATE INDEX IF NOT EXISTS idx_failed_transaction_timestamp
    ON failed_transaction (timestamp);

-- price_snapshot
CREATE INDEX IF NOT EXISTS idx_price_snapshot_pool_ts
    ON price_snapshot (pool_address, snapshot_ts);
CREATE INDEX IF NOT EXISTS idx_price_snapshot_interval_type
    ON price_snapshot (interval_type);
CREATE INDEX IF NOT EXISTS idx_price_snapshot_snapshot_ts
    ON price_snapshot (snapshot_ts);

-- user_profile
CREATE INDEX IF NOT EXISTS idx_user_profile_label
    ON user_profile (label);
CREATE INDEX IF NOT EXISTS idx_user_profile_total_volume_usd
    ON user_profile (total_volume_usd DESC);

-- trace_log
CREATE INDEX IF NOT EXISTS idx_trace_log_tx_hash
    ON trace_log (tx_hash);
CREATE INDEX IF NOT EXISTS idx_trace_log_call_depth
    ON trace_log (call_depth);
CREATE INDEX IF NOT EXISTS idx_trace_log_call_type
    ON trace_log (call_type);

-- ================================================================
-- 5. COMMENTS
-- ================================================================

-- block
COMMENT ON TABLE block IS '이더리움 블록 헤더 정보';
COMMENT ON COLUMN block.block_number IS '블록 높이 (체인 상 고유 식별자)';
COMMENT ON COLUMN block.timestamp IS '블록 생성 시각 (UTC)';
COMMENT ON COLUMN block.gas_used IS '블록 내 모든 트랜잭션이 소비한 총 가스';

-- token
COMMENT ON TABLE token IS 'ERC-20 토큰 메타데이터 레지스트리';
COMMENT ON COLUMN token.token_address IS '토큰 컨트랙트 주소 (0x 접두사 42자)';
COMMENT ON COLUMN token.decimals IS '토큰 소수점 자릿수 (USDC=6, WETH=18 등)';

-- transaction
COMMENT ON TABLE transaction IS '이더리움 트랜잭션 (성공+실패 모두 포함)';
COMMENT ON COLUMN transaction.tx_hash IS '트랜잭션 해시 (0x 접두사 66자)';
COMMENT ON COLUMN transaction.from_addr IS '트랜잭션 발신자 EOA 주소';
COMMENT ON COLUMN transaction.to_addr IS '수신 주소. NULL이면 컨트랙트 생성 트랜잭션';
COMMENT ON COLUMN transaction.gas_price IS '가스 가격 (wei 단위, 1 ETH = 10^18 wei)';
COMMENT ON COLUMN transaction.value IS '전송된 ETH 양 (wei 단위)';
COMMENT ON COLUMN transaction.status IS '실행 결과: 1=성공, 0=실패 (revert)';
COMMENT ON COLUMN transaction.input_data IS '트랜���션 콜데이터 (ABI 인코딩된 함수 호출)';

-- pool
COMMENT ON TABLE pool IS 'Uniswap V3 유동성 풀';
COMMENT ON COLUMN pool.pair_name IS '토큰 쌍 표시명 (e.g. WETH/USDC)';
COMMENT ON COLUMN pool.fee_tier IS '수수료 티어 (bps): 100=0.01%, 500=0.05%, 3000=0.3%, 10000=1%';
COMMENT ON COLUMN pool.token0_address IS '풀의 token0 — Uniswap��� 주소 크기순으로 정렬';
COMMENT ON COLUMN pool.token1_address IS '풀의 token1 — 항상 token0 < token1 (주소 기준)';

-- swap_event
COMMENT ON TABLE swap_event IS 'Uniswap V3 Swap 이벤트 로그';
COMMENT ON COLUMN swap_event.amount0 IS 'token0 변동량 (부호 있음: 양수=풀로 유입, 음수=풀에서 유출)';
COMMENT ON COLUMN swap_event.amount1 IS 'token1 변동량 (부호 있음)';
COMMENT ON COLUMN swap_event.amount_in IS '스왑에 투입된 토큰 절대량';
COMMENT ON COLUMN swap_event.amount_out IS '스왑으로 받은 토큰 절대량';
COMMENT ON COLUMN swap_event.sqrt_price_x96 IS 'Uniswap V3 가격: price = (sqrtPriceX96 / 2^96)^2';
COMMENT ON COLUMN swap_event.liquidity IS '스왑 시점의 활�� 유동성';
COMMENT ON COLUMN swap_event.tick IS '스왑 후 현재 틱 (가격 구간 인덱스)';
COMMENT ON COLUMN swap_event.log_index IS '트랜잭션 내 이벤트 순서 번호';

-- liquidity_event
COMMENT ON TABLE liquidity_event IS 'Uniswap V3 유동성 공급(Mint)/회수(Burn) 이벤트';
COMMENT ON COLUMN liquidity_event.event_type IS 'MINT=유동성 공급, BURN=유동성 회수';
COMMENT ON COLUMN liquidity_event.provider IS '유동성 공급/회수자 주소';
COMMENT ON COLUMN liquidity_event.tick_lower IS '포지션 하한 틱 (가격 하한)';
COMMENT ON COLUMN liquidity_event.tick_upper IS '포지션 상한 틱 (가격 상한)';
COMMENT ON COLUMN liquidity_event.liquidity IS '공급/회수된 유동성 단위';

-- token_transfer
COMMENT ON TABLE token_transfer IS 'ERC-20 Transfer 이벤트 로그';
COMMENT ON COLUMN token_transfer.amount IS '전송된 토큰 수량 (raw, 토큰 decimals 미적용)';

-- failed_transaction
COMMENT ON TABLE failed_transaction IS '실패(revert)한 트랜잭션 상세 분석';
COMMENT ON COLUMN failed_transaction.error_category IS '에러 분류: INSUFFICIENT_BALANCE, SLIPPAGE_EXCEEDED, DEADLINE_EXPIRED, UNAUTHORIZED, TRANSFER_FAILED, UNKNOWN';
COMMENT ON COLUMN failed_transaction.revert_reason IS '디코딩된 revert 사유 문자열 (e.g. "Too little received")';
COMMENT ON COLUMN failed_transaction.failing_function IS '실패한 함수 시그니처 (e.g. "exactInputSingle")';

-- price_snapshot
COMMENT ON TABLE price_snapshot IS '풀별 주기적 가격 스냅샷 (OHLCV 기반)';
COMMENT ON COLUMN price_snapshot.price IS 'token1/token0 가격 (소수점 18자리 정밀도)';
COMMENT ON COLUMN price_snapshot.tick IS '스냅샷 시점 틱';
COMMENT ON COLUMN price_snapshot.liquidity IS '스냅샷 시점 활성 유동성';
COMMENT ON COLUMN price_snapshot.interval_type IS '스냅샷 주기: 1m, 5m, 15m, 1h, 4h, 1d';

-- user_profile
COMMENT ON TABLE user_profile IS '유저별 활동 집계 프로필';
COMMENT ON COLUMN user_profile.label IS '유저 분류: whale(고래), bot(봇), retail(개인) 등';
COMMENT ON COLUMN user_profile.total_volume_usd IS '누적 거래량 (USD 환산)';

-- trace_log
COMMENT ON TABLE trace_log IS 'debug_traceTransaction으로 추출한 내부 호출 트리';
COMMENT ON COLUMN trace_log.call_depth IS '호출 깊이 (0=최상위 호��)';
COMMENT ON COLUMN trace_log.call_type IS '호출 유형: CALL, DELEGATECALL, STATICCALL, CREATE, CREATE2';
COMMENT ON COLUMN trace_log.value IS '내부 호출 시 전송된 ETH (wei 단위)';
COMMENT ON COLUMN trace_log.input IS 'ABI 인코딩된 호출 입력 데이터';
COMMENT ON COLUMN trace_log.output IS '호출 반환 데이터';
COMMENT ON COLUMN trace_log.error IS '호출 실패 시 에러 메시지';

-- audit_log
COMMENT ON TABLE audit_log IS '대규모 이벤트 감사 로그 (트리거 자동 기록)';

-- ================================================================
-- 6. SEED DATA (DML) — 트리거 생성 전에 삽입
-- ================================================================

-- block
INSERT INTO block (block_number, timestamp, gas_used)
VALUES
    (18000000, '2023-09-01 12:00:00+00', 12345678),
    (18000001, '2023-09-01 12:00:12+00', 15234567),
    (18000002, '2023-09-01 12:00:24+00', 11987654)
ON CONFLICT DO NOTHING;

-- token
INSERT INTO token (token_address, symbol, name, decimals)
VALUES
    ('0xC02aaA39b223FE8D0A0e5C4F27eAD9083C756Cc2', 'WETH',  'Wrapped Ether',   18),
    ('0xA0b86991c6218b36c1d19D4a2e9Eb0cE3606eB48', 'USDC',  'USD Coin',         6),
    ('0xdAC17F958D2ee523a2206206994597C13D831ec7', 'USDT',  'Tether USD',       6),
    ('0x6B175474E89094C44Da98b954EedeAC495271d0F', 'DAI',   'Dai Stablecoin',  18),
    ('0x2260FAC5E5542a773Aa44fBCfeDf7C193bc2C599', 'WBTC',  'Wrapped BTC',      8)
ON CONFLICT DO NOTHING;

-- pool
INSERT INTO pool (pool_address, pair_name, token0_address, token1_address, fee_tier, created_at)
VALUES
    ('0x8ad599c3A0ff1De082011EFDDc58f1908eb6e6D8', 'USDC/WETH',
     '0xA0b86991c6218b36c1d19D4a2e9Eb0cE3606eB48', '0xC02aaA39b223FE8D0A0e5C4F27eAD9083C756Cc2',
     3000, '2021-05-05 00:00:00+00'),
    ('0x4e68Ccd3E89f51C3074ca5072bbAC773960dFa36', 'USDT/WETH',
     '0xC02aaA39b223FE8D0A0e5C4F27eAD9083C756Cc2', '0xdAC17F958D2ee523a2206206994597C13D831ec7',
     3000, '2021-05-05 00:00:00+00'),
    ('0xCBCdF9626bC03E24f779434178A73a0B4bad62eD', 'WBTC/WETH',
     '0x2260FAC5E5542a773Aa44fBCfeDf7C193bc2C599', '0xC02aaA39b223FE8D0A0e5C4F27eAD9083C756Cc2',
     3000, '2021-05-05 00:00:00+00')
ON CONFLICT DO NOTHING;

-- transaction (5 성공 + 3 실패)
INSERT INTO transaction (tx_hash, from_addr, to_addr, block_number, gas_used, gas_price, value, status, input_data)
VALUES
    ('0xabc1230000000000000000000000000000000000000000000000000000000001',
     '0x1111111111111111111111111111111111111111', '0xE592427A0AEce92De3Edee1F18E0157C05861564',
     18000000, 152000, 30000000000, 0, 1, '0x414bf389'),
    ('0xabc1230000000000000000000000000000000000000000000000000000000002',
     '0x2222222222222222222222222222222222222222', '0xE592427A0AEce92De3Edee1F18E0157C05861564',
     18000000, 185000, 35000000000, 0, 1, '0x414bf389'),
    ('0xabc1230000000000000000000000000000000000000000000000000000000003',
     '0x3333333333333333333333333333333333333333', '0xE592427A0AEce92De3Edee1F18E0157C05861564',
     18000001, 210000, 28000000000, 0, 1, '0xc04b8d59'),
    ('0xabc1230000000000000000000000000000000000000000000000000000000004',
     '0x1111111111111111111111111111111111111111', '0xC36442b4a4522E871399CD717aBDD847Ab11FE88',
     18000001, 320000, 32000000000, 0, 1, '0x88316456'),
    ('0xabc1230000000000000000000000000000000000000000000000000000000005',
     '0x4444444444444444444444444444444444444444', '0xE592427A0AEce92De3Edee1F18E0157C05861564',
     18000002, 175000, 25000000000, 1000000000000000000, 1, '0x414bf389'),
    ('0xdead000000000000000000000000000000000000000000000000000000000001',
     '0x5555555555555555555555555555555555555555', '0xE592427A0AEce92De3Edee1F18E0157C05861564',
     18000000, 45000, 30000000000, 0, 0, '0x414bf389'),
    ('0xdead000000000000000000000000000000000000000000000000000000000002',
     '0x6666666666666666666666666666666666666666', '0xE592427A0AEce92De3Edee1F18E0157C05861564',
     18000001, 38000, 35000000000, 0, 0, '0x414bf389'),
    ('0xdead000000000000000000000000000000000000000000000000000000000003',
     '0x7777777777777777777777777777777777777777', '0xE592427A0AEce92De3Edee1F18E0157C05861564',
     18000002, 52000, 28000000000, 0, 0, '0xc04b8d59')
ON CONFLICT DO NOTHING;

-- swap_event
INSERT INTO swap_event (pool_address, tx_hash, sender, recipient, amount0, amount1, amount_in, amount_out, sqrt_price_x96, liquidity, tick, log_index, timestamp)
VALUES
    ('0x8ad599c3A0ff1De082011EFDDc58f1908eb6e6D8',
     '0xabc1230000000000000000000000000000000000000000000000000000000001',
     '0x1111111111111111111111111111111111111111', '0x1111111111111111111111111111111111111111',
     -1640000000, 1000000000000000000, 1000000000000000000, 1640000000,
     1991798296589384459143174368, 16702985042588483200, 201234, 0, '2023-09-01 12:00:05+00'),
    ('0x8ad599c3A0ff1De082011EFDDc58f1908eb6e6D8',
     '0xabc1230000000000000000000000000000000000000000000000000000000002',
     '0x2222222222222222222222222222222222222222', '0x2222222222222222222222222222222222222222',
     5000000000, -3040000000000000000, 5000000000, 3040000000000000000,
     1989998296589384459143174368, 16702985042588483200, 201230, 0, '2023-09-01 12:00:08+00'),
    ('0x4e68Ccd3E89f51C3074ca5072bbAC773960dFa36',
     '0xabc1230000000000000000000000000000000000000000000000000000000003',
     '0x3333333333333333333333333333333333333333', '0x3333333333333333333333333333333333333333',
     2000000000000000000, -3280000000, 2000000000000000000, 3280000000,
     1990500000000000000000000000, 14500000000000000000, 201220, 0, '2023-09-01 12:00:15+00'),
    ('0xCBCdF9626bC03E24f779434178A73a0B4bad62eD',
     '0xabc1230000000000000000000000000000000000000000000000000000000003',
     '0x3333333333333333333333333333333333333333', '0x3333333333333333333333333333333333333333',
     50000000, -8200000000000000000, 50000000, 8200000000000000000,
     31280000000000000000000000000, 520000000000000000, 258100, 1, '2023-09-01 12:00:15+00'),
    ('0x8ad599c3A0ff1De082011EFDDc58f1908eb6e6D8',
     '0xabc1230000000000000000000000000000000000000000000000000000000005',
     '0x4444444444444444444444444444444444444444', '0x4444444444444444444444444444444444444444',
     -16380000000, 10000000000000000000, 10000000000000000000, 16380000000,
     1988200000000000000000000000, 16702985042588483200, 201225, 0, '2023-09-01 12:00:28+00'),
    ('0x4e68Ccd3E89f51C3074ca5072bbAC773960dFa36',
     '0xabc1230000000000000000000000000000000000000000000000000000000005',
     '0x4444444444444444444444444444444444444444', '0x4444444444444444444444444444444444444444',
     500000000000000000, -818000000, 500000000000000000, 818000000,
     1990200000000000000000000000, 14500000000000000000, 201218, 1, '2023-09-01 12:00:28+00')
ON CONFLICT DO NOTHING;

-- liquidity_event
INSERT INTO liquidity_event (event_type, pool_address, tx_hash, provider, token0_amount, token1_amount, tick_lower, tick_upper, liquidity, log_index, timestamp)
VALUES
    ('MINT', '0x8ad599c3A0ff1De082011EFDDc58f1908eb6e6D8',
     '0xabc1230000000000000000000000000000000000000000000000000000000004',
     '0x1111111111111111111111111111111111111111',
     5000000000, 3000000000000000000, 200000, 202000, 8500000000000000000, 0, '2023-09-01 12:00:18+00'),
    ('MINT', '0xCBCdF9626bC03E24f779434178A73a0B4bad62eD',
     '0xabc1230000000000000000000000000000000000000000000000000000000004',
     '0x1111111111111111111111111111111111111111',
     100000000, 16400000000000000000, 257000, 259000, 420000000000000000, 1, '2023-09-01 12:00:18+00'),
    ('BURN', '0x8ad599c3A0ff1De082011EFDDc58f1908eb6e6D8',
     '0xabc1230000000000000000000000000000000000000000000000000000000005',
     '0x4444444444444444444444444444444444444444',
     2000000000, 1200000000000000000, 199000, 203000, 3200000000000000000, 2, '2023-09-01 12:00:28+00'),
    ('BURN', '0x4e68Ccd3E89f51C3074ca5072bbAC773960dFa36',
     '0xabc1230000000000000000000000000000000000000000000000000000000005',
     '0x4444444444444444444444444444444444444444',
     800000000000000000, 1300000000, 200000, 202000, 2100000000000000000, 3, '2023-09-01 12:00:28+00')
ON CONFLICT DO NOTHING;

-- token_transfer
INSERT INTO token_transfer (tx_hash, token_address, from_addr, to_addr, amount, log_index, timestamp)
VALUES
    ('0xabc1230000000000000000000000000000000000000000000000000000000001',
     '0xC02aaA39b223FE8D0A0e5C4F27eAD9083C756Cc2',
     '0x1111111111111111111111111111111111111111', '0x8ad599c3A0ff1De082011EFDDc58f1908eb6e6D8',
     1000000000000000000, 1, '2023-09-01 12:00:05+00'),
    ('0xabc1230000000000000000000000000000000000000000000000000000000001',
     '0xA0b86991c6218b36c1d19D4a2e9Eb0cE3606eB48',
     '0x8ad599c3A0ff1De082011EFDDc58f1908eb6e6D8', '0x1111111111111111111111111111111111111111',
     1640000000, 2, '2023-09-01 12:00:05+00'),
    ('0xabc1230000000000000000000000000000000000000000000000000000000002',
     '0xA0b86991c6218b36c1d19D4a2e9Eb0cE3606eB48',
     '0x2222222222222222222222222222222222222222', '0x8ad599c3A0ff1De082011EFDDc58f1908eb6e6D8',
     5000000000, 1, '2023-09-01 12:00:08+00'),
    ('0xabc1230000000000000000000000000000000000000000000000000000000003',
     '0x2260FAC5E5542a773Aa44fBCfeDf7C193bc2C599',
     '0x3333333333333333333333333333333333333333', '0xCBCdF9626bC03E24f779434178A73a0B4bad62eD',
     50000000, 2, '2023-09-01 12:00:15+00'),
    ('0xabc1230000000000000000000000000000000000000000000000000000000005',
     '0xdAC17F958D2ee523a2206206994597C13D831ec7',
     '0x4e68Ccd3E89f51C3074ca5072bbAC773960dFa36', '0x4444444444444444444444444444444444444444',
     818000000, 4, '2023-09-01 12:00:28+00')
ON CONFLICT DO NOTHING;

-- failed_transaction
INSERT INTO failed_transaction (tx_hash, error_category, revert_reason, failing_function, gas_used, timestamp)
VALUES
    ('0xdead000000000000000000000000000000000000000000000000000000000001',
     'SLIPPAGE_EXCEEDED', 'Too little received', 'exactInputSingle', 45000, '2023-09-01 12:00:03+00'),
    ('0xdead000000000000000000000000000000000000000000000000000000000002',
     'DEADLINE_EXPIRED', 'Transaction too old', 'exactInputSingle', 38000, '2023-09-01 12:00:14+00'),
    ('0xdead000000000000000000000000000000000000000000000000000000000003',
     'INSUFFICIENT_BALANCE', 'STF', 'exactInput', 52000, '2023-09-01 12:00:26+00')
ON CONFLICT DO NOTHING;

-- price_snapshot
INSERT INTO price_snapshot (pool_address, price, tick, liquidity, snapshot_ts, interval_type)
VALUES
    ('0x8ad599c3A0ff1De082011EFDDc58f1908eb6e6D8', 1640.250000000000000000, 201234, 16702985042588483200, '2023-09-01 12:00:00+00', '1m'),
    ('0x8ad599c3A0ff1De082011EFDDc58f1908eb6e6D8', 1638.800000000000000000, 201225, 16702985042588483200, '2023-09-01 12:01:00+00', '1m'),
    ('0x8ad599c3A0ff1De082011EFDDc58f1908eb6e6D8', 1639.500000000000000000, 201230, 16702985042588483200, '2023-09-01 12:00:00+00', '1h'),
    ('0x4e68Ccd3E89f51C3074ca5072bbAC773960dFa36', 1640.000000000000000000, 201220, 14500000000000000000, '2023-09-01 12:00:00+00', '1m'),
    ('0xCBCdF9626bC03E24f779434178A73a0B4bad62eD', 16.400000000000000000, 258100, 520000000000000000, '2023-09-01 12:00:00+00', '1m'),
    ('0xCBCdF9626bC03E24f779434178A73a0B4bad62eD', 16.380000000000000000, 258095, 520000000000000000, '2023-09-01 12:00:00+00', '1h')
ON CONFLICT DO NOTHING;

-- user_profile
INSERT INTO user_profile (user_address, label, first_seen, last_seen, total_swaps, total_volume_usd)
VALUES
    ('0x1111111111111111111111111111111111111111', 'whale',  '2023-08-15 10:00:00+00', '2023-09-01 12:00:18+00', 245, 1850000.00),
    ('0x2222222222222222222222222222222222222222', 'retail', '2023-09-01 11:50:00+00', '2023-09-01 12:00:08+00', 3, 8200.00),
    ('0x3333333333333333333333333333333333333333', 'bot',    '2023-07-01 00:00:00+00', '2023-09-01 12:00:15+00', 12450, 5430000.00),
    ('0x4444444444444444444444444444444444444444', NULL,     '2023-09-01 12:00:28+00', '2023-09-01 12:00:28+00', 2, 28120.00)
ON CONFLICT DO NOTHING;

-- trace_log
INSERT INTO trace_log (tx_hash, call_depth, call_type, from_addr, to_addr, value, gas_used, input, output, error)
VALUES
    ('0xabc1230000000000000000000000000000000000000000000000000000000001',
     0, 'CALL', '0x1111111111111111111111111111111111111111', '0xE592427A0AEce92De3Edee1F18E0157C05861564',
     0, 85000, '0x414bf389', '0x0000000000000000000000000000000000000000000000000000000061a80000', NULL),
    ('0xabc1230000000000000000000000000000000000000000000000000000000001',
     1, 'CALL', '0xE592427A0AEce92De3Edee1F18E0157C05861564', '0x8ad599c3A0ff1De082011EFDDc58f1908eb6e6D8',
     0, 62000, '0x128acb08', '0x', NULL),
    ('0xabc1230000000000000000000000000000000000000000000000000000000001',
     2, 'STATICCALL', '0x8ad599c3A0ff1De082011EFDDc58f1908eb6e6D8', '0xA0b86991c6218b36c1d19D4a2e9Eb0cE3606eB48',
     0, 2600, '0x70a08231', '0x0000000000000000000000000000000000000000000000000000000061a80000', NULL),
    ('0xdead000000000000000000000000000000000000000000000000000000000001',
     0, 'CALL', '0x5555555555555555555555555555555555555555', '0xE592427A0AEce92De3Edee1F18E0157C05861564',
     0, 35000, '0x414bf389', NULL, 'Too little received'),
    ('0xabc1230000000000000000000000000000000000000000000000000000000003',
     1, 'DELEGATECALL', '0xE592427A0AEce92De3Edee1F18E0157C05861564', '0x68b3465833fb72A70ecDF485E0e4C7bD8665Fc45',
     0, 48000, '0xc04b8d59', '0x', NULL)
ON CONFLICT DO NOTHING;

-- ================================================================
-- 7. TRIGGERS
-- ================================================================

-- 1. trg_swap_update_user_profile
CREATE OR REPLACE FUNCTION fn_trg_swap_update_user_profile()
RETURNS TRIGGER LANGUAGE plpgsql AS $$
BEGIN
    INSERT INTO user_profile (user_address, first_seen, last_seen, total_swaps, total_volume_usd)
    VALUES (NEW.sender, NEW.timestamp, NEW.timestamp, 1, 0)
    ON CONFLICT (user_address) DO UPDATE SET
        last_seen   = GREATEST(user_profile.last_seen, EXCLUDED.last_seen),
        total_swaps = user_profile.total_swaps + 1;
    IF NEW.recipient <> NEW.sender THEN
        INSERT INTO user_profile (user_address, first_seen, last_seen, total_swaps, total_volume_usd)
        VALUES (NEW.recipient, NEW.timestamp, NEW.timestamp, 0, 0)
        ON CONFLICT (user_address) DO UPDATE SET
            last_seen = GREATEST(user_profile.last_seen, EXCLUDED.last_seen);
    END IF;
    RETURN NEW;
END; $$;

DROP TRIGGER IF EXISTS trg_swap_update_user_profile ON swap_event;
CREATE TRIGGER trg_swap_update_user_profile
    AFTER INSERT ON swap_event FOR EACH ROW
    EXECUTE FUNCTION fn_trg_swap_update_user_profile();

-- 2. trg_transaction_check_failed
CREATE OR REPLACE FUNCTION fn_trg_transaction_check_failed()
RETURNS TRIGGER LANGUAGE plpgsql AS $$
BEGIN
    IF NEW.status = 0 THEN
        INSERT INTO failed_transaction (tx_hash, error_category, gas_used, timestamp)
        SELECT NEW.tx_hash, 'UNKNOWN', NEW.gas_used, b.timestamp
        FROM block b WHERE b.block_number = NEW.block_number
        ON CONFLICT (tx_hash) DO NOTHING;
    END IF;
    RETURN NEW;
END; $$;

DROP TRIGGER IF EXISTS trg_transaction_check_failed ON transaction;
CREATE TRIGGER trg_transaction_check_failed
    AFTER INSERT ON transaction FOR EACH ROW
    EXECUTE FUNCTION fn_trg_transaction_check_failed();

-- 3. trg_price_snapshot_notify
CREATE OR REPLACE FUNCTION fn_trg_price_snapshot_notify()
RETURNS TRIGGER LANGUAGE plpgsql AS $$
BEGIN
    PERFORM pg_notify('price_update',
        json_build_object(
            'pool_address', NEW.pool_address, 'price', NEW.price,
            'tick', NEW.tick, 'interval', NEW.interval_type, 'timestamp', NEW.snapshot_ts
        )::TEXT);
    RETURN NEW;
END; $$;

DROP TRIGGER IF EXISTS trg_price_snapshot_notify ON price_snapshot;
CREATE TRIGGER trg_price_snapshot_notify
    AFTER INSERT ON price_snapshot FOR EACH ROW
    EXECUTE FUNCTION fn_trg_price_snapshot_notify();

-- 4. trg_block_timestamp_validate
CREATE OR REPLACE FUNCTION fn_trg_block_timestamp_validate()
RETURNS TRIGGER LANGUAGE plpgsql AS $$
BEGIN
    IF NEW.timestamp > NOW() + INTERVAL '15 seconds' THEN
        RAISE EXCEPTION 'Block timestamp is in the future: % (current: %)', NEW.timestamp, NOW();
    END IF;
    RETURN NEW;
END; $$;

DROP TRIGGER IF EXISTS trg_block_timestamp_validate ON block;
CREATE TRIGGER trg_block_timestamp_validate
    BEFORE INSERT ON block FOR EACH ROW
    EXECUTE FUNCTION fn_trg_block_timestamp_validate();

-- 5. trg_liquidity_event_audit
CREATE OR REPLACE FUNCTION fn_trg_liquidity_event_audit()
RETURNS TRIGGER LANGUAGE plpgsql AS $$
BEGIN
    IF NEW.liquidity > 1000000000000000000 THEN
        INSERT INTO audit_log (table_name, event_type, record_id, details)
        VALUES ('liquidity_event', NEW.event_type::TEXT, NEW.event_id::TEXT,
            json_build_object(
                'pool_address', NEW.pool_address, 'provider', NEW.provider,
                'liquidity', NEW.liquidity, 'token0_amount', NEW.token0_amount,
                'token1_amount', NEW.token1_amount, 'timestamp', NEW.timestamp
            )::JSONB);
    END IF;
    RETURN NEW;
END; $$;

DROP TRIGGER IF EXISTS trg_liquidity_event_audit ON liquidity_event;
CREATE TRIGGER trg_liquidity_event_audit
    AFTER INSERT ON liquidity_event FOR EACH ROW
    EXECUTE FUNCTION fn_trg_liquidity_event_audit();

-- ================================================================
-- 8. VIEWS (7개)
-- ================================================================

-- 1. vw_daily_swap_volume
CREATE OR REPLACE VIEW vw_daily_swap_volume AS
SELECT s.pool_address, p.pair_name, DATE(s.timestamp) AS swap_date,
       COUNT(*) AS swap_count, SUM(s.amount_in) AS total_amount_in, SUM(s.amount_out) AS total_amount_out
FROM swap_event s JOIN pool p ON s.pool_address = p.pool_address
GROUP BY s.pool_address, p.pair_name, DATE(s.timestamp)
ORDER BY swap_date DESC, swap_count DESC;

-- 2. vw_top_traders
CREATE OR REPLACE VIEW vw_top_traders AS
SELECT user_address, label, total_swaps, total_volume_usd,
       DENSE_RANK() OVER (ORDER BY total_volume_usd DESC) AS volume_rank
FROM user_profile WHERE total_swaps > 0 ORDER BY volume_rank;

-- 3. vw_pool_liquidity_summary
CREATE OR REPLACE VIEW vw_pool_liquidity_summary AS
WITH latest_price AS (
    SELECT DISTINCT ON (pool_address) pool_address, price AS latest_price, tick AS latest_tick,
           liquidity AS current_liquidity, snapshot_ts AS last_snapshot
    FROM price_snapshot ORDER BY pool_address, snapshot_ts DESC
),
liquidity_stats AS (
    SELECT pool_address,
           COUNT(*) FILTER (WHERE event_type = 'MINT') AS total_mints,
           COUNT(*) FILTER (WHERE event_type = 'BURN') AS total_burns,
           SUM(CASE WHEN event_type = 'MINT' THEN liquidity ELSE -liquidity END) AS net_liquidity
    FROM liquidity_event GROUP BY pool_address
)
SELECT p.pool_address, p.pair_name, p.fee_tier, lp.latest_price, lp.latest_tick,
       lp.current_liquidity, lp.last_snapshot,
       COALESCE(ls.total_mints, 0) AS total_mints, COALESCE(ls.total_burns, 0) AS total_burns,
       COALESCE(ls.net_liquidity, 0) AS net_liquidity
FROM pool p LEFT JOIN latest_price lp ON p.pool_address = lp.pool_address
LEFT JOIN liquidity_stats ls ON p.pool_address = ls.pool_address;

-- 4. vw_failed_tx_analysis
CREATE OR REPLACE VIEW vw_failed_tx_analysis AS
WITH total AS (SELECT COUNT(*) AS total_failures FROM failed_transaction)
SELECT f.error_category, COUNT(*) AS failure_count, ROUND(AVG(f.gas_used)) AS avg_gas_wasted,
       ROUND(100.0 * COUNT(*) / GREATEST(t.total_failures, 1), 2) AS pct_of_total,
       MAX(f.timestamp) AS most_recent_failure
FROM failed_transaction f CROSS JOIN total t
GROUP BY f.error_category, t.total_failures ORDER BY failure_count DESC;

-- 5. vw_hourly_gas_stats
CREATE OR REPLACE VIEW vw_hourly_gas_stats AS
SELECT date_trunc('hour', b.timestamp) AS hour_bucket, COUNT(t.tx_hash) AS tx_count,
       ROUND(AVG(t.gas_used)) AS avg_gas_used, MAX(t.gas_used) AS max_gas_used,
       MIN(t.gas_used) AS min_gas_used, SUM(t.gas_used) AS total_gas_used
FROM transaction t JOIN block b ON t.block_number = b.block_number
GROUP BY date_trunc('hour', b.timestamp) ORDER BY hour_bucket DESC;

-- 6. vw_token_activity
CREATE OR REPLACE VIEW vw_token_activity AS
WITH transfer_stats AS (
    SELECT token_address, COUNT(*) AS transfer_count, SUM(amount) AS total_transferred
    FROM token_transfer GROUP BY token_address
),
pool_stats AS (
    SELECT token_address, COUNT(*) AS pool_count FROM (
        SELECT token0_address AS token_address FROM pool
        UNION ALL SELECT token1_address FROM pool
    ) sub GROUP BY token_address
)
SELECT tk.token_address, tk.symbol, tk.name, tk.decimals,
       COALESCE(ts.transfer_count, 0) AS transfer_count,
       COALESCE(ts.total_transferred, 0) AS total_transferred,
       COALESCE(ps.pool_count, 0) AS pool_count
FROM token tk LEFT JOIN transfer_stats ts ON tk.token_address = ts.token_address
LEFT JOIN pool_stats ps ON tk.token_address = ps.token_address
ORDER BY transfer_count DESC;

-- 7. vw_pool_fee_revenue
CREATE OR REPLACE VIEW vw_pool_fee_revenue AS
SELECT p.pool_address, p.pair_name, p.fee_tier, COUNT(s.event_id) AS total_swaps,
       SUM(s.amount_in) AS total_volume, SUM(s.amount_in) * p.fee_tier / 1000000 AS estimated_fee_revenue
FROM pool p LEFT JOIN swap_event s ON p.pool_address = s.pool_address
GROUP BY p.pool_address, p.pair_name, p.fee_tier ORDER BY estimated_fee_revenue DESC;

-- ================================================================
-- 9. PROCEDURES & FUNCTIONS
-- ================================================================

-- sp_register_pool
CREATE OR REPLACE PROCEDURE sp_register_pool(
    p_pool_address VARCHAR(42), p_pair_name VARCHAR(50),
    p_token0_addr VARCHAR(42), p_token0_symbol VARCHAR(20), p_token0_name VARCHAR(100), p_token0_dec SMALLINT,
    p_token1_addr VARCHAR(42), p_token1_symbol VARCHAR(20), p_token1_name VARCHAR(100), p_token1_dec SMALLINT,
    p_fee_tier INTEGER
) LANGUAGE plpgsql AS $$
BEGIN
    INSERT INTO token (token_address, symbol, name, decimals) VALUES (p_token0_addr, p_token0_symbol, p_token0_name, p_token0_dec) ON CONFLICT (token_address) DO NOTHING;
    INSERT INTO token (token_address, symbol, name, decimals) VALUES (p_token1_addr, p_token1_symbol, p_token1_name, p_token1_dec) ON CONFLICT (token_address) DO NOTHING;
    INSERT INTO pool (pool_address, pair_name, token0_address, token1_address, fee_tier) VALUES (p_pool_address, p_pair_name, p_token0_addr, p_token1_addr, p_fee_tier) ON CONFLICT (pool_address) DO NOTHING;
END; $$;

-- sp_update_user_profile_after_swap
CREATE OR REPLACE PROCEDURE sp_update_user_profile_after_swap(
    p_user_address VARCHAR(42), p_swap_volume_usd NUMERIC(20,2), p_swap_timestamp TIMESTAMPTZ
) LANGUAGE plpgsql AS $$
DECLARE v_total_swaps INTEGER; v_total_volume NUMERIC(20,2); v_label VARCHAR(50);
BEGIN
    INSERT INTO user_profile (user_address, first_seen, last_seen, total_swaps, total_volume_usd)
    VALUES (p_user_address, p_swap_timestamp, p_swap_timestamp, 1, p_swap_volume_usd)
    ON CONFLICT (user_address) DO UPDATE SET
        last_seen = GREATEST(user_profile.last_seen, EXCLUDED.last_seen),
        total_swaps = user_profile.total_swaps + 1,
        total_volume_usd = user_profile.total_volume_usd + EXCLUDED.total_volume_usd;
    SELECT total_swaps, total_volume_usd INTO v_total_swaps, v_total_volume FROM user_profile WHERE user_address = p_user_address;
    IF v_total_volume > 1000000 THEN v_label := 'whale';
    ELSIF v_total_swaps > 100 THEN v_label := 'bot';
    ELSE v_label := 'retail'; END IF;
    UPDATE user_profile SET label = v_label WHERE user_address = p_user_address;
END; $$;

-- sp_record_failed_transaction
CREATE OR REPLACE PROCEDURE sp_record_failed_transaction(
    p_tx_hash VARCHAR(66), p_error_category error_category, p_revert_reason TEXT,
    p_failing_function VARCHAR(100), p_gas_used BIGINT, p_timestamp TIMESTAMPTZ
) LANGUAGE plpgsql AS $$
BEGIN
    INSERT INTO failed_transaction (tx_hash, error_category, revert_reason, failing_function, gas_used, timestamp)
    VALUES (p_tx_hash, p_error_category, p_revert_reason, p_failing_function, p_gas_used, p_timestamp)
    ON CONFLICT (tx_hash) DO UPDATE SET
        error_category = EXCLUDED.error_category, revert_reason = EXCLUDED.revert_reason, failing_function = EXCLUDED.failing_function;
END; $$;

-- sp_take_price_snapshot
CREATE OR REPLACE PROCEDURE sp_take_price_snapshot(
    p_pool_address VARCHAR(42), p_price NUMERIC(30,18), p_tick INTEGER,
    p_liquidity NUMERIC(38,0), p_interval_type snapshot_interval
) LANGUAGE plpgsql AS $$
DECLARE v_snapshot_ts TIMESTAMPTZ;
BEGIN
    v_snapshot_ts := CASE p_interval_type
        WHEN '1m'  THEN date_trunc('minute', NOW())
        WHEN '5m'  THEN date_trunc('hour', NOW()) + INTERVAL '1 minute' * (EXTRACT(MINUTE FROM NOW())::INT / 5 * 5)
        WHEN '15m' THEN date_trunc('hour', NOW()) + INTERVAL '1 minute' * (EXTRACT(MINUTE FROM NOW())::INT / 15 * 15)
        WHEN '1h'  THEN date_trunc('hour', NOW())
        WHEN '4h'  THEN date_trunc('day', NOW()) + INTERVAL '1 hour' * (EXTRACT(HOUR FROM NOW())::INT / 4 * 4)
        WHEN '1d'  THEN date_trunc('day', NOW())
    END;
    INSERT INTO price_snapshot (pool_address, price, tick, liquidity, snapshot_ts, interval_type)
    VALUES (p_pool_address, p_price, p_tick, p_liquidity, v_snapshot_ts, p_interval_type)
    ON CONFLICT (pool_address, snapshot_ts, interval_type) DO UPDATE SET
        price = EXCLUDED.price, tick = EXCLUDED.tick, liquidity = EXCLUDED.liquidity;
END; $$;

-- fn_get_pool_stats
CREATE OR REPLACE FUNCTION fn_get_pool_stats(
    p_pool_address VARCHAR(42), p_from_date TIMESTAMPTZ, p_to_date TIMESTAMPTZ
) RETURNS TABLE (
    pair_name VARCHAR(50), swap_count BIGINT, unique_traders BIGINT,
    total_volume_in NUMERIC, avg_trade_size NUMERIC, liquidity_events BIGINT, estimated_fees NUMERIC
) LANGUAGE plpgsql AS $$
BEGIN
    RETURN QUERY
    SELECT p.pair_name, COUNT(s.event_id), COUNT(DISTINCT s.sender), SUM(s.amount_in),
           CASE WHEN COUNT(s.event_id) > 0 THEN SUM(s.amount_in) / COUNT(s.event_id) ELSE 0 END,
           (SELECT COUNT(*) FROM liquidity_event le WHERE le.pool_address = p_pool_address AND le.timestamp BETWEEN p_from_date AND p_to_date),
           SUM(s.amount_in) * p.fee_tier / 1000000
    FROM pool p LEFT JOIN swap_event s ON p.pool_address = s.pool_address AND s.timestamp BETWEEN p_from_date AND p_to_date
    WHERE p.pool_address = p_pool_address GROUP BY p.pair_name, p.fee_tier;
END; $$;

-- ================================================================
-- 10. AUTH (Roles, RLS)
-- ================================================================

DO $$ BEGIN CREATE ROLE defi_readonly NOLOGIN; EXCEPTION WHEN duplicate_object THEN NULL; END $$;
DO $$ BEGIN CREATE ROLE defi_indexer NOLOGIN; EXCEPTION WHEN duplicate_object THEN NULL; END $$;
DO $$ BEGIN CREATE ROLE defi_admin NOLOGIN; EXCEPTION WHEN duplicate_object THEN NULL; END $$;

GRANT USAGE ON SCHEMA public TO defi_readonly;
GRANT USAGE ON SCHEMA public TO defi_indexer;
GRANT USAGE ON SCHEMA public TO defi_admin;

GRANT SELECT ON ALL TABLES IN SCHEMA public TO defi_readonly;
ALTER DEFAULT PRIVILEGES IN SCHEMA public GRANT SELECT ON TABLES TO defi_readonly;

GRANT SELECT, INSERT, UPDATE ON ALL TABLES IN SCHEMA public TO defi_indexer;
GRANT USAGE, SELECT ON ALL SEQUENCES IN SCHEMA public TO defi_indexer;
ALTER DEFAULT PRIVILEGES IN SCHEMA public GRANT SELECT, INSERT, UPDATE ON TABLES TO defi_indexer;
ALTER DEFAULT PRIVILEGES IN SCHEMA public GRANT USAGE, SELECT ON SEQUENCES TO defi_indexer;

GRANT ALL PRIVILEGES ON ALL TABLES IN SCHEMA public TO defi_admin;
GRANT ALL PRIVILEGES ON ALL SEQUENCES IN SCHEMA public TO defi_admin;
ALTER DEFAULT PRIVILEGES IN SCHEMA public GRANT ALL PRIVILEGES ON TABLES TO defi_admin;
ALTER DEFAULT PRIVILEGES IN SCHEMA public GRANT ALL PRIVILEGES ON SEQUENCES TO defi_admin;
GRANT EXECUTE ON ALL FUNCTIONS IN SCHEMA public TO defi_admin;
GRANT EXECUTE ON ALL PROCEDURES IN SCHEMA public TO defi_admin;
GRANT EXECUTE ON ALL FUNCTIONS IN SCHEMA public TO defi_indexer;
GRANT EXECUTE ON ALL PROCEDURES IN SCHEMA public TO defi_indexer;

-- Row-Level Security
ALTER TABLE user_profile ENABLE ROW LEVEL SECURITY;

DROP POLICY IF EXISTS user_profile_readonly_policy ON user_profile;
CREATE POLICY user_profile_readonly_policy ON user_profile FOR SELECT TO defi_readonly USING (total_swaps > 0);

DROP POLICY IF EXISTS user_profile_indexer_policy ON user_profile;
CREATE POLICY user_profile_indexer_policy ON user_profile FOR ALL TO defi_indexer USING (TRUE) WITH CHECK (TRUE);

DROP POLICY IF EXISTS user_profile_admin_policy ON user_profile;
CREATE POLICY user_profile_admin_policy ON user_profile FOR ALL TO defi_admin USING (TRUE) WITH CHECK (TRUE);

-- 샘플 사용자
DO $$ BEGIN CREATE ROLE analyst_alice LOGIN PASSWORD 'analyst_pass' IN ROLE defi_readonly; EXCEPTION WHEN duplicate_object THEN NULL; END $$;
DO $$ BEGIN CREATE ROLE indexer_bot LOGIN PASSWORD 'indexer_pass' IN ROLE defi_indexer; EXCEPTION WHEN duplicate_object THEN NULL; END $$;
DO $$ BEGIN CREATE ROLE admin_owner LOGIN PASSWORD 'admin_pass' IN ROLE defi_admin; EXCEPTION WHEN duplicate_object THEN NULL; END $$;

COMMIT;

-- ================================================================
-- 11. QUERIES (10 SELECT — 트랜잭션 외부에서 실행)
-- ================================================================

-- Q1: 풀별 스왑 횟수 및 총 거래량
SELECT p.pair_name, COUNT(s.event_id) AS swap_count, SUM(s.amount_in) AS total_amount_in
FROM swap_event s JOIN pool p ON s.pool_address = p.pool_address
GROUP BY p.pair_name ORDER BY swap_count DESC;

-- Q2: 유저별 스왑 횟수
SELECT u.user_address, u.label, u.total_swaps, u.total_volume_usd
FROM user_profile u ORDER BY u.total_volume_usd DESC;

-- Q3: 실패한 트랜잭션 목록
SELECT t.tx_hash, t.from_addr, f.error_category, f.revert_reason, f.failing_function
FROM failed_transaction f JOIN transaction t ON f.tx_hash = t.tx_hash;

-- Q4: 블록별 가스 사용량
SELECT b.block_number, b.timestamp, b.gas_used AS block_gas_used, COUNT(t.tx_hash) AS tx_count
FROM block b LEFT JOIN transaction t ON b.block_number = t.block_number
GROUP BY b.block_number, b.timestamp, b.gas_used ORDER BY b.block_number;

-- Q5: 토큰별 전송 횟수
SELECT tk.symbol, tk.name, COUNT(tt.transfer_id) AS transfer_count, SUM(tt.amount) AS total_amount
FROM token_transfer tt JOIN token tk ON tt.token_address = tk.token_address
GROUP BY tk.symbol, tk.name ORDER BY transfer_count DESC;

-- Q6: 풀별 최신 가격 스냅샷 (1m)
SELECT p.pair_name, ps.price, ps.tick, ps.liquidity, ps.snapshot_ts
FROM price_snapshot ps JOIN pool p ON ps.pool_address = p.pool_address
WHERE ps.interval_type = '1m' ORDER BY ps.snapshot_ts DESC;

-- Q7: 평균 이상 가스 트랜잭션
SELECT tx_hash, from_addr, gas_used, status FROM transaction
WHERE gas_used > (SELECT AVG(gas_used) FROM transaction) ORDER BY gas_used DESC;

-- Q8: 유동성 공급/회수 내역
SELECT le.event_type, p.pair_name, le.provider, le.token0_amount, le.token1_amount, le.timestamp
FROM liquidity_event le JOIN pool p ON le.pool_address = p.pool_address ORDER BY le.timestamp;

-- Q9: 콜 깊이별 내부 호출 분포
SELECT call_depth, call_type, COUNT(*) AS call_count
FROM trace_log GROUP BY call_depth, call_type ORDER BY call_depth;

-- Q10: 스왑 이벤트와 블록 정보
SELECT b.block_number, b.timestamp, p.pair_name, s.amount_in, s.amount_out
FROM swap_event s JOIN transaction t ON s.tx_hash = t.tx_hash
JOIN block b ON t.block_number = b.block_number
JOIN pool p ON s.pool_address = p.pool_address ORDER BY b.block_number;

-- ================================================================
-- 12. OLAP (고급 분석 쿼리 8개)
-- ================================================================

-- OLAP 1: 7일 이동평균 스왑 볼륨
WITH daily_volumes AS (
    SELECT s.pool_address, p.pair_name, DATE(s.timestamp) AS swap_date,
           SUM(s.amount_in) AS daily_volume, COUNT(*) AS daily_swaps
    FROM swap_event s JOIN pool p ON s.pool_address = p.pool_address
    GROUP BY s.pool_address, p.pair_name, DATE(s.timestamp)
)
SELECT *, AVG(daily_volume) OVER (PARTITION BY pool_address ORDER BY swap_date ROWS BETWEEN 6 PRECEDING AND CURRENT ROW) AS moving_avg_7d
FROM daily_volumes ORDER BY pool_address, swap_date;

-- OLAP 2: 풀별 누적 볼륨
SELECT s.pool_address, p.pair_name, s.timestamp, s.amount_in,
       SUM(s.amount_in) OVER (PARTITION BY s.pool_address ORDER BY s.timestamp) AS cumulative_volume,
       ROW_NUMBER() OVER (PARTITION BY s.pool_address ORDER BY s.timestamp) AS trade_sequence
FROM swap_event s JOIN pool p ON s.pool_address = p.pool_address ORDER BY s.pool_address, s.timestamp;

-- OLAP 3: 풀별 트레이더 랭킹
WITH trader_volumes AS (
    SELECT s.pool_address, p.pair_name, s.sender, COUNT(*) AS trade_count, SUM(s.amount_in) AS total_volume
    FROM swap_event s JOIN pool p ON s.pool_address = p.pool_address
    GROUP BY s.pool_address, p.pair_name, s.sender
)
SELECT *, DENSE_RANK() OVER (PARTITION BY pool_address ORDER BY total_volume DESC) AS volume_rank,
       ROUND(100.0 * total_volume / SUM(total_volume) OVER (PARTITION BY pool_address), 2) AS volume_share_pct
FROM trader_volumes ORDER BY pool_address, volume_rank;

-- OLAP 4: 시간대별 트랜잭션 처리량 (LAG/LEAD)
WITH hourly_counts AS (
    SELECT date_trunc('hour', b.timestamp) AS hour_bucket, COUNT(*) AS tx_count, SUM(t.gas_used) AS total_gas
    FROM transaction t JOIN block b ON t.block_number = b.block_number GROUP BY 1
)
SELECT *, LAG(tx_count) OVER (ORDER BY hour_bucket) AS prev_hour_count,
       tx_count - LAG(tx_count) OVER (ORDER BY hour_bucket) AS count_delta,
       LEAD(tx_count) OVER (ORDER BY hour_bucket) AS next_hour_count
FROM hourly_counts ORDER BY hour_bucket;

-- OLAP 5: 블록별 가스 가격 백분위
SELECT t.block_number, COUNT(*) AS tx_count,
       MIN(t.gas_price) AS min_gas_price,
       PERCENTILE_CONT(0.25) WITHIN GROUP (ORDER BY t.gas_price) AS p25_gas_price,
       PERCENTILE_CONT(0.50) WITHIN GROUP (ORDER BY t.gas_price) AS median_gas_price,
       PERCENTILE_CONT(0.75) WITHIN GROUP (ORDER BY t.gas_price) AS p75_gas_price,
       PERCENTILE_CONT(0.95) WITHIN GROUP (ORDER BY t.gas_price) AS p95_gas_price,
       MAX(t.gas_price) AS max_gas_price
FROM transaction t GROUP BY t.block_number ORDER BY t.block_number;

-- OLAP 6: LP 리텐션 분석
WITH first_mint AS (
    SELECT provider, pool_address, MIN(timestamp) AS first_mint_ts
    FROM liquidity_event WHERE event_type = 'MINT' GROUP BY provider, pool_address
),
first_burn AS (
    SELECT provider, pool_address, MIN(timestamp) AS first_burn_ts
    FROM liquidity_event WHERE event_type = 'BURN' GROUP BY provider, pool_address
),
retention AS (
    SELECT fm.provider, fm.pool_address, p.pair_name, fm.first_mint_ts, fb.first_burn_ts,
           fb.first_burn_ts - fm.first_mint_ts AS retention_duration,
           CASE WHEN fb.first_burn_ts IS NULL THEN 'STILL_ACTIVE'
                WHEN fb.first_burn_ts - fm.first_mint_ts < INTERVAL '1 hour' THEN 'SHORT_TERM'
                WHEN fb.first_burn_ts - fm.first_mint_ts < INTERVAL '1 day' THEN 'MEDIUM_TERM'
                ELSE 'LONG_TERM' END AS retention_category
    FROM first_mint fm LEFT JOIN first_burn fb ON fm.provider = fb.provider AND fm.pool_address = fb.pool_address
    JOIN pool p ON fm.pool_address = p.pool_address
)
SELECT retention_category, COUNT(*) AS provider_count,
       AVG(EXTRACT(EPOCH FROM retention_duration) / 3600) AS avg_hours_retained
FROM retention GROUP BY retention_category ORDER BY provider_count DESC;

-- OLAP 7: 풀 시장점유율 추이
WITH daily_pool_volume AS (
    SELECT DATE(s.timestamp) AS swap_date, s.pool_address, p.pair_name, SUM(s.amount_in) AS pool_volume
    FROM swap_event s JOIN pool p ON s.pool_address = p.pool_address
    GROUP BY DATE(s.timestamp), s.pool_address, p.pair_name
)
SELECT *, SUM(pool_volume) OVER (PARTITION BY swap_date) AS total_daily_volume,
       ROUND(100.0 * pool_volume / NULLIF(SUM(pool_volume) OVER (PARTITION BY swap_date), 0), 2) AS market_share_pct,
       RANK() OVER (PARTITION BY swap_date ORDER BY pool_volume DESC) AS daily_rank
FROM daily_pool_volume ORDER BY swap_date, daily_rank;

-- OLAP 8: 에러 카테고리별 트렌드 피벗
SELECT DATE(f.timestamp) AS failure_date, COUNT(*) AS total_failures,
       COUNT(*) FILTER (WHERE f.error_category = 'INSUFFICIENT_BALANCE') AS insufficient_balance,
       COUNT(*) FILTER (WHERE f.error_category = 'SLIPPAGE_EXCEEDED') AS slippage_exceeded,
       COUNT(*) FILTER (WHERE f.error_category = 'DEADLINE_EXPIRED') AS deadline_expired,
       COUNT(*) FILTER (WHERE f.error_category = 'UNAUTHORIZED') AS unauthorized,
       COUNT(*) FILTER (WHERE f.error_category = 'TRANSFER_FAILED') AS transfer_failed,
       COUNT(*) FILTER (WHERE f.error_category = 'UNKNOWN') AS unknown_error,
       ROUND(AVG(f.gas_used)) AS avg_gas_wasted
FROM failed_transaction f GROUP BY DATE(f.timestamp) ORDER BY failure_date;

-- ================================================================
-- END OF SCRIPT
-- ================================================================
