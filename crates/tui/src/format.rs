//! 표시용 포매터 — `web/src/lib/format.ts`의 Rust 이식.
//!
//! 색/라벨은 웹 대시보드와 시각적으로 일치시켜 두 클라이언트가 같은 카테고리를
//! 같은 색으로 보여주도록 한다.

use chrono::{DateTime, Utc};
use ratatui::style::Color;

/// 필터 순환용 — 10개 카테고리의 SCREAMING_SNAKE 와이어 값.
pub const CATEGORIES: [&str; 10] = [
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

/// 와이어 `error_category`(PascalCase 또는 SCREAMING_SNAKE)를 정규 SCREAMING_SNAKE로.
///
/// API 응답은 PascalCase(`"Unknown"`), 필터/시드 키는 SCREAMING_SNAKE를 쓰는
/// 스큐를 흡수한다. 알 수 없는 입력은 `"UNKNOWN"`.
pub fn normalize_category(raw: &str) -> &'static str {
    match raw {
        "InsufficientBalance" | "INSUFFICIENT_BALANCE" => "INSUFFICIENT_BALANCE",
        "InsufficientAllowance" | "INSUFFICIENT_ALLOWANCE" => "INSUFFICIENT_ALLOWANCE",
        "SlippageExceeded" | "SLIPPAGE_EXCEEDED" => "SLIPPAGE_EXCEEDED",
        "SlippageAmountOut" | "SLIPPAGE_AMOUNT_OUT" => "SLIPPAGE_AMOUNT_OUT",
        "SlippageAmountIn" | "SLIPPAGE_AMOUNT_IN" => "SLIPPAGE_AMOUNT_IN",
        "SlippagePriceImpact" | "SLIPPAGE_PRICE_IMPACT" => "SLIPPAGE_PRICE_IMPACT",
        "DeadlineExpired" | "DEADLINE_EXPIRED" => "DEADLINE_EXPIRED",
        "Unauthorized" | "UNAUTHORIZED" => "UNAUTHORIZED",
        "TransferFailed" | "TRANSFER_FAILED" => "TRANSFER_FAILED",
        _ => "UNKNOWN",
    }
}

/// 카테고리의 사람이 읽는 라벨 (web `ERROR_LABELS` 미러).
pub fn error_category_label(raw: &str) -> &'static str {
    match normalize_category(raw) {
        "INSUFFICIENT_BALANCE" => "Insufficient balance",
        "INSUFFICIENT_ALLOWANCE" => "Insufficient allowance",
        "SLIPPAGE_EXCEEDED" => "Slippage exceeded",
        "SLIPPAGE_AMOUNT_OUT" => "Slippage (amount out)",
        "SLIPPAGE_AMOUNT_IN" => "Slippage (amount in)",
        "SLIPPAGE_PRICE_IMPACT" => "Slippage (price impact)",
        "DEADLINE_EXPIRED" => "Deadline expired",
        "UNAUTHORIZED" => "Unauthorized",
        "TRANSFER_FAILED" => "Transfer failed",
        _ => "Unknown",
    }
}

/// 카테고리별 안정적 색 (web `ERROR_COLORS` 미러).
pub fn error_category_color(raw: &str) -> Color {
    match normalize_category(raw) {
        "INSUFFICIENT_BALANCE" => Color::Rgb(0xF6, 0x60, 0x61),
        "INSUFFICIENT_ALLOWANCE" => Color::Rgb(0xFF, 0x89, 0x89),
        "SLIPPAGE_EXCEEDED" => Color::Rgb(0xF4, 0xBD, 0x50),
        "SLIPPAGE_AMOUNT_OUT" => Color::Rgb(0xFF, 0xD5, 0x80),
        "SLIPPAGE_AMOUNT_IN" => Color::Rgb(0xE5, 0xA9, 0x3D),
        "SLIPPAGE_PRICE_IMPACT" => Color::Rgb(0xC9, 0x92, 0x32),
        "DEADLINE_EXPIRED" => Color::Rgb(0xE8, 0x89, 0x57),
        "UNAUTHORIZED" => Color::Rgb(0xC9, 0x81, 0xE6),
        "TRANSFER_FAILED" => Color::Rgb(0xE0, 0x6D, 0x6E),
        _ => Color::Rgb(0x88, 0x88, 0x88),
    }
}

/// UNKNOWN revert 클러스터 종류별 안정적 색.
///
/// NO_DATA는 중립 회색(출력 자체가 없음), CUSTOM_ERROR는 보라(미디코딩 셀렉터),
/// PANIC은 적색(Solidity Panic), TEXT는 앰버(즉시 룰 추가 가능한 후보).
pub fn cluster_kind_color(kind: &str) -> Color {
    match kind {
        "CUSTOM_ERROR" => Color::Rgb(0xC9, 0x81, 0xE6),
        "PANIC" => Color::Rgb(0xF6, 0x60, 0x61),
        "TEXT" => Color::Rgb(0xF4, 0xBD, 0x50),
        _ => Color::Rgb(0x88, 0x88, 0x88), // NO_DATA 및 미지의 값
    }
}

/// 와이어 `Decimal`(문자열)을 `f64`로 — 비유한 값은 0.
pub fn to_number(s: &str) -> f64 {
    let n: f64 = s.trim().parse().unwrap_or(f64::NAN);
    if n.is_finite() {
        n
    } else {
        0.0
    }
}

/// 소수점 끝 0/`.` 제거. (`"1.00" → "1"`, `"1.20" → "1.2"`)
fn trim_zeros(s: &str) -> &str {
    if s.contains('.') {
        s.trim_end_matches('0').trim_end_matches('.')
    } else {
        s
    }
}

/// `1234567 → "1.23M"` (web `formatCompact` 근사 — K/M/B/T).
pub fn format_compact(n: f64) -> String {
    let abs = n.abs();
    let (div, suffix) = if abs >= 1e12 {
        (1e12, "T")
    } else if abs >= 1e9 {
        (1e9, "B")
    } else if abs >= 1e6 {
        (1e6, "M")
    } else if abs >= 1e3 {
        (1e3, "K")
    } else {
        (1.0, "")
    };
    let scaled = format!("{:.2}", n / div);
    format!("{}{}", trim_zeros(&scaled), suffix)
}

/// 와이어 비율 문자열(`"48.00"`)을 `"48.00%"`로.
pub fn format_pct_str(s: &str) -> String {
    format!("{:.2}%", to_number(s))
}

/// 정수에 천 단위 쉼표. (`18000000 → "18,000,000"`)
pub fn group_thousands(n: i64) -> String {
    let digits = n.unsigned_abs().to_string();
    let bytes = digits.as_bytes();
    let len = bytes.len();
    let mut out = String::with_capacity(len + len / 3);
    for (i, b) in bytes.iter().enumerate() {
        if i > 0 && (len - i) % 3 == 0 {
            out.push(',');
        }
        out.push(*b as char);
    }
    if n < 0 {
        format!("-{out}")
    } else {
        out
    }
}

/// 긴 해시/주소를 `0x1234ab…ef90` (앞 6 + 뒤 4)로 축약 (web `shortAddress`).
pub fn truncate_hash(s: &str) -> String {
    if s.len() <= 12 {
        return s.to_string();
    }
    format!("{}…{}", &s[..6], &s[s.len() - 4..])
}

/// RFC3339 시각을 상대 표현(`"3d ago"`, `"just now"`)으로.
pub fn time_ago(iso: &str) -> String {
    time_ago_at(iso, Utc::now())
}

/// [`time_ago`]의 테스트 가능 버전 — `now`를 주입받는다.
fn time_ago_at(iso: &str, now: DateTime<Utc>) -> String {
    let then = match DateTime::parse_from_rfc3339(iso) {
        Ok(t) => t.with_timezone(&Utc),
        Err(_) => return iso.to_string(),
    };
    let secs = (now - then).num_seconds();
    if secs < 60 {
        return "just now".to_string();
    }
    let mins = secs / 60;
    if mins < 60 {
        return format!("{mins}m ago");
    }
    let hours = mins / 60;
    if hours < 24 {
        return format!("{hours}h ago");
    }
    let days = hours / 24;
    if days < 30 {
        return format!("{days}d ago");
    }
    let months = days / 30;
    if months < 12 {
        return format!("{months}mo ago");
    }
    format!("{}y ago", months / 12)
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::TimeZone;

    #[test]
    fn normalize_accepts_both_cases() {
        assert_eq!(normalize_category("Unknown"), "UNKNOWN");
        assert_eq!(normalize_category("UNKNOWN"), "UNKNOWN");
        assert_eq!(
            normalize_category("SlippageAmountOut"),
            "SLIPPAGE_AMOUNT_OUT"
        );
        assert_eq!(
            normalize_category("SLIPPAGE_AMOUNT_OUT"),
            "SLIPPAGE_AMOUNT_OUT"
        );
        assert_eq!(normalize_category("something-weird"), "UNKNOWN");
    }

    #[test]
    fn label_and_color_via_normalize() {
        assert_eq!(
            error_category_label("SlippageAmountOut"),
            "Slippage (amount out)"
        );
        assert_eq!(
            error_category_color("Unknown"),
            Color::Rgb(0x88, 0x88, 0x88)
        );
    }

    #[test]
    fn cluster_kind_colors_are_stable() {
        assert_eq!(cluster_kind_color("PANIC"), Color::Rgb(0xF6, 0x60, 0x61));
        assert_eq!(cluster_kind_color("NO_DATA"), Color::Rgb(0x88, 0x88, 0x88));
        assert_eq!(
            cluster_kind_color("something-weird"),
            Color::Rgb(0x88, 0x88, 0x88)
        );
    }

    #[test]
    fn compact_thresholds() {
        assert_eq!(format_compact(1_234_567.0), "1.23M");
        assert_eq!(format_compact(1000.0), "1K");
        assert_eq!(format_compact(1500.0), "1.5K");
        assert_eq!(format_compact(999.0), "999");
        assert_eq!(format_compact(2_500_000_000.0), "2.5B");
    }

    #[test]
    fn to_number_handles_decimals_and_garbage() {
        assert_eq!(to_number("45000.00"), 45000.0);
        assert_eq!(to_number("  12.5 "), 12.5);
        assert_eq!(to_number(""), 0.0);
        assert_eq!(to_number("NaN"), 0.0);
    }

    #[test]
    fn group_thousands_formats() {
        assert_eq!(group_thousands(18_000_000), "18,000,000");
        assert_eq!(group_thousands(999), "999");
        assert_eq!(group_thousands(-1234), "-1,234");
    }

    #[test]
    fn truncate_hash_shortens() {
        assert_eq!(
            truncate_hash("0xdead000000000000000000000000000000000000000000000000000000000001"),
            "0xdead…0001"
        );
        assert_eq!(truncate_hash("0xabcd"), "0xabcd");
    }

    #[test]
    fn time_ago_buckets() {
        let now = Utc.with_ymd_and_hms(2023, 9, 1, 12, 0, 0).unwrap();
        assert_eq!(time_ago_at("2023-09-01T11:59:30Z", now), "just now");
        assert_eq!(time_ago_at("2023-09-01T11:30:00Z", now), "30m ago");
        assert_eq!(time_ago_at("2023-09-01T09:00:00Z", now), "3h ago");
        assert_eq!(time_ago_at("2023-08-29T12:00:00Z", now), "3d ago");
        assert_eq!(time_ago_at("not-a-date", now), "not-a-date");
    }
}
