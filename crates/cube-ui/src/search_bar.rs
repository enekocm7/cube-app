//! The search field of the shortcuts and settings dialogs and the trainer.

use cube_core::{Action, Keybinds};
use cube_locale::Strings;

use crate::app::key_press;
use gpui_kit::assets::IconName;
use gpui_kit::component::input::{Input, InputEvent, InputState};
use gpui_kit::component::{ActiveTheme as _, Icon};
use gpui_kit::prelude::FluentBuilder as _;
use gpui_kit::*;

/// Vertical space the bar takes above a dialog's scrolling body, which the body
/// gives up so the whole dialog keeps its height.
pub(crate) const BAR_HEIGHT: Rems = Rems(4.25);

/// Creates the field and re-renders `V` whenever its text changes.
pub(crate) fn new_search<V: 'static>(
    strings: &'static Strings,
    window: &mut Window,
    cx: &mut Context<V>,
) -> Entity<InputState> {
    let input = cx.new(|cx| InputState::new(window, cx).placeholder(strings.search_placeholder));
    cx.subscribe_in(&input, window, |_, _, event: &InputEvent, _, cx| {
        if matches!(event, InputEvent::Change) {
            cx.notify();
        }
    })
    .detach();
    input
}

/// The field, laid out above a dialog's scrolling body. `hint` is the key that
/// jumps to it, shown while it is empty.
pub(crate) fn search_bar(input: &Entity<InputState>, hint: Option<String>, cx: &App) -> Div {
    div()
        .h(BAR_HEIGHT)
        .px_4()
        .pt_4()
        .child(search_field(input, hint, cx))
}

/// The field on its own, for wherever it is laid out. Without a `hint`, which
/// is the case when the search key is unbound, no key is shown.
pub(crate) fn search_field(input: &Entity<InputState>, hint: Option<String>, cx: &App) -> Input {
    let theme = cx.theme();
    let (muted, border) = (theme.muted_foreground, theme.border);
    let mono = theme.mono_font_family.clone();
    let empty = input.read(cx).value().is_empty();
    Input::new(input)
        .w_full()
        .cleanable(true)
        .prefix(Icon::new(IconName::Search).text_color(muted))
        .when_some(hint.filter(|_| empty), |input, hint| {
            input.suffix(
                div()
                    .px_1p5()
                    .rounded(px(4.))
                    .border_1()
                    .border_color(border)
                    .font_family(mono)
                    .text_xs()
                    .text_color(muted)
                    .child(hint),
            )
        })
}

/// Moves focus to the field when the configured search key is pressed anywhere
/// else in the dialog. Returns whether the key was taken, so it is not typed
/// into the field.
pub(crate) fn focus_on_search_key(
    event: &KeyDownEvent,
    keybinds: &Keybinds,
    input: &Entity<InputState>,
    window: &mut Window,
    cx: &mut App,
) -> bool {
    let focus = input.read(cx).focus_handle(cx);
    if event.is_held || focus.is_focused(window) {
        return false;
    }
    let Some(press) = key_press(&event.keystroke) else {
        return false;
    };
    if !keybinds.is_pressed(Action::FocusSearch, &press) {
        return false;
    }
    focus.focus(window, cx);
    true
}

/// What the user has typed so far.
pub(crate) fn query(input: &Entity<InputState>, cx: &App) -> String {
    input.read(cx).value().trim().to_owned()
}
