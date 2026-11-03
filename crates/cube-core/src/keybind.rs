//! Keyboard actions and their bindings.
//!
//! Bindings live in [`Settings::keybindings`](crate::Settings::keybindings) as
//! text such as `"Ctrl+N"` or `"Space"`. Only overrides are stored: anything
//! missing falls back to [`Keybinds::default`], so a settings file written by
//! an older version keeps working and a broken entry never makes the app
//! unusable.

use std::collections::BTreeMap;
use std::fmt;

/// Something the user can do from the keyboard.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum Action {
    Timer,
    Cancel,
    NextScramble,
    ToggleInspection,
    PreviousSolve,
    NextSolve,
    OpenSolve,
    DeleteSolve,
    PenaltyPlusTwo,
    PenaltyDnf,
    NextEvent,
    PreviousEvent,
    NextSession,
    PreviousSession,
    NewSession,
    RenameSession,
    DeleteSession,
    OpenStats,
    OpenSettings,
    ShowHelp,
}

impl Action {
    /// Every action, in the order the help dialog lists them.
    pub const ALL: [Self; 20] = [
        Self::Timer,
        Self::Cancel,
        Self::NextScramble,
        Self::ToggleInspection,
        Self::PreviousSolve,
        Self::NextSolve,
        Self::OpenSolve,
        Self::DeleteSolve,
        Self::PenaltyPlusTwo,
        Self::PenaltyDnf,
        Self::NextEvent,
        Self::PreviousEvent,
        Self::NextSession,
        Self::PreviousSession,
        Self::NewSession,
        Self::RenameSession,
        Self::DeleteSession,
        Self::OpenStats,
        Self::OpenSettings,
        Self::ShowHelp,
    ];

    /// Returns the name used as the key of a `[keybindings]` entry.
    pub const fn name(self) -> &'static str {
        match self {
            Self::Timer => "timer",
            Self::Cancel => "cancel",
            Self::NextScramble => "next_scramble",
            Self::ToggleInspection => "toggle_inspection",
            Self::PreviousSolve => "previous_solve",
            Self::NextSolve => "next_solve",
            Self::OpenSolve => "open_solve",
            Self::DeleteSolve => "delete_solve",
            Self::PenaltyPlusTwo => "penalty_plus_two",
            Self::PenaltyDnf => "penalty_dnf",
            Self::NextEvent => "next_event",
            Self::PreviousEvent => "previous_event",
            Self::NextSession => "next_session",
            Self::PreviousSession => "previous_session",
            Self::NewSession => "new_session",
            Self::RenameSession => "rename_session",
            Self::DeleteSession => "delete_session",
            Self::OpenStats => "open_stats",
            Self::OpenSettings => "open_settings",
            Self::ShowHelp => "show_help",
        }
    }

    /// Matches an action name, ignoring case.
    pub fn from_name(name: &str) -> Option<Self> {
        Self::ALL
            .into_iter()
            .find(|action| action.name().eq_ignore_ascii_case(name.trim()))
    }
}

/// A key press as the application sees it, normalized so that bindings compare
/// equally on every platform.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct KeyPress {
    key: String,
    ctrl: bool,
    alt: bool,
    shift: bool,
}

impl KeyPress {
    /// Builds a press from a platform keystroke, folding shifted letters and
    /// platform keys (Cmd on macOS, Win on Windows) into the shared model.
    ///
    /// Returns `None` when the key is not one the bindings understand.
    pub fn new(key: &str, ctrl: bool, alt: bool, shift: bool) -> Option<Self> {
        let (key, uppercase) = normalize_key(key)?;
        Some(Self {
            key,
            ctrl,
            alt,
            shift: shift || uppercase,
        })
    }

    /// Returns the normalized key, such as `n`, `space` or `f2`.
    pub fn key(&self) -> &str {
        &self.key
    }
}

/// A single key with optional modifiers.
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub struct KeyBinding {
    key: String,
    ctrl: bool,
    alt: bool,
    shift: bool,
}

impl KeyBinding {
    /// Parses `"Ctrl+Shift+N"`, `"ctrl-n"`, `"Space"` or `"F2"`.
    ///
    /// Separators are `+` and `-`; names and modifiers are case-insensitive.
    pub fn parse(input: &str) -> Option<Self> {
        let input = input.trim();
        if input.is_empty() {
            return None;
        }

        let mut parts: Vec<&str> = input.split('+').collect();
        // Accept GPUI-style `ctrl-n` too, but leave `page-down` as a key name.
        if parts.len() == 1 && input.contains('-') {
            let dashed: Vec<&str> = input.split('-').collect();
            if dashed.len() > 1
                && dashed[..dashed.len() - 1]
                    .iter()
                    .all(|part| is_modifier(part))
            {
                parts = dashed;
            }
        }

        // Shift has to be written out (`Shift+N`); an uppercase key is only
        // shorthand for the same letter, so `Ctrl+N` means Ctrl+n.
        let (key, _) = normalize_key(parts.last()?.trim())?;
        let mut binding = Self {
            key,
            ctrl: false,
            alt: false,
            shift: false,
        };
        for part in &parts[..parts.len() - 1] {
            match part.trim().to_ascii_lowercase().as_str() {
                "ctrl" | "control" | "cmd" | "super" | "win" | "meta" => binding.ctrl = true,
                "alt" | "opt" | "option" => binding.alt = true,
                "shift" => binding.shift = true,
                _ => return None,
            }
        }
        Some(binding)
    }

    /// Returns whether any modifier is required.
    pub const fn is_modified(&self) -> bool {
        self.ctrl || self.alt || self.shift
    }

    /// Returns whether this binding matches a normalized press.
    pub fn matches(&self, press: &KeyPress) -> bool {
        self.key == press.key
            && self.ctrl == press.ctrl
            && self.alt == press.alt
            && self.shift == press.shift
    }
}

impl fmt::Display for KeyBinding {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        if self.ctrl {
            formatter.write_str("Ctrl+")?;
        }
        if self.alt {
            formatter.write_str("Alt+")?;
        }
        if self.shift {
            formatter.write_str("Shift+")?;
        }
        formatter.write_str(&display_key(&self.key))
    }
}

/// Every action's effective binding.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Keybinds(BTreeMap<Action, KeyBinding>);

impl Default for Keybinds {
    fn default() -> Self {
        let bindings = [
            (Action::Timer, "Space"),
            (Action::Cancel, "Escape"),
            (Action::NextScramble, "n"),
            (Action::ToggleInspection, "i"),
            (Action::PreviousSolve, "Up"),
            (Action::NextSolve, "Down"),
            (Action::OpenSolve, "Enter"),
            (Action::DeleteSolve, "d"),
            (Action::PenaltyPlusTwo, "2"),
            (Action::PenaltyDnf, "Shift+D"),
            (Action::NextEvent, "e"),
            (Action::PreviousEvent, "Shift+E"),
            (Action::NextSession, "]"),
            (Action::PreviousSession, "["),
            (Action::NewSession, "s"),
            (Action::RenameSession, "F2"),
            (Action::DeleteSession, "Shift+S"),
            (Action::OpenStats, "a"),
            (Action::OpenSettings, "Ctrl+,"),
            (Action::ShowHelp, "F1"),
        ];
        Self(
            bindings
                .into_iter()
                .map(|(action, binding)| {
                    (
                        action,
                        KeyBinding::parse(binding).expect("default keybindings are valid"),
                    )
                })
                .collect(),
        )
    }
}

impl Keybinds {
    /// Merges configured overrides with the defaults.
    ///
    /// Unknown action names, unreadable keys, a modified timer key and
    /// bindings that would collide with another action are ignored.
    pub fn from_overrides(overrides: &BTreeMap<String, String>) -> Self {
        let mut bindings = Self::default();
        for (name, text) in overrides {
            let Some(action) = Action::from_name(name) else {
                continue;
            };
            let Some(binding) = KeyBinding::parse(text) else {
                continue;
            };
            if action == Action::Timer && binding.is_modified() {
                continue;
            }
            if bindings
                .0
                .iter()
                .any(|(other, current)| *other != action && *current == binding)
            {
                continue;
            }
            bindings.0.insert(action, binding);
        }
        bindings
    }

    /// Returns an action's binding, which always exists.
    pub fn get(&self, action: Action) -> &KeyBinding {
        &self.0[&action]
    }

    /// Returns an action's binding as it should be shown to the user.
    pub fn label(&self, action: Action) -> String {
        self.get(action).to_string()
    }

    /// Finds the action bound to a press.
    pub fn action_for(&self, press: &KeyPress) -> Option<Action> {
        self.0
            .iter()
            .find_map(|(action, binding)| binding.matches(press).then_some(*action))
    }
}

fn is_modifier(part: &str) -> bool {
    matches!(
        part.trim().to_ascii_lowercase().as_str(),
        "ctrl" | "control" | "cmd" | "super" | "win" | "meta" | "alt" | "opt" | "option" | "shift"
    )
}

/// Returns the canonical key name and whether an uppercase letter implied
/// Shift. Named keys stay as GPUI spells them (`space`, `escape`, `pageup`).
fn normalize_key(raw: &str) -> Option<(String, bool)> {
    let mut chars = raw.chars();
    let first = chars.next()?;
    if chars.next().is_none() {
        if first == ' ' {
            return Some(("space".to_owned(), false));
        }
        return if first.is_ascii_uppercase() {
            Some((first.to_ascii_lowercase().to_string(), true))
        } else {
            Some((first.to_string(), false))
        };
    }

    let lower = raw.to_ascii_lowercase();
    let key = match lower.as_str() {
        "space" | "spacebar" => "space",
        "esc" | "escape" => "escape",
        "enter" | "return" => "enter",
        "tab" => "tab",
        "backspace" => "backspace",
        "delete" | "del" => "delete",
        "insert" | "ins" => "insert",
        "home" => "home",
        "end" => "end",
        "pageup" | "page-up" | "pgup" => "pageup",
        "pagedown" | "page-down" | "pgdn" => "pagedown",
        "up" | "arrowup" | "arrow-up" => "up",
        "down" | "arrowdown" | "arrow-down" => "down",
        "left" | "arrowleft" | "arrow-left" => "left",
        "right" | "arrowright" | "arrow-right" => "right",
        _ => {
            let number = lower
                .strip_prefix('f')
                .and_then(|number| number.parse::<u8>().ok())
                .filter(|number| (1..=24).contains(number))?;
            return Some((format!("f{number}"), false));
        }
    };
    Some((key.to_owned(), false))
}

fn display_key(key: &str) -> String {
    match key {
        "space" => "Space".to_owned(),
        "escape" => "Escape".to_owned(),
        "enter" => "Enter".to_owned(),
        "tab" => "Tab".to_owned(),
        "backspace" => "Backspace".to_owned(),
        "delete" => "Delete".to_owned(),
        "insert" => "Insert".to_owned(),
        "home" => "Home".to_owned(),
        "end" => "End".to_owned(),
        "pageup" => "PageUp".to_owned(),
        "pagedown" => "PageDown".to_owned(),
        "up" => "Up".to_owned(),
        "down" => "Down".to_owned(),
        "left" => "Left".to_owned(),
        "right" => "Right".to_owned(),
        key if key.chars().count() == 1 => key.to_ascii_uppercase(),
        key => key.to_uppercase(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn defaults_are_unique_and_cover_every_action() {
        let keybinds = Keybinds::default();
        let mut seen = std::collections::HashSet::new();
        for action in Action::ALL {
            assert!(
                seen.insert(keybinds.label(action)),
                "{action:?} shares a binding with another action"
            );
        }
        assert_eq!(seen.len(), Action::ALL.len());
        assert!(!keybinds.get(Action::Timer).is_modified());
    }

    #[test]
    fn bindings_round_trip_through_text() {
        for text in [
            "Space",
            "Ctrl+Shift+N",
            "ctrl-n",
            "F2",
            "Shift+D",
            "PageDown",
            "Ctrl+,",
        ] {
            let binding = KeyBinding::parse(text).unwrap();
            let display = binding.to_string();
            assert_eq!(
                KeyBinding::parse(&display).unwrap(),
                binding,
                "{text} became {display}"
            );
        }
    }

    #[test]
    fn partial_overrides_keep_the_other_defaults() {
        let overrides = BTreeMap::from([("next_scramble".to_owned(), "Ctrl+N".to_owned())]);
        let keybinds = Keybinds::from_overrides(&overrides);
        assert_eq!(keybinds.label(Action::NextScramble), "Ctrl+N");
        assert_eq!(keybinds.label(Action::Timer), "Space");
    }

    #[test]
    fn unknown_actions_and_broken_keys_are_ignored() {
        let overrides = BTreeMap::from([
            ("not_an_action".to_owned(), "x".to_owned()),
            ("next_scramble".to_owned(), "Ctrl+NotAKey".to_owned()),
        ]);
        let keybinds = Keybinds::from_overrides(&overrides);
        assert_eq!(keybinds.label(Action::NextScramble), "N");
    }

    #[test]
    fn a_modified_timer_key_is_rejected() {
        let overrides = BTreeMap::from([("timer".to_owned(), "Ctrl+Space".to_owned())]);
        let keybinds = Keybinds::from_overrides(&overrides);
        assert_eq!(keybinds.label(Action::Timer), "Space");
    }

    #[test]
    fn a_duplicate_binding_is_rejected() {
        let overrides = BTreeMap::from([("next_scramble".to_owned(), "space".to_owned())]);
        let keybinds = Keybinds::from_overrides(&overrides);
        assert_eq!(keybinds.label(Action::NextScramble), "N");
    }

    #[test]
    fn shifted_letters_are_normalized() {
        let binding = KeyBinding::parse("Shift+E").unwrap();
        let press = KeyPress::new("E", false, false, false).unwrap();
        assert_eq!(binding.to_string(), "Shift+E");
        assert!(binding.matches(&press));
    }

    #[test]
    fn a_capital_letter_from_the_platform_means_shift() {
        let binding = KeyBinding::parse("Shift+n").unwrap();
        assert!(binding.matches(&KeyPress::new("N", false, false, false).unwrap()));
        assert!(binding.matches(&KeyPress::new("n", false, false, true).unwrap()));
        assert!(
            !KeyBinding::parse("n")
                .unwrap()
                .matches(&KeyPress::new("N", false, false, false).unwrap())
        );
    }

    #[test]
    fn unsupported_and_unknown_keys_are_rejected() {
        assert!(KeyBinding::parse("F25").is_none());
        assert!(KeyBinding::parse("Ctrl+NotAKey").is_none());
        assert!(KeyBinding::parse("").is_none());
        assert!(KeyBinding::parse("Ctrl+").is_none());
    }

    #[test]
    fn action_names_round_trip() {
        for action in Action::ALL {
            assert_eq!(Action::from_name(action.name()), Some(action));
        }
        assert_eq!(
            Action::from_name("NEXT_SCRAMBLE"),
            Some(Action::NextScramble)
        );
        assert_eq!(Action::from_name("nope"), None);
    }

    #[test]
    fn presses_resolve_to_their_action() {
        let keybinds = Keybinds::default();
        let press = |key: &str, ctrl, alt, shift| KeyPress::new(key, ctrl, alt, shift).unwrap();
        assert_eq!(
            keybinds.action_for(&press("n", false, false, false)),
            Some(Action::NextScramble)
        );
        assert_eq!(
            keybinds.action_for(&press("E", false, false, false)),
            Some(Action::PreviousEvent)
        );
        assert_eq!(
            keybinds.action_for(&press("Escape", false, false, false)),
            Some(Action::Cancel)
        );
        assert_eq!(keybinds.action_for(&press("n", true, false, false)), None);
    }
}
