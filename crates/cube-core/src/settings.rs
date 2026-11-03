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

/// Which releases the update check looks at.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum UpdateChannel {
    /// Finished releases only.
    #[default]
    Stable,
    /// Also offer pre-releases.
    Prerelease,
}

/// How often the automatic update check runs.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum UpdateFrequency {
    /// On every launch.
    #[default]
    Start,
    /// At most once a day.
    Daily,
    /// At most once a week.
    Weekly,
}

/// Updater state and preferences.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default)]
pub struct UpdateSettings {
    /// Runs the update check automatically.
    pub auto_check: bool,
    /// Stable releases only, or pre-releases too.
    pub channel: UpdateChannel,
    /// How often the automatic check runs.
    pub frequency: UpdateFrequency,
    /// Unix timestamp of the last check (any check, manual or automatic).
    pub last_check: Option<i64>,
    /// Version the user chose to skip.
    pub skipped_version: Option<String>,
    /// Version that ran last, used to show "Updated to vX" after an update.
    pub last_run_version: Option<String>,
}

impl Default for UpdateSettings {
    fn default() -> Self {
        Self {
            auto_check: true,
            channel: UpdateChannel::default(),
            frequency: UpdateFrequency::default(),
            last_check: None,
            skipped_version: None,
            last_run_version: None,
        }
    }
}

impl UpdateSettings {
    /// Returns whether an automatic check is due at `now` (Unix seconds).
    pub fn check_due(&self, now: i64) -> bool {
        if !self.auto_check {
            return false;
        }
        let hours = match self.last_check {
            Some(last) => now.saturating_sub(last).max(0) / 3600,
            // Never checked: always due.
            None => return true,
        };
        match self.frequency {
            UpdateFrequency::Start => true,
            UpdateFrequency::Daily => hours >= 24,
            UpdateFrequency::Weekly => hours >= 24 * 7,
        }
    }
}

/// Persisted user preferences. Unknown keys are ignored, missing keys default.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default)]
#[allow(clippy::struct_excessive_bools, reason = "independent user toggles")]
pub struct Settings {
    pub inspection: bool,
    /// Plays the 8-second and 12-second inspection warnings.
    pub inspection_audio: bool,
    /// Inspection length in milliseconds.
    pub inspection_limit_ms: u64,
    /// How long the timer key must be held, in milliseconds.
    pub hold_ms: u64,
    pub theme: ThemePreference,
    /// A cube-tui theme file in the shared `themes` folder; empty uses `theme`.
    #[serde(skip_serializing_if = "String::is_empty")]
    pub theme_file: String,
    /// A language code; empty follows the system language.
    pub language: String,
    pub toasts: bool,
    /// Keeps the 3x3x3 solver searching for shorter scrambles, like `TNoodle`.
    pub accurate_3x3: bool,
    /// Adds a rolling ao5 column to the solve list.
    pub solve_list_ao5: bool,
    /// Whether the start hint has been used, so it is no longer shown while idle.
    pub start_hint_seen: bool,
    pub updates: UpdateSettings,
}

impl Default for Settings {
    fn default() -> Self {
        Self {
            inspection: true,
            inspection_audio: false,
            inspection_limit_ms: 15_000,
            hold_ms: DEFAULT_HOLD.as_millis() as u64,
            theme: ThemePreference::default(),
            theme_file: String::new(),
            language: String::new(),
            toasts: true,
            accurate_3x3: false,
            solve_list_ao5: false,
            start_hint_seen: false,
            updates: UpdateSettings::default(),
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
    fn inspection_audio_is_off_by_default() {
        assert!(!Settings::default().inspection_audio);
        let settings: Settings = toml::from_str("inspection_audio = true").unwrap();
        assert!(settings.inspection_audio);
    }

    #[test]
    fn settings_round_trip() {
        let mut settings = Settings {
            theme: ThemePreference::Dark,
            theme_file: "default.toml".into(),
            language: "es".into(),
            start_hint_seen: true,
            ..Settings::default()
        };
        settings.updates.frequency = UpdateFrequency::Weekly;
        settings.updates.skipped_version = Some("0.2.0".into());
        let text = toml::to_string(&settings).unwrap();
        assert_eq!(toml::from_str::<Settings>(&text).unwrap(), settings);
    }

    #[test]
    fn a_missing_theme_file_keeps_the_built_in_theme() {
        let settings: Settings = toml::from_str("theme = 'dark'").unwrap();
        assert_eq!(settings.theme_file, "");
        assert_eq!(settings.theme, ThemePreference::Dark);
    }

    #[test]
    fn an_empty_theme_file_is_left_out_of_the_saved_file() {
        let text = toml::to_string(&Settings::default()).unwrap();
        assert!(!text.contains("theme_file"));
    }

    #[test]
    fn update_settings_default_to_checking_every_launch() {
        let updates = Settings::default().updates;
        assert!(updates.auto_check);
        assert_eq!(updates.channel, UpdateChannel::Stable);
        assert_eq!(updates.frequency, UpdateFrequency::Start);
        assert!(updates.skipped_version.is_none());
        assert!(updates.check_due(0));
    }

    #[test]
    fn update_frequency_respects_the_last_check() {
        let mut updates = UpdateSettings {
            frequency: UpdateFrequency::Daily,
            last_check: Some(1_000_000),
            ..UpdateSettings::default()
        };
        assert!(!updates.check_due(1_000_000 + 23 * 3600));
        assert!(updates.check_due(1_000_000 + 24 * 3600));

        updates.frequency = UpdateFrequency::Weekly;
        assert!(!updates.check_due(1_000_000 + 6 * 24 * 3600));
        assert!(updates.check_due(1_000_000 + 7 * 24 * 3600));

        updates.frequency = UpdateFrequency::Start;
        assert!(updates.check_due(1_000_000));

        updates.auto_check = false;
        assert!(!updates.check_due(1_000_000));
    }

    #[test]
    fn old_settings_files_gain_the_updater_defaults() {
        let settings: Settings =
            toml::from_str("inspection = false\n[updates]\nauto_check = false\n").unwrap();
        assert!(!settings.inspection);
        assert!(!settings.updates.auto_check);
        assert_eq!(settings.updates.frequency, UpdateFrequency::Start);
    }
}
