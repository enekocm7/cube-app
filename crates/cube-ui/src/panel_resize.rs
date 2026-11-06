//! Dragging the inner edges of the side panels to resize them.
//!
//! GPUI supplies the drag; the limits come from `cube_core::panels`. A drag
//! updates the settings as it moves, and they are written to disk when the
//! mouse is released, not on every pixel.

use cube_core::format::format_ms;
use cube_core::panels::{
    DEFAULT_SIDEBAR_WIDTH, DEFAULT_STATS_WIDTH, PanelContent, clamp_panel_width, fit_panels,
    panel_minimums,
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

/// Width of the grab strip, centred on the panel's border: half on each side.
const GRAB_WIDTH: Pixels = px(10.);

/// The strip that resizes a panel, centred on its border: 5 px on the panel's
/// side and 5 px on the timer's side. Dragging it resizes the panel, and
/// double-clicking it restores the default width.
///
/// It is drawn over the row, last, and occludes, so the 5 px that land on the
/// solve list only resize: they do not click solve rows or scroll the list.
/// `panel_width` is the width the panel is drawn at.
pub(crate) fn render_grab(
    side: PanelSide,
    panel_width: Pixels,
    cx: &mut Context<CubeApp>,
) -> impl IntoElement + use<> {
    let (id, hover) = match side {
        PanelSide::Sidebar => ("sidebar-grab", cx.theme().muted),
        PanelSide::Stats => ("stats-grab", cx.theme().muted),
    };
    let half = GRAB_WIDTH / 2.;
    let grab = div()
        .id(id)
        .absolute()
        .top_0()
        .bottom_0()
        .w(GRAB_WIDTH)
        .cursor_col_resize()
        .occlude()
        .hover(move |grab| grab.bg(hover))
        .on_drag(PanelResize(side), |_, _, _, cx| cx.new(|_| NoGhost))
        .on_double_click(cx.listener(move |app, _, window, cx| {
            app.reset_panel_width(side, window, cx);
        }));
    match side {
        // The sidebar's border is its right edge, `panel_width` from the left.
        PanelSide::Sidebar => grab.left(panel_width - half),
        // The stats panel's border is its left edge, `panel_width` from the right.
        PanelSide::Stats => grab.right(panel_width - half),
    }
}

/// How wide `text` is when drawn in `family` at `size`.
fn text_width(window: &Window, text: &str, family: &SharedString, size: Pixels) -> f32 {
    if text.is_empty() {
        return 0.;
    }
    let run = TextRun {
        len: text.len(),
        font: font(family.clone()),
        color: black(),
        background_color: None,
        underline: None,
        strikethrough: None,
    };
    f32::from(
        window
            .text_system()
            .shape_line(text.to_owned().into(), size, &[run], None)
            .width,
    )
}

/// The longest of `texts` by character count. Every value here is set in a
/// monospace font, so the longest string is the widest.
fn longest(texts: impl IntoIterator<Item = String>) -> String {
    texts
        .into_iter()
        .max_by_key(|text| text.chars().count())
        .unwrap_or_default()
}

/// The width of the window's content, in whole pixels.
fn window_width(window: &Window) -> u32 {
    f32::from(window.viewport_size().width) as u32
}

impl CubeApp {
    /// How wide each panel's content is, measured with the real font at the
    /// current text size, so a panel never needs to wrap or cut a value.
    pub(crate) fn panel_content(&self, window: &Window, cx: &App) -> PanelContent {
        let theme = cx.theme();
        let (text_font, mono) = (&theme.font_family, &theme.mono_font_family);
        let rem = window.rem_size();
        let (small, extra_small) = (rem * 0.875, rem * 0.75);
        let rems = |count: f32| f32::from(rem) * count;

        // One statistics row: label, then the current and best values.
        let stats = &self.stats;
        let optional_ms = |ms: Option<u64>| ms.map_or_else(|| "–".to_owned(), format_ms);
        let widest = longest(
            [
                self.session()
                    .solves()
                    .last()
                    .map_or_else(|| "–".to_owned(), ToString::to_string),
                optional_ms(stats.best.map(|(ms, _)| ms)),
            ]
            .into_iter()
            .chain(stats.windows.iter().flat_map(|window| {
                [
                    crate::app::format_average(window.current),
                    crate::app::format_average(window.best.map(|(average, _)| average)),
                ]
            })),
        );
        let strings = self.strings();
        let column = text_width(window, &widest, mono, small)
            .max(text_width(
                window,
                strings.stats_current,
                text_font,
                extra_small,
            ))
            .max(text_width(
                window,
                strings.stats_best,
                text_font,
                extra_small,
            ));
        let stats_row = rems(1.5 + crate::stats_panel::LABEL_WIDTH.0 + 2. * 0.5) + 2. * column;

        // One solve row: number, time and, if shown, the average.
        let solves = self.session().solves();
        let show_average = self.settings.solve_list_ao5 && !solves.is_empty();
        let time = longest(solves.iter().map(ToString::to_string));
        let number = crate::sidebar::number_width(solves.len(), self.language()).0;
        let mut sidebar_row = rems(0.75 + number + 0.5) + text_width(window, &time, mono, small);
        if show_average {
            // The right padding clears the scrollbar instead of matching the left.
            sidebar_row += rems(0.75 + crate::sidebar::AVERAGE_WIDTH.0 + 0.5) + 22.;
        } else {
            sidebar_row += rems(0.75);
        }

        PanelContent {
            sidebar: sidebar_row.ceil() as u32,
            stats: stats_row.ceil() as u32,
        }
    }

    /// The sidebar and statistics widths to draw, fitted to the window.
    pub(crate) fn panel_widths(&self, window: &Window, cx: &App) -> (Pixels, Pixels) {
        let (sidebar, stats) = fit_panels(
            window_width(window),
            self.settings.sidebar_width,
            self.settings.stats_panel_width,
            self.settings.text_scale(),
            self.panel_content(window, cx),
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
        let scale = self.settings.text_scale();
        let content = self.panel_content(window, cx);
        let (sidebar_min, stats_min) = panel_minimums(available, content);
        let (sidebar, stats) = fit_panels(
            available,
            self.settings.sidebar_width,
            self.settings.stats_panel_width,
            scale,
            content,
        );
        match side {
            PanelSide::Sidebar => {
                self.settings.sidebar_width =
                    clamp_panel_width(requested, available, stats, scale, sidebar_min);
            }
            PanelSide::Stats => {
                self.settings.stats_panel_width =
                    clamp_panel_width(requested, available, sidebar, scale, stats_min);
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
