"""
Amarillo — minimal Python example client.

Self-contained: depends only on the Python 3.9+ standard library
(``urllib.request``, ``json``, ``hmac``, ``hashlib``, ``dataclasses``,
``typing``). No ``pip install`` required. This is intentionally an
*example* client (design decision D017) — PyPI packaging is a separate
future slice.

Wire types and endpoint paths mirror ``crates/api/src/routes/*.rs`` and
``crates/db/src/models.rs``. The response contract is additive (D004 / D014):
new fields don't break existing readers, ``None`` always means "absent /
unseeded" (never "the backend forgot the field").
"""
import hashlib
import hmac
import json
import urllib.error
import urllib.parse
import urllib.request
from dataclasses import dataclass
from typing import Any, Dict, List, Optional

# ── Wire types ────────────────────────────────────────────────────────

# `failed_transaction.error_category` — SCREAMING_SNAKE_CASE on the wire.
# S12.1 adds four additive sub-categories next to the original six. The
# generics (SLIPPAGE_EXCEEDED / INSUFFICIENT_BALANCE) stay as fallbacks so
# historical data classified before the subdivision survives intact.
# Allowed values (10 total):
#   "INSUFFICIENT_BALANCE", "INSUFFICIENT_ALLOWANCE",
#   "SLIPPAGE_EXCEEDED", "SLIPPAGE_AMOUNT_OUT", "SLIPPAGE_AMOUNT_IN",
#   "SLIPPAGE_PRICE_IMPACT",
#   "DEADLINE_EXPIRED", "UNAUTHORIZED", "TRANSFER_FAILED", "UNKNOWN"
ErrorCategory = str


@dataclass(frozen=True)
class FailedTransaction:
    """Single ``failed_transaction`` row."""

    tx_hash: str
    error_category: ErrorCategory
    revert_reason: Optional[str]
    failing_function: Optional[str]
    gas_used: int
    timestamp: str

    @classmethod
    def from_dict(cls, d: dict) -> "FailedTransaction":
        return cls(
            tx_hash=d["tx_hash"],
            error_category=d["error_category"],
            revert_reason=d["revert_reason"],
            failing_function=d["failing_function"],
            gas_used=d["gas_used"],
            timestamp=d["timestamp"],
        )


@dataclass(frozen=True)
class TraceLog:
    """Flattened ``trace_log`` frame; pre-order DFS by ``trace_id``."""

    tx_hash: str
    call_depth: int
    call_type: str
    from_addr: str
    to_addr: Optional[str]
    value: str
    gas_used: int
    input: Optional[str]
    output: Optional[str]
    error: Optional[str]
    trace_id: int

    @classmethod
    def from_dict(cls, d: dict) -> "TraceLog":
        return cls(
            tx_hash=d["tx_hash"],
            call_depth=d["call_depth"],
            call_type=d["call_type"],
            from_addr=d["from_addr"],
            to_addr=d["to_addr"],
            value=d["value"],
            gas_used=d["gas_used"],
            input=d["input"],
            output=d["output"],
            error=d["error"],
            trace_id=d["trace_id"],
        )


@dataclass(frozen=True)
class DecodedArg:
    """S11.1 — one decoded argument: Solidity type + JSON-lowered value.

    ``value`` carries whatever JSON sent us — ``str`` for addresses /
    decimal-encoded integers / ``0x`` hex bytes, ``bool`` for booleans,
    ``list`` for tuples and arrays. The caller branches on ``type``.
    """

    type: str
    value: Any

    @classmethod
    def from_dict(cls, d: dict) -> "DecodedArg":
        return cls(type=d["type"], value=d["value"])


@dataclass(frozen=True)
class DecodedFunction:
    """S11 — 4-byte selector resolved against the self-owned ABI seed.

    S11.1: ``args`` carries the typed parameter values decoded from the
    call's input bytes. ``None`` is explicit — either decoding wasn't
    attempted (no input, input shorter than 4 bytes) or it failed (length
    mismatch, malformed dynamic offsets). The surrounding object stays
    populated; ``name`` + ``signature`` is still useful diagnostic data on
    an args miss (D027).
    """

    selector: str
    name: str
    signature: str
    source: Optional[str]
    args: Optional[List[DecodedArg]]

    @classmethod
    def from_dict(cls, d: dict) -> "DecodedFunction":
        raw_args = d["args"]
        return cls(
            selector=d["selector"],
            name=d["name"],
            signature=d["signature"],
            source=d["source"],
            args=(
                [DecodedArg.from_dict(a) for a in raw_args]
                if raw_args is not None
                else None
            ),
        )


@dataclass(frozen=True)
class Diagnosis:
    """S12 — category-level diagnosis: message + recommended_action."""

    message: str
    recommended_action: Optional[str]
    source: Optional[str]

    @classmethod
    def from_dict(cls, d: dict) -> "Diagnosis":
        return cls(
            message=d["message"],
            recommended_action=d["recommended_action"],
            source=d["source"],
        )


@dataclass(frozen=True)
class FailedTxDetail:
    """``GET /v1/failed-tx/{tx_hash}`` payload (S10 + S11 + S11.1 + S12 cumulative)."""

    failed: FailedTransaction
    call_tree: List[TraceLog]
    call_tree_truncated: bool
    root_cause: Optional[TraceLog]
    failing_function_decoded: Optional[DecodedFunction]
    root_cause_decoded: Optional[DecodedFunction]
    diagnosis: Optional[Diagnosis]

    @classmethod
    def from_dict(cls, d: dict) -> "FailedTxDetail":
        return cls(
            failed=FailedTransaction.from_dict(d["failed"]),
            call_tree=[TraceLog.from_dict(t) for t in d["call_tree"]],
            call_tree_truncated=d["call_tree_truncated"],
            root_cause=(
                TraceLog.from_dict(d["root_cause"])
                if d["root_cause"] is not None
                else None
            ),
            failing_function_decoded=(
                DecodedFunction.from_dict(d["failing_function_decoded"])
                if d["failing_function_decoded"] is not None
                else None
            ),
            root_cause_decoded=(
                DecodedFunction.from_dict(d["root_cause_decoded"])
                if d["root_cause_decoded"] is not None
                else None
            ),
            diagnosis=(
                Diagnosis.from_dict(d["diagnosis"])
                if d["diagnosis"] is not None
                else None
            ),
        )


@dataclass(frozen=True)
class FailedTxByLabelPoint:
    """One row of ``GET /v1/analytics/failed-tx/by-label`` (S09)."""

    label: str
    address: str
    total_failures: int
    by_category: Dict[str, int]

    @classmethod
    def from_dict(cls, d: dict) -> "FailedTxByLabelPoint":
        return cls(
            label=d["label"],
            address=d["address"],
            total_failures=d["total_failures"],
            by_category=dict(d["by_category"]),
        )


@dataclass(frozen=True)
class ContractLabel:
    """S15 / M005 — contract label row (admin endpoints)."""

    address: str
    label: str
    owner_id: Optional[str]
    created_at: str

    @classmethod
    def from_dict(cls, d: dict) -> "ContractLabel":
        return cls(
            address=d["address"],
            label=d["label"],
            owner_id=d["owner_id"],
            created_at=d["created_at"],
        )


@dataclass(frozen=True)
class AlertSubscription:
    """``GET /v1/alert-subscriptions`` row; never carries ``signing_secret``."""

    subscription_id: int
    error_category: Optional[ErrorCategory]
    to_addr: Optional[str]
    webhook_url: str
    active: bool
    created_at: str

    @classmethod
    def from_dict(cls, d: dict) -> "AlertSubscription":
        return cls(
            subscription_id=d["subscription_id"],
            error_category=d["error_category"],
            to_addr=d["to_addr"],
            webhook_url=d["webhook_url"],
            active=d["active"],
            created_at=d["created_at"],
        )


@dataclass(frozen=True)
class AlertSubscriptionCreated:
    """``POST /v1/alert-subscriptions`` response — ``signing_secret`` once."""

    subscription_id: int
    error_category: Optional[ErrorCategory]
    to_addr: Optional[str]
    webhook_url: str
    signing_secret: str
    active: bool
    created_at: str

    @classmethod
    def from_dict(cls, d: dict) -> "AlertSubscriptionCreated":
        return cls(
            subscription_id=d["subscription_id"],
            error_category=d["error_category"],
            to_addr=d["to_addr"],
            webhook_url=d["webhook_url"],
            signing_secret=d["signing_secret"],
            active=d["active"],
            created_at=d["created_at"],
        )


# ── HTTP error ────────────────────────────────────────────────────────


class AmarilloError(Exception):
    """HTTP error from the Amarillo API. ``status`` is the HTTP code."""

    def __init__(self, status: int, message: str) -> None:
        super().__init__(message)
        self.status = status


# ── Client ────────────────────────────────────────────────────────────


class AmarilloClient:
    """Drop-in client for Amarillo.

    Read-only embeds:

        client = AmarilloClient("http://localhost:3000")
        client.get_failed_tx("0xabc…")

    Write/admin calls (S16/M006) require ``api_key``:

        client = AmarilloClient("http://localhost:3000", api_key=os.environ["AMARILLO_ADMIN_API_KEY"])
        client.create_contract_label(address, label, owner_id="…")

    Calling a write method without ``api_key`` set raises ``ValueError``
    locally — surface the operator's mistake before sending an unsigned
    request that the server would just 401 (S17/D021).
    """

    def __init__(self, base_url: str, api_key: Optional[str] = None) -> None:
        self.base_url = base_url.rstrip("/")
        self.api_key = api_key

    def _request(
        self,
        method: str,
        path: str,
        body: Optional[dict] = None,
        *,
        auth: bool = False,
    ) -> Any:
        data: Optional[bytes] = None
        headers: Dict[str, str] = {}
        if body is not None:
            data = json.dumps(body).encode("utf-8")
            headers["Content-Type"] = "application/json"
        if auth:
            if not self.api_key:
                raise ValueError(
                    "missing API key: this call requires `api_key` on AmarilloClient (S16/M006)"
                )
            headers["Authorization"] = f"Bearer {self.api_key}"
        req = urllib.request.Request(
            f"{self.base_url}{path}",
            data=data,
            method=method,
            headers=headers,
        )
        try:
            with urllib.request.urlopen(req, timeout=30) as resp:
                if resp.status == 204:
                    return None
                text = resp.read().decode("utf-8")
                return json.loads(text) if text else None
        except urllib.error.HTTPError as err:
            body_text = err.read().decode("utf-8", errors="replace")
            try:
                msg = json.loads(body_text).get("error", body_text)
            except json.JSONDecodeError:
                msg = body_text
            raise AmarilloError(err.code, msg) from err

    def get_failed_tx(self, tx_hash: str) -> FailedTxDetail:
        """``GET /v1/failed-tx/{tx_hash}`` — single-tx diagnosis."""
        r = self._request("GET", f"/v1/failed-tx/{tx_hash}")
        return FailedTxDetail.from_dict(r["data"])

    def list_failed_tx(
        self,
        category: Optional[ErrorCategory] = None,
        from_ts: Optional[str] = None,
        to_ts: Optional[str] = None,
        limit: Optional[int] = None,
        offset: Optional[int] = None,
    ) -> dict:
        """``GET /v1/failed-tx`` — filtered list with accurate ``total``."""
        params: Dict[str, Any] = {}
        if category is not None:
            params["category"] = category
        if from_ts is not None:
            params["from"] = from_ts
        if to_ts is not None:
            params["to"] = to_ts
        if limit is not None:
            params["limit"] = limit
        if offset is not None:
            params["offset"] = offset
        return self._request("GET", "/v1/failed-tx?" + urllib.parse.urlencode(params))

    def get_failed_tx_timeseries(
        self,
        interval: Optional[str] = None,
        from_ts: Optional[str] = None,
        to_ts: Optional[str] = None,
    ) -> list:
        """``GET /v1/analytics/failed-tx/timeseries`` — bucketed trend."""
        params: Dict[str, Any] = {}
        if interval is not None:
            params["interval"] = interval
        if from_ts is not None:
            params["from"] = from_ts
        if to_ts is not None:
            params["to"] = to_ts
        r = self._request(
            "GET",
            "/v1/analytics/failed-tx/timeseries?" + urllib.parse.urlencode(params),
        )
        return r["data"]

    def get_failed_tx_by_label(
        self,
        from_ts: Optional[str] = None,
        to_ts: Optional[str] = None,
        owner: Optional[str] = None,
        limit: Optional[int] = None,
    ) -> List[FailedTxByLabelPoint]:
        """``GET /v1/analytics/failed-tx/by-label`` — failures by labeled contract."""
        params: Dict[str, Any] = {}
        if from_ts is not None:
            params["from"] = from_ts
        if to_ts is not None:
            params["to"] = to_ts
        if owner is not None:
            params["owner"] = owner
        if limit is not None:
            params["limit"] = limit
        r = self._request(
            "GET",
            "/v1/analytics/failed-tx/by-label?" + urllib.parse.urlencode(params),
        )
        return [FailedTxByLabelPoint.from_dict(p) for p in r["data"]]

    def create_alert_subscription(
        self,
        webhook_url: str,
        error_category: Optional[ErrorCategory] = None,
        to_addr: Optional[str] = None,
    ) -> AlertSubscriptionCreated:
        """``POST /v1/alert-subscriptions`` — ``signing_secret`` revealed exactly once.

        Requires ``api_key`` set on the client (S16/M006).
        """
        body: dict = {"webhook_url": webhook_url}
        if error_category is not None:
            body["error_category"] = error_category
        if to_addr is not None:
            body["to_addr"] = to_addr
        r = self._request("POST", "/v1/alert-subscriptions", body, auth=True)
        return AlertSubscriptionCreated.from_dict(r["data"])

    def list_alert_subscriptions(self) -> List[AlertSubscription]:
        """``GET /v1/alert-subscriptions`` — never returns ``signing_secret``. Public (no api_key)."""
        r = self._request("GET", "/v1/alert-subscriptions")
        return [AlertSubscription.from_dict(s) for s in r["data"]]

    def delete_alert_subscription(self, sub_id: int) -> None:
        """``DELETE /v1/alert-subscriptions/{id}`` — soft-deactivate.

        Requires ``api_key`` set on the client (S16/M006).
        """
        self._request("DELETE", f"/v1/alert-subscriptions/{sub_id}", auth=True)

    def rotate_alert_secret(self, sub_id: int) -> AlertSubscriptionCreated:
        """``POST /v1/alert-subscriptions/{id}/rotate-secret`` — same one-time secret contract.

        Requires ``api_key`` set on the client (S16/M006).
        """
        r = self._request(
            "POST", f"/v1/alert-subscriptions/{sub_id}/rotate-secret", auth=True
        )
        return AlertSubscriptionCreated.from_dict(r["data"])

    def create_contract_label(
        self,
        address: str,
        label: str,
        owner_id: Optional[str] = None,
    ) -> ContractLabel:
        """``POST /v1/contract-labels`` — admin UPSERT (S15 / M005).

        Returns the row with ``address`` lowercased server-side. Calling twice
        with the same address overwrites ``label`` / ``owner_id``.

        Requires ``api_key`` set on the client (S16/M006) — ``Authorization: Bearer``
        header is attached automatically. Calling without ``api_key`` raises
        ``ValueError`` before the request leaves the client.
        """
        body: dict = {"address": address, "label": label}
        if owner_id is not None:
            body["owner_id"] = owner_id
        r = self._request("POST", "/v1/contract-labels", body, auth=True)
        return ContractLabel.from_dict(r["data"])

    def delete_contract_label(self, address: str) -> None:
        """``DELETE /v1/contract-labels/{address}`` — admin (S15 / M005).

        Raises ``AmarilloError(404)`` if the address is missing. Idempotency:
        a second delete re-raises 404, which operators treat as a no-op
        signal on retry. Requires ``api_key`` set on the client (S16/M006).
        """
        from urllib.parse import quote
        self._request(
            "DELETE", f"/v1/contract-labels/{quote(address, safe='')}", auth=True
        )


# ── Webhook signature verification ────────────────────────────────────


def verify_alert_signature(
    raw_body: bytes,
    signature_header: Optional[str],
    signing_secret: str,
) -> bool:
    """
    Verifies the ``X-Amarillo-Signature`` header against the raw POST body.

    The dispatcher signs the **raw request bytes** with HMAC-SHA256, keyed by
    the **32 bytes obtained by hex-decoding** ``signing_secret`` (64 hex
    chars). The header value is ``sha256=<hex>``. Mirror of
    ``crates/indexer/src/alerts.rs::sign_payload``.

    Uses ``hmac.compare_digest`` for constant-time comparison.
    """
    if not signature_header or not signature_header.startswith("sha256="):
        return False
    sig_hex = signature_header[len("sha256=") :]
    hex_chars = set("0123456789abcdefABCDEF")
    if not sig_hex or not all(c in hex_chars for c in sig_hex):
        return False
    if len(signing_secret) != 64 or not all(c in hex_chars for c in signing_secret):
        return False
    try:
        key = bytes.fromhex(signing_secret)
        actual = bytes.fromhex(sig_hex)
    except ValueError:
        return False
    expected = hmac.new(key, raw_body, hashlib.sha256).digest()
    if len(actual) != len(expected):
        return False
    return hmac.compare_digest(actual, expected)
