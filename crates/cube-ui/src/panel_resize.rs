//! Dragging the inner edges of the side panels to resize them.
//!
//! GPUI supplies the drag; the limits come from `cube_core::panels`. A drag
//! updates the settings as it moves, and they are written to disk when the
//! mouse is released, not on every pixel.

use cube_core::panels::{
    DEFAULT_SIDEBAR_WIDTH, DEFAULT_STATS_WIDTH, GRAB_WIDTH, clamp_panel_width, fit_panels,
};
use gpui_kit::component::{ActiveTheme as _, InteractiveElementExt as _};
use gpui_kit::*;

use crate::app::CubeApp;

/// Which panel's inner edge is being resized.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum PanelSide {
    Sidebar,
    Stats,
}

/// The payload of a resize drag. GPUI routes drags by payload type, so the
/// listener on the body row only ever sees these.
#[derive(Clone, Copy)]
pub(crate) struct PanelResize(PanelSide);

/// What follows the pointer during a drag. A resize shows nothing of its own;
/// the panel's edge moving is the feedback.
struct NoGhost;

impl Render for NoGhost {
    fn render(&mut self, _: &mut Window, _: &mut Context<Self>) -> impl IntoElement {
        div().size_0()
    }
}

/// The grab column between a panel and the timer. Dragging it resizes the
/// panel, and double-clicking it restores the default width.
///
/// It sits in the row rather than over either neighbour, so it never covers
/// the solve list's scrollbar or the timer's controls.
pub(crate) fn render_grab_column(
    side: PanelSide,
    cx: &mut Context<CubeApp>,
) -> impl IntoElement + use<> {
    let id = match side {
        PanelSide::Sidebar => "sidebar-grab",
        PanelSide::Stats => "stats-grab",
    };
    let hover = cx.theme().muted;
    div()
        .id(id)
        .flex_none()
        .w(px(GRAB_WIDTH as f32))
        .h_full()
        .cursor_col_resize()
        .occlude()
        .hover(move |grab| grab.bg(hover))
        .on_drag(PanelResize(side), |_, _, _, cx| cx.new(|_| NoGhost))
        .on_double_click(cx.listener(move |app, _, window, cx| {
            app.reset_panel_width(side, window, cx);
        }))
}

/// The width of the window's content, in whole pixels.
fn window_width(window: &Window) -> u32 {
    f32::from(window.viewport_size().width) as u32
}

impl CubeApp {
    /// The sidebar and statistics widths to draw, fitted to the window.
    pub(crate) fn panel_widths(&self, window: &Window) -> (Pixels, Pixels) {
        let (sidebar, stats) = fit_panels(
            window_width(window),
            self.settings.sidebar_width,
            self.settings.stats_panel_width,
        );
        (px(sidebar as f32), px(stats as f32))
    }

    /// Follows a resize drag. The sidebar hugs the body's left edge and the
    /// statistics panel its right, so the pointer's distance from that edge is
    /// the width the drag asks for.
    pub(crate) fn on_panel_drag(
        &mut self,
        event: &DragMoveEvent<PanelResize>,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let PanelResize(side) = *event.drag(cx);
        let body = event.bounds;
        let pointer = f32::from(event.event.position.x);
        // A pointer past the edge gives a negative distance, which saturates to zero.
        let requested = match side {
            PanelSide::Sidebar => pointer - f32::from(body.left()),
            PanelSide::Stats => f32::from(body.right()) - pointer,
        }
        .round() as u32;

        // The other panel keeps the width it is drawn at, not the one stored
        // for a window that may be wider than the current one.
        let available = window_width(window);
        let (sidebar, stats) = fit_panels(
            available,
            self.settings.sidebar_width,
            self.settings.stats_panel_width,
        );
        match side {
            PanelSide::Sidebar => {
                self.settings.sidebar_width = clamp_panel_width(requested, available, stats);
            }
            PanelSide::Stats => {
                self.settings.stats_panel_width = clamp_panel_width(requested, available, sidebar);
            }
        }
        self.panel_resizing = true;
        cx.notify();
    }

    /// Ends a resize drag and saves the new widths. Runs on every release, so
    /// a drag that ends outside the body is saved too.
    pub(crate) fn on_panel_release(
        &mut self,
        _: &MouseUpEvent,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if std::mem::take(&mut self.panel_resizing) {
            self.save_settings(window, cx);
        }
    }

    /// Restores a panel to its default width and saves it.
    pub(crate) fn reset_panel_width(
        &mut self,
        side: PanelSide,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        match side {
            PanelSide::Sidebar => self.settings.sidebar_width = DEFAULT_SIDEBAR_WIDTH,
            PanelSide::Stats => self.settings.stats_panel_width = DEFAULT_STATS_WIDTH,
        }
        self.panel_resizing = false;
        self.save_settings(window, cx);
        cx.notify();
    }
}
