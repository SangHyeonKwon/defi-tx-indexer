# 분석 — UNKNOWN Revert 사유 클러스터링

> classifier가 분류하지 못한(UNKNOWN) 실패의 revert 사유를 **템플릿 단위로
> 자동 클러스터링**해서, 다음에 추가할 classifier 룰/카테고리 후보를
> 데이터에서 뽑아내는 파이프라인. SQL만으로 동작한다 —
> `fn_revert_template` + `vw_unknown_revert_clusters`
> (`sql/views/001_views.sql`).

## 문제

`decoder::classifier::classify_error`는 키워드 substring 매칭 기반
휴리스틱이라, 룰에 없는 revert 사유는 전부 `UNKNOWN`으로 떨어진다.
UNKNOWN 버킷이 커질수록 Failure Intelligence의 핵심 가치(카테고리별
진단·트렌드)가 희석되는데, 지금까지는 **UNKNOWN 안에 뭐가 들어있는지
들여다볼 도구가 없었다**.

## 방법 — 템플릿 정규화 클러스터링

revert 문자열은 사람이 아니라 컨트랙트가 생성하므로 형태가 기계적이다.
가변부(주소·금액·hex 인자)만 치환하면 같은 원인의 실패가 정확히 한
클러스터로 묶인다. 임베딩·ML 없이 정규식 5개로 충분하며, 인덱서
파이프라인에 의존성을 추가하지 않는다.

`fn_revert_template(reason)`의 규칙 (순서 중요):

| 입력 형태 | 템플릿 | 근거 |
|-----------|--------|------|
| `NULL` / `''` / `'0x'` | `(no revert data)` | out-of-gas, bare `revert()` — 출력 자체가 없음 |
| `Panic(0xNN)` | 원문 유지 | `decoder::trace`의 정규형. 코드별 의미가 다름 (0x11 오버플로우, 0x12 0나눗셈, 0x32 배열 범위) |
| `0x` + hex ≥8자 | `custom_error:0x` + 셀렉터 8 hex | 미디코딩 커스텀 에러 — 인자가 달라도 같은 에러면 셀렉터가 같다 |
| `0x` + hex <8자 | `(undecodable output)` | 셀렉터조차 없는 출력 |
| 일반 텍스트 | 주소→`{addr}`, hex→`{hex}`, 숫자 4자리 이상→`{n}`, 공백 정리 | 짧은 숫자는 보존 — `M0`, `51`(Aave), `GS013`은 코드 자체가 식별자 |

`vw_unknown_revert_clusters`는 템플릿별로 빈도(`occurrences`,
`pct_of_unknown`), 비용(`total_gas_wasted`, `avg_gas_wasted`), 영향 범위
(`distinct_senders`, `distinct_selectors`), 대표 샘플과 관측
구간(`first_seen`/`last_seen`)을 랭킹한다. `cluster_kind`(NO_DATA /
CUSTOM_ERROR / PANIC / TEXT)는 클러스터의 "다음 액션"을 결정한다 (아래).

## 검증

실 메인넷에서 흔한 revert 패턴 21종을 빈도 가중치로 합성한 119건
코퍼스(UNKNOWN 105건)를 로컬 PG16에 넣고 뷰를 실행했다. 코퍼스의
카테고리 배정은 `classifier.rs` 룰을 그대로 손추적해 현재 동작과
일치시켰다. 결과 (상위 8개):

| template | kind | occurrences | pct | 검증 포인트 |
|---|---|---|---|---|
| `custom_error:0xa9ad62f8` | CUSTOM_ERROR | 18 | 17.1% | **인자만 다른 hex 18건이 셀렉터 하나로 수렴** |
| `(no revert data)` | NO_DATA | 16 | 15.2% | NULL 실패 격리 |
| `TransferHelper: TRANSFER_FROM_FAILED` | TEXT | 12 | 11.4% | 즉시 룰 추가 가능한 미분류 발견 |
| `Panic(0x11)` | PANIC | 9 | 8.6% | Panic 코드별 구분 유지 |
| `UniswapV2: K` | TEXT | 7 | 6.7% | 짧은 코드 보존 |
| `Received amount ... expected: {n} received: {n}` | TEXT | 6 | 5.7% | **금액만 다른 6건이 `{n}` 템플릿 하나로 수렴** |
| `custom_error:0x1f2a2005` | CUSTOM_ERROR | 6 | 5.7% | bare 셀렉터 처리 |
| `Min return not reached` | TEXT | 5 | 4.8% | 애그리게이터 계열 미분류 발견 |

(셀렉터 값은 합성 — 실제 프로토콜 셀렉터 아님. `pct_of_unknown` 합계
100% 확인. `sql/full_script.sql` 단독 실행도 스크래치 DB에서 통과.)

## 클러스터 종류별 다음 액션

- **TEXT** — 문자열이 이미 디코딩돼 있으므로 classifier에 키워드 룰을
  바로 추가할 수 있다. 검증 코퍼스에서 나온 실전 후보:
  - `transfer_from_failed` → `TRANSFER_FAILED` — **현행 룰의 실매칭
    구멍**: `TransferHelper: TRANSFER_FROM_FAILED`(V2 계열 최다 실패
    사유)는 `transfer_failed` substring을 포함하지 않아 UNKNOWN으로
    떨어진다.
  - `min return` / `return amount` → 슬리피지 계열 — 동시에
    `Return amount is not enough`가 현행 `not enough` 룰에 걸려
    `INSUFFICIENT_BALANCE`로 **오분류**되는 사례도 확인 (애그리게이터의
    "덜 받음"은 잔액 문제가 아니라 슬리피지다). 룰 순서상 슬리피지
    세부 룰을 잔액 룰보다 앞에 둬야 잡을 수 있으므로 별도 검토 필요.
  - `UniswapV2: K`, `LOK`, `SPL`, `IIA` 등 Uniswap 축약 코드 — 빈도가
    차오르면 전용 룰 추가.
- **CUSTOM_ERROR** — 셀렉터를 `function_signature` 테이블 방식처럼
  에러 시그니처 사전(4byte 디렉터리 등)과 대조해 디코딩 룰을 추가.
  셀렉터 단위로 이미 묶여 있으므로 상위 몇 개만 처리해도 커버리지가
  크게 오른다.
- **PANIC** — 코드별 매핑이 기계적으로 가능 (`0x11` 산술 오버플로우
  등). 카테고리 신설(`ARITHMETIC_ERROR` 등) 또는 진단 메시지 확장 후보.
- **NO_DATA** — revert 출력이 없는 실패 (out-of-gas 등). 문자열로는
  더 못 나누고, `trace_log`의 콜트리 피처(깊이·gas 소진 패턴)로
  후속 분석해야 하는 영역.

## 사용법

```sql
-- UNKNOWN 상위 클러스터 (= 다음 classifier 룰 후보)
SELECT template, cluster_kind, occurrences, pct_of_unknown, total_gas_wasted
FROM vw_unknown_revert_clusters
LIMIT 20;

-- 액션 종류별 요약
SELECT cluster_kind, COUNT(*) AS clusters, SUM(occurrences) AS txs
FROM vw_unknown_revert_clusters
GROUP BY cluster_kind ORDER BY txs DESC;
```

## 후속 작업

이 분석에서 곧바로 이어진 작업 (같은 브랜치에서 완료):

1. ~~`classifier.rs` 룰 추가~~ — **완료**: TRANSFER_FROM_FAILED 룰 구멍
   + `not enough` 오분류 수정, 회귀 테스트 포함.
2. ~~API 노출 및 TUI 패널~~ — **완료**:
   `GET /v1/analytics/failed-tx/unknown-clusters` + TUI `[4] Clusters` 탭.

남은 것 (별도 PR):

3. `sql/olap/001_window_functions.sql` Query 8 피벗이 세분화 이전 6개
   카테고리만 세는 문제 (S12.1에서 추가된 4개 미반영) — 별도 수정.
4. 에러 셀렉터 사전 테이블(`error_signature`) — `function_signature`와
   동일한 패턴으로 CUSTOM_ERROR 클러스터 디코딩.
5. 실제 메인넷 backfill 데이터로 클러스터 분포 재검증 — 지금까지의
   검증은 합성 코퍼스 기준이다.
