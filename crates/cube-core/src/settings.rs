//! Settings of the desktop app.
//!
//! These live in their own file: cube-tui rewrites `config.toml` from its own
//! schema, so sharing it would lose settings on both sides.

use std::collections::BTreeMap;
use std::time::Duration;

use serde::{Deserialize, Serialize};

use crate::history::History;
use crate::keybind::Keybinds;
use crate::panels::{DEFAULT_SIDEBAR_WIDTH, DEFAULT_STATS_WIDTH};
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

/// Which text uses the bundled `JetBrains Mono` instead of the system fonts.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum FontChoice {
    /// The system fonts: the theme's interface font and the system's monospace
    /// for times. The bundled font is not loaded.
    #[default]
    System,
    /// `JetBrains Mono` for the timer's digits only.
    Timer,
    /// `JetBrains Mono` for every time, leaving the interface in the theme's
    /// font.
    Times,
    /// `JetBrains Mono` for the whole interface.
    Whole,
}

impl FontChoice {
    /// Every choice, in the order the settings menu lists them.
    pub const ALL: [Self; 4] = [Self::System, Self::Timer, Self::Times, Self::Whole];
}

/// How large the scramble is drawn.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum ScrambleSize {
    /// The original size.
    Small,
    #[default]
    Normal,
    Large,
    /// For reading the scramble from across the room.
    Largest,
    /// The size in `scramble_custom_pt`.
    Custom,
}

impl ScrambleSize {
    /// Every size, in the order the settings menu lists them.
    pub const ALL: [Self; 5] = [
        Self::Small,
        Self::Normal,
        Self::Large,
        Self::Largest,
        Self::Custom,
    ];

    /// Text size in points. `Custom` has no fixed size and reports the default.
    pub const fn points(self) -> u32 {
        match self {
            Self::Small => 12,
            Self::Normal | Self::Custom => 16,
            Self::Large => 22,
            Self::Largest => 30,
        }
    }
}

/// Smallest custom scramble size, in points.
pub const MIN_SCRAMBLE_PT: u32 = 6;
/// Largest custom scramble size, in points.
pub const MAX_SCRAMBLE_PT: u32 = 120;

/// A custom scramble size that is not a whole number within range.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct InvalidScramblePt;

/// Parses a custom scramble size typed by the user. Blank means "use the
/// preset" (`Ok(None)`); anything else must be a whole number of points within
/// [`MIN_SCRAMBLE_PT`]..=[`MAX_SCRAMBLE_PT`].
pub fn parse_scramble_pt(input: &str) -> Result<Option<u32>, InvalidScramblePt> {
    let input = input.trim().trim_end_matches("pt").trim();
    if input.is_empty() {
        return Ok(None);
    }
    match input.parse::<u32>() {
        Ok(points) if (MIN_SCRAMBLE_PT..=MAX_SCRAMBLE_PT).contains(&points) => Ok(Some(points)),
        _ => Err(InvalidScramblePt),
    }
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
    /// Asks before deleting a solve or session; Undo works either way.
    pub confirm_delete: bool,
    /// Keeps the 3x3x3 solver searching for shorter scrambles, like `TNoodle`.
    pub accurate_3x3: bool,
    /// Adds a rolling ao5 column to the solve list.
    pub solve_list_ao5: bool,
    /// Keeps the solve list scrollbar visible instead of only while scrolling.
    pub always_show_scrollbar: bool,
    /// Which text uses the bundled `JetBrains Mono`.
    pub font: FontChoice,
    /// How large the scramble is drawn.
    pub scramble_size: ScrambleSize,
    /// The scramble size in points used when `scramble_size` is `custom`.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub scramble_custom_pt: Option<u32>,
    /// Whether the start hint has been used, so it is no longer shown while idle.
    pub start_hint_seen: bool,
    /// Action name to keystroke overrides; see [`crate::keybind`].
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub keybindings: BTreeMap<String, String>,
    /// Width of the session and solve list, in pixels. Clamped to the window
    /// when drawn; see [`crate::panels`].
    pub sidebar_width: u32,
    /// Width of the statistics panel, in pixels. Clamped to the window when drawn.
    pub stats_panel_width: u32,
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
            confirm_delete: true,
            accurate_3x3: false,
            solve_list_ao5: false,
            always_show_scrollbar: false,
            font: FontChoice::default(),
            scramble_size: ScrambleSize::default(),
            scramble_custom_pt: None,
            start_hint_seen: false,
            keybindings: BTreeMap::new(),
            sidebar_width: DEFAULT_SIDEBAR_WIDTH,
            stats_panel_width: DEFAULT_STATS_WIDTH,
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

    /// Scramble text size in points: the custom size when chosen, else the preset.
    pub fn scramble_pt(&self) -> u32 {
        match (self.scramble_size, self.scramble_custom_pt) {
            (ScrambleSize::Custom, Some(pt)) => pt.clamp(MIN_SCRAMBLE_PT, MAX_SCRAMBLE_PT),
            (size, _) => size.points(),
        }
    }

    /// Scramble text size in pixels (1pt = 4/3px).
    pub fn scramble_font_px(&self) -> f32 {
        self.scramble_pt() as f32 * 4.0 / 3.0
    }

    /// Scramble line height in pixels, scaled with the text.
    pub fn scramble_line_px(&self) -> f32 {
        self.scramble_font_px() * 1.5
    }

    /// Builds the effective key bindings, defaults plus configured overrides.
    pub fn keybinds(&self) -> Keybinds {
        Keybinds::from_overrides(&self.keybindings)
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
    fn scramble_size_defaults_and_scales_lines() {
        let mut settings = Settings::default();
        assert_eq!(settings.scramble_size, ScrambleSize::Normal);
        assert_eq!(settings.scramble_pt(), 16);
        let parsed: Settings = toml::from_str("scramble_size = 'largest'").unwrap();
        assert_eq!(parsed.scramble_size, ScrambleSize::Largest);
        let sizes: Vec<u32> = ScrambleSize::ALL[..4]
            .iter()
            .map(|size| size.points())
            .collect();
        assert!(sizes.windows(2).all(|pair| pair[0] < pair[1]));

        settings.scramble_custom_pt = Some(50);
        assert_eq!(
            settings.scramble_pt(),
            16,
            "ignored unless custom is chosen"
        );
        settings.scramble_size = ScrambleSize::Custom;
        assert_eq!(settings.scramble_pt(), 50);
        assert!((settings.scramble_line_px() - 100.0).abs() < 1e-3);
        settings.scramble_custom_pt = Some(1000);
        assert_eq!(settings.scramble_pt(), MAX_SCRAMBLE_PT);
    }

    #[test]
    fn custom_scramble_size_is_parsed_strictly() {
        assert_eq!(parse_scramble_pt(""), Ok(None));
        assert_eq!(parse_scramble_pt(" 24 "), Ok(Some(24)));
        assert_eq!(parse_scramble_pt("24pt"), Ok(Some(24)));
        assert!(parse_scramble_pt("5").is_err());
        assert!(parse_scramble_pt("121").is_err());
        assert!(parse_scramble_pt("abc").is_err());
        assert!(parse_scramble_pt("-3").is_err());
    }

    #[test]
    fn font_choices_default_to_the_system_font() {
        assert_eq!(Settings::default().font, FontChoice::System);
        let settings: Settings = toml::from_str("font = 'timer'").unwrap();
        assert_eq!(settings.font, FontChoice::Timer);
        let settings: Settings = toml::from_str("font = 'whole'").unwrap();
        assert_eq!(settings.font, FontChoice::Whole);
    }

    #[test]
    fn the_scrollbar_is_transient_by_default_and_round_trips() {
        assert!(!Settings::default().always_show_scrollbar);
        let settings: Settings = toml::from_str("").unwrap();
        assert!(!settings.always_show_scrollbar);
        let on = Settings {
            always_show_scrollbar: true,
            ..Settings::default()
        };
        let text = toml::to_string(&on).unwrap();
        assert!(
            toml::from_str::<Settings>(&text)
                .unwrap()
                .always_show_scrollbar
        );
    }

    #[test]
    fn settings_round_trip() {
        let mut settings = Settings {
            theme: ThemePreference::Dark,
            theme_file: "default.toml".into(),
            language: "es".into(),
            start_hint_seen: true,
            keybindings: BTreeMap::from([("next_scramble".into(), "Ctrl+N".into())]),
            ..Settings::default()
        };
        settings.updates.frequency = UpdateFrequency::Weekly;
        settings.updates.skipped_version = Some("0.2.0".into());
        let text = toml::to_string(&settings).unwrap();
        assert_eq!(toml::from_str::<Settings>(&text).unwrap(), settings);
    }

    #[test]
    fn keybinding_overrides_tolerate_unknown_and_broken_entries() {
        let settings: Settings = toml::from_str(
            "[keybindings]\nnext_scramble = \"Ctrl+N\"\nnot_an_action = \"x\"\ntimer = \"Ctrl+Space\"",
        )
        .unwrap();
        let keybinds = settings.keybinds();
        assert_eq!(
            keybinds.label(crate::Action::NextScramble).as_deref(),
            Some("Ctrl + N")
        );
        assert_eq!(
            keybinds.label(crate::Action::Timer).as_deref(),
            Some("Space")
        );
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
    fn panel_widths_default_and_survive_a_round_trip() {
        let defaults = Settings::default();
        assert_eq!(defaults.sidebar_width, DEFAULT_SIDEBAR_WIDTH);
        assert_eq!(defaults.stats_panel_width, DEFAULT_STATS_WIDTH);

        let settings = Settings {
            sidebar_width: 420,
            stats_panel_width: 310,
            ..Settings::default()
        };
        let text = toml::to_string(&settings).unwrap();
        let loaded: Settings = toml::from_str(&text).unwrap();
        assert_eq!(loaded.sidebar_width, 420);
        assert_eq!(loaded.stats_panel_width, 310);
    }

    #[test]
    fn settings_files_without_panel_widths_keep_the_defaults() {
        let settings: Settings = toml::from_str("inspection = false\nhold_ms = 300").unwrap();
        assert_eq!(settings.sidebar_width, DEFAULT_SIDEBAR_WIDTH);
        assert_eq!(settings.stats_panel_width, DEFAULT_STATS_WIDTH);
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
