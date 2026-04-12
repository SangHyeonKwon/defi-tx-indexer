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
    tx_hash: &str,
    trace_json: &serde_json::Value,
) -> Result<FlattenedTrace, DecodeError> {
    let root = parse_call_frame(trace_json)?;
    let mut frames = Vec::new();
    flatten_call_frame(&root, 0, &mut frames);

    Ok(FlattenedTrace {
        tx_hash: tx_hash.to_string(),
        frames,
    })
}

/// 리버트 사유를 ABI 디코딩한다.
///
/// `Error(string)` 또는 `Panic(uint256)` 시그니처를 디코딩하여
/// 사람이 읽을 수 있는 문자열을 반환한다.
pub fn decode_revert_reason(output: &[u8]) -> Result<String, DecodeError> {
    if output.len() < 4 {
        return Ok(format!("0x{}", bytes_to_hex(output)));
    }

    let selector = &output[..4];

    // Error(string) — selector 0x08c379a0
    if selector == [0x08, 0xc3, 0x79, 0xa0] {
        return decode_abi_string(&output[4..]);
    }

    // Panic(uint256) — selector 0x4e487b71
    if selector == [0x4e, 0x48, 0x7b, 0x71] && output.len() >= 36 {
        let panic_code = output[35]; // uint256의 마지막 바이트
        return Ok(format!("Panic(0x{panic_code:02x})"));
    }

    // 알 수 없는 에러 — hex 반환
    Ok(format!("0x{}", bytes_to_hex(output)))
}

/// JSON에서 `CallFrame`을 수동 파싱한다.
///
/// `callTracer`의 JSON 필드명(camelCase, hex 값)을 처리한다.
fn parse_call_frame(json: &serde_json::Value) -> Result<CallFrame, DecodeError> {
    let call_type = json
        .get("type")
        .and_then(|v| v.as_str())
        .ok_or_else(|| DecodeError::TraceParse("missing 'type' field".to_string()))?
        .to_string();

    let from = json
        .get("from")
        .and_then(|v| v.as_str())
        .unwrap_or("0x0000000000000000000000000000000000000000")
        .to_lowercase();

    let to = json
        .get("to")
        .and_then(|v| v.as_str())
        .map(|s| s.to_lowercase());

    let value = json
        .get("value")
        .and_then(|v| v.as_str())
        .map(|s| s.to_string());

    let gas_used = json
        .get("gasUsed")
        .and_then(|v| v.as_str())
        .and_then(|s| {
            let s = s.strip_prefix("0x").unwrap_or(s);
            u64::from_str_radix(s, 16).ok()
        })
        .unwrap_or(0);

    let input = json
        .get("input")
        .and_then(|v| v.as_str())
        .map(|s| s.to_string());

    let output = json
        .get("output")
        .and_then(|v| v.as_str())
        .map(|s| s.to_string());

    let error = json
        .get("error")
        .and_then(|v| v.as_str())
        .map(|s| s.to_string());

    let calls = json
        .get("calls")
        .and_then(|v| v.as_array())
        .map(|arr| {
            arr.iter()
                .filter_map(|c| parse_call_frame(c).ok())
                .collect()
        })
        .unwrap_or_default();

    Ok(CallFrame {
        call_type,
        from,
        to,
        value,
        gas_used,
        input,
        output,
        error,
        calls,
    })
}

/// 호출 프레임을 재귀적으로 플래튼한다.
fn flatten_call_frame(frame: &CallFrame, depth: i32, out: &mut Vec<FlatFrame>) {
    let value_decimal = frame
        .value
        .as_ref()
        .map(|v| {
            let hex = v.strip_prefix("0x").unwrap_or(v);
            u128::from_str_radix(hex, 16)
                .map(|n| n.to_string())
                .unwrap_or_else(|_| "0".to_string())
        })
        .unwrap_or_else(|| "0".to_string());

    out.push(FlatFrame {
        depth,
        call_type: frame.call_type.clone(),
        from: frame.from.clone(),
        to: frame.to.clone(),
        value: value_decimal,
        gas_used: frame.gas_used as i64,
        input: frame.input.clone(),
        output: frame.output.clone(),
        error: frame.error.clone(),
    });

    for child in &frame.calls {
        flatten_call_frame(child, depth + 1, out);
    }
}

/// ABI 인코딩된 `Error(string)` 데이터에서 문자열을 추출한다.
fn decode_abi_string(data: &[u8]) -> Result<String, DecodeError> {
    // 최소 64바이트 필요: offset(32) + length(32)
    if data.len() < 64 {
        return Err(DecodeError::AbiDecode(
            "ABI string data too short".to_string(),
        ));
    }

    // offset 32바이트 스킵, 길이 추출 (마지막 8바이트만 사용)
    let mut len_buf = [0u8; 8];
    len_buf.copy_from_slice(&data[56..64]);
    let len = u64::from_be_bytes(len_buf) as usize;

    let str_start = 64;
    let str_end = str_start + len;
    if str_end > data.len() {
        return Err(DecodeError::AbiDecode(
            "string length exceeds data".to_string(),
        ));
    }

    Ok(String::from_utf8_lossy(&data[str_start..str_end]).to_string())
}

/// 바이트 슬라이스를 hex 문자열로 변환한다.
fn bytes_to_hex(bytes: &[u8]) -> String {
    bytes.iter().map(|b| format!("{b:02x}")).collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn test_parse_trace_simple() {
        let trace = json!({
            "type": "CALL",
            "from": "0xabc",
            "to": "0xdef",
            "value": "0x0",
            "gasUsed": "0x5208",
            "input": "0x",
            "output": "0x"
        });

        let result = parse_trace("0xtx", &trace).expect("should parse");
        assert_eq!(result.tx_hash, "0xtx");
        assert_eq!(result.frames.len(), 1);
        assert_eq!(result.frames[0].call_type, "CALL");
        assert_eq!(result.frames[0].gas_used, 21000);
        assert_eq!(result.frames[0].value, "0");
        assert_eq!(result.frames[0].depth, 0);
    }

    #[test]
    fn test_parse_trace_nested() {
        let trace = json!({
            "type": "CALL",
            "from": "0xaaa",
            "to": "0xbbb",
            "value": "0xde0b6b3a7640000",
            "gasUsed": "0x10000",
            "input": "0x",
            "output": "0x",
            "calls": [
                {
                    "type": "DELEGATECALL",
                    "from": "0xbbb",
                    "to": "0xccc",
                    "gasUsed": "0x5000",
                    "input": "0xabcd",
                    "output": "0x"
                },
                {
                    "type": "STATICCALL",
                    "from": "0xbbb",
                    "to": "0xddd",
                    "gasUsed": "0x1000",
                    "input": "0x",
                    "output": "0x1234"
                }
            ]
        });

        let result = parse_trace("0xtx", &trace).expect("should parse");
        assert_eq!(result.frames.len(), 3);
        assert_eq!(result.frames[0].depth, 0);
        assert_eq!(result.frames[0].value, "1000000000000000000"); // 1 ETH
        assert_eq!(result.frames[1].depth, 1);
        assert_eq!(result.frames[1].call_type, "DELEGATECALL");
        assert_eq!(result.frames[2].depth, 1);
        assert_eq!(result.frames[2].call_type, "STATICCALL");
    }

    #[test]
    fn test_parse_trace_missing_type() {
        let bad = json!({"from": "0x1"});
        let result = parse_trace("0xtx", &bad);
        assert!(matches!(result, Err(DecodeError::TraceParse(_))));
    }

    #[test]
    fn test_decode_revert_error_string() {
        // Error(string) "Insufficient balance"
        let msg = b"Insufficient balance";
        let mut data = vec![0x08, 0xc3, 0x79, 0xa0]; // selector
        data.extend_from_slice(&[0u8; 31]);
        data.push(0x20); // offset = 32
        data.extend_from_slice(&[0u8; 31]);
        data.push(msg.len() as u8); // length
        data.extend_from_slice(msg);
        // pad to 32
        let pad = 32 - (msg.len() % 32);
        if pad < 32 {
            data.extend(vec![0u8; pad]);
        }

        let reason = decode_revert_reason(&data).expect("should decode");
        assert_eq!(reason, "Insufficient balance");
    }

    #[test]
    fn test_decode_revert_panic() {
        let mut data = vec![0x4e, 0x48, 0x7b, 0x71]; // Panic selector
        data.extend_from_slice(&[0u8; 31]);
        data.push(0x01); // panic code 1

        let reason = decode_revert_reason(&data).expect("should decode");
        assert_eq!(reason, "Panic(0x01)");
    }

    #[test]
    fn test_decode_revert_unknown() {
        let data = vec![0xde, 0xad, 0xbe, 0xef, 0x01, 0x02];
        let reason = decode_revert_reason(&data).expect("should decode");
        assert_eq!(reason, "0xdeadbeef0102");
    }

    #[test]
    fn test_decode_revert_short() {
        let data = vec![0x01, 0x02];
        let reason = decode_revert_reason(&data).expect("should decode");
        assert_eq!(reason, "0x0102");
    }
}
