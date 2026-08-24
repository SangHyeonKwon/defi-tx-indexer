# API — Failure Intelligence (M001)

> The edge Dune does not provide out of the box:
> per-transaction failure **diagnosis**, a filtered **list** with accurate
> totals, and a category **trend** feed. Three endpoints, consistent envelope,
> embeddable. Runnable example & smoke: `./scripts/verify-failed-tx.sh`.

## Authentication

**Read-only GET endpoints are public** (embed-friendly). **Write/admin
endpoints require an API key** (S16/M006).

The api server reads its admin key from `AMARILLO_ADMIN_API_KEY` at startup
(`ApiConfig::from_env` refuses to boot if it's missing or empty — D004
spirit, no silent default). 32+ bytes is recommended (a WARN log fires
below that, but boot is not blocked — operational flexibility).

Send the key as a Bearer token on every protected request:

```http
Authorization: Bearer <AMARILLO_ADMIN_API_KEY>
```

### Protected routes (S16/D021 — X policy: write/admin only)

| Method   | Path                                                   | Slice |
|----------|--------------------------------------------------------|-------|
| `POST`   | `/v1/contract-labels`                                  | S15   |
| `DELETE` | `/v1/contract-labels/{address}`                        | S15   |
| `POST`   | `/v1/alert-subscriptions`                              | S08   |
| `DELETE` | `/v1/alert-subscriptions/{id}`                         | S08   |
| `POST`   | `/v1/alert-subscriptions/{id}/rotate-secret`           | HARDEN2 |

All other `GET /v1/*` routes (single diagnosis, list, timeseries, by-label,
list-subscriptions, etc.) and `/health` ignore the header and remain
public — that preserves the embed surface (M001's core value).

### Failure response

Any failure to authenticate — header missing, malformed prefix, wrong
length, wrong value — returns the **same** 401 (S16/D021 — no info leak):

```http
HTTP/1.1 401 Unauthorized
Content-Type: application/json

{"error":"unauthorized"}
```

The server does **not** indicate which check failed. Comparing the key
itself is constant-time (`subtle::ConstantTimeEq`) to keep timing attacks
off the table.

### Examples

```bash
# Valid call
curl -sX POST http://localhost:3000/v1/contract-labels \
  -H "authorization: Bearer ${AMARILLO_ADMIN_API_KEY}" \
  -H 'content-type: application/json' \
  -d '{"address":"0xfeed…","label":"MyBot","owner_id":"alice"}'

# Missing header — 401
curl -sX POST http://localhost:3000/v1/contract-labels \
  -H 'content-type: application/json' \
  -d '{"address":"0xfeed…","label":"x"}'
# → {"error":"unauthorized"}

# Public GET — works without the key
curl -s http://localhost:3000/v1/failed-tx/0xdead…0001
```

### Rotation

A leak is handled by rotating the env var and restarting the api:

1. Generate a new value (32+ random bytes; e.g. `openssl rand -hex 32`).
2. Update `AMARILLO_ADMIN_API_KEY` in your `.env` / secret manager.
3. Restart the api service (`docker compose up -d --force-recreate api`
   or your orchestrator's equivalent).
4. Update every client that holds the key — verify scripts, examples,
   the frontend `/alerts` page (S18), any external integration. All of
   them get *new* HTTP 401s until they pick up the new key.

In-flight requests with the old key receive 401 immediately after the
new binary starts; there's no overlap window. Multi-key runtime rotation
(no restart) would require a DB-backed key store, which is a separate
slice (M006+).

### Why no JWT / OAuth / scope?

The current call pattern is **server↔server only** — operator scripts,
example clients, and the frontend's server side. No human-OAuth flow,
no per-tenant scope, no short-lived expiry is in scope (D021/D017
spirit: "first user asks first"). The shape is intentionally minimal so
that the auth model can later evolve toward DB-backed multi-key or
signed-JWT *if a real use case asks for it* — none has so far.

---

## `GET /v1/failed-tx/{tx_hash}`

Returns the decoded/classified failure record for a single transaction plus its
flattened call trace.

### Path parameters

| Param | Type | Description |
|-------|------|-------------|
| `tx_hash` | string | Transaction hash (`0x…`, 66 chars). |

### Response `200`

`ApiResponse<FailedTxDetail>` — same envelope as every other endpoint:

```jsonc
{
  "data": {
    "failed": {
      "tx_hash": "0xdead…0001",
      "error_category": "Unknown",        // ErrorCategory enum
      "revert_reason": null,              // decoded Error(string)/Panic, if any
      "failing_function": null,           // 4-byte selector of failing call
      "gas_used": 45000,
      "timestamp": "2023-09-01T12:00:00Z"
    },
    "call_tree": [                         // trace_log frames, pre-order (trace_id asc)
      {
        "tx_hash": "0xdead…0001",
        "call_depth": 0,
        "call_type": "CALL",
        "from_addr": "0x5555…5555",
        "to_addr": "0xE592…1564",         // Uniswap V3 Router
        "value": "0",
        "gas_used": 35000,
        "input": "0x414bf389",            // exactInputSingle selector
        "output": null,
        "error": "Too little received",   // ← revert text at the call site
        "trace_id": 4
      }
      // … deeper frames
    ],
    "call_tree_truncated": false,          // true if capped at MAX 2000 frames
    "root_cause": {                        // first call_tree frame with error ≠ null, or null (S10 / M004)
      "tx_hash": "0xdead…0001",
      "call_depth": 0,
      "call_type": "CALL",
      "from_addr": "0x5555…5555",
      "to_addr": "0xE592…1564",
      "value": "0",
      "gas_used": 35000,
      "input": "0x414bf389",
      "output": null,
      "error": "Too little received",
      "trace_id": 4
    }
  }
}
```

`call_tree` is ordered by `trace_id ASC` — i.e. **pre-order DFS**, the exact
order the indexer flattened the call tree when inserting (`trace_id` is a
BIGSERIAL that preserves that order). This is the correct linear tree order;
sorting by `call_depth` first would interleave sibling subtrees and make the
tree unreconstructable. Frames are flattened, not nested — nested
reconstruction is a future slice; use `call_depth` to indent/rebuild.

`call_tree` is capped at **2000** frames; `call_tree_truncated: true` signals a
partial response (defense against pathologically large traces).

`root_cause` is a *single* `trace_log` frame — the earliest entry in `call_tree`
whose `error` is non-null, i.e. **where the revert actually originated**
(pre-order DFS order). It is a redundant projection of `call_tree` provided
for the common "where did the failure come from?" question, so clients don't
have to scan the array. **`null` is explicit, not absence** — it signals
that the indexer recorded no per-frame error for this transaction (silent
default is intentionally not allowed — design decision D014).

Per-frame trace analysis is what Dune can't expose out of the box — Dune's
query model has no notion of `trace_log.error` because traces aren't part
of its public dataset. `root_cause` makes that asymmetry directly visible
to embedding products (S10 of M004).

`failing_function_decoded` resolves the 4-byte `failing_function` selector
(e.g. `0xa9059cbb`) against our self-owned `function_signature` ABI seed
table into a human-readable shape:

```jsonc
"failing_function_decoded": {              // S11 / M004 — null if no match
  "selector":  "0xa9059cbb",               // always lowercased
  "name":      "transfer",
  "signature": "transfer(address,uint256)",
  "source":    "erc20",                    // 'erc20' | 'uniswap-v3-router' | ...
  "args": [                                // S11.1 — null if input absent / decode failed
    { "type": "address", "value": "0xabc0000000000000000000000000000000000def" },
    { "type": "uint256", "value": "1000000000000000000" }
  ]
}
```

`null` is explicit (D014) — it means either `failing_function` itself was
`null` *or* the selector isn't in our seed. The contract guarantees:

- `selector === data.failed.failing_function.toLowerCase()` (self-consistency).
- `name` and `signature` are non-empty strings.
- `args` is `null` *or* an array of `{type, value}`; never absent.

### ABI args decoding (S11.1)

`args` is the typed view of the input bytes — built by walking the call's
top-level types from `signature` against the same input that the failing
call broadcast on-chain (the root frame's `input`). Each element carries:

- `type` — Solidity type string verbatim from `signature` (e.g. `"address"`,
  `"uint256"`, `"(address,uint24,uint256)"` for a nested tuple).
- `value` — lowered to JSON:
  - `address` → `"0x" + 40-hex` (lowercased).
  - `uint{N}` / `int{N}` → **decimal string** (JSON `number` only safely
    holds integers up to 2^53; `uint256` would silently lose precision in
    any JS/TS client otherwise).
  - `bool` → JSON boolean.
  - `bytes` / `bytesN` → `"0x" + hex`.
  - `string` → JSON string.
  - tuple / fixed-array / dynamic array → JSON array (recursive).

`args` is `null` whenever decoding *didn't happen* (the root frame has no
`input`, or the input is shorter than 4 bytes) or *failed* (input length
mismatch against `signature`, malformed dynamic offsets, etc.). The
surrounding `DecodedFunction` stays populated — `name` + `signature` is
still useful diagnostic data; we don't collapse it on an args miss (D027).

Param names are not surfaced — our seeded signatures are anonymous (e.g.
`transfer(address,uint256)`) and adding a field that is *always* `null`
would be misleading (D004/D014 silent-default rejection).

### `root_cause_decoded` (S11.1)

A second `DecodedFunction` keyed on `root_cause.input` instead of the
transaction's top-level call. Shape identical to `failing_function_decoded`.
Useful when the revert originated inside a sub-call whose function is
different from the top-level — e.g. a `swap` whose nested `transfer`
reverted will show `failing_function_decoded.name === "swap"` and
`root_cause_decoded.name === "transfer"`.

```jsonc
"root_cause_decoded": {                    // S11.1 — null if root_cause/input absent or unseeded
  "selector":  "0x095ea7b3",
  "name":      "approve",
  "signature": "approve(address,uint256)",
  "source":    "erc20",
  "args":      [ /* ... */ ]
}
```

Self-consistency: `root_cause_decoded.selector` always equals the first
four bytes of `root_cause.input` (lowercased). `null` covers three
distinct cases: `root_cause` itself is `null`, `root_cause.input` is `null`,
or the selector isn't in our seed.

Why self-owned ABI seed and not 4byte.directory? Two reasons (D015, D008
spirit):

1. **No runtime third-party dependency.** Production calls don't hit an
   external endpoint mid-request; the seed lives entirely in our migrations.
2. **Curated quality.** Public selector databases are full of garbage from
   typos and collisions. We seed only what we've verified against EIP-20
   and the Uniswap V3 ABI; an operator extending the seed is a deliberate
   `INSERT INTO function_signature ... ON CONFLICT DO NOTHING`.

Library choice for `args`: **alloy-sol-types** via `DynSolType::parse` (the
same `alloy` workspace dep the indexer already uses for RPC). Zero new
dependencies; full Solidity type system including nested tuples covered
out of the box (D025).

`diagnosis` answers "*why* did it fail, and what should I do about it?" by
joining `data.failed.error_category` against the self-owned
`category_diagnosis` seed (S12 / M004):

```jsonc
"diagnosis": {                              // S12 / M004 — null if category not seeded
  "message":            "The trade output was below the minimum acceptable amount (price slippage).",
  "recommended_action": "Increase slippage tolerance, or split the trade to reduce price impact.",
  "source":             "builtin"
}
```

All **ten** current `ErrorCategory` variants ship seeded with
`source: "builtin"`, so for any classified failure `diagnosis` is non-null.
`null` is still the contract for categories an operator hasn't seeded yet —
silent default is intentionally not allowed (D014).

| Category | Meaning | Notes |
|----------|---------|-------|
| `UNKNOWN` | Couldn't classify from the revert reason. | Fallback. |
| `INSUFFICIENT_BALANCE` | Sender lacks the token / ETH balance. | Includes `"STF"`, `"exceeds balance"`. |
| `INSUFFICIENT_ALLOWANCE` *(S12.1)* | ERC-20 allowance shortfall (caller must `approve` first). | Split from `INSUFFICIENT_BALANCE` — the fix is different (call `approve`, not top up). |
| `SLIPPAGE_EXCEEDED` | Generic slippage — fallback when no specific sub-category matches. | Kept as fallback for backward compat. |
| `SLIPPAGE_AMOUNT_OUT` *(S12.1)* | Buy-side slippage — output fell below `amountOutMin`. | `"too little received"`. |
| `SLIPPAGE_AMOUNT_IN` *(S12.1)* | Sell-side slippage — input exceeded `amountInMax`. | `"too much requested"`. |
| `SLIPPAGE_PRICE_IMPACT` *(S12.1)* | Pool price moved past `sqrtPriceLimitX96` during execution. | `"price slipped"` / `"amount out"`. |
| `DEADLINE_EXPIRED` | Mined after the specified deadline. | |
| `UNAUTHORIZED` | Caller lacks ownership or approval. | |
| `TRANSFER_FAILED` | An ERC-20 transfer returned false / threw. | |

The four S12.1 variants are *additive* — kept alongside the original generic
categories (`SLIPPAGE_EXCEEDED`, `INSUFFICIENT_BALANCE`) so historical data
classified before the subdivision lives on unchanged (D028: PostgreSQL
`ALTER TYPE ... DROP VALUE` is restricted, so backward compat is enforced
by *additivity*). New transactions get the more specific category whenever
the classifier matches a sub-pattern; otherwise they fall back to the
generic. The `category_diagnosis` seed carries a distinct `message` +
`recommended_action` for each sub-category so dApp developers see the
right fix in one round-trip.

Operators tune the messaging by `INSERT INTO category_diagnosis (...) ON
CONFLICT (error_category) DO UPDATE SET ...` — same self-owned seed
philosophy as `function_signature` (D015 / D016).

### Response `400` / `404`

A syntactically invalid `tx_hash` (not `0x` + 64 hex) is a **client error →
400**. A well-formed hash with no failure record is **404**:

```json
{ "error": "invalid tx_hash (expected 0x + 64 hex): 0xnothex" }   // 400
{ "error": "failed transaction 0x0000…0000" }                     // 404
```

### Why this beats the aggregate view

`/v1/analytics/failed-tx` (the existing `vw_failed_tx_analysis`) only reports
counts per `error_category`. In the example above the aggregate would bucket
this tx as **Unknown**, yet the per-tx call tree exposes the real cause —
`"Too little received"` (a slippage failure) at the router call. The
diagnosis endpoint is strictly more informative than the rollup.

## `GET /v1/failed-tx`

Filtered, paginated list of failed transactions with an accurate `total`
(the drill-down's list side; S02 of M001).

### Query parameters (all optional)

| Param | Type | Description |
|-------|------|-------------|
| `category` | string | `ErrorCategory` filter, SCREAMING_SNAKE_CASE (e.g. `SLIPPAGE_EXCEEDED`). |
| `from` | string | Lower bound on `timestamp`, RFC 3339. |
| `to` | string | Upper bound on `timestamp`, RFC 3339. |
| `limit` | int | Page size, default 20, clamped 1–100. |
| `offset` | int | Skip count, default 0. |

### Response `200`

`TotalPaginatedResponse<FailedTransaction>` — note `pagination.total` is the
full filtered count, independent of `limit`/`offset` (this is the bit Dune
dashboards don't expose as an embeddable contract):

```jsonc
{
  "data": [
    { "tx_hash": "0xdead…0001", "error_category": "Unknown",
      "revert_reason": null, "failing_function": null,
      "gas_used": 45000, "timestamp": "2023-09-01T12:00:00Z" }
  ],
  "pagination": { "limit": 20, "offset": 0, "count": 1, "total": 3 }
}
```

The existing `PaginatedResponse` (no `total`) is left unchanged for contract
compatibility; this endpoint uses the additive `TotalPaginatedResponse`
(design decision D005).

### Response `400`

Client input errors are `400`, **not** `404` (the request is well-formed-but-invalid,
no resource lookup happened):

```json
{ "error": "invalid `category`: BOGUS" }
```

— e.g. an unknown `category` value or a non-RFC3339 `from`/`to`.

## `GET /v1/analytics/failed-tx/timeseries`

Failure counts bucketed by time × error category — the trend feed for charts
(S03 of M001).

### Query parameters

| Param | Type | Description |
|-------|------|-------------|
| `interval` | string | Bucket unit: `hour` \| `day` \| `week`. Default `day`. **Whitelist only.** |
| `from` | string | Lower bound on `timestamp`, RFC 3339 (optional). |
| `to` | string | Upper bound on `timestamp`, RFC 3339 (optional). |

`interval` is a closed enum mapped to a fixed `date_trunc` literal **and** passed
as a bound parameter — never string-interpolated (SQL-injection safe).

### Response `200`

`ApiResponse<FailedTxTrendPoint[]>`, ordered by `bucket ASC, error_category ASC`:

```jsonc
{
  "data": [
    { "bucket": "2023-09-01T00:00:00Z", "error_category": "Unknown", "failure_count": 3 }
  ]
}
```

The per-category counts reconcile with `/v1/failed-tx`'s `total` for the same
filter window (asserted by the db integration test).

### Response `400`

Unknown `interval`, or non-RFC3339 `from`/`to`:

```json
{ "error": "invalid `interval` (hour|day|week): bogus" }
```

## `GET /v1/analytics/failed-tx/unknown-clusters`

Clusters of failures the classifier left as `UNKNOWN`, grouped by a normalized
revert-reason template (`fn_revert_template`) and ranked by frequency / wasted
gas — reads `vw_unknown_revert_clusters`. The top rows are the next
classifier-rule candidates: a `TEXT` cluster can usually be turned into a rule
immediately, a `CUSTOM_ERROR` cluster identifies an undecoded 4-byte selector.

Public GET, no auth. No query parameters: the view is an aggregate over the
UNKNOWN population, so the result set is inherently small — all rows are
returned, in the view's order (`occurrences DESC, total_gas_wasted DESC`).

### Response `200`

`ApiResponse<UnknownRevertCluster[]>`:

Measured against the seed data (one bare-revert UNKNOWN failure):

```jsonc
{
  "data": [
    {
      "template": "(no revert data)",          // normalized cluster key
      "cluster_kind": "NO_DATA",               // NO_DATA | CUSTOM_ERROR | PANIC | TEXT
      "occurrences": 1,
      "pct_of_unknown": "100",                 // NUMERIC → string, % of all UNKNOWN
      "total_gas_wasted": "41000",             // NUMERIC (SUM of BIGINT) → string
      "avg_gas_wasted": "41000",               // NUMERIC (ROUND(AVG)) → string
      "distinct_senders": 1,
      "distinct_selectors": 0,
      "sample_revert_reason": null,            // independent MIN — example only
      "sample_tx_hash": "0xdead000000000000000000000000000000000000000000000000000000000004",
      "first_seen": "2023-09-01T12:00:29Z",
      "last_seen": "2023-09-01T12:00:29Z"
    }
  ]
}
```

`cluster_kind` semantics:

| Kind | Meaning |
|------|---------|
| `NO_DATA` | No revert output at all (out-of-gas, bare `revert()`) |
| `CUSTOM_ERROR` | Undecoded custom error, clustered by 4-byte selector |
| `PANIC` | Solidity `Panic(uint256)` (0x11 overflow, 0x12 div-by-zero, …) |
| `TEXT` | Decoded `Error(string)` that matched no classifier rule |

Caveats: `sample_revert_reason` and `sample_tx_hash` are *independent* `MIN`s —
they may not come from the same row, so treat them as representative examples
only. NUMERIC columns serialize as JSON **strings** (BigDecimal), consistent
with the other analytics endpoints — note that trailing zeros are not
preserved (`ROUND(…, 2)` of `100.00` arrives as `"100"`), so clients must
parse, not string-match.

An empty UNKNOWN population yields **200** with `{ "data": [] }`.

## `GET /v1/analytics/failed-tx/by-label` — Failures by labeled contract (S09 / M003)

Joins on-chain failure data (`failed_transaction × transaction`) with the
**off-chain** `contract_label` table (a private mapping `address → human
label` that we store ourselves) to expose **failure distribution per labeled
contract**. This is the M003 "on-chain × private-data join" example —
exactly the kind of question Dune can't answer because Dune has no access to
your private label store.

Query parameters (all optional):

| Param   | Type        | Default | Meaning |
|---------|-------------|---------|---------|
| `from`  | RFC3339     | none    | Inclusive lower time bound (else: any) |
| `to`    | RFC3339     | none    | Inclusive upper time bound (else: any) |
| `owner` | text        | none    | Tenancy filter — empty/absent = all labels; otherwise must equal `contract_label.owner_id` |
| `limit` | integer     | 50      | Clamped 1..=200; rows are `total_failures` DESC |

Response: `ApiResponse<FailedTxByLabelPoint[]>` where each row is

```json
{
  "label": "Uniswap V3 SwapRouter",
  "address": "0xe592427a0aece92de3edee1f18e0157c05861564",
  "total_failures": 47,
  "by_category": { "SLIPPAGE_EXCEEDED": 31, "UNKNOWN": 16 }
}
```

Pivot invariant: `sum(by_category) === total_failures` (verified by
`scripts/verify-failed-tx-by-label.sh`). `address` is always lowercased
0x + 40 hex (the `contract_label` PK convention).

Errors:

- non-RFC3339 `from`/`to` → **400** `{ "error": "invalid `from` …" }`
- unknown `owner` (no matching tenant) → **200** with `{ "data": [] }`
- empty result (no labels or no matching failures) → **200** with `{ "data": [] }`

### Where labels come from

The migration `migrations/20240105000001_add_contract_label.sql` seeds:

- The Uniswap V3 SwapRouter + Factory addresses (global, `owner_id IS NULL`).
- One label per existing `pool` row (`"<pair_name> (pool)"`).

Operators add their own labels via the (admin-only) HTTP endpoints below
(S15 / M005) or by hand-rolled SQL — both target the same `contract_label`
table, so anything you create with `POST /v1/contract-labels` shows up in
`GET /v1/analytics/failed-tx/by-label` the next call.

### `POST /v1/contract-labels` — register a label (admin, S15 / M005)

```jsonc
// request
{
  "address":  "0xfeed000000000000000000000000000000000515",
  "label":    "MyArbBot-3",
  "owner_id": "alice"                  // optional; null = public label
}

// response 201
{
  "data": {
    "address":    "0xfeed000000000000000000000000000000000515",
    "label":      "MyArbBot-3",
    "owner_id":   "alice",
    "created_at": "…"
  }
}
```

`address` is normalized to lowercase. The endpoint is **UPSERT** — calling
twice with the same address overwrites the row's `label` / `owner_id`, so
operators can use the same call to rename or re-tag without a prior DELETE.

Errors:

- `address` isn't `0x` + 40 hex → **400**
- `label` is empty or > 100 bytes → **400**
- `owner_id` > 100 bytes (when present) → **400**

### `DELETE /v1/contract-labels/{address}` — remove a label (admin, S15 / M005)

- Bad address → **400**
- Address not in the table → **404**
- Successful removal → **204**

Idempotency note: the *second* DELETE on the same address returns 404 (the
row is already gone). Operators treating 404 as a no-op signal during retry
is the intended interpretation.

### Auth

Both admin endpoints are protected by `Authorization: Bearer
<AMARILLO_ADMIN_API_KEY>` (S16/M006) — see the [Authentication](#authentication)
section at the top of this file for the full policy, including how the
header is verified, the 401 contract, and rotation. The earlier
"unauthenticated, demo-scope" state (D008 / D019) was closed by S16.

### Why this is the moat

Dune queries every dataset that's *publicly indexed*. Your label set is
*consumer-specific*: which contracts you deployed, which counterparties you
care about, which user IDs your KYC system already knows. The endpoint above
shows a single concrete instance (contract labels); the same pattern fits
bot-operator self-labels or exchange KYC mapping by swapping the off-chain
table — same join, different private side.

## Verify

```bash
docker compose up -d
docker compose run --rm seed

# HTTP layer: end-to-end via the running api (200 seeded / order / 404)
./scripts/verify-failed-tx.sh

# By-label endpoint: shape + invariants + 400 + empty tenant (S09 / M003)
./scripts/verify-failed-tx-by-label.sh    # set API_PORT=… if 3001 is taken

# DB layer: query + invariants (incl. trace pre-order regression guard
# + label aggregate / owner filter / future-window emptiness)
cargo test -p db -- --ignored
```

Two complementary layers: the script exercises the live HTTP endpoint; the
`cargo test -p db -- --ignored` integration tests exercise the db queries
directly (and assert the call-tree pre-order invariant). Override the script
fixture with `GOOD_HASH=… DATABASE_URL=… API_PORT=… ./scripts/verify-failed-tx.sh`.

> **Note on `:3000`** — `verify-failed-tx.sh` builds and runs the API **locally
> on port 3001** against the compose Postgres, so it does not depend on the
> compose `api` service. The `docker compose` `api` (port **3000**) serves the
> image *as previously built*; to expose the M001 endpoints there too, rebuild:
> `docker compose up -d --build api`.
