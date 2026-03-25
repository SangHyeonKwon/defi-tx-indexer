# DB_Ethereum_Query

A Rust-based DeFi transaction indexer that collects, decodes, and stores Uniswap V3 events from the Ethereum blockchain into PostgreSQL.

## Architecture

```
Ethereum Node (RPC/WebSocket)
  -> [indexer] Block & receipt collection (concurrent, chunked)
  -> [decoder] Raw logs -> typed structs (SwapEvent, LiquidityEvent, ...)
  -> [decoder::trace] debug_traceTransaction -> revert reasons & call tree
  -> [db] sqlx batch INSERT -> PostgreSQL
  -> [sql/views] Analytical views & aggregations
```

### Crate Structure

| Crate | Type | Role |
|-------|------|------|
| `crates/indexer/` | Binary | Block collection & orchestration (config, worker pool) |
| `crates/decoder/` | Library | ABI decoding & event parsing (Swap, Mint/Burn, trace) |
| `crates/db/` | Library | SQLx-based DB interactions (models, queries, migrations) |

### SQL Directory (`sql/`)

Standalone SQL scripts organized by category: `ddl/`, `dml/`, `queries/`, `procedures/`, `triggers/`, `views/`, `olap/`, `auth/`. The `full_script.sql` is the consolidated submission script.

## Tech Stack

| Layer | Technology |
|-------|-----------|
| Language | Rust (2021 edition, stable) |
| Async Runtime | Tokio (multi-threaded) |
| Ethereum RPC | alloy (alloy-rs) |
| DB Driver | sqlx (async, compile-time query validation) |
| Serialization | serde + serde_json |
| Logging | tracing + tracing-subscriber |
| Error Handling | thiserror (libraries) / anyhow (binary) |
| Database | PostgreSQL 16+ |
| Migrations | sqlx-cli |

## Getting Started

### Prerequisites

- Rust (stable)
- PostgreSQL 16+
- sqlx-cli

```bash
cargo install sqlx-cli --no-default-features --features postgres
```

### Setup

```bash
cp .env.example .env        # Fill in DATABASE_URL and RPC_URL
sqlx database create
sqlx migrate run
```

### Build & Run

```bash
# Build
cargo build                  # Debug
cargo build --release        # Release

# Run the indexer
cargo run -p indexer -- --from-block 18000000 --to-block 18001000
```

### Test & Lint

```bash
cargo test                   # All unit tests
cargo test -p decoder        # Specific crate only
cargo test -p db -- --ignored # Integration tests (requires PostgreSQL)

cargo clippy -- -D warnings
cargo fmt --check
```

### Run SQL Scripts

```bash
psql $DATABASE_URL -f sql/full_script.sql
```

## Domain Context

- **Swap** — Token exchange on Uniswap: `Swap(sender, recipient, amount0, amount1, sqrtPriceX96, liquidity, tick)`
- **Pool** — Uniswap V3 liquidity pool: token pair + fee tier (500 / 3000 / 10000 bps)
- **Mint / Burn** — Add / remove liquidity from a pool
- **Revert Reason** — Encoded error from a failed `require()`, extracted via transaction trace
- **sqrtPriceX96** — Uniswap V3 price encoding: `price = (sqrtPriceX96 / 2^96)^2`

### Key Contracts (Ethereum Mainnet)

| Contract | Address |
|----------|---------|
| Uniswap V3 Factory | `0x1F98431c8aD98523631AE4a59f267346ea31F984` |
| Uniswap V3 Router | `0xE592427A0AEce92De3Edee1F18E0157C05861564` |

---

# DB_Ethereum_Query (한국어)

이더리움 블록체인에서 Uniswap V3 이벤트를 수집·디코딩·저장하는 Rust 기반 DeFi 트랜잭션 인덱서입니다.

## 아키텍처

```
이더리움 노드 (RPC/WebSocket)
  -> [indexer] 블록 & 영수증 수집 (동시 처리, 청크 단위)
  -> [decoder] raw 로그 -> 타입 구조체 (SwapEvent, LiquidityEvent, ...)
  -> [decoder::trace] debug_traceTransaction -> 리버트 사유 & 콜 트리 파싱
  -> [db] sqlx 배치 INSERT -> PostgreSQL
  -> [sql/views] 분석용 뷰 & 집계
```

### 크레이트 구조

| 크레이트 | 타입 | 역할 |
|----------|------|------|
| `crates/indexer/` | 바이너리 | 블록 수집 & 오케스트레이션 (설정, 워커 풀) |
| `crates/decoder/` | 라이브러리 | ABI 디코딩 & 이벤트 파싱 (Swap, Mint/Burn, trace) |
| `crates/db/` | 라이브러리 | SQLx 기반 DB 인터랙션 (모델, 쿼리, 마이그레이션) |

### SQL 디렉토리 (`sql/`)

독립 SQL 스크립트를 카테고리별로 분류: `ddl/`, `dml/`, `queries/`, `procedures/`, `triggers/`, `views/`, `olap/`, `auth/`. `full_script.sql`이 통합 제출본입니다.

## 기술 스택

| 레이어 | 기술 |
|--------|------|
| 언어 | Rust (2021 에디션, stable) |
| 비동기 런타임 | Tokio (멀티스레드) |
| 이더리움 RPC | alloy (alloy-rs) |
| DB 드라이버 | sqlx (비동기, 컴파일 타임 쿼리 검증) |
| 직렬화 | serde + serde_json |
| 로깅 | tracing + tracing-subscriber |
| 에러 처리 | thiserror (라이브러리) / anyhow (바이너리) |
| 데이터베이스 | PostgreSQL 16+ |
| 마이그레이션 | sqlx-cli |

## 시작하기

### 사전 요구사항

- Rust (stable)
- PostgreSQL 16+
- sqlx-cli

```bash
cargo install sqlx-cli --no-default-features --features postgres
```

### 초기 설정

```bash
cp .env.example .env        # DATABASE_URL, RPC_URL 채우기
sqlx database create
sqlx migrate run
```

### 빌드 & 실행

```bash
# 빌드
cargo build                  # 디버그
cargo build --release        # 릴리즈

# 인덱서 실행
cargo run -p indexer -- --from-block 18000000 --to-block 18001000
```

### 테스트 & 린트

```bash
cargo test                   # 전체 유닛 테스트
cargo test -p decoder        # 특정 크레이트만
cargo test -p db -- --ignored # 통합 테스트 (PostgreSQL 필요)

cargo clippy -- -D warnings
cargo fmt --check
```

### SQL 스크립트 실행

```bash
psql $DATABASE_URL -f sql/full_script.sql
```

## 도메인 컨텍스트

- **Swap** — Uniswap 상의 토큰 교환: `Swap(sender, recipient, amount0, amount1, sqrtPriceX96, liquidity, tick)`
- **Pool** — Uniswap V3 유동성 풀: 토큰 쌍 + 수수료 티어 (500 / 3000 / 10000 bps)
- **Mint / Burn** — 풀에 유동성 추가 / 제거
- **Revert Reason** — `require()` 실패 시 인코딩된 에러, 트랜잭션 trace에서 추출
- **sqrtPriceX96** — Uniswap V3 가격 인코딩: `price = (sqrtPriceX96 / 2^96)^2`

### 주요 컨트랙트 (이더리움 메인넷)

| 컨트랙트 | 주소 |
|----------|------|
| Uniswap V3 Factory | `0x1F98431c8aD98523631AE4a59f267346ea31F984` |
| Uniswap V3 Router | `0xE592427A0AEce92De3Edee1F18E0157C05861564` |
