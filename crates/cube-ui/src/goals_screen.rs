//! The goals screen, the dialog that edits a goal, and the goals on the timer.
//!
//! Everything about what a goal means lives in `cube_core::goals`; this module
//! keeps the list up to date and draws it.

use std::cell::Cell;
use std::ops::Range;
use std::rc::Rc;

use chrono::Local;
use cube_core::format::format_ms;
use cube_core::goals::{
    self, MAX_COUNT, SINGLE, TARGET_SIZES, parse_count, parse_target, step_selection,
};
use cube_core::persistence;
use cube_core::{
    Goal, GoalError, GoalKind, GoalProgress, Measure, Period, SessionAnchor, WcaEvent,
};
use cube_locale::{Language, Strings};
use gpui_kit::assets::IconName;
use gpui_kit::component::button::{Button, ButtonVariants as _};
use gpui_kit::component::input::{Input, InputState};
use gpui_kit::component::menu::{DropdownMenu as _, PopupMenuItem};
use gpui_kit::component::notification::Notification;
use gpui_kit::component::progress::Progress;
use gpui_kit::component::scroll::Scrollbar;
use gpui_kit::component::{
    ActiveTheme as _, Selectable as _, Sizable as _, WindowExt as _, h_flex, v_flex,
};
use gpui_kit::prelude::FluentBuilder as _;
use gpui_kit::*;

use crate::app::{CubeApp, Screen, average_label, format_average};
use crate::details::{field_error, field_label, keep_focus};
use crate::dialog_size::{DialogSize, dialog_body, dialog_body_height, dialog_title, dialog_width};
use crate::dialog_submit::{DialogSubmit as _, Submit, on_press};
use crate::palette::Palette;
use crate::scroll_fade::{FadeScroll, fading_scroll};
use crate::stats_screen::empty_message;

const DIALOG_SIZE: DialogSize = DialogSize::FORM;
/// Height of a row of the list, and of the card inside it.
const ROW_HEIGHT: Rems = Rems(4.5);
const CARD_HEIGHT: Rems = Rems(4.);
/// Width of a card's progress column.
const PROGRESS_WIDTH: Rems = Rems(14.);
/// Room kept on a row's right for the overlay scrollbar.
const SCROLLBAR_GUTTER: Pixels = px(16.);

/// What a new goal starts with in the form.
const DEFAULT_SOLVES: u32 = 50;
const DEFAULT_DAYS: u32 = 7;
const DEFAULT_SIZE: usize = 12;

/// Describes a goal in one line.
fn goal_summary(goal: &Goal, strings: &Strings, language: Language) -> String {
    let count = |count: u32| language.format_count(u64::from(count));
    match goal.kind {
        GoalKind::Volume { solves, period } => match period {
            Period::Day => strings.goal_summary_day,
            Period::Week => strings.goal_summary_week,
        }
        .replace("{count}", &count(solves)),
        GoalKind::TargetTime { target_ms, size } => strings
            .goal_summary_time
            .replace("{result}", &result_label(size, strings))
            .replace("{time}", &format_ms(target_ms)),
        GoalKind::Streak {
            days,
            solves_per_day,
        } => strings
            .goal_summary_streak
            .replace("{count}", &count(solves_per_day))
            .replace("{days}", &count(days)),
    }
}

/// Names a single or an average, as a target time counts it.
fn result_label(size: usize, strings: &Strings) -> String {
    if size == SINGLE {
        strings.stat_single.to_owned()
    } else {
        average_label(size)
    }
}

/// The progress as figures, such as `23 / 50` or `21.34 / 20.00`.
fn progress_text(progress: &GoalProgress, language: Language) -> String {
    match progress.measure {
        Measure::Count { done, target } => format!(
            "{} / {}",
            language.format_count(u64::from(done)),
            language.format_count(u64::from(target))
        ),
        Measure::Time { best, target_ms } => {
            format!("{} / {}", format_average(best), format_ms(target_ms))
        }
    }
}

impl CubeApp {
    /// Brings the progress up to date when the solves or the day have changed,
    /// and notes the goals that were just reached.
    pub(crate) fn refresh_goals(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let now = Local::now();
        if !self.goals_stale && now.date_naive() == self.goals_day {
            return;
        }
        self.goals_stale = false;
        self.goals_day = now.date_naive();
        let moved = goals::follow_sessions(&mut self.goals, &self.sessions);
        let progress = goals::progress_of(&self.goals, &self.sessions, &now);
        let reached = goals::newly_reached(&self.goal_progress, &progress);
        self.goal_progress = progress;
        if moved {
            self.save_goals(window, cx);
        }
        let (strings, language) = (self.strings(), self.language());
        for index in reached {
            let summary = goal_summary(&self.goals[index], strings, language);
            let message = strings.toast_goal_reached.replace("{goal}", &summary);
            self.toast(&message, window, cx);
        }
        cx.notify();
    }

    /// Writes the goals to disk. Without a path, which means the file on disk
    /// could not be read, nothing is written over it.
    fn save_goals(&self, window: &mut Window, cx: &mut Context<Self>) {
        if let Some(path) = &self.goals_path
            && let Err(error) = persistence::save_goals(path, &self.goals)
        {
            let message = self
                .strings()
                .toast_goals_save_failed
                .replace("{error}", &format!("{error:#}"));
            window.push_notification(Notification::error(message), cx);
        }
    }

    /// Catches up after the list itself changed. The progress is replaced
    /// outright: a goal typed in as already met has not just been reached.
    fn goals_edited(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        self.goal_progress = goals::progress_of(&self.goals, &self.sessions, &Local::now());
        self.selected_goal = self
            .selected_goal
            .map(|index| index.min(self.goals.len().saturating_sub(1)))
            .filter(|_| !self.goals.is_empty());
        self.save_goals(window, cx);
        cx.notify();
    }

    /// Returns the goal an action applies to: the selection, or the first.
    fn target_goal(&self) -> Option<usize> {
        let last = self.goals.len().checked_sub(1)?;
        Some(self.selected_goal.unwrap_or(0).min(last))
    }

    /// Moves the goal selection down the list (`forward`) or up it.
    pub(crate) fn step_goal(&mut self, forward: bool, cx: &mut Context<Self>) {
        self.selected_goal = step_selection(self.selected_goal, self.goals.len(), forward);
        if let Some(index) = self.selected_goal {
            self.goals_scroll
                .scroll_to_item(index, ScrollStrategy::Nearest);
        }
        cx.notify();
    }

    /// Edits the selected goal.
    pub(crate) fn open_target_goal(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if let Some(index) = self.target_goal() {
            self.open_goal_form(Some(index), window, cx);
        }
    }

    /// Deletes the selected goal.
    pub(crate) fn delete_target_goal(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if let Some(index) = self.target_goal() {
            self.confirm_delete_goal(index, window, cx);
        }
    }

    /// Deletes the goal at `index`, asking first when confirmations are on.
    fn confirm_delete_goal(&mut self, index: usize, window: &mut Window, cx: &mut Context<Self>) {
        if !self.settings.confirm_delete {
            self.delete_goal(index, window, cx);
            self.focus.focus(window, cx);
            return;
        }
        let strings = self.strings();
        let on_confirm = cx.listener(move |app: &mut Self, (): &(), window, cx| {
            app.delete_goal(index, window, cx);
            app.focus.focus(window, cx);
        });
        self.open_confirm(
            strings.goals_delete,
            strings.goals_delete_confirm.to_owned(),
            on_confirm,
            window,
            cx,
        );
    }

    fn delete_goal(&mut self, index: usize, window: &mut Window, cx: &mut Context<Self>) {
        if index < self.goals.len() {
            self.goals.remove(index);
            self.goals_edited(window, cx);
        }
    }

    /// Adds a goal, or replaces the one at `index`.
    fn store_goal(
        &mut self,
        index: Option<usize>,
        draft: GoalDraft,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let GoalDraft { event, kind, scope } = draft;
        let session = match scope {
            Scope::All => None,
            Scope::Session(session) => SessionAnchor::new(&self.sessions, session),
            Scope::Missing => index
                .and_then(|index| self.goals.get(index))
                .and_then(|goal| goal.session.clone()),
        };
        let goal = Goal {
            event,
            kind,
            session,
        };
        let position = if let Some(index) = index.filter(|&index| index < self.goals.len()) {
            self.goals[index] = goal;
            index
        } else {
            self.goals.push(goal);
            self.goals.len() - 1
        };
        self.selected_goal = Some(position);
        self.goals_scroll
            .scroll_to_item(position, ScrollStrategy::Nearest);
        self.goals_edited(window, cx);
        self.focus.focus(window, cx);
    }

    /// Opens the form for a new goal, or for the one at `index`.
    pub(crate) fn open_goal_form(
        &mut self,
        index: Option<usize>,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let strings = self.strings();
        if self.goals_path.is_none() {
            let message = strings.goals_unavailable;
            window.push_notification(Notification::warning(message), cx);
            return;
        }
        let form = self.new_goal_form(index, window, cx);
        let app = cx.entity().downgrade();
        let title = if index.is_some() {
            strings.goals_edit
        } else {
            strings.goals_new
        };
        let first_build = Cell::new(true);
        window.open_dialog(cx, move |dialog, window, cx| {
            if first_build.replace(false) {
                // So Tab reaches the first control and Enter saves right away.
                let focus = form.read(cx).focus.clone();
                focus.focus(window, cx);
            }
            let form_for_save = form.clone();
            let save: Submit = Rc::new(move |window, cx| {
                let saved = form_for_save.update(cx, |form, cx| form.save(window, cx));
                if saved {
                    window.close_dialog(cx);
                }
            });
            let app = app.clone();
            let buttons = h_flex()
                .gap_2()
                .child(
                    Button::new("goal-cancel")
                        .outline()
                        .label(strings.cancel)
                        .on_click(|_, window, cx| window.close_dialog(cx)),
                )
                .child(
                    Button::new("goal-save")
                        .primary()
                        .label(strings.save)
                        .on_click(on_press(&save)),
                );
            dialog
                .title(dialog_title(title, strings.close))
                .close_button(false)
                .w(dialog_width(DIALOG_SIZE, window))
                .submit_on_enter(&save)
                .child(form.clone())
                .footer(
                    h_flex()
                        .w_full()
                        .justify_between()
                        .child(div().when_some(index, |slot, index| {
                            slot.child(
                                Button::new("goal-delete")
                                    .danger()
                                    .label(strings.goals_delete)
                                    .on_click(move |_, window, cx| {
                                        window.close_dialog(cx);
                                        app.update(cx, |app, cx| {
                                            app.confirm_delete_goal(index, window, cx);
                                        })
                                        .ok();
                                    }),
                            )
                        }))
                        .child(buttons),
                )
        });
    }

    fn new_goal_form(
        &self,
        index: Option<usize>,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> Entity<GoalForm> {
        let goal = index.and_then(|index| self.goals.get(index));
        let progress = index.and_then(|index| self.goal_progress.get(index));
        let scope = match (goal.and_then(|goal| goal.session.as_ref()), progress) {
            (None, _) => Scope::All,
            (
                Some(_),
                Some(GoalProgress {
                    session: Some(session),
                    ..
                }),
            ) => Scope::Session(*session),
            (Some(_), _) => Scope::Missing,
        };
        let mut solves = DEFAULT_SOLVES;
        let mut days = DEFAULT_DAYS;
        let mut target = String::new();
        let mut size = DEFAULT_SIZE;
        let mut period = Period::Day;
        let mut choice = KindChoice::Volume;
        match goal.map(|goal| goal.kind) {
            None => {}
            Some(GoalKind::Volume {
                solves: count,
                period: every,
            }) => {
                solves = count;
                period = every;
            }
            Some(GoalKind::TargetTime {
                target_ms,
                size: result,
            }) => {
                choice = KindChoice::Time;
                target = format_ms(target_ms);
                size = result;
            }
            Some(GoalKind::Streak {
                days: streak,
                solves_per_day,
            }) => {
                choice = KindChoice::Streak;
                days = streak;
                solves = solves_per_day;
            }
        }
        let number = |value: u32, window: &mut Window, cx: &mut Context<Self>| {
            cx.new(|cx| InputState::new(window, cx).default_value(value.to_string()))
        };
        let solves = number(solves, window, cx);
        let days = number(days, window, cx);
        let target = cx.new(|cx| InputState::new(window, cx).default_value(target));
        let app = cx.entity().downgrade();
        let sessions = (0..self.sessions.len())
            .map(|index| self.session_label(index))
            .collect();
        let event = goal.map_or(self.event, |goal| goal.event);
        let (strings, language) = (self.strings(), self.language());
        cx.new(|cx| GoalForm {
            app,
            focus: cx.focus_handle(),
            index,
            event,
            choice,
            period,
            size,
            scope,
            sessions,
            missing_session: goal
                .and_then(|goal| goal.session.as_ref())
                .map(|anchor| anchor.name().to_owned())
                .filter(|name| !name.is_empty()),
            solves,
            days,
            target,
            solves_error: None,
            days_error: None,
            target_error: None,
            strings,
            language,
        })
    }

    /// The title bar's goals button, beside the Bluetooth one.
    pub(crate) fn render_goals_button(&self, cx: &mut Context<Self>) -> impl IntoElement + use<> {
        let title = self.strings().goals_title;
        Button::new("open-goals")
            .ghost()
            .small()
            .icon(IconName::CircleCheck)
            .selected(self.screen == Screen::Goals)
            .tooltip(title)
            .accessibility_label(title)
            .tab_stop(false)
            .on_click(cx.listener(|app, _, window, cx| {
                if !app.timer.is_active() {
                    app.toggle_screen(Screen::Goals, cx);
                }
                app.focus.focus(window, cx);
            }))
    }

    /// The screen listing every goal.
    pub(crate) fn render_goals(&self, cx: &mut Context<Self>) -> AnyElement {
        let strings = self.strings();
        let theme = cx.theme();
        let (background, muted) = (theme.background, theme.muted_foreground);
        let count = self.goals.len();
        let body = if count > 0 {
            div()
                .relative()
                .flex_1()
                .min_h_0()
                .child(
                    fading_scroll(
                        uniform_list("goals-list", count, cx.processor(Self::render_goal_rows))
                            .size_full()
                            .track_scroll(&self.goals_scroll),
                        &self.goals_scroll,
                        background,
                    )
                    .size_full(),
                )
                .child(Scrollbar::vertical(&self.goals_scroll).mode(self.scrollbar_mode()))
                .into_any_element()
        } else if self.goals_path.is_some() {
            empty_message(strings.goals_empty, muted)
        } else {
            empty_message(strings.goals_unavailable, muted)
        };

        v_flex()
            .flex_1()
            .min_h_0()
            .px_6()
            .py_4()
            .gap_2()
            .child(
                h_flex()
                    .flex_none()
                    .pr(SCROLLBAR_GUTTER)
                    .justify_between()
                    .child(
                        div()
                            .text_lg()
                            .font_weight(FontWeight::SEMIBOLD)
                            .child(strings.goals_title),
                    )
                    .child(
                        Button::new("goal-new")
                            .primary()
                            .small()
                            .icon(IconName::Plus)
                            .label(strings.goals_new)
                            .tab_stop(false)
                            .on_click(cx.listener(|app, _, window, cx| {
                                app.open_goal_form(None, window, cx);
                            })),
                    ),
            )
            .child(body)
            .into_any_element()
    }

    fn render_goal_rows(
        &mut self,
        range: Range<usize>,
        _: &mut Window,
        cx: &mut Context<Self>,
    ) -> Vec<AnyElement> {
        range
            .filter_map(|index| {
                let goal = self.goals.get(index)?;
                let progress = self.goal_progress.get(index)?;
                Some(self.render_goal_row(index, goal, progress, cx))
            })
            .collect()
    }

    fn render_goal_row(
        &self,
        index: usize,
        goal: &Goal,
        progress: &GoalProgress,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        let (strings, language) = (self.strings(), self.language());
        let colors = Palette::of(cx);
        let theme = cx.theme();
        let (border, muted, hover) = (theme.border, theme.muted_foreground, theme.muted);
        let accent = theme.accent;
        let selected = self.target_goal() == Some(index);
        let scope = match progress.session {
            _ if progress.paused => strings.goal_paused.to_owned(),
            Some(session) => self.session_label(session),
            None => strings.goal_all_sessions.to_owned(),
        };

        h_flex()
            .id(("goal", index))
            .w_full()
            .h(ROW_HEIGHT)
            .pr(SCROLLBAR_GUTTER)
            .on_click(cx.listener(move |app, _, window, cx| {
                app.selected_goal = Some(index);
                app.focus.focus(window, cx);
                cx.notify();
            }))
            .child(
                h_flex()
                    .w_full()
                    .h(CARD_HEIGHT)
                    .px_3()
                    .gap_4()
                    .rounded_md()
                    .border_1()
                    .border_color(border)
                    .cursor_pointer()
                    .hover(move |card| card.bg(hover))
                    .when(selected, |card| card.bg(accent))
                    .child(
                        v_flex()
                            .flex_1()
                            .min_w_0()
                            .gap_1()
                            .child(
                                div()
                                    .truncate()
                                    .child(goal_summary(goal, strings, language)),
                            )
                            .child(
                                div()
                                    .text_xs()
                                    .truncate()
                                    .text_color(if progress.paused {
                                        colors.orange
                                    } else {
                                        muted
                                    })
                                    .child(format!("{} · {scope}", goal.event.name())),
                            ),
                    )
                    .child(self.render_goal_meter(index, progress, cx))
                    .child(self.render_goal_buttons(index, cx)),
            )
            .into_any_element()
    }

    /// A goal's figures over its progress bar.
    fn render_goal_meter(&self, index: usize, progress: &GoalProgress, cx: &App) -> Div {
        let colors = Palette::of(cx);
        let reached = progress.reached();
        v_flex()
            .flex_none()
            .w(PROGRESS_WIDTH)
            .gap_1()
            .child(
                h_flex()
                    .justify_between()
                    .gap_2()
                    .text_xs()
                    .child(
                        div()
                            .min_w_0()
                            .truncate()
                            .text_color(colors.green)
                            .when(reached, |label| label.child(self.strings().goal_reached)),
                    )
                    .child(
                        div()
                            .flex_none()
                            .font_family(cx.theme().mono_font_family.clone())
                            .child(progress_text(progress, self.language())),
                    ),
            )
            .child(
                Progress::new(("goal-progress", index))
                    .small()
                    .color(if reached { colors.green } else { colors.blue })
                    .value(progress.percent()),
            )
    }

    /// The mouse's way to edit or delete a goal.
    fn render_goal_buttons(&self, index: usize, cx: &mut Context<Self>) -> Div {
        let strings = self.strings();
        h_flex()
            .flex_none()
            .gap_1()
            .child(
                Button::new(("goal-edit", index))
                    .ghost()
                    .small()
                    .icon(IconName::Pencil)
                    .tooltip(strings.goals_edit)
                    .accessibility_label(strings.goals_edit)
                    .tab_stop(false)
                    .on_click(cx.listener(move |app, _, window, cx| {
                        app.selected_goal = Some(index);
                        app.open_goal_form(Some(index), window, cx);
                    })),
            )
            .child(
                Button::new(("goal-remove", index))
                    .ghost()
                    .small()
                    .icon(IconName::Trash)
                    .tooltip(strings.goals_delete)
                    .accessibility_label(strings.goals_delete)
                    .tab_stop(false)
                    .on_click(cx.listener(move |app, _, window, cx| {
                        app.confirm_delete_goal(index, window, cx);
                    })),
            )
    }

    /// The stats panel's section with the goals the timer is working towards.
    pub(crate) fn render_active_goals(&self, cx: &Context<Self>) -> Option<Div> {
        let (strings, language) = (self.strings(), self.language());
        let colors = Palette::of(cx);
        let theme = cx.theme();
        let (border, muted) = (theme.border, theme.muted_foreground);
        let foreground = theme.foreground;
        let mono = theme.mono_font_family.clone();
        let rows: Vec<Div> = self
            .goals
            .iter()
            .zip(&self.goal_progress)
            .enumerate()
            .filter(|(_, (goal, progress))| goal.is_active_on(progress, self.event, self.current))
            .map(|(index, (goal, progress))| {
                let reached = progress.reached();
                let (color, bar) = if reached {
                    (colors.green, colors.green)
                } else {
                    (foreground, colors.blue)
                };
                v_flex()
                    .flex_none()
                    .px_3()
                    .py_1()
                    .gap_1()
                    .text_xs()
                    .child(
                        h_flex()
                            .gap_2()
                            .child(
                                div()
                                    .flex_1()
                                    .min_w_0()
                                    .truncate()
                                    .text_color(muted)
                                    .child(goal_summary(goal, strings, language)),
                            )
                            .child(
                                div()
                                    .flex_none()
                                    .font_family(mono.clone())
                                    .text_color(color)
                                    .child(if reached {
                                        strings.goal_reached.to_owned()
                                    } else {
                                        progress_text(progress, language)
                                    }),
                            ),
                    )
                    .child(
                        Progress::new(("goal-panel", index))
                            .xsmall()
                            .color(bar)
                            .value(progress.percent()),
                    )
            })
            .collect();
        (!rows.is_empty()).then(|| {
            v_flex()
                .flex_none()
                .mt_2()
                .pt_1()
                .border_t_1()
                .border_color(border)
                .child(
                    div()
                        .px_3()
                        .text_xs()
                        .text_color(muted)
                        .child(strings.goals_title),
                )
                .children(rows)
        })
    }
}

/// The three kinds of goal, as the form offers them.
#[derive(Clone, Copy, PartialEq, Eq)]
enum KindChoice {
    Volume,
    Time,
    Streak,
}

/// Which solves the form's goal counts.
#[derive(Clone, Copy, PartialEq, Eq)]
enum Scope {
    All,
    Session(usize),
    /// The session the goal was limited to, which is deleted for now.
    Missing,
}

/// What the form hands over once it is valid.
#[derive(Clone, Copy)]
struct GoalDraft {
    event: WcaEvent,
    kind: GoalKind,
    scope: Scope,
}

/// The dialog's contents.
struct GoalForm {
    app: WeakEntity<CubeApp>,
    focus: FocusHandle,
    /// The goal being edited; `None` adds a new one.
    index: Option<usize>,
    event: WcaEvent,
    choice: KindChoice,
    period: Period,
    size: usize,
    scope: Scope,
    sessions: Vec<String>,
    /// The name of the deleted session of a paused goal, if it had one.
    missing_session: Option<String>,
    solves: Entity<InputState>,
    days: Entity<InputState>,
    target: Entity<InputState>,
    solves_error: Option<String>,
    days_error: Option<String>,
    target_error: Option<String>,
    strings: &'static Strings,
    language: Language,
}

impl GoalForm {
    fn error_message(&self, error: GoalError) -> String {
        match error {
            GoalError::InvalidCount => self
                .strings
                .goal_error_count
                .replace("{max}", &self.language.format_count(u64::from(MAX_COUNT))),
            GoalError::InvalidTime | GoalError::InvalidSize => self.strings.invalid_time.to_owned(),
        }
    }

    /// Applies the form; returns whether the dialog may close.
    fn save(&mut self, window: &mut Window, cx: &mut Context<Self>) -> bool {
        let text = |input: &Entity<InputState>, cx: &App| input.read(cx).value().to_string();
        let choice = self.choice;
        let solves = (choice != KindChoice::Time).then(|| parse_count(&text(&self.solves, cx)));
        let days = (choice == KindChoice::Streak).then(|| parse_count(&text(&self.days, cx)));
        let target = (choice == KindChoice::Time).then(|| parse_target(&text(&self.target, cx)));
        self.solves_error = solves
            .and_then(Result::err)
            .map(|error| self.error_message(error));
        self.days_error = days
            .and_then(Result::err)
            .map(|error| self.error_message(error));
        self.target_error = target
            .and_then(Result::err)
            .map(|error| self.error_message(error));

        let kind = match (choice, solves, days, target) {
            (KindChoice::Volume, Some(Ok(solves)), _, _) => GoalKind::Volume {
                solves,
                period: self.period,
            },
            (KindChoice::Time, _, _, Some(Ok(target_ms))) => GoalKind::TargetTime {
                target_ms,
                size: self.size,
            },
            (KindChoice::Streak, Some(Ok(solves_per_day)), Some(Ok(days)), _) => GoalKind::Streak {
                days,
                solves_per_day,
            },
            _ => {
                cx.notify();
                return false;
            }
        };
        let draft = GoalDraft {
            event: self.event,
            kind,
            scope: self.scope,
        };
        let index = self.index;
        self.app
            .update(cx, |app, cx| app.store_goal(index, draft, window, cx))
            .is_ok()
    }

    /// A button opening a list of `items`, one of which is `current`.
    fn menu<T: Copy + PartialEq + 'static>(
        id: &'static str,
        label: String,
        items: Vec<(T, String)>,
        current: T,
        apply: fn(&mut Self, T),
        cx: &mut Context<Self>,
    ) -> impl IntoElement + use<T> {
        let form = cx.entity().downgrade();
        Button::new(id)
            .outline()
            .label(label)
            .dropdown_caret(true)
            .dropdown_menu(move |mut menu, _, _| {
                for (value, text) in &items {
                    let (form, value) = (form.clone(), *value);
                    menu = menu.item(
                        PopupMenuItem::new(text.clone())
                            .checked(value == current)
                            .on_click(move |_, _, cx| {
                                form.update(cx, |form: &mut Self, cx: &mut Context<Self>| {
                                    apply(form, value);
                                    cx.notify();
                                })
                                .ok();
                            }),
                    );
                }
                menu
            })
    }

    fn render_kind_row(&self, cx: &mut Context<Self>) -> impl IntoElement + use<> {
        let strings = self.strings;
        let muted = cx.theme().muted_foreground;
        let current = self.choice;
        let kind =
            |id: &'static str, text: &'static str, choice: KindChoice, cx: &mut Context<Self>| {
                Button::new(id)
                    .outline()
                    .label(text)
                    .selected(current == choice)
                    .on_click(cx.listener(move |form, _, _, cx| {
                        form.choice = choice;
                        cx.notify();
                    }))
            };
        v_flex()
            .gap_1()
            .child(field_label(strings.goal_field_type, muted))
            .child(
                h_flex()
                    .gap_2()
                    .child(kind(
                        "goal-volume",
                        strings.goal_type_volume,
                        KindChoice::Volume,
                        cx,
                    ))
                    .child(kind(
                        "goal-time",
                        strings.goal_type_time,
                        KindChoice::Time,
                        cx,
                    ))
                    .child(kind(
                        "goal-streak",
                        strings.goal_type_streak,
                        KindChoice::Streak,
                        cx,
                    )),
            )
    }

    /// The label of a session choice.
    fn scope_label(&self, scope: Scope) -> String {
        let strings = self.strings;
        match scope {
            Scope::All => strings.goal_all_sessions.to_owned(),
            Scope::Session(index) => self.sessions.get(index).cloned().unwrap_or_default(),
            Scope::Missing => self
                .missing_session
                .clone()
                .unwrap_or_else(|| strings.goal_paused.to_owned()),
        }
    }

    fn render_scope_row(&self, cx: &mut Context<Self>) -> impl IntoElement + use<> {
        let strings = self.strings;
        let muted = cx.theme().muted_foreground;
        let events = WcaEvent::ALL
            .iter()
            .map(|&event| (event, event.name().to_owned()))
            .collect();
        let mut scopes = vec![Scope::All];
        if self.scope == Scope::Missing {
            scopes.push(Scope::Missing);
        }
        scopes.extend((0..self.sessions.len()).map(Scope::Session));
        let scopes = scopes
            .into_iter()
            .map(|scope| (scope, self.scope_label(scope)))
            .collect();
        h_flex()
            .gap_6()
            .child(
                v_flex()
                    .flex_1()
                    .min_w_0()
                    .gap_1()
                    .child(field_label(strings.field_event, muted))
                    .child(Self::menu(
                        "goal-event",
                        self.event.name().to_owned(),
                        events,
                        self.event,
                        |form, event| form.event = event,
                        cx,
                    )),
            )
            .child(
                v_flex()
                    .flex_1()
                    .min_w_0()
                    .gap_1()
                    .child(field_label(strings.goal_field_session, muted))
                    .child(Self::menu(
                        "goal-session",
                        self.scope_label(self.scope),
                        scopes,
                        self.scope,
                        |form, scope| form.scope = scope,
                        cx,
                    )),
            )
    }

    /// One text field with its label and error.
    fn field(
        id: &'static str,
        label: &'static str,
        input: &Entity<InputState>,
        error: Option<&String>,
        cx: &App,
    ) -> Div {
        v_flex()
            .flex_1()
            .min_w_0()
            .gap_1()
            .child(field_label(label, cx.theme().muted_foreground))
            .child(keep_focus(id, Input::new(input)))
            .children(field_error(error, Palette::of(cx).red))
    }

    /// The fields of the chosen kind of goal.
    fn render_kind_fields(&self, cx: &mut Context<Self>) -> impl IntoElement + use<> {
        let strings = self.strings;
        let muted = cx.theme().muted_foreground;
        let row = h_flex().gap_6().items_start();
        match self.choice {
            KindChoice::Volume => {
                let current = self.period;
                let period = |id: &'static str,
                              text: &'static str,
                              period: Period,
                              cx: &mut Context<Self>| {
                    Button::new(id)
                        .outline()
                        .label(text)
                        .selected(current == period)
                        .on_click(cx.listener(move |form, _, _, cx| {
                            form.period = period;
                            cx.notify();
                        }))
                };
                let day = period("goal-day", strings.goal_period_day, Period::Day, cx);
                let week = period("goal-week", strings.goal_period_week, Period::Week, cx);
                row.child(Self::field(
                    "goal-solves",
                    strings.goal_field_solves,
                    &self.solves,
                    self.solves_error.as_ref(),
                    cx,
                ))
                .child(
                    v_flex()
                        .flex_1()
                        .min_w_0()
                        .gap_1()
                        .child(field_label(strings.goal_field_period, muted))
                        .child(h_flex().gap_2().child(day).child(week)),
                )
            }
            KindChoice::Time => {
                let sizes = TARGET_SIZES
                    .iter()
                    .map(|&size| (size, result_label(size, strings)))
                    .collect();
                row.child(Self::field(
                    "goal-target",
                    strings.goal_type_time,
                    &self.target,
                    self.target_error.as_ref(),
                    cx,
                ))
                .child(
                    v_flex()
                        .flex_1()
                        .min_w_0()
                        .gap_1()
                        .child(field_label(strings.goal_field_result, muted))
                        .child(Self::menu(
                            "goal-size",
                            result_label(self.size, strings),
                            sizes,
                            self.size,
                            |form, size| form.size = size,
                            cx,
                        )),
                )
            }
            KindChoice::Streak => row
                .child(Self::field(
                    "goal-days",
                    strings.goal_field_days,
                    &self.days,
                    self.days_error.as_ref(),
                    cx,
                ))
                .child(Self::field(
                    "goal-per-day",
                    strings.goal_field_solves_per_day,
                    &self.solves,
                    self.solves_error.as_ref(),
                    cx,
                )),
        }
    }
}

impl Render for GoalForm {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let background = cx.theme().background;
        let max_height = dialog_body_height(DIALOG_SIZE, window);
        dialog_body(
            FadeScroll::new("goal-form", background)
                .track_focus(&self.focus)
                .max_h(max_height)
                .gap_3()
                .on_mouse_down(
                    MouseButton::Left,
                    cx.listener(|form, _, window, cx| {
                        // Clicking the dialog's empty space moves focus off any
                        // field; presses inside fields stop before this.
                        form.focus.focus(window, cx);
                    }),
                )
                .child(self.render_kind_row(cx))
                .child(self.render_scope_row(cx))
                .child(self.render_kind_fields(cx)),
        )
    }
}
