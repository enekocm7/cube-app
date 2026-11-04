//! Manual mode: typing in a time from another timer.
//!
//! Mirrors cube-tui's manual entry. The text accepts up to two leading
//! digits, an optional `:` with up to two more digits, and an optional `.`
//! with up to three more digits, so what can be typed is always a prefix of a
//! valid time. Parsing and the duration rules are the shared
//! [`parse_time`](crate::format::parse_time) and
//! [`validate_duration`](crate::validation::validate_duration).

use crate::format::parse_time;
use crate::solve::Modifier;
use crate::validation::validate_duration;

/// Why the typed text cannot be recorded.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ManualError {
    /// Nothing has been typed.
    Empty,
    /// The text is not a time, or is outside the accepted range.
    Invalid,
}

/// The time being typed in manual mode.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct ManualEntry {
    text: String,
    error: Option<ManualError>,
}

impl ManualEntry {
    /// The duration exactly as typed, before any penalty.
    pub fn text(&self) -> &str {
        &self.text
    }

    /// Why the last attempt to record failed, until the text changes.
    pub const fn error(&self) -> Option<ManualError> {
        self.error
    }

    /// Whether nothing has been typed.
    pub const fn is_empty(&self) -> bool {
        self.text.is_empty()
    }

    /// Adds typed or pasted text, skipping what cannot extend a valid time.
    /// Returns whether anything was added.
    pub fn push_text(&mut self, text: &str) -> bool {
        let mut changed = false;
        for character in text.chars() {
            changed |= self.push_char(character);
        }
        changed
    }

    fn push_char(&mut self, character: char) -> bool {
        if !self.accepts(character) {
            return false;
        }
        self.text.push(character);
        self.error = None;
        true
    }

    fn accepts(&self, character: char) -> bool {
        let text = &self.text;
        let colon = text.find(':');
        let dot = text.find('.');
        match character {
            digit if digit.is_ascii_digit() => {
                if let Some(dot) = dot {
                    text.len() - dot - 1 < 3
                } else if let Some(colon) = colon {
                    text.len() - colon - 1 < 2
                } else {
                    text.len() < 2
                }
            }
            ':' => colon.is_none() && dot.is_none() && !text.is_empty(),
            '.' => {
                dot.is_none()
                    && !text.is_empty()
                    && colon.is_none_or(|colon| text.len() > colon + 1)
            }
            _ => false,
        }
    }

    /// Removes the last character. Returns whether there was one.
    pub fn backspace(&mut self) -> bool {
        let changed = self.text.pop().is_some();
        if changed {
            self.error = None;
        }
        changed
    }

    /// Empties the text and any error, ready for the next solve.
    pub fn clear(&mut self) {
        self.text.clear();
        self.error = None;
    }

    /// The typed duration in milliseconds, if it can be recorded.
    ///
    /// # Errors
    ///
    /// [`ManualError::Empty`] when nothing is typed, and
    /// [`ManualError::Invalid`] when the text is not a valid duration.
    pub fn parse(&self) -> Result<u64, ManualError> {
        if self.text.is_empty() {
            return Err(ManualError::Empty);
        }
        let time_ms = parse_time(&self.text).ok_or(ManualError::Invalid)?;
        if validate_duration(time_ms, Modifier::None) {
            Ok(time_ms)
        } else {
            Err(ManualError::Invalid)
        }
    }

    /// Like [`parse`](Self::parse), but remembers a failure for display.
    pub fn commit(&mut self) -> Option<u64> {
        match self.parse() {
            Ok(time_ms) => Some(time_ms),
            Err(error) => {
                self.error = Some(error);
                None
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn typed(text: &str) -> ManualEntry {
        let mut entry = ManualEntry::default();
        entry.push_text(text);
        entry
    }

    #[test]
    fn each_part_is_capped() {
        assert_eq!(typed("123").text(), "12");
        assert_eq!(typed("12:345").text(), "12:34");
        assert_eq!(typed("12:34.5678").text(), "12:34.567");
        assert_eq!(typed("0.76").text(), "0.76");
        assert_eq!(typed("0.7689").text(), "0.768");
    }

    #[test]
    fn separators_need_something_before_them() {
        assert_eq!(typed(":").text(), "");
        assert_eq!(typed(".").text(), "");
        assert_eq!(typed("1::").text(), "1:");
        assert_eq!(typed("1:.").text(), "1:");
        assert_eq!(typed("1.2:").text(), "1.2");
        assert_eq!(typed("1.2.3").text(), "1.23");
        assert_eq!(typed("a-b 1").text(), "1");
    }

    #[test]
    fn pasting_skips_what_does_not_fit() {
        let mut entry = ManualEntry::default();
        assert!(entry.push_text(" 1:32.49 s"));
        assert_eq!(entry.text(), "1:32.49");
        assert!(!entry.push_text("x"));
    }

    #[test]
    fn backspace_edits_and_clears_the_error() {
        let mut entry = typed("12:99");
        assert_eq!(entry.commit(), None);
        assert_eq!(entry.error(), Some(ManualError::Invalid));
        assert!(entry.backspace());
        assert_eq!(entry.error(), None);
        assert_eq!(entry.text(), "12:9");
        let mut empty = ManualEntry::default();
        assert!(!empty.backspace());
    }

    #[test]
    fn short_formats_parse() {
        for (text, expected) in [
            ("0.5", 500),
            ("0.76", 760),
            ("8.98", 8_980),
            ("1:32.49", 92_490),
            ("8", 8_000),
            ("12:34", 754_000),
            ("12.345", 12_345),
        ] {
            assert_eq!(typed(text).parse(), Ok(expected), "{text}");
        }
    }

    #[test]
    fn unrecordable_input_is_reported() {
        assert_eq!(ManualEntry::default().parse(), Err(ManualError::Empty));
        assert_eq!(typed("12:99").parse(), Err(ManualError::Invalid));
        assert_eq!(typed("0").parse(), Err(ManualError::Invalid));
        assert_eq!(typed("0.0").parse(), Err(ManualError::Invalid));
        assert_eq!(typed("1:").parse(), Err(ManualError::Invalid));
        let mut entry = ManualEntry::default();
        assert_eq!(entry.commit(), None);
        assert_eq!(entry.error(), Some(ManualError::Empty));
        entry.clear();
        assert_eq!(entry.error(), None);
    }
}
