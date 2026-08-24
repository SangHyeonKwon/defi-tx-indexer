//! API 응답 와이어 미러 (client-side DTO).
//!
//! `db` 크레이트 모델은 `Serialize + FromRow`만 derive하므로 직접 재사용할 수
//! 없다 — 여기서 `Deserialize` DTO를 별도로 정의한다.
//!
//! ## 두 가지 와이어 포맷 주의 (코드 주석으로 명시)
//!
//! 1. **`error_category`는 PascalCase로 직렬화된다** (예: `"Unknown"`,
//!    `"SlippageAmountOut"`). `db::models::ErrorCategory`에 `#[serde(rename_all)]`이
//!    없어 serde가 Rust 변수명을 그대로 내보내기 때문(`#[sqlx(rename_all=...)]`은
//!    DB 드라이버 전용). 반면 **필터 쿼리 파라미터**는 SCREAMING_SNAKE를 기대한다.
//!    → DTO는 `String`으로 받고 [`crate::format::normalize_category`]가 양쪽을
//!    흡수한다. enum으로 받으면 한쪽이 깨진다.
//! 2. **BigDecimal 필드는 JSON 문자열로 직렬화된다** (예: `"45000.00"`).
//!    숫자 타입으로 받으면 역직렬화 실패 → `String`으로 받고
//!    [`crate::format::to_number`]로 파싱한다.

use serde::Deserialize;
use serde_json::Value;

/// 단일 리소스 응답 래퍼: `{ "data": T }`.
#[derive(Debug, Clone, Deserialize)]
pub struct ApiResponse<T> {
    /// 응답 데이터.
    pub data: T,
}

/// `total` 포함 페이지네이션 메타.
#[derive(Debug, Clone, Deserialize)]
pub struct PaginationMeta {
    /// 요청된 최대 아이템 수.
    pub limit: i64,
    /// 건너뛴 아이템 수.
    pub offset: i64,
    /// 이 페이지에 반환된 아이템 수.
    pub count: i64,
    /// 필터 적용 후 전체 건수 (LIMIT/OFFSET 무관).
    pub total: i64,
}

/// `total` 포함 페이지네이션 목록 응답: `{ "data": [T], "pagination": {...} }`.
#[derive(Debug, Clone, Deserialize)]
pub struct TotalPaginated<T> {
    /// 응답 데이터 목록.
    pub data: Vec<T>,
    /// 페이지네이션 정보 (`total` 포함).
    pub pagination: PaginationMeta,
}

/// API 에러 본문: `{ "error": "message" }`.
#[derive(Debug, Clone, Deserialize)]
pub struct ApiErrorBody {
    /// 에러 메시지.
    pub error: String,
}

/// 실패한 트랜잭션 한 건 (목록/상세 공통).
#[derive(Debug, Clone, Deserialize)]
pub struct FailedTransaction {
    /// 트랜잭션 해시.
    pub tx_hash: String,
    /// 에러 카테고리 — **와이어에서 PascalCase** (모듈 주석 참고).
    pub error_category: String,
    /// 리버트 사유 (디코딩된 텍스트).
    pub revert_reason: Option<String>,
    /// 실패한 함수 (4-byte selector 또는 함수명).
    pub failing_function: Option<String>,
    /// 사용된(낭비된) 가스.
    pub gas_used: i64,
    /// 실패 타임스탬프 (RFC3339).
    pub timestamp: String,
}

/// 트랜잭션 내부 호출 트레이스 한 프레임.
///
/// 일부 필드(`tx_hash`/`input`/`output`)는 와이어 계약을 충실히 미러하기 위해
/// 유지하되 MVP UI에선 렌더하지 않는다(테스트는 전체를 역직렬화). 의도된
/// 미사용이므로 `#[allow(dead_code)]`로 명시한다.
#[derive(Debug, Clone, Deserialize)]
pub struct TraceLog {
    /// 트랜잭션 해시 (상세 화면에선 `failed.tx_hash`와 중복이라 미표시).
    #[allow(dead_code)]
    pub tx_hash: String,
    /// 호출 깊이 (들여쓰기 레벨).
    pub call_depth: i32,
    /// 호출 타입 (CALL, DELEGATECALL, STATICCALL, CREATE, CREATE2).
    pub call_type: String,
    /// 호출자 주소.
    pub from_addr: String,
    /// 대상 주소 (None = CREATE).
    pub to_addr: Option<String>,
    /// 전송 값 (wei) — **BigDecimal → 문자열**.
    pub value: String,
    /// 사용된 가스.
    pub gas_used: i64,
    /// 입력 데이터 (hex) — 디코딩 결과로 대체 표시하므로 raw는 미표시.
    #[allow(dead_code)]
    pub input: Option<String>,
    /// 출력 데이터 (hex) — MVP 미표시.
    #[allow(dead_code)]
    pub output: Option<String>,
    /// 에러 메시지 (이 프레임이 revert 했으면 Some).
    pub error: Option<String>,
    /// pre-order DFS 인덱스 (트리 재구성/ root_cause 매칭 키).
    pub trace_id: i64,
}

/// 디코딩된 단일 함수 인자 — `{ "type": "...", "value": ... }`.
#[derive(Debug, Clone, Deserialize)]
pub struct DecodedArg {
    /// Solidity 타입 문자열 (`type` 키, 예약어라 rename).
    #[serde(rename = "type")]
    pub ty: String,
    /// JSON-friendly 값 (string | bool | array, 재귀).
    pub value: Value,
}

/// selector → 사람이 읽는 함수명/시그니처 (+ 디코딩된 인자).
#[derive(Debug, Clone, Deserialize)]
pub struct DecodedFunction {
    /// 4-byte selector.
    pub selector: String,
    /// 함수명.
    pub name: String,
    /// ABI 시그니처.
    pub signature: String,
    /// 시드 출처 (erc20, uniswap-v3-router 등).
    pub source: Option<String>,
    /// 타입된 인자값 — `null`은 *디코드 시도 안 함 또는 실패* (명시 신호).
    pub args: Option<Vec<DecodedArg>>,
}

/// 카테고리 진단 — 왜 실패했나 + 어떻게 고치나.
#[derive(Debug, Clone, Deserialize)]
pub struct Diagnosis {
    /// 진단 메시지 (왜 실패했나).
    pub message: String,
    /// 추천 액션 (어떻게 고치나).
    pub recommended_action: Option<String>,
    /// 시드 출처.
    pub source: Option<String>,
}

/// 단건 실패 트랜잭션 진단 결과 (평면 구조 — `data` 아래 바로 위치).
#[derive(Debug, Clone, Deserialize)]
pub struct FailedTxDetail {
    /// 실패 트랜잭션 메타 + 분류.
    pub failed: FailedTransaction,
    /// 평탄화된 콜 프레임 (`trace_id` 오름차순 = pre-order DFS).
    pub call_tree: Vec<TraceLog>,
    /// `call_tree`가 상한에서 잘렸으면 true.
    pub call_tree_truncated: bool,
    /// 실제 revert가 발생한 첫 트레이스 프레임 (없으면 null).
    pub root_cause: Option<TraceLog>,
    /// `failing_function` selector를 디코딩한 결과 (없으면 null).
    pub failing_function_decoded: Option<DecodedFunction>,
    /// `root_cause.input`을 디코딩한 결과 (없으면 null).
    pub root_cause_decoded: Option<DecodedFunction>,
    /// 카테고리 진단 (시드 미존재 시 null).
    pub diagnosis: Option<Diagnosis>,
}

/// 카테고리별 실패 분석 한 행 (`/v1/analytics/failed-tx`).
#[derive(Debug, Clone, Deserialize)]
pub struct FailedTxAnalysis {
    /// 에러 카테고리 — **와이어에서 PascalCase**.
    pub error_category: String,
    /// 실패 건수.
    pub failure_count: i64,
    /// 평균 낭비 가스 — **BigDecimal → 문자열**.
    pub avg_gas_wasted: String,
    /// 전체 대비 비율(%) — **BigDecimal → 문자열**.
    pub pct_of_total: String,
    /// 가장 최근 실패 시각 (RFC3339).
    pub most_recent_failure: String,
}

/// UNKNOWN revert 클러스터 한 행 (`/v1/analytics/failed-tx/unknown-clusters`).
///
/// 일부 필드(`avg_gas_wasted`/`distinct_selectors`/`sample_*`/`first_seen`)는
/// 와이어 계약을 충실히 미러하기 위해 유지하되 MVP UI에선 렌더하지 않는다
/// (테스트는 전체를 역직렬화). 의도된 미사용이므로 `#[allow(dead_code)]`.
#[derive(Debug, Clone, Deserialize)]
pub struct UnknownRevertCluster {
    /// 정규화된 revert 템플릿 (클러스터 키).
    pub template: String,
    /// 클러스터 종류: NO_DATA | CUSTOM_ERROR | PANIC | TEXT.
    pub cluster_kind: String,
    /// 발생 건수.
    pub occurrences: i64,
    /// 전체 UNKNOWN 대비 비율(%) — **BigDecimal → 문자열**.
    pub pct_of_unknown: String,
    /// 총 낭비 가스 — **BigDecimal → 문자열**.
    pub total_gas_wasted: String,
    /// 평균 낭비 가스 — **BigDecimal → 문자열** (MVP 미표시).
    #[allow(dead_code)]
    pub avg_gas_wasted: String,
    /// 고유 발신자 수.
    pub distinct_senders: i64,
    /// 고유 함수 셀렉터 수 (MVP 미표시).
    #[allow(dead_code)]
    pub distinct_selectors: i64,
    /// 대표 revert 사유 — 독립 MIN, 예시 용도 (MVP 미표시).
    #[allow(dead_code)]
    pub sample_revert_reason: Option<String>,
    /// 대표 tx 해시 — 독립 MIN, 예시 용도 (MVP 미표시).
    #[allow(dead_code)]
    pub sample_tx_hash: Option<String>,
    /// 최초 발생 시각 (RFC3339, MVP 미표시).
    #[allow(dead_code)]
    pub first_seen: String,
    /// 최근 발생 시각 (RFC3339).
    pub last_seen: String,
}

#[cfg(test)]
mod tests {
    use super::*;

    /// README 실측 응답 — `error_category`가 PascalCase(`"Unknown"`),
    /// `failing_function_decoded`/`args`가 `null`인 케이스가 깨지지 않아야 한다.
    /// (발견 1·2 회귀 가드)
    #[test]
    fn deserialize_failed_tx_detail_measured_shape() {
        let json = r#"{
          "data": {
            "failed": {
              "tx_hash": "0xdead000000000000000000000000000000000000000000000000000000000001",
              "error_category": "Unknown",
              "revert_reason": null,
              "failing_function": null,
              "gas_used": 45000,
              "timestamp": "2023-09-01T12:00:00Z"
            },
            "call_tree": [
              {
                "tx_hash": "0xdead000000000000000000000000000000000000000000000000000000000001",
                "call_depth": 0,
                "call_type": "CALL",
                "from_addr": "0xabc",
                "to_addr": "0xdef",
                "value": "0",
                "gas_used": 45000,
                "input": "0x414bf389",
                "output": null,
                "error": "Too little received",
                "trace_id": 16
              }
            ],
            "call_tree_truncated": false,
            "root_cause": {
              "tx_hash": "0xdead000000000000000000000000000000000000000000000000000000000001",
              "call_depth": 0,
              "call_type": "CALL",
              "from_addr": "0xabc",
              "to_addr": "0xdef",
              "value": "0",
              "gas_used": 45000,
              "input": "0x414bf389",
              "output": null,
              "error": "Too little received",
              "trace_id": 16
            },
            "failing_function_decoded": null,
            "root_cause_decoded": {
              "selector": "0x414bf389",
              "name": "exactInputSingle",
              "signature": "exactInputSingle((address,address,uint24,address,uint256,uint256,uint256,uint160))",
              "source": "uniswap-v3-router",
              "args": null
            },
            "diagnosis": {
              "message": "The exact failure mode could not be classified from the trace alone.",
              "recommended_action": "Inspect root_cause and the call_tree; raise an issue with the tx hash.",
              "source": "builtin"
            }
          }
        }"#;
        let parsed: ApiResponse<FailedTxDetail> =
            serde_json::from_str(json).expect("detail deserializes");
        let d = parsed.data;
        assert_eq!(d.failed.error_category, "Unknown");
        assert!(d.failing_function_decoded.is_none());
        let rcd = d.root_cause_decoded.expect("root_cause_decoded present");
        assert_eq!(rcd.name, "exactInputSingle");
        assert!(rcd.args.is_none());
        assert_eq!(d.root_cause.expect("root cause").trace_id, 16);
    }

    /// 목록 응답 — `pagination.total`이 LIMIT/OFFSET과 별개로 노출되어야 한다.
    #[test]
    fn deserialize_failed_tx_list_with_total() {
        let json = r#"{
          "data": [
            {"tx_hash":"0xa","error_category":"SlippageAmountOut","revert_reason":"Too little received","failing_function":"0x414bf389","gas_used":52000,"timestamp":"2023-09-01T12:15:30Z"}
          ],
          "pagination": {"limit":3,"offset":0,"count":1,"total":3}
        }"#;
        let parsed: TotalPaginated<FailedTransaction> =
            serde_json::from_str(json).expect("list deserializes");
        assert_eq!(parsed.pagination.total, 3);
        assert_eq!(parsed.data.len(), 1);
        assert_eq!(parsed.data[0].error_category, "SlippageAmountOut");
    }

    /// 분석 응답 — BigDecimal 필드가 **문자열**로 들어와야 한다.
    #[test]
    fn deserialize_analysis_with_string_decimals() {
        let json = r#"{
          "data": [
            {"error_category":"SlippageAmountOut","failure_count":12,"avg_gas_wasted":"52000.00","pct_of_total":"48.00","most_recent_failure":"2023-09-01T12:15:30Z"}
          ]
        }"#;
        let parsed: ApiResponse<Vec<FailedTxAnalysis>> =
            serde_json::from_str(json).expect("analysis deserializes");
        assert_eq!(parsed.data[0].avg_gas_wasted, "52000.00");
        assert_eq!(parsed.data[0].pct_of_total, "48.00");
    }

    /// 클러스터 실측 응답 — NUMERIC 필드가 **문자열**(BigDecimal은 후행 0을
    /// 보존하지 않아 `"100"`), nullable sample 필드가 깨지지 않아야 한다.
    #[test]
    fn deserialize_unknown_clusters_measured_shape() {
        let json = r#"{
          "data": [
            {
              "template": "(no revert data)",
              "cluster_kind": "NO_DATA",
              "occurrences": 1,
              "pct_of_unknown": "100",
              "total_gas_wasted": "41000",
              "avg_gas_wasted": "41000",
              "distinct_senders": 1,
              "distinct_selectors": 0,
              "sample_revert_reason": null,
              "sample_tx_hash": "0xdead000000000000000000000000000000000000000000000000000000000004",
              "first_seen": "2023-09-01T12:00:29Z",
              "last_seen": "2023-09-01T12:00:29Z"
            }
          ]
        }"#;
        let parsed: ApiResponse<Vec<UnknownRevertCluster>> =
            serde_json::from_str(json).expect("clusters deserialize");
        let c = &parsed.data[0];
        assert_eq!(c.cluster_kind, "NO_DATA");
        assert_eq!(c.pct_of_unknown, "100");
        assert!(c.sample_revert_reason.is_none());
        assert!(c.sample_tx_hash.is_some());
    }

    /// `/v1/blocks/latest` — 빈 DB에서 `{ "data": null }`.
    #[test]
    fn deserialize_latest_block_nullable() {
        let some: ApiResponse<Option<i64>> =
            serde_json::from_str(r#"{"data": 18000000}"#).expect("ok");
        assert_eq!(some.data, Some(18000000));
        let none: ApiResponse<Option<i64>> = serde_json::from_str(r#"{"data": null}"#).expect("ok");
        assert_eq!(none.data, None);
    }
}
