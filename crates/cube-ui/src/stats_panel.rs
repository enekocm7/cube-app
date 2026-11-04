//! The right panel: current and best statistics of the session.

use cube_core::format::format_ms;
use cube_core::stats::Average;
use cube_locale::Strings;
use gpui_kit::component::{ActiveTheme as _, h_flex, v_flex};
use gpui_kit::prelude::FluentBuilder as _;
use gpui_kit::*;

use crate::app::{CubeApp, average_label, format_average};

/// What clicking a value of the panel opens.
#[derive(Clone, Copy)]
enum Target {
    Solve(usize),
    Average { end: usize, size: usize },
}

impl CubeApp {
    pub(crate) fn render_stats_panel(
        &self,
        width: Pixels,
        cx: &mut Context<Self>,
    ) -> impl IntoElement + use<> {
        let strings = self.strings();
        let stats = &self.stats;
        let theme = cx.theme();
        let (border, muted) = (theme.border, theme.muted_foreground);
        let (hover, selected_bg) = (theme.muted, theme.accent);
        let mono = theme.mono_font_family.clone();
        let optional_ms = |ms: Option<u64>| ms.map_or_else(|| "–".to_owned(), format_ms);

        let cell = |text: String| {
            div()
                .flex_1()
                .font_family(mono.clone())
                .text_right()
                .child(text)
        };
        let row = |label: String, current: AnyElement, best: AnyElement| {
            h_flex()
                .h(px(28.))
                .px_3()
                .gap_2()
                .text_sm()
                .child(div().w(px(52.)).text_color(muted).child(label))
                .child(current)
                .child(best)
        };
        let value = |id: ElementId, text: String, target: Option<Target>| {
            cell(text)
                .id(id)
                .when_some(target, |cell, target| {
                    cell.cursor_pointer()
                        .rounded_sm()
                        .hover(move |cell| cell.bg(hover))
                        .on_click(cx.listener(move |app, _, window, cx| match target {
                            Target::Solve(index) => app.open_solve_details(index, window, cx),
                            Target::Average { end, size } => {
                                app.open_average(end, size, window, cx);
                            }
                        }))
                })
                .into_any_element()
        };

        let latest = self
            .session()
            .solves()
            .last()
            .map_or_else(|| "–".to_owned(), ToString::to_string);
        let last_solve = stats.count.checked_sub(1);
        let windows: Vec<_> = stats
            .windows
            .iter()
            .enumerate()
            .map(|(index, window)| {
                let size = window.size;
                row(
                    average_label(size),
                    value(
                        ("stats-current", size).into(),
                        format_average(window.current),
                        last_solve
                            .filter(|_| window.current.is_some())
                            .map(|end| Target::Average { end, size }),
                    ),
                    value(
                        ("stats-best", size).into(),
                        format_average(window.best.map(|(average, _): (Average, usize)| average)),
                        window.best.map(|(_, end)| Target::Average { end, size }),
                    ),
                )
                .when(self.selected_average == Some(index), |row| {
                    row.bg(selected_bg)
                })
            })
            .collect();

        v_flex()
            .w(width)
            .flex_shrink_0()
            .h_full()
            .py_2()
            .border_l_1()
            .border_color(border)
            .child(panel_header(strings, muted))
            .child(row(
                strings.stat_single.to_owned(),
                value(
                    "stats-latest-single".into(),
                    latest,
                    last_solve.map(Target::Solve),
                ),
                value(
                    "stats-best-single".into(),
                    optional_ms(stats.best.map(|(ms, _)| ms)),
                    stats.best.map(|(_, index)| Target::Solve(index)),
                ),
            ))
            .children(windows)
            .child(div().my_2().mx_3().h(px(1.)).bg(border))
            .children(self.render_summary(cx))
    }

    /// The whole-session figures under the averages.
    fn render_summary(&self, cx: &App) -> [impl IntoElement + use<>; 5] {
        let strings = self.strings();
        let stats = &self.stats;
        let theme = cx.theme();
        let muted = theme.muted_foreground;
        let mono = theme.mono_font_family.clone();
        let optional_ms = |ms: Option<u64>| ms.map_or_else(|| "–".to_owned(), format_ms);
        let summary = |label: &'static str, value: String| {
            h_flex()
                .h(px(26.))
                .px_3()
                .justify_between()
                .text_sm()
                .child(div().text_color(muted).child(label))
                .child(div().font_family(mono.clone()).child(value))
        };
        [
            summary(strings.stat_mean, optional_ms(stats.mean)),
            summary(strings.stat_std_dev, optional_ms(stats.std_dev)),
            summary(
                strings.stat_worst,
                optional_ms(stats.worst.map(|(ms, _)| ms)),
            ),
            summary(strings.stat_solves, stats.count.to_string()),
            summary(strings.stat_dnfs, stats.dnf_count.to_string()),
        ]
    }
}

/// The names of the two value columns.
fn panel_header(strings: &'static Strings, muted: Hsla) -> impl IntoElement {
    h_flex()
        .h(px(28.))
        .px_3()
        .gap_2()
        .text_xs()
        .text_color(muted)
        .child(div().w(px(52.)))
        .child(div().flex_1().text_right().child(strings.stats_current))
        .child(div().flex_1().text_right().child(strings.stats_best))
}
