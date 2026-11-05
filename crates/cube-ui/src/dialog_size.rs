//! Sizing every dialog from the window it opens in.
//!
//! The rules belong to `cube_core::dialogs` so they can be tested without GPUI;
//! this module only turns them into pixels for the views.

use gpui_kit::assets::IconName;
use gpui_kit::component::Size as ComponentSize;
use gpui_kit::component::button::{Button, ButtonVariants as _};
use gpui_kit::component::{Sizable as _, WindowExt as _, h_flex};
use gpui_kit::*;

pub(crate) use cube_core::dialogs::DialogSize;
use cube_core::dialogs::{BODY_INSET, NORMAL_REM_PX};

/// How many times larger than normal the text of `window` is.
pub(crate) fn text_scale(window: &Window) -> f32 {
    f32::from(window.rem_size()) / NORMAL_REM_PX
}

/// The width `size` asks for in `window`.
pub(crate) fn dialog_width(size: DialogSize, window: &Window) -> Pixels {
    px(size.width(f32::from(window.viewport_size().width), text_scale(window)))
}

/// The tallest the scrolling body of `size` may get in `window`.
pub(crate) fn dialog_body_height(size: DialogSize, window: &Window) -> Pixels {
    px(size.body_height(f32::from(window.viewport_size().height)))
}

/// Wraps a dialog's scroll region so a gap stays between its clipped content
/// and the divider above the footer at every scroll position.
pub(crate) fn dialog_body(scroll: impl IntoElement) -> Div {
    div().pb(px(BODY_INSET)).child(scroll)
}

/// A dialog title with the shared close `X` on its right.
///
/// Every closable dialog builds its title with this and sets
/// `.close_button(false)`, so the built-in (smaller) `X` never shows next to
/// it and all dialogs have the same button in the same place. The button is
/// a regular focusable `Button`, so `Tab` then `Enter`/`Space` closes it.
pub(crate) fn dialog_title(title: impl IntoElement, close_label: &'static str) -> impl IntoElement {
    h_flex()
        .w_full()
        .items_center()
        .justify_between()
        .pb_2()
        .child(title)
        .child(dialog_close_button(close_label))
}

/// The shared close `X`, for a dialog that lays out its own header.
pub(crate) fn dialog_close_button(close_label: &'static str) -> Button {
    Button::new("dialog-close-icon")
        .ghost()
        .with_size(ComponentSize::Size(px(36.)))
        .icon(IconName::Close)
        .tooltip(close_label)
        .accessibility_label(close_label)
        .on_click(|_, window, cx| window.close_dialog(cx))
}
