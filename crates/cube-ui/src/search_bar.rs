//! The search field at the top of the shortcuts and settings dialogs.

use cube_locale::Strings;
use gpui_kit::assets::IconName;
use gpui_kit::component::input::{Input, InputEvent, InputState};
use gpui_kit::component::{ActiveTheme as _, Icon};
use gpui_kit::prelude::FluentBuilder as _;
use gpui_kit::*;

/// Vertical space the bar takes above a dialog's scrolling body, which the body
/// gives up so the whole dialog keeps its height.
pub(crate) const BAR_HEIGHT: Pixels = px(68.);

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
pub(crate) fn search_bar(input: &Entity<InputState>, cx: &App) -> Div {
    let theme = cx.theme();
    let (muted, border) = (theme.muted_foreground, theme.border);
    let mono = theme.mono_font_family.clone();
    let empty = input.read(cx).value().is_empty();
    div().h(BAR_HEIGHT).px_4().pt_4().child(
        Input::new(input)
            .w_full()
            .cleanable(true)
            .prefix(Icon::new(IconName::Search).text_color(muted))
            .when(empty, |input| {
                // Shows which key jumps to the field.
                input.suffix(
                    div()
                        .px_1p5()
                        .rounded(px(4.))
                        .border_1()
                        .border_color(border)
                        .font_family(mono)
                        .text_xs()
                        .text_color(muted)
                        .child("/"),
                )
            }),
    )
}

/// Moves focus to the field when `/` is pressed anywhere else in the dialog.
/// Returns whether the key was taken, so the `/` is not typed into the field.
pub(crate) fn focus_on_slash(
    event: &KeyDownEvent,
    input: &Entity<InputState>,
    window: &mut Window,
    cx: &mut App,
) -> bool {
    let keystroke = &event.keystroke;
    let plain =
        !keystroke.modifiers.control && !keystroke.modifiers.alt && !keystroke.modifiers.platform;
    let focus = input.read(cx).focus_handle(cx);
    if event.is_held
        || !plain
        || keystroke.key_char.as_deref() != Some("/")
        || focus.is_focused(window)
    {
        return false;
    }
    focus.focus(window, cx);
    true
}

/// What the user has typed so far.
pub(crate) fn query(input: &Entity<InputState>, cx: &App) -> String {
    input.read(cx).value().trim().to_owned()
}
