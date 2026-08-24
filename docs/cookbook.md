# Amarillo cookbook

Three end-to-end scenarios against the Amarillo Failure Intelligence API,
shown side by side in `curl`, TypeScript (via [`examples/typescript-client/`](../examples/typescript-client/)),
and Python (via [`examples/python-client/`](../examples/python-client/)). Both
example clients are stdlib-only — copy them straight into your project; no
`npm install`, no `pip install`.

The full endpoint contract reference lives in [`api-failed-tx.md`](api-failed-tx.md)
and [`api-alerts.md`](api-alerts.md). This file is about *using* it.

> **Authentication note (S16/M006)** — write/admin endpoints
> (`POST/DELETE /v1/contract-labels(*)`, `POST/DELETE/rotate /v1/alert-subscriptions(*)`)
> require `Authorization: Bearer <AMARILLO_ADMIN_API_KEY>`. **GET endpoints
> are public** (embed-friendly) and ignore the header. The examples below
> source the key from your environment:
> ```bash
> export AMARILLO_ADMIN_API_KEY=$(grep ^AMARILLO_ADMIN_API_KEY .env | cut -d= -f2)
> ```
> Both example clients accept the key in their constructor — see
> [`api-failed-tx.md#Authentication`](api-failed-tx.md#authentication) for the
> policy in full.

---

## 1. Single-tx diagnosis

The flagship question: "this hash failed — *where, what, why, and what should
I do?*" M001~M003 (data / real-time / actionable alerts) plus M004 (depth —
S10 `root_cause` + S11 `failing_function_decoded` + S12 `diagnosis`) all
converge on one endpoint.

### curl

```bash
curl -s http://localhost:3000/v1/failed-tx/0xdead000000000000000000000000000000000000000000000000000000000001 | jq
```

Response (S10/S11/S12 additive on top of the M001 base):

```jsonc
{
  "data": {
    "failed": {
      "tx_hash": "0xdead…0001",
      "error_category": "Unknown",
      "revert_reason": null,
      "failing_function": "0xa9059cbb",
      "gas_used": 45000,
      "timestamp": "2023-09-01T12:00:00Z"
    },
    "call_tree": [ /* … pre-order DFS, trace_id ASC */ ],
    "call_tree_truncated": false,
    "root_cause":               { "trace_id": 16, "error": "Too little received", "call_depth": 2, /* … */ },
    "failing_function_decoded": {
      "selector":  "0xa9059cbb",
      "name":      "transfer",
      "signature": "transfer(address,uint256)",
      "source":    "erc20",
      // S11.1 — typed args. `uint256` is a decimal *string* (precision-safe).
      "args": [
        { "type": "address", "value": "0xabc0000000000000000000000000000000000def" },
        { "type": "uint256", "value": "1000000000000000000" }
      ]
    },
    // S11.1 — `root_cause.input` decoded the same way. Useful when the revert
    // originated in a sub-call whose function differs from the top-level
    // (e.g. outer `swap` whose nested `transfer` reverted).
    "root_cause_decoded": {
      "selector":  "0x095ea7b3",
      "name":      "approve",
      "signature": "approve(address,uint256)",
      "source":    "erc20",
      "args":      [ /* … same shape as above */ ]
    },
    "diagnosis":                { "message": "…", "recommended_action": "…", "source": "builtin" }
  }
}
```

`null` is **always explicit**, never silently absent — D014 / D016. The
backend won't drop a field; if it's not seeded / not applicable, you get
`null` and decide how to handle it. `args` follows the same rule: `null`
means decoding wasn't attempted (no input bytes) or failed; the surrounding
`DecodedFunction` stays populated (D027). For `uint*` / `int*`, values
arrive as **decimal strings** so `uint256` round-trips through JS without
losing precision past 2^53.

**S12.1: subdivided categories.** `error_category` has ten possible values
now — the original six plus four sub-categories that distinguish slippage
flavors and `INSUFFICIENT_ALLOWANCE` from generic `INSUFFICIENT_BALANCE`.
The `diagnosis.message` and `recommended_action` are tuned per
sub-category, so a buy-side `SLIPPAGE_AMOUNT_OUT` failure tells you to
raise `amountOutMin` rather than the generic "increase slippage tolerance"
the parent category gives. Branch on the sub-category in your UI when you
have it; fall back to the parent for backward compat. See
[`api-failed-tx.md`](api-failed-tx.md#response-200) for the full table.

### TypeScript

```typescript
import { AmarilloClient } from "./client.ts";

const client = new AmarilloClient("http://localhost:3000");
const detail = await client.getFailedTx("0xdead…0001");

console.log("category:", detail.failed.error_category);
console.log("function:",
  detail.failing_function_decoded?.name ?? detail.failed.failing_function);
if (detail.diagnosis) {
  console.log("why:    ", detail.diagnosis.message);
  console.log("action: ", detail.diagnosis.recommended_action ?? "(none)");
}
if (detail.root_cause) {
  console.log("revert frame:", detail.root_cause.trace_id, detail.root_cause.error);
}
```

### Python

```python
from client import AmarilloClient

client = AmarilloClient("http://localhost:3000")
detail = client.get_failed_tx("0xdead…0001")

print("category:", detail.failed.error_category)
fn = (
    detail.failing_function_decoded.name
    if detail.failing_function_decoded is not None
    else detail.failed.failing_function
)
print("function:", fn)
if detail.diagnosis is not None:
    print("why:    ", detail.diagnosis.message)
    print("action: ", detail.diagnosis.recommended_action or "(none)")
if detail.root_cause is not None:
    print("revert frame:", detail.root_cause.trace_id, repr(detail.root_cause.error))
```

---

## 2. Alert subscription + webhook HMAC verification

"Notify my webhook every time a `SLIPPAGE_EXCEEDED` failure happens." The
`signing_secret` is revealed **exactly once** at creation (and rotation);
the dispatcher signs each delivery with HMAC-SHA256 of the **raw body
bytes**, keyed by the 32 bytes obtained from hex-decoding the secret.

### curl: create the subscription

```bash
curl -sX POST http://localhost:3000/v1/alert-subscriptions \
  -H "authorization: Bearer ${AMARILLO_ADMIN_API_KEY}" \
  -H 'content-type: application/json' \
  -d '{"webhook_url":"https://example.com/hook","error_category":"SLIPPAGE_EXCEEDED"}' \
  | jq
```

```jsonc
{
  "data": {
    "subscription_id": 42,
    "webhook_url":     "https://example.com/hook",
    "error_category":  "SLIPPAGE_EXCEEDED",
    "to_addr":         null,
    "signing_secret":  "<64 hex chars — store immediately, never returned again>",
    "active":          true,
    "created_at":      "…"
  }
}
```

Each delivery the dispatcher POSTs carries:

- `Content-Type: application/json`
- `X-Amarillo-Signature: sha256=<hex>`

### TypeScript receiver (Express)

```typescript
import express from "express";
import { verifyAlertSignature } from "./client.ts";

const SECRET = process.env.AMARILLO_SIGNING_SECRET!; // 64 hex chars
const app = express();

app.post(
  "/amarillo-webhook",
  express.raw({ type: "application/json" }), // raw body — NOT express.json()
  (req, res) => {
    const ok = verifyAlertSignature(
      req.body,                             // Buffer
      req.header("x-amarillo-signature"),
      SECRET,
    );
    if (!ok) return res.status(401).json({ error: "bad signature" });
    const payload = JSON.parse(req.body.toString("utf8"));
    // handle payload…
    res.json({ ok: true });
  },
);
```

### Python receiver (Flask)

```python
import os
from flask import Flask, request, abort
from client import verify_alert_signature

SECRET = os.environ["AMARILLO_SIGNING_SECRET"]  # 64 hex chars
app = Flask(__name__)


@app.post("/amarillo-webhook")
def webhook():
    if not verify_alert_signature(
        request.get_data(),                            # raw bytes — NOT request.json
        request.headers.get("X-Amarillo-Signature"),
        SECRET,
    ):
        abort(401, description="bad signature")
    payload = request.get_json()                       # safe after verification
    # handle payload…
    return {"ok": True}
```

`request.get_data()` / `express.raw()` — read **raw bytes**, not parsed
JSON. Reading-then-reserializing would break byte-equality and your
signatures would never verify.

### Rotating the secret

```bash
curl -sX POST http://localhost:3000/v1/alert-subscriptions/42/rotate-secret \
  -H "authorization: Bearer ${AMARILLO_ADMIN_API_KEY}" \
  | jq
```

Same one-time-reveal contract as creation. The dispatcher will start
signing with the new secret immediately — flip over your receiver before
calling rotate.

---

## 3. Failures by labeled contract

The S09 demo of an on-chain × off-chain join — failures grouped by labeled
contract. Operators seed labels via the `contract_label` table (the migration
ships a Uniswap V3 SwapRouter + Factory + per-pool starter set).

### curl

```bash
curl -s "http://localhost:3000/v1/analytics/failed-tx/by-label?limit=10" | jq
```

```jsonc
{
  "data": [
    {
      "label":          "Uniswap V3 SwapRouter",
      "address":        "0xe592427a0aece92de3edee1f18e0157c05861564",
      "total_failures": 47,
      "by_category":    { "SLIPPAGE_EXCEEDED": 31, "UNKNOWN": 16 }
    }
  ]
}
```

Pivot invariant: `sum(by_category) === total_failures` (asserted in
`scripts/verify-failed-tx-by-label.sh`). `address` is always lowercased.

### TypeScript

```typescript
const rows = await client.getFailedTxByLabel({ limit: 10 });
for (const r of rows) {
  console.log(`${r.label} (${r.address.slice(0, 10)}…) total=${r.total_failures}`);
  for (const [cat, n] of Object.entries(r.by_category)) {
    console.log(`  ${cat}: ${n}`);
  }
}
```

### Python

```python
rows = client.get_failed_tx_by_label(limit=10)
for r in rows:
    print(f"{r.label} ({r.address[:10]}…) total={r.total_failures}")
    for cat, n in r.by_category.items():
        print(f"  {cat}: {n}")
```

---

## 4. Bot operator playbook (M005)

End-to-end flow for the bot-operator persona: register your bot's address as
a private label, subscribe to *rate-threshold* alerts (S14 — only fires when
the failure count crosses a window threshold, then debounces), receive
signed deliveries, and slice the failures by *your* labels (filtered to
`owner_id=you`). All write/admin steps below require the admin API key
(S16/M006 — [`api-failed-tx.md#Authentication`](api-failed-tx.md#authentication)).

### Step 1: register your bot as a label

```bash
curl -sX POST http://localhost:3000/v1/contract-labels \
  -H "authorization: Bearer ${AMARILLO_ADMIN_API_KEY}" \
  -H 'content-type: application/json' \
  -d '{"address":"0xfeed000000000000000000000000000000000bee","label":"MyArbBot-3","owner_id":"alice"}' \
  | jq
```

The endpoint is **UPSERT** — re-POSTing the same address rewrites the
label/owner. `address` lowercases server-side.

```typescript
import { AmarilloClient } from "./client.ts";

const client = new AmarilloClient("http://localhost:3000", {
  apiKey: process.env.AMARILLO_ADMIN_API_KEY,
});
const label = await client.createContractLabel({
  address:  "0xfeed000000000000000000000000000000000bee",
  label:    "MyArbBot-3",
  owner_id: "alice",
});
console.log(label.address, label.label, label.owner_id);
```

```python
import os
from client import AmarilloClient

client = AmarilloClient(
    "http://localhost:3000",
    api_key=os.environ["AMARILLO_ADMIN_API_KEY"],
)
label = client.create_contract_label(
    address="0xfeed000000000000000000000000000000000bee",
    label="MyArbBot-3",
    owner_id="alice",
)
print(label.address, label.label, label.owner_id)
```

Cleanup (when you're done with the demo):
`await client.deleteContractLabel("0xfeed…0bee")` /
`client.delete_contract_label("0xfeed…0bee")` — returns 204 the first time,
raises `AmarilloError(404)` on the second (intentional idempotency
signal — operators treat 404 as "already removed").

### Step 2: subscribe with `sub_type='rate_threshold'`

```bash
curl -sX POST http://localhost:3000/v1/alert-subscriptions \
  -H "authorization: Bearer ${AMARILLO_ADMIN_API_KEY}" \
  -H 'content-type: application/json' \
  -d '{
        "webhook_url":           "https://my-receiver.example.com/bot-alerts",
        "to_addr":               "0xfeed000000000000000000000000000000000bee",
        "sub_type":              "rate_threshold",
        "threshold_count":       10,
        "threshold_window_secs": 300,
        "debounce_secs":         600
      }' \
  | jq
```

Save the one-time `signing_secret` *now* — the server never returns it again.

### Step 3: dispatcher fires when count ≥ 10 in 5 min

The dispatcher (`indexer --dispatch-alerts`) polls, computes the rolling
match count, and POSTs to `webhook_url` when the threshold is crossed.
After a delivery it silences the same subscription for `debounce_secs`,
so a single noisy outage produces *one* page, not a cascade.

Receiver outline — verify the signature *before* trusting the body, branch
on `sub_type`:

```typescript
// Node / Express (TypeScript)
import { verifyAlertSignature } from "./client.ts";

app.post(
  "/bot-alerts",
  express.raw({ type: "application/json" }),
  (req, res) => {
    const ok = verifyAlertSignature(
      req.body,
      req.header("x-amarillo-signature"),
      process.env.AMARILLO_SIGNING_SECRET!,
    );
    if (!ok) return res.status(401).json({ error: "bad signature" });
    const payload = JSON.parse(req.body.toString("utf8"));
    if (payload.sub_type === "rate_threshold") {
      // payload: { subscription_id, sub_type, match_count, threshold_count, threshold_window_secs }
      pageOps(`Bot rate spike: ${payload.match_count} failures in ${payload.threshold_window_secs}s`);
    } else {
      // S08 per-event payload: { subscription_id, tx_hash }
      logToTicket(payload);
    }
    res.json({ ok: true });
  },
);
```

```python
# Flask (Python)
from flask import Flask, request, abort
from client import verify_alert_signature
import os

SECRET = os.environ["AMARILLO_SIGNING_SECRET"]
app = Flask(__name__)


@app.post("/bot-alerts")
def webhook():
    if not verify_alert_signature(
        request.get_data(),
        request.headers.get("X-Amarillo-Signature"),
        SECRET,
    ):
        abort(401, description="bad signature")
    payload = request.get_json()
    if payload["sub_type"] == "rate_threshold":
        page_ops(
            f"Bot rate spike: {payload['match_count']} failures in "
            f"{payload['threshold_window_secs']}s"
        )
    else:
        log_to_ticket(payload)
    return {"ok": True}
```

### Step 4: investigate with `by-label?owner=you`

```bash
curl -s "http://localhost:3000/v1/analytics/failed-tx/by-label?owner=alice&limit=20" | jq
```

Only labels you own come back — public labels (Uniswap router, etc.) stay
out of view. Pivot invariant still holds: `sum(by_category) === total_failures`.

```typescript
const rows = await client.getFailedTxByLabel({ owner: "alice", limit: 20 });
for (const r of rows) {
  console.log(`${r.label} (${r.address.slice(0, 10)}…): ${r.total_failures} failures`);
  for (const [cat, n] of Object.entries(r.by_category)) {
    console.log(`  ${cat}: ${n}`);
  }
}
```

```python
rows = client.get_failed_tx_by_label(owner="alice", limit=20)
for r in rows:
    print(f"{r.label} ({r.address[:10]}…): {r.total_failures} failures")
    for cat, n in r.by_category.items():
        print(f"  {cat}: {n}")
```

### Race semantics (S14 / D018)

Two dispatcher workers can briefly fire the same rate alert before either
writes its `alert_rate_dispatch` row — at worst one extra delivery. Permanent
duplication is impossible (the *next* cycle's debounce check sees the
latest `dispatched_at`). Receivers needing strict exactly-once should
dedupe on `subscription_id + match_count`.

### If you forget the API key

A write call without `Authorization: Bearer <key>` (or with a wrong key)
returns **401** — same response for every cause (header missing / wrong
prefix / wrong key / wrong length) so the server doesn't leak which part
failed (S16/M006/D021):

```bash
$ curl -sX POST http://localhost:3000/v1/contract-labels \
    -H 'content-type: application/json' \
    -d '{"address":"0xfeed…","label":"x"}' \
    -w '\nHTTP %{http_code}\n'
{"error":"unauthorized"}
HTTP 401
```

The example clients surface this *locally* before the request leaves —
TypeScript throws `AmarilloError(0, "missing API key: …")`, Python raises
`ValueError`. That keeps the operator's mistake noisy at the call site
instead of waiting for a 401.

---

## 5. From the `/alerts` page (S18)

The same write flow as scenario 2/4, driven through the web dashboard
instead of curl/TS/Python. Use this when you want to demo the
subscription lifecycle without writing a script.

**Open the page**:

```bash
docker compose up -d            # postgres + api + web
open http://localhost:8080/alerts  # or your VITE_APP_BASE_PATH
```

**Step 1 — apply the admin API key**:

The top of `/alerts` has an "Admin API key" panel. Paste your key, click
**Apply**. The page collapses the input to "Key active (N chars)" and
the *Create / Rotate / Deactivate* buttons enable. While the key is
empty, those buttons are disabled with a `title="API key required
(S16/M006)"` tooltip — no accidental writes.

The key lives in **React state only** (D024). No `localStorage`, no
`sessionStorage`, no URL parameter, no build-time injection — refresh
clears it on purpose. The trade-off is friction; the gain is no
persistent secret surface accessible to XSS, DevTools, or browser
history. Use a password manager's auto-fill to soften the re-entry
friction.

**Step 2 — create a subscription**:

Fill the form (webhook URL, optional category / to_addr, optional rate
threshold). Click **Create subscription**. The `signing_secret` is
revealed *once* in a modal — copy it into your webhook receiver's env
or vault now, because the server can never reveal it again.

**Step 3 — rotate or deactivate from the table**:

Each row has Rotate / Deactivate actions. Rotate fires a fresh secret
in the same one-time modal; flip your receiver to the new secret
*before* clicking, since the dispatcher signs with the new value
immediately. Deactivate is a soft-delete — `alert_delivery` history is
preserved for audit.

**Step 4 — recover from 401**:

If the server returns 401 (e.g. you cleared the key, or the value was
wrong), the page shows a red banner: "Unauthorized — enter or re-enter
your admin API key in the panel above…". Click **Clear**, paste the
correct key, **Apply**, and retry. Page state for the form is preserved,
so you don't lose what you typed.

**Why a session-only key on the frontend (D024)**:

- `localStorage` / `sessionStorage` are *XSS targets* — a content-injected
  script would lift the key on any visit.
- `NEXT_PUBLIC_*` / `VITE_*` build-time env vars get baked into the
  bundle; source maps and DevTools expose them.
- URL params end up in browser history and proxy logs.

A React state slot loses to XSS only if the attacker is already on the
page during use — same window of exposure as the server-side env-only
key model on the backend (D023). Refresh = lose key is the operational
signal that bridges those two halves.

---

## Why no `npm install` / `pip install`?

The example clients are intentionally *stdlib-only* (design decision
D017). Copy `client.ts` (or `client.py`) into your project and it works —
no transitive dependencies, no version churn, no semver-of-our-own to
manage. The "install" is a `cp`. PyPI and npm packaging is a separate
future slice.

## M004 in one paragraph

`/v1/failed-tx/{tx_hash}` answers **four** questions in a single round-trip:
*did it fail* (`failed`), *where did the revert originate* (`root_cause`,
S10), *what function was called* (`failing_function_decoded`, S11), and
*why did it happen + what should I try next* (`diagnosis`, S12). That
depth of per-transaction diagnosis isn't in Dune's query model: there's no
`trace.error` in the public datasets, no consumer-specific ABI seeds, and
no curated category-level recommendations. The moat the rest of M001~M004
builds toward sits in this one response.
