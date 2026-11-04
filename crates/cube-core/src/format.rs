//! Parsing and formatting of solve durations.

/// Upper bound accepted by [`parse_time`], exclusive.
const MAX_MS: u64 = 24 * 60 * 60 * 1000;

/// Formats milliseconds for display: `9.123`, `1:02.345` or `1:00:00.000`.
pub fn format_ms(ms: u64) -> String {
    let millis = ms % 1000;
    let total_seconds = ms / 1000;
    let seconds = total_seconds % 60;
    let minutes = total_seconds / 60 % 60;
    let hours = total_seconds / 3600;
    if hours > 0 {
        format!("{hours}:{minutes:02}:{seconds:02}.{millis:03}")
    } else if minutes > 0 {
        format!("{minutes}:{seconds:02}.{millis:03}")
    } else {
        format!("{seconds}.{millis:03}")
    }
}

/// Formats a difference between two times, such as `+1.200` or `−0.345`.
pub fn format_delta(ms: i64) -> String {
    let sign = if ms < 0 { '−' } else { '+' };
    format!("{sign}{}", format_ms(ms.unsigned_abs()))
}

/// Parses seconds, `M:SS` or `H:MM:SS` with up to three decimals into milliseconds.
///
/// Integer arithmetic avoids rounding milliseconds away. Returns `None` for
/// anything else, including durations of 24 hours or more.
pub fn parse_time(text: &str) -> Option<u64> {
    let text = text.trim();
    let (whole, fraction) = text.split_once('.').unwrap_or((text, "0"));
    let all_digits = |part: &str| !part.is_empty() && part.bytes().all(|b| b.is_ascii_digit());
    if !all_digits(fraction) || fraction.len() > 3 {
        return None;
    }

    let parts: Vec<&str> = whole.split(':').collect();
    if parts.len() > 3 {
        return None;
    }
    let mut seconds = 0_u64;
    for (index, part) in parts.iter().enumerate() {
        if !all_digits(part) {
            return None;
        }
        let value: u64 = part.parse().ok()?;
        if index > 0 && value >= 60 {
            return None;
        }
        seconds = seconds.checked_mul(60)?.checked_add(value)?;
    }

    let scale = 10_u64.pow(3 - fraction.len() as u32);
    let millis = fraction.parse::<u64>().ok()? * scale;
    let total = seconds.checked_mul(1000)?.checked_add(millis)?;
    (total < MAX_MS).then_some(total)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn deltas_always_carry_a_sign() {
        assert_eq!(format_delta(1_200), "+1.200");
        assert_eq!(format_delta(-345), "−0.345");
        assert_eq!(format_delta(0), "+0.000");
        assert_eq!(format_delta(-61_000), "−1:01.000");
    }

    #[test]
    fn formats_only_the_units_in_use() {
        assert_eq!(format_ms(0), "0.000");
        assert_eq!(format_ms(9_123), "9.123");
        assert_eq!(format_ms(62_345), "1:02.345");
        assert_eq!(format_ms(3_600_000), "1:00:00.000");
    }

    #[test]
    fn parses_every_supported_shape() {
        assert_eq!(parse_time("9"), Some(9_000));
        assert_eq!(parse_time(" 9.5 "), Some(9_500));
        assert_eq!(parse_time("9.123"), Some(9_123));
        assert_eq!(parse_time("1:02.345"), Some(62_345));
        assert_eq!(parse_time("1:00:00"), Some(3_600_000));
    }

    #[test]
    fn rejects_malformed_times() {
        for text in [
            "", ".", "9.", ".5", "9.1234", "1:60", "1:2:3:4", "a", "-1", "24:00:00",
        ] {
            assert_eq!(parse_time(text), None, "{text:?}");
        }
    }

    #[test]
    fn formatted_times_parse_back() {
        for ms in [0, 9_123, 62_345, 3_600_000] {
            assert_eq!(parse_time(&format_ms(ms)), Some(ms));
        }
    }
}
