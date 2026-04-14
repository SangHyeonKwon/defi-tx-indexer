<div align="center">

# defi-tx-indexer

**High-performance Ethereum DeFi transaction indexer built with Rust**

Collects, decodes, and stores Uniswap V3 events from historical block ranges into PostgreSQL.

[![Rust](https://img.shields.io/badge/Rust-2021_edition-f74c00?logo=rust&logoColor=white)](https://www.rust-lang.org/)
[![PostgreSQL](https://img.shields.io/badge/PostgreSQL-16+-336791?logo=postgresql&logoColor=white)](https://www.postgresql.org/)
[![Ethereum](https://img.shields.io/badge/Ethereum-Mainnet-3C3C3D?logo=ethereum&logoColor=white)](https://ethereum.org/)
[![Uniswap](https://img.shields.io/badge/Uniswap-V3-FF007A?logo=uniswap&logoColor=white)](https://uniswap.org/)

</div>

---

## Highlights

- **Concurrent block processing** — Tokio-based worker pool with semaphore-controlled parallelism
- **Full ABI decoding** — Swap, Mint, Burn, Transfer events with typed structs via alloy
- **Transaction tracing** — `debug_traceTransaction` for revert reasons, error classification, and call trees
- **True batch INSERT** — PostgreSQL UNNEST pattern for single-statement bulk inserts
- **Crash recovery** — Checkpoint-based resumption from last processed block
- **RPC resilience** — Exponential backoff retry with parallel block+receipt fetching
- **REST API** — axum-based JSON API with pagination, error handling, and CORS
- **Compile-time SQL validation** — sqlx ensures query correctness at build time
- **Comprehensive SQL layer** — DDL, views, stored procedures, triggers, OLAP queries, and row-level security

## Architecture

```
Ethereum Node (RPC)
  -> [indexer] Block & receipt collection (concurrent, chunked)
  -> [decoder] Raw logs -> typed structs (SwapEvent, LiquidityEvent, ...)
  -> [decoder::trace] debug_traceTransaction -> revert reasons & call tree
  -> [decoder::classifier] revert reason -> ErrorCategory mapping
  -> [db] sqlx UNNEST batch INSERT -> PostgreSQL
  -> [api] axum REST API -> JSON responses
  -> [sql/views] Analytical views & aggregations
```

### Crate Structure

| Crate | Type | Role |
|-------|------|------|
| `crates/indexer/` | Binary | Block collection & orchestration (config, worker pool) |
| `crates/api/` | Binary | REST API server (axum, read-only endpoints) |
| `crates/decoder/` | Library | ABI decoding & event parsing (Swap, Mint/Burn, trace) |
| `crates/db/` | Library | SQLx-based DB interactions (models, queries, migrations) |

### SQL Directory (`sql/`)

Standalone SQL scripts organized by category: `ddl/`, `dml/`, `queries/`, `procedures/`, `triggers/`, `views/`, `olap/`, `auth/`.

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
| Web Framework | axum 0.8 + tower-http |
| CLI | clap (derive) |
| Resilience | backoff (exponential retry) |
| Database | PostgreSQL 16+ |

## Quick Start (Docker)

```bash
# Start PostgreSQL + API server
docker compose up -d

# Load demo data (views, procedures, seed data)
docker compose run --rm seed

# Verify
curl http://localhost:3000/health
curl http://localhost:3000/v1/pools
curl http://localhost:3000/v1/traders/top
```

## Getting Started (Local)

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
cargo build --release

# Show CLI help
cargo run -p indexer -- --help

# Index blocks 18,000,000 ~ 18,001,000
cargo run -p indexer -- --from-block 18000000 --to-block 18001000

# Start the API server (default port 3000)
cargo run -p api
```

### API Endpoints

```bash
# Health check
curl http://localhost:3000/health

# Blocks
curl http://localhost:3000/v1/blocks/latest
curl http://localhost:3000/v1/blocks/18000000

# Pools & tokens
curl "http://localhost:3000/v1/pools?limit=10"
curl http://localhost:3000/v1/pools/0x8ad599c3a0ff1de082011efddc58f1908eb6e6d8
curl "http://localhost:3000/v1/pools/0x.../stats?from_date=2023-09-01T00:00:00Z&to_date=2023-09-30T00:00:00Z"
curl "http://localhost:3000/v1/tokens?limit=10"

# Swaps (filterable by pool)
curl "http://localhost:3000/v1/swaps?pool_address=0x...&limit=20"

# Analytics
curl http://localhost:3000/v1/traders/top?limit=10
curl http://localhost:3000/v1/analytics/daily-volume
curl http://localhost:3000/v1/analytics/failed-tx
```

### Test & Lint

```bash
cargo test                   # All unit tests
cargo test -p decoder        # Specific crate
cargo test -p db -- --ignored # Integration tests (requires PostgreSQL)

cargo clippy -- -D warnings
cargo fmt --check
```

## Domain Context

| Term | Description |
|------|-------------|
| **Swap** | Token exchange on Uniswap V3 — `Swap(sender, recipient, amount0, amount1, sqrtPriceX96, liquidity, tick)` |
| **Pool** | Liquidity pool — token pair + fee tier (500 / 3000 / 10000 bps) |
| **Mint / Burn** | Add / remove liquidity from a pool |
| **Revert Reason** | Encoded error from failed `require()`, extracted via tx trace |
| **sqrtPriceX96** | Price encoding — `price = (sqrtPriceX96 / 2^96)^2` |

### Key Contracts (Ethereum Mainnet)

| Contract | Address |
|----------|---------|
| Uniswap V3 Factory | `0x1F98431c8aD98523631AE4a59f267346ea31F984` |
| Uniswap V3 Router | `0xE592427A0AEce92De3Edee1F18E0157C05861564` |
