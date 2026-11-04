//! GPUI user interface for cube.

// GPUI is designed to be glob-imported, and view helpers take `self` so every
// call site reads the same whether or not they need state today.
#![allow(clippy::wildcard_imports, clippy::unused_self)]

use gpui_kit::component::TitleBar;
use gpui_kit::*;

mod app;
mod audio;
mod details;
mod dialog_size;
mod dialog_submit;
mod fonts;
mod help;
mod palette;
mod panel_resize;
mod preview;
mod scroll_fade;
mod settings_view;
mod sidebar;
mod stats_panel;
mod stats_screen;
mod theme_editor;
mod timer_view;
mod update_ui;

/// Starts the application and blocks until the last window closes.
pub fn run() {
    gpui_kit::application()
        .with_assets(gpui_kit::assets::AllAssets)
        .run(|cx| {
            gpui_kit::init(cx);
            dialog_submit::assert_no_dialog_key_conflicts(cx);
            let state = app::Loaded::load();
            palette::apply(&state.settings, cx);

            let bounds = Bounds::centered(None, size(px(1180.), px(760.)), cx);
            let options = WindowOptions {
                window_bounds: Some(WindowBounds::Windowed(bounds)),
                window_min_size: Some(size(px(910.), px(560.))),
                ..TitleBar::window_options()
            };
            gpui_kit::open_window(options, cx, move |window, cx| {
                window.set_window_title("cube");
                cx.new(|cx| app::CubeApp::new(state, window, cx))
            })
            .expect("failed to open the main window");
            cx.activate(true);
        });
}
