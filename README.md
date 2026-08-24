<div align="center">

# amarillo

**Ethereum Failure Intelligence API**

"*Why* did this transaction revert?" — per-tx diagnosis + real-time + embed-ready API.
Targets only the trace-level surface Dune structurally cannot reach.

[![Rust](https://img.shields.io/badge/Rust-stable-f74c00?logo=rust&logoColor=white)](https://www.rust-lang.org/)
[![PostgreSQL](https://img.shields.io/badge/PostgreSQL-16+-336791?logo=postgresql&logoColor=white)](https://www.postgresql.org/)
[![Ethereum](https://img.shields.io/badge/Ethereum-Mainnet-3C3C3D?logo=ethereum&logoColor=white)](https://ethereum.org/)
[![Uniswap](https://img.shields.io/badge/Uniswap-V3-FF007A?logo=uniswap&logoColor=white)](https://uniswap.org/)

**English** · [한국어](README.ko.md)

</div>

---

## What you get

**One call returns four answers, baked in:**

```jsonc
GET /v1/failed-tx/0xdead…0001

{
  "data": {
    "failed": {
      "error_category": "SLIPPAGE_AMOUNT_OUT",   // 10 classified categories
      "failing_function": "0x414bf389",
      "revert_reason":    "Too little received",
      ...
    },

    "root_cause": {                              // (1) where it reverted
      "trace_id":   7,
      "call_depth": 2,
      "error":      "Too little received",
      "input":      "0x414bf389...",
      ...
    },

    "failing_function_decoded": {                // (2) which function
      "name":      "exactInputSingle",
      "signature": "exactInputSingle((address,address,uint24,...))",
      "args": [                                  //   down to typed args
        { "type": "(address,address,uint24,address,uint256,uint256,uint256,uint160)",
          "value": [ "0xc02a...", "0xa0b8...", "3000", ... ] }
      ]
    },

    "root_cause_decoded": {                      //   if a subcall reverted separately, that too
      "name": "approve", "signature": "approve(address,uint256)", "args": [...]
    },

    "diagnosis": {                               // (3) why it failed + how to fix
      "message":            "Trade output fell below the minimum amount you specified (buy-side slippage).",
      "recommended_action": "Increase amountOutMin tolerance, or split the trade to lower price impact.",
      "source":             "builtin"
    },

    "call_tree": [ /* … pre-order DFS, trace_id ASC */ ]
  }
}
```

`null` is always **explicit** (silent defaults rejected). Every field is *additive* — no client regressions.

## Why this exists

Dune is the baseline for SQL analytics. amarillo does *not compete* with Dune — it targets only
the *trace-level surface Dune structurally cannot reach*:

| What Dune cannot do | What amarillo nails |
|---------------------|--------------------|
| `trace.error` per-frame attribution | `root_cause` + `call_tree` (`debug_traceTransaction` parsing) |
| Consumer-specific ABI decoding | Self-owned `function_signature` seed (17 selectors) + `alloy::dyn_abi` runtime decode |
| Per-request webhook delivery | `/v1/alert-subscriptions` outbox dispatcher + HMAC-SHA256 |
| Real-time failure stream | `--follow --confirmations N` + dynamic reorg scan window |
| Private-data joins | `contract_label.owner_id` — partitioned via `?owner=` filter |

## Feature matrix

| Capability | Surface | Persona |
|-----------|---------|---------|
| Single-tx diagnosis (root_cause + decoded fn + diagnosis) | `GET /v1/failed-tx/{tx_hash}` | dApp developer |
| Filtered list + exact `total` | `GET /v1/failed-tx?category=&from=&to=&limit=&offset=` | dApp / data team |
| Category × time trend | `GET /v1/analytics/failed-tx/timeseries?interval=hour\|day\|week` | dApp / SRE |
| Failure distribution by labeled contract (`owner_id` partitioned) | `GET /v1/analytics/failed-tx/by-label?owner=...` | Bot operator / KYC |
| Bot label admin (UPSERT / DELETE) | `POST/DELETE /v1/contract-labels` | Bot operator |
| Per-event webhook subscription (HMAC-SHA256) | `POST /v1/alert-subscriptions` (one-time `signing_secret`) | dApp / Bot operator |
| Rate-threshold alerts (count ≥ N in window, debounce) | `sub_type=rate_threshold` | Bot operator |
| `signing_secret` rotation | `POST /v1/alert-subscriptions/{id}/rotate-secret` | Operator |
| Real-time indexer (reorg-safe) | `indexer --follow` + dynamic scan window (~`REORG_SCAN_CAP=4096`) | Operator |
| ABI args decoder (10 variants + nested tuple) | `failing_function_decoded.args` + `root_cause_decoded` | dApp developer |
| Category diagnosis (10 classes) | `error_category` enum + `category_diagnosis` seed | dApp developer |
| Admin API key auth (write/admin guarded, GET public) | `Authorization: Bearer ${AMARILLO_ADMIN_API_KEY}` | Operator |
| SSRF guard (URL validation + DNS-time IP block) | `db::validators::webhook_url_is_safe` + `SafeDnsResolver` | Operator (indirect) |
| Drop-in clients (zero deps) | `examples/typescript-client/`, `examples/python-client/` | dApp developer |

## Quick start

**Prereqs**: Rust stable / PostgreSQL 16+ / docker (optional). An RPC key is only needed for
*backfill indexing* — the demo seed runs without one.

```bash
# 1) env setup (AMARILLO_ADMIN_API_KEY is required — the server refuses to boot without it)
cp .env.example .env
echo "AMARILLO_ADMIN_API_KEY=$(openssl rand -hex 32)" >> .env

# 2) one-line docker compose
docker compose up -d
docker compose run --rm seed

# 3) single-tx diagnosis — against a seeded failed tx
curl http://localhost:3000/v1/failed-tx/0xdead000000000000000000000000000000000000000000000000000000000001 | jq

# 4) guarded endpoint (write/admin)
source .env
curl -sX POST http://localhost:3000/v1/contract-labels \
  -H "Authorization: Bearer ${AMARILLO_ADMIN_API_KEY}" \
  -H 'Content-Type: application/json' \
  -d '{"address":"0xfeed000000000000000000000000000000000bee","label":"MyArbBot","owner_id":"alice"}' | jq

# 5) dashboard
open http://localhost:8080
```

> **Mainnet indexing**: `cargo run -p indexer -- --follow --rpc-url <YOUR_RPC>`. Small-window
> backfill works on Alchemy/Infura free tier, but 24/7 follow *requires a paid plan*.

## Live output (measured, on the docker compose seed)

Actual responses and gate output you get by following the Quick start verbatim.

### `GET /v1/failed-tx/0xdead…0001`

```json
{
  "failed": {
    "tx_hash":        "0xdead000000000000000000000000000000000000000000000000000000000001",
    "error_category": "Unknown",
    "revert_reason":  null,
    "failing_function": null,
    "gas_used":       45000,
    "timestamp":      "2023-09-01T12:00:00Z"
  },
  "root_cause": {
    "trace_id":   16,
    "call_depth": 0,
    "error":      "Too little received",
    "input":      "0x414bf389"
  },
  "root_cause_decoded": {
    "selector":  "0x414bf389",
    "name":      "exactInputSingle",
    "signature": "exactInputSingle((address,address,uint24,address,uint256,uint256,uint256,uint160))",
    "source":    "uniswap-v3-router",
    "args":      null
  },
  "failing_function_decoded": null,
  "diagnosis": {
    "message":            "The exact failure mode could not be classified from the trace alone.",
    "recommended_action": "Inspect root_cause and the call_tree; raise an issue with the tx hash.",
    "source":             "builtin"
  }
}
```

> For the seeded tx, **`root_cause_decoded` resolves Uniswap V3 `exactInputSingle` immediately
> from the self-owned ABI seed (D015)**. `args: null` means the selector is present but typed
> bytes are absent, so decoding is not attempted (D027) — the object itself is preserved.

### `GET /v1/failed-tx?limit=3` (filter · pagination + exact `total`)

```json
{
  "data": [
    { "tx_hash": "0xdead00000000...", "error_category": "Unknown", "gas_used": 52000, "timestamp": "2023-09-01T12:00:24Z" },
    { "tx_hash": "0xdead00000000...", "error_category": "Unknown", "gas_used": 38000, "timestamp": "2023-09-01T12:00:12Z" },
    { "tx_hash": "0xdead00000000...", "error_category": "Unknown", "gas_used": 45000, "timestamp": "2023-09-01T12:00:00Z" }
  ],
  "pagination": { "limit": 3, "offset": 0, "count": 3, "total": 3 }
}
```

### `GET /v1/analytics/failed-tx/timeseries?interval=day`

```json
{
  "data": [
    { "bucket": "2023-09-01T00:00:00Z", "error_category": "Unknown", "failure_count": 3 }
  ]
}
```

### `POST /v1/contract-labels` — *no Authorization header*

```
HTTP/1.1 401 Unauthorized
{"error":"unauthorized"}
```

A single info-leak-safe response (D021) — missing header / malformed Bearer / key mismatch /
length mismatch all return the *same* 401. Neither key presence nor length is leaked.

### `verify-failed-tx.sh` (measured)

```
GOOD (0xdead...0001): HTTP 200
  PASS
  ORDER OK (pre-order: root first, trace_id strictly ascending)
  ROOT OK (trace_id=16 matches first error frame in call_tree)
  DECODED OK (null — selector absent or not in self-owned ABI seed)
  ROOT_DECODED OK (exactInputSingle :: exactInputSingle((address,address,uint24,address,uint256,uint256,uint256,uint160)))
  DIAG OK (msg="The exact failure mode could not be clas…")
BAD  (0x0000…): HTTP 404  PASS
MALFORMED (0xnothex): HTTP 400  PASS
LIST (?category=UNKNOWN&limit=2): HTTP 200  PASS (total=3, returned=2)
LIST (?category=BOGUS): HTTP 400  PASS
LIST (?from=not-a-date): HTTP 400  PASS
TIMESERIES (?interval=day): HTTP 200  PASS (points=1)
TIMESERIES (?interval=bogus): HTTP 400  PASS
ALL PASS
```

### Test gate (single-breath rerun)

```
cargo test -p decoder    31/31  (abi 9 + classifier 10 + events 7 + trace 5)
cargo test -p api        20/20  (config 6 + auth 7 + integration 7)
cargo test -p indexer    36/36  (follow + reorg + worker + dispatcher)
cargo test -p db --lib   17/17  (validators)
cargo test -p db --ign.  27/27  (alerts 3 + alert_rate 3 + category_diagnosis 3
                                 + failed_tx 10 + function_signature 4
                                 + labels 3 + rollback 1)
web test                 41/41  (contract.test 34 + App.smoke 3 + client.test 4)
cargo fmt --check        clean
clippy --all-targets     0 warnings
```

## Screenshots

Dashboard screenshots live under `docs/screenshots/` (guide below) — boot it and capture yourself:

| Screen | URL | Suggested capture |
|--------|-----|-------------------|
| Overview | http://localhost:8080 | KPI cards + daily volume chart |
| Failed Tx | http://localhost:8080/failed-tx | Category donut + timeseries + **Tx Inspection** (root_cause + decoded args panel) |
| Alerts | http://localhost:8080/alerts | **API key input panel** + subscription form + list (create / rotate / disable actions) + one-time secret modal |

## Architecture

```
Ethereum node (RPC / WebSocket)
  → [indexer]  --follow / --from-block --to-block worker pool, depth-aware reorg
  → [decoder]  Uniswap V3 events (Swap / Mint / Burn) + Transfer decoding
  → [decoder::trace]   debug_traceTransaction → revert reason + call tree
  → [decoder::classifier]  revert reason → ErrorCategory (10 variants)
  → [decoder::abi]     selector + signature + input → DecodedArg[] (typed)
  → [db]      sqlx UNNEST batch INSERT → PostgreSQL
  → [api]     axum REST + admin API key gate (`_: AdminAuth` extractor)
  → [indexer --dispatch-alerts]  outbox → HMAC-signed webhook (SSRF + DNS guard)
```

| Crate | Type | Role |
|-------|------|------|
| `crates/indexer/` | Binary | Block ingest · orchestration · follow · dispatcher |
| `crates/api/` | Binary + Lib | axum REST server (auth + 5 guarded routes) |
| `crates/decoder/` | Library | ABI decode · trace parse · error classifier |
| `crates/db/` | Library | SQLx models · queries · migrations |
| `crates/tui/` | Binary | Terminal dashboard — REST API client (ratatui) |

## Drop-in clients

```bash
examples/typescript-client/   # fetch + node:crypto, zero external deps
examples/python-client/       # urllib + hmac stdlib, zero external deps
```

`AmarilloClient` covers every `/v1/*` call + `verifyAlertSignature` (HMAC-SHA256).
**No `npm install` / `pip install`** — copy one or two `.ts` / `.py` files and you're done
(D017 spirit; publishing is a separate slice, S13.1).

End-to-end five scenarios (single-tx diagnosis / alerts + HMAC / label distribution / bot
operator playbook / `/alerts` UI flow): [`docs/cookbook.md`](docs/cookbook.md).

Full API reference + Authentication policy: [`docs/api-failed-tx.md`](docs/api-failed-tx.md).

## Terminal UI

![amarillo TUI — Overview screen](docs/screenshots/tui-overview.png)

```bash
cargo run -p api                                            # API must be up
AMARILLO_API_URL=http://127.0.0.1:3000 cargo run -p tui     # terminal dashboard
```

A second reference client (alongside `web/`) — the failure intelligence in your
terminal. Pure REST client, so it needs no DB credentials and can point at any
deployed amarillo instance. Overview (KPIs + category distribution), filterable
failed-tx table, and call-tree drill-down with decoded args + diagnosis. Details:
[`crates/tui/README.md`](crates/tui/README.md).

## Scope (deliberate)

- **Chain**: Ethereum mainnet
- **Protocol**: Uniswap V3
- **Frozen** (D003): depth (real-time / diagnosis / consistency) first, breadth (multi-chain ·
  multi-protocol) deliberately uninvested
- **Not**: general on-chain analytics dashboard (the surface Dune dominates)

## Tech stack

| Layer | Technology |
|-------|-----------|
| Language | Rust (2021 edition, stable, `rust-version = "1.75"`) |
| Async runtime | Tokio (multi-threaded) |
| Ethereum RPC | alloy 1 (`[full]` features) |
| ABI runtime decode | `alloy::dyn_abi` (S11.1) |
| DB driver | sqlx (async, compile-time query validation) |
| Web framework | axum 0.8 + tower-http |
| Crypto / auth | hmac + sha2 + subtle (constant-time eq) |
| Outbound HTTP | reqwest (rustls, default-features off) |
| Resilience | backoff (exponential retry) |
| Database | PostgreSQL 16+ |
| Migrations | sqlx-cli |
| Dashboard | Vite + React 19 + TanStack Query + Recharts |

## Honest limits

- **RPC cost is the biggest variable**: mainnet `--follow` 24/7 burns through Alchemy's free
  tier (300M CU/month) in a few days. Start with *small-window backfill* or *historical seed +
  demo*.
- **Self-owned ABI seed**: 17 curated selectors. Unseeded selectors yield
  `failing_function_decoded: null` — operators extend via
  `INSERT INTO function_signature ... ON CONFLICT DO NOTHING` (D015 self-seed policy).
- **API key is a single value**: env-only. Rotation = update env + restart. Zero-downtime
  multi-key rotation is a separate milestone.
- **Classifier is heuristic**: revert reason string pattern matching. No external
  4byte.directory dependency (D015 spirit).
- **`SLIPPAGE_EXCEEDED` / `INSUFFICIENT_BALANCE` are permanent fallbacks**: PostgreSQL enums
  cannot `DROP VALUE`, so the direction is *additive only*.
- **No live mainnet auto-regression**: all verification is on docker compose seed data.
- **No bot operator dashboard UI**: bot operators are CLI/script users; the dApp persona has
  `/failed-tx` + `/alerts` UI.

## Local dev

```bash
# Prerequisites
cargo install sqlx-cli --no-default-features --features postgres

# Setup
cp .env.example .env
sqlx database create
sqlx migrate run

# Build & test
cargo build --release
cargo test                            # All unit tests
cargo test -p db -- --ignored         # Integration tests (PG required)
cargo clippy --workspace --all-targets -- -D warnings
cargo fmt --check

# Run indexer (backfill)
cargo run -p indexer -- --from-block 18000000 --to-block 18001000

# Run indexer (follow)
cargo run -p indexer -- --follow --confirmations 12

# Run dispatcher (separate process)
cargo run -p indexer -- --dispatch-alerts

# Run API server (default port 3000)
cargo run -p api

# Verify the API surface
./scripts/verify-failed-tx.sh           # public GETs + diagnosis semantics
./scripts/verify-alerts.sh              # alerts CRUD + HMAC + 401 cases
./scripts/verify-failed-tx-by-label.sh  # by-label + admin endpoints
```
