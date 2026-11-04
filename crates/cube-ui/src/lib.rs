//! GPUI user interface for cube.

// GPUI is designed to be glob-imported, and view helpers take `self` so every
// call site reads the same whether or not they need state today.
#![allow(clippy::wildcard_imports, clippy::unused_self)]

use cube_core::window_state::Area;
use gpui_kit::component::TitleBar;
use gpui_kit::*;

mod app;
mod audio;
mod average_details;
mod bluetooth;
mod details;
mod dialog_size;
mod dialog_submit;
mod fonts;
mod help;
mod palette;
mod panel_resize;
mod preview;
mod scroll_fade;
mod search_bar;
mod settings_view;
mod sidebar;
mod stats_panel;
mod stats_screen;
mod theme_editor;
mod timer_view;
mod update_ui;

const MIN_WIDTH: f32 = 910.;
const MIN_HEIGHT: f32 = 560.;

/// The window bounds saved last time, if they still fit a connected monitor.
fn saved_bounds(settings: &cube_core::Settings, cx: &App) -> Option<WindowBounds> {
    let displays: Vec<Area> = cx
        .displays()
        .iter()
        .map(|display| {
            let bounds = display.visible_bounds();
            Area {
                x: f32::from(bounds.origin.x).round() as i32,
                y: f32::from(bounds.origin.y).round() as i32,
                width: f32::from(bounds.size.width).max(0.) as u32,
                height: f32::from(bounds.size.height).max(0.) as u32,
            }
        })
        .collect();
    let state = settings
        .window?
        .fit(&displays, (MIN_WIDTH as u32, MIN_HEIGHT as u32))?;
    let bounds = Bounds::new(
        point(px(state.x as f32), px(state.y as f32)),
        size(px(state.width as f32), px(state.height as f32)),
    );
    Some(if state.maximized {
        WindowBounds::Maximized(bounds)
    } else {
        WindowBounds::Windowed(bounds)
    })
}

/// Starts the application and blocks until the last window closes.
pub fn run() {
    gpui_kit::application()
        .with_assets(gpui_kit::assets::AllAssets)
        .run(|cx| {
            gpui_kit::init(cx);
            dialog_submit::assert_no_dialog_key_conflicts(cx);
            let state = app::Loaded::load();
            palette::apply(&state.settings, cx);

            let window_bounds = saved_bounds(&state.settings, cx).unwrap_or_else(|| {
                WindowBounds::Windowed(Bounds::centered(None, size(px(1180.), px(760.)), cx))
            });
            let options = WindowOptions {
                window_bounds: Some(window_bounds),
                window_min_size: Some(size(px(MIN_WIDTH), px(MIN_HEIGHT))),
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
