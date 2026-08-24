/**
 * TypeScript mirror of the REST API contract.
 *
 * These types mirror the serialized shapes of `crates/db/src/models.rs`
 * (view-backed models) and the response envelopes in
 * `crates/api/src/response.rs` / `crates/api/src/error.rs`. The backend is
 * the source of truth — keep this file in sync if the API changes.
 *
 * `BigDecimal` values are expected as decimal strings on the wire (serde), but
 * the UI tolerates either string/number and normalizes through `api/contract`.
 * Display formatting then coerces through `toNumber()` where approximation is
 * acceptable.
 */

export type Decimal = number | string;
/** RFC 3339 / ISO 8601 timestamp, e.g. `2023-09-01T00:00:00Z`. */
export type IsoDateTime = string;
/** Calendar date, e.g. `2023-09-01`. */
export type IsoDate = string;

/** `FailedTransaction.error_category` — SCREAMING_SNAKE_CASE on the wire. */
// S12.1 — 4 신규 세부 카테고리(`SLIPPAGE_AMOUNT_OUT` / `SLIPPAGE_AMOUNT_IN` /
// `SLIPPAGE_PRICE_IMPACT` / `INSUFFICIENT_ALLOWANCE`)가 fallback (SLIPPAGE_EXCEEDED /
// INSUFFICIENT_BALANCE) 위에 가산. classifier는 우선 세부 매칭, 미매칭 시 fallback.
export type ErrorCategory =
  | "INSUFFICIENT_BALANCE"
  | "INSUFFICIENT_ALLOWANCE"
  | "SLIPPAGE_EXCEEDED"
  | "SLIPPAGE_AMOUNT_OUT"
  | "SLIPPAGE_AMOUNT_IN"
  | "SLIPPAGE_PRICE_IMPACT"
  | "DEADLINE_EXPIRED"
  | "UNAUTHORIZED"
  | "TRANSFER_FAILED"
  | "UNKNOWN";

export const ERROR_CATEGORIES: ErrorCategory[] = [
  "INSUFFICIENT_BALANCE",
  "INSUFFICIENT_ALLOWANCE",
  "SLIPPAGE_EXCEEDED",
  "SLIPPAGE_AMOUNT_OUT",
  "SLIPPAGE_AMOUNT_IN",
  "SLIPPAGE_PRICE_IMPACT",
  "DEADLINE_EXPIRED",
  "UNAUTHORIZED",
  "TRANSFER_FAILED",
  "UNKNOWN",
];

// ── Table models ────────────────────────────────────────────────

export interface Block {
  block_number: number;
  timestamp: IsoDateTime;
  gas_used: number;
}

export interface Token {
  token_address: string;
  symbol: string;
  name: string;
  decimals: number;
}

export interface Pool {
  pool_address: string;
  pair_name: string;
  token0_address: string;
  token1_address: string;
  fee_tier: number;
  created_at: IsoDateTime;
}

export interface SwapEvent {
  pool_address: string;
  tx_hash: string;
  sender: string;
  recipient: string;
  amount0: Decimal;
  amount1: Decimal;
  amount_in: Decimal;
  amount_out: Decimal;
  sqrt_price_x96: Decimal;
  liquidity: Decimal;
  tick: number;
  log_index: number;
  timestamp: IsoDateTime;
  event_id: number;
}

// ── View-backed models (API response shapes) ────────────────────

export interface DailySwapVolume {
  pool_address: string;
  pair_name: string;
  swap_date: IsoDate;
  swap_count: number;
  total_amount_in: Decimal;
  total_amount_out: Decimal;
}

export interface TopTrader {
  user_address: string;
  label: string | null;
  total_swaps: number;
  total_volume_usd: Decimal;
  volume_rank: number;
}

export interface FailedTxAnalysis {
  error_category: ErrorCategory;
  failure_count: number;
  avg_gas_wasted: Decimal;
  pct_of_total: Decimal;
  most_recent_failure: IsoDateTime;
}

// ── Failure-intelligence: per-tx detail / filtered list / timeseries (M001) ─

/** Single `failed_transaction` row — `crates/db/src/models.rs`. */
export interface FailedTransaction {
  tx_hash: string;
  error_category: ErrorCategory;
  revert_reason: string | null;
  failing_function: string | null;
  gas_used: number;
  timestamp: IsoDateTime;
}

/**
 * One flattened call-tree frame (`trace_log` row). The frames are returned in
 * pre-order DFS = strictly ascending `trace_id` (the order they were inserted).
 */
export interface TraceLog {
  tx_hash: string;
  call_depth: number;
  call_type: string;
  from_addr: string;
  to_addr: string | null;
  value: Decimal;
  gas_used: number;
  input: string | null;
  output: string | null;
  error: string | null;
  trace_id: number;
}

/**
 * S11 / M004 — 4-byte selector resolved against the self-owned `function_signature`
 * ABI seed into a human-readable function name/signature. ABI args decoding is a
 * separate slice (S11.1 sketch, D015).
 */
export interface DecodedArg {
  /** Solidity type string verbatim from the signature (e.g. `address`, `uint256`,
   *  or `(address,uint24,uint256)` for a nested tuple). */
  type: string;
  /** JSON-lowered value — `address` / `bytes` → `0x…` hex, `uint*`/`int*` →
   *  decimal **string** (precision-safe), `bool` → boolean, tuple/array →
   *  nested JSON array. See docs/api-failed-tx.md#abi-args-decoding-s111. */
  value: unknown;
}

export interface DecodedFunction {
  /** Lowercased `0x` + 8 hex (matches `data.failed.failing_function`). */
  selector: string;
  /** Function name, e.g. `transfer`. */
  name: string;
  /** ABI signature, e.g. `transfer(address,uint256)`. */
  signature: string;
  /** Seed origin (`erc20` | `uniswap-v3-router` | …); `null` when not tagged. */
  source: string | null;
  /**
   * S11.1 — typed argument values decoded from the call's input bytes.
   * `null` is explicit (D027): either decoding was *not attempted* (no input,
   * input shorter than 4 bytes) or it *failed* (length mismatch, malformed
   * dynamic offsets, …). The surrounding object stays populated — name +
   * signature is still useful diagnostic data on an args miss.
   */
  args: DecodedArg[] | null;
}

/**
 * S12 / M004 — category-level diagnosis seeded for each ErrorCategory variant.
 * `error_category` itself isn't repeated here — the response context already
 * carries it under `data.failed.error_category`. Enum subdivision is a
 * separate slice (S12.1 sketch, D016).
 */
export interface Diagnosis {
  /** Human-readable explanation: *why* the transaction failed. */
  message: string;
  /** Suggested next step (e.g. "Increase slippage tolerance."); `null` when not seeded. */
  recommended_action: string | null;
  /** Seed origin (e.g. `builtin`); `null` when not tagged. */
  source: string | null;
}

/** `GET /v1/failed-tx/{tx_hash}` payload (S01 + S04 truncation, S10 root_cause, S11 decoded fn). */
export interface FailedTxDetail {
  failed: FailedTransaction;
  call_tree: TraceLog[];
  /** True when `call_tree` hit the response cap; the tail was dropped. */
  call_tree_truncated: boolean;
  /**
   * The first `call_tree` frame whose `error` is non-null — the *origin* of
   * the revert in `trace_id ASC` (= pre-order DFS) order (S10 / M004).
   * `null` is explicit: the indexer recorded no per-frame error for this tx
   * (silent default is intentionally not allowed — design decision D014).
   */
  root_cause: TraceLog | null;
  /**
   * `data.failed.failing_function` (4-byte selector) resolved against the
   * self-owned `function_signature` ABI seed (S11 / M004). `null` is explicit
   * — either the selector itself was `null`, or no seed row matched.
   * S11.1: `DecodedFunction.args` carries the typed arg values (null on
   * decode miss; the object itself stays populated — D027).
   */
  failing_function_decoded: DecodedFunction | null;
  /**
   * S11.1 — the same shape as `failing_function_decoded`, but keyed on
   * `root_cause.input` instead of the top-level call. Useful when the
   * revert originated in a sub-call whose function differs from the
   * transaction's outer signature (e.g. an outer `swap` whose nested
   * `transfer` reverted). `null` covers three cases: `root_cause` is
   * `null`, `root_cause.input` is `null`, or the selector isn't in the seed.
   * The `selector` field always equals the first 4 bytes of
   * `root_cause.input` (lowercased) when non-null.
   */
  root_cause_decoded: DecodedFunction | null;
  /**
   * `data.failed.error_category` resolved against the `category_diagnosis`
   * seed (S12 / M004) — message + recommended_action for the dApp developer.
   * `null` is explicit (silent default forbidden, D014 / D016): all six current
   * ErrorCategory variants ship seeded, so non-null whenever the category
   * matches a seed row.
   */
  diagnosis: Diagnosis | null;
}

/** One bucket of the failure timeseries (`failed_tx_timeseries`, S03). */
export interface FailedTxTrendPoint {
  bucket: IsoDateTime;
  error_category: ErrorCategory;
  failure_count: number;
}

/** Allowed bucket sizes for the failure timeseries (`date_trunc` whitelist). */
export type TimeBucket = "hour" | "day" | "week";

export const TIME_BUCKETS: TimeBucket[] = ["hour", "day", "week"];

/**
 * `/v1/analytics/failed-tx/by-label` — failure distribution per labeled
 * contract (S09 / M003 "on-chain × off-chain join" demo). The aggregator
 * pivots category counts into `by_category` so consumers can render a
 * compact distribution without re-aggregating. Pivot invariant:
 * `sum(Object.values(by_category)) === total_failures`.
 */
export interface FailedTxByLabelPoint {
  label: string;
  /** Lowercased 0x + 40 hex; matches `transaction.to_addr`. */
  address: string;
  total_failures: number;
  /** `{ "SLIPPAGE_EXCEEDED": 3, "UNKNOWN": 1, ... }` — per-category counts. */
  by_category: Record<string, number>;
}

// ── Alert subscriptions (S08 + HARDEN2) ─────────────────────────────

/** `per_event` (S08, 1-매칭=1-웹훅) | `rate_threshold` (S14/M005, 윈도우 임계). */
export type AlertSubType = "per_event" | "rate_threshold";

/**
 * `/v1/alert-subscriptions` list/get row. The backend serde-skips
 * `signing_secret` here (`#[serde(skip_serializing)]` on the model), so this
 * type **intentionally omits it** — the secret is only present on
 * `AlertSubscriptionCreated` (one-time reveal on POST and rotate).
 */
export interface AlertSubscription {
  subscription_id: number;
  /** Match category; `null` = all categories. */
  error_category: ErrorCategory | null;
  /** Match contract address (lowercased); `null` = all addresses. */
  to_addr: string | null;
  webhook_url: string;
  active: boolean;
  created_at: IsoDateTime;
  /** S14/M005 — sub mode. Default `per_event`. */
  sub_type: AlertSubType;
  /** rate_threshold required (per_event always `null`). Match count in window. */
  threshold_count: number | null;
  /** rate_threshold required: window length in seconds. */
  threshold_window_secs: number | null;
  /** rate_threshold required: silence period after a delivery, seconds. */
  debounce_secs: number | null;
}

/**
 * `POST /v1/alert-subscriptions` and `POST .../rotate-secret` response. Adds
 * `signing_secret` to the regular subscription shape — **revealed exactly
 * once**, never returned afterwards. The UI must surface it in a copy modal
 * and then drop the value from memory; do **not** persist it in any cache.
 */
export interface AlertSubscriptionCreated {
  subscription_id: number;
  error_category: ErrorCategory | null;
  to_addr: string | null;
  webhook_url: string;
  /**
   * 64-character hex. The dispatcher hex-decodes it to 32 bytes and uses
   * those as the HMAC-SHA256 key (HARDEN2-T02). Receivers must do the same
   * — see `docs/api-alerts.md`.
   */
  signing_secret: string;
  active: boolean;
  created_at: IsoDateTime;
  sub_type: AlertSubType;
  threshold_count: number | null;
  threshold_window_secs: number | null;
  debounce_secs: number | null;
}

/** `POST /v1/alert-subscriptions` request body. */
export interface CreateAlertSubscriptionBody {
  webhook_url: string;
  error_category?: ErrorCategory;
  to_addr?: string;
  /** Optional — default `per_event` if omitted (S14/M005). */
  sub_type?: AlertSubType;
  threshold_count?: number;
  threshold_window_secs?: number;
  debounce_secs?: number;
}

export interface PoolStats {
  pair_name: string;
  swap_count: number;
  unique_traders: number;
  total_volume_in: Decimal;
  avg_trade_size: Decimal;
  liquidity_events: number;
  estimated_fees: Decimal;
}

// ── Response envelopes ──────────────────────────────────────────

/** `crates/api/src/response.rs` — single-resource wrapper. */
export interface ApiResponse<T> {
  data: T;
}

export interface PaginationInfo {
  limit: number;
  offset: number;
  /** Count of rows in this page, not total rows across all pages. */
  count: number;
}

/** `crates/api/src/response.rs` — paginated list wrapper. */
export interface PaginatedResponse<T> {
  data: T[];
  pagination: PaginationInfo;
}

/**
 * Pagination meta WITH a filter-adjusted total (D005). New endpoints
 * (`GET /v1/failed-tx`) use this so embed consumers can show "N of TOTAL".
 */
export interface PaginationMeta {
  limit: number;
  offset: number;
  /** Count of rows in this page (may be < `limit` on the last page). */
  count: number;
  /** Total rows across all pages, after filters. */
  total: number;
}

/** `crates/api/src/response.rs` — paginated list wrapper WITH total (D005). */
export interface TotalPaginatedResponse<T> {
  data: T[];
  pagination: PaginationMeta;
}

/** `crates/api/src/error.rs` — error body is `{ "error": string }`. */
export interface ApiErrorBody {
  error: string;
}
