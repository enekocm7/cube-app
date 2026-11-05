//! The stats screen: a table of every solve's rolling averages.

use std::ops::Range;

use cube_core::stats::{AVERAGE_SIZES, Average, DEFAULT_AVERAGE};
use gpui_kit::component::scroll::Scrollbar;
use gpui_kit::component::{ActiveTheme as _, StyledExt, h_flex, v_flex};
use gpui_kit::prelude::FluentBuilder as _;
use gpui_kit::*;

use crate::app::{CubeApp, average_label, format_average};
use crate::palette::Palette;
use crate::scroll_fade::fading_scroll;
use crate::sidebar::solve_time_color;

/// Height of a table row, and of the tile inside it.
const ROW_HEIGHT: Rems = Rems(2.25);
const TILE_HEIGHT: Rems = Rems(2.);
/// Width of the table's solve number column.
const NUMBER_WIDTH: Rems = Rems(2.75);
/// Room kept on a table row's right for the overlay scrollbar.
const SCROLLBAR_GUTTER: Pixels = px(16.);
/// How strongly the fastest and slowest tiles of a column are tinted.
const MAX_TINT: f32 = 0.42;

impl CubeApp {
    pub(crate) fn render_table(&self, cx: &mut Context<Self>) -> AnyElement {
        let strings = self.strings();
        let count = self.session().len();
        let theme = cx.theme();
        let (background, muted) = (theme.background, theme.muted_foreground);
        if count == 0 {
            return empty_message(strings.table_empty, muted);
        }

        v_flex()
            .flex_1()
            .min_h_0()
            .px_6()
            .py_4()
            .gap_2()
            .child(self.render_table_header(cx))
            .child(
                div()
                    .relative()
                    .flex_1()
                    .min_h_0()
                    .child(
                        fading_scroll(
                            uniform_list(
                                "stats-table",
                                count,
                                cx.processor(Self::render_table_rows),
                            )
                            .size_full()
                            .track_scroll(&self.stats_table_scroll),
                            &self.stats_table_scroll,
                            background,
                        )
                        .size_full(),
                    )
                    .child(
                        Scrollbar::vertical(&self.stats_table_scroll).mode(self.scrollbar_mode()),
                    ),
            )
            .into_any_element()
    }

    /// Every column's name over its best value, which opens that result.
    fn render_table_header(&self, cx: &Context<Self>) -> impl IntoElement + use<> {
        let strings = self.strings();
        let selected_column = self.selected_average.unwrap_or(DEFAULT_AVERAGE);
        let single = self.stats.best;
        let single_label = header_cell(
            strings.table_time.to_owned(),
            single.map(|(ms, _)| Average::Time(ms)),
            false,
            cx,
        )
        .id("stats-best-time")
        .mr_2()
        .when_some(single, |cell, (_, index)| {
            cell.cursor_pointer()
                .on_click(cx.listener(move |app, _, window, cx| {
                    app.open_solve_details(index, window, cx);
                }))
        });
        let averages: Vec<_> = AVERAGE_SIZES
            .into_iter()
            .enumerate()
            .map(|(column, size)| {
                let best = self.stats.windows.get(column).and_then(|stats| stats.best);
                header_cell(
                    average_label(size),
                    best.map(|(average, _)| average),
                    column == selected_column,
                    cx,
                )
                .id(("stats-best-average", column))
                .when_some(best, |cell, (_, end)| {
                    cell.cursor_pointer()
                        .on_click(cx.listener(move |app, _, window, cx| {
                            app.selected_average = Some(column);
                            app.open_average(end, size, window, cx);
                            cx.notify();
                        }))
                })
            })
            .collect();
        table_row()
            .h_auto()
            .flex_none()
            .child(div().w(NUMBER_WIDTH).flex_none())
            .child(single_label)
            .children(averages)
    }

    /// Builds the visible rows, newest first like the solve list.
    fn render_table_rows(
        &mut self,
        range: Range<usize>,
        _: &mut Window,
        cx: &mut Context<Self>,
    ) -> Vec<AnyElement> {
        let table = self.stats_table();
        let colors = Palette::of(cx);
        let theme = cx.theme();
        let (muted, band) = (theme.muted_foreground, theme.accent);
        let (ring, hover, on_best) = (theme.primary, theme.border, theme.background);
        let mono = theme.mono_font_family.clone();
        let solves = self.session().solves();
        let target = self.target_solve();
        let selected_column = self.selected_average.unwrap_or(DEFAULT_AVERAGE);

        range
            .filter_map(|row| {
                let index = solves.len().checked_sub(row + 1)?;
                let solve = solves.get(index)?;
                let selected_row = target == Some(index);
                let tiles: Vec<_> = table
                    .row(index)?
                    .iter()
                    .enumerate()
                    .map(|(column, &average)| {
                        let size = AVERAGE_SIZES[column];
                        let selected = selected_row && column == selected_column;
                        let best = average.is_some_and(|average| table.is_best(column, average));
                        let fill = if best {
                            Some(colors.green)
                        } else {
                            average
                                .and_then(|average| table.heat(column, average))
                                .map(|heat| tint(heat, colors))
                        };
                        tile(
                            ("stats-average", index * AVERAGE_SIZES.len() + column),
                            selected,
                            ring,
                        )
                        .when_some(fill, gpui_kit::Styled::bg)
                        .when(best, |tile| tile.text_color(on_best))
                        .when(average == Some(Average::Dnf), |tile| {
                            tile.text_color(colors.red)
                        })
                        .when_some(average, |tile, average| {
                            tile.cursor_pointer()
                                .when(!selected, |tile| {
                                    tile.hover(move |tile| tile.border_color(hover))
                                })
                                .on_click(cx.listener(move |app, _, window, cx| {
                                    app.selected_solve = Some(index);
                                    app.selected_average = Some(column);
                                    app.open_average(index, size, window, cx);
                                    cx.notify();
                                }))
                                .child(format_average(Some(average)))
                        })
                    })
                    .collect();
                Some(
                    table_row()
                        .rounded(px(8.))
                        .font_family(mono.clone())
                        .when(selected_row, |row| row.bg(band))
                        .child(
                            div()
                                .w(NUMBER_WIDTH)
                                .flex_none()
                                .truncate()
                                .text_xs()
                                .text_color(muted)
                                .text_right()
                                .pr_2()
                                .child(self.language().format_count((index + 1) as u64)),
                        )
                        .child(
                            tile(("stats-time", index), false, ring)
                                .mr_2()
                                .cursor_pointer()
                                .hover(move |tile| tile.border_color(hover))
                                .when_some(
                                    solve_time_color(solve, colors),
                                    gpui_kit::Styled::text_color,
                                )
                                .on_click(cx.listener(move |app, _, window, cx| {
                                    app.selected_solve = Some(index);
                                    app.open_solve_details(index, window, cx);
                                    cx.notify();
                                }))
                                .child(solve.to_string()),
                        )
                        .children(tiles)
                        .into_any_element(),
                )
            })
            .collect()
    }
}

/// The shared layout of a table row, so the header lines up with the rows.
fn table_row() -> Div {
    h_flex()
        .w_full()
        .h(ROW_HEIGHT)
        .pr(SCROLLBAR_GUTTER)
        .gap_1()
        .text_sm()
}

/// One value of the table. The border is always there, so selecting or
/// hovering a tile recolors it without moving the text.
fn tile(id: impl Into<ElementId>, selected: bool, ring: Hsla) -> Stateful<Div> {
    div()
        .id(id)
        .flex()
        .flex_1()
        .min_w_0()
        .h(TILE_HEIGHT)
        .items_center()
        .justify_end()
        .px_2()
        .rounded(px(6.))
        .border_2()
        .border_color(if selected { ring } else { transparent_black() })
}

/// Shades a tile by its place in the column: green for fast, orange for slow,
/// and nothing in between, so only the times that stand out catch the eye.
fn tint(heat: f32, colors: Palette) -> Hsla {
    let color = if heat < 0.0 {
        colors.green
    } else {
        colors.orange
    };
    color.opacity(heat.abs().sqrt() * MAX_TINT)
}

/// A column's name, with its best value underneath.
fn header_cell(label: String, best: Option<Average>, selected: bool, cx: &App) -> Div {
    let theme = cx.theme();
    let best_color = if matches!(best, Some(Average::Time(_))) {
        Palette::of(cx).green
    } else {
        theme.muted_foreground
    };
    v_flex()
        .flex_1()
        .min_w_0()
        .items_end()
        .px_2()
        .child(
            div()
                .text_color(if selected {
                    theme.foreground
                } else {
                    theme.muted_foreground
                })
                .when(selected, StyledExt::font_semibold)
                .child(label),
        )
        .child(
            div()
                .font_family(theme.mono_font_family.clone())
                .text_color(best_color)
                .child(format_average(best)),
        )
}

pub(crate) fn empty_message(message: &'static str, color: Hsla) -> AnyElement {
    v_flex()
        .flex_1()
        .items_center()
        .justify_center()
        .text_color(color)
        .child(message)
        .into_any_element()
}
