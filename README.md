<div align="center">

# amarillo

**Ethereum Failure Intelligence API**

"*Why* did this transaction revert?" — per-tx diagnosis at the trace level.
The *why*, not just the *what*. Real-time, embed-ready.

[![Rust](https://img.shields.io/badge/Rust-stable-f74c00?logo=rust&logoColor=white)](https://www.rust-lang.org/)
[![PostgreSQL](https://img.shields.io/badge/PostgreSQL-16+-336791?logo=postgresql&logoColor=white)](https://www.postgresql.org/)
[![Ethereum](https://img.shields.io/badge/Ethereum-Mainnet-3C3C3D?logo=ethereum&logoColor=white)](https://ethereum.org/)
[![Uniswap](https://img.shields.io/badge/Uniswap-V3-FF007A?logo=uniswap&logoColor=white)](https://uniswap.org/)

**English** · [한국어](README.ko.md)

</div>

---

## What you get

**One call returns four answers:** where it reverted, which function, why, and how to fix it.

```jsonc
GET /v1/failed-tx/0xdead…0001
{
  "failed":       { "error_category": "SLIPPAGE_AMOUNT_OUT", "revert_reason": "Too little received", ... },
  "root_cause":   { "call_depth": 2, "error": "Too little received", ... },   // (1) where
  "failing_function_decoded": { "name": "exactInputSingle", "args": [ ... ] }, // (2) which fn, typed args
  "diagnosis": {                                                               // (3) why + (4) fix
    "message": "Trade output fell below the minimum amount you specified (buy-side slippage).",
    "recommended_action": "Increase amountOutMin tolerance, or split the trade to lower price impact."
  },
  "call_tree": [ /* pre-order DFS, trace_id ASC */ ]
}
```

`null` is always explicit (no silent defaults). Every field is additive — no client regressions.
Full response shapes: [`docs/api-failed-tx.md`](docs/api-failed-tx.md).

## Why it exists

General SQL analytics platforms (Dune being the largest) answer *what happened* and *how much*.
amarillo answers **why this specific transaction reverted** — the trace-level surface those
platforms aren't built for:

| What SQL analytics platforms can't | What amarillo provides |
|------------------------------------|------------------------|
| Per-frame `trace.error` attribution | `root_cause` + `call_tree` from `debug_traceTransaction` |
| Consumer-specific ABI decoding | self-owned `function_signature` seed + `alloy::dyn_abi` runtime decode |
| Per-request webhook delivery | `/v1/alert-subscriptions` outbox dispatcher + HMAC-SHA256 |
| Real-time failure stream | `--follow --confirmations N` + dynamic reorg scan window |
| Private-data joins | `contract_label.owner_id`, partitioned via `?owner=` |

## API surface

| Endpoint | What it returns | Auth |
|----------|-----------------|------|
| `GET /v1/failed-tx/{tx_hash}` | single-tx diagnosis (root_cause + decoded fn + why/fix) | public |
| `GET /v1/failed-tx?category=&from=&to=&limit=&offset=` | filtered list + exact `total` | public |
| `GET /v1/analytics/failed-tx` | category distribution + avg gas wasted | public |
| `GET /v1/analytics/failed-tx/timeseries?interval=hour\|day\|week` | category × time trend | public |
| `GET /v1/analytics/failed-tx/by-label?owner=` | distribution by labeled contract (owner-partitioned) | public |
| `GET /v1/analytics/failed-tx/unknown-clusters` | UNKNOWN reverts clustered by normalized template | public |
| `POST` / `DELETE /v1/contract-labels` | bot-label admin (UPSERT / DELETE) | Bearer |
| `POST /v1/alert-subscriptions` (+ `/rotate-secret`) | webhook / rate-threshold alerts, one-time signing secret | Bearer |

Write/admin endpoints require `Authorization: Bearer ${AMARILLO_ADMIN_API_KEY}`; GETs are public
and embed-friendly. Missing/malformed/mismatched keys all return the same `401` (no info leak).

## Quick start

**Prereqs**: Rust stable / PostgreSQL 16+ / docker (optional). An RPC key is only needed for
backfill indexing — the demo seed runs without one.

```bash
# 1) env — AMARILLO_ADMIN_API_KEY is required (server refuses to boot without it)
cp .env.example .env
echo "AMARILLO_ADMIN_API_KEY=$(openssl rand -hex 32)" >> .env

# 2) bring it up + seed demo data
docker compose up -d
docker compose run --rm seed

# 3) single-tx diagnosis against a seeded failed tx
curl http://localhost:3000/v1/failed-tx/0xdead000000000000000000000000000000000000000000000000000000000001 | jq

# 4) dashboard
open http://localhost:8080
```

> **Mainnet indexing**: `cargo run -p indexer -- --follow --rpc-url <YOUR_RPC>`. Small-window
> backfill works on a free RPC tier; 24/7 follow requires a paid plan.

## Architecture

```
Ethereum node (RPC / WebSocket)
  → [indexer]  --follow / backfill worker pool, depth-aware reorg
  → [decoder]  Uniswap V3 events + trace → revert reason, call tree, typed ABI args
  → [decoder::classifier]  revert reason → ErrorCategory (10 variants)
  → [db]       sqlx UNNEST batch INSERT → PostgreSQL
  → [api]      axum REST + admin API key gate
  → [indexer --dispatch-alerts]  outbox → HMAC-signed webhook (SSRF + DNS guard)
```

| Crate | Type | Role |
|-------|------|------|
| `crates/indexer/` | Binary | Block ingest · orchestration · follow · dispatcher |
| `crates/api/` | Binary + Lib | axum REST server (auth + guarded routes) |
| `crates/decoder/` | Library | ABI decode · trace parse · error classifier |
| `crates/db/` | Library | SQLx models · queries · migrations |
| `crates/tui/` | Binary | Terminal dashboard (ratatui, REST client) |

## Clients & UIs

- **Web dashboard** — Vite + React 19 + Recharts, served at `:8080`.
- **Terminal UI** — `cargo run -p tui` (needs the API up). Pure REST client, points at any
  deployed instance. Details: [`crates/tui/README.md`](crates/tui/README.md).
- **Drop-in clients** — `examples/{typescript,python}-client/`, zero external deps; copy one
  file, done. Cover every `/v1/*` call + `verifyAlertSignature` (HMAC-SHA256).

End-to-end scenarios: [`docs/cookbook.md`](docs/cookbook.md). Full API + auth reference:
[`docs/api-failed-tx.md`](docs/api-failed-tx.md).

## Scope (deliberate)

Ethereum mainnet · Uniswap V3. Depth (real-time / diagnosis / consistency) is invested first;
breadth (multi-chain, multi-protocol) is deliberately frozen. This is **not** a general on-chain
analytics dashboard — that surface belongs to the general analytics platforms.

## Honest limits

- **RPC cost is the biggest variable** — mainnet `--follow` 24/7 burns a free tier in days. Start
  with small-window backfill or the historical seed.
- **ABI seed & classifier are curated/heuristic** — unseeded selectors yield
  `failing_function_decoded: null`; revert classification is string pattern matching (no external
  4byte dependency). Both are operator-extensible.
- **API key is a single env value** — rotation = update env + restart.
- **Verification is on docker compose seed data** — no live mainnet auto-regression.

## Local dev

```bash
cargo install sqlx-cli --no-default-features --features postgres
cp .env.example .env && sqlx database create && sqlx migrate run

cargo test                                   # unit
cargo test -p db -- --ignored                # integration (PG required)
cargo clippy --workspace --all-targets -- -D warnings && cargo fmt --check

cargo run -p indexer -- --from-block 18000000 --to-block 18001000   # backfill
cargo run -p indexer -- --follow --confirmations 12                 # follow
cargo run -p indexer -- --dispatch-alerts                           # webhook dispatcher
cargo run -p api                                                    # API (:3000)

./scripts/verify-failed-tx.sh          # public GETs + diagnosis semantics
./scripts/verify-alerts.sh             # alerts CRUD + HMAC + 401 cases
./scripts/verify-failed-tx-by-label.sh # by-label + admin endpoints
```
