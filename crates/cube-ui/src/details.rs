//! The solve details dialog, which is also where a solve is edited.

use chrono::{DateTime, Local};
use cube_core::format::{format_ms, parse_time};
use cube_core::{Modifier, Solve, SolveSnapshot};
use gpui_kit::component::button::{Button, ButtonVariants as _};
use gpui_kit::component::input::{Input, InputState};
use gpui_kit::component::{ActiveTheme as _, Selectable as _, WindowExt as _, h_flex, v_flex};
use gpui_kit::prelude::FluentBuilder as _;
use gpui_kit::*;

use crate::app::CubeApp;
use crate::palette::Palette;

/// The editable form shown inside the dialog.
struct SolveForm {
    app: WeakEntity<CubeApp>,
    index: usize,
    snapshot: SolveSnapshot,
    edit_count: usize,
    time: Entity<InputState>,
    comment: Entity<InputState>,
    modifier: Modifier,
    time_invalid: bool,
    strings: &'static cube_locale::Strings,
}

impl SolveForm {
    /// Applies the form to the solve; returns whether the dialog may close.
    fn save(&mut self, window: &mut Window, cx: &mut Context<Self>) -> bool {
        let Some(time_ms) = parse_time(&self.time.read(cx).value()) else {
            self.time_invalid = true;
            cx.notify();
            return false;
        };
        let after = SolveSnapshot {
            time_ms,
            modifier: self.modifier,
            comment: self.comment.read(cx).value().trim().to_owned(),
            ..self.snapshot.clone()
        };
        let index = self.index;
        self.app
            .update(cx, |app, cx| app.edit_solve(index, after, window, cx))
            .is_ok()
    }
}

impl Render for SolveForm {
    fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let strings = self.strings;
        let colors = Palette::of(cx);
        let theme = cx.theme();
        let muted = theme.muted_foreground;
        let mono = theme.mono_font_family.clone();
        let label = |text: &'static str| div().text_xs().text_color(muted).child(text);
        let penalty = |id: &'static str,
                       text: &'static str,
                       modifier: Modifier,
                       current: Modifier,
                       cx: &mut Context<Self>| {
            Button::new(id)
                .outline()
                .label(text)
                .selected(current == modifier)
                .on_click(cx.listener(move |form, _, _, cx| {
                    form.modifier = modifier;
                    cx.notify();
                }))
        };
        let current = self.modifier;

        v_flex()
            .gap_3()
            .child(
                h_flex()
                    .gap_6()
                    .child(
                        v_flex()
                            .gap_1()
                            .child(label(strings.field_event))
                            .child(self.snapshot.event.name()),
                    )
                    .child(
                        v_flex()
                            .gap_1()
                            .child(label(strings.field_date))
                            .child(format_date(self.snapshot.solved_at_unix_ms, strings)),
                    ),
            )
            .child(
                v_flex().gap_1().child(label(strings.field_scramble)).child(
                    div()
                        .font_family(mono)
                        .text_sm()
                        .child(self.snapshot.scramble.clone()),
                ),
            )
            .child(
                v_flex()
                    .gap_1()
                    .child(label(strings.field_time))
                    .child(Input::new(&self.time))
                    .when(self.time_invalid, |column| {
                        column.child(
                            div()
                                .text_xs()
                                .text_color(colors.red)
                                .child(strings.invalid_time),
                        )
                    }),
            )
            .child(
                v_flex().gap_1().child(label(strings.field_penalty)).child(
                    h_flex()
                        .gap_2()
                        .child(penalty(
                            "penalty-none",
                            strings.penalty_none,
                            Modifier::None,
                            current,
                            cx,
                        ))
                        .child(penalty("penalty-two", "+2", Modifier::PlusTwo, current, cx))
                        .child(penalty("penalty-dnf", "DNF", Modifier::DNF, current, cx)),
                ),
            )
            .child(
                v_flex()
                    .gap_1()
                    .child(label(strings.field_comment))
                    .child(Input::new(&self.comment)),
            )
            .when(self.edit_count > 0, |column| {
                column.child(label_owned(
                    strings
                        .edited_times
                        .replace("{count}", &self.edit_count.to_string()),
                    muted,
                ))
            })
    }
}

fn label_owned(text: String, color: Hsla) -> impl IntoElement {
    div().text_xs().text_color(color).child(text)
}

/// Formats a solve's date in local time; solves from old saves may have none.
fn format_date(unix_ms: u64, strings: &cube_locale::Strings) -> String {
    i64::try_from(unix_ms)
        .ok()
        .filter(|&ms| ms != 0)
        .and_then(DateTime::from_timestamp_millis)
        .map_or_else(
            || strings.date_unknown.to_owned(),
            |date| {
                date.with_timezone(&Local)
                    .format("%Y-%m-%d %H:%M")
                    .to_string()
            },
        )
}

impl CubeApp {
    pub(crate) fn open_solve_details(
        &self,
        index: usize,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let Some(solve) = self.session().solves().get(index) else {
            return;
        };
        let strings = self.strings();
        let app = cx.entity().downgrade();
        let form = new_form(app.clone(), index, solve, strings, window, cx);
        let title = format!("{} · #{}", strings.solve_details, index + 1);

        window.open_dialog(cx, move |dialog, _, _| {
            let (form_for_save, app_for_delete) = (form.clone(), app.clone());
            dialog
                .title(title.clone())
                .w(px(520.))
                .child(form.clone())
                .footer(
                    h_flex()
                        .w_full()
                        .justify_between()
                        .child(
                            Button::new("solve-delete")
                                .ghost()
                                .label(strings.delete_solve)
                                .on_click(move |_, window, cx| {
                                    window.close_dialog(cx);
                                    app_for_delete
                                        .update(cx, |app, cx| {
                                            app.confirm_delete_solve(index, window, cx);
                                        })
                                        .ok();
                                }),
                        )
                        .child(
                            h_flex()
                                .gap_2()
                                .child(
                                    Button::new("solve-cancel")
                                        .outline()
                                        .label(strings.cancel)
                                        .on_click(|_, window, cx| window.close_dialog(cx)),
                                )
                                .child(
                                    Button::new("solve-save")
                                        .primary()
                                        .label(strings.save)
                                        .on_click(move |_, window, cx| {
                                            let saved = form_for_save
                                                .update(cx, |form, cx| form.save(window, cx));
                                            if saved {
                                                window.close_dialog(cx);
                                            }
                                        }),
                                ),
                        ),
                )
        });
    }

    pub(crate) fn confirm_delete_solve(
        &self,
        index: usize,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let strings = self.strings();
        let on_confirm = cx.listener(move |app: &mut Self, (): &(), window, cx| {
            app.delete_solve(index, window, cx);
            app.focus.focus(window, cx);
        });
        self.open_confirm(
            strings.delete_solve,
            strings.delete_solve_confirm.to_owned(),
            on_confirm,
            window,
            cx,
        );
    }
}

fn new_form(
    app: WeakEntity<CubeApp>,
    index: usize,
    solve: &Solve,
    strings: &'static cube_locale::Strings,
    window: &mut Window,
    cx: &mut Context<CubeApp>,
) -> Entity<SolveForm> {
    let time = format_ms(solve.raw_ms());
    let comment = solve.comment().to_owned();
    let time = cx.new(|cx| InputState::new(window, cx).default_value(time));
    let comment = cx.new(|cx| InputState::new(window, cx).default_value(comment));
    let snapshot = solve.snapshot();
    let edit_count = solve.changes().len();
    let modifier = solve.modifier();
    cx.new(|_| SolveForm {
        app,
        index,
        snapshot,
        edit_count,
        time,
        comment,
        modifier,
        time_invalid: false,
        strings,
    })
}
