//! UI string tables.
//!
//! Each language is one [`Strings`] value, so a missing translation is a compile
//! error rather than a blank label at runtime.

mod en;
mod es;

/// A supported interface language.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum Language {
    #[default]
    English,
    Spanish,
}

impl Language {
    /// Every language, in the order shown in settings.
    pub const ALL: [Self; 2] = [Self::English, Self::Spanish];

    /// Returns the code stored in the settings file.
    pub const fn code(self) -> &'static str {
        match self {
            Self::English => "en",
            Self::Spanish => "es",
        }
    }

    /// Returns the language's name, written in that language.
    pub const fn native_name(self) -> &'static str {
        match self {
            Self::English => "English",
            Self::Spanish => "Español",
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
            Self::Spanish => &es::STRINGS,
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
    pub settings: &'static str,

    pub hint_start: &'static str,
    pub hint_hold: &'static str,
    pub hint_release: &'static str,
    pub hint_inspecting: &'static str,
    pub hint_stop: &'static str,
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
    pub chart_averages: &'static str,
    pub chart_distribution: &'static str,
    pub chart_empty: &'static str,

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

    pub settings_timer: &'static str,
    pub settings_appearance: &'static str,
    pub settings_theme: &'static str,
    pub settings_data: &'static str,
    pub setting_inspection: &'static str,
    pub setting_inspection_audio: &'static str,
    pub setting_toasts: &'static str,
    pub setting_accurate: &'static str,
    pub setting_solve_list_ao5: &'static str,
    pub setting_font: &'static str,
    pub font_system: &'static str,
    pub font_timer: &'static str,
    pub font_times: &'static str,
    pub font_whole: &'static str,
    pub setting_theme: &'static str,
    pub setting_language: &'static str,
    pub setting_import: &'static str,
    pub setting_export: &'static str,
    pub import_button: &'static str,
    pub export_button: &'static str,
    /// The label of the import file picker's confirm button.
    pub import_prompt: &'static str,
    pub theme_system: &'static str,
    pub theme_light: &'static str,
    pub theme_dark: &'static str,

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
    pub toast_plus_two: &'static str,
    pub toast_dnf: &'static str,
    /// `{error}`: why the cue could not be played.
    pub toast_audio_failed: &'static str,
    pub toast_cancelled: &'static str,
    /// `{error}`: the cause.
    pub toast_save_failed: &'static str,
    /// `{error}`: the cause.
    pub toast_load_failed: &'static str,
    /// `{imported}`: how many solves were added; `{skipped}`: how many were not.
    pub toast_import_done: &'static str,
    /// `{path}`: the file that was written.
    pub toast_export_done: &'static str,
    /// `{error}`: the cause.
    pub toast_import_failed: &'static str,
    /// `{error}`: the cause.
    pub toast_export_failed: &'static str,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn codes_match_with_or_without_a_region() {
        assert_eq!(Language::from_code("es"), Some(Language::Spanish));
        assert_eq!(Language::from_code("es-ES"), Some(Language::Spanish));
        assert_eq!(Language::from_code("EN_gb"), Some(Language::English));
        assert_eq!(Language::from_code("fr"), None);
        assert_eq!(Language::from_code(""), None);
    }

    #[test]
    fn every_language_keeps_the_placeholders() {
        for language in Language::ALL {
            let strings = language.strings();
            assert!(strings.toast_new_best.contains("{label}"));
            assert!(strings.toast_new_best.contains("{time}"));
            assert!(strings.delete_session_confirm.contains("{name}"));
            assert!(strings.delete_session_confirm.contains("{count}"));
            assert!(strings.session_default_name.contains("{number}"));
            assert!(strings.invalid_scramble.contains("{event}"));
            assert!(strings.history_show.contains("{count}"));
            assert!(strings.history_revision.contains("{number}"));
            assert!(strings.history_revision.contains("{date}"));
            assert!(strings.toast_save_failed.contains("{error}"));
            assert!(strings.toast_load_failed.contains("{error}"));
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
