//! The solve times chart: every time with its rolling averages drawn over it,
//! personal bests marked on the line and DNFs along the top edge.
//!
//! gpui-kit's `LineChart` draws one unbroken series, so this is its own plot.

use std::cell::Cell;
use std::ops::Range;
use std::rc::Rc;

use cube_core::charts::{ChartPoint, OVERLAY_SIZES};
use cube_core::format::format_ms;
use cube_locale::{Language, Strings};
use gpui_kit::component::ActiveTheme as _;
use gpui_kit::component::plot::label::Text;
use gpui_kit::component::plot::shape::Line;
use gpui_kit::component::plot::tooltip::{CrossLine, Dot, Tooltip};
use gpui_kit::component::plot::{
    Curve, Grid, PathCaches, Plot, PlotElement, PlotLabel, TooltipState,
};
use gpui_kit::*;

use crate::app::average_label;
use crate::palette::Palette;

/// Room left of the plot for the time labels.
const LEFT_GUTTER: f32 = 56.;
/// Room under the plot for the solve numbers.
const BOTTOM_GUTTER: f32 = 20.;
/// Room kept right of the plot, so the last point is not cut in half.
const RIGHT_MARGIN: f32 = 14.;
/// The strip along the top that holds the DNF marks.
const DNF_LANE: f32 = 14.;
const Y_TICKS: u32 = 5;
const X_TICKS: usize = 8;
/// How much the time axis extends past the extreme values, as a share of them.
const Y_PADDING: f64 = 0.05;
const TIME_STROKE: f32 = 1.5;
const AVERAGE_STROKE: f32 = 2.;
const PERSONAL_BEST_SIZE: f32 = 9.;
const DNF_SIZE: f32 = 6.;
const HOVER_DOT_SIZE: f32 = 8.;
const HOVER_HALO_SIZE: f32 = 14.;
/// The most points that still get a dot each, once they have room apart.
const MAX_DOTTED_POINTS: usize = 80;
const POINT_DOT_SIZE: f32 = 4.;

/// Where a value of the chart lands inside its element.
struct Layout {
    count: usize,
    width: f32,
    bottom: f32,
    low: f64,
    high: f64,
}

impl Layout {
    /// Returns `None` when nothing has a time to scale to.
    fn new(points: &[ChartPoint], size: Size<Pixels>) -> Option<Self> {
        let values = points.iter().flat_map(|point| {
            point
                .ms
                .into_iter()
                .chain(point.averages.iter().copied().flatten())
        });
        let (low, high) = values.fold(None, |extent: Option<(u64, u64)>, ms| {
            Some(extent.map_or((ms, ms), |(low, high)| (low.min(ms), high.max(ms))))
        })?;
        // A flat series still needs an axis to sit in the middle of.
        let padding = ((high - low) as f64 * Y_PADDING).max(500.);
        Some(Self {
            count: points.len(),
            width: f32::from(size.width) - LEFT_GUTTER - RIGHT_MARGIN,
            bottom: f32::from(size.height) - BOTTOM_GUTTER,
            low: (low as f64 - padding).max(0.),
            high: high as f64 + padding,
        })
    }

    fn x(&self, index: usize) -> f32 {
        if self.count <= 1 {
            return LEFT_GUTTER + self.width / 2.;
        }
        LEFT_GUTTER + self.width * index as f32 / (self.count - 1) as f32
    }

    fn y(&self, ms: f64) -> f32 {
        let share = (ms - self.low) / (self.high - self.low);
        self.bottom - (self.bottom - DNF_LANE) * share as f32
    }

    /// Returns the index of the point closest to `x`.
    fn nearest(&self, x: f32) -> usize {
        if self.count <= 1 {
            return 0;
        }
        let share = ((x - LEFT_GUTTER) / self.width).clamp(0., 1.);
        (share * (self.count - 1) as f32).round() as usize
    }

    /// Returns the value of each tick of the time axis, from low to high.
    fn ticks(&self) -> impl Iterator<Item = f64> {
        let (low, step) = (self.low, (self.high - self.low) / f64::from(Y_TICKS - 1));
        (0..Y_TICKS).map(move |tick| low + step * f64::from(tick))
    }
}

/// Returns how far along the plotted area `position` is, from `0` to `1`.
pub(crate) fn plot_share(bounds: Bounds<Pixels>, position: Point<Pixels>) -> f64 {
    let width = f32::from(bounds.size.width) - LEFT_GUTTER - RIGHT_MARGIN;
    let x = f32::from(position.x - bounds.origin.x) - LEFT_GUTTER;
    f64::from((x / width).clamp(0., 1.))
}

pub(crate) struct TimesPlot {
    points: Rc<Vec<ChartPoint>>,
    /// The points showing, as indices into `points`.
    range: Range<usize>,
    /// Told where the plot lands, for whoever maps the cursor onto it.
    bounds: Option<Rc<Cell<Option<Bounds<Pixels>>>>>,
    /// The name of each session, when points of several can show.
    session_labels: Option<Rc<Vec<String>>>,
    /// The mean of the times shown, in milliseconds.
    mean: Option<u64>,
    /// The line colors of [`OVERLAY_SIZES`].
    overlays: [Hsla; OVERLAY_SIZES.len()],
    colors: Palette,
    strings: &'static Strings,
    language: Language,
}

impl TimesPlot {
    pub(crate) fn new(
        points: Rc<Vec<ChartPoint>>,
        range: Range<usize>,
        mean: Option<u64>,
        overlays: [Hsla; OVERLAY_SIZES.len()],
        colors: Palette,
        strings: &'static Strings,
        language: Language,
    ) -> Self {
        Self {
            points,
            range,
            bounds: None,
            session_labels: None,
            mean,
            overlays,
            colors,
            strings,
            language,
        }
    }

    pub(crate) fn report_bounds(mut self, bounds: &Rc<Cell<Option<Bounds<Pixels>>>>) -> Self {
        self.bounds = Some(Rc::clone(bounds));
        self
    }

    /// Names each point's session in the tooltip.
    pub(crate) fn session_labels(mut self, labels: Option<Rc<Vec<String>>>) -> Self {
        self.session_labels = labels;
        self
    }

    fn visible(&self) -> &[ChartPoint] {
        self.points.get(self.range.clone()).unwrap_or_default()
    }

    /// One stroke per series, the averages last so they stay on top.
    fn lines(&self, layout: &Layout) -> Vec<Line<(f32, f32)>> {
        let times = series(layout, self.visible(), |point| point.ms);
        let time_color = self.colors.blue.opacity(0.6);
        let mut time_line = stroke(times, time_color, TIME_STROKE);
        if layout.count <= MAX_DOTTED_POINTS {
            time_line = time_line
                .dot()
                .dot_size(px(POINT_DOT_SIZE))
                .dot_fill(self.colors.blue);
        }
        let mut lines = vec![time_line];
        for (column, &color) in self.overlays.iter().enumerate() {
            let average = series(layout, self.visible(), |point| point.averages[column]);
            lines.push(stroke(average, color, AVERAGE_STROKE));
        }
        lines
    }

    fn paint_axes(&self, layout: &Layout, bounds: Bounds<Pixels>, window: &mut Window, cx: &App) {
        let theme = cx.theme();
        let (border, muted) = (theme.border, theme.muted_foreground);
        let plot = Bounds::new(
            bounds.origin + point(px(LEFT_GUTTER), px(0.)),
            size(px(layout.width), px(layout.bottom)),
        );
        Grid::new()
            .y(layout.ticks().map(|ms| px(layout.y(ms))))
            .stroke(border)
            .dash_array(&[px(4.), px(2.)])
            .paint(&plot, window);
        // The mean is of every solve, so a zoomed axis can leave it out.
        let mean = self.mean.map(|mean| mean as f64);
        if let Some(mean) = mean.filter(|mean| (layout.low..=layout.high).contains(mean)) {
            Grid::new()
                .y([px(layout.y(mean))])
                .stroke(muted)
                .dash_array(&[px(6.), px(4.)])
                .paint(&plot, window);
        }
    }

    fn paint_labels(
        &self,
        layout: &Layout,
        bounds: Bounds<Pixels>,
        window: &mut Window,
        cx: &mut App,
    ) {
        let muted = cx.theme().muted_foreground;
        let times = layout.ticks().map(|ms| {
            let origin = point(px(LEFT_GUTTER - 6.), px(layout.y(ms) - 6.));
            Text::new(format_ms(ms as u64), origin, muted).align(TextAlign::Right)
        });
        let step = layout.count.div_ceil(X_TICKS).max(1);
        let numbers = self
            .visible()
            .iter()
            .enumerate()
            .step_by(step)
            .map(|(index, point_at)| {
                let origin = point(px(layout.x(index)), px(layout.bottom + 6.));
                Text::new(
                    self.language.format_count(point_at.axis as u64),
                    origin,
                    muted,
                )
                .align(TextAlign::Center)
            });
        PlotLabel::new(times.chain(numbers).collect()).paint(&bounds, window, cx);
    }

    fn paint_marks(&self, layout: &Layout, bounds: Bounds<Pixels>, window: &mut Window, cx: &App) {
        let background = cx.theme().background;
        for (index, point_at) in self.visible().iter().enumerate() {
            let x = layout.x(index);
            match point_at.ms {
                Some(ms) if point_at.personal_best => {
                    let center = point(px(x), px(layout.y(ms as f64)));
                    window.paint_quad(mark(
                        bounds.origin + center,
                        PERSONAL_BEST_SIZE,
                        self.colors.green,
                        background,
                    ));
                }
                Some(_) => {}
                None => {
                    let center = point(px(x), px(DNF_LANE / 2.));
                    let red = self.colors.red;
                    window.paint_quad(mark(bounds.origin + center, DNF_SIZE, red, red));
                }
            }
        }
    }
}

/// Places the points of `value` that have one. A point without a value leaves
/// no vertex, so the line runs straight across it.
fn series(
    layout: &Layout,
    points: &[ChartPoint],
    value: impl Fn(&ChartPoint) -> Option<u64>,
) -> Vec<(f32, f32)> {
    points
        .iter()
        .enumerate()
        .filter_map(|(index, point_at)| {
            let ms = value(point_at)?;
            Some((layout.x(index), layout.y(ms as f64)))
        })
        .collect()
}

fn stroke(vertices: Vec<(f32, f32)>, color: Hsla, width: f32) -> Line<(f32, f32)> {
    Line::new()
        .data(vertices)
        .x(|vertex: &(f32, f32)| Some(vertex.0))
        .y(|vertex: &(f32, f32)| Some(vertex.1))
        .stroke(color)
        .stroke_width(px(width))
        .curve(Curve::Linear)
}

/// A filled circle around `center`.
fn mark(center: Point<Pixels>, diameter: f32, fill: Hsla, ring: Hsla) -> PaintQuad {
    let radius = px(diameter / 2.);
    quad(
        Bounds::new(
            center - point(radius, radius),
            size(px(diameter), px(diameter)),
        ),
        radius,
        fill,
        px(1.),
        ring,
        BorderStyle::default(),
    )
}

impl IntoElement for TimesPlot {
    type Element = PlotElement<Self>;

    fn into_element(self) -> Self::Element {
        PlotElement::new(self)
    }
}

impl Plot for TimesPlot {
    fn paint(&mut self, bounds: Bounds<Pixels>, window: &mut Window, cx: &mut App) {
        if let Some(report) = &self.bounds {
            report.set(Some(bounds));
        }
        let Some(layout) = Layout::new(self.visible(), bounds.size) else {
            return;
        };
        self.paint_axes(&layout, bounds, window, cx);
        self.paint_labels(&layout, bounds, window, cx);
        let lines = self.lines(&layout);
        let caches = PathCaches::for_paint("times", window, cx);
        caches.update(cx, |caches, _| {
            for (slot, line) in lines.iter().enumerate() {
                line.paint_cached(&bounds, caches.slot(slot), window);
            }
        });
        self.paint_marks(&layout, bounds, window, cx);
    }

    fn id(&self) -> Option<ElementId> {
        Some("times-plot".into())
    }

    fn tooltip_state(
        &self,
        position: Point<Pixels>,
        bounds: Bounds<Pixels>,
        _cx: &App,
    ) -> Option<TooltipState> {
        let layout = Layout::new(self.visible(), bounds.size)?;
        if f32::from(position.y) > layout.bottom || f32::from(position.x) < LEFT_GUTTER {
            return None;
        }
        let index = layout.nearest(f32::from(position.x));
        let x = layout.x(index);
        let y = self
            .visible()
            .get(index)?
            .ms
            .map_or(DNF_LANE / 2., |ms| layout.y(ms as f64));
        Some(TooltipState::new(
            index,
            point(px(x), position.y),
            vec![point(px(x), px(y))],
        ))
    }

    fn tooltip(
        &self,
        state: &TooltipState,
        cursor: Point<Pixels>,
        bounds: Bounds<Pixels>,
        _window: &mut Window,
        cx: &mut App,
    ) -> Option<AnyElement> {
        let hovered = self.visible().get(state.index)?;
        let layout = Layout::new(self.visible(), bounds.size)?;
        let strings = self.strings;
        let time_color = if hovered.ms.is_some() {
            self.colors.blue
        } else {
            self.colors.red
        };
        let number = self.language.format_count(hovered.number as u64);
        let mut tooltip = Tooltip::new(cursor, bounds.size)
            .gap(px(8.))
            .cross_line(CrossLine::new(state.cross_line).height(layout.bottom))
            .dots(Some(
                Dot::new(*state.dots.first()?)
                    .size(HOVER_DOT_SIZE)
                    .halo(HOVER_HALO_SIZE)
                    .stroke(cx.theme().background)
                    .fill(time_color),
            ))
            .title(strings.chart_solve.replace("{number}", &number))
            .row(
                time_color,
                strings.table_time,
                hovered.ms.map_or_else(|| "DNF".to_owned(), format_ms),
            );
        for (column, size) in OVERLAY_SIZES.into_iter().enumerate() {
            if let Some(ms) = hovered.averages[column] {
                tooltip = tooltip.row(self.overlays[column], average_label(size), format_ms(ms));
            }
        }
        let session = self.session_labels.as_ref();
        if let Some(label) = session.and_then(|labels| labels.get(hovered.session)) {
            tooltip = tooltip.plain_row(strings.chart_column_session, label.clone());
        }
        if hovered.personal_best {
            tooltip = tooltip.plain_row(strings.chart_personal_best, "");
        }
        Some(tooltip.into_any_element())
    }
}
