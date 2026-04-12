use serde::{Deserialize, Serialize};

use crate::error::DecodeError;

/// `debug_traceTransaction` 결과의 단일 호출 프레임.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CallFrame {
    /// 호출 타입 (CALL, DELEGATECALL, STATICCALL, CREATE, CREATE2)
    pub call_type: String,
    /// 호출자 주소
    pub from: String,
    /// 대상 주소 (CREATE일 경우 None)
    pub to: Option<String>,
    /// 전송 값 (hex)
    pub value: Option<String>,
    /// 사용된 가스
    pub gas_used: u64,
    /// 입력 데이터 (hex)
    pub input: Option<String>,
    /// 출력 데이터 (hex)
    pub output: Option<String>,
    /// 에러 메시지
    pub error: Option<String>,
    /// 중첩 호출
    pub calls: Vec<CallFrame>,
}

/// 트레이스 파싱 결과 (플래튼된 호출 목록).
#[derive(Debug, Clone)]
pub struct FlattenedTrace {
    /// 트랜잭션 해시
    pub tx_hash: String,
    /// 플래튼된 호출 프레임
    pub frames: Vec<FlatFrame>,
}

/// 단일 플래튼된 호출 프레임 (depth 포함).
#[derive(Debug, Clone)]
pub struct FlatFrame {
    /// 호출 깊이 (0부터 시작)
    pub depth: i32,
    /// 호출 타입
    pub call_type: String,
    /// 호출자
    pub from: String,
    /// 대상
    pub to: Option<String>,
    /// 전송 값 (wei, 10진수 문자열)
    pub value: String,
    /// 사용된 가스
    pub gas_used: i64,
    /// 입력 데이터
    pub input: Option<String>,
    /// 출력 데이터
    pub output: Option<String>,
    /// 에러 메시지
    pub error: Option<String>,
}

/// 트레이스 JSON 응답을 파싱하여 플래튼된 호출 트리를 반환한다.
///
/// `debug_traceTransaction`의 `callTracer` 응답을 파싱한다.
pub fn parse_trace(
    _tx_hash: &str,
    _trace_json: &serde_json::Value,
) -> Result<FlattenedTrace, DecodeError> {
    // Phase 3에서 구현
    // 1. JSON에서 CallFrame 역직렬화
    // 2. 재귀적으로 호출 트리 순회
    // 3. 각 프레임을 FlatFrame으로 변환 (depth 할당)
    Err(DecodeError::TraceParse("not yet implemented".to_string()))
}

/// 리버트 사유를 ABI 디코딩한다.
///
/// `Error(string)` 또는 커스텀 에러 시그니처를 디코딩하여 사람이 읽을 수 있는 문자열을 반환한다.
pub fn decode_revert_reason(_output: &[u8]) -> Result<String, DecodeError> {
    // Phase 3에서 구현
    // 1. 첫 4바이트로 에러 시그니처 확인
    // 2. Error(string) → ABI 디코딩
    // 3. 커스텀 에러 → 시그니처 DB 조회 또는 hex 반환
    Err(DecodeError::AbiDecode("not yet implemented".to_string()))
}
