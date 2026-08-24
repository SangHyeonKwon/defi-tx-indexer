# Amarillo — TypeScript example client

Minimal `fetch`-based client for the Amarillo Failure Intelligence API. **No
npm dependencies** — drop `client.ts` (and optionally `examples.ts`) into
your project and you're done. The only runtime requirement is Node 18+
(needs the global `fetch` and `node:crypto`). This is intentionally an
*example client* (design decision D017) — npm publishing is a separate
future slice.

## What's in it

- `client.ts` — `AmarilloClient` covering every `/v1/*` endpoint, the wire
  types that mirror `crates/db/src/models.rs`, and `verifyAlertSignature(…)`
  for receiving signed webhook deliveries.
- `examples.ts` — three runnable scenarios: single-tx diagnosis, alert
  subscription + HMAC verification, failures-by-labeled-contract.
- `tsconfig.json` — minimal `strict` config for the typecheck.

## Use

```bash
# 1. Typecheck (the only "build" needed)
npx --package=typescript@5 tsc --noEmit -p examples/typescript-client/tsconfig.json

# 2. Run the demo against your local stack
npx --package=tsx tsx examples/typescript-client/examples.ts http://localhost:3000
```

The endpoint contracts (response shapes, error envelopes) are documented at
`docs/api-failed-tx.md`, `docs/api-alerts.md`, and `docs/cookbook.md`. The
explicit-null contract (D014 / D016) — `null` always means "absent /
unseeded", never "the backend forgot the field" — is enforced server-side
and reflected in this client's optional fields.

## Webhook receiver outline

```ts
import express from "express";
import { verifyAlertSignature } from "./client.ts";

const SECRET = process.env.AMARILLO_SIGNING_SECRET!; // 64 hex chars
const app = express();

// Capture the *raw* body so the HMAC verifies. `express.json()` would
// re-encode and break byte-equality.
app.post("/amarillo-webhook", express.raw({ type: "application/json" }), (req, res) => {
  const ok = verifyAlertSignature(
    req.body, // Buffer
    req.header("x-amarillo-signature"),
    SECRET,
  );
  if (!ok) return res.status(401).json({ error: "bad signature" });
  const payload = JSON.parse(req.body.toString("utf8"));
  // handle payload …
  res.status(200).json({ ok: true });
});
```

Use `express.raw()` (or your framework's equivalent) — the dispatcher signs
the **raw bytes**, so `JSON.parse → JSON.stringify` will break the
verification.
