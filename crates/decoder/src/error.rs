/// 디코딩 에러 타입.
///
/// ABI 디코딩 및 이벤트 파싱 과정에서 발생하는 모든 에러를 포함한다.
#[derive(Debug, thiserror::Error)]
pub enum DecodeError {
    /// 알 수 없는 이벤트 토픽
    #[error("unknown event topic: {0}")]
    UnknownTopic(String),

    /// ABI 디코딩 실패
    #[error("ABI decode error: {0}")]
    AbiDecode(String),

    /// 필수 필드 누락
    #[error("missing field: {0}")]
    MissingField(String),

    /// 로그 데이터 길이 불일치
    #[error("invalid data length: expected {expected}, got {actual}")]
    InvalidDataLength { expected: usize, actual: usize },

    /// 트레이스 파싱 에러
    #[error("trace parse error: {0}")]
    TraceParse(String),

    /// JSON 파싱 에러
    #[error("JSON error: {0}")]
    Json(#[from] serde_json::Error),
}
