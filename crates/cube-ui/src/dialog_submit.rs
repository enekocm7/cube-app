//! Routes Enter in a dialog to its primary action.
//!
//! GPUI Kit binds Enter to `Confirm` in the dialog's key context, and a key
//! binding fires before a focused control sees the key. Every dialog therefore
//! installs [`DialogSubmit::submit_on_enter`] or
//! [`DialogSubmit::close_on_enter`] rather than a bare `on_ok`. Escape stays
//! the toolkit's `Cancel`, which never reaches these handlers.
//!
//! Scopes resolve themselves: the deepest key context takes the key, so a
//! multi-line field or an open menu keeps Enter, and the app's own shortcuts
//! stand down while a dialog is open.

use std::rc::Rc;

use cube_core::dialog_keys::{EnterFocus, KeyClaim, key_conflicts};
use gpui_kit::component::WindowExt as _;
use gpui_kit::component::dialog::Dialog;
use gpui_kit::*;

/// A dialog's primary action, shared by its footer button and Enter so both
/// behave the same. It closes the dialog itself, when it should close at all.
pub(crate) type Submit = Rc<dyn Fn(&mut Window, &mut App)>;

/// The key context the toolkit gives a dialog.
const DIALOG_CONTEXT: &str = "Dialog";

/// What holds focus right now, as far as Enter is concerned.
fn enter_focus(window: &mut Window, cx: &mut App) -> EnterFocus {
    if window.has_focused_input(cx) {
        EnterFocus::TextInput
    } else if window.focused(cx).is_some_and(|focus| focus.tab_stop) {
        EnterFocus::Control
    } else {
        EnterFocus::Surface
    }
}

/// Handles one Enter press in a dialog.
///
/// Always returns `false`, so the toolkit never closes the dialog on its own:
/// `submit` does that. Closing afterwards would be a second path, and would
/// take the wrong dialog when `submit` has just opened another one.
pub(crate) fn route_enter(submit: &Submit, window: &mut Window, cx: &mut App) -> bool {
    if enter_focus(window, cx).submits_dialog() {
        submit(window, cx);
    } else {
        // Let the key go on, so the focused control activates on key up.
        cx.propagate();
    }
    false
}

/// Closes the topmost dialog, for dialogs with no other action.
pub(crate) fn close_action() -> Submit {
    Rc::new(Window::close_dialog)
}

/// Runs `submit` when its footer button is pressed.
pub(crate) fn on_press(submit: &Submit) -> impl Fn(&ClickEvent, &mut Window, &mut App) + 'static {
    let submit = Rc::clone(submit);
    move |_, window, cx| submit(window, cx)
}

pub(crate) trait DialogSubmit {
    /// Makes Enter run the dialog's primary action.
    fn submit_on_enter(self, submit: &Submit) -> Self;

    /// Makes Enter close a dialog that has no primary action.
    fn close_on_enter(self) -> Self;
}

impl DialogSubmit for Dialog {
    fn submit_on_enter(self, submit: &Submit) -> Self {
        let submit = Rc::clone(submit);
        self.on_ok(move |_, window, cx| route_enter(&submit, window, cx))
    }

    fn close_on_enter(self) -> Self {
        self.submit_on_enter(&close_action())
    }
}

/// Panics in debug builds when two actions claim one key in the dialog scope,
/// where the later binding would silently shadow `Confirm` or `Cancel`.
pub(crate) fn assert_no_dialog_key_conflicts(cx: &App) {
    if !cfg!(debug_assertions) {
        return;
    }
    let keymap = cx.key_bindings();
    let keymap = keymap.borrow();
    let bindings: Vec<(String, String, &'static str)> = keymap
        .bindings()
        .filter_map(|binding| {
            let scope = binding.predicate()?.to_string();
            let [keystroke] = binding.keystrokes() else {
                return None;
            };
            let keystroke = keystroke.as_keystroke().unparse(cx.platform_info().os);
            Some((scope, keystroke, binding.action().name()))
        })
        .filter(|(scope, _, _)| scope == DIALOG_CONTEXT)
        .collect();
    let claims: Vec<KeyClaim<'_>> = bindings
        .iter()
        .map(|(scope, keystroke, action)| KeyClaim {
            scope,
            keystroke,
            action,
        })
        .collect();
    let conflicts = key_conflicts(&claims);
    assert!(conflicts.is_empty(), "dialog key conflict: {conflicts:?}");
}
