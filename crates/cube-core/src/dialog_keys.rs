//! What Enter means inside a dialog, and the check that keeps it unambiguous.
//!
//! A dialog binds Enter to one `Confirm` action, which fires before a focused
//! control sees the key. The UI reports what holds focus; this module decides
//! who owns the press, so one Enter runs at most one thing.

use std::collections::BTreeMap;

/// What held focus in a dialog when Enter was pressed.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum EnterFocus {
    /// The dialog itself, or a part of it that is not a control.
    Surface,
    /// A text field that passed Enter on. A multi-line field keeps the key to
    /// insert a newline, and an open completion menu keeps it to accept an
    /// item, so neither reaches the dialog.
    TextInput,
    /// A button, switch or other control, which activates itself.
    Control,
}

impl EnterFocus {
    /// Whether Enter runs the dialog's primary action.
    ///
    /// A focused control keeps the key: Enter on a focused Cancel button must
    /// cancel, not run the Delete beside it.
    pub const fn submits_dialog(self) -> bool {
        matches!(self, Self::Surface | Self::TextInput)
    }
}

/// One key binding, as registered with the UI toolkit.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct KeyClaim<'a> {
    /// The context the binding applies in; the innermost one wins a key.
    pub scope: &'a str,
    /// The key, such as `enter`.
    pub keystroke: &'a str,
    /// The action it triggers.
    pub action: &'a str,
}

/// Two or more actions claiming the same key in the same scope.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct KeyConflict {
    /// The scope they share.
    pub scope: String,
    /// The key they share.
    pub keystroke: String,
    /// The competing actions, sorted.
    pub actions: Vec<String>,
}

/// Finds keys that more than one action claims within one scope.
///
/// Different scopes may share a key, since only the innermost handles it. In
/// the same scope the later binding would silently shadow the earlier one.
/// Binding an action twice is harmless and is not reported.
pub fn key_conflicts(claims: &[KeyClaim<'_>]) -> Vec<KeyConflict> {
    let mut actions: BTreeMap<(&str, &str), Vec<&str>> = BTreeMap::new();
    for claim in claims {
        let claimed = actions.entry((claim.scope, claim.keystroke)).or_default();
        if !claimed.contains(&claim.action) {
            claimed.push(claim.action);
        }
    }
    actions
        .into_iter()
        .filter(|(_, actions)| actions.len() > 1)
        .map(|((scope, keystroke), mut actions)| {
            actions.sort_unstable();
            KeyConflict {
                scope: scope.to_owned(),
                keystroke: keystroke.to_owned(),
                actions: actions.into_iter().map(str::to_owned).collect(),
            }
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn claim<'a>(scope: &'a str, keystroke: &'a str, action: &'a str) -> KeyClaim<'a> {
        KeyClaim {
            scope,
            keystroke,
            action,
        }
    }

    #[test]
    fn enter_submits_from_the_dialog_and_from_a_text_field() {
        assert!(EnterFocus::Surface.submits_dialog());
        assert!(EnterFocus::TextInput.submits_dialog());
    }

    #[test]
    fn a_focused_control_keeps_enter_for_itself() {
        assert!(!EnterFocus::Control.submits_dialog());
    }

    #[test]
    fn two_actions_on_one_key_in_one_scope_conflict() {
        let conflicts = key_conflicts(&[
            claim("Dialog", "enter", "Confirm"),
            claim("Dialog", "escape", "Cancel"),
            claim("Dialog", "enter", "Submit"),
        ]);
        assert_eq!(
            conflicts,
            [KeyConflict {
                scope: "Dialog".to_owned(),
                keystroke: "enter".to_owned(),
                actions: vec!["Confirm".to_owned(), "Submit".to_owned()],
            }]
        );
    }

    #[test]
    fn scopes_may_share_a_key() {
        let claims = [
            claim("Dialog", "enter", "Confirm"),
            claim("Input", "enter", "Enter"),
        ];
        assert_eq!(key_conflicts(&claims), Vec::<KeyConflict>::new());
    }

    #[test]
    fn repeating_a_binding_is_not_a_conflict() {
        let claims = [
            claim("Dialog", "enter", "Confirm"),
            claim("Dialog", "enter", "Confirm"),
        ];
        assert_eq!(key_conflicts(&claims), Vec::<KeyConflict>::new());
    }
}
