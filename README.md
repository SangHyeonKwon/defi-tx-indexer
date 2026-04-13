<div align="center">

# defi-tx-indexer

**High-performance Ethereum DeFi transaction indexer built with Rust**

Collects, decodes, and stores Uniswap V3 events into PostgreSQL — in real time.

[![Rust](https://img.shields.io/badge/Rust-2021_edition-f74c00?logo=rust&logoColor=white)](https://www.rust-lang.org/)
[![PostgreSQL](https://img.shields.io/badge/PostgreSQL-16+-336791?logo=postgresql&logoColor=white)](https://www.postgresql.org/)
[![Ethereum](https://img.shields.io/badge/Ethereum-Mainnet-3C3C3D?logo=ethereum&logoColor=white)](https://ethereum.org/)
[![Uniswap](https://img.shields.io/badge/Uniswap-V3-FF007A?logo=uniswap&logoColor=white)](https://uniswap.org/)

</div>

---

## Highlights

- **Concurrent block processing** — Tokio-based worker pool with semaphore-controlled parallelism
- **Full ABI decoding** — Swap, Mint, Burn, Transfer events with typed structs via alloy
- **Transaction tracing** — `debug_traceTransaction` parsing for revert reasons and internal call trees
- **Compile-time SQL validation** — sqlx ensures query correctness at build time
- **Comprehensive SQL layer** — DDL, views, stored procedures, triggers, OLAP queries, and row-level security

## Architecture

```
Ethereum Node (RPC)
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
| Database | PostgreSQL 16+ |

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
cargo build --release

# Index blocks 18,000,000 ~ 18,001,000
cargo run -p indexer -- --from-block 18000000 --to-block 18001000
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
