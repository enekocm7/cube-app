//! The charts screen: trends of the filtered solves, a calendar of solves per
//! day, a comparison of sessions and the all-time bests.

use std::cell::Cell;
use std::ops::Range;
use std::rc::Rc;

use chrono::{Datelike as _, NaiveDate};
use cube_core::charts::{
    self, COMPARE_COLUMNS, Calendar, ChartFilter, ChartPoint, ChartRange, ChartView, ChartZoom,
    CompareRow, EventBests, MAX_HEAT, OVERLAY_SIZES, Record, Selection, TagSide,
};
use cube_core::dialogs::DialogSize;
use cube_core::format::format_ms;
use cube_core::stats::{self, AVERAGE_SIZES, SessionStats};
use cube_core::tags;
use cube_core::{Action, History, WcaEvent};
use cube_locale::Strings;
use gpui_kit::assets::IconName;
use gpui_kit::component::button::{Button, ButtonVariants as _};
use gpui_kit::component::chart::BarChart;
use gpui_kit::component::input::{Input, InputState};
use gpui_kit::component::menu::{DropdownMenu as _, PopupMenuItem};
use gpui_kit::component::{
    ActiveTheme as _, Icon, Selectable as _, Sizable as _, StyledExt, WindowExt as _, h_flex,
    v_flex,
};
use gpui_kit::prelude::FluentBuilder as _;
use gpui_kit::*;

use crate::app::{CubeApp, Screen, average_label};
use crate::dialog_size::{dialog_title, dialog_width};
use crate::dialog_submit::{DialogSubmit as _, Submit, on_press};
use crate::palette::Palette;
use crate::scroll_fade::FadeScroll;
use crate::stats_screen::empty_message;
use crate::times_plot::{TimesPlot, plot_share};

const CHART_HEIGHT: Pixels = px(260.);
/// Histogram buckets. Few enough to label, enough to show a shape.
const BUCKETS: usize = 12;
/// Timed solves needed before the charts say anything.
const MIN_SOLVES: usize = 2;
/// How far one press of `Up` or `Down` scrolls.
const SCROLL_STEP: Pixels = px(80.);
/// Widest the session filter gets, so a long name leaves room for the others.
const SESSION_FILTER_MAX_WIDTH: Rems = Rems(13.75);
/// How much of the window's height the session menu may take before it scrolls.
const SESSION_MENU_SHARE: f32 = 0.7;
/// How much one press of the zoom keys narrows the times chart.
pub(crate) const KEY_ZOOM: f64 = 0.75;
/// How far the wheel turns to halve the stretch of the times chart.
const WHEEL_HALVING: f32 = 120.;
/// How far the wheel turns to pan by a whole stretch.
const WHEEL_PAN: f32 = 400.;
/// How much of its stretch one press of `Left` or `Right` pans the chart.
const KEY_PAN: f64 = 0.25;
/// Height of a calendar day, and the gap around it.
const DAY_HEIGHT: Pixels = px(14.);
const DAY_GAP: Pixels = px(3.);
/// Widest the calendar stretches on a large window.
const CALENDAR_MAX_WIDTH: Pixels = px(1100.);
/// How strong the first shade of a calendar day is, and what each level adds.
const HEAT_BASE: f32 = 0.2;
const HEAT_STEP: f32 = 0.2;
/// Width of a table's name column, as a multiple of a value column.
const NAME_COLUMN_GROW: f32 = 2.;

#[derive(Clone)]
struct Bucket {
    label: String,
    count: f64,
}

/// A session's bar in the comparison.
#[derive(Clone)]
struct SessionMean {
    label: String,
    seconds: f64,
}

/// The data behind the charts, built once per change to the solves or filter.
pub struct ChartData {
    points: Rc<Vec<ChartPoint>>,
    /// The mean of the solves shown, in milliseconds.
    mean: Option<u64>,
    distribution: Vec<Bucket>,
    calendar: Calendar,
    compare: Vec<CompareRow>,
    /// The two sides of the tag comparison.
    tags: [TagSide; 2],
    bests: Vec<EventBests>,
}

impl ChartData {
    pub fn new(
        sessions: &[History],
        current: usize,
        filter: &ChartFilter,
        compare_tag: Option<&str>,
    ) -> Self {
        let today = charts::today();
        let selection = Selection::across(sessions, current, filter, today);
        let visible = selection.visible();
        let distribution = stats::histogram(&visible, BUCKETS)
            .into_iter()
            .map(|bucket| {
                let mut label = format_ms(bucket.start_ms);
                // A trailing "+" says the bar also holds slower solves past
                // the charted range.
                if bucket.overflow {
                    label.push('+');
                }
                Bucket {
                    label,
                    count: f64::from(bucket.count),
                }
            })
            .collect();
        Self {
            points: Rc::new(selection.points()),
            mean: SessionStats::compute(&visible).mean,
            distribution,
            calendar: Calendar::new(&visible),
            compare: charts::compare(sessions, filter, today),
            tags: [filter.tag.as_deref(), compare_tag]
                .map(|tag| TagSide::new(sessions, current, filter, tag, today)),
            bests: charts::all_time_bests(sessions),
        }
    }
}

impl CubeApp {
    pub(crate) fn render_charts(&mut self, cx: &mut Context<Self>) -> AnyElement {
        let data = self.chart_data();
        let body = match self.chart_view {
            ChartView::Trends => self.render_trends(&data, cx),
            ChartView::Calendar => self.render_calendar(&data, cx),
            ChartView::Compare => self.render_compare(&data, cx),
            ChartView::Tags => self.render_tag_compare(&data, cx),
            ChartView::Bests => self.render_bests(&data, cx),
        };
        v_flex()
            .flex_1()
            .min_h_0()
            .child(self.render_chart_bar(cx))
            .child(body)
            .into_any_element()
    }

    /// The view tabs, with the filters on the right.
    fn render_chart_bar(&self, cx: &mut Context<Self>) -> impl IntoElement + use<> {
        let strings = self.strings();
        let tabs: Vec<_> = ChartView::ALL
            .into_iter()
            .enumerate()
            .map(|(index, view)| {
                Button::new(("chart-view", index))
                    .ghost()
                    .small()
                    .label(view_label(strings, view))
                    .tab_stop(false)
                    .selected(self.chart_view == view)
                    .on_click(cx.listener(move |app, _, window, cx| {
                        app.show_chart_view(view, cx);
                        app.focus.focus(window, cx);
                    }))
            })
            .collect();
        h_flex()
            .flex_none()
            .px_6()
            .pt_4()
            .gap_1()
            .justify_between()
            // Larger text moves the filters under the tabs instead of past the
            // window's edge.
            .flex_wrap()
            .child(h_flex().gap_1().children(tabs))
            .child(
                h_flex()
                    .gap_2()
                    .flex_wrap()
                    // The comparison lists every session whatever the scope,
                    // and the bests ignore every filter, so only the filters
                    // that change the view show.
                    .when(
                        self.chart_view != ChartView::Compare
                            && self.chart_view != ChartView::Bests,
                        |row| row.child(self.render_session_filter(cx)),
                    )
                    .when(self.chart_view != ChartView::Bests, |row| {
                        row.child(self.render_event_filter(cx))
                    })
                    .when(self.chart_view != ChartView::Bests, |row| {
                        row.child(self.render_range_filter(cx))
                    })
                    .when(self.tag_filter_shown() && self.has_tags(), |row| {
                        row.child(self.render_tag_filter(cx))
                    })
                    .when(
                        self.chart_view == ChartView::Tags && self.has_tags(),
                        |row| row.child(self.render_compare_tag_filter(cx)),
                    ),
            )
    }

    /// Picks every session, or one. Picking one opens it, as `[` and `]` do,
    /// so there is only ever one idea of which session is showing.
    fn render_session_filter(&self, cx: &mut Context<Self>) -> impl IntoElement + use<> {
        let strings = self.strings();
        let entity = cx.entity().downgrade();
        let (all, current) = (self.chart_filter.all_sessions, self.current);
        let labels = self.session_labels();
        let label = if all {
            strings.chart_all_sessions.to_owned()
        } else {
            labels[current].clone()
        };
        let button = filter_button("chart-session", label).w_full();
        div()
            .max_w(SESSION_FILTER_MAX_WIDTH)
            .child(button.dropdown_menu(move |menu, window, cx| {
                let everything = entity.clone();
                let mut menu = menu
                    .scrollable(true)
                    .max_h(window.viewport_size().height * SESSION_MENU_SHARE)
                    .item(
                        PopupMenuItem::new(strings.chart_all_sessions)
                            .checked(all)
                            .on_click(move |_, window, cx| {
                                everything
                                    .update(cx, |app, cx| {
                                        app.show_chart_sessions(true, app.current, cx);
                                        app.focus.focus(window, cx);
                                    })
                                    .ok();
                            }),
                    );
                let _ = cx;
                for (index, label) in labels.iter().enumerate() {
                    let entity = entity.clone();
                    menu = menu.item(
                        PopupMenuItem::new(label.clone())
                            .checked(!all && index == current)
                            .on_click(move |_, window, cx| {
                                entity
                                    .update(cx, |app, cx| {
                                        app.show_chart_sessions(false, index, cx);
                                        app.focus.focus(window, cx);
                                    })
                                    .ok();
                            }),
                    );
                }
                menu
            }))
    }

    fn render_event_filter(&self, cx: &mut Context<Self>) -> impl IntoElement + use<> {
        let strings = self.strings();
        let entity = cx.entity().downgrade();
        let selected = self.chart_filter.event;
        let events = charts::events(&self.sessions);
        filter_button("chart-event", event_label(strings, selected)).dropdown_menu(
            move |mut menu, _, _| {
                let options = std::iter::once(None).chain(events.iter().copied().map(Some));
                for option in options {
                    let entity = entity.clone();
                    menu = menu.item(
                        PopupMenuItem::new(event_label(strings, option))
                            .checked(option == selected)
                            .on_click(move |_, window, cx| {
                                entity
                                    .update(cx, |app, cx| {
                                        app.set_chart_event(option, cx);
                                        app.focus.focus(window, cx);
                                    })
                                    .ok();
                            }),
                    );
                }
                menu
            },
        )
    }

    fn render_range_filter(&self, cx: &mut Context<Self>) -> impl IntoElement + use<> {
        let strings = self.strings();
        let language = self.language();
        let entity = cx.entity().downgrade();
        let selected = self.chart_filter.range;
        filter_button("chart-range", range_label(strings, language, selected)).dropdown_menu(
            move |mut menu, _, _| {
                for range in ChartRange::PRESETS {
                    let entity = entity.clone();
                    menu = menu.item(
                        PopupMenuItem::new(range_label(strings, language, range))
                            .checked(range == selected)
                            .on_click(move |_, window, cx| {
                                entity
                                    .update(cx, |app, cx| {
                                        app.set_chart_range(range, cx);
                                        app.focus.focus(window, cx);
                                    })
                                    .ok();
                            }),
                    );
                }
                let entity = entity.clone();
                menu.item(
                    PopupMenuItem::new(strings.chart_range_custom)
                        .checked(matches!(selected, ChartRange::Dates { .. }))
                        .on_click(move |_, window, cx| {
                            entity
                                .update(cx, |app, cx| app.open_chart_dates(window, cx))
                                .ok();
                        }),
                )
            },
        )
    }

    /// Picks the tag the charts and the stats table are filtered by.
    pub(crate) fn render_tag_filter(&self, cx: &mut Context<Self>) -> impl IntoElement + use<> {
        let selected = self.chart_filter.tag.clone();
        self.render_tag_menu("tag-filter", selected, Self::set_tag_filter, cx)
    }

    /// Picks what the tag comparison sets against the tag filter.
    fn render_compare_tag_filter(&self, cx: &mut Context<Self>) -> impl IntoElement + use<> {
        let selected = self.compare_tag.clone();
        self.render_tag_menu("compare-tag", selected, Self::set_compare_tag, cx)
    }

    fn render_tag_menu(
        &self,
        id: &'static str,
        selected: Option<String>,
        pick: fn(&mut Self, Option<String>, &mut Context<Self>),
        cx: &mut Context<Self>,
    ) -> impl IntoElement + use<> {
        let strings = self.strings();
        let entity = cx.entity().downgrade();
        let known = self.known_tags();
        let label = tag_label(strings, selected.as_deref());
        div().max_w(SESSION_FILTER_MAX_WIDTH).child(
            filter_button(id, label)
                .w_full()
                .dropdown_menu(move |menu, window, _| {
                    let mut menu = menu
                        .scrollable(true)
                        .max_h(window.viewport_size().height * SESSION_MENU_SHARE);
                    for option in std::iter::once(None).chain(known.iter().cloned().map(Some)) {
                        let entity = entity.clone();
                        let checked = match (&option, &selected) {
                            (Some(option), Some(selected)) => tags::same(option, selected),
                            (option, selected) => option.is_none() && selected.is_none(),
                        };
                        menu = menu.item(
                            PopupMenuItem::new(tag_label(strings, option.as_deref()))
                                .checked(checked)
                                .on_click(move |_, window, cx| {
                                    let option = option.clone();
                                    entity
                                        .update(cx, |app, cx| {
                                            pick(app, option, cx);
                                            app.focus.focus(window, cx);
                                        })
                                        .ok();
                                }),
                        );
                    }
                    menu
                }),
        )
    }

    fn render_trends(&self, data: &ChartData, cx: &mut Context<Self>) -> AnyElement {
        let strings = self.strings();
        let language = self.language();
        let colors = Palette::of(cx);
        let theme = cx.theme();
        let (muted, background, foreground) =
            (theme.muted_foreground, theme.background, theme.foreground);

        let timed = data.points.iter().filter(|point| point.ms.is_some());
        if timed.count() < MIN_SOLVES {
            return empty_message(self.chart_empty_text(), muted);
        }

        let overlays = [colors.orange, colors.yellow, foreground];
        let range = self.chart_range(data);
        let zoom_text = self.chart_zoom.and_then(|_| {
            let first = data.points.get(range.start)?.axis;
            let last = data.points.get(range.end.checked_sub(1)?)?.axis;
            let text = strings
                .chart_zoom_range
                .replace("{first}", &language.format_count(first as u64));
            Some(text.replace("{last}", &language.format_count(last as u64)))
        });
        let times_plot = TimesPlot::new(
            Rc::clone(&data.points),
            range,
            data.mean,
            overlays,
            colors,
            strings,
            language,
        )
        .report_bounds(&self.chart_plot_bounds)
        .session_labels(
            self.chart_filter
                .all_sessions
                .then(|| Rc::new(self.session_labels())),
        );
        let distribution = BarChart::new(data.distribution.clone())
            .band(|bucket: &Bucket| bucket.label.clone())
            .value(|bucket: &Bucket| bucket.count)
            .fill(move |_, _, _, _| colors.orange)
            .tick_margin(2);

        let mut legend = vec![legend_item(colors.blue, strings.table_time.to_owned())];
        for (size, color) in OVERLAY_SIZES.into_iter().zip(overlays) {
            legend.push(legend_item(color, average_label(size)));
        }
        legend.push(legend_item(
            colors.green,
            strings.chart_personal_best.to_owned(),
        ));
        legend.push(legend_item(colors.red, "DNF".to_owned()));

        FadeScroll::new("chart-trends", background)
            .track(&self.chart_scroll)
            .flex_1()
            .min_h_0()
            .p_6()
            .gap_6()
            .child(
                v_flex()
                    .gap_2()
                    .child(
                        h_flex()
                            .justify_between()
                            .child(
                                h_flex()
                                    .gap_3()
                                    .child(section_title(strings.chart_times, muted))
                                    .when_some(zoom_text, |row, text| {
                                        row.child(div().text_xs().text_color(muted).child(text))
                                            .child(
                                                Button::new("chart-reset-zoom")
                                                    .ghost()
                                                    .xsmall()
                                                    .label(strings.chart_reset_zoom)
                                                    .tab_stop(false)
                                                    .on_click(cx.listener(|app, _, window, cx| {
                                                        app.reset_chart_zoom(cx);
                                                        app.focus.focus(window, cx);
                                                    })),
                                            )
                                    }),
                            )
                            .child(h_flex().gap_3().children(legend)),
                    )
                    .child(
                        div()
                            .h(CHART_HEIGHT)
                            .on_scroll(cx.listener(Self::on_chart_wheel))
                            .child(times_plot),
                    ),
            )
            .child(
                v_flex()
                    .gap_2()
                    .child(section_title(strings.chart_distribution, muted))
                    .child(div().h(CHART_HEIGHT).child(distribution)),
            )
            .into_any_element()
    }

    /// A year of days, with the selected day's solves underneath.
    fn render_calendar(&self, data: &ChartData, cx: &mut Context<Self>) -> AnyElement {
        let strings = self.strings();
        let language = self.language();
        let theme = cx.theme();
        let (muted, background) = (theme.muted_foreground, theme.background);
        if data.calendar.latest().is_none() {
            return empty_message(self.chart_empty_text(), muted);
        }

        let selected = self.chart_day(data);
        let year = selected.year();
        let summary = data.calendar.year(year);
        let totals = strings
            .chart_year_summary
            .replace(
                "{solves}",
                &language.format_count(u64::from(summary.solves)),
            )
            .replace(
                "{days}",
                &language.format_count(u64::from(summary.active_days)),
            );
        let busiest = summary.busiest.map(|(day, count)| {
            strings
                .chart_busiest_day
                .replace("{date}", &language.format_day(day))
                .replace("{count}", &language.format_count(u64::from(count)))
        });
        let day_text = strings
            .chart_day
            .replace("{date}", &language.format_day(selected))
            .replace(
                "{count}",
                &language.format_count(u64::from(data.calendar.count(selected))),
            );

        FadeScroll::new("chart-calendar", background)
            .track(&self.chart_scroll)
            .flex_1()
            .min_h_0()
            .p_6()
            .gap_4()
            .child(
                h_flex()
                    .gap_2()
                    .child(year_button("chart-year-back", -1, strings, cx))
                    .child(div().font_semibold().child(year.to_string()))
                    .child(year_button("chart-year-forward", 1, strings, cx))
                    .child(div().ml_2().text_sm().text_color(muted).child(totals)),
            )
            .child(self.render_year(data, year, selected, cx))
            .child(
                v_flex()
                    .gap_1()
                    .text_sm()
                    .child(div().child(day_text))
                    .when_some(busiest, |column, busiest| {
                        column.child(div().text_color(muted).child(busiest))
                    }),
            )
            .into_any_element()
    }

    fn render_year(
        &self,
        data: &ChartData,
        year: i32,
        selected: NaiveDate,
        cx: &mut Context<Self>,
    ) -> impl IntoElement + use<> {
        let strings = self.strings();
        let colors = Palette::of(cx);
        let theme = cx.theme();
        let (empty, ring, muted) = (theme.muted, theme.foreground, theme.muted_foreground);
        let most = data.calendar.year(year).busiest.map_or(0, |(_, most)| most);

        let weeks = charts::year_weeks(year);
        let months = weeks.iter().map(|week| {
            let first = week.iter().flatten().find(|day| day.day() == 1);
            div()
                .flex_1()
                .min_w_0()
                .text_xs()
                .text_color(muted)
                .whitespace_nowrap()
                .when_some(first, |cell, day| {
                    cell.child(strings.chart_months[day.month0() as usize])
                })
        });
        let columns = weeks.iter().map(|week| {
            let days = week.iter().map(|&day| {
                let Some(day) = day else {
                    return div().h(DAY_HEIGHT).into_any_element();
                };
                let heat = charts::heat_level(data.calendar.count(day), most);
                let fill = if heat == 0 {
                    empty
                } else {
                    colors.green.opacity(heat_opacity(heat))
                };
                div()
                    .id(("chart-day", day.ordinal()))
                    .h(DAY_HEIGHT)
                    .rounded(px(2.))
                    .bg(fill)
                    .cursor_pointer()
                    .when(day == selected, |cell| cell.border_1().border_color(ring))
                    .on_click(cx.listener(move |app, _, _, cx| app.select_chart_day(day, cx)))
                    .into_any_element()
            });
            v_flex().flex_1().min_w_0().gap(DAY_GAP).children(days)
        });

        v_flex()
            .w_full()
            .max_w(CALENDAR_MAX_WIDTH)
            .gap_1()
            .child(h_flex().gap(DAY_GAP).children(months))
            .child(h_flex().items_start().gap(DAY_GAP).children(columns))
    }

    /// Every session side by side, the fastest of each column in green.
    fn render_compare(&self, data: &ChartData, cx: &mut Context<Self>) -> AnyElement {
        let strings = self.strings();
        let language = self.language();
        let colors = Palette::of(cx);
        let theme = cx.theme();
        let (muted, background) = (theme.muted_foreground, theme.background);
        let mono = theme.mono_font_family.clone();
        if data.compare.is_empty() {
            return empty_message(self.chart_empty_text(), muted);
        }

        let mut header = vec![strings.stat_single.to_owned(), strings.stat_mean.to_owned()];
        header.extend(OVERLAY_SIZES.into_iter().map(average_label));
        let fastest = charts::fastest(&data.compare);
        let rows: Vec<_> = data
            .compare
            .iter()
            .map(|row| {
                let values = (0..COMPARE_COLUMNS).map(|column| {
                    let value = row.values[column];
                    value_cell(format_value(value))
                        .when(value.is_some() && value == fastest[column], |cell| {
                            cell.text_color(colors.green)
                        })
                });
                table_row()
                    .font_family(mono.clone())
                    .when(row.session == self.current, StyledExt::font_semibold)
                    .child(name_cell(self.session_label(row.session)))
                    .child(value_cell(language.format_count(row.count as u64)))
                    .children(values)
            })
            .collect();

        let means: Vec<SessionMean> = data
            .compare
            .iter()
            .filter_map(|row| {
                Some(SessionMean {
                    label: self.session_label(row.session),
                    seconds: row.mean()? as f64 / 1000.0,
                })
            })
            .collect();
        let tick_margin = (means.len() / 10).max(1);
        let chart = (means.len() >= 2).then(|| {
            BarChart::new(means)
                .band(|mean: &SessionMean| mean.label.clone())
                .value(|mean: &SessionMean| mean.seconds)
                .fill(move |_, _, _, _| colors.blue)
                .tick_margin(tick_margin)
                .tooltip_value(|_: &SessionMean, seconds| {
                    format_ms((seconds * 1000.0).round() as u64).into()
                })
        });

        FadeScroll::new("chart-compare", background)
            .track(&self.chart_scroll)
            .flex_1()
            .min_h_0()
            .p_6()
            .gap_6()
            .child(
                v_flex()
                    .child(
                        table_row()
                            .text_color(muted)
                            .child(name_cell(strings.chart_column_session.to_owned()))
                            .child(value_cell(strings.stat_solves.to_owned()))
                            .children(header.into_iter().map(value_cell)),
                    )
                    .children(rows),
            )
            .when_some(chart, |column, chart| {
                column.child(
                    v_flex()
                        .gap_2()
                        .child(section_title(strings.chart_compare_means, muted))
                        .child(div().h(CHART_HEIGHT).child(chart)),
                )
            })
            .into_any_element()
    }

    /// Two tags side by side. Each works on what the other filters keep.
    fn render_tag_compare(&self, data: &ChartData, cx: &mut Context<Self>) -> AnyElement {
        let strings = self.strings();
        let language = self.language();
        let colors = Palette::of(cx);
        let theme = cx.theme();
        let (muted, background) = (theme.muted_foreground, theme.background);
        let mono = theme.mono_font_family.clone();
        if !self.has_tags() {
            return empty_message(strings.tags_none, muted);
        }
        let [left, right] = &data.tags;
        if left.count == 0 && right.count == 0 {
            return empty_message(self.chart_empty_text(), muted);
        }

        let mut names = vec![strings.stat_single.to_owned(), strings.stat_mean.to_owned()];
        names.extend(OVERLAY_SIZES.into_iter().map(average_label));
        let rows = names.into_iter().enumerate().map(|(column, name)| {
            let (first, second) = (left.values[column], right.values[column]);
            let fastest = first.into_iter().chain(second).min();
            // A tie has no winner to point out.
            let wins = |value: Option<u64>| value.is_some() && value == fastest && first != second;
            table_row()
                .child(name_cell(name))
                .child(
                    value_cell(format_value(first))
                        .font_family(mono.clone())
                        .when(wins(first), |cell| cell.text_color(colors.green)),
                )
                .child(
                    value_cell(format_value(second))
                        .font_family(mono.clone())
                        .when(wins(second), |cell| cell.text_color(colors.green)),
                )
        });

        FadeScroll::new("chart-tags", background)
            .track(&self.chart_scroll)
            .flex_1()
            .min_h_0()
            .p_6()
            .child(
                v_flex()
                    .child(
                        table_row()
                            .text_color(muted)
                            .child(name_cell(String::new()))
                            .child(
                                value_cell(tag_label(strings, self.chart_filter.tag.as_deref()))
                                    .truncate(),
                            )
                            .child(
                                value_cell(tag_label(strings, self.compare_tag.as_deref()))
                                    .truncate(),
                            ),
                    )
                    .child(
                        table_row()
                            .child(name_cell(strings.stat_solves.to_owned()))
                            .child(
                                value_cell(language.format_count(left.count as u64))
                                    .font_family(mono.clone()),
                            )
                            .child(
                                value_cell(language.format_count(right.count as u64))
                                    .font_family(mono.clone()),
                            ),
                    )
                    .children(rows),
            )
            .into_any_element()
    }

    /// Each event's best results, with the session each one came from.
    fn render_bests(&self, data: &ChartData, cx: &mut Context<Self>) -> AnyElement {
        let strings = self.strings();
        let language = self.language();
        let theme = cx.theme();
        let (muted, background) = (theme.muted_foreground, theme.background);
        let mono = theme.mono_font_family.clone();
        if data.bests.is_empty() {
            return empty_message(strings.chart_empty, muted);
        }

        let record_cell = |record: Option<Record>| {
            let cell = value_cell(format_value(record.map(|record| record.ms)))
                .font_family(mono.clone())
                .flex()
                .flex_col()
                .items_end();
            cell.when_some(record, |cell, record| {
                cell.child(
                    div()
                        .max_w_full()
                        .truncate()
                        .text_xs()
                        .text_color(muted)
                        .child(self.session_label(record.session)),
                )
            })
        };
        let rows = data.bests.iter().map(|bests| {
            table_row()
                .h_auto()
                .py_1()
                .child(name_cell(bests.event.name().to_owned()))
                .child(
                    value_cell(language.format_count(bests.count as u64)).font_family(mono.clone()),
                )
                .child(record_cell(bests.single))
                .children(bests.averages.iter().map(|&record| record_cell(record)))
        });

        FadeScroll::new("chart-bests", background)
            .track(&self.chart_scroll)
            .flex_1()
            .min_h_0()
            .p_6()
            .child(
                v_flex()
                    .child(
                        table_row()
                            .text_color(muted)
                            .child(name_cell(strings.chart_column_event.to_owned()))
                            .child(value_cell(strings.stat_solves.to_owned()))
                            .child(value_cell(strings.stat_single.to_owned()))
                            .children(
                                AVERAGE_SIZES
                                    .into_iter()
                                    .map(|size| value_cell(average_label(size))),
                            ),
                    )
                    .children(rows),
            )
            .into_any_element()
    }

    /// The message of a view with nothing to show: a filter can be to blame
    /// only while one is on.
    fn chart_empty_text(&self) -> &'static str {
        let strings = self.strings();
        if self.chart_filter == ChartFilter::default() {
            strings.chart_empty
        } else {
            strings.chart_no_match
        }
    }

    /// Returns the calendar's selected day, the latest with solves until one
    /// is chosen.
    fn chart_day(&self, data: &ChartData) -> NaiveDate {
        self.chart_day
            .or_else(|| data.calendar.latest())
            .unwrap_or_else(charts::today)
    }

    fn select_chart_day(&mut self, day: NaiveDate, cx: &mut Context<Self>) {
        self.chart_day = Some(day);
        cx.notify();
    }

    fn show_chart_view(&mut self, view: ChartView, cx: &mut Context<Self>) {
        self.chart_view = view;
        self.chart_scroll.set_offset(Point::default());
        cx.notify();
    }

    pub(crate) fn next_chart_view(&mut self, cx: &mut Context<Self>) {
        self.show_chart_view(self.chart_view.next(), cx);
    }

    fn set_chart_event(&mut self, event: Option<WcaEvent>, cx: &mut Context<Self>) {
        self.chart_filter.event = event;
        self.chart_filter_changed(cx);
    }

    fn set_chart_range(&mut self, range: ChartRange, cx: &mut Context<Self>) {
        self.chart_filter.range = range;
        self.chart_filter_changed(cx);
    }

    /// Shows every session, or the session at `index`.
    fn show_chart_sessions(&mut self, all: bool, index: usize, cx: &mut Context<Self>) {
        self.chart_filter.all_sessions = all;
        if index == self.current {
            self.chart_filter_changed(cx);
        } else {
            self.select_session(index, cx);
        }
    }

    pub(crate) fn toggle_chart_sessions(&mut self, cx: &mut Context<Self>) {
        self.show_chart_sessions(!self.chart_filter.all_sessions, self.current, cx);
    }

    fn session_labels(&self) -> Vec<String> {
        (0..self.sessions.len())
            .map(|index| self.session_label(index))
            .collect()
    }

    fn chart_filter_changed(&mut self, cx: &mut Context<Self>) {
        self.clear_chart_data();
        self.chart_day = None;
        cx.notify();
    }

    /// Returns whether any tag exists, so the controls for them mean anything.
    fn has_tags(&self) -> bool {
        !self.tag_usage.is_empty() || self.chart_filter.tag.is_some()
    }

    /// Returns whether what is showing follows the tag filter.
    pub(crate) fn tag_filter_shown(&self) -> bool {
        match self.screen {
            Screen::Stats => true,
            Screen::Charts => self.chart_view != ChartView::Bests,
            Screen::Timer | Screen::Goals | Screen::Trainer => false,
        }
    }

    fn set_tag_filter(&mut self, tag: Option<String>, cx: &mut Context<Self>) {
        self.chart_filter.tag = tag;
        self.clear_stats_table();
        self.stats_table_scroll
            .scroll_to_item(0, ScrollStrategy::Top);
        self.chart_filter_changed(cx);
    }

    fn set_compare_tag(&mut self, tag: Option<String>, cx: &mut Context<Self>) {
        self.compare_tag = tag;
        self.chart_filter_changed(cx);
    }

    pub(crate) fn next_tag_filter(&mut self, cx: &mut Context<Self>) {
        let tag = tags::next_filter(self.chart_filter.tag.as_deref(), &self.known_tags());
        self.set_tag_filter(tag, cx);
    }

    pub(crate) fn next_compare_tag(&mut self, cx: &mut Context<Self>) {
        let tag = tags::next_filter(self.compare_tag.as_deref(), &self.known_tags());
        self.set_compare_tag(tag, cx);
    }

    pub(crate) fn next_chart_event(&mut self, cx: &mut Context<Self>) {
        let available = charts::events(&self.sessions);
        self.set_chart_event(charts::next_event(self.chart_filter.event, &available), cx);
    }

    pub(crate) fn next_chart_range(&mut self, cx: &mut Context<Self>) {
        self.set_chart_range(self.chart_filter.range.next(), cx);
    }

    /// Returns the indices of the times chart's points showing.
    fn chart_range(&self, data: &ChartData) -> Range<usize> {
        let count = data.points.len();
        self.chart_zoom.map_or(0..count, |zoom| zoom.range(count))
    }

    /// Narrows or widens the times chart by `factor`, around `anchor`.
    pub(crate) fn zoom_chart(&mut self, factor: f64, anchor: f64, cx: &mut Context<Self>) {
        if self.chart_view != ChartView::Trends {
            return;
        }
        let count = self.chart_data().points.len();
        let zoom = self.chart_zoom.unwrap_or_else(|| ChartZoom::full(count));
        let zoom = zoom.scaled(count, factor, anchor);
        self.chart_zoom = (!zoom.is_full(count)).then_some(zoom);
        cx.notify();
    }

    fn pan_chart(&mut self, share: f64, cx: &mut Context<Self>) {
        let count = self.chart_data().points.len();
        self.chart_zoom = self.chart_zoom.map(|zoom| zoom.panned(count, share));
        cx.notify();
    }

    pub(crate) fn reset_chart_zoom(&mut self, cx: &mut Context<Self>) {
        self.chart_zoom = None;
        cx.notify();
    }

    /// Zooms around the cursor, or pans sideways, which is what `Shift` makes
    /// of the wheel. Zooming out with nothing left to show lets the wheel go
    /// on to scroll the page.
    fn on_chart_wheel(&mut self, event: &ScrollEvent, window: &mut Window, cx: &mut Context<Self>) {
        let delta = event.delta.pixel_delta(window.line_height());
        let (sideways, turned) = (f32::from(delta.x), f32::from(delta.y));
        if sideways != 0. {
            if self.chart_zoom.is_some() {
                cx.stop_propagation();
                self.pan_chart(f64::from(-sideways / WHEEL_PAN), cx);
            }
            return;
        }
        if turned == 0. || (turned < 0. && self.chart_zoom.is_none()) {
            return;
        }
        cx.stop_propagation();
        let anchor = self
            .chart_plot_bounds
            .get()
            .map_or(0.5, |bounds| plot_share(bounds, event.position));
        self.zoom_chart(f64::from((-turned / WHEEL_HALVING).exp2()), anchor, cx);
    }

    /// Runs an arrow key: on the calendar it moves the selected day, and
    /// elsewhere `Up` and `Down` scroll. `Left` and `Right` pan a zoomed chart.
    pub(crate) fn chart_arrow(&mut self, action: Action, cx: &mut Context<Self>) {
        if self.chart_view == ChartView::Calendar {
            let delta = match action {
                Action::PreviousSolve => -7,
                Action::NextSolve => 7,
                Action::PreviousAverage => -1,
                _ => 1,
            };
            let data = self.chart_data();
            let day = self.chart_day(&data);
            self.select_chart_day(charts::step_day(day, delta, charts::today()), cx);
            return;
        }
        let step = match action {
            Action::PreviousSolve => SCROLL_STEP,
            Action::NextSolve => -SCROLL_STEP,
            Action::PreviousAverage => return self.pan_chart(-KEY_PAN, cx),
            _ => return self.pan_chart(KEY_PAN, cx),
        };
        let offset = self.chart_scroll.offset();
        let lowest = -self.chart_scroll.max_offset().y;
        let y = (offset.y + step).clamp(lowest, px(0.));
        self.chart_scroll.set_offset(point(offset.x, y));
        cx.notify();
    }

    /// Asks for the first and last day of a custom range.
    pub(crate) fn open_chart_dates(&self, window: &mut Window, cx: &mut Context<Self>) {
        let strings = self.strings();
        let (from, to) = match self.chart_filter.range {
            ChartRange::Dates { from, to } => (charts::format_day(from), charts::format_day(to)),
            _ => (String::new(), String::new()),
        };
        let date_input = |value: String, window: &mut Window, cx: &mut Context<Self>| {
            cx.new(|cx| {
                InputState::new(window, cx)
                    .default_value(value)
                    .placeholder(strings.chart_date_placeholder)
            })
        };
        let from = date_input(from, window, cx);
        let to = date_input(to, window, cx);
        let entity = cx.entity().downgrade();
        let invalid = Rc::new(Cell::new(false));
        let first_build = Cell::new(true);
        window.open_dialog(cx, move |dialog, window, cx| {
            if first_build.replace(false) {
                from.focus_handle(cx).focus(window, cx);
            }
            let (from_value, to_value) = (from.clone(), to.clone());
            let (entity, invalid_on_save) = (entity.clone(), Rc::clone(&invalid));
            let save: Submit = Rc::new(move |window, cx| {
                let from = from_value.read(cx).value().to_string();
                let to = to_value.read(cx).value().to_string();
                let Some(range) = ChartRange::from_dates(&from, &to) else {
                    invalid_on_save.set(true);
                    window.refresh();
                    return;
                };
                entity
                    .update(cx, |app, cx| {
                        app.set_chart_range(range, cx);
                        app.focus.focus(window, cx);
                    })
                    .ok();
                window.close_dialog(cx);
            });
            let red = Palette::of(cx).red;
            dialog
                .title(dialog_title(strings.chart_dates_title, strings.close))
                .close_button(false)
                .w(dialog_width(DialogSize::COMPACT, window))
                .submit_on_enter(&save)
                .child(
                    v_flex()
                        .py_2()
                        .gap_3()
                        .child(date_field(
                            "chart-date-from",
                            strings.chart_date_from,
                            &from,
                        ))
                        .child(date_field("chart-date-to", strings.chart_date_to, &to))
                        .when(invalid.get(), |column| {
                            column.child(
                                div()
                                    .text_xs()
                                    .text_color(red)
                                    .child(strings.chart_dates_invalid),
                            )
                        }),
                )
                .footer(
                    h_flex()
                        .w_full()
                        .justify_end()
                        .gap_2()
                        .child(
                            Button::new("chart-dates-cancel")
                                .outline()
                                .label(strings.cancel)
                                .on_click(|_, window, cx| window.close_dialog(cx)),
                        )
                        .child(
                            Button::new("chart-dates-save")
                                .primary()
                                .label(strings.save)
                                .on_click(on_press(&save)),
                        ),
                )
        });
    }
}

const fn view_label(strings: &'static Strings, view: ChartView) -> &'static str {
    match view {
        ChartView::Trends => strings.chart_view_trends,
        ChartView::Calendar => strings.chart_view_calendar,
        ChartView::Compare => strings.chart_view_compare,
        ChartView::Tags => strings.field_tags,
        ChartView::Bests => strings.chart_view_bests,
    }
}

fn tag_label(strings: &'static Strings, tag: Option<&str>) -> String {
    tag.unwrap_or(strings.chart_all_tags).to_owned()
}

fn event_label(strings: &'static Strings, event: Option<WcaEvent>) -> &'static str {
    event.map_or(strings.chart_all_events, WcaEvent::name)
}

fn range_label(
    strings: &'static Strings,
    language: cube_locale::Language,
    range: ChartRange,
) -> String {
    match range {
        ChartRange::All => strings.chart_range_all.to_owned(),
        ChartRange::LastSolves(count) => strings
            .chart_range_last_solves
            .replace("{count}", &language.format_count(count as u64)),
        ChartRange::LastDays(count) => strings
            .chart_range_last_days
            .replace("{count}", &language.format_count(u64::from(count))),
        ChartRange::Dates { from, to } => strings
            .chart_range_dates
            .replace("{from}", &language.format_day(from))
            .replace("{to}", &language.format_day(to)),
    }
}

/// A filter's button, showing what it is set to.
fn filter_button(id: &'static str, label: impl Into<SharedString>) -> Button {
    let label = label.into();
    Button::new(id).outline().small().tab_stop(false).child(
        h_flex()
            .min_w_0()
            .gap_1()
            .items_center()
            .child(div().min_w_0().truncate().child(label))
            .child(Icon::new(IconName::ChevronDown).size(px(14.)).flex_none()),
    )
}

fn year_button(
    id: &'static str,
    delta: i32,
    strings: &'static Strings,
    cx: &mut Context<CubeApp>,
) -> impl IntoElement + use<> {
    let (icon, label) = if delta < 0 {
        (IconName::ChevronLeft, strings.chart_previous_year)
    } else {
        (IconName::ChevronRight, strings.chart_next_year)
    };
    Button::new(id)
        .ghost()
        .small()
        .icon(icon)
        .tooltip(label)
        .accessibility_label(label)
        .tab_stop(false)
        .on_click(cx.listener(move |app, _, window, cx| {
            let data = app.chart_data();
            let day = app.chart_day(&data);
            app.select_chart_day(charts::step_year(day, delta, charts::today()), cx);
            app.focus.focus(window, cx);
        }))
}

fn date_field(
    id: &'static str,
    label: &'static str,
    input: &Entity<InputState>,
) -> impl IntoElement + use<> {
    v_flex().gap_1().text_sm().child(label).child(
        div()
            .id(id)
            .on_mouse_down(MouseButton::Left, |_, _, cx| cx.stop_propagation())
            .child(Input::new(input).focus_bordered(false)),
    )
}

fn heat_opacity(heat: u8) -> f32 {
    debug_assert!(heat <= MAX_HEAT);
    HEAT_BASE + HEAT_STEP * f32::from(heat)
}

fn format_value(ms: Option<u64>) -> String {
    ms.map_or_else(|| "–".to_owned(), format_ms)
}

fn section_title(title: &'static str, color: Hsla) -> impl IntoElement {
    div().text_sm().text_color(color).child(title)
}

fn legend_item(color: Hsla, label: String) -> impl IntoElement {
    h_flex()
        .gap_1()
        .text_xs()
        .child(div().size(px(8.)).rounded_full().bg(color))
        .child(label)
}

/// The shared layout of a comparison or bests row.
fn table_row() -> Div {
    h_flex().w_full().h(Rems(2.)).gap_2().text_sm()
}

fn name_cell(name: String) -> Div {
    div()
        .flex_grow(NAME_COLUMN_GROW)
        .flex_shrink(1.)
        .flex_basis(relative(0.))
        .min_w_0()
        .truncate()
        .child(name)
}

fn value_cell(text: String) -> Div {
    div().flex_1().min_w_0().text_right().child(text)
}
