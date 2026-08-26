# amarillo-tui

Terminal dashboard for the **amarillo Failure Intelligence API** — the same
failure-intelligence the web dashboard shows, in your terminal.

It is a pure **REST API client** (via `reqwest`): it never touches Postgres, so
it only needs `AMARILLO_API_URL`. Point it at a local server or a deployed
amarillo instance.

![amarillo TUI — Overview screen](../../docs/screenshots/tui-overview.png)

## Screens

- **Overview** — KPI cards (latest block, total failed tx, top category) + a
  horizontal bar distribution of failures per error category
  (`GET /v1/analytics/failed-tx`).
- **Failed Tx** — filterable/paginated table (`GET /v1/failed-tx`). Filter by
  category and time window; page through results.
- **Detail** — drill into one transaction (`GET /v1/failed-tx/{tx_hash}`): the
  call tree with the **root-cause frame highlighted**, decoded function +
  typed args, and the human diagnosis (why it failed + how to fix).
- **Clusters** — UNKNOWN-revert clusters
  (`GET /v1/analytics/failed-tx/unknown-clusters`): failures the classifier
  could not categorize, grouped by normalized revert-reason template and ranked
  by occurrences / wasted gas. Columns: template, kind
  (`NO_DATA | CUSTOM_ERROR | PANIC | TEXT`), count, % of UNKNOWN, total/avg gas
  wasted, last seen. The top rows are the next classifier-rule candidates.

## Keys

| Key | Action |
|-----|--------|
| `Tab` / `Shift+Tab`, `1`/`2`/`3`/`4` | switch screen |
| `j`/`k`, `↑`/`↓` | move selection (Detail: scroll) |
| `Enter` | open detail for the selected row |
| `Esc` | back / close help |
| `c` / `t` | cycle category / time-window filter |
| `n` / `b`, `PgDn`/`PgUp` | next / previous page |
| `r` | refresh now |
| `p` | toggle auto-poll |
| `?` | help overlay |
| `q` / `Ctrl+C` | quit |

## Configuration (env)

| Var | Default | Meaning |
|-----|---------|---------|
| `AMARILLO_API_URL` | `http://127.0.0.1:3000` | API base URL |
| `AMARILLO_ADMIN_API_KEY` | — | Bearer key (unused by read-only MVP; wired for future writes) |
| `AMARILLO_TUI_REFRESH_SECS` | `10` | auto-poll interval |
| `AMARILLO_TUI_TIMEOUT_SECS` | `15` | per-request HTTP timeout |
| `AMARILLO_TUI_LOG_DIR` | `.` | directory for `amarillo-tui.log` |
| `RUST_LOG` | `info` | log filter |

> Logs go to `amarillo-tui.log`, **never stdout/stderr** — the TUI owns the
> terminal. Tail that file while debugging.

## Run

```bash
# 1. API server must be up (which needs Postgres + migrations + data)
cargo run -p api

# 2. In another terminal
AMARILLO_API_URL=http://127.0.0.1:3000 cargo run -p tui
```

## Architecture

Single async `tokio::select!` loop (`app::run`) over three sources — terminal
input (`crossterm::EventStream`), a refresh tick (`tokio::time::interval`), and
fetched data (`mpsc` channel). HTTP calls run in spawned tasks so the render
loop never blocks; each screen tracks an `Idle | Loading | Loaded | Failed`
state for non-blocking spinners and error banners.

Module map: `config` · `error` · `dto` (wire mirror) · `client` (reqwest) ·
`format` (web parity) · `terminal` (setup + panic hook) · `event` (key → action)
· `app` (state machine + loop) · `ui/{overview,failed_tx,detail,clusters}`.
