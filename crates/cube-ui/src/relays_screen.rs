//! The Relays screen: several puzzles timed as one attempt, with a split for
//! each.
//!
//! The rules (one inspection, a key press per puzzle, penalties, totals) live
//! in `cube_core::timer` and `cube_core::relay`; this module keeps the
//! screen's state and draws it. The attempt is timed by the same timer as every
//! other solve, so holding the key and releasing it work as they do on the
//! timer screen.

use std::path::PathBuf;
use std::rc::Rc;
use std::time::{Instant, SystemTime, UNIX_EPOCH};

use cube_core::format::format_ms;
use cube_core::relay::{self, RelayAttempt, RelayData, RelayError};
use cube_core::{Action, Modifier, Readiness, TimerOutcome, TimerPhase, WcaEvent, persistence};
use gpui_kit::component::button::Button;
use gpui_kit::component::notification::Notification;
use gpui_kit::component::{ActiveTheme as _, Sizable as _, WindowExt as _, h_flex, v_flex};
use gpui_kit::prelude::FluentBuilder as _;
use gpui_kit::*;

use crate::app::{CubeApp, Screen};
use crate::multi_blind_screen::{Accept, Prompt, digits_panel, empty_message_owned};
use crate::palette::Palette;
use crate::scroll_fade::FadeScroll;

/// Height of a row, which follows the text size.
const ROW_HEIGHT: Rems = Rems(1.75);
/// Width of the column numbering the puzzles.
const NUMBER_WIDTH: Rems = Rems(2.5);
/// Width of the column naming the events.
const EVENT_WIDTH: Rems = Rems(8.);

/// Where an attempt stands.
enum Phase {
    /// Nothing under way: the attempts are showing.
    Idle,
    /// The scrambles are being prepared. `id` tells this request from an
    /// older one that was abandoned.
    Preparing { id: u64 },
    /// The scrambles are ready, and the timer starts the attempt.
    Ready { scrambles: Vec<String> },
}

/// The screen's state.
pub(crate) struct RelayState {
    data: RelayData,
    /// Where the relays are saved; `None` keeps saving off.
    path: Option<PathBuf>,
    phase: Phase,
    /// The events of the relay chosen to run.
    current: Vec<WcaEvent>,
    /// Counts the requests for scrambles.
    requests: u64,
    /// The attempt selected with the keyboard, as an index into `data`.
    selected: Option<usize>,
    /// The puzzle of the selected attempt that penalties apply to.
    puzzle: usize,
}

impl RelayState {
    pub(crate) fn new(data: RelayData, path: Option<PathBuf>) -> Self {
        let current = data
            .attempts
            .last()
            .map_or_else(|| relay::presets()[0].clone(), |last| last.events.clone());
        Self {
            data,
            path,
            phase: Phase::Idle,
            current,
            requests: 0,
            selected: None,
            puzzle: 0,
        }
    }

    /// The indexes of the attempts at the chosen relay, oldest first.
    fn attempts_of_current(&self) -> Vec<usize> {
        self.data
            .attempts
            .iter()
            .enumerate()
            .filter(|(_, attempt)| attempt.events == self.current)
            .map(|(index, _)| index)
            .collect()
    }

    /// The attempt actions apply to: the selection if it belongs to the chosen
    /// relay, or else the newest.
    fn target(&self) -> Option<usize> {
        let indexes = self.attempts_of_current();
        match self.selected {
            Some(selected) if indexes.contains(&selected) => Some(selected),
            _ => indexes.last().copied(),
        }
    }

    fn scrambles(&self) -> Option<&[String]> {
        match &self.phase {
            Phase::Ready { scrambles } => Some(scrambles),
            Phase::Idle | Phase::Preparing { .. } => None,
        }
    }
}

impl CubeApp {
    /// Handles the timer key on this screen: it asks for the scrambles of a
    /// new attempt, or starts, splits and stops the one that is ready.
    pub(crate) fn relay_timer_key(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        match self.relays.phase {
            Phase::Idle => self.prepare_relay(window, cx),
            Phase::Ready { .. } => {
                self.key_down = true;
                self.press_timer(window, cx);
            }
            Phase::Preparing { .. } => {}
        }
    }

    /// Runs a shortcut on this screen. Returns `false` for those that work the
    /// same everywhere.
    pub(crate) fn relay_action(
        &mut self,
        action: Action,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> bool {
        let idle = matches!(self.relays.phase, Phase::Idle);
        match action {
            Action::OpenSolve if idle => self.prepare_relay(window, cx),
            Action::PreviousSolve if idle => self.step_relay_attempt(true, cx),
            Action::NextSolve if idle => self.step_relay_attempt(false, cx),
            Action::PreviousAverage if idle => self.step_relay_puzzle(false, cx),
            Action::NextAverage if idle => self.step_relay_puzzle(true, cx),
            Action::PenaltyPlusTwo if idle => self.penalize_relay(Modifier::PlusTwo, window, cx),
            Action::PenaltyDnf if idle => self.penalize_relay(Modifier::DNF, window, cx),
            Action::DeleteSolve if idle => self.confirm_delete_relay_attempt(window, cx),
            Action::NextRelay if idle => self.next_relay(cx),
            Action::NewRelay if idle => self.open_new_relay(window, cx),
            Action::DeleteRelay if idle => self.delete_custom_relay(window, cx),
            Action::CopyRelayScrambles => self.copy_relay_scrambles(window, cx),
            Action::OpenSolve
            | Action::PreviousSolve
            | Action::NextSolve
            | Action::PreviousAverage
            | Action::NextAverage
            | Action::PenaltyPlusTwo
            | Action::PenaltyDnf
            | Action::DeleteSolve
            | Action::NextRelay
            | Action::NewRelay
            | Action::DeleteRelay => {}
            _ => return false,
        }
        true
    }

    /// Goes back to the attempts from scrambles that were never used.
    pub(crate) fn leave_relay_attempt(&mut self, cx: &mut Context<Self>) -> bool {
        if self.screen != Screen::Relays || matches!(self.relays.phase, Phase::Idle) {
            return false;
        }
        self.relays.phase = Phase::Idle;
        cx.notify();
        true
    }

    fn step_relay_attempt(&mut self, newer: bool, cx: &mut Context<Self>) {
        let indexes = self.relays.attempts_of_current();
        let Some(current) = self.relays.target() else {
            return;
        };
        let Some(position) = indexes.iter().position(|&index| index == current) else {
            return;
        };
        let next = if newer {
            (position + 1).min(indexes.len() - 1)
        } else {
            position.saturating_sub(1)
        };
        self.relays.selected = Some(indexes[next]);
        self.relays.puzzle = 0;
        cx.notify();
    }

    fn step_relay_puzzle(&mut self, forward: bool, cx: &mut Context<Self>) {
        let count = self.relays.current.len();
        if count == 0 {
            return;
        }
        let puzzle = self.relays.puzzle;
        self.relays.puzzle = if forward {
            (puzzle + 1) % count
        } else {
            (puzzle + count - 1) % count
        };
        cx.notify();
    }

    /// Gives the selected puzzle of the selected attempt a penalty, or takes
    /// it away when it has it already.
    fn penalize_relay(&mut self, modifier: Modifier, window: &mut Window, cx: &mut Context<Self>) {
        let Some(index) = self.relays.target() else {
            return;
        };
        if self.relays.path.is_none() {
            return;
        }
        let puzzle = self.relays.puzzle;
        if let Some(attempt) = self.relays.data.attempts.get_mut(index) {
            attempt.toggle_penalty(puzzle, modifier);
        }
        self.relays.selected = Some(index);
        self.save_relays(window, cx);
        cx.notify();
    }

    /// Chooses the next relay on offer.
    fn next_relay(&mut self, cx: &mut Context<Self>) {
        let relays = self.relays.data.relays();
        let position = relays
            .iter()
            .position(|events| *events == self.relays.current)
            .map_or(0, |position| position + 1);
        let next = position % relays.len();
        self.relays.current.clone_from(&relays[next]);
        self.relays.selected = None;
        self.relays.puzzle = 0;
        cx.notify();
    }

    /// Asks for the events of a custom relay.
    fn open_new_relay(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let strings = self.strings();
        if self.relays.path.is_none() {
            window.push_notification(Notification::warning(strings.relays_unavailable), cx);
            return;
        }
        let prompt = Prompt {
            title: strings.relays_new_title,
            label: strings.relays_new_label.to_owned(),
            default: String::new(),
            button: strings.relays_create,
        };
        let accept: Accept = Rc::new(move |app, text, window, cx| {
            let strings = app.strings();
            let events = relay::parse_events(text).map_err(|error| match error {
                RelayError::TooFew => strings
                    .relays_too_few
                    .replace("{min}", &relay::MIN_EVENTS.to_string()),
                RelayError::TooMany => strings
                    .relays_too_many
                    .replace("{max}", &relay::MAX_EVENTS.to_string()),
                RelayError::Unknown(name) => strings.relays_unknown.replace("{name}", &name),
                RelayError::Blindfolded(event) => {
                    strings.relays_blindfolded.replace("{name}", event.name())
                }
            })?;
            if !app.relays.data.relays().contains(&events) {
                app.relays.data.custom.push(events.clone());
                app.save_relays(window, cx);
            }
            app.relays.current = events;
            app.relays.selected = None;
            app.relays.puzzle = 0;
            cx.notify();
            Ok(())
        });
        self.open_number_prompt(prompt, accept, window, cx);
    }

    /// Removes the chosen relay from the custom ones, keeping its attempts.
    fn delete_custom_relay(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let current = self.relays.current.clone();
        let Some(position) = self
            .relays
            .data
            .custom
            .iter()
            .position(|events| *events == current)
        else {
            return;
        };
        if self.relays.path.is_none() {
            return;
        }
        self.relays.data.custom.remove(position);
        self.save_relays(window, cx);
        // The attempts stay, so the relay stays chosen only while it is offered.
        if !self.relays.data.relays().contains(&current) {
            self.next_relay(cx);
        }
        cx.notify();
    }

    /// Starts preparing the scrambles of a new attempt.
    fn prepare_relay(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let strings = self.strings();
        if self.relays.path.is_none() {
            window.push_notification(Notification::warning(strings.relays_unavailable), cx);
            return;
        }
        self.relays.requests += 1;
        let id = self.relays.requests;
        let events = self.relays.current.clone();
        self.relays.phase = Phase::Preparing { id };
        cx.spawn_in(window, async move |this, cx| {
            let scrambles = cx
                .background_executor()
                .spawn(async move { relay::generate_scrambles(&events) })
                .await;
            this.update(cx, |app, cx| {
                // Only the latest request counts: this one may have been left.
                if matches!(app.relays.phase, Phase::Preparing { id: current } if current == id) {
                    app.relays.phase = Phase::Ready { scrambles };
                    cx.notify();
                }
            })
            .ok();
        })
        .detach();
        cx.notify();
    }

    /// The number of puzzles the next attempt has, for the timer.
    pub(crate) fn relay_puzzles(&self) -> usize {
        self.relays.current.len()
    }

    /// Records the attempt that just ended.
    pub(crate) fn finish_relay_attempt(
        &mut self,
        outcome: TimerOutcome,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let phase = std::mem::replace(&mut self.relays.phase, Phase::Idle);
        let Phase::Ready { scrambles } = phase else {
            self.relays.phase = phase;
            return;
        };
        let splits = self.timer.splits().durations(outcome.time_ms);
        let solved_at_unix_ms = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map_or(0, |elapsed| elapsed.as_millis() as u64);
        self.relays.data.attempts.push(RelayAttempt::new(
            self.relays.current.clone(),
            scrambles,
            splits,
            outcome.modifier,
            solved_at_unix_ms,
        ));
        self.relays.selected = Some(self.relays.data.attempts.len() - 1);
        self.relays.puzzle = 0;
        self.save_relays(window, cx);
        cx.notify();
    }

    /// Writes the relays to disk. Without a path, which means the file on
    /// disk could not be read, nothing is written over it.
    fn save_relays(&self, window: &mut Window, cx: &mut Context<Self>) {
        if let Some(path) = &self.relays.path
            && let Err(error) = persistence::save_relays(path, &self.relays.data)
        {
            let template = self.strings().toast_relay_save_failed;
            self.report_error(template, &error, window, cx);
        }
    }

    fn confirm_delete_relay_attempt(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let Some(index) = self.relays.target() else {
            return;
        };
        if !self.settings.confirm_delete {
            self.delete_relay_attempt(index, window, cx);
            return;
        }
        let strings = self.strings();
        let on_confirm = cx.listener(move |app: &mut Self, (): &(), window, cx| {
            app.delete_relay_attempt(index, window, cx);
            app.focus.focus(window, cx);
        });
        let message = strings.trainer_delete_confirm.to_owned();
        self.open_confirm(strings.delete_solve, message, on_confirm, window, cx);
    }

    fn delete_relay_attempt(&mut self, index: usize, window: &mut Window, cx: &mut Context<Self>) {
        if index >= self.relays.data.attempts.len() || self.relays.path.is_none() {
            return;
        }
        self.relays.data.attempts.remove(index);
        self.relays.selected = None;
        self.relays.puzzle = 0;
        self.save_relays(window, cx);
        cx.notify();
    }

    /// Copies the scrambles of the attempt under way.
    pub(crate) fn copy_relay_scrambles(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if self.screen != Screen::Relays {
            return;
        }
        let Some(scrambles) = self.relays.scrambles() else {
            return;
        };
        let text = relay::scrambles_text(&self.relays.current, scrambles);
        cx.write_to_clipboard(ClipboardItem::new_string(text));
        self.toast(self.strings().relays_scrambles_copied, window, cx);
    }

    /// A time with its penalty, as shown in a list.
    fn relay_split_text(attempt: &RelayAttempt, index: usize) -> String {
        let marker = match attempt.modifiers.get(index).copied().unwrap_or_default() {
            Modifier::None => "",
            Modifier::PlusTwo => "+",
            Modifier::DNF => "DNF ",
        };
        match (attempt.split_ms(index), attempt.splits_ms.get(index)) {
            (Some(ms), _) => format!("{}{}", format_ms(ms), marker),
            (None, Some(&raw)) => format!("{marker}({})", format_ms(raw)),
            (None, None) => "–".to_owned(),
        }
    }

    /// The text for a whole attempt: the total and every split.
    fn relay_attempt_text(attempt: &RelayAttempt) -> String {
        let total = attempt
            .total_ms()
            .map_or_else(|| "DNF".to_owned(), format_ms);
        let splits: Vec<String> = (0..attempt.puzzles())
            .map(|index| Self::relay_split_text(attempt, index))
            .collect();
        format!("{total} · {}", splits.join(" / "))
    }

    /// The line summarizing the chosen relay.
    fn relay_summary_text(&self) -> String {
        let summary = self.relays.data.summary(&self.relays.current);
        let show = |ms: Option<u64>| ms.map_or_else(|| "–".to_owned(), format_ms);
        self.strings()
            .relays_summary
            .replace("{relay}", &relay::name(&self.relays.current))
            .replace("{attempts}", &summary.attempts.to_string())
            .replace("{best}", &show(summary.best))
            .replace("{mean}", &show(summary.mean))
    }

    /// The Relays screen.
    pub(crate) fn render_relays(&self, window: &Window, cx: &mut Context<Self>) -> AnyElement {
        let strings = self.strings();
        let theme = cx.theme();
        let (background, muted) = (theme.background, theme.muted_foreground);
        let timer_key = self
            .keybinds()
            .label(Action::Timer)
            .unwrap_or_else(|| strings.unassigned.to_owned());
        let key_hint = |text: &str| text.replace("{key}", &timer_key);

        let body = match &self.relays.phase {
            Phase::Idle => {
                let hint = key_hint(strings.relays_start_hint)
                    .replace("{relay}", &relay::name(&self.relays.current));
                self.render_relay_attempts(&hint, cx)
            }
            Phase::Preparing { .. } => empty_message_owned(
                strings
                    .relays_preparing
                    .replace("{count}", &self.relays.current.len().to_string()),
                muted,
            ),
            Phase::Ready { scrambles } => {
                if self.timer.is_active() {
                    self.render_relay_running(scrambles, window, cx)
                } else {
                    self.render_relay_scrambles(
                        scrambles,
                        &key_hint(strings.relays_ready),
                        background,
                        cx,
                    )
                }
            }
        };

        v_flex()
            .flex_1()
            .min_h_0()
            .px_6()
            .py_4()
            .gap_2()
            .child(
                div()
                    .flex_none()
                    .text_lg()
                    .font_weight(FontWeight::SEMIBOLD)
                    .child(strings.nav_relays),
            )
            .child(
                div()
                    .flex_none()
                    .text_sm()
                    .text_color(muted)
                    .child(self.relay_summary_text()),
            )
            .child(body)
            .into_any_element()
    }

    /// The attempts at the chosen relay, newest first, with the splits of the
    /// selected one.
    #[allow(clippy::too_many_lines, reason = "one list and one detail view")]
    fn render_relay_attempts(&self, hint: &str, cx: &mut Context<Self>) -> AnyElement {
        let strings = self.strings();
        let theme = cx.theme();
        let (background, muted, accent) = (theme.background, theme.muted_foreground, theme.accent);
        let mono = theme.mono_font_family.clone();
        let colors = Palette::of(cx);
        let language = self.language();
        let target = self.relays.target();
        let indexes = self.relays.attempts_of_current();

        let list: AnyElement = if indexes.is_empty() {
            let message = if self.relays.path.is_some() {
                strings.relays_empty
            } else {
                strings.relays_unavailable
            };
            crate::stats_screen::empty_message(message, muted)
        } else {
            FadeScroll::new("relay-attempts", background)
                .flex_1()
                .min_h_0()
                .children(indexes.iter().rev().enumerate().map(|(row, &index)| {
                    let attempt = &self.relays.data.attempts[index];
                    let number = indexes.len() - row;
                    h_flex()
                        .id(("relay-attempt", index))
                        .flex_none()
                        .h(ROW_HEIGHT)
                        .px_3()
                        .gap_3()
                        .rounded_sm()
                        .text_sm()
                        .when(target == Some(index), |row| row.bg(accent))
                        .on_click(cx.listener(move |app, _, window, cx| {
                            app.relays.selected = Some(index);
                            app.relays.puzzle = 0;
                            app.focus.focus(window, cx);
                            cx.notify();
                        }))
                        .child(
                            div()
                                .flex_none()
                                .w(NUMBER_WIDTH)
                                .text_right()
                                .text_color(muted)
                                .font_family(mono.clone())
                                .child(number.to_string()),
                        )
                        .child(
                            div()
                                .flex_1()
                                .min_w_0()
                                .truncate()
                                .font_family(mono.clone())
                                .child(Self::relay_attempt_text(attempt)),
                        )
                        .child(
                            div()
                                .flex_none()
                                .whitespace_nowrap()
                                .text_color(muted)
                                .child(language.format_date(attempt.solved_at_unix_ms)),
                        )
                        .into_any_element()
                }))
                .into_any_element()
        };

        let detail = target
            .and_then(|index| self.relays.data.attempts.get(index))
            .map(|attempt| {
                let total = attempt
                    .total_ms()
                    .map_or_else(|| "DNF".to_owned(), format_ms);
                v_flex()
                    .flex_none()
                    .child(
                        h_flex()
                            .h(ROW_HEIGHT)
                            .px_3()
                            .gap_3()
                            .text_sm()
                            .font_weight(FontWeight::SEMIBOLD)
                            .child(div().flex_none().w(NUMBER_WIDTH))
                            .child(
                                div()
                                    .flex_none()
                                    .w(EVENT_WIDTH)
                                    .whitespace_nowrap()
                                    .child(strings.relays_total),
                            )
                            .child(
                                div()
                                    .font_family(mono.clone())
                                    .whitespace_nowrap()
                                    .child(total),
                            ),
                    )
                    .children((0..attempt.puzzles()).map(|puzzle| {
                        let penalized = attempt.modifiers[puzzle] != Modifier::None;
                        h_flex()
                            .h(ROW_HEIGHT)
                            .px_3()
                            .gap_3()
                            .rounded_sm()
                            .text_sm()
                            .when(self.relays.puzzle == puzzle, |row| row.bg(accent))
                            .child(
                                div()
                                    .flex_none()
                                    .w(NUMBER_WIDTH)
                                    .text_right()
                                    .text_color(muted)
                                    .child((puzzle + 1).to_string()),
                            )
                            .child(
                                div()
                                    .flex_none()
                                    .w(EVENT_WIDTH)
                                    .truncate()
                                    .child(attempt.events[puzzle].name()),
                            )
                            .child(
                                div()
                                    .font_family(mono.clone())
                                    .whitespace_nowrap()
                                    .when(penalized, |cell| cell.text_color(colors.yellow))
                                    .child(Self::relay_split_text(attempt, puzzle)),
                            )
                            .into_any_element()
                    }))
            });

        v_flex()
            .flex_1()
            .min_h_0()
            .gap_2()
            .child(
                div()
                    .flex_none()
                    .text_sm()
                    .text_color(muted)
                    .child(hint.to_owned()),
            )
            .child(
                div()
                    .flex_none()
                    .text_sm()
                    .text_color(muted)
                    .child(strings.relays_penalty_hint),
            )
            .children(detail)
            .child(list)
            .into_any_element()
    }

    /// The scrambles of every puzzle, shown before the attempt starts.
    fn render_relay_scrambles(
        &self,
        scrambles: &[String],
        hint: &str,
        background: Hsla,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        let strings = self.strings();
        let theme = cx.theme();
        let muted = theme.muted_foreground;
        let mono = theme.mono_font_family.clone();
        let scramble_color = Palette::of(cx).scramble;
        let events = self.relays.current.clone();

        v_flex()
            .flex_1()
            .min_h_0()
            .gap_2()
            .child(
                h_flex()
                    .flex_none()
                    .justify_between()
                    .gap_2()
                    .child(div().text_sm().text_color(muted).child(hint.to_owned()))
                    .child(
                        Button::new("relay-copy")
                            .outline()
                            .small()
                            .label(strings.action_copy_relay_scrambles)
                            .tab_stop(false)
                            .on_click(cx.listener(|app, _, window, cx| {
                                app.copy_relay_scrambles(window, cx);
                                app.focus.focus(window, cx);
                            })),
                    ),
            )
            .child(
                FadeScroll::new("relay-scrambles", background)
                    .flex_1()
                    .min_h_0()
                    .gap_1()
                    .font_family(mono)
                    .text_color(scramble_color)
                    .children(scrambles.iter().enumerate().map(|(index, scramble)| {
                        h_flex()
                            .flex_none()
                            .gap_3()
                            .items_start()
                            .child(
                                div()
                                    .flex_none()
                                    .w(NUMBER_WIDTH)
                                    .text_right()
                                    .text_color(muted)
                                    .child((index + 1).to_string()),
                            )
                            .child(
                                div()
                                    .flex_none()
                                    .w(EVENT_WIDTH)
                                    .truncate()
                                    .text_color(muted)
                                    .child(events[index].name()),
                            )
                            .child(div().flex_1().min_w_0().child(scramble.clone()))
                            .into_any_element()
                    })),
            )
            .into_any_element()
    }

    /// The digits while the key is held, in inspection or running, with the
    /// scramble of the puzzle being solved and the splits so far.
    #[allow(clippy::too_many_lines, reason = "the digits and what surrounds them")]
    fn render_relay_running(
        &self,
        scrambles: &[String],
        window: &Window,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        let strings = self.strings();
        let colors = Palette::of(cx);
        let theme = cx.theme();
        let (foreground, muted) = (theme.foreground, theme.muted_foreground);
        let mono = theme.mono_font_family.clone();
        let now = Instant::now();
        let timer_key = self
            .keybinds()
            .label(Action::Timer)
            .unwrap_or_else(|| strings.unassigned.to_owned());
        let splits = *self.timer.splits();
        let current = splits.current().min(self.relays.current.len() - 1);
        let running_hint = strings
            .relays_running
            .replace("{current}", &(current + 1).to_string())
            .replace("{total}", &self.relays.current.len().to_string())
            .replace("{event}", self.relays.current[current].name())
            .replace("{key}", &timer_key);

        let (text, color, hint) = match self.timer.phase() {
            TimerPhase::Running { .. } => (
                format_ms(self.timer.elapsed(now).as_millis() as u64),
                foreground,
                running_hint,
            ),
            TimerPhase::Inspecting { .. } => {
                let elapsed = self.timer.elapsed(now);
                let modifier = self.timer.inspection_modifier(elapsed);
                let text = match modifier {
                    Modifier::None => elapsed.as_secs().to_string(),
                    Modifier::PlusTwo => "+2".to_owned(),
                    Modifier::DNF => "DNF".to_owned(),
                };
                let color = if modifier == Modifier::None {
                    colors.orange
                } else {
                    colors.yellow
                };
                (
                    text,
                    color,
                    strings.hint_inspecting.replace("{key}", &timer_key),
                )
            }
            _ => match self.timer.readiness(now) {
                Readiness::Waiting => (
                    format_ms(0),
                    colors.red,
                    strings.hint_hold.replace("{key}", &timer_key),
                ),
                Readiness::Ready => (
                    format_ms(0),
                    colors.green,
                    strings.hint_release.replace("{key}", &timer_key),
                ),
                Readiness::Released => (format_ms(0), foreground, running_hint),
            },
        };

        let elapsed_ms = self.timer.elapsed(now).as_millis() as u64;
        let done = if matches!(self.timer.phase(), TimerPhase::Running { .. }) {
            splits.durations(elapsed_ms)
        } else {
            Vec::new()
        };
        let scramble = scrambles.get(current).cloned().unwrap_or_default();
        let scramble_color = colors.scramble;
        v_flex()
            .flex_1()
            .min_h_0()
            .items_center()
            .justify_center()
            .gap_3()
            .child(digits_panel(
                text,
                color,
                &hint,
                muted,
                mono.clone(),
                window,
            ))
            .child(
                div()
                    .max_w_full()
                    .px_6()
                    .text_center()
                    .font_family(mono.clone())
                    .text_color(scramble_color)
                    .child(scramble),
            )
            .child(
                v_flex()
                    .items_center()
                    .text_sm()
                    .text_color(muted)
                    .font_family(mono)
                    .children(
                        done.iter()
                            .take(splits.marked())
                            .enumerate()
                            .map(|(index, &ms)| {
                                div().child(format!(
                                    "{} {}",
                                    self.relays.current[index].name(),
                                    format_ms(ms)
                                ))
                            }),
                    ),
            )
            .into_any_element()
    }
}
