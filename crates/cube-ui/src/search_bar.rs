//! The search field at the top of the shortcuts and settings dialogs.

use cube_locale::Strings;
use gpui_kit::component::Sizable as _;
use gpui_kit::component::input::{Input, InputEvent, InputState};
use gpui_kit::*;

/// Vertical space the bar takes above a dialog's scrolling body, which the body
/// gives up so the whole dialog keeps its height.
pub(crate) const BAR_HEIGHT: Pixels = px(56.);

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

/// The field, laid out above a dialog's scrolling body.
pub(crate) fn search_bar(input: &Entity<InputState>) -> Div {
    div()
        .h(BAR_HEIGHT)
        .px_4()
        .pt_4()
        .child(Input::new(input).small().w_full())
}

/// What the user has typed so far.
pub(crate) fn query(input: &Entity<InputState>, cx: &App) -> String {
    input.read(cx).value().trim().to_owned()
}
