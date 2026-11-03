//! Settings of the desktop app.
//!
//! These live in their own file: cube-tui rewrites `config.toml` from its own
//! schema, so sharing it would lose settings on both sides.

use std::time::Duration;

use serde::{Deserialize, Serialize};

use crate::history::History;
use crate::timer::{DEFAULT_HOLD, Timer};

/// The color scheme to use.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum ThemePreference {
    System,
    Light,
    #[default]
    Dark,
}

/// Persisted user preferences. Unknown keys are ignored, missing keys default.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default)]
#[allow(clippy::struct_excessive_bools, reason = "independent user toggles")]
pub struct Settings {
    pub inspection: bool,
    /// Inspection length in milliseconds.
    pub inspection_limit_ms: u64,
    /// How long the timer key must be held, in milliseconds.
    pub hold_ms: u64,
    pub theme: ThemePreference,
    /// A language code; empty follows the system language.
    pub language: String,
    pub toasts: bool,
    /// Keeps the 3x3x3 solver searching for shorter scrambles, like `TNoodle`.
    pub accurate_3x3: bool,
    /// Adds a rolling ao5 column to the solve list.
    pub solve_list_ao5: bool,
    /// Whether the start hint has been used, so it is no longer shown while idle.
    pub start_hint_seen: bool,
}

impl Default for Settings {
    fn default() -> Self {
        Self {
            inspection: true,
            inspection_limit_ms: 15_000,
            hold_ms: DEFAULT_HOLD.as_millis() as u64,
            theme: ThemePreference::default(),
            language: String::new(),
            toasts: true,
            accurate_3x3: false,
            solve_list_ao5: false,
            start_hint_seen: false,
        }
    }
}

impl Settings {
    /// Builds an idle timer configured from these settings.
    pub const fn timer(&self) -> Timer {
        Timer::new(
            self.inspection,
            Duration::from_millis(self.inspection_limit_ms),
            Duration::from_millis(self.hold_ms),
        )
    }

    /// Returns whether the idle timer should still show the start hint.
    pub const fn shows_start_hint(&self) -> bool {
        !self.start_hint_seen
    }

    /// Treats an existing history as having used the start hint, so users who
    /// upgrade from an older version or from cube-tui never see it again.
    pub fn adopt_existing_history(&mut self, sessions: &[History]) {
        if sessions.iter().any(|session| !session.is_empty()) {
            self.start_hint_seen = true;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::event::WcaEvent;
    use crate::solve::{Modifier, Solve};

    #[test]
    fn missing_and_unknown_keys_are_tolerated() {
        let settings: Settings = toml::from_str("inspection = false\nfuture = 1").unwrap();
        assert!(!settings.inspection);
        assert_eq!(settings.inspection_limit_ms, 15_000);
        assert!(settings.shows_start_hint());
    }

    #[test]
    fn existing_history_counts_as_seen() {
        let mut settings = Settings::default();
        settings.adopt_existing_history(&[History::new()]);
        assert!(settings.shows_start_hint());

        let mut history = History::new();
        history.push(Solve::new(
            10_000,
            WcaEvent::Cube3x3,
            String::new(),
            Modifier::None,
        ));
        settings.adopt_existing_history(&[History::new(), history]);
        assert!(!settings.shows_start_hint());
    }

    #[test]
    fn settings_round_trip() {
        let settings = Settings {
            theme: ThemePreference::Dark,
            language: "es".into(),
            start_hint_seen: true,
            ..Settings::default()
        };
        let text = toml::to_string(&settings).unwrap();
        assert_eq!(toml::from_str::<Settings>(&text).unwrap(), settings);
    }
}
