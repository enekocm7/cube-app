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
    /// `{count}`: how many times the solve was edited.
    pub edited_times: &'static str,
    pub invalid_time: &'static str,

    pub save: &'static str,
    pub cancel: &'static str,
    pub close: &'static str,
    pub delete: &'static str,
    pub delete_solve: &'static str,
    pub delete_solve_confirm: &'static str,
    pub solve_deleted: &'static str,

    pub settings_timer: &'static str,
    pub settings_appearance: &'static str,
    pub settings_data: &'static str,
    pub setting_inspection: &'static str,
    pub setting_toasts: &'static str,
    pub setting_accurate: &'static str,
    pub setting_solve_list_ao5: &'static str,
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

    /// `{label}`: the statistic; `{time}`: its new value.
    pub toast_new_best: &'static str,
    pub toast_plus_two: &'static str,
    pub toast_dnf: &'static str,
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
            assert!(strings.edited_times.contains("{count}"));
            assert!(strings.toast_save_failed.contains("{error}"));
            assert!(strings.toast_load_failed.contains("{error}"));
            assert!(strings.toast_import_done.contains("{imported}"));
            assert!(strings.toast_import_done.contains("{skipped}"));
            assert!(strings.toast_export_done.contains("{path}"));
            assert!(strings.toast_import_failed.contains("{error}"));
            assert!(strings.toast_export_failed.contains("{error}"));
        }
    }
}
