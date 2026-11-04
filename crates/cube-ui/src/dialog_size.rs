//! Sizing every dialog from the window it opens in.
//!
//! The rules belong to `cube_core::dialogs` so they can be tested without GPUI;
//! this module only turns them into pixels for the views.

use gpui_kit::*;

pub(crate) use cube_core::dialogs::DialogSize;

/// The width `size` asks for in `window`.
pub(crate) fn dialog_width(size: DialogSize, window: &Window) -> Pixels {
    px(size.width(f32::from(window.viewport_size().width)))
}

/// The tallest the scrolling body of `size` may get in `window`.
pub(crate) fn dialog_body_height(size: DialogSize, window: &Window) -> Pixels {
    px(size.body_height(f32::from(window.viewport_size().height)))
}
