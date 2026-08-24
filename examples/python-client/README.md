# Amarillo — Python example client

Minimal `urllib`-based client for the Amarillo Failure Intelligence API.
**Stdlib only** — no `pip install`, no `requirements.txt`. Python 3.9+.
This is intentionally an *example* client (design decision D017) —
PyPI packaging is a separate future slice.

## What's in it

- `client.py` — `AmarilloClient` covering every `/v1/*` endpoint, the wire
  types as `@dataclass(frozen=True)` (immutable, mirrored from
  `crates/db/src/models.rs`), and `verify_alert_signature(…)` for
  receiving signed webhook deliveries.
- `examples.py` — three runnable scenarios: single-tx diagnosis, alert
  subscription + HMAC verification, failures-by-labeled-contract.

## Use

```bash
# 1. Syntax check (the only "build" needed)
python3 -m py_compile examples/python-client/client.py examples/python-client/examples.py

# 2. Run the demo against your local stack
python3 examples/python-client/examples.py http://localhost:3000
```

The endpoint contracts (response shapes, error envelopes, explicit-null
semantics) are documented in `docs/api-failed-tx.md`,
`docs/api-alerts.md`, and `docs/cookbook.md`.

## Webhook receiver outline (Flask)

```python
from flask import Flask, request, abort
from client import verify_alert_signature

SECRET = "<64-hex-char signing_secret>"  # store in env, not source
app = Flask(__name__)

@app.post("/amarillo-webhook")
def webhook() -> dict:
    if not verify_alert_signature(
        request.get_data(),                            # raw bytes — NOT request.json
        request.headers.get("X-Amarillo-Signature"),
        SECRET,
    ):
        abort(401, description="bad signature")
    payload = request.get_json()                       # safe to parse after verification
    # handle payload …
    return {"ok": True}
```

Use `request.get_data()` (or your framework's equivalent for raw body) —
the dispatcher signs the **raw bytes**, so reading-then-reserializing
JSON would break verification. Same warning as the TypeScript client.
