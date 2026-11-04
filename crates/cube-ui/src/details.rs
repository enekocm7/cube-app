//! The solve details dialog, which is also where a solve is edited.

use chrono::{DateTime, Local};
use cube_core::format::{format_ms, parse_time};
use cube_core::validation::{
    format_date_input, parse_date, parse_date_input, validate_comment, validate_duration,
    validate_scramble,
};
use cube_core::{Modifier, Solve, SolveChange, SolveSnapshot, WcaEvent};
use gpui_kit::assets::IconName;
use gpui_kit::component::StyledExt as _;
use gpui_kit::component::button::{Button, ButtonVariants as _};
use gpui_kit::component::collapsible::Collapsible;
use gpui_kit::component::input::{Input, InputState, Textarea, TextareaState};
use gpui_kit::component::menu::{DropdownMenu as _, PopupMenuItem};
use gpui_kit::component::{
    ActiveTheme as _, Selectable as _, Sizable as _, WindowExt as _, h_flex, v_flex,
};
use gpui_kit::prelude::FluentBuilder as _;
use gpui_kit::*;

use crate::app::CubeApp;
use crate::dialog_size::{DialogSize, dialog_body, dialog_body_height, dialog_title, dialog_width};
use crate::dialog_submit::{DialogSubmit as _, Submit, on_press};
use crate::palette::Palette;
use crate::scroll_fade::FadeScroll;

/// One saved edit, with only the fields it changed.
struct HistoryEntry {
    header: String,
    rows: Vec<HistoryRow>,
}

/// One changed field of a saved edit.
struct HistoryRow {
    label: &'static str,
    before: String,
    after: String,
    mono: bool,
}

/// The editable form shown inside the dialog.
struct SolveForm {
    app: WeakEntity<CubeApp>,
    focus: FocusHandle,
    index: usize,
    changes: Vec<SolveChange>,
    event: WcaEvent,
    modifier: Modifier,
    /// The solve's seconds and milliseconds, kept while the date field only
    /// edits down to the minute: milliseconds don't matter for a date.
    date_subminute_ms: u64,
    history_expanded: bool,
    time: Entity<InputState>,
    scramble: Entity<TextareaState>,
    date: Entity<InputState>,
    comment: Entity<InputState>,
    time_error: Option<String>,
    scramble_error: Option<String>,
    date_error: Option<String>,
    comment_error: Option<String>,
    strings: &'static cube_locale::Strings,
}

impl SolveForm {
    /// Applies the form to the solve; returns whether the dialog may close.
    fn save(&mut self, window: &mut Window, cx: &mut Context<Self>) -> bool {
        let strings = self.strings;
        let event = self.event;
        let modifier = self.modifier;
        let time_text = self.time.read(cx).value().to_string();
        let scramble_text = self.scramble.read(cx).value().to_string();
        let date_text = self.date.read(cx).value().to_string();
        let comment_text = self.comment.read(cx).value().to_string();
        self.time_error = None;
        self.scramble_error = None;
        self.date_error = None;
        self.comment_error = None;

        let time_ms = match parse_time(&time_text) {
            Some(time_ms) if validate_duration(time_ms, modifier) => Some(time_ms),
            Some(_) => {
                self.time_error = Some(strings.invalid_duration.to_owned());
                None
            }
            None => {
                self.time_error = Some(strings.invalid_time.to_owned());
                None
            }
        };
        self.scramble_error = (!validate_scramble(scramble_text.trim(), event))
            .then(|| strings.invalid_scramble.replace("{event}", event.name()));
        let solved_at_unix_ms = if let Some(parsed) = parse_date_input(&date_text) {
            // Minute-precision edits keep the solve's seconds and
            // milliseconds; blank stays unknown.
            if parsed == 0 || parsed % 60_000 != 0 {
                Some(parsed)
            } else {
                Some(parsed + self.date_subminute_ms)
            }
        } else if let Some(parsed) = parse_date(&date_text) {
            // A pasted RFC 3339 date is kept exactly as typed.
            Some(parsed)
        } else {
            self.date_error = Some(strings.invalid_date.to_owned());
            None
        };
        self.comment_error =
            (!validate_comment(&comment_text)).then(|| strings.invalid_comment.to_owned());

        let (Some(time_ms), Some(solved_at_unix_ms)) = (time_ms, solved_at_unix_ms) else {
            cx.notify();
            return false;
        };
        if self.scramble_error.is_some() || self.comment_error.is_some() {
            cx.notify();
            return false;
        }
        let after = SolveSnapshot {
            time_ms,
            event,
            scramble: scramble_text.trim().to_owned(),
            solved_at_unix_ms,
            modifier,
            comment: comment_text.trim().to_owned(),
        };
        let index = self.index;
        self.app
            .update(cx, |app, cx| app.edit_solve(index, after, window, cx))
            .is_ok()
    }

    /// Snapshots the audit trail for rendering, newest first.
    fn history_entries(&self) -> Vec<HistoryEntry> {
        let strings = self.strings;
        self.changes
            .iter()
            .enumerate()
            .rev()
            .map(|(index, change)| {
                let header = strings
                    .history_revision
                    .replace("{number}", &(index + 1).to_string())
                    .replace("{date}", &format_date(change.changed_at_unix_ms, strings));
                let before = &change.before;
                let after = &change.after;
                let fields: [(&'static str, String, String, bool); 6] = [
                    (
                        strings.field_time,
                        format_ms(before.time_ms),
                        format_ms(after.time_ms),
                        false,
                    ),
                    (
                        strings.field_event,
                        before.event.name().to_owned(),
                        after.event.name().to_owned(),
                        false,
                    ),
                    (
                        strings.field_scramble,
                        before.scramble.clone(),
                        after.scramble.clone(),
                        true,
                    ),
                    (
                        strings.field_penalty,
                        penalty_label(before.modifier, strings),
                        penalty_label(after.modifier, strings),
                        false,
                    ),
                    (
                        strings.field_date,
                        format_date(before.solved_at_unix_ms, strings),
                        format_date(after.solved_at_unix_ms, strings),
                        false,
                    ),
                    (
                        strings.field_comment,
                        before.comment.clone(),
                        after.comment.clone(),
                        false,
                    ),
                ];
                let rows = fields
                    .into_iter()
                    .filter(|(_, before, after, _)| before != after)
                    .map(|(label, before, after, mono)| HistoryRow {
                        label,
                        before,
                        after,
                        mono,
                    })
                    .collect();
                HistoryEntry { header, rows }
            })
            .collect()
    }
}

impl Render for SolveForm {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let strings = self.strings;
        let colors = Palette::of(cx);
        let theme = cx.theme();
        let muted = theme.muted_foreground;
        let background = theme.background;
        let max_height = dialog_body_height(DialogSize::FORM, window);

        dialog_body(
            FadeScroll::new("solve-form", background)
                .track_focus(&self.focus)
                .max_h(max_height)
                .gap_3()
                .on_mouse_down(
                    MouseButton::Left,
                    cx.listener(|form, _, window, cx| {
                        // Clicking the dialog's empty space moves focus off any
                        // field; presses inside fields stop below instead.
                        form.focus.focus(window, cx);
                    }),
                )
                .child(self.render_event_and_date(cx))
                .child(
                    v_flex()
                        .gap_1()
                        .child(field_label(strings.field_scramble, muted))
                        .child(keep_focus("field-scramble", Textarea::new(&self.scramble)))
                        .children(field_error(self.scramble_error.as_ref(), colors.red)),
                )
                .child(
                    v_flex()
                        .gap_1()
                        .child(field_label(strings.field_time, muted))
                        .child(keep_focus("field-time", Input::new(&self.time)))
                        .children(field_error(self.time_error.as_ref(), colors.red)),
                )
                .child(self.render_penalty_row(cx))
                .child(
                    v_flex()
                        .gap_1()
                        .child(field_label(strings.field_comment, muted))
                        .child(keep_focus("field-comment", Input::new(&self.comment)))
                        .children(field_error(self.comment_error.as_ref(), colors.red)),
                )
                .child(self.render_history(cx)),
        )
    }
}

impl SolveForm {
    /// The event picker next to the date field.
    fn render_event_and_date(&self, cx: &mut Context<Self>) -> impl IntoElement {
        let strings = self.strings;
        let muted = cx.theme().muted_foreground;
        let red = Palette::of(cx).red;
        let event = self.event;
        let form = cx.entity().downgrade();
        h_flex()
            .gap_6()
            .child(
                v_flex()
                    .flex_1()
                    .gap_1()
                    .child(field_label(strings.field_event, muted))
                    .child(
                        Button::new("solve-event")
                            .outline()
                            .label(event.name())
                            .dropdown_caret(true)
                            .dropdown_menu(move |mut menu, _, _| {
                                for item in WcaEvent::ALL {
                                    let form = form.clone();
                                    menu = menu.item(
                                        PopupMenuItem::new(item.name())
                                            .checked(item == event)
                                            .on_click(move |_, _, cx| {
                                                form.update(
                                                    cx,
                                                    |form: &mut SolveForm,
                                                     cx: &mut Context<SolveForm>| {
                                                        form.event = item;
                                                        form.scramble_error = None;
                                                        cx.notify();
                                                    },
                                                )
                                                .ok();
                                            }),
                                    );
                                }
                                menu
                            }),
                    ),
            )
            .child(
                v_flex()
                    .flex_1()
                    .gap_1()
                    .child(field_label(strings.field_date, muted))
                    .child(keep_focus("field-date", Input::new(&self.date)))
                    .children(field_error(self.date_error.as_ref(), red)),
            )
    }

    /// The penalty choices.
    fn render_penalty_row(&self, cx: &mut Context<Self>) -> impl IntoElement {
        let strings = self.strings;
        let muted = cx.theme().muted_foreground;
        let current = self.modifier;
        let penalty =
            |id: &'static str, text: &'static str, modifier: Modifier, cx: &mut Context<Self>| {
                Button::new(id)
                    .outline()
                    .label(text)
                    .selected(current == modifier)
                    .on_click(cx.listener(move |form, _, _, cx| {
                        form.modifier = modifier;
                        cx.notify();
                    }))
            };
        v_flex()
            .gap_1()
            .child(field_label(strings.field_penalty, muted))
            .child(
                h_flex()
                    .gap_2()
                    .child(penalty(
                        "penalty-none",
                        strings.penalty_none,
                        Modifier::None,
                        cx,
                    ))
                    .child(penalty("penalty-two", "+2", Modifier::PlusTwo, cx))
                    .child(penalty("penalty-dnf", "DNF", Modifier::DNF, cx)),
            )
    }

    /// Every saved edit, newest first, with only the fields it changed.
    ///
    /// The entries hide behind a toggle that counts them, so the dialog stays
    /// short until the history is actually wanted.
    fn render_history(&self, cx: &mut Context<Self>) -> impl IntoElement {
        let strings = self.strings;
        let theme = cx.theme();
        let muted = theme.muted_foreground;
        let background = theme.background;
        let mono = theme.mono_font_family.clone();
        let count = self.changes.len();
        if count == 0 {
            return v_flex()
                .gap_1()
                .child(field_label(strings.edit_history, muted))
                .child(
                    div()
                        .text_sm()
                        .text_color(muted)
                        .child(strings.history_empty),
                )
                .into_any_element();
        }
        let entries = self.history_entries();
        let toggle_label = if self.history_expanded {
            strings.history_hide.to_owned()
        } else {
            strings.history_show.replace("{count}", &count.to_string())
        };
        Collapsible::new()
            .open(self.history_expanded)
            .child(
                Button::new("history-toggle")
                    .text()
                    .small()
                    .icon(if self.history_expanded {
                        IconName::ChevronUp
                    } else {
                        IconName::ChevronDown
                    })
                    .label(toggle_label)
                    .tab_stop(false)
                    .on_click(cx.listener(|form, _, _, cx| {
                        form.history_expanded = !form.history_expanded;
                        cx.notify();
                    })),
            )
            .content(
                FadeScroll::new("history-list", background)
                    .nested()
                    .max_h(px(260.))
                    .gap_3()
                    .pt_1()
                    .children(entries.into_iter().map(|entry| {
                        v_flex()
                            .gap_1()
                            .child(div().text_xs().font_semibold().child(entry.header))
                            .children(entry.rows.into_iter().map(|row| {
                                let value_font = mono.clone();
                                v_flex()
                                    .child(field_label(row.label, muted))
                                    .child(history_value(
                                        strings.history_before,
                                        &row.before,
                                        row.mono,
                                        value_font.clone(),
                                        strings,
                                    ))
                                    .child(history_value(
                                        strings.history_after,
                                        &row.after,
                                        row.mono,
                                        value_font,
                                        strings,
                                    ))
                            }))
                            .into_any_element()
                    })),
            )
            .into_any_element()
    }
}

/// A small muted caption above a field.
fn field_label(text: &'static str, muted: Hsla) -> impl IntoElement {
    div().text_xs().text_color(muted).child(text)
}

/// Wraps a text field so presses inside it never reach the dialog
/// background, which would pull focus away from the field just pressed.
fn keep_focus(id: &'static str, field: impl IntoElement) -> impl IntoElement {
    div()
        .id(id)
        .on_mouse_down(MouseButton::Left, |_, _, cx| cx.stop_propagation())
        .child(field)
}

/// A validation message below a field, if there is one.
fn field_error(message: Option<&String>, red: Hsla) -> Option<AnyElement> {
    message.cloned().map(|message| {
        div()
            .text_xs()
            .text_color(red)
            .child(message)
            .into_any_element()
    })
}

fn history_value(
    caption: &'static str,
    value: &str,
    mono: bool,
    mono_font: SharedString,
    strings: &'static cube_locale::Strings,
) -> AnyElement {
    let value = if value.is_empty() {
        strings.empty_value.to_owned()
    } else {
        value.to_owned()
    };
    div()
        .text_sm()
        .when(mono, |line| line.font_family(mono_font))
        .child(format!("{caption}: {value}"))
        .into_any_element()
}

fn penalty_label(modifier: Modifier, strings: &'static cube_locale::Strings) -> String {
    match modifier {
        Modifier::None => strings.penalty_none.to_owned(),
        Modifier::PlusTwo => "+2".to_owned(),
        Modifier::DNF => "DNF".to_owned(),
    }
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

        window.open_dialog(cx, move |dialog, window, _| {
            let (form_for_save, app_for_delete) = (form.clone(), app.clone());
            let save: Submit = std::rc::Rc::new(move |window, cx| {
                let saved = form_for_save.update(cx, |form, cx| form.save(window, cx));
                if saved {
                    window.close_dialog(cx);
                }
            });
            dialog
                .title(dialog_title(title.clone(), strings.close))
                .close_button(false)
                .w(dialog_width(DialogSize::FORM, window))
                .submit_on_enter(&save)
                .child(form.clone())
                .footer(
                    h_flex()
                        .w_full()
                        .justify_between()
                        .child(
                            Button::new("solve-delete")
                                .danger()
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
                                        .on_click(on_press(&save)),
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
    let scramble = solve.scramble().to_owned();
    let date = format_date_input(solve.solved_at_unix_ms());
    let comment = solve.comment().to_owned();
    let date_placeholder = strings.date_placeholder;
    let time = cx.new(|cx| InputState::new(window, cx).default_value(time));
    let scramble = cx.new(|cx| {
        TextareaState::new(window, cx)
            .default_value(scramble)
            .auto_grow(2, 4)
    });
    let date = cx.new(|cx| {
        InputState::new(window, cx)
            .default_value(date)
            .placeholder(date_placeholder)
    });
    let comment = cx.new(|cx| InputState::new(window, cx).default_value(comment));
    let changes = solve.changes().to_vec();
    let date_subminute_ms = solve.solved_at_unix_ms() % 60_000;
    let event = solve.event();
    let modifier = solve.modifier();
    cx.new(|cx| SolveForm {
        app,
        focus: cx.focus_handle(),
        index,
        changes,
        event,
        modifier,
        date_subminute_ms,
        history_expanded: false,
        time,
        scramble,
        date,
        comment,
        time_error: None,
        scramble_error: None,
        date_error: None,
        comment_error: None,
        strings,
    })
}
