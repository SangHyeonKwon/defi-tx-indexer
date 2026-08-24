# Real-time Follow Mode (S05)

The indexer can **follow the chain head** instead of processing a fixed block
range — M002's first slice.

## Usage

```bash
# fixed range (M001, unchanged)
cargo run -p indexer -- --from-block 18000000 --to-block 18001000

# follow the chain head continuously (Ctrl-C to stop)
cargo run -p indexer -- --follow
cargo run -p indexer -- --follow --poll-interval-secs 6 --confirmations 20

# drive cycles by a newHeads subscription instead of polling (opt-in, D011)
WS_URL=wss://… cargo run -p indexer -- --follow --subscribe
```

| Flag | Default | Meaning |
|------|---------|---------|
| `--follow` | off | Continuous head-follow instead of a fixed range. |
| `--poll-interval-secs` | 12 | Sleep between head polls. |
| `--confirmations` | 12 | Index only up to `head - N` (shallow-reorg cushion). |
| `--subscribe` | off | Drive cycles by a `newHeads` subscription; needs `WS_URL`. Falls back to polling if WS is unset/unavailable (D011). |

`--from-block` is **optional** with `--follow` (resume point comes from the
`indexer_checkpoint`, not the CLI). `--to-block` is ignored with `--follow`.
`WS_URL` is read from the environment (not a CLI flag — it can carry an API
key, so it is **never logged**). `--subscribe` without a usable `WS_URL`
just runs polling (no error, no regression).

## How it works

Loop: fetch chain head (`get_block_number`, retried with backoff) → compute the
next range via the pure `next_target(head, confirmations, checkpoint)` →
`index_range(..)` (reuses the M001 pipeline + per-chunk checkpointing) → sleep →
repeat. `Ctrl-C` finishes the in-flight chunk and stops cleanly.

- **No backfill in follow mode**: with no checkpoint, follow starts at the
  *safe tip* (`head - confirmations`), not genesis. Backfill is the fixed-range
  mode's job; run that first if you need history.
- **Confirmations lag** trades latency for reorg safety (D009).

## Observability (S07-T01)

Every loop iteration emits **one structured `tracing` summary line**
(`"follow cycle summary"`, INFO) with parseable fields — no log scraping
needed, no new dependency (reuses `tracing` + `chrono`):

| Field | Meaning |
|-------|---------|
| `cycle` | Loop iteration count (process-local, from 1). |
| `head` | Chain head from `get_block_number`. |
| `checkpoint` | Last processed block (`-1` if none yet). |
| `lag` | Indexing lag = `head − checkpoint` (0 if no checkpoint). |
| `indexed_this_cycle` | Blocks indexed in this iteration. |
| `blocks_total` | Cumulative blocks indexed since process start. |
| `reorgs_total` | Cumulative reorg count. |
| `last_reorg_depth` | Blocks rolled back in the most recent reorg (0 if none). |
| `last_poll` | UTC timestamp of this cycle. |

A reorg cycle instead emits two WARN lines (`"reorg detected — rolling
back"` then `"rolled back — re-indexing next cycle"`) carrying
`cycle`, `fork`, `depth` (rolled-back block count), `reorgs_total`.

These counters live in process memory only (not persisted) and are
**observation-only**: branch flow, timing, RPC/DB calls are unchanged
(no behavior/perf regression).

## Subscribe trigger (S07-T02, D011)

By default each cycle is woken by `sleep(--poll-interval-secs)`. With
`--subscribe` **and** a usable `WS_URL`, a background task subscribes to
`newHeads` and ticks the loop on every new head instead — latency is no
longer bounded by the poll interval. **Only the trigger changes**: the
reorg check, `next_target`, and `index_range` are byte-for-byte identical.

Pure/testable split (same philosophy as `next_target`/`classify_fork`):
`resolve_trigger_mode(subscribe, ws_url)` decides Polling vs Subscribe
(trims the URL; blank ⇒ Polling) and is unit-tested without WS/RPC. The
live WS task is compile + clippy checked and manually smoked.

Fallback is unconditional (no regression): WS connect failure, subscribe
failure, or stream end closes the tick channel and the loop **reverts to
polling** automatically. The WS URL is **never logged** (it can carry a
secret); only the mode label (`polling`/`subscribe`) appears.

## Reorg detection & correction (S06)

Every poll, *before* indexing, the loop compares the most recent local block
hashes against the chain (`detect_fork`, driving the pure `classify_fork` /
`next_scan_depth`). On a mismatch it calls `rollback_from_block(fork)` —
deletes `>= fork` across block/tx/events/trace/failed in one transaction and
rewinds the checkpoint — then re-indexes on the next iteration.

- **Safety**: if any chain hash is unavailable (RPC error / block absent) the
  check yields *no fork* — never a destructive rollback on uncertain data.
- **Lazy + dynamic widening (S07-T03, resolves review R1/R2)**: the scan
  starts at the tip and fetches chain hashes **on demand**, tip → down. No
  reorg ⇒ the tip hash matches ⇒ **1 RPC** and stop (R2: the old code
  prefetched the whole window every poll). On a mismatch it descends, and if
  the whole loaded range still mismatches it **widens ×4 up to
  `REORG_SCAN_CAP` (4096 blocks)** until it finds the *true* minimum common
  ancestor, then rolls back from exactly there. This **eliminates the earlier
  under-deletion gap** (a reorg deeper than a fixed 64-window used to leave
  stale pre-window blocks). Residual: only a reorg **deeper than 4096 blocks**
  (~64× mainnet PoS finality — effectively impossible) falls back to a
  best-effort floor rollback; that bound is now explicit, not an unstated
  "safe" assumption.

## Limits / scope

- Polling **by default**; `eth_subscribe` (`newHeads`) is opt-in via
  `--subscribe` + `WS_URL`, with automatic polling fallback (S07-T02, D011).
- Reorgs up to `REORG_SCAN_CAP` (4096 blocks) are corrected to the **exact**
  minimum common ancestor (S07-T03). Only a deeper-than-4096 reorg
  (~64× mainnet finality) is a best-effort floor rollback — the bound is now
  explicit. (An earlier draft of this doc / D010 / S06-SUMMARY mislabeled the
  old 64-window behavior as a safe "conservative over-delete"; that was a
  review-R1 honesty fix, and S07-T03 then removed the gap itself.)
- **R3/R4 — REALIZED (HARDEN-T01)**: follow caps a single cycle at
  `FOLLOW_CYCLE_BLOCK_CAP` (= 500 blocks; pure `cap_range_to`) so that
  reorg checks keep running even when the checkpoint is far behind, and
  `index_range` checks a cancellation flag between chunks so Ctrl-C
  during a backfill stops at the next chunk boundary (≤ `batch_size`
  blocks of additional work) with per-chunk checkpoint preserving
  partial progress. Larger ranges naturally resume on subsequent cycles.

## Verification

The follow loop needs a live `RPC_URL`, which CI may not have. So the
**decision logic is verified without RPC**:

```bash
cargo test -p indexer        # next_target + classify_fork + next_scan_depth + resolve_trigger_mode (no RPC/DB/WS)
cargo test -p db -- --ignored # rollback_from_block idempotency (needs Postgres)
```

Live smoke (manual, requires RPC + Postgres):

```bash
export DATABASE_URL=postgres://defi:defi@localhost:5432/defi_analytics
export RPC_URL=<an ethereum rpc endpoint>
cargo run -p indexer -- --follow --poll-interval-secs 6
# expect: "follow mode started" (trigger=polling), periodic "indexing new
#         range" logs, checkpoint advancing; Ctrl-C → "stopping follow loop"

# subscribe trigger (also needs a WS endpoint):
export WS_URL=wss://<an ethereum ws endpoint>
cargo run -p indexer -- --follow --subscribe
# expect: "follow mode started" with trigger=subscribe, then "subscribe
#         mode: newHeads subscription active", cycles ticking on new heads.
#         Unset/unreachable WS → "falling back to polling" (no crash).
```
