//! Validation for manually edited solve fields.
//!
//! The rules match cube-tui's solve editor, so a solve accepted here is
//! accepted there: durations stay below a day and are positive unless the
//! solve is a DNF, dates are RFC 3339 timestamps with a timezone (blank for
//! unknown), comments are bounded plain text, and scrambles use the notation
//! of their event.
//!
//! Dates also have a friendlier local `YYYY-MM-DD HH:MM` form for the desktop
//! dialog, where milliseconds would only get in the way. It parses to the
//! same millisecond timestamps, so stored solves stay compatible.

use chrono::{DateTime, Local, NaiveDateTime, SecondsFormat, TimeZone};

use crate::event::WcaEvent;
use crate::solve::Modifier;

/// Upper bound for solve durations, exclusive: 24 hours in milliseconds.
const MAX_MS: u64 = 24 * 60 * 60 * 1000;

/// Checks a duration and its penalty together, independently of its text form.
///
/// Zero is only valid for a DNF, and every duration must stay below one day.
pub const fn validate_duration(time_ms: u64, modifier: Modifier) -> bool {
    if time_ms >= MAX_MS {
        return false;
    }
    if time_ms == 0 && !matches!(modifier, Modifier::DNF) {
        return false;
    }
    true
}

/// Parses an RFC 3339 date with a timezone at millisecond precision.
///
/// Blank means the original solve date is unknown and parses as `0`.
/// Returns `None` for anything else, including dates without a timezone,
/// dates at finer than millisecond precision, and dates before 1970.
pub fn parse_date(text: &str) -> Option<u64> {
    if text.trim().is_empty() {
        return Some(0);
    }
    DateTime::parse_from_rfc3339(text.trim())
        .ok()
        .filter(|date| date.timestamp_subsec_nanos() % 1_000_000 == 0)
        .and_then(|date| u64::try_from(date.timestamp_millis()).ok())
        .filter(|&date| date > 0)
}

/// Formats Unix milliseconds as an RFC 3339 date in UTC, for editing.
///
/// Returns an empty string when the date is unknown (`0`) or out of range,
/// matching what [`parse_date`] reads back as unknown.
pub fn format_rfc3339(unix_ms: u64) -> String {
    if unix_ms == 0 {
        return String::new();
    }
    i64::try_from(unix_ms)
        .ok()
        .and_then(DateTime::from_timestamp_millis)
        .map_or_else(String::new, |date| {
            date.to_rfc3339_opts(SecondsFormat::Millis, true)
        })
}

/// Formats Unix milliseconds as a local date and time for editing:
/// `2026-11-02 00:07:32`.
///
/// Milliseconds are left out: they don't matter for a solve's date. Returns
/// an empty string when the date is unknown (`0`) or out of range, matching
/// what [`parse_date_input`] reads back as unknown.
pub fn format_date_input(unix_ms: u64) -> String {
    if unix_ms == 0 {
        return String::new();
    }
    i64::try_from(unix_ms)
        .ok()
        .and_then(DateTime::from_timestamp_millis)
        .map_or_else(String::new, |date| {
            date.with_timezone(&Local)
                .format("%Y-%m-%d %H:%M:%S")
                .to_string()
        })
}

/// Parses a local `YYYY-MM-DD HH:MM` date, optionally with `:SS` seconds.
///
/// Blank means the original solve date is unknown and parses as `0`.
/// Returns `None` for anything else, including impossible calendar dates and
/// dates before 1970. Ambiguous local times resolve to their earliest
/// occurrence.
pub fn parse_date_input(text: &str) -> Option<u64> {
    let text = text.trim();
    if text.is_empty() {
        return Some(0);
    }
    let naive = NaiveDateTime::parse_from_str(text, "%Y-%m-%d %H:%M")
        .or_else(|_| NaiveDateTime::parse_from_str(text, "%Y-%m-%d %H:%M:%S"))
        .ok()?;
    let date = Local.from_local_datetime(&naive).earliest()?;
    u64::try_from(date.timestamp_millis())
        .ok()
        .filter(|&ms| ms > 0)
}

/// Checks a comment: at most 4096 characters, without terminal control
/// characters other than newline, carriage return and tab.
pub fn validate_comment(text: &str) -> bool {
    text.chars().count() <= 4096
        && text
            .chars()
            .all(|c| !c.is_control() || matches!(c, '\n' | '\r' | '\t'))
}

/// Checks a solution, which follows the same rules as a comment: `//` notes
/// and line breaks are part of how solutions are written.
pub fn validate_solution(text: &str) -> bool {
    validate_comment(text)
}

/// Checks event-specific scramble notation, not whether a puzzle state is
/// reachable.
///
/// A scramble is required, may be at most 16384 bytes, and must not contain
/// control characters other than newline, carriage return and tab.
pub fn validate_scramble(text: &str, event: WcaEvent) -> bool {
    if text.trim().is_empty() || text.len() > 16_384 {
        return false;
    }
    if text
        .chars()
        .any(|c| c.is_control() && !matches!(c, '\n' | '\r' | '\t'))
    {
        return false;
    }
    if event == WcaEvent::Square1 {
        return is_square_one(text);
    }
    text.split_whitespace()
        .all(|token| is_valid_move(token, event))
}

fn is_valid_move(token: &str, event: WcaEvent) -> bool {
    let base = token
        .strip_suffix('\'')
        .or_else(|| token.strip_suffix('2'))
        .unwrap_or(token);
    match event {
        WcaEvent::Cube2x2
        | WcaEvent::Cube3x3
        | WcaEvent::Cube3x3OneHanded
        | WcaEvent::Cube4x4
        | WcaEvent::Cube5x5
        | WcaEvent::Cube6x6
        | WcaEvent::Cube7x7 => {
            let size = match event {
                WcaEvent::Cube2x2 => 2,
                WcaEvent::Cube3x3 | WcaEvent::Cube3x3OneHanded => 3,
                WcaEvent::Cube4x4 => 4,
                WcaEvent::Cube5x5 => 5,
                WcaEvent::Cube6x6 => 6,
                _ => 7,
            };
            if matches!(base, "R" | "L" | "U" | "D" | "F" | "B" | "x" | "y" | "z") {
                return true;
            }
            if matches!(base, "M" | "E" | "S") {
                return size >= 3;
            }
            if matches!(base, "r" | "l" | "u" | "d" | "f" | "b") {
                return size >= 3;
            }
            let Some(wide) = base.strip_suffix('w') else {
                return false;
            };
            let (layers, face) = if wide.starts_with(|c: char| c.is_ascii_digit()) {
                let split = wide
                    .find(|c: char| !c.is_ascii_digit())
                    .unwrap_or(wide.len());
                let Ok(layers) = wide[..split].parse::<u8>() else {
                    return false;
                };
                (layers, &wide[split..])
            } else {
                (2, wide)
            };
            layers >= 2 && layers < size && matches!(face, "R" | "L" | "U" | "D" | "F" | "B")
        }
        WcaEvent::Megaminx => matches!(token, "R++" | "R--" | "D++" | "D--" | "U" | "U'"),
        WcaEvent::Pyraminx => {
            !token.ends_with('2') && matches!(base, "R" | "L" | "U" | "B" | "r" | "l" | "u" | "b")
        }
        WcaEvent::Skewb => !token.ends_with('2') && matches!(base, "R" | "L" | "U" | "B"),
        WcaEvent::Fto => {
            !token.ends_with('2')
                && matches!(
                    base,
                    "R" | "L" | "U" | "D" | "F" | "B" | "BR" | "BL" | "Br" | "Bl"
                )
        }
        WcaEvent::Clock => is_clock_move(token),
        WcaEvent::Square1 => false,
    }
}

fn is_clock_move(token: &str) -> bool {
    if matches!(token, "y2" | "UR" | "DR" | "DL" | "UL") {
        return true;
    }
    ["ALL", "UR", "DR", "DL", "UL", "U", "R", "D", "L"]
        .iter()
        .any(|prefix| {
            let Some(turn) = token.strip_prefix(prefix) else {
                return false;
            };
            // Support WCA's UR3+ and the built-in generator's UR+3 notation.
            let digits = turn
                .strip_suffix(['+', '-'])
                .or_else(|| turn.strip_prefix(['+', '-']));
            digits.is_some_and(|digits| {
                digits.len() == 1 && matches!(digits.as_bytes()[0], b'0'..=b'6')
            })
        })
}

fn is_square_one(text: &str) -> bool {
    let compact: String = text.chars().filter(|c| !c.is_whitespace()).collect();
    let mut rest = compact.as_str();
    let mut previous_pair = false;
    let mut previous_slice = false;
    while !rest.is_empty() {
        if let Some(next) = rest.strip_prefix('/') {
            if previous_slice {
                return false;
            }
            previous_pair = false;
            previous_slice = true;
            rest = next;
        } else if let Some(pair) = rest.strip_prefix('(') {
            if previous_pair {
                return false;
            }
            let Some((numbers, next)) = pair.split_once(')') else {
                return false;
            };
            let Some((first, second)) = numbers.split_once(',') else {
                return false;
            };
            if ![first, second]
                .iter()
                .all(|n| n.parse::<i8>().is_ok_and(|n| (-5..=6).contains(&n)))
            {
                return false;
            }
            rest = next;
            previous_pair = true;
            previous_slice = false;
        } else {
            return false;
        }
    }
    true
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn durations_must_be_positive_unless_dnf_and_below_a_day() {
        assert!(validate_duration(1, Modifier::None));
        assert!(validate_duration(12_000, Modifier::PlusTwo));
        assert!(validate_duration(0, Modifier::DNF));
        assert!(!validate_duration(0, Modifier::None));
        assert!(!validate_duration(0, Modifier::PlusTwo));
        assert!(!validate_duration(86_400_000, Modifier::DNF));
        assert!(!validate_duration(86_400_001, Modifier::None));
    }

    #[test]
    fn dates_require_a_timezone_and_supported_precision() {
        assert_eq!(parse_date(""), Some(0));
        assert_eq!(parse_date("   "), Some(0));
        assert_eq!(
            parse_date("2026-09-29T14:30:00.123+02:00"),
            parse_date("2026-09-29T12:30:00.123Z")
        );
        for text in [
            "yesterday",
            "2026-02-30T12:00:00Z",
            "2026-09-29T12:00:00",
            "1960-01-01T00:00:00Z",
            "2026-09-29T12:00:00.1234Z",
        ] {
            assert_eq!(parse_date(text), None, "{text}");
        }
    }

    #[test]
    fn unknown_dates_format_blank_and_round_trip() {
        assert_eq!(format_rfc3339(0), "");
        assert_eq!(
            parse_date(&format_rfc3339(1_760_000_000_000)),
            Some(1_760_000_000_000)
        );
    }

    #[test]
    fn friendly_dates_cover_minute_precision_in_local_time() {
        assert_eq!(parse_date_input(""), Some(0));
        assert_eq!(parse_date_input("   "), Some(0));
        let minute = parse_date_input("2026-11-02 00:07").unwrap();
        assert_eq!(minute % 60_000, 0);
        assert_eq!(format_date_input(minute), "2026-11-02 00:07:00");
        assert_eq!(format_date_input(0), "");
        // Seconds are shown and round-trip.
        let with_seconds = parse_date_input("2026-11-02 00:07:32").unwrap();
        assert_eq!(with_seconds, minute + 32_000);
        assert_eq!(format_date_input(with_seconds), "2026-11-02 00:07:32");
        for text in [
            "yesterday",
            "2026-11-02",
            "00:07",
            "2026-02-30 12:00",
            "2026-13-01 00:00",
            "1900-01-01 00:00",
            "2026-11-02T00:07:32.275Z",
        ] {
            assert_eq!(parse_date_input(text), None, "{text}");
        }
    }

    #[test]
    fn comments_are_bounded_plain_text() {
        assert!(validate_comment(""));
        assert!(validate_comment("nice\nsecond line\ttab"));
        assert!(validate_comment(&"界".repeat(4096)));
        assert!(!validate_comment(&"界".repeat(4097)));
        assert!(!validate_comment("control\u{1b}"));
    }

    #[test]
    fn generated_scrambles_pass_validation_for_every_event() {
        for event in WcaEvent::ALL {
            let scramble = crate::scramble::generate(event);
            assert!(
                validate_scramble(&scramble, event),
                "{}: {}",
                event.name(),
                scramble.as_str()
            );
        }
    }

    #[test]
    fn notation_validation_is_event_specific() {
        let valid = [
            (WcaEvent::Cube3x3, "R U2 F'"),
            (WcaEvent::Cube4x4, "Rw U2 Fw'"),
            (WcaEvent::Cube7x7, "3Rw 3Uw2 B'"),
            (WcaEvent::Megaminx, "R++ D-- U'\nR-- D++ U"),
            (WcaEvent::Pyraminx, "R U' l b'"),
            (WcaEvent::Fto, "BR BL' U Br'"),
            (WcaEvent::Skewb, "R U B'"),
            (WcaEvent::Square1, "/ (1, -3) / (0,6)"),
            (WcaEvent::Clock, "UR3+ DR2- y2 ALL0+ UR"),
            (WcaEvent::Clock, "UR+3 DR-2 y2 ALL+0"),
        ];
        for (event, scramble) in valid {
            assert!(validate_scramble(scramble, event), "{scramble}");
        }
        let invalid = [
            (WcaEvent::Cube3x3, "R banana"),
            (WcaEvent::Cube3x3, "3Rw"),
            (WcaEvent::Cube4x4, "4Rw"),
            (WcaEvent::Cube3x3, "R22"),
            (WcaEvent::Cube3x3, "R\u{1b}"),
            (WcaEvent::Megaminx, "R+ D--"),
            (WcaEvent::Pyraminx, "R2"),
            (WcaEvent::Skewb, "l"),
            (WcaEvent::Square1, "(7,0) /"),
            (WcaEvent::Square1, "(1,0) (0,1)"),
            (WcaEvent::Square1, "(1,x) /"),
            (WcaEvent::Clock, "UR12+"),
            (WcaEvent::Clock, "R U F2"),
        ];
        for (event, scramble) in invalid {
            assert!(!validate_scramble(scramble, event), "{scramble}");
        }
        assert!(!validate_scramble("   ", WcaEvent::Cube3x3));
        assert!(!validate_scramble(&"R ".repeat(9000), WcaEvent::Cube3x3));
    }
}
