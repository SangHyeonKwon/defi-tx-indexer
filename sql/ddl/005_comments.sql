-- ============================================
-- DeFi Analytics Database (Uniswap V3)
-- DDL: Table & Column Comments
-- ============================================

-- ────────────────────────────────────────────
-- block
-- ────────────────────────────────────────────
COMMENT ON TABLE block IS '이더리움 블록 헤더 정보';
COMMENT ON COLUMN block.block_number IS '블록 높이 (체인 상 고유 식별자)';
COMMENT ON COLUMN block.timestamp IS '블록 생성 시각 (UTC)';
COMMENT ON COLUMN block.gas_used IS '블록 내 모든 트랜잭션이 소비한 총 가스';

-- ────────────────────────────────────────────
-- token
-- ────────────────────────────────────────────
COMMENT ON TABLE token IS 'ERC-20 토큰 메타데이터 레지스트리';
COMMENT ON COLUMN token.token_address IS '토큰 컨트랙트 주소 (0x 접두사 42자)';
COMMENT ON COLUMN token.decimals IS '토큰 소수점 자릿수 (USDC=6, WETH=18 등)';

-- ────────────────────────────────────────────
-- transaction
-- ────────────────────────────────────────────
COMMENT ON TABLE transaction IS '이더리움 트랜잭션 (성공+실패 모두 포함)';
COMMENT ON COLUMN transaction.tx_hash IS '트랜잭션 해시 (0x 접두사 66자)';
COMMENT ON COLUMN transaction.from_addr IS '트랜잭션 발신자 EOA 주소';
COMMENT ON COLUMN transaction.to_addr IS '수신 주소. NULL이면 컨트랙트 생성 트랜잭션';
COMMENT ON COLUMN transaction.gas_price IS '가스 가격 (wei 단위, 1 ETH = 10^18 wei)';
COMMENT ON COLUMN transaction.value IS '전송된 ETH 양 (wei 단위)';
COMMENT ON COLUMN transaction.status IS '실행 결과: 1=성공, 0=실패 (revert)';
COMMENT ON COLUMN transaction.input_data IS '트랜잭션 콜데이터 (ABI 인코딩된 함수 호출)';

-- ────────────────────────────────────────────
-- pool
-- ────────────────────────────────────────────
COMMENT ON TABLE pool IS 'Uniswap V3 유동성 풀';
COMMENT ON COLUMN pool.pair_name IS '토큰 쌍 표시명 (e.g. WETH/USDC)';
COMMENT ON COLUMN pool.fee_tier IS '수수료 티어 (bps): 100=0.01%, 500=0.05%, 3000=0.3%, 10000=1%';
COMMENT ON COLUMN pool.token0_address IS '풀의 token0 — Uniswap이 주소 크기순으로 정렬';
COMMENT ON COLUMN pool.token1_address IS '풀의 token1 — 항상 token0 < token1 (주소 기준)';

-- ────────────────────────────────────────────
-- swap_event
-- ────────────────────────────────────────────
COMMENT ON TABLE swap_event IS 'Uniswap V3 Swap 이벤트 로그';
COMMENT ON COLUMN swap_event.amount0 IS 'token0 변동량 (부호 있음: 양수=풀로 유입, 음수=풀에서 유출)';
COMMENT ON COLUMN swap_event.amount1 IS 'token1 변동량 (부호 있음)';
COMMENT ON COLUMN swap_event.amount_in IS '스왑에 투입된 토큰 절대량';
COMMENT ON COLUMN swap_event.amount_out IS '스왑으로 받은 토큰 절대량';
COMMENT ON COLUMN swap_event.sqrt_price_x96 IS 'Uniswap V3 가격: price = (sqrtPriceX96 / 2^96)^2';
COMMENT ON COLUMN swap_event.liquidity IS '스왑 시점의 활성 유동성';
COMMENT ON COLUMN swap_event.tick IS '스왑 후 현재 틱 (가격 구간 인덱스)';
COMMENT ON COLUMN swap_event.log_index IS '트랜잭션 내 이벤트 순서 번호';

-- ────────────────────────────────────────────
-- liquidity_event
-- ────────────────────────────────────────────
COMMENT ON TABLE liquidity_event IS 'Uniswap V3 유동성 공급(Mint)/회수(Burn) 이벤트';
COMMENT ON COLUMN liquidity_event.event_type IS 'MINT=유동성 공급, BURN=유동성 회수';
COMMENT ON COLUMN liquidity_event.provider IS '유동성 공급/회수자 주소';
COMMENT ON COLUMN liquidity_event.tick_lower IS '포지션 하한 틱 (가격 하한)';
COMMENT ON COLUMN liquidity_event.tick_upper IS '포지션 상한 틱 (가격 상한)';
COMMENT ON COLUMN liquidity_event.liquidity IS '공급/회수된 유동성 단위';

-- ────────────────────────────────────────────
-- token_transfer
-- ────────────────────────────────────────────
COMMENT ON TABLE token_transfer IS 'ERC-20 Transfer 이벤트 로그';
COMMENT ON COLUMN token_transfer.amount IS '전송된 토큰 수량 (raw, 토큰 decimals 미적용)';

-- ────────────────────────────────────────────
-- failed_transaction
-- ────────────────────────────────────────────
COMMENT ON TABLE failed_transaction IS '실패(revert)한 트랜잭션 상세 분석';
COMMENT ON COLUMN failed_transaction.error_category IS '에러 분류: INSUFFICIENT_BALANCE, SLIPPAGE_EXCEEDED, DEADLINE_EXPIRED, UNAUTHORIZED, TRANSFER_FAILED, UNKNOWN';
COMMENT ON COLUMN failed_transaction.revert_reason IS '디코딩된 revert 사유 문자열 (e.g. "Too little received")';
COMMENT ON COLUMN failed_transaction.failing_function IS '실패한 함수 시그니처 (e.g. "exactInputSingle")';

-- ────────────────────────────────────────────
-- price_snapshot
-- ────────────────────────────────────────────
COMMENT ON TABLE price_snapshot IS '풀별 주기적 가격 스냅샷 (OHLCV 기반)';
COMMENT ON COLUMN price_snapshot.price IS 'token1/token0 가격 (소수점 18자리 정밀도)';
COMMENT ON COLUMN price_snapshot.tick IS '스냅샷 시점 틱';
COMMENT ON COLUMN price_snapshot.liquidity IS '스냅샷 시점 활성 유동성';
COMMENT ON COLUMN price_snapshot.interval_type IS '스냅샷 주기: 1m, 5m, 15m, 1h, 4h, 1d';

-- ────────────────────────────────────────────
-- user_profile
-- ────────────────────────────────────────────
COMMENT ON TABLE user_profile IS '유저별 활동 집계 프로필';
COMMENT ON COLUMN user_profile.label IS '유저 분류: whale(고래), bot(봇), retail(개인) 등';
COMMENT ON COLUMN user_profile.total_volume_usd IS '누적 거래량 (USD 환산)';

-- ────────────────────────────────────────────
-- trace_log
-- ────────────────────────────────────────────
COMMENT ON TABLE trace_log IS 'debug_traceTransaction으로 추출한 내부 호출 트리';
COMMENT ON COLUMN trace_log.call_depth IS '호출 깊이 (0=최상위 호출)';
COMMENT ON COLUMN trace_log.call_type IS '호출 유형: CALL, DELEGATECALL, STATICCALL, CREATE, CREATE2';
COMMENT ON COLUMN trace_log.value IS '내부 호출 시 전송된 ETH (wei 단위)';
COMMENT ON COLUMN trace_log.input IS 'ABI 인코딩된 호출 입력 데이터';
COMMENT ON COLUMN trace_log.output IS '호출 반환 데이터';
COMMENT ON COLUMN trace_log.error IS '호출 실패 시 에러 메시지';
