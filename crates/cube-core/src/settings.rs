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
            WcaEvent::Cube2x2
            | WcaEvent::Cube3x3
            | WcaEvent::Cube3x3OneHanded
            | WcaEvent::Cube3x3Blind
            | WcaEvent::Cube4x4
            | WcaEvent::Cube4x4Blind
            | WcaEvent::Square1 => Self::Huge,
            WcaEvent::Fto => Self::Big,
            WcaEvent::Cube5x5
            | WcaEvent::Cube5x5Blind
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

/// How large the library's cases are drawn.
///
/// The scale applies to the case rows, their thumbnails, the detail drawing and
/// the panels around them. Every dimension it scales is in rems, so it also
/// follows the text size.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum LibrarySize {
    Small,
    #[default]
    Normal,
    Big,
}

impl LibrarySize {
    /// Every size, in the order the settings menu lists them.
    pub const ALL: [Self; 3] = [Self::Small, Self::Normal, Self::Big];

    /// How many times the original dimensions are drawn.
    pub const fn scale(self) -> f32 {
        match self {
            Self::Small => 1.,
            Self::Normal => 1.2,
            Self::Big => 1.45,
        }
    }
}

/// Reads a per-event map leniently: an unknown event or a value that does not
/// parse is dropped instead of stopping the file from loading.
fn lenient_event_map<'de, D, V>(deserializer: D) -> Result<BTreeMap<WcaEvent, V>, D::Error>
where
    D: Deserializer<'de>,
    V: serde::de::DeserializeOwned,
{
    use serde::de::value::Error;

    let raw = BTreeMap::<String, toml::Value>::deserialize(deserializer)?;
    Ok(raw
        .into_iter()
        .filter_map(|(name, value)| {
            let event =
                WcaEvent::deserialize(IntoDeserializer::<Error>::into_deserializer(name.as_str()))
                    .ok()?;
            Some((event, value.try_into::<V>().ok()?))
        })
        .collect())
}

/// Reads the per-event preview sizes: a size above the event's largest is
/// lowered and `Normal` is not kept.
fn lenient_preview_sizes<'de, D>(
    deserializer: D,
) -> Result<BTreeMap<WcaEvent, PreviewSize>, D::Error>
where
    D: Deserializer<'de>,
{
    let mut sizes: BTreeMap<WcaEvent, PreviewSize> = lenient_event_map(deserializer)?;
    for (event, size) in &mut sizes {
        *size = size.for_event(*event);
    }
    sizes.retain(|_, size| *size != PreviewSize::Normal);
    Ok(sizes)
}

/// A preset size with a custom one, as the general settings and each event
/// choose them.
pub trait SizePreset: Copy + PartialEq {
    /// The preset that takes its size from a custom point size.
    const CUSTOM: Self;
    /// Smallest custom size, in points.
    const MIN_PT: u32;
    /// Largest custom size, in points.
    const MAX_PT: u32;

    /// Size in points. `Custom` has no fixed size and reports the default.
    fn preset_points(self) -> u32;
}

impl SizePreset for ScrambleSize {
    const CUSTOM: Self = Self::Custom;
    const MIN_PT: u32 = MIN_SCRAMBLE_PT;
    const MAX_PT: u32 = MAX_SCRAMBLE_PT;

    fn preset_points(self) -> u32 {
        self.points()
    }
}

impl SizePreset for TimerSize {
    const CUSTOM: Self = Self::Custom;
    const MIN_PT: u32 = MIN_TIMER_PT;
    const MAX_PT: u32 = MAX_TIMER_PT;

    fn preset_points(self) -> u32 {
        self.points()
    }
}

/// A size an event chooses for itself: a preset and, for `Custom`, its points.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct SizeChoice<S> {
    pub size: S,
    /// The points used when `size` is `custom`; unset reports the default.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub custom_pt: Option<u32>,
}

impl<S: SizePreset> SizeChoice<S> {
    /// The choice as points: the custom size when chosen, else the preset.
    pub fn points(self) -> u32 {
        match (self.size == S::CUSTOM, self.custom_pt) {
            (true, Some(pt)) => pt.clamp(S::MIN_PT, S::MAX_PT),
            _ => self.size.preset_points(),
        }
    }

    /// Drops the custom size of a preset that does not use it.
    fn tidy(mut self) -> Self {
        if self.size != S::CUSTOM {
            self.custom_pt = None;
        }
        self
    }
}

/// Reads per-event size choices, clamping custom sizes to their range.
fn lenient_size_choices<'de, D, S>(
    deserializer: D,
) -> Result<BTreeMap<WcaEvent, SizeChoice<S>>, D::Error>
where
    D: Deserializer<'de>,
    S: SizePreset + serde::de::DeserializeOwned,
{
    let mut choices: BTreeMap<WcaEvent, SizeChoice<S>> = lenient_event_map(deserializer)?;
    for choice in choices.values_mut() {
        *choice = choice.tidy();
        choice.custom_pt = choice.custom_pt.map(|pt| pt.clamp(S::MIN_PT, S::MAX_PT));
    }
    Ok(choices)
}

/// The choice `event` has: its own, else the general one.
fn event_choice<S: SizePreset>(
    own: &BTreeMap<WcaEvent, SizeChoice<S>>,
    event: WcaEvent,
    general: SizeChoice<S>,
) -> SizeChoice<S> {
    own.get(&event).copied().unwrap_or(general)
}

/// Changes the choice of `event` only. A choice equal to the general one is
/// not stored, so the event keeps following it.
fn change_event_choice<S: SizePreset>(
    own: &mut BTreeMap<WcaEvent, SizeChoice<S>>,
    event: WcaEvent,
    general: SizeChoice<S>,
    change: impl FnOnce(&mut SizeChoice<S>),
) {
    let mut choice = event_choice(own, event, general);
    change(&mut choice);
    let choice = choice.tidy();
    if choice == general.tidy() {
        own.remove(&event);
    } else {
        own.insert(event, choice);
    }
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

/// Whether a set of the algorithm library appears in the library and in the
/// trainer. Both are on until the user turns them off.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default)]
pub struct SetVisibility {
    /// Shows the set in the library: its tree, search results and reviews.
    pub library: bool,
    /// Offers the set in the trainer's picker.
    pub training: bool,
}

impl Default for SetVisibility {
    fn default() -> Self {
        Self {
            library: true,
            training: true,
        }
    }
}

impl SetVisibility {
    /// Returns whether neither switch was touched, so nothing needs saving.
    fn is_default(self) -> bool {
        self == Self::default()
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
    /// On the Relays screen, makes the timer key end each puzzle instead of
    /// stopping the whole relay.
    pub relay_key_splits: bool,
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
    /// The timer size of each event that has its own; the others use
    /// `timer_size`.
    #[serde(
        deserialize_with = "lenient_size_choices",
        skip_serializing_if = "BTreeMap::is_empty"
    )]
    pub timer_sizes: BTreeMap<WcaEvent, SizeChoice<TimerSize>>,
    /// How large the scramble is drawn.
    pub scramble_size: ScrambleSize,
    /// Draws the puzzle as the scramble leaves it, on the timer and in the
    /// solve details.
    pub scramble_preview: bool,
    /// Draws the preview in 3D, to turn around, instead of as the flat net.
    pub preview_3d: bool,
    /// The preview size of each event that differs from `Normal`.
    #[serde(
        deserialize_with = "lenient_preview_sizes",
        skip_serializing_if = "BTreeMap::is_empty"
    )]
    pub preview_sizes: BTreeMap<WcaEvent, PreviewSize>,
    /// The scramble size in points used when `scramble_size` is `custom`.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub scramble_custom_pt: Option<u32>,
    /// The scramble size of each event that has its own; the others use
    /// `scramble_size`.
    #[serde(
        deserialize_with = "lenient_size_choices",
        skip_serializing_if = "BTreeMap::is_empty"
    )]
    pub scramble_sizes: BTreeMap<WcaEvent, SizeChoice<ScrambleSize>>,
    /// How large the library's cases are drawn.
    pub library_size: LibrarySize,
    /// Draws the library's cases in 3D, to turn around, instead of flat.
    pub library_3d: bool,
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
    /// The sets hidden from the library or left out of the trainer, by stable
    /// set id. A set missing here is shown in both.
    #[serde(skip_serializing_if = "BTreeMap::is_empty")]
    pub set_visibility: BTreeMap<String, SetVisibility>,
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
            relay_key_splits: false,
            accurate_3x3: false,
            solve_list_ao5: false,
            always_show_scrollbar: false,
            font: FontChoice::default(),
            text_size: TextSize::default(),
            text_custom_pt: None,
            timer_size: TimerSize::default(),
            timer_custom_pt: None,
            timer_sizes: BTreeMap::new(),
            scramble_size: ScrambleSize::default(),
            scramble_preview: true,
            preview_3d: false,
            preview_sizes: BTreeMap::new(),
            scramble_custom_pt: None,
            scramble_sizes: BTreeMap::new(),
            library_size: LibrarySize::default(),
            library_3d: false,
            start_hint_seen: false,
            active_tags: Vec::new(),
            user_backup_dir: String::new(),
            keybindings: BTreeMap::new(),
            set_visibility: BTreeMap::new(),
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

    /// The general timer size, for the events without their own.
    pub const fn timer_general(&self) -> SizeChoice<TimerSize> {
        SizeChoice {
            size: self.timer_size,
            custom_pt: self.timer_custom_pt,
        }
    }

    /// The timer size `event` uses: its own, else the general one.
    pub fn timer_choice(&self, event: WcaEvent) -> SizeChoice<TimerSize> {
        event_choice(&self.timer_sizes, event, self.timer_general())
    }

    /// Timer size in points for `event`.
    pub fn timer_pt_for(&self, event: WcaEvent) -> u32 {
        self.timer_choice(event).points()
    }

    /// The timer digits' font size in pixels for `event` (1pt = 4/3px).
    pub fn timer_digits_px(&self, event: WcaEvent) -> f32 {
        self.timer_pt_for(event) as f32 * 4.0 / 3.0
    }

    /// Gives `event` its own timer preset, starting `Custom` from the points
    /// it shows now. Matching the general size makes it follow that again.
    pub fn set_timer_size(&mut self, event: WcaEvent, size: TimerSize) {
        let shown = self.timer_pt_for(event);
        let general = self.timer_general();
        change_event_choice(&mut self.timer_sizes, event, general, |choice| {
            if size == TimerSize::Custom && choice.custom_pt.is_none() {
                choice.custom_pt = Some(shown);
            }
            choice.size = size;
        });
    }

    /// Sets the custom timer points of `event`; `None` clears them.
    pub fn set_timer_custom_pt(&mut self, event: WcaEvent, points: Option<u32>) {
        let general = self.timer_general();
        change_event_choice(&mut self.timer_sizes, event, general, |choice| {
            choice.custom_pt = points.map(|pt| pt.clamp(MIN_TIMER_PT, MAX_TIMER_PT));
        });
    }

    /// Scramble text size in points: the custom size when chosen, else the preset.
    pub fn scramble_pt(&self) -> u32 {
        match (self.scramble_size, self.scramble_custom_pt) {
            (ScrambleSize::Custom, Some(pt)) => pt.clamp(MIN_SCRAMBLE_PT, MAX_SCRAMBLE_PT),
            (size, _) => size.points(),
        }
    }

    /// The general scramble size, for the events without their own.
    pub const fn scramble_general(&self) -> SizeChoice<ScrambleSize> {
        SizeChoice {
            size: self.scramble_size,
            custom_pt: self.scramble_custom_pt,
        }
    }

    /// The scramble size `event` uses: its own, else the general one.
    pub fn scramble_choice(&self, event: WcaEvent) -> SizeChoice<ScrambleSize> {
        event_choice(&self.scramble_sizes, event, self.scramble_general())
    }

    /// Scramble text size in points for `event`.
    pub fn scramble_pt_for(&self, event: WcaEvent) -> u32 {
        self.scramble_choice(event).points()
    }

    /// Scramble text size in pixels for `event` (1pt = 4/3px).
    pub fn scramble_font_px(&self, event: WcaEvent) -> f32 {
        self.scramble_pt_for(event) as f32 * 4.0 / 3.0
    }

    /// Scramble line height in pixels for `event`, scaled with the text.
    pub fn scramble_line_px(&self, event: WcaEvent) -> f32 {
        self.scramble_font_px(event) * 1.5
    }

    /// Gives `event` its own scramble preset, starting `Custom` from the points
    /// it shows now. Matching the general size makes it follow that again.
    pub fn set_scramble_size(&mut self, event: WcaEvent, size: ScrambleSize) {
        let shown = self.scramble_pt_for(event);
        let general = self.scramble_general();
        change_event_choice(&mut self.scramble_sizes, event, general, |choice| {
            if size == ScrambleSize::Custom && choice.custom_pt.is_none() {
                choice.custom_pt = Some(shown);
            }
            choice.size = size;
        });
    }

    /// Sets the custom scramble points of `event`; `None` clears them.
    pub fn set_scramble_custom_pt(&mut self, event: WcaEvent, points: Option<u32>) {
        let general = self.scramble_general();
        change_event_choice(&mut self.scramble_sizes, event, general, |choice| {
            choice.custom_pt = points.map(|pt| pt.clamp(MIN_SCRAMBLE_PT, MAX_SCRAMBLE_PT));
        });
    }

    /// How many times the original library dimensions are drawn.
    pub const fn library_scale(&self) -> f32 {
        self.library_size.scale()
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

    /// Returns whether the set `id` appears in the library.
    pub fn shows_set_in_library(&self, id: &str) -> bool {
        self.set_visibility
            .get(id)
            .is_none_or(|visibility| visibility.library)
    }

    /// Returns whether the set `id` is offered in the trainer.
    pub fn includes_set_in_training(&self, id: &str) -> bool {
        self.set_visibility
            .get(id)
            .is_none_or(|visibility| visibility.training)
    }

    /// Shows or hides the set `id` in the library.
    pub fn show_set_in_library(&mut self, id: &str, show: bool) {
        self.change_set_visibility(id, |visibility| visibility.library = show);
    }

    /// Includes or leaves out the set `id` in the trainer.
    pub fn include_set_in_training(&mut self, id: &str, include: bool) {
        self.change_set_visibility(id, |visibility| visibility.training = include);
    }

    /// Changes one set's visibility, dropping the entry once both switches are
    /// back at their defaults.
    fn change_set_visibility(&mut self, id: &str, change: impl FnOnce(&mut SetVisibility)) {
        let mut visibility = self.set_visibility.get(id).copied().unwrap_or_default();
        change(&mut visibility);
        if visibility.is_default() {
            self.set_visibility.remove(id);
        } else {
            self.set_visibility.insert(id.to_owned(), visibility);
        }
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
    fn an_event_without_its_own_scramble_size_follows_the_general_one() {
        let mut settings = Settings {
            scramble_size: ScrambleSize::Large,
            ..Settings::default()
        };
        for event in WcaEvent::ALL {
            assert_eq!(settings.scramble_pt_for(event), 22);
        }
        assert!(
            !toml::to_string(&settings)
                .unwrap()
                .contains("scramble_sizes")
        );
        settings.set_scramble_size(WcaEvent::Cube2x2, ScrambleSize::Largest);
        settings.set_scramble_size(WcaEvent::Cube7x7, ScrambleSize::Small);
        assert_eq!(settings.scramble_pt_for(WcaEvent::Cube2x2), 30);
        assert_eq!(settings.scramble_pt_for(WcaEvent::Cube7x7), 12);
        assert_eq!(settings.scramble_pt_for(WcaEvent::Cube3x3), 22);
        assert_eq!(settings.scramble_pt(), 22);
        assert!((settings.scramble_font_px(WcaEvent::Cube2x2) - 40.).abs() < 1e-4);
        assert!((settings.scramble_line_px(WcaEvent::Cube2x2) - 60.).abs() < 1e-4);
        // Matching the general size again follows it.
        settings.set_scramble_size(WcaEvent::Cube2x2, ScrambleSize::Large);
        assert_eq!(settings.scramble_sizes.len(), 1);
    }

    #[test]
    fn each_event_keeps_its_own_custom_scramble_points() {
        let mut settings = Settings::default();
        settings.set_scramble_size(WcaEvent::Cube4x4, ScrambleSize::Custom);
        // Custom starts from what the event showed.
        assert_eq!(settings.scramble_pt_for(WcaEvent::Cube4x4), 16);
        settings.set_scramble_custom_pt(WcaEvent::Cube4x4, Some(40));
        settings.set_scramble_size(WcaEvent::Clock, ScrambleSize::Custom);
        settings.set_scramble_custom_pt(WcaEvent::Clock, Some(9));
        assert_eq!(settings.scramble_pt_for(WcaEvent::Cube4x4), 40);
        assert_eq!(settings.scramble_pt_for(WcaEvent::Clock), 9);
        assert_eq!(settings.scramble_pt_for(WcaEvent::Cube3x3), 16);
        settings.set_scramble_custom_pt(WcaEvent::Clock, Some(1000));
        assert_eq!(settings.scramble_pt_for(WcaEvent::Clock), MAX_SCRAMBLE_PT);
        settings.set_scramble_custom_pt(WcaEvent::Clock, Some(1));
        assert_eq!(settings.scramble_pt_for(WcaEvent::Clock), MIN_SCRAMBLE_PT);
    }

    #[test]
    fn per_event_scramble_sizes_load_leniently_and_older_files_are_unchanged() {
        let old: Settings = toml::from_str("scramble_size = 'large'").unwrap();
        assert!(old.scramble_sizes.is_empty());
        assert_eq!(old.scramble_pt_for(WcaEvent::Cube7x7), 22);
        let text = "[scramble_sizes.Cube2x2]\nsize = 'largest'\n\n[scramble_sizes.Cube3x3]\nsize = 'custom'\ncustom_pt = 500\n\n[scramble_sizes.Cube4x4]\nsize = 'bogus'\n\n[scramble_sizes.Future]\nsize = 'small'\n";
        let loaded: Settings = toml::from_str(text).unwrap();
        assert_eq!(loaded.scramble_pt_for(WcaEvent::Cube2x2), 30);
        assert_eq!(loaded.scramble_pt_for(WcaEvent::Cube3x3), MAX_SCRAMBLE_PT);
        assert_eq!(loaded.scramble_pt_for(WcaEvent::Cube4x4), 16);
        assert_eq!(loaded.scramble_sizes.len(), 2);
        let again: Settings = toml::from_str(&toml::to_string(&loaded).unwrap()).unwrap();
        assert_eq!(again.scramble_sizes, loaded.scramble_sizes);
    }

    #[test]
    fn an_event_without_its_own_timer_size_follows_the_general_one() {
        let mut settings = Settings {
            timer_size: TimerSize::Large,
            ..Settings::default()
        };
        for event in WcaEvent::ALL {
            assert_eq!(settings.timer_pt_for(event), 105);
        }
        assert!(!toml::to_string(&settings).unwrap().contains("timer_sizes"));
        settings.set_timer_size(WcaEvent::Cube2x2, TimerSize::Largest);
        settings.set_timer_size(WcaEvent::Cube7x7, TimerSize::Small);
        assert_eq!(settings.timer_pt_for(WcaEvent::Cube2x2), 126);
        assert_eq!(settings.timer_pt_for(WcaEvent::Cube7x7), 63);
        assert_eq!(settings.timer_pt_for(WcaEvent::Cube3x3), 105);
        assert_eq!(settings.timer_pt(), 105);
        assert!((settings.timer_digits_px(WcaEvent::Cube2x2) - 168.).abs() < 1e-4);
        // Matching the general size again follows it.
        settings.set_timer_size(WcaEvent::Cube2x2, TimerSize::Large);
        assert_eq!(settings.timer_sizes.len(), 1);
    }

    #[test]
    fn each_event_keeps_its_own_custom_timer_points() {
        let mut settings = Settings::default();
        settings.set_timer_size(WcaEvent::Cube4x4, TimerSize::Custom);
        // Custom starts from what the event showed.
        assert_eq!(settings.timer_pt_for(WcaEvent::Cube4x4), 84);
        settings.set_timer_custom_pt(WcaEvent::Cube4x4, Some(150));
        settings.set_timer_size(WcaEvent::Clock, TimerSize::Custom);
        settings.set_timer_custom_pt(WcaEvent::Clock, Some(30));
        assert_eq!(settings.timer_pt_for(WcaEvent::Cube4x4), 150);
        assert_eq!(settings.timer_pt_for(WcaEvent::Clock), 30);
        assert_eq!(settings.timer_pt_for(WcaEvent::Cube3x3), 84);
        settings.set_timer_custom_pt(WcaEvent::Clock, Some(1000));
        assert_eq!(settings.timer_pt_for(WcaEvent::Clock), MAX_TIMER_PT);
        settings.set_timer_custom_pt(WcaEvent::Clock, Some(1));
        assert_eq!(settings.timer_pt_for(WcaEvent::Clock), MIN_TIMER_PT);
    }

    #[test]
    fn per_event_timer_sizes_load_leniently_and_older_files_are_unchanged() {
        let old: Settings = toml::from_str("timer_size = 'large'").unwrap();
        assert!(old.timer_sizes.is_empty());
        assert_eq!(old.timer_pt_for(WcaEvent::Cube7x7), 105);
        let text = "[timer_sizes.Cube2x2]\nsize = 'largest'\n\n[timer_sizes.Cube3x3]\nsize = 'custom'\ncustom_pt = 5000\n\n[timer_sizes.Cube4x4]\nsize = 'bogus'\n\n[timer_sizes.Future]\nsize = 'small'\n";
        let loaded: Settings = toml::from_str(text).unwrap();
        assert_eq!(loaded.timer_pt_for(WcaEvent::Cube2x2), 126);
        assert_eq!(loaded.timer_pt_for(WcaEvent::Cube3x3), MAX_TIMER_PT);
        assert_eq!(loaded.timer_pt_for(WcaEvent::Cube4x4), 84);
        assert_eq!(loaded.timer_sizes.len(), 2);
        let again: Settings = toml::from_str(&toml::to_string(&loaded).unwrap()).unwrap();
        assert_eq!(again.timer_sizes, loaded.timer_sizes);
    }

    #[test]
    fn missing_and_unknown_keys_are_tolerated() {
        let settings: Settings = toml::from_str("inspection = false\nfuture = 1").unwrap();
        assert!(!settings.inspection);
        assert_eq!(settings.inspection_limit_ms, 15_000);
        assert!(settings.shows_start_hint());
    }

    #[test]
    fn library_sets_are_visible_and_offered_by_default() {
        let settings = Settings::default();
        assert!(settings.shows_set_in_library("pll"));
        assert!(settings.includes_set_in_training("pll"));
        assert!(
            !toml::to_string(&settings)
                .unwrap()
                .contains("set_visibility")
        );
        let old: Settings = toml::from_str("inspection = false").unwrap();
        assert!(old.shows_set_in_library("pll"));
        assert!(old.includes_set_in_training("pll"));
    }

    #[test]
    fn each_set_hides_from_the_library_and_from_training_separately() {
        let mut settings = Settings::default();
        settings.show_set_in_library("pll", false);
        assert!(!settings.shows_set_in_library("pll"));
        assert!(settings.includes_set_in_training("pll"));
        settings.include_set_in_training("pll", false);
        let text = toml::to_string(&settings).unwrap();
        let loaded: Settings = toml::from_str(&text).unwrap();
        assert!(!loaded.shows_set_in_library("pll"));
        assert!(!loaded.includes_set_in_training("pll"));
        // The other sets keep the defaults.
        assert!(loaded.shows_set_in_library("oll"));

        let mut settings = loaded;
        settings.show_set_in_library("pll", true);
        assert!(settings.shows_set_in_library("pll"));
        assert!(!settings.includes_set_in_training("pll"));
        settings.include_set_in_training("pll", true);
        assert!(settings.set_visibility.is_empty());
    }

    #[test]
    fn a_stored_set_visibility_may_name_only_one_switch() {
        let settings: Settings = toml::from_str(
            "[set_visibility.pll]\nlibrary = false\n\n[set_visibility.future]\ntraining = false\n",
        )
        .unwrap();
        assert!(!settings.shows_set_in_library("pll"));
        assert!(settings.includes_set_in_training("pll"));
        assert!(settings.shows_set_in_library("future"));
        assert!(!settings.includes_set_in_training("future"));
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
    fn the_relay_key_stops_the_whole_relay_by_default() {
        assert!(!Settings::default().relay_key_splits);
        let settings: Settings = toml::from_str("relay_key_splits = true").unwrap();
        assert!(settings.relay_key_splits);
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
    fn the_preview_is_the_flat_net_unless_3d_is_chosen() {
        assert!(!Settings::default().preview_3d);
        let old: Settings = toml::from_str("scramble_preview = true").unwrap();
        assert!(!old.preview_3d);
        let settings: Settings = toml::from_str("preview_3d = true").unwrap();
        assert!(settings.preview_3d);
        let saved = toml::to_string(&settings).unwrap();
        let loaded: Settings = toml::from_str(&saved).unwrap();
        assert!(loaded.preview_3d);
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
        assert!((settings.scramble_line_px(WcaEvent::Cube3x3) - 100.0).abs() < 1e-3);
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
        assert!((settings.timer_digits_px(WcaEvent::Cube3x3) - 112.).abs() < f32::EPSILON);

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
        assert!((settings.timer_digits_px(WcaEvent::Cube3x3) - 120.).abs() < f32::EPSILON);
    }

    #[test]
    fn library_size_defaults_to_normal_and_scales_in_order() {
        let settings = Settings::default();
        assert_eq!(settings.library_size, LibrarySize::Normal);
        assert!((settings.library_scale() - 1.2).abs() < f32::EPSILON);

        assert!(
            LibrarySize::ALL
                .windows(2)
                .all(|pair| pair[0].scale() < pair[1].scale())
        );
        let parsed: Settings = toml::from_str("library_size = 'big'").unwrap();
        assert_eq!(parsed.library_size, LibrarySize::Big);
        assert!(parsed.library_scale() > settings.library_scale());
    }

    #[test]
    fn library_cases_are_flat_unless_3d_is_chosen() {
        assert!(!Settings::default().library_3d);
        let old: Settings = toml::from_str("preview_3d = true").unwrap();
        assert!(!old.library_3d);
        let settings: Settings = toml::from_str("library_3d = true").unwrap();
        let saved = toml::to_string(&settings).unwrap();
        assert!(toml::from_str::<Settings>(&saved).unwrap().library_3d);
    }

    #[test]
    fn library_size_round_trips_and_older_files_get_normal() {
        let old: Settings = toml::from_str("inspection = false").unwrap();
        assert_eq!(old.library_size, LibrarySize::Normal);
        let settings = Settings {
            library_size: LibrarySize::Big,
            ..Settings::default()
        };
        let text = toml::to_string(&settings).unwrap();
        assert_eq!(
            toml::from_str::<Settings>(&text).unwrap().library_size,
            LibrarySize::Big
        );
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
