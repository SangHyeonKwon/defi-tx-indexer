<div align="center">

# amarillo

**Ethereum Failure Intelligence API**

"이 트랜잭션이 *왜* revert 됐는가" — trace-level 단건 진단.
*무엇*이 아니라 *왜*를 답한다. 실시간, 임베드용.

[![Rust](https://img.shields.io/badge/Rust-stable-f74c00?logo=rust&logoColor=white)](https://www.rust-lang.org/)
[![PostgreSQL](https://img.shields.io/badge/PostgreSQL-16+-336791?logo=postgresql&logoColor=white)](https://www.postgresql.org/)
[![Ethereum](https://img.shields.io/badge/Ethereum-Mainnet-3C3C3D?logo=ethereum&logoColor=white)](https://ethereum.org/)
[![Uniswap](https://img.shields.io/badge/Uniswap-V3-FF007A?logo=uniswap&logoColor=white)](https://uniswap.org/)

[English](README.md) · **한국어**

</div>

---

## What you get

**한 호출에 네 가지 답:** 어디서 revert 났나, 어떤 함수인가, 왜, 어떻게 고치나.

```jsonc
GET /v1/failed-tx/0xdead…0001
{
  "failed":       { "error_category": "SLIPPAGE_AMOUNT_OUT", "revert_reason": "Too little received", ... },
  "root_cause":   { "call_depth": 2, "error": "Too little received", ... },   // ① 어디서
  "failing_function_decoded": { "name": "exactInputSingle", "args": [ ... ] }, // ② 어떤 함수, typed args
  "diagnosis": {                                                               // ③ 왜 + ④ 어떻게
    "message": "Trade output fell below the minimum amount you specified (buy-side slippage).",
    "recommended_action": "Increase amountOutMin tolerance, or split the trade to lower price impact."
  },
  "call_tree": [ /* pre-order DFS, trace_id ASC */ ]
}
```

`null`은 항상 명시적(silent default 거부). 모든 필드가 additive — 클라이언트 무회귀.
전체 응답 형태: [`docs/api-failed-tx.md`](docs/api-failed-tx.md).

## Why it exists

일반 SQL 분석 플랫폼(Dune이 그중 가장 큼)은 *무엇이 일어났나*, *얼마나*를 답한다.
amarillo는 **이 특정 트랜잭션이 왜 revert 됐나** — 그 플랫폼들이 다루도록 만들어지지 않은
trace-level 영역을 공략한다:

| SQL 분석 플랫폼이 못 하는 것 | amarillo가 제공하는 것 |
|------------------------------|------------------------|
| per-frame `trace.error` attribution | `root_cause` + `call_tree` (`debug_traceTransaction` 파싱) |
| Consumer별 ABI 디코딩 | 자기소유 `function_signature` 시드 + `alloy::dyn_abi` 런타임 디코딩 |
| Per-request webhook 전달 | `/v1/alert-subscriptions` outbox 디스패처 + HMAC-SHA256 |
| 실시간 실패 스트림 | `--follow --confirmations N` + 동적 reorg scan window |
| Private-data join | `contract_label.owner_id`, `?owner=` 필터로 분리 |

## API surface

| 엔드포인트 | 반환 | 인증 |
|-----------|------|------|
| `GET /v1/failed-tx/{tx_hash}` | 단건 진단 (root_cause + decoded fn + 왜/고치는법) | 공개 |
| `GET /v1/failed-tx?category=&from=&to=&limit=&offset=` | 필터 목록 + 정확한 `total` | 공개 |
| `GET /v1/analytics/failed-tx` | 카테고리 분포 + 평균 낭비 가스 | 공개 |
| `GET /v1/analytics/failed-tx/timeseries?interval=hour\|day\|week` | 카테고리 × 시간 추이 | 공개 |
| `GET /v1/analytics/failed-tx/by-label?owner=` | 라벨된 컨트랙트별 분포 (owner 분리) | 공개 |
| `GET /v1/analytics/failed-tx/unknown-clusters` | UNKNOWN revert를 정규화 템플릿으로 클러스터링 | 공개 |
| `POST` / `DELETE /v1/contract-labels` | 봇 라벨 admin (UPSERT / DELETE) | Bearer |
| `POST /v1/alert-subscriptions` (+ `/rotate-secret`) | webhook / rate-threshold 알림, 1회성 signing secret | Bearer |

write/admin 엔드포인트는 `Authorization: Bearer ${AMARILLO_ADMIN_API_KEY}` 필요, GET은 공개·임베드용.
헤더 누락 / 형식 오류 / 키 불일치 모두 같은 `401`(정보 노출 없음).

## Quick start

**필수**: Rust stable / PostgreSQL 16+ / docker (선택). RPC 키는 backfill 인덱싱에만 필요 —
데모 시드만 본다면 불요.

```bash
# 1) env — AMARILLO_ADMIN_API_KEY 필수 (없으면 서버 부팅 거부)
cp .env.example .env
echo "AMARILLO_ADMIN_API_KEY=$(openssl rand -hex 32)" >> .env

# 2) 띄우고 데모 데이터 시드
docker compose up -d
docker compose run --rm seed

# 3) 시드된 실패 tx 단건 진단
curl http://localhost:3000/v1/failed-tx/0xdead000000000000000000000000000000000000000000000000000000000001 | jq

# 4) 대시보드
open http://localhost:8080
```

> **메인넷 인덱싱**: `cargo run -p indexer -- --follow --rpc-url <YOUR_RPC>`. 무료 RPC tier로
> 작은 윈도우 backfill은 가능하지만 24/7 follow는 paid plan 필요.

## Architecture

```
이더리움 노드 (RPC / WebSocket)
  → [indexer]  --follow / backfill 워커 풀, depth-aware reorg
  → [decoder]  Uniswap V3 이벤트 + 트레이스 → revert reason, call tree, typed ABI args
  → [decoder::classifier]  revert reason → ErrorCategory (10 변형)
  → [db]       sqlx UNNEST 배치 INSERT → PostgreSQL
  → [api]      axum REST + admin API key 게이트
  → [indexer --dispatch-alerts]  outbox → HMAC-signed webhook (SSRF + DNS guard)
```

| Crate | Type | Role |
|-------|------|------|
| `crates/indexer/` | Binary | 블록 수집 · 오케스트레이션 · follow · 디스패처 |
| `crates/api/` | Binary + Lib | axum REST 서버 (auth + 보호 라우트) |
| `crates/decoder/` | Library | ABI 디코딩 · 트레이스 파싱 · error classifier |
| `crates/db/` | Library | SQLx 모델 · 쿼리 · 마이그레이션 |
| `crates/tui/` | Binary | 터미널 대시보드 (ratatui, REST 클라이언트) |

## Clients & UIs

- **웹 대시보드** — Vite + React 19 + Recharts, `:8080`에서 서빙.
- **터미널 UI** — `cargo run -p tui` (API가 떠 있어야 함). 순수 REST 클라이언트라 배포된 어떤
  인스턴스든 가리킬 수 있음. 상세: [`crates/tui/README.md`](crates/tui/README.md).
- **Drop-in 클라이언트** — `examples/{typescript,python}-client/`, 외부 의존 0. 파일 하나
  복사하면 끝. 모든 `/v1/*` 호출 + `verifyAlertSignature`(HMAC-SHA256) 포함.

End-to-end 시나리오: [`docs/cookbook.md`](docs/cookbook.md). 전체 API + 인증 레퍼런스:
[`docs/api-failed-tx.md`](docs/api-failed-tx.md).

## Scope (deliberate)

Ethereum 메인넷 · Uniswap V3. 깊이(실시간 / 진단 / 정합성)를 먼저 투자하고, 폭(멀티체인·
멀티프로토콜)은 의도적으로 동결. 이건 일반 on-chain 분석 대시보드가 **아니다** — 그 영역은
일반 분석 플랫폼의 몫이다.

## Honest limits

- **RPC 비용이 가장 큰 변수** — 메인넷 `--follow` 24/7은 무료 tier를 며칠 만에 소진. 작은 윈도우
  backfill이나 과거 데이터 시드로 시작.
- **ABI 시드·classifier는 큐레이트/휴리스틱** — 미시드 selector는 `failing_function_decoded: null`,
  revert 분류는 문자열 패턴 매칭(외부 4byte 의존 없음). 둘 다 운영자가 확장 가능.
- **API key는 단일 env 값** — 회전 = env 갱신 + 재시작.
- **검증은 docker compose 시드 데이터 기준** — 라이브 메인넷 자동 회귀는 없음.

## Local dev

```bash
cargo install sqlx-cli --no-default-features --features postgres
cp .env.example .env && sqlx database create && sqlx migrate run

cargo test                                   # 유닛
cargo test -p db -- --ignored                # 통합 (PG 필요)
cargo clippy --workspace --all-targets -- -D warnings && cargo fmt --check

cargo run -p indexer -- --from-block 18000000 --to-block 18001000   # backfill
cargo run -p indexer -- --follow --confirmations 12                 # follow
cargo run -p indexer -- --dispatch-alerts                           # webhook 디스패처
cargo run -p api                                                    # API (:3000)

./scripts/verify-failed-tx.sh          # 공개 GET + 진단 시맨틱
./scripts/verify-alerts.sh             # 알림 CRUD + HMAC + 401 케이스
./scripts/verify-failed-tx-by-label.sh # by-label + admin 엔드포인트
```
