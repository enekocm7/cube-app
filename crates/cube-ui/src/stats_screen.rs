//! The stats screen: charts of the current session.

use cube_core::Solve;
use cube_core::format::format_ms;
use cube_core::stats::{self, Average};
use gpui_kit::component::chart::{BarChart, LineChart};
use gpui_kit::component::scroll::ScrollableElement as _;
use gpui_kit::component::{ActiveTheme as _, h_flex, v_flex};
use gpui_kit::prelude::FluentBuilder as _;
use gpui_kit::*;

use crate::app::CubeApp;
use crate::palette::Palette;

const CHART_HEIGHT: Pixels = px(240.);
/// Histogram buckets. Few enough to label, enough to show a shape.
const BUCKETS: usize = 12;
/// Solves needed before charts say anything.
const MIN_SOLVES: usize = 2;

/// One charted value: `x` is the 1-based solve number.
#[derive(Clone)]
struct Point {
    x: String,
    seconds: f64,
}

#[derive(Clone)]
struct Bucket {
    label: String,
    count: f64,
}

/// The series behind the charts, built once per change to the session.
pub struct ChartData {
    times: Vec<Point>,
    averages: Vec<Point>,
    distribution: Vec<Bucket>,
}

impl ChartData {
    pub fn new(solves: &[Solve]) -> Self {
        let times: Vec<Point> = solves
            .iter()
            .enumerate()
            .filter_map(|(index, solve)| Some(point(index, solve.effective_ms()?)))
            .collect();
        let averages = stats::rolling(solves, 5)
            .into_iter()
            .filter_map(|(index, average)| match average {
                Average::Time(ms) => Some(point(index, ms)),
                Average::Dnf => None,
            })
            .collect();
        let distribution = stats::histogram(solves, BUCKETS)
            .into_iter()
            .map(|(start_ms, count)| Bucket {
                label: format_ms(start_ms),
                count: f64::from(count),
            })
            .collect();
        Self {
            times,
            averages,
            distribution,
        }
    }
}

fn point(index: usize, ms: u64) -> Point {
    Point {
        x: (index + 1).to_string(),
        seconds: ms as f64 / 1000.0,
    }
}

impl CubeApp {
    pub(crate) fn render_stats_screen(
        &mut self,
        cx: &mut Context<Self>,
    ) -> impl IntoElement + use<> {
        let strings = self.strings();
        let data = self.chart_data();
        let colors = Palette::of(cx);
        let muted = cx.theme().muted_foreground;

        if data.times.len() < MIN_SOLVES {
            return v_flex()
                .flex_1()
                .items_center()
                .justify_center()
                .text_color(muted)
                .child(strings.chart_empty)
                .into_any_element();
        }

        let mean = self.stats.mean.map(|ms| ms as f64 / 1000.0);
        let mut times = line_chart(data.times.clone(), colors.blue);
        if let Some(mean) = mean {
            times = times.reference_line(mean);
        }
        let averages = line_chart(data.averages.clone(), colors.green);
        let distribution = BarChart::new(data.distribution.clone())
            .band(|bucket: &Bucket| bucket.label.clone())
            .value(|bucket: &Bucket| bucket.count)
            .fill(move |_, _, _, _| colors.orange)
            .tick_margin(2);

        v_flex()
            .id("stats-screen")
            .flex_1()
            .min_h_0()
            .p_6()
            .gap_6()
            .overflow_y_scrollbar()
            .child(section(strings.chart_times, times, cx))
            .when(!data.averages.is_empty(), |column| {
                column.child(section(strings.chart_averages, averages, cx))
            })
            .child(section(strings.chart_distribution, distribution, cx))
            .into_any_element()
    }
}

fn line_chart(points: Vec<Point>, color: Hsla) -> LineChart<Point, String, f64> {
    let tick_margin = (points.len() / 8).max(1);
    LineChart::new(points)
        .x(|point: &Point| point.x.clone())
        .y(|point: &Point| point.seconds)
        .stroke(color)
        .linear()
        .tick_margin(tick_margin)
        .y_axis(true)
        .y_tick_format(|seconds| format!("{seconds:.1}"))
}

fn section(title: &'static str, chart: impl IntoElement, cx: &App) -> impl IntoElement {
    let muted = cx.theme().muted_foreground;
    v_flex()
        .gap_2()
        .child(h_flex().text_sm().text_color(muted).child(title))
        .child(div().h(CHART_HEIGHT).child(chart))
}
