//! Settings of the desktop app.
//!
//! These live in their own file: cube-tui rewrites `config.toml` from its own
//! schema, so sharing it would lose settings on both sides.

use std::collections::BTreeMap;
use std::path::PathBuf;
use std::time::Duration;

use serde::de::IntoDeserializer;
use serde::{Deserialize, Deserializer, Serialize};

use crate::event::WcaEvent;
use crate::history::History;
use crate::keybind::Keybinds;
use crate::panels::{DEFAULT_SIDEBAR_WIDTH, DEFAULT_STATS_WIDTH};
use crate::timer::{DEFAULT_HOLD, Timer};
use crate::window_state::WindowState;

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

/// Which text uses the bundled `Cube Mono` instead of the system fonts.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum FontChoice {
    /// The system fonts: the theme's interface font and the system's monospace
    /// for times. The bundled font is not loaded.
    System,
    /// `Cube Mono` for the timer's digits only.
    Timer,
    /// `Cube Mono` for every time, leaving the interface in the theme's font.
    #[default]
    Times,
    /// `Cube Mono` for the whole interface.
    Whole,
}

impl FontChoice {
    /// Every choice, in the order the settings menu lists them.
    pub const ALL: [Self; 4] = [Self::System, Self::Timer, Self::Times, Self::Whole];
}

/// How large the interface text is drawn.
///
/// The scale applies to text only: panels, dialogs and other layout dimensions
/// keep their pixel sizes, so the interface fits the window at every choice.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum TextSize {
    Small,
    #[default]
    Normal,
    Large,
    Largest,
    /// The size in `text_custom_pt`.
    Custom,
}

impl TextSize {
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
            Self::Small => 10,
            Self::Normal | Self::Custom => 12,
            Self::Large => 14,
            Self::Largest => 15,
        }
    }
}

/// How large the timer's digits are drawn.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum TimerSize {
    Small,
    #[default]
    Normal,
    Large,
    Largest,
    /// The size in `timer_custom_pt`.
    Custom,
}

impl TimerSize {
    /// Every size, in the order the settings menu lists them.
    pub const ALL: [Self; 5] = [
        Self::Small,
        Self::Normal,
        Self::Large,
        Self::Largest,
        Self::Custom,
    ];

    /// Digits size in points. `Custom` has no fixed size and reports the default.
    pub const fn points(self) -> u32 {
        match self {
            Self::Small => 63,
            Self::Normal | Self::Custom => 84,
            Self::Large => 105,
            Self::Largest => 126,
        }
    }
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

/// How large the scramble preview under the timer is drawn for an event.
///
/// Each event has its own size. A size above [`PreviewSize::largest_for`] the
/// event is not offered, and a stored one is lowered to it.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum PreviewSize {
    /// Smaller than the original.
    Small,
    /// The original size.
    #[default]
    Normal,
    Big,
    Huge,
}

impl PreviewSize {
    /// Every size, smallest first.
    pub const ALL: [Self; 4] = [Self::Small, Self::Normal, Self::Big, Self::Huge];

    /// The largest size offered for `event`: bigger drawings only help the
    /// small puzzles.
    pub const fn largest_for(event: WcaEvent) -> Self {
        match event {
            WcaEvent::Cube2x2 | WcaEvent::Cube3x3 | WcaEvent::Cube4x4 | WcaEvent::Square1 => {
                Self::Huge
            }
            WcaEvent::Fto => Self::Big,
            WcaEvent::Cube5x5
            | WcaEvent::Cube6x6
            | WcaEvent::Cube7x7
            | WcaEvent::Megaminx
            | WcaEvent::Pyraminx
            | WcaEvent::Skewb
            | WcaEvent::Clock => Self::Normal,
        }
    }

    /// The sizes offered for `event`, smallest first.
    pub fn offered_for(event: WcaEvent) -> impl Iterator<Item = Self> {
        let largest = Self::largest_for(event);
        Self::ALL.into_iter().filter(move |size| *size <= largest)
    }

    /// This size, lowered to the largest `event` offers.
    #[must_use]
    pub fn for_event(self, event: WcaEvent) -> Self {
        self.min(Self::largest_for(event))
    }

    /// The share of the window's height the preview may take.
    pub const fn height_share(self) -> f32 {
        match self {
            Self::Small => 0.18,
            Self::Normal => 0.28,
            Self::Big => 0.36,
            Self::Huge => 0.44,
        }
    }

    /// How many times its natural size the drawing may be scaled up. Never
    /// beyond [`crate::scramble::PREVIEW_SCALE`], the scale it is rasterized at.
    pub const fn max_scale(self) -> f32 {
        match self {
            Self::Small => 1.25,
            Self::Normal => 2.,
            Self::Big => 3.,
            Self::Huge => 4.,
        }
    }
}

/// Reads the per-event preview sizes leniently: an unknown event or size is
/// dropped instead of stopping the file from loading, a size above the event's
/// largest is lowered and `Normal` is not kept.
fn lenient_preview_sizes<'de, D>(
    deserializer: D,
) -> Result<BTreeMap<WcaEvent, PreviewSize>, D::Error>
where
    D: Deserializer<'de>,
{
    use serde::de::value::Error;

    let raw = BTreeMap::<String, toml::Value>::deserialize(deserializer)?;
    let mut sizes = BTreeMap::new();
    for (name, value) in raw {
        let event =
            WcaEvent::deserialize(IntoDeserializer::<Error>::into_deserializer(name.as_str())).ok();
        let size = value.as_str().and_then(|text| {
            PreviewSize::deserialize(IntoDeserializer::<Error>::into_deserializer(text)).ok()
        });
        if let (Some(event), Some(size)) = (event, size) {
            let size = size.for_event(event);
            if size != PreviewSize::Normal {
                sizes.insert(event, size);
            }
        }
    }
    Ok(sizes)
}

/// Smallest custom text size, in points.
pub const MIN_TEXT_PT: u32 = 8;
/// Largest custom text size, in points.
pub const MAX_TEXT_PT: u32 = 24;

/// Smallest custom timer size, in points.
pub const MIN_TIMER_PT: u32 = 24;
/// Largest custom timer size, in points.
pub const MAX_TIMER_PT: u32 = 240;

/// Smallest custom scramble size, in points.
pub const MIN_SCRAMBLE_PT: u32 = 6;
/// Largest custom scramble size, in points.
pub const MAX_SCRAMBLE_PT: u32 = 120;

/// Earlier solves a best needs by default before it counts.
pub const DEFAULT_BEST_MIN_SOLVES: u32 = 100;
/// The most earlier solves a best can be set to need.
pub const MAX_BEST_MIN_SOLVES: u32 = 100_000;

/// A custom size that is not a whole number within its range.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct InvalidSizePt;

/// Parses a size typed by the user. Blank means "use the preset" (`Ok(None)`);
/// anything else must be a whole number of points within `min`..=`max`.
fn parse_size_pt(input: &str, min: u32, max: u32) -> Result<Option<u32>, InvalidSizePt> {
    let input = input.trim().trim_end_matches("pt").trim();
    if input.is_empty() {
        return Ok(None);
    }
    match input.parse::<u32>() {
        Ok(points) if (min..=max).contains(&points) => Ok(Some(points)),
        _ => Err(InvalidSizePt),
    }
}

/// Parses a custom text size typed by the user.
pub fn parse_text_pt(input: &str) -> Result<Option<u32>, InvalidSizePt> {
    parse_size_pt(input, MIN_TEXT_PT, MAX_TEXT_PT)
}

/// Parses a custom timer size typed by the user.
pub fn parse_timer_pt(input: &str) -> Result<Option<u32>, InvalidSizePt> {
    parse_size_pt(input, MIN_TIMER_PT, MAX_TIMER_PT)
}

/// Parses a custom scramble size typed by the user.
pub fn parse_scramble_pt(input: &str) -> Result<Option<u32>, InvalidSizePt> {
    parse_size_pt(input, MIN_SCRAMBLE_PT, MAX_SCRAMBLE_PT)
}

/// Parses the earlier solves a best needs, as typed by the user. Blank means
/// "use the default" (`Ok(None)`).
pub fn parse_best_min_solves(input: &str) -> Result<Option<u32>, InvalidSizePt> {
    parse_size_pt(input, 0, MAX_BEST_MIN_SOLVES)
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
    /// Flashes the digits when a solve sets a personal best.
    pub celebrate_bests: bool,
    /// Plays a sound when a solve sets a personal best.
    pub celebrate_sound: bool,
    /// Solves of the event a session, or the whole history, must already hold
    /// before a best in it is celebrated or announced.
    pub best_min_solves: u32,
    /// Holds a steady color instead of flashing. GPUI does not report the
    /// system's reduced motion preference, so this stands in for it.
    pub reduce_motion: bool,
    /// Touching the digits starts and stops the timer like the timer key.
    pub touch_to_start: bool,
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
    /// Which text uses the bundled `Cube Mono`.
    pub font: FontChoice,
    /// How large the interface text is drawn.
    pub text_size: TextSize,
    /// The text size in points used when `text_size` is `custom`.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub text_custom_pt: Option<u32>,
    /// How large the timer's digits are drawn.
    pub timer_size: TimerSize,
    /// The timer size in points used when `timer_size` is `custom`.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub timer_custom_pt: Option<u32>,
    /// How large the scramble is drawn.
    pub scramble_size: ScrambleSize,
    /// Draws the puzzle as the scramble leaves it, on the timer and in the
    /// solve details.
    pub scramble_preview: bool,
    /// The preview size of each event that differs from `Normal`.
    #[serde(
        deserialize_with = "lenient_preview_sizes",
        skip_serializing_if = "BTreeMap::is_empty"
    )]
    pub preview_sizes: BTreeMap<WcaEvent, PreviewSize>,
    /// The scramble size in points used when `scramble_size` is `custom`.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub scramble_custom_pt: Option<u32>,
    /// Whether the start hint has been used, so it is no longer shown while idle.
    pub start_hint_seen: bool,
    /// The tags the next solves get. They stay until changed.
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub active_tags: Vec<String>,
    /// A folder for backups of the user's own, on top of the app's; empty for
    /// none.
    pub user_backup_dir: String,
    /// Action name to keystroke overrides; see [`crate::keybind`].
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub keybindings: BTreeMap<String, String>,
    /// Width of the session and solve list, in pixels. Clamped to the window
    /// when drawn; see [`crate::panels`].
    pub sidebar_width: u32,
    /// Width of the statistics panel, in pixels. Clamped to the window when drawn.
    pub stats_panel_width: u32,
    /// Where the main window was when it last changed; unset opens it centered.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub window: Option<WindowState>,
    pub updates: UpdateSettings,
}

impl Settings {
    /// Returns the folder for the user's own backups, if they chose one.
    pub fn user_backups(&self) -> Option<PathBuf> {
        (!self.user_backup_dir.is_empty()).then(|| PathBuf::from(&self.user_backup_dir))
    }
}

impl Default for Settings {
    fn default() -> Self {
        Self {
            inspection: true,
            inspection_audio: false,
            celebrate_bests: true,
            celebrate_sound: false,
            best_min_solves: DEFAULT_BEST_MIN_SOLVES,
            reduce_motion: false,
            touch_to_start: false,
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
            text_size: TextSize::default(),
            text_custom_pt: None,
            timer_size: TimerSize::default(),
            timer_custom_pt: None,
            scramble_size: ScrambleSize::default(),
            scramble_preview: true,
            preview_sizes: BTreeMap::new(),
            scramble_custom_pt: None,
            start_hint_seen: false,
            active_tags: Vec::new(),
            user_backup_dir: String::new(),
            keybindings: BTreeMap::new(),
            sidebar_width: DEFAULT_SIDEBAR_WIDTH,
            stats_panel_width: DEFAULT_STATS_WIDTH,
            window: None,
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

    /// Text size in points: the custom size when chosen, else the preset.
    pub fn text_pt(&self) -> u32 {
        match (self.text_size, self.text_custom_pt) {
            (TextSize::Custom, Some(pt)) => pt.clamp(MIN_TEXT_PT, MAX_TEXT_PT),
            (size, _) => size.points(),
        }
    }

    /// The size of one rem in pixels for the chosen text size (1pt = 4/3px).
    pub fn rem_px(&self) -> f32 {
        self.text_pt() as f32 * 4.0 / 3.0
    }

    /// How many times larger the text is than the normal size.
    pub fn text_scale(&self) -> f32 {
        self.text_pt() as f32 / TextSize::Normal.points() as f32
    }

    /// Timer size in points: the custom size when chosen, else the preset.
    pub fn timer_pt(&self) -> u32 {
        match (self.timer_size, self.timer_custom_pt) {
            (TimerSize::Custom, Some(pt)) => pt.clamp(MIN_TIMER_PT, MAX_TIMER_PT),
            (size, _) => size.points(),
        }
    }

    /// The timer digits' font size in pixels (1pt = 4/3px).
    pub fn timer_digits_px(&self) -> f32 {
        self.timer_pt() as f32 * 4.0 / 3.0
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

    /// The preview size for `event`: `Normal` unless chosen, never above what
    /// the event offers.
    pub fn preview_size(&self, event: WcaEvent) -> PreviewSize {
        self.preview_sizes
            .get(&event)
            .copied()
            .unwrap_or_default()
            .for_event(event)
    }

    /// Sets the preview size for `event` only, keeping `Normal` implicit.
    pub fn set_preview_size(&mut self, event: WcaEvent, size: PreviewSize) {
        let size = size.for_event(event);
        if size == PreviewSize::Normal {
            self.preview_sizes.remove(&event);
        } else {
            self.preview_sizes.insert(event, size);
        }
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
    fn the_active_tags_survive_a_round_trip_and_default_to_none() {
        assert_eq!(Settings::default().active_tags, Vec::<String>::new());
        let settings = Settings {
            active_tags: vec!["GAN".to_owned(), "warm-up".to_owned()],
            ..Settings::default()
        };
        let text = toml::to_string(&settings).unwrap();
        let loaded: Settings = toml::from_str(&text).unwrap();
        assert_eq!(loaded.active_tags, settings.active_tags);
    }

    #[test]
    fn preview_sizes_default_to_normal_and_stay_minimal() {
        let settings = Settings::default();
        for event in WcaEvent::ALL {
            assert_eq!(settings.preview_size(event), PreviewSize::Normal);
        }
        assert!(
            !toml::to_string(&settings)
                .unwrap()
                .contains("preview_sizes")
        );
    }

    #[test]
    fn each_event_offers_sizes_up_to_its_largest() {
        use WcaEvent::*;
        for event in [Cube2x2, Cube3x3, Cube4x4, Square1] {
            let offered: Vec<_> = PreviewSize::offered_for(event).collect();
            assert_eq!(offered, PreviewSize::ALL, "{event:?}");
        }
        let fto: Vec<_> = PreviewSize::offered_for(Fto).collect();
        assert_eq!(
            fto,
            [PreviewSize::Small, PreviewSize::Normal, PreviewSize::Big]
        );
        for event in [Cube5x5, Cube6x6, Cube7x7, Megaminx, Pyraminx, Skewb, Clock] {
            let offered: Vec<_> = PreviewSize::offered_for(event).collect();
            assert_eq!(
                offered,
                [PreviewSize::Small, PreviewSize::Normal],
                "{event:?}"
            );
        }
    }

    #[test]
    fn sizes_grow_and_stay_within_the_raster_scale() {
        for pair in PreviewSize::ALL.windows(2) {
            assert!(pair[0].height_share() < pair[1].height_share());
            assert!(pair[0].max_scale() < pair[1].max_scale());
        }
        let scale = crate::scramble::PREVIEW_SCALE as f32;
        assert!(PreviewSize::Huge.max_scale() <= scale);
        assert!((PreviewSize::Normal.height_share() - 0.28).abs() < f32::EPSILON);
        assert!((PreviewSize::Normal.max_scale() - 2.).abs() < f32::EPSILON);
    }

    #[test]
    fn changing_one_events_preview_size_leaves_the_others() {
        let mut settings = Settings::default();
        settings.set_preview_size(WcaEvent::Cube2x2, PreviewSize::Huge);
        settings.set_preview_size(WcaEvent::Cube7x7, PreviewSize::Small);
        assert_eq!(settings.preview_size(WcaEvent::Cube2x2), PreviewSize::Huge);
        assert_eq!(settings.preview_size(WcaEvent::Cube7x7), PreviewSize::Small);
        assert_eq!(
            settings.preview_size(WcaEvent::Cube3x3),
            PreviewSize::Normal
        );
        settings.set_preview_size(WcaEvent::Cube2x2, PreviewSize::Normal);
        assert_eq!(settings.preview_sizes.len(), 1);
        // A size the event does not offer is lowered.
        settings.set_preview_size(WcaEvent::Fto, PreviewSize::Huge);
        assert_eq!(settings.preview_size(WcaEvent::Fto), PreviewSize::Big);
    }

    #[test]
    fn preview_sizes_round_trip_and_older_files_load() {
        let old: Settings = toml::from_str("inspection = false").unwrap();
        assert!(old.preview_sizes.is_empty());
        let mut settings = Settings::default();
        settings.set_preview_size(WcaEvent::Cube4x4, PreviewSize::Big);
        let text = toml::to_string(&settings).unwrap();
        let loaded: Settings = toml::from_str(&text).unwrap();
        assert_eq!(loaded.preview_sizes, settings.preview_sizes);
    }

    #[test]
    fn stored_preview_sizes_are_lowered_and_bad_ones_ignored() {
        let text = "[preview_sizes]\nCube7x7 = 'huge'\nCube2x2 = 'big'\nCube3x3 = 'normal'\nCube4x4 = 'gigantic'\nFuture = 'small'\nSkewb = 3\nFto = 'huge'\n";
        let loaded: Settings = toml::from_str(text).unwrap();
        assert_eq!(loaded.preview_size(WcaEvent::Cube7x7), PreviewSize::Normal);
        assert_eq!(loaded.preview_size(WcaEvent::Cube2x2), PreviewSize::Big);
        assert_eq!(loaded.preview_size(WcaEvent::Fto), PreviewSize::Big);
        assert_eq!(loaded.preview_size(WcaEvent::Cube4x4), PreviewSize::Normal);
        assert_eq!(loaded.preview_sizes.len(), 2);
    }

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
    fn scramble_preview_is_on_by_default_and_can_be_hidden() {
        assert!(Settings::default().scramble_preview);
        let settings: Settings = toml::from_str("scramble_preview = false").unwrap();
        assert!(!settings.scramble_preview);
        let settings: Settings = toml::from_str("inspection = false").unwrap();
        assert!(settings.scramble_preview);
    }

    #[test]
    fn older_files_get_the_celebration_defaults() {
        let settings: Settings = toml::from_str("inspection = false").unwrap();
        assert!(settings.celebrate_bests);
        assert!(!settings.celebrate_sound);
        assert_eq!(settings.best_min_solves, DEFAULT_BEST_MIN_SOLVES);
        assert!(!settings.reduce_motion);
    }

    #[test]
    fn the_best_minimum_takes_blank_zero_and_its_maximum_only() {
        assert_eq!(parse_best_min_solves(" "), Ok(None));
        assert_eq!(parse_best_min_solves("0"), Ok(Some(0)));
        assert_eq!(parse_best_min_solves("250"), Ok(Some(250)));
        assert_eq!(
            parse_best_min_solves("100000"),
            Ok(Some(MAX_BEST_MIN_SOLVES))
        );
        assert_eq!(parse_best_min_solves("100001"), Err(InvalidSizePt));
        assert_eq!(parse_best_min_solves("-1"), Err(InvalidSizePt));
        assert_eq!(parse_best_min_solves("many"), Err(InvalidSizePt));
    }

    #[test]
    fn inspection_audio_is_off_by_default() {
        assert!(!Settings::default().inspection_audio);
        let settings: Settings = toml::from_str("inspection_audio = true").unwrap();
        assert!(settings.inspection_audio);
    }

    #[test]
    fn touch_to_start_is_off_by_default_and_round_trips() {
        assert!(!Settings::default().touch_to_start);
        let settings: Settings = toml::from_str("inspection = false").unwrap();
        assert!(!settings.touch_to_start);
        let on = Settings {
            touch_to_start: true,
            ..Settings::default()
        };
        let text = toml::to_string(&on).unwrap();
        assert!(toml::from_str::<Settings>(&text).unwrap().touch_to_start);
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
    fn font_choices_default_to_every_time() {
        assert_eq!(Settings::default().font, FontChoice::Times);
        let settings: Settings = toml::from_str("font = 'system'").unwrap();
        assert_eq!(settings.font, FontChoice::System);
        let settings: Settings = toml::from_str("font = 'timer'").unwrap();
        assert_eq!(settings.font, FontChoice::Timer);
        let settings: Settings = toml::from_str("font = 'whole'").unwrap();
        assert_eq!(settings.font, FontChoice::Whole);
    }

    #[test]
    fn text_size_defaults_to_normal_and_scales_in_order() {
        let mut settings = Settings::default();
        assert_eq!(settings.text_size, TextSize::Normal);
        assert!((settings.rem_px() - 16.).abs() < f32::EPSILON);
        assert!((settings.text_scale() - 1.).abs() < f32::EPSILON);

        let sizes: Vec<u32> = TextSize::ALL[..4]
            .iter()
            .map(|size| size.points())
            .collect();
        assert!(sizes.windows(2).all(|pair| pair[0] < pair[1]));

        let parsed: Settings = toml::from_str("text_size = 'largest'").unwrap();
        assert_eq!(parsed.text_size, TextSize::Largest);
        assert!((parsed.rem_px() - 20.).abs() < f32::EPSILON);

        settings.text_custom_pt = Some(18);
        assert_eq!(settings.text_pt(), 12, "ignored unless custom is chosen");
        settings.text_size = TextSize::Custom;
        assert_eq!(settings.text_pt(), 18);
        assert!((settings.rem_px() - 24.).abs() < f32::EPSILON);
        settings.text_custom_pt = Some(1000);
        assert_eq!(settings.text_pt(), MAX_TEXT_PT);
    }

    #[test]
    fn timer_size_defaults_to_its_original_digits() {
        let mut settings = Settings::default();
        assert_eq!(settings.timer_size, TimerSize::Normal);
        assert!((settings.timer_digits_px() - 112.).abs() < f32::EPSILON);

        let sizes: Vec<u32> = TimerSize::ALL[..4]
            .iter()
            .map(|size| size.points())
            .collect();
        assert!(sizes.windows(2).all(|pair| pair[0] < pair[1]));

        let parsed: Settings = toml::from_str("timer_size = 'large'").unwrap();
        assert_eq!(parsed.timer_size, TimerSize::Large);

        settings.timer_custom_pt = Some(90);
        assert_eq!(settings.timer_pt(), 84, "ignored unless custom is chosen");
        settings.timer_size = TimerSize::Custom;
        assert_eq!(settings.timer_pt(), 90);
        assert!((settings.timer_digits_px() - 120.).abs() < f32::EPSILON);
    }

    #[test]
    fn custom_text_and_timer_sizes_are_parsed_strictly() {
        assert_eq!(parse_text_pt(""), Ok(None));
        assert_eq!(parse_text_pt("18pt"), Ok(Some(18)));
        assert!(parse_text_pt("7").is_err());
        assert!(parse_text_pt("25").is_err());
        assert!(parse_text_pt("abc").is_err());

        assert_eq!(parse_timer_pt(" 120 "), Ok(Some(120)));
        assert!(parse_timer_pt("23").is_err());
        assert!(parse_timer_pt("241").is_err());
    }

    #[test]
    fn old_settings_files_keep_the_original_text_and_timer_sizes() {
        let settings: Settings = toml::from_str("inspection = false").unwrap();
        assert_eq!(settings.text_size, TextSize::Normal);
        assert_eq!(settings.timer_size, TimerSize::Normal);
        assert_eq!(settings.text_custom_pt, None);
        assert_eq!(settings.timer_custom_pt, None);
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
            text_size: TextSize::Custom,
            text_custom_pt: Some(18),
            timer_size: TimerSize::Custom,
            timer_custom_pt: Some(90),
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
    fn the_window_state_round_trips_and_is_optional() {
        assert_eq!(Settings::default().window, None);
        let window = WindowState {
            x: -20,
            y: 40,
            width: 1000,
            height: 700,
            maximized: true,
        };
        let settings = Settings {
            window: Some(window),
            ..Settings::default()
        };
        let text = toml::to_string(&settings).unwrap();
        let loaded: Settings = toml::from_str(&text).unwrap();
        assert_eq!(loaded.window, Some(window));
        assert!(
            !toml::to_string(&Settings::default())
                .unwrap()
                .contains("window")
        );
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
