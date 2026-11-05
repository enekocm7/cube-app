//! The average details dialog: the solves behind one average, marking the
//! trimmed ones, and copying an average as text.

use cube_core::format::format_delta;
use cube_core::stats::{self, AVERAGE_SIZES, Average, AverageDetail, DEFAULT_AVERAGE, Trim};
use cube_locale::{Language, Strings};
use gpui_kit::component::button::{Button, ButtonVariants as _};
use gpui_kit::component::{ActiveTheme as _, WindowExt as _, h_flex, v_flex};
use gpui_kit::prelude::FluentBuilder as _;
use gpui_kit::*;

use crate::app::{CubeApp, average_label, format_average};
use crate::dialog_size::{DialogSize, dialog_body, dialog_body_height, dialog_title, dialog_width};
use crate::dialog_submit::{DialogSubmit as _, close_action, on_press};
use crate::palette::Palette;
use crate::scroll_fade::FadeScroll;
use crate::sidebar::solve_time_color;

const DIALOG_SIZE: DialogSize = DialogSize::STANDARD;
/// Room for the result and the range under it, which stay put while the
/// attempts scroll.
const HERO_HEIGHT: Rems = Rems(5.75);
const LEGEND_HEIGHT: Rems = Rems(1.5);
const ROW_HEIGHT: Rems = Rems(2.125);
const NUMBER_WIDTH: Rems = Rems(2.25);
const TIME_WIDTH: Rems = Rems(6.);
const NOTE_WIDTH: Rems = Rems(4.);
/// How far the lanes start: the number and time columns and the gap after them.
const LANE_INDENT: Rems = Rems(NUMBER_WIDTH.0 + TIME_WIDTH.0 + 0.75);
const DOT_SIZE: Pixels = px(10.);
const STEM_HEIGHT: Pixels = px(2.);

/// Returns the full name of an average: `Mean of 3` or `Average of 5`.
fn average_title(strings: &Strings, language: Language, size: usize) -> String {
    let template = if size == 3 {
        strings.mean_of
    } else {
        strings.average_of
    };
    template.replace("{size}", &language.format_count(size as u64))
}

impl CubeApp {
    /// Returns the window the average shortcuts act on.
    pub(crate) fn selected_average_size(&self) -> usize {
        AVERAGE_SIZES[self.selected_average.unwrap_or(DEFAULT_AVERAGE)]
    }

    /// Moves the average selection one window shorter or longer.
    pub(crate) fn step_selected_average(&mut self, forward: bool, cx: &mut Context<Self>) {
        self.selected_average = Some(stats::step_average(self.selected_average, forward));
        cx.notify();
    }

    /// Opens the selected average, ending at the target solve.
    pub(crate) fn open_selected_average(&self, window: &mut Window, cx: &mut Context<Self>) {
        let end = self.target_solve().unwrap_or_default();
        self.open_average(end, self.selected_average_size(), window, cx);
    }

    /// Opens the fastest average of the selected window.
    pub(crate) fn open_best_average(&self, window: &mut Window, cx: &mut Context<Self>) {
        let size = self.selected_average_size();
        let best = self.stats.windows.iter().find(|stats| stats.size == size);
        match best.and_then(|stats| stats.best) {
            Some((_, end)) => self.open_average(end, size, window, cx),
            None => self.warn_missing_average(size, window, cx),
        }
    }

    /// Copies the selected average, ending at the target solve.
    pub(crate) fn copy_selected_average(&self, window: &mut Window, cx: &mut Context<Self>) {
        let end = self.target_solve().unwrap_or_default();
        self.copy_average(end, self.selected_average_size(), window, cx);
    }

    fn copy_average(&self, end: usize, size: usize, window: &mut Window, cx: &mut Context<Self>) {
        let strings = self.strings();
        let Some(detail) = AverageDetail::at(self.session().solves(), end, size) else {
            self.warn_missing_average(size, window, cx);
            return;
        };
        let text = detail.share_text(&average_title(strings, self.language(), size));
        cx.write_to_clipboard(ClipboardItem::new_string(text));
        self.toast(strings.average_copied, window, cx);
    }

    fn warn_missing_average(&self, size: usize, window: &mut Window, cx: &mut Context<Self>) {
        let message = self
            .strings()
            .average_missing
            .replace("{label}", &average_label(size));
        self.toast_warning(&message, window, cx);
    }

    /// Opens the average of the `size` solves ending at `end`.
    ///
    /// The dialog reads the session on every build, so editing or deleting one
    /// of its solves from here updates it.
    pub(crate) fn open_average(
        &self,
        end: usize,
        size: usize,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if AverageDetail::at(self.session().solves(), end, size).is_none() {
            self.warn_missing_average(size, window, cx);
            return;
        }
        let app = cx.entity();
        window.open_dialog(cx, move |dialog, window, cx| {
            let state = app.read(cx);
            let strings = state.strings();
            let title = average_title(strings, state.language(), size);
            let max_height = dialog_body_height(DIALOG_SIZE, window);
            let body = render_body(&app, end, size, max_height, window.rem_size(), cx);
            let copy_app = app.clone();
            dialog
                .title(dialog_title(title, strings.close))
                .close_button(false)
                .w(dialog_width(DIALOG_SIZE, window))
                .close_on_enter()
                .child(body)
                .footer(
                    h_flex()
                        .w_full()
                        .justify_between()
                        .child(
                            Button::new("average-copy")
                                .outline()
                                .label(strings.copy_average)
                                .on_click(move |_, window, cx| {
                                    copy_app.update(cx, |app, cx| {
                                        app.copy_average(end, size, window, cx);
                                    });
                                }),
                        )
                        .child(
                            Button::new("average-close")
                                .primary()
                                .label(strings.close)
                                .on_click(on_press(&close_action())),
                        ),
                )
        });
    }
}
/// The result and the attempts behind it, or a note once a deletion has left
/// the window incomplete.
fn render_body(
    app: &Entity<CubeApp>,
    end: usize,
    size: usize,
    max_height: Pixels,
    rem_size: Pixels,
    cx: &App,
) -> AnyElement {
    let state = app.read(cx);
    let strings = state.strings();
    let language = state.language();
    let theme = cx.theme();
    let (background, muted) = (theme.background, theme.muted_foreground);
    let mono = theme.mono_font_family.clone();
    let Some(detail) = AverageDetail::at(state.session().solves(), end, size) else {
        let message = strings
            .average_missing
            .replace("{label}", &average_label(size));
        return dialog_body(div().text_color(muted).child(message)).into_any_element();
    };

    let is_dnf = detail.average() == Average::Dnf;
    let range = strings
        .average_range
        .replace(
            "{first}",
            &language.format_count((detail.first() + 1) as u64),
        )
        .replace("{last}", &language.format_count((end + 1) as u64));
    let rows: Vec<_> = (0..detail.attempts().len())
        .map(|position| render_attempt(app, &detail, position, strings, language, cx))
        .collect();

    dialog_body(
        v_flex()
            .child(
                v_flex()
                    .h(HERO_HEIGHT)
                    .child(
                        div()
                            .text_size(Rems(3.))
                            .line_height(Rems(3.375))
                            .font_family(mono)
                            .when(is_dnf, |result| result.text_color(Palette::of(cx).red))
                            .child(format_average(Some(detail.average()))),
                    )
                    .child(div().text_sm().text_color(muted).child(range)),
            )
            .child(lane_legend(strings, !is_dnf, muted))
            .child(
                FadeScroll::new("average-solves", background)
                    .max_h(
                        (max_height
                            - HERO_HEIGHT.to_pixels(rem_size)
                            - LEGEND_HEIGHT.to_pixels(rem_size))
                        .max(px(0.)),
                    )
                    .children(rows),
            ),
    )
    .into_any_element()
}

/// Names the two directions of the lanes, in line with them.
fn lane_legend(strings: &Strings, show: bool, muted: Hsla) -> impl IntoElement + use<> {
    h_flex()
        .h(LEGEND_HEIGHT)
        .px_3()
        .gap_3()
        .text_xs()
        .text_color(muted)
        .child(div().w(LANE_INDENT).flex_none())
        .child(
            h_flex()
                .flex_1()
                .mx_2()
                .justify_between()
                .when(show, |legend| {
                    legend
                        .child(strings.average_faster.to_owned())
                        .child(strings.average_slower.to_owned())
                }),
        )
        .child(div().w(NOTE_WIDTH).flex_none())
}

/// One attempt, as a button that opens the solve.
fn render_attempt(
    app: &Entity<CubeApp>,
    detail: &AverageDetail<'_>,
    position: usize,
    strings: &Strings,
    language: Language,
    cx: &App,
) -> impl IntoElement + use<> {
    let theme = cx.theme();
    let (muted, foreground) = (theme.muted_foreground, theme.foreground);
    let (axis, background) = (theme.border, theme.background);
    let mono = theme.mono_font_family.clone();
    let colors = Palette::of(cx);
    let solve = &detail.attempts()[position];
    let index = detail.first() + position;
    let trim = detail.trim(position);
    let deviation = detail.deviation(position);

    let direction = match deviation {
        Some(deviation) if deviation < 0 => colors.green,
        Some(deviation) if deviation > 0 => colors.orange,
        Some(_) => foreground,
        None => colors.red,
    };
    let (time_color, note, note_color) = match trim {
        Some(Trim::Best) => (
            Some(muted),
            Some(strings.average_trim_best.to_owned()),
            muted,
        ),
        Some(Trim::Worst) => (
            Some(muted),
            Some(strings.average_trim_worst.to_owned()),
            muted,
        ),
        None => (
            solve_time_color(solve, colors),
            deviation.map(format_delta),
            direction,
        ),
    };
    let lane = Lane {
        offset: detail.offset(position),
        trimmed: trim.is_some(),
        color: if trim.is_some() && deviation.is_some() {
            muted
        } else {
            direction
        },
        axis,
        background,
    };

    let app = app.clone();
    Button::new(("average-solve", index))
        .ghost()
        .w_full()
        .h(ROW_HEIGHT)
        .child(
            h_flex()
                .w_full()
                .h_full()
                .gap_3()
                .text_sm()
                .font_family(mono)
                .child(
                    div()
                        .w(NUMBER_WIDTH)
                        .flex_none()
                        .whitespace_nowrap()
                        .text_xs()
                        .text_right()
                        .text_color(muted)
                        .child(language.format_count((index + 1) as u64)),
                )
                .child(
                    div()
                        .w(TIME_WIDTH)
                        .flex_none()
                        .whitespace_nowrap()
                        .text_right()
                        .when_some(time_color, gpui_kit::Styled::text_color)
                        .child(detail.attempt_text(position)),
                )
                .child(lane.render())
                .child(
                    div()
                        .w(NOTE_WIDTH)
                        .flex_none()
                        .whitespace_nowrap()
                        .text_xs()
                        .text_right()
                        .text_color(note_color)
                        .when_some(note, ParentElement::child),
                ),
        )
        .on_click(move |_, window, cx| {
            app.update(cx, |app, cx| app.open_solve_details(index, window, cx));
        })
}

/// Where one attempt landed around the average, which runs down the middle of
/// every row. An attempt that counts is a solid dot tied to the average; a
/// trimmed one is a hollow ring on its own.
struct Lane {
    offset: Option<f32>,
    trimmed: bool,
    color: Hsla,
    axis: Hsla,
    background: Hsla,
}

impl Lane {
    fn render(self) -> Div {
        let Self {
            offset,
            trimmed,
            color,
            axis,
            background,
        } = self;
        let lane = div().relative().flex_1().h(ROW_HEIGHT).mx_2();
        let Some(offset) = offset else {
            return lane;
        };
        let at = offset.mul_add(0.5, 0.5);
        lane.child(
            div()
                .absolute()
                .top_0()
                .bottom_0()
                .left(relative(0.5))
                .w(px(1.))
                .bg(axis),
        )
        .when(!trimmed, |lane| {
            lane.child(
                div()
                    .absolute()
                    .top(relative(0.5))
                    .mt(-STEM_HEIGHT / 2.)
                    .left(relative(at.min(0.5)))
                    .w(relative((at - 0.5).abs()))
                    .h(STEM_HEIGHT)
                    .bg(color),
            )
        })
        .child(
            div()
                .absolute()
                .top(relative(0.5))
                .mt(-DOT_SIZE / 2.)
                .left(relative(at))
                .ml(-DOT_SIZE / 2.)
                .size(DOT_SIZE)
                .rounded_full()
                .map(|dot| {
                    if trimmed {
                        dot.border_2().border_color(color).bg(background)
                    } else {
                        dot.bg(color)
                    }
                }),
        )
    }
}
