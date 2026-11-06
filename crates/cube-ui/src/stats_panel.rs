//! The right panel: current and best statistics of the session.

use cube_core::format::format_ms;
use cube_core::panels::stats_rows_stack;
use cube_core::stats::{Average, SessionStats};
use cube_locale::Strings;
use gpui_kit::component::{ActiveTheme as _, h_flex, v_flex};
use gpui_kit::prelude::FluentBuilder as _;
use gpui_kit::*;

use crate::app::{CubeApp, average_label, format_average};
use crate::scroll_fade::FadeScroll;

/// Height of a row, which follows the text size.
const ROW_HEIGHT: Rems = Rems(1.75);
/// Width of the column naming each row.
const LABEL_WIDTH: Rems = Rems(3.25);

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
        let background = theme.background;
        let (hover, selected_bg) = (theme.muted, theme.accent);
        let mono = theme.mono_font_family.clone();
        let optional_ms = |ms: Option<u64>| ms.map_or_else(|| "–".to_owned(), format_ms);

        let cell = |text: String| {
            div()
                .flex_1()
                .min_w_0()
                .truncate()
                .font_family(mono.clone())
                .text_right()
                .child(text)
        };
        let stacked = stats_rows_stack(f32::from(width) as u32, self.settings.text_scale());
        let row = |label: String, current: AnyElement, best: AnyElement| {
            stats_row(label, current, best, stacked, muted)
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

        let goals = self.render_active_goals(cx);

        v_flex()
            .w(width)
            .flex_shrink_0()
            .h_full()
            .py_2()
            .border_l_1()
            .border_color(border)
            .child(panel_header(strings, muted, stacked))
            .child(
                FadeScroll::new("stats-panel", background)
                    .flex_1()
                    .min_h_0()
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
                    .children(goals),
            )
            .child(session_summary(strings, stats, &mono, border, muted))
    }
}

/// The session-wide figures pinned below the averages.
fn session_summary(
    strings: &'static Strings,
    stats: &SessionStats,
    mono: &SharedString,
    border: Hsla,
    muted: Hsla,
) -> Div {
    let ms = |ms: Option<u64>| ms.map_or_else(|| "–".to_owned(), format_ms);
    let rows = [
        (strings.stat_solves, stats.count.to_string()),
        (strings.stat_mean, ms(stats.mean)),
        (strings.stat_std_dev, ms(stats.std_dev)),
        (strings.stat_dnfs, stats.dnf_count.to_string()),
    ];
    v_flex()
        .flex_none()
        .pt_1()
        .border_t_1()
        .border_color(border)
        .children(rows.map(|(label, text)| {
            let value = div()
                .flex_1()
                .min_w_0()
                .truncate()
                .font_family(mono.clone())
                .text_right()
                .child(text);
            summary_row(label, value, muted)
        }))
}

/// One session-wide figure pinned below the averages: its label beside its
/// value.
fn summary_row(label: &'static str, value: Div, muted: Hsla) -> Div {
    h_flex()
        .flex_none()
        .h(ROW_HEIGHT)
        .px_3()
        .gap_2()
        .text_sm()
        .child(div().flex_none().truncate().text_color(muted).child(label))
        .child(value)
}

/// One row of the panel: its label beside its values, or above them when
/// `stacked`.
fn stats_row(
    label: String,
    current: AnyElement,
    best: AnyElement,
    stacked: bool,
    muted: Hsla,
) -> Div {
    let name = div().flex_none().truncate().text_color(muted).child(label);
    if stacked {
        v_flex()
            .flex_none()
            .px_3()
            .py_1()
            .text_sm()
            .child(name)
            .child(
                h_flex()
                    .w_full()
                    .gap_2()
                    .text_xs()
                    .child(current)
                    .child(best),
            )
    } else {
        h_flex()
            .flex_none()
            .h(ROW_HEIGHT)
            .px_3()
            .gap_2()
            .text_sm()
            .child(name.w(LABEL_WIDTH))
            .child(current)
            .child(best)
    }
}

/// The names of the two value columns.
fn panel_header(strings: &'static Strings, muted: Hsla, stacked: bool) -> impl IntoElement {
    h_flex()
        .h(ROW_HEIGHT)
        .px_3()
        .gap_2()
        .text_xs()
        .text_color(muted)
        .when(!stacked, |header| {
            header.child(div().w(LABEL_WIDTH).flex_none())
        })
        .child(heading(strings.stats_current))
        .child(heading(strings.stats_best))
}

/// The name of a value column, cut short rather than folded.
fn heading(text: &'static str) -> Div {
    div().flex_1().min_w_0().truncate().text_right().child(text)
}
