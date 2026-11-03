//! Settings of the desktop app.
//!
//! These live in their own file: cube-tui rewrites `config.toml` from its own
//! schema, so sharing it would lose settings on both sides.

use std::time::Duration;

use serde::{Deserialize, Serialize};

use crate::timer::{DEFAULT_HOLD, Timer};

/// The color scheme to use.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum ThemePreference {
    #[default]
    System,
    Light,
    Dark,
}

/// Persisted user preferences. Unknown keys are ignored, missing keys default.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default)]
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
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn missing_and_unknown_keys_are_tolerated() {
        let settings: Settings = toml::from_str("inspection = false\nfuture = 1").unwrap();
        assert!(!settings.inspection);
        assert_eq!(settings.inspection_limit_ms, 15_000);
    }

    #[test]
    fn settings_round_trip() {
        let settings = Settings {
            theme: ThemePreference::Dark,
            language: "es".into(),
            ..Settings::default()
        };
        let text = toml::to_string(&settings).unwrap();
        assert_eq!(toml::from_str::<Settings>(&text).unwrap(), settings);
    }
}
