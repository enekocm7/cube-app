//! The categories of the settings dialog and how the keyboard moves through
//! them.

/// One entry of the settings dialog's category list.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum SettingsCategory {
    #[default]
    General,
    Timer,
    Appearance,
    Library,
    Shortcuts,
    Data,
    Updates,
}

impl SettingsCategory {
    /// Every category, in the order the list shows them.
    pub const ALL: [Self; 7] = [
        Self::General,
        Self::Timer,
        Self::Appearance,
        Self::Library,
        Self::Shortcuts,
        Self::Data,
        Self::Updates,
    ];

    /// The category `step` places after this one in `list`, wrapping around at
    /// the ends. A category missing from `list` starts from its first entry,
    /// or its last when moving backwards.
    ///
    /// Returns `None` for an empty list.
    #[must_use]
    pub fn step_in(self, list: &[Self], step: isize) -> Option<Self> {
        let len = list.len().cast_signed();
        if len == 0 {
            return None;
        }
        let target = match list.iter().position(|category| *category == self) {
            Some(index) => index.cast_signed() + step,
            None if step < 0 => step,
            None => step - 1,
        };
        Some(list[target.rem_euclid(len).cast_unsigned()])
    }
}

#[cfg(test)]
mod tests {
    use super::SettingsCategory::{self, Appearance, Data, General, Timer, Updates};

    #[test]
    fn stepping_moves_through_the_list_in_order() {
        assert_eq!(General.step_in(&SettingsCategory::ALL, 1), Some(Timer));
        assert_eq!(Appearance.step_in(&SettingsCategory::ALL, -1), Some(Timer));
    }

    #[test]
    fn stepping_wraps_around_at_both_ends() {
        assert_eq!(Updates.step_in(&SettingsCategory::ALL, 1), Some(General));
        assert_eq!(General.step_in(&SettingsCategory::ALL, -1), Some(Updates));
    }

    #[test]
    fn a_category_outside_a_filtered_list_starts_from_its_edge() {
        let list = [Timer, Data];
        assert_eq!(General.step_in(&list, 1), Some(Timer));
        assert_eq!(General.step_in(&list, -1), Some(Data));
    }

    #[test]
    fn an_empty_list_has_nowhere_to_go() {
        assert_eq!(General.step_in(&[], 1), None);
    }

    #[test]
    fn the_first_category_is_the_default() {
        assert_eq!(SettingsCategory::default(), SettingsCategory::ALL[0]);
    }
}
