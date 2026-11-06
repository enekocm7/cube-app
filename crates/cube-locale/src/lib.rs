//! UI string tables and locale-aware formatting.
//!
//! Each language is one [`Strings`] value, so a missing translation is a compile
//! error rather than a blank label at runtime.
//!
//! Dates and counts follow the selected language through [`Language::format_date`]
//! and [`Language::format_count`]. Solve durations keep their conventional
//! `9.123` shape in every language: that is the notation the timer, the manual
//! entry parser and cube-tui all share.

mod ar;
mod cs;
mod de;
mod en;
mod es;
mod fr;
mod it;
mod ja;
mod ko;
mod nl;
mod pl;
mod pt;
mod ru;
mod tr;
mod uk;
mod zh;

use chrono::{Datelike as _, Local, Timelike as _};

/// A supported interface language.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum Language {
    #[default]
    English,
    Arabic,
    Czech,
    German,
    Spanish,
    French,
    Italian,
    Japanese,
    Korean,
    Dutch,
    Polish,
    Portuguese,
    Russian,
    Turkish,
    Ukrainian,
    Chinese,
}

impl Language {
    /// Every language, in the order shown in settings.
    ///
    /// English comes first; the rest is alphabetical by native name.
    pub const ALL: [Self; 16] = [
        Self::English,
        Self::Arabic,
        Self::Czech,
        Self::German,
        Self::Spanish,
        Self::French,
        Self::Italian,
        Self::Japanese,
        Self::Korean,
        Self::Dutch,
        Self::Polish,
        Self::Portuguese,
        Self::Russian,
        Self::Turkish,
        Self::Ukrainian,
        Self::Chinese,
    ];

    /// Returns the code stored in the settings file.
    pub const fn code(self) -> &'static str {
        match self {
            Self::English => "en",
            Self::Arabic => "ar",
            Self::Czech => "cs",
            Self::German => "de",
            Self::Spanish => "es",
            Self::French => "fr",
            Self::Italian => "it",
            Self::Japanese => "ja",
            Self::Korean => "ko",
            Self::Dutch => "nl",
            Self::Polish => "pl",
            Self::Portuguese => "pt",
            Self::Russian => "ru",
            Self::Turkish => "tr",
            Self::Ukrainian => "uk",
            Self::Chinese => "zh",
        }
    }

    /// Returns the language's name, written in that language.
    pub const fn native_name(self) -> &'static str {
        match self {
            Self::English => "English",
            Self::Arabic => "العربية",
            Self::Czech => "Čeština",
            Self::German => "Deutsch",
            Self::Spanish => "Español",
            Self::French => "Français",
            Self::Italian => "Italiano",
            Self::Japanese => "日本語",
            Self::Korean => "한국어",
            Self::Dutch => "Nederlands",
            Self::Polish => "Polski",
            Self::Portuguese => "Português",
            Self::Russian => "Русский",
            Self::Turkish => "Türkçe",
            Self::Ukrainian => "Українська",
            Self::Chinese => "中文",
        }
    }

    /// Matches a code or locale such as `es` or `es-ES`, ignoring the region.
    pub fn from_code(code: &str) -> Option<Self> {
        let primary = code.split(['-', '_']).next()?;
        Self::ALL
            .into_iter()
            .find(|language| language.code().eq_ignore_ascii_case(primary))
    }

    /// Returns this language's string table.
    pub const fn strings(self) -> &'static Strings {
        match self {
            Self::English => &en::STRINGS,
            Self::Arabic => &ar::STRINGS,
            Self::Czech => &cs::STRINGS,
            Self::German => &de::STRINGS,
            Self::Spanish => &es::STRINGS,
            Self::French => &fr::STRINGS,
            Self::Italian => &it::STRINGS,
            Self::Japanese => &ja::STRINGS,
            Self::Korean => &ko::STRINGS,
            Self::Dutch => &nl::STRINGS,
            Self::Polish => &pl::STRINGS,
            Self::Portuguese => &pt::STRINGS,
            Self::Russian => &ru::STRINGS,
            Self::Turkish => &tr::STRINGS,
            Self::Ukrainian => &uk::STRINGS,
            Self::Chinese => &zh::STRINGS,
        }
    }

    /// The separator between groups of three digits in counts.
    pub const fn thousands_separator(self) -> char {
        match self {
            Self::French => '\u{202f}',
            Self::Arabic
            | Self::Czech
            | Self::German
            | Self::Spanish
            | Self::Italian
            | Self::Dutch
            | Self::Polish
            | Self::Portuguese
            | Self::Russian
            | Self::Turkish
            | Self::Ukrainian => '.',
            Self::English | Self::Japanese | Self::Korean | Self::Chinese => ',',
        }
    }

    /// The separator between a whole number and its fraction.
    pub const fn decimal_separator(self) -> char {
        match self {
            Self::Arabic
            | Self::Czech
            | Self::German
            | Self::Spanish
            | Self::French
            | Self::Italian
            | Self::Dutch
            | Self::Polish
            | Self::Portuguese
            | Self::Russian
            | Self::Turkish
            | Self::Ukrainian => ',',
            Self::English | Self::Japanese | Self::Korean | Self::Chinese => '.',
        }
    }

    /// Formats a count with this language's digit grouping: `1234` becomes
    /// `1,234` in English, `1.234` in German and `1 234` in French.
    pub fn format_count(self, value: u64) -> String {
        let digits = value.to_string();
        let separator = self.thousands_separator();
        let mut out = String::with_capacity(digits.len() + digits.len() / 3);
        for (index, digit) in digits.chars().enumerate() {
            if index > 0 && (digits.len() - index) % 3 == 0 {
                out.push(separator);
            }
            out.push(digit);
        }
        out
    }

    /// Formats a non-negative number with `decimals` fraction digits and this
    /// language's decimal separator.
    pub fn format_decimal(self, value: f64, decimals: usize) -> String {
        let text = format!("{value:.decimals$}");
        match self.decimal_separator() {
            '.' => text,
            separator => text.replace('.', &separator.to_string()),
        }
    }

    /// Formats Unix milliseconds as a local date and time: `2026-11-02 12:30`
    /// in English, `02.11.2026 12:30` in German. Zero means the date is
    /// unknown and renders as an empty string.
    pub fn format_date(self, unix_ms: u64) -> String {
        let Some(date) = i64::try_from(unix_ms)
            .ok()
            .filter(|&ms| ms != 0)
            .and_then(chrono::DateTime::from_timestamp_millis)
        else {
            return String::new();
        };
        let local = date.with_timezone(&Local);
        let date = self.format_date_parts(local.year(), local.month(), local.day());
        format!("{date} {:02}:{:02}", local.hour(), local.minute())
    }

    /// Formats a calendar day in this language's date order.
    pub fn format_day(self, day: chrono::NaiveDate) -> String {
        self.format_date_parts(day.year(), day.month(), day.day())
    }

    fn format_date_parts(self, year: i32, month: u32, day: u32) -> String {
        match self {
            Self::English | Self::Korean | Self::Chinese => {
                format!("{year:04}-{month:02}-{day:02}")
            }
            Self::Japanese => format!("{year:04}/{month:02}/{day:02}"),
            Self::Dutch => format!("{day:02}-{month:02}-{year:04}"),
            Self::Arabic | Self::Spanish | Self::French | Self::Italian | Self::Portuguese => {
                format!("{day:02}/{month:02}/{year:04}")
            }
            Self::Czech
            | Self::German
            | Self::Polish
            | Self::Russian
            | Self::Turkish
            | Self::Ukrainian => format!("{day:02}.{month:02}.{year:04}"),
        }
    }
}

/// Every piece of text in the interface.
///
/// `{name}` markers are filled in with [`str::replace`] by the caller.
#[derive(Debug)]
pub struct Strings {
    pub nav_timer: &'static str,
    pub nav_stats: &'static str,
    pub nav_charts: &'static str,
    pub settings: &'static str,

    /// `{key}`: the timer's binding.
    pub hint_start: &'static str,
    pub hint_hold: &'static str,
    pub hint_release: &'static str,
    /// `{key}`: the timer's binding.
    pub hint_inspecting: &'static str,
    /// `{key}`: the timer's binding.
    pub hint_stop: &'static str,
    pub manual_mode: &'static str,
    pub manual_empty: &'static str,
    pub manual_invalid: &'static str,
    pub generating_scramble: &'static str,
    pub new_scramble: &'static str,
    pub copy_scramble: &'static str,
    pub scramble_copied: &'static str,

    pub sessions: &'static str,
    /// `{number}`: the session's position.
    pub session_default_name: &'static str,
    pub new_session: &'static str,
    pub rename_session: &'static str,
    pub delete_session: &'static str,
    /// `{name}`: the session; `{count}`: its number of solves.
    pub delete_session_confirm: &'static str,
    pub session_name: &'static str,
    pub max_sessions: &'static str,
    pub last_session: &'static str,

    pub solves: &'static str,
    pub no_solves: &'static str,

    pub stats_current: &'static str,
    pub stats_best: &'static str,
    pub stat_single: &'static str,
    pub stat_worst: &'static str,
    pub stat_mean: &'static str,
    pub stat_std_dev: &'static str,
    pub stat_solves: &'static str,
    pub stat_dnfs: &'static str,
    pub chart_times: &'static str,
    pub chart_distribution: &'static str,
    pub chart_empty: &'static str,
    pub chart_view_trends: &'static str,
    pub chart_view_calendar: &'static str,
    pub chart_view_compare: &'static str,
    pub chart_view_bests: &'static str,
    pub chart_all_events: &'static str,
    pub chart_all_sessions: &'static str,
    pub chart_range_all: &'static str,
    pub chart_range_last_solves: &'static str,
    pub chart_range_last_days: &'static str,
    pub chart_range_dates: &'static str,
    pub chart_range_custom: &'static str,
    pub chart_personal_best: &'static str,
    pub chart_no_match: &'static str,
    pub chart_solve: &'static str,
    pub chart_year_summary: &'static str,
    pub chart_busiest_day: &'static str,
    pub chart_day: &'static str,
    pub chart_previous_year: &'static str,
    pub chart_next_year: &'static str,
    pub chart_compare_means: &'static str,
    pub chart_column_session: &'static str,
    pub chart_column_event: &'static str,
    pub chart_dates_title: &'static str,
    pub chart_date_from: &'static str,
    pub chart_date_to: &'static str,
    pub chart_date_placeholder: &'static str,
    pub chart_dates_invalid: &'static str,
    pub chart_months: [&'static str; 12],
    pub chart_zoom_range: &'static str,
    pub chart_reset_zoom: &'static str,
    pub table_time: &'static str,
    pub table_empty: &'static str,
    pub mean_of: &'static str,
    pub average_of: &'static str,
    pub average_trim_best: &'static str,
    pub average_trim_worst: &'static str,
    pub average_missing: &'static str,
    pub average_range: &'static str,
    pub average_faster: &'static str,
    pub average_slower: &'static str,
    pub copy_average: &'static str,
    pub average_copied: &'static str,
    pub share_button: &'static str,
    pub share_title: &'static str,
    pub share_copy_image: &'static str,
    pub share_save_image: &'static str,
    pub share_copy_link: &'static str,
    pub share_image_copied: &'static str,
    pub share_image_saved: &'static str,
    pub share_link_copied: &'static str,
    pub share_failed: &'static str,
    pub share_save_failed: &'static str,
    pub share_link_unavailable: &'static str,
    pub share_average_too_long: &'static str,
    pub share_session_best: &'static str,
    pub field_solution: &'static str,
    pub invalid_solution: &'static str,

    pub solve_details: &'static str,
    pub field_time: &'static str,
    pub field_penalty: &'static str,
    pub field_scramble: &'static str,
    pub field_comment: &'static str,
    pub field_date: &'static str,
    pub field_event: &'static str,
    pub date_unknown: &'static str,
    pub penalty_none: &'static str,
    pub invalid_time: &'static str,
    pub invalid_duration: &'static str,
    /// `{event}`: the event the scramble must match.
    pub invalid_scramble: &'static str,
    pub invalid_date: &'static str,
    pub invalid_comment: &'static str,
    pub date_placeholder: &'static str,
    pub edit_history: &'static str,
    pub history_empty: &'static str,
    /// `{count}`: how many edits the solve has.
    pub history_show: &'static str,
    pub history_hide: &'static str,
    /// `{number}`: the revision; `{date}`: when it was saved.
    pub history_revision: &'static str,
    pub history_before: &'static str,
    pub history_after: &'static str,
    pub empty_value: &'static str,

    pub save: &'static str,
    pub cancel: &'static str,
    pub close: &'static str,
    pub delete: &'static str,
    pub delete_solve: &'static str,
    pub delete_solve_confirm: &'static str,
    pub solve_deleted: &'static str,
    pub action_undo_delete: &'static str,
    pub action_redo_delete: &'static str,
    pub setting_confirm_delete: &'static str,
    pub undo: &'static str,
    pub session_deleted: &'static str,

    pub settings_timer: &'static str,
    pub settings_appearance: &'static str,
    pub settings_font_size: &'static str,
    pub settings_theme: &'static str,
    pub settings_data: &'static str,
    pub settings_general: &'static str,
    pub settings_notifications: &'static str,
    pub settings_scrambles: &'static str,
    pub settings_solve_list: &'static str,
    pub settings_import_export: &'static str,
    pub settings_backups: &'static str,
    pub settings_search_results: &'static str,
    pub settings_open_shortcuts: &'static str,
    pub setting_inspection_desc: &'static str,
    pub setting_inspection_audio_desc: &'static str,
    pub setting_celebrate: &'static str,
    pub setting_celebrate_desc: &'static str,
    pub setting_celebrate_sound: &'static str,
    pub setting_celebrate_sound_desc: &'static str,
    pub setting_best_min_solves: &'static str,
    pub setting_best_min_solves_desc: &'static str,
    pub setting_reduce_motion: &'static str,
    pub setting_reduce_motion_desc: &'static str,
    pub setting_touch_to_start_desc: &'static str,
    pub setting_accurate_desc: &'static str,
    pub setting_scramble_preview_desc: &'static str,
    pub setting_event_sizes: &'static str,
    pub setting_event_sizes_desc: &'static str,
    pub event_sizes_open: &'static str,
    /// `{event}`: the event the size is for.
    pub setting_preview_size: &'static str,
    pub setting_preview_size_desc: &'static str,
    pub preview_size_small: &'static str,
    pub preview_size_normal: &'static str,
    pub preview_size_big: &'static str,
    pub preview_size_huge: &'static str,
    pub setting_theme_desc: &'static str,
    pub custom_themes_desc: &'static str,
    pub setting_font_desc: &'static str,
    pub setting_text_size_desc: &'static str,
    pub setting_timer_size_desc: &'static str,
    /// `{event}`: the event the size is for.
    pub setting_timer_size_event: &'static str,
    pub setting_timer_size_event_desc: &'static str,
    /// `{event}`: the event the size is for.
    pub setting_timer_custom_event: &'static str,
    pub setting_scramble_size_desc: &'static str,
    /// `{event}`: the event the size is for.
    pub setting_scramble_size_event: &'static str,
    pub setting_scramble_size_event_desc: &'static str,
    /// `{event}`: the event the size is for.
    pub setting_scramble_custom_event: &'static str,
    pub size_custom_range: &'static str,
    pub setting_solve_list_ao5_desc: &'static str,
    pub setting_always_show_scrollbar_desc: &'static str,
    pub setting_toasts_desc: &'static str,
    pub setting_confirm_delete_desc: &'static str,
    pub setting_language_desc: &'static str,
    pub setting_import_desc: &'static str,
    pub setting_export_desc: &'static str,
    pub setting_backup_now_desc: &'static str,
    pub setting_check_updates_desc: &'static str,
    pub setting_update_channel_desc: &'static str,
    pub setting_update_frequency_desc: &'static str,
    pub setting_check_now_desc: &'static str,
    pub setting_shortcuts_desc: &'static str,
    pub setting_inspection: &'static str,
    pub setting_inspection_audio: &'static str,
    pub setting_touch_to_start: &'static str,
    pub setting_toasts: &'static str,
    pub setting_accurate: &'static str,
    pub setting_solve_list_ao5: &'static str,
    pub setting_always_show_scrollbar: &'static str,
    pub setting_text_size: &'static str,
    pub text_size_small: &'static str,
    pub text_size_normal: &'static str,
    pub text_size_large: &'static str,
    pub text_size_largest: &'static str,
    pub text_size_custom: &'static str,
    pub setting_text_custom: &'static str,
    pub setting_timer_size: &'static str,
    pub timer_size_small: &'static str,
    pub timer_size_normal: &'static str,
    pub timer_size_large: &'static str,
    pub timer_size_largest: &'static str,
    pub timer_size_custom: &'static str,
    pub setting_timer_custom: &'static str,
    pub setting_font: &'static str,
    pub font_system: &'static str,
    pub font_timer: &'static str,
    pub font_times: &'static str,
    pub font_whole: &'static str,
    pub setting_scramble_preview: &'static str,
    pub setting_scramble_size: &'static str,
    pub setting_scramble_custom: &'static str,
    pub size_custom_invalid: &'static str,
    pub scramble_size_small: &'static str,
    pub scramble_size_normal: &'static str,
    pub scramble_size_large: &'static str,
    pub scramble_size_largest: &'static str,
    pub scramble_size_custom: &'static str,
    pub setting_theme: &'static str,
    pub setting_language: &'static str,
    pub setting_import: &'static str,
    pub setting_export: &'static str,
    pub import_button: &'static str,
    pub export_button: &'static str,
    /// The label of the import file picker's confirm button.
    pub import_prompt: &'static str,
    pub setting_backup_folder: &'static str,
    /// Shown in place of the folder's path when none is chosen.
    pub backup_folder_none: &'static str,
    pub backup_folder_button: &'static str,
    pub backup_folder_clear: &'static str,
    /// The label of the backup folder picker's confirm button.
    pub backup_folder_prompt: &'static str,
    pub setting_backup_now: &'static str,
    pub backup_now_button: &'static str,
    pub theme_system: &'static str,
    pub theme_light: &'static str,
    pub theme_dark: &'static str,

    pub help_title: &'static str,
    pub help_hint: &'static str,
    pub help_any_key_stops: &'static str,
    pub help_section_timer: &'static str,
    pub help_section_solves: &'static str,
    pub help_section_events_sessions: &'static str,
    pub help_section_averages: &'static str,
    pub help_section_screens: &'static str,
    pub help_section_charts: &'static str,
    pub action_timer: &'static str,
    pub action_cancel: &'static str,
    pub action_toggle_inspection: &'static str,
    pub action_toggle_scramble_preview: &'static str,
    pub action_manual_entry: &'static str,
    pub action_previous_solve: &'static str,
    pub action_next_solve: &'static str,
    pub action_open_solve: &'static str,
    pub action_penalty_plus_two: &'static str,
    pub action_penalty_dnf: &'static str,
    pub action_next_event: &'static str,
    pub action_previous_event: &'static str,
    pub action_next_session: &'static str,
    pub action_previous_session: &'static str,
    pub action_open_stats: &'static str,
    pub action_open_charts: &'static str,
    pub action_next_chart_view: &'static str,
    pub action_next_chart_event: &'static str,
    pub action_next_chart_range: &'static str,
    pub action_chart_dates: &'static str,
    pub action_toggle_chart_sessions: &'static str,
    pub action_chart_zoom_in: &'static str,
    pub action_chart_zoom_out: &'static str,
    pub action_chart_reset_zoom: &'static str,
    pub action_previous_average: &'static str,
    pub action_next_average: &'static str,
    pub action_open_average: &'static str,
    pub action_open_best_average: &'static str,
    pub action_copy_average: &'static str,
    pub action_share_solve: &'static str,
    pub action_share_average: &'static str,
    pub action_copy_solve_link: &'static str,
    pub action_open_bluetooth: &'static str,
    pub action_show_help: &'static str,
    pub action_focus_search: &'static str,
    pub unassigned: &'static str,
    pub search_placeholder: &'static str,
    pub search_no_results: &'static str,
    pub search_on: &'static str,
    pub search_off: &'static str,

    pub settings_keybindings: &'static str,
    pub keybind_hint: &'static str,
    pub keybind_edit: &'static str,
    pub keybind_done: &'static str,
    pub keybind_capture: &'static str,
    /// `{action}`: the action whose shortcut is being captured.
    pub keybind_capture_hint: &'static str,
    pub keybind_change: &'static str,
    pub keybind_remove: &'static str,
    pub keybind_error_invalid: &'static str,
    pub keybind_error_timer_modified: &'static str,
    /// `{action}`: the action that already uses the key.
    pub keybind_error_conflict: &'static str,
    pub keybind_error_timer_required: &'static str,
    pub custom_themes: &'static str,
    pub new_theme: &'static str,
    pub edit_theme: &'static str,
    pub delete_theme: &'static str,
    pub theme_name: &'static str,
    pub theme_name_placeholder: &'static str,
    pub theme_colors: &'static str,
    pub theme_background: &'static str,
    pub theme_border: &'static str,
    pub theme_scramble: &'static str,
    pub theme_selection: &'static str,
    pub theme_selection_text: &'static str,
    pub theme_text: &'static str,
    pub theme_accent: &'static str,
    pub theme_preview: &'static str,
    pub theme_preview_selected: &'static str,
    pub theme_alpha_hint: &'static str,
    pub theme_name_empty: &'static str,
    pub theme_name_invalid: &'static str,
    pub theme_name_too_long: &'static str,
    pub theme_name_exists: &'static str,
    pub theme_invalid_color: &'static str,
    /// `{error}`: the cause.
    pub theme_save_failed: &'static str,
    /// `{name}`: the theme.
    pub theme_saved: &'static str,
    /// `{name}`: the theme.
    pub theme_delete_confirm: &'static str,
    /// `{name}`: the theme.
    pub theme_deleted: &'static str,
    /// `{error}`: the cause.
    pub theme_delete_failed: &'static str,
    /// `{error}`: the cause.
    pub theme_load_failed: &'static str,
    pub no_themes: &'static str,

    pub settings_updates: &'static str,
    pub setting_check_updates: &'static str,
    pub setting_update_channel: &'static str,
    pub setting_update_frequency: &'static str,
    pub setting_check_now: &'static str,
    pub setting_clear_skip: &'static str,
    /// `{version}`: the version being skipped.
    pub setting_skipping: &'static str,
    pub update_channel_stable: &'static str,
    pub update_channel_prerelease: &'static str,
    pub update_frequency_start: &'static str,
    pub update_frequency_daily: &'static str,
    pub update_frequency_weekly: &'static str,
    pub update_now: &'static str,
    pub update_release_notes: &'static str,
    pub update_skip: &'static str,
    pub update_later: &'static str,
    pub update_restart_now: &'static str,

    /// `{version}`: the version being downloaded.
    pub update_downloading: &'static str,
    pub update_updating_title: &'static str,
    pub update_updated_title: &'static str,
    /// `{current}`: the version that is now running.
    pub update_updated_body: &'static str,
    pub update_check_failed_title: &'static str,
    /// `{error}`: the cause.
    pub update_check_failed_body: &'static str,
    pub update_uptodate_title: &'static str,
    /// `{version}`: the running version.
    pub update_uptodate_body: &'static str,
    pub update_available_title: &'static str,
    /// `{version}`: the new version; `{current}`: the running one.
    pub update_available_body: &'static str,
    pub update_skipped_title: &'static str,
    /// `{version}`: the skipped version.
    pub update_skipped_body: &'static str,
    pub update_no_notes: &'static str,
    /// `{version}`: the new version.
    pub update_notes_title: &'static str,
    pub update_no_asset: &'static str,
    pub update_failed_title: &'static str,
    pub update_cancelled_body: &'static str,
    pub update_restart_title: &'static str,
    /// `{version}`: the version waiting to be installed.
    pub update_restart_body: &'static str,
    /// `{error}`: the cause.
    pub update_install_failed: &'static str,

    /// `{label}`: the statistic; `{time}`: its new value.
    pub toast_new_best: &'static str,
    pub toast_celebration_audio_failed: &'static str,
    pub toast_plus_two: &'static str,
    pub toast_dnf: &'static str,
    /// `{error}`: why the cue could not be played.
    pub toast_audio_failed: &'static str,
    pub toast_cancelled: &'static str,
    pub bluetooth_title: &'static str,
    pub bluetooth_scanning: &'static str,
    pub bluetooth_pick: &'static str,
    pub bluetooth_connect: &'static str,
    pub bluetooth_disconnect: &'static str,
    /// `{name}`: the timer's name.
    pub bluetooth_connecting: &'static str,
    /// `{name}`: the timer's name.
    pub bluetooth_connected: &'static str,
    pub bluetooth_disconnected: &'static str,
    pub bluetooth_lost: &'static str,
    pub bluetooth_no_adapter: &'static str,
    /// `{error}`: the cause.
    pub bluetooth_scan_failed: &'static str,
    pub bluetooth_not_found: &'static str,
    pub bluetooth_timed_out: &'static str,
    /// `{error}`: the cause.
    pub bluetooth_connect_failed: &'static str,
    /// `{error}`: the cause.
    pub bluetooth_disconnect_failed: &'static str,
    pub bluetooth_incomplete_update: &'static str,
    /// `{state}`: the number the timer sent.
    pub bluetooth_unsupported_state: &'static str,
    pub bluetooth_incomplete_time: &'static str,
    /// `{error}`: the cause.
    pub bluetooth_time_unreadable: &'static str,
    /// `{error}`: the cause.
    pub toast_save_failed: &'static str,
    /// `{error}`: the cause.
    pub toast_load_failed: &'static str,
    /// `{path}`: the newest backup of the solves.
    pub toast_backup_available: &'static str,
    /// `{path}`: the backup that was written.
    pub toast_backup_done: &'static str,
    /// `{error}`: the cause.
    pub toast_backup_failed: &'static str,
    /// `{added}`: solves taken from the file; `{updated}`: solves replaced by
    /// newer edits; `{kept}`: solves the file lost that were kept.
    pub toast_history_merged: &'static str,
    /// `{path}`: where the unreadable settings file is now.
    pub toast_settings_reset: &'static str,
    /// `{imported}`: how many solves were added; `{skipped}`: how many were not.
    pub toast_import_done: &'static str,
    /// `{path}`: the file that was written.
    pub toast_export_done: &'static str,
    /// `{error}`: the cause.
    pub toast_import_failed: &'static str,
    /// `{error}`: the cause.
    pub toast_export_failed: &'static str,
    pub field_tags: &'static str,
    pub tag_new_placeholder: &'static str,
    pub tag_add: &'static str,
    pub tags_none: &'static str,
    pub tag_invalid: &'static str,
    pub tag_exists: &'static str,
    pub tags_unavailable: &'static str,
    pub toast_tags_load_failed: &'static str,
    pub tags_active_title: &'static str,
    pub tags_solve_title: &'static str,
    pub tags_manage: &'static str,
    pub tag_rename: &'static str,
    pub tag_delete: &'static str,
    pub tag_delete_confirm: &'static str,
    pub tags_active_label: &'static str,
    pub tags_active_none: &'static str,
    pub chart_all_tags: &'static str,
    pub action_edit_tags: &'static str,
    pub action_active_tags: &'static str,
    pub action_next_tag_filter: &'static str,
    pub action_next_compare_tag: &'static str,
    pub action_open_goals: &'static str,
    pub goals_title: &'static str,
    pub nav_trainer: &'static str,
    pub trainer_toggle_all: &'static str,
    pub trainer_toggle_case: &'static str,
    pub trainer_toggle_group: &'static str,
    pub trainer_selected_count: &'static str,
    pub trainer_start: &'static str,
    pub trainer_nothing_selected: &'static str,
    pub trainer_sort_slowest: &'static str,
    pub trainer_attempts: &'static str,
    pub trainer_best: &'static str,
    pub trainer_recent: &'static str,
    pub trainer_back: &'static str,
    pub trainer_progress: &'static str,
    pub trainer_reveal: &'static str,
    pub trainer_skip: &'static str,
    pub trainer_no_attempts: &'static str,
    pub trainer_case_attempts: &'static str,
    pub trainer_edit_algorithm: &'static str,
    pub trainer_algorithm_hint: &'static str,
    pub trainer_algorithm_invalid: &'static str,
    pub trainer_algorithm_wrong_case: &'static str,
    pub trainer_delete_confirm: &'static str,
    pub toast_trainer_load_failed: &'static str,
    pub toast_trainer_save_failed: &'static str,
    pub toast_trainer_scramble_failed: &'static str,
    pub action_open_trainer: &'static str,
    pub action_next_trainer_set: &'static str,
    pub goals_empty: &'static str,
    pub goals_unavailable: &'static str,
    pub goals_new: &'static str,
    pub goals_edit: &'static str,
    pub goals_delete: &'static str,
    pub goals_delete_confirm: &'static str,
    pub goal_field_type: &'static str,
    pub goal_type_volume: &'static str,
    pub goal_type_time: &'static str,
    pub goal_type_streak: &'static str,
    pub goal_field_session: &'static str,
    pub goal_all_sessions: &'static str,
    pub goal_field_solves: &'static str,
    pub goal_field_period: &'static str,
    pub goal_period_day: &'static str,
    pub goal_period_week: &'static str,
    pub goal_field_result: &'static str,
    pub goal_field_days: &'static str,
    pub goal_field_solves_per_day: &'static str,
    /// `{count}`: the solves a day.
    pub goal_summary_day: &'static str,
    /// `{count}`: the solves a week.
    pub goal_summary_week: &'static str,
    /// `{result}`: a single or an average; `{time}`: the time to beat.
    pub goal_summary_time: &'static str,
    /// `{count}`: the solves a day; `{days}`: the days in a row.
    pub goal_summary_streak: &'static str,
    pub goal_reached: &'static str,
    pub goal_paused: &'static str,
    /// `{max}`: the largest number allowed.
    pub goal_error_count: &'static str,
    /// `{goal}`: the goal, as its summary describes it.
    pub toast_goal_reached: &'static str,
    /// `{error}`: the cause.
    pub toast_goals_load_failed: &'static str,
    /// `{error}`: the cause.
    pub toast_goals_save_failed: &'static str,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn codes_match_with_or_without_a_region() {
        assert_eq!(Language::from_code("es"), Some(Language::Spanish));
        assert_eq!(Language::from_code("es-ES"), Some(Language::Spanish));
        assert_eq!(Language::from_code("EN_gb"), Some(Language::English));
        assert_eq!(Language::from_code("pt_BR"), Some(Language::Portuguese));
        assert_eq!(Language::from_code("zh-Hant-TW"), Some(Language::Chinese));
        assert_eq!(Language::from_code("xx"), None);
        assert_eq!(Language::from_code(""), None);
    }

    #[test]
    fn every_language_has_a_unique_code_and_name() {
        assert_eq!(Language::ALL.len(), 16);
        for (index, language) in Language::ALL.into_iter().enumerate() {
            assert_eq!(Language::from_code(language.code()), Some(language));
            assert_ne!(language.native_name(), "");
            assert!(
                Language::ALL
                    .into_iter()
                    .skip(index + 1)
                    .all(|other| other.code() != language.code())
            );
        }
    }

    #[test]
    fn counts_group_and_fractional_numbers_follow_the_language() {
        assert_eq!(Language::English.format_count(1_234_567), "1,234,567");
        assert_eq!(Language::German.format_count(1_234_567), "1.234.567");
        assert_eq!(
            Language::French.format_count(1_234_567),
            "1\u{202f}234\u{202f}567"
        );
        assert_eq!(Language::Chinese.format_count(1_234), "1,234");
        assert_eq!(Language::English.format_count(999), "999");
        assert_eq!(Language::English.format_decimal(12.5, 1), "12.5");
        assert_eq!(Language::German.format_decimal(12.5, 1), "12,5");
        assert_eq!(Language::French.format_decimal(66.7, 1), "66,7");
    }

    #[test]
    fn dates_use_the_language_order() {
        // 2026-11-02 12:30 UTC; the local zone may shift the day, so only the
        // separators and the time are checked.
        let stamp = 1_794_007_800_000;
        let english = Language::English.format_date(stamp);
        assert!(english.contains('-'), "{english}");
        assert!(english.contains(':'), "{english}");
        let german = Language::German.format_date(stamp);
        assert!(german.contains('.'), "{german}");
        let french = Language::French.format_date(stamp);
        assert!(french.contains('/'), "{french}");
        assert_eq!(Language::English.format_date(0), "");
        let day = chrono::NaiveDate::from_ymd_opt(2026, 11, 2).unwrap();
        assert_eq!(Language::German.format_day(day), "02.11.2026");
        assert_eq!(Language::English.format_day(day), "2026-11-02");
    }

    #[test]
    fn every_language_keeps_the_trainer_placeholders() {
        for language in Language::ALL {
            let strings = language.strings();
            assert!(strings.trainer_selected_count.contains("{count}"));
            assert!(strings.trainer_selected_count.contains("{total}"));
            assert!(strings.trainer_progress.contains("{current}"));
            assert!(strings.trainer_progress.contains("{total}"));
            assert!(strings.trainer_algorithm_hint.contains("{algorithm}"));
            assert!(strings.trainer_algorithm_invalid.contains("{move}"));
            assert!(strings.trainer_algorithm_wrong_case.contains("{case}"));
            assert!(strings.toast_trainer_load_failed.contains("{error}"));
            assert!(strings.toast_trainer_save_failed.contains("{error}"));
            assert!(strings.toast_trainer_scramble_failed.contains("{error}"));
        }
    }

    #[test]
    fn every_language_keeps_the_placeholders() {
        for language in Language::ALL {
            let strings = language.strings();
            assert!(strings.chart_range_last_solves.contains("{count}"));
            assert!(strings.chart_range_last_days.contains("{count}"));
            assert!(strings.chart_range_dates.contains("{from}"));
            assert!(strings.chart_range_dates.contains("{to}"));
            assert!(strings.chart_solve.contains("{number}"));
            assert!(strings.chart_zoom_range.contains("{first}"));
            assert!(strings.chart_zoom_range.contains("{last}"));
            assert!(strings.chart_year_summary.contains("{solves}"));
            assert!(strings.chart_year_summary.contains("{days}"));
            assert!(strings.chart_busiest_day.contains("{date}"));
            assert!(strings.chart_busiest_day.contains("{count}"));
            assert!(strings.chart_day.contains("{date}"));
            assert!(strings.chart_day.contains("{count}"));
            assert!(strings.average_range.contains("{first}"));
            assert!(strings.average_range.contains("{last}"));
            assert!(strings.mean_of.contains("{size}"));
            assert!(strings.average_of.contains("{size}"));
            assert!(strings.average_missing.contains("{label}"));
            assert!(strings.toast_new_best.contains("{label}"));
            assert!(strings.toast_new_best.contains("{time}"));
            assert!(strings.toast_celebration_audio_failed.contains("{error}"));
            assert!(strings.delete_session_confirm.contains("{name}"));
            assert!(strings.delete_session_confirm.contains("{count}"));
            assert!(strings.session_default_name.contains("{number}"));
            assert!(strings.invalid_scramble.contains("{event}"));
            assert!(strings.history_show.contains("{count}"));
            assert!(strings.history_revision.contains("{number}"));
            assert!(strings.history_revision.contains("{date}"));
            assert!(strings.toast_save_failed.contains("{error}"));
            assert!(strings.goal_summary_day.contains("{count}"));
            assert!(strings.goal_summary_week.contains("{count}"));
            assert!(strings.goal_summary_time.contains("{result}"));
            assert!(strings.goal_summary_time.contains("{time}"));
            assert!(strings.goal_summary_streak.contains("{count}"));
            assert!(strings.goal_summary_streak.contains("{days}"));
            assert!(strings.goal_error_count.contains("{max}"));
            assert!(strings.toast_goal_reached.contains("{goal}"));
            assert!(strings.tag_invalid.contains("{max}"));
            assert!(strings.toast_tags_load_failed.contains("{error}"));
            assert!(strings.tag_delete_confirm.contains("{tag}"));
            assert!(strings.tag_delete_confirm.contains("{count}"));
            assert!(strings.tags_active_label.contains("{tags}"));
            assert!(strings.toast_goals_load_failed.contains("{error}"));
            assert!(strings.toast_goals_save_failed.contains("{error}"));
            assert!(strings.toast_load_failed.contains("{error}"));
            assert!(strings.toast_backup_available.contains("{path}"));
            assert!(strings.toast_backup_done.contains("{path}"));
            assert!(strings.toast_backup_failed.contains("{error}"));
            assert!(strings.toast_history_merged.contains("{added}"));
            assert!(strings.toast_history_merged.contains("{updated}"));
            assert!(strings.toast_history_merged.contains("{kept}"));
            assert!(strings.toast_settings_reset.contains("{path}"));
            assert!(strings.theme_save_failed.contains("{error}"));
            assert!(strings.theme_delete_failed.contains("{error}"));
            assert!(strings.theme_load_failed.contains("{error}"));
            assert!(strings.theme_saved.contains("{name}"));
            assert!(strings.theme_delete_confirm.contains("{name}"));
            assert!(strings.theme_deleted.contains("{name}"));
            assert!(strings.toast_import_done.contains("{imported}"));
            assert!(strings.toast_import_done.contains("{skipped}"));
            assert!(strings.toast_export_done.contains("{path}"));
            assert!(strings.toast_import_failed.contains("{error}"));
            assert!(strings.toast_export_failed.contains("{error}"));
            assert!(strings.toast_audio_failed.contains("{error}"));
            assert!(strings.bluetooth_connecting.contains("{name}"));
            assert!(strings.bluetooth_connected.contains("{name}"));
            assert!(strings.bluetooth_scan_failed.contains("{error}"));
            assert!(strings.bluetooth_connect_failed.contains("{error}"));
            assert!(strings.bluetooth_disconnect_failed.contains("{error}"));
            assert!(strings.bluetooth_unsupported_state.contains("{state}"));
            assert!(strings.bluetooth_time_unreadable.contains("{error}"));
            assert!(strings.hint_start.contains("{key}"));
            assert!(strings.hint_inspecting.contains("{key}"));
            assert!(strings.hint_stop.contains("{key}"));
            assert!(strings.size_custom_invalid.contains("{min}"));
            assert!(strings.size_custom_invalid.contains("{max}"));
            assert!(strings.size_custom_range.contains("{min}"));
            assert!(strings.size_custom_range.contains("{max}"));
            assert!(strings.setting_best_min_solves_desc.contains("{min}"));
            assert!(strings.setting_best_min_solves_desc.contains("{max}"));
            assert!(strings.keybind_capture_hint.contains("{action}"));
            assert!(strings.keybind_error_conflict.contains("{action}"));
            assert!(strings.setting_skipping.contains("{version}"));
            assert!(strings.update_downloading.contains("{version}"));
            assert!(strings.update_updated_body.contains("{current}"));
            assert!(strings.update_check_failed_body.contains("{error}"));
            assert!(strings.update_uptodate_body.contains("{version}"));
            assert!(strings.update_available_body.contains("{version}"));
            assert!(strings.update_available_body.contains("{current}"));
            assert!(strings.update_skipped_body.contains("{version}"));
            assert!(strings.update_notes_title.contains("{version}"));
            assert!(strings.update_restart_body.contains("{version}"));
            assert!(strings.update_install_failed.contains("{error}"));
        }
    }
}
