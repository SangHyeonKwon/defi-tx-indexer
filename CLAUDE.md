# CLAUDE.md

This file provides guidance to Claude Code (claude.ai/code) when working with code in this repository.

## 절대 규칙 (위반 금지)

- **시크릿 커밋 금지** — API 키, RPC 엔드포인트, 프라이빗 키는 `.env`에만 (git-ignored)
- **프로덕션 코드에서 `unwrap()` 금지** — `?` 연산자 또는 명시적 에러 처리 사용. 테스트에서만 허용
- **유저 입력 raw SQL 실행 금지** — 반드시 `sqlx::query!` 또는 `sqlx::query_as!`로 파라미터화
- **마이그레이션 우회 금지** — 모든 스키마 변경은 `migrations/` 폴더를 통해서만
- **async 런타임 블로킹 금지** — `std::thread::sleep`이나 무거운 CPU 작업은 `spawn_blocking` 사용
- **`main` 브랜치 직접 푸시 금지** — feature 브랜치 + PR로만
- **모든 public 함수에 doc comment (`///`) 필수**
- **모든 SQL 스크립트는 멱등성 보장** — `IF NOT EXISTS`, `CREATE OR REPLACE`, `ON CONFLICT DO NOTHING` 사용

## 아키텍처

Rust 워크스페이스 (Cargo workspace) 기반 DeFi 트랜잭션 인덱서 + REST API. 이더리움 블록체인에서 Uniswap V3 이벤트를 수집·디코딩·저장하고, JSON API로 제공한다.

### 크레이트 구조

- **`crates/indexer/`** — 인덱서 바이너리. 블록 수집 & 오케스트레이션 (`config.rs`: 환경변수/CLI, `worker.rs`: 블록 범위 워커 풀)
- **`crates/api/`** — API 바이너리. axum REST 서버 (config, error, pagination, routes/)
- **`crates/decoder/`** — 순수 라이브러리. ABI 디코딩 & 이벤트 파싱 (Swap, Mint/Burn, trace)
- **`crates/db/`** — 순수 라이브러리. SQLx 기반 모든 DB 인터랙션 (models, queries, migrate)

### 데이터 흐름

```
이더리움 노드 (RPC/WebSocket)
  → [indexer] 블록 & 영수증 수집 (동시 처리, 청크 단위)
  → [decoder] raw 로그를 패턴 매칭 → 타입 구조체 (SwapEvent, LiquidityEvent, ...)
  → [decoder::trace] debug_traceTransaction → 리버트 사유 & 콜 트리 파싱
  → [decoder::classifier] revert reason → ErrorCategory 매핑
  → [db] sqlx UNNEST 배치 INSERT → PostgreSQL
  → [api] axum REST API → JSON 응답 (읽기 전용)
  → [sql/views] 분석용 뷰 & 집계
```

### 기술 스택

| 레이어 | 기술 |
|--------|------|
| 언어 | Rust (2021 에디션, stable) |
| 비동기 런타임 | Tokio (멀티스레드) |
| 이더리움 RPC | alloy (alloy-rs) |
| DB 드라이버 | sqlx (비동기, 컴파일 타임 쿼리 검증) |
| 직렬화 | serde + serde_json |
| 로깅 | tracing + tracing-subscriber |
| 에러 처리 | thiserror (라이브러리), anyhow (바이너리) |
| CLI | clap (derive) |
| 복원력 | backoff (지수 백오프 재시도) |
| 데이터베이스 | PostgreSQL 16+ |
| 마이그레이션 | sqlx-cli |

### SQL 디렉토리 (`sql/`)

수업 제출용 독립 SQL 스크립트. `ddl/`, `dml/`, `queries/`, `procedures/`, `triggers/`, `views/`, `olap/`, `auth/` 로 분류되며 `full_script.sql`이 통합 제출본.

## 빌드 / 테스트 / 실행

```bash
# 사전 요구사항: Rust stable, PostgreSQL 16+, sqlx-cli
cargo install sqlx-cli --no-default-features --features postgres

# 초기 설정
cp .env.example .env        # DATABASE_URL, RPC_URL 채우기
sqlx database create
sqlx migrate run

# 빌드
cargo build                  # 디버그
cargo build --release        # 릴리즈

# 인덱서 실행
cargo run -p indexer -- --from-block 18000000 --to-block 18001000

# 테스트
cargo test                   # 전체 유닛 테스트
cargo test -p decoder        # 특정 크레이트만
cargo test -p db -- --ignored # 통합 테스트 (PG 필요)

# 린트
cargo clippy -- -D warnings
cargo fmt --check

# SQL 제출 스크립트 실행
psql $DATABASE_URL -f sql/full_script.sql
```

## 도메인 컨텍스트

### 핵심 용어

| 용어 | 의미 |
|------|------|
| **Swap** | Uniswap 상의 토큰 교환 — `Swap(sender, recipient, amount0, amount1, sqrtPriceX96, liquidity, tick)` |
| **Pool** | Uniswap V3 유동성 풀 — 토큰 쌍 + 수수료 티어 (500/3000/10000 bps) |
| **Mint/Burn** | 풀에 유동성 추가/제거 |
| **Revert Reason** | `require()` 실패 시 인코딩된 에러 — tx trace에서 추출 |
| **Trace** | `debug_traceTransaction` — 트랜잭션의 전체 내부 호출 트리 |
| **sqrtPriceX96** | Uniswap V3 가격 인코딩 — `price = (sqrtPriceX96 / 2^96)^2` |

### 에러 카테고리 (FailedTransaction.error_category)

`INSUFFICIENT_BALANCE` | `SLIPPAGE_EXCEEDED` | `DEADLINE_EXPIRED` | `UNAUTHORIZED` | `TRANSFER_FAILED` | `UNKNOWN`

### 주요 컨트랙트 주소 (이더리움 메인넷)

- Uniswap V3 Factory: `0x1F98431c8aD98523631AE4a59f267346ea31F984`
- Uniswap V3 Router: `0xE592427A0AEce92De3Edee1F18E0157C05861564`

## 코딩 컨벤션

### Rust

- **에러 타입**: 크레이트당 하나의 `Error` enum (`thiserror`), 바이너리는 `anyhow`로 래핑
- **구조체 필드 순서**: 도메인 필드 먼저, 메타데이터(타임스탬프, ID)는 마지막
- **DB 모델**: `sqlx::FromRow` + `serde::Serialize` derive, `db/src/models.rs`에 집중
- **디코딩 함수**: 반환 타입 `Result<DecodedEvent, DecodeError>`, 절대 panic 금지
- **동시성**: `tokio::JoinSet`으로 병렬 블록 수집, 세마포어로 동시 실행 수 제한
- **로깅**: `tracing::{info, warn, error, debug, instrument}` 사용 — `println!` 금지

### SQL

- **네이밍**: 테이블/컬럼 `snake_case`, 뷰 `vw_`, 프로시저 `sp_`, 함수 `fn_`, 트리거 `trg_`, 인덱스 `idx_`
- **PK**: `{테이블}_pkey`, **FK**: `{테이블}_{컬럼}_fkey`
- **마이그레이션**: `BEGIN;`으로 시작, `COMMIT;`으로 종료
- **주석**: 모든 테이블과 비자명 컬럼에 `COMMENT ON` 필수
- **타임스탬프**: 항상 `TIMESTAMPTZ` 사용, `TIMESTAMP` 금지

### Git

- **브랜치**: `feat/xxx`, `fix/xxx`, `sql/xxx`, `docs/xxx`
- **커밋**: `type(scope): 메시지` — scope: `indexer`, `decoder`, `db`, `sql`, `docs`, `ci`
- **커밋 단위**: 논리적 변경 하나 — SQL과 Rust를 같은 커밋에 섞지 않기
