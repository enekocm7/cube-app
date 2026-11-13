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

use cube_core::format::{format_ms, parse_time};
use cube_core::relay::{self, RelayAttempt, RelayData, RelayError};
use cube_core::{Action, Modifier, Readiness, TimerOutcome, TimerPhase, WcaEvent, persistence};
use cube_locale::Strings;
use gpui_kit::assets::IconName;
use gpui_kit::component::button::{Button, ButtonVariants as _};
use gpui_kit::component::input::{Input, InputEvent, InputState};
use gpui_kit::component::menu::{DropdownMenu as _, PopupMenuItem};
use gpui_kit::component::notification::Notification;
use gpui_kit::component::{
    ActiveTheme as _, Icon, Selectable as _, Sizable as _, WindowExt as _, h_flex, v_flex,
};
use gpui_kit::prelude::FluentBuilder as _;
use gpui_kit::*;

use crate::app::{CubeApp, Screen};
use crate::details::{field_error, keep_focus};
use crate::dialog_size::{DialogSize, dialog_body, dialog_body_height, dialog_title, dialog_width};
use crate::dialog_submit::{DialogSubmit as _, Submit, on_press};
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

/// The form of the dialog that edits a saved attempt: the split of each
/// puzzle and its penalty. The total is the sum of the splits, so it follows
/// what is typed.
struct RelayEditForm {
    events: Vec<WcaEvent>,
    splits: Vec<Entity<InputState>>,
    modifiers: Vec<Modifier>,
    error: Option<String>,
    strings: &'static cube_locale::Strings,
}

impl RelayEditForm {
    /// The total of the splits as they are typed, `DNF` when a puzzle is one,
    /// or `–` while a split is not a time.
    fn total_text(&self, cx: &App) -> String {
        let mut splits_ms = Vec::with_capacity(self.splits.len());
        for input in &self.splits {
            let text = input.read(cx).value().to_string();
            let Some(ms) = parse_time(&text).filter(|&ms| ms > 0) else {
                return "–".to_owned();
            };
            splits_ms.push(ms);
        }
        relay::total_of(&splits_ms, &self.modifiers).map_or_else(|| "DNF".to_owned(), format_ms)
    }
}

impl Render for RelayEditForm {
    fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let strings = self.strings;
        let theme = cx.theme();
        let (muted, border) = (theme.muted_foreground, theme.border);
        let mono = theme.mono_font_family.clone();
        let red = Palette::of(cx).red;
        let total = self.total_text(cx);

        let rows = (0..self.events.len()).map(|index| {
            let event = self.events[index];
            let modifier = self.modifiers[index];
            let penalty = |id: (&'static str, usize), text: &'static str, value: Modifier| {
                Button::new(id)
                    .outline()
                    .xsmall()
                    .label(text)
                    .selected(modifier == value)
                    .accessibility_label(format!("{}: {}", event.name(), text))
                    .on_click(cx.listener(move |form, _, _, cx| {
                        form.modifiers[index] = value;
                        form.error = None;
                        cx.notify();
                    }))
            };
            h_flex()
                .id(("relay-edit-row", index))
                .flex_none()
                .items_center()
                .gap_2()
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
                        .flex_1()
                        .min_w_0()
                        .truncate()
                        .whitespace_nowrap()
                        .child(event.name()),
                )
                .child(div().flex_none().w(Rems(9.)).child(keep_focus(
                    ("relay-edit-split", index),
                    Input::new(&self.splits[index]).focus_bordered(false),
                )))
                .child(penalty(
                    ("relay-edit-none", index),
                    strings.penalty_none,
                    Modifier::None,
                ))
                .child(penalty(
                    ("relay-edit-plus-two", index),
                    "+2",
                    Modifier::PlusTwo,
                ))
                .child(penalty(("relay-edit-dnf", index), "DNF", Modifier::DNF))
                .into_any_element()
        });

        v_flex()
            .gap_2()
            .py_2()
            .children(rows)
            .child(
                h_flex()
                    .flex_none()
                    .justify_between()
                    .gap_3()
                    .pt_2()
                    .border_t_1()
                    .border_color(border)
                    .child(
                        div()
                            .text_sm()
                            .text_color(muted)
                            .child(strings.relays_total),
                    )
                    .child(div().font_family(mono).child(total)),
            )
            .children(field_error(self.error.as_ref(), red))
    }
}

impl CubeApp {
    /// Returns whether the scrambles of an attempt are ready to be timed.
    pub(crate) fn relays_ready(&self) -> bool {
        matches!(self.relays.phase, Phase::Ready { .. })
    }

    /// Handles the timer key on this screen: it asks for the scrambles of a
    /// new attempt, or starts, splits and stops the one that is ready.
    pub(crate) fn relay_timer_key(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        match self.relays.phase {
            Phase::Idle => self.prepare_relay(window, cx),
            Phase::Ready { .. } => {
                // A connected hardware timer starts and stops the attempt
                // itself, so the key leaves it alone.
                if self.hardware_timing() {
                    if !self.timer.is_active() {
                        self.toast_warning(self.strings().bluetooth_blocks_start, window, cx);
                    }
                    return;
                }
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
            // As on the timer, Enter opens the selected attempt, or the
            // newest; the timer key starts a new one.
            Action::OpenSolve if idle => match self.relays.target() {
                Some(index) => self.open_edit_relay(index, window, cx),
                None => self.prepare_relay(window, cx),
            },
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
            Action::ToggleRelayKeySplits => self.toggle_relay_key_splits(window, cx),
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
    pub(crate) fn open_new_relay(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let strings = self.strings();
        if self.relays.path.is_none() {
            crate::toasts::push(
                Notification::warning(strings.relays_unavailable),
                window,
                cx,
            );
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
            app.prepare_relay(window, cx);
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
            crate::toasts::push(
                Notification::warning(strings.relays_unavailable),
                window,
                cx,
            );
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
        let solved_at_unix_ms = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map_or(0, |elapsed| elapsed.as_millis() as u64);
        // A hardware timer reports the total only, and the key that passes a
        // puzzle marks no time, so the attempt keeps just its total.
        let attempt = if self.hardware_attempt {
            RelayAttempt::timed_by_hardware(
                self.relays.current.clone(),
                scrambles,
                outcome.time_ms,
                solved_at_unix_ms,
            )
        } else {
            let splits = self.timer.splits().durations(outcome.time_ms);
            RelayAttempt::new(
                self.relays.current.clone(),
                scrambles,
                splits,
                outcome.modifier,
                solved_at_unix_ms,
            )
        };
        self.relays.data.attempts.push(attempt);
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

    /// Opens the dialog that edits a saved attempt: the split of each puzzle
    /// and its penalty.
    #[allow(clippy::too_many_lines, reason = "the fields, the form and its dialog")]
    fn open_edit_relay(&mut self, index: usize, window: &mut Window, cx: &mut Context<Self>) {
        let strings = self.strings();
        if self.relays.path.is_none() {
            crate::toasts::push(
                Notification::warning(strings.relays_unavailable),
                window,
                cx,
            );
            return;
        }
        let Some(attempt) = self.relays.data.attempts.get(index) else {
            return;
        };
        let events = attempt.events.clone();
        let modifiers = attempt.modifiers.clone();
        // An attempt with only a total has no split to fill in, so each field
        // starts empty and may stay that way.
        let splits: Vec<Entity<InputState>> = (0..attempt.puzzles())
            .map(|index| {
                let text = attempt
                    .splits_ms
                    .get(index)
                    .map_or_else(String::new, |&ms| format_ms(ms));
                cx.new(|cx| InputState::new(window, cx).default_value(text))
            })
            .collect();
        let first = splits.first().cloned();
        let form_events = events.clone();
        let form_modifiers = modifiers.clone();
        let form_splits = splits.clone();
        self.relays.selected = Some(index);
        self.relays.puzzle = 0;
        let view = cx.new(|cx| {
            for input in &splits {
                cx.subscribe_in(
                    input,
                    window,
                    |form: &mut RelayEditForm, _, event: &InputEvent, _, cx| {
                        if matches!(event, InputEvent::Change) {
                            form.error = None;
                            cx.notify();
                        }
                    },
                )
                .detach();
            }
            RelayEditForm {
                events: form_events,
                splits: form_splits,
                modifiers: form_modifiers,
                error: None,
                strings,
            }
        });
        let app = cx.entity().downgrade();
        let submitted = view.clone();
        let first_build = std::cell::Cell::new(true);
        window.open_dialog(cx, move |dialog, window, cx| {
            if first_build.replace(false)
                && let Some(first) = &first
            {
                first.focus_handle(cx).focus(window, cx);
            }
            let (app, form) = (app.clone(), submitted.clone());
            let splits = splits.clone();
            let submit: Submit = Rc::new(move |window, cx| {
                let texts: Vec<String> = splits
                    .iter()
                    .map(|input| input.read(cx).value().to_string())
                    .collect();
                let modifiers = form.read(cx).modifiers.clone();
                let outcome = app
                    .update(cx, |app, cx| {
                        app.apply_relay_edit(index, &texts, &modifiers, window, cx)
                    })
                    .unwrap_or(Ok(()));
                match outcome {
                    Ok(()) => window.close_dialog(cx),
                    Err(message) => form.update(cx, |form, cx| {
                        form.error = Some(message);
                        cx.notify();
                    }),
                }
            });
            let background = cx.theme().background;
            dialog
                .title(dialog_title(strings.relays_edit_title, strings.close))
                .close_button(false)
                .w(dialog_width(DialogSize::FORM, window))
                .submit_on_enter(&submit)
                .child(dialog_body(
                    FadeScroll::new("relay-edit", background)
                        .max_h(dialog_body_height(DialogSize::FORM, window))
                        .child(view.clone()),
                ))
                .footer(
                    h_flex()
                        .w_full()
                        .justify_end()
                        .gap_2()
                        .child(
                            Button::new("relay-edit-cancel")
                                .outline()
                                .label(strings.cancel)
                                .on_click(|_, window, cx| window.close_dialog(cx)),
                        )
                        .child(
                            Button::new("relay-edit-save")
                                .primary()
                                .label(strings.save)
                                .on_click(on_press(&submit)),
                        ),
                )
        });
    }

    /// Checks the edited splits and penalties and, if they hold, replaces
    /// those of the attempt. The message says what to fix otherwise.
    fn apply_relay_edit(
        &mut self,
        index: usize,
        texts: &[String],
        modifiers: &[Modifier],
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> Result<(), String> {
        let strings = self.strings();
        if let Some(attempt) = self.relays.data.attempts.get_mut(index) {
            attempt.edit(texts, modifiers).map_err(|puzzle| {
                strings
                    .relays_edit_invalid_split
                    .replace("{puzzle}", &(puzzle + 1).to_string())
            })?;
        }
        self.save_relays(window, cx);
        cx.notify();
        Ok(())
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

    /// The time of a puzzle with its penalty, or that the timer kept no time
    /// for it.
    fn relay_split_text(strings: &Strings, attempt: &RelayAttempt, index: usize) -> String {
        let raw = attempt.splits_ms.get(index).copied().unwrap_or_default();
        // A split of zero means the timer kept no time for the puzzle, as on
        // an attempt a hardware timer timed or one passed the instant it
        // started.
        if !attempt.has_splits() || raw == 0 {
            return strings.relays_no_record.to_owned();
        }
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

    /// The text for a whole attempt in the list: the relay it is and its
    /// total.
    fn relay_attempt_text(attempt: &RelayAttempt) -> String {
        let total = attempt
            .total_ms()
            .map_or_else(|| "DNF".to_owned(), format_ms);
        format!("{} · {total}", relay::name(&attempt.events))
    }

    /// The attempts, best and mean of the chosen relay, as small cards.
    fn render_relay_stats(&self, cx: &mut Context<Self>) -> impl IntoElement + use<> {
        let strings = self.strings();
        let theme = cx.theme();
        let (border, muted) = (theme.border, theme.muted_foreground);
        let mono = theme.mono_font_family.clone();
        let summary = self.relays.data.summary(&self.relays.current);
        let show = |ms: Option<u64>| ms.map_or_else(|| "–".to_owned(), format_ms);
        let card = |label: &'static str, value: String| {
            v_flex()
                .flex_1()
                .min_w_0()
                .px_3()
                .py_2()
                .gap_1()
                .border_1()
                .border_color(border)
                .rounded_md()
                .child(
                    div()
                        .text_xs()
                        .text_color(muted)
                        .whitespace_nowrap()
                        .child(label),
                )
                .child(
                    div()
                        .text_lg()
                        .font_family(mono.clone())
                        .font_weight(FontWeight::SEMIBOLD)
                        .truncate()
                        .child(value),
                )
        };
        h_flex()
            .flex_none()
            .gap_2()
            .child(card(strings.stat_solves, summary.attempts.to_string()))
            .child(card(strings.stat_single, show(summary.best)))
            .child(card(strings.stat_mean, show(summary.mean)))
    }

    /// The relay picker and the button that creates a relay.
    fn render_relay_controls(&self, cx: &mut Context<Self>) -> impl IntoElement + use<> {
        let strings = self.strings();
        let entity = cx.entity().downgrade();
        let current = self.relays.current.clone();
        let relays = self.relays.data.relays();
        let custom_label = strings.action_new_relay;
        h_flex()
            .flex_none()
            .gap_2()
            .child(self.render_relay_mode_button(cx))
            .child(
                Button::new("relay-select")
                    .outline()
                    .small()
                    .child(
                        h_flex()
                            .gap_1()
                            .items_center()
                            .child(
                                div()
                                    .max_w(Rems(16.))
                                    .truncate()
                                    .child(relay::name(&current)),
                            )
                            .child(Icon::new(IconName::ChevronDown).size(px(14.))),
                    )
                    .tab_stop(false)
                    .dropdown_menu(move |menu, window, _| {
                        let mut menu = menu
                            .scrollable(true)
                            .max_h(window.viewport_size().height * 0.6);
                        for events in &relays {
                            let entity = entity.clone();
                            let chosen = events.clone();
                            menu = menu.item(
                                PopupMenuItem::new(relay::name(events))
                                    .checked(*events == current)
                                    .on_click(move |_, window, cx| {
                                        let chosen = chosen.clone();
                                        entity
                                            .update(cx, |app, cx| {
                                                app.choose_relay(chosen, cx);
                                                app.focus.focus(window, cx);
                                            })
                                            .ok();
                                    }),
                            );
                        }
                        let custom = entity.clone();
                        menu.separator()
                            .item(PopupMenuItem::new(custom_label).on_click(
                                move |_, window, cx| {
                                    custom
                                        .update(cx, |app, cx| app.open_new_relay(window, cx))
                                        .ok();
                                },
                            ))
                    }),
            )
            .child(
                Button::new("relay-new")
                    .primary()
                    .small()
                    .icon(IconName::Plus)
                    .label(strings.relays_new_title)
                    .tab_stop(false)
                    .on_click(cx.listener(|app, _, window, cx| {
                        app.prepare_relay(window, cx);
                        app.focus.focus(window, cx);
                    })),
            )
    }

    /// Chooses the relay to run.
    fn choose_relay(&mut self, events: Vec<WcaEvent>, cx: &mut Context<Self>) {
        if matches!(self.relays.phase, Phase::Idle) {
            self.relays.current = events;
            self.relays.selected = None;
            self.relays.puzzle = 0;
            cx.notify();
        }
    }

    /// The Relays screen.
    pub(crate) fn render_relays(&self, window: &Window, cx: &mut Context<Self>) -> AnyElement {
        let strings = self.strings();
        let theme = cx.theme();
        let (background, muted) = (theme.background, theme.muted_foreground);
        let body = match &self.relays.phase {
            Phase::Idle => self.render_relay_attempts(cx),
            Phase::Preparing { .. } => empty_message_owned(
                strings
                    .relays_preparing
                    .replace("{count}", &self.relays.current.len().to_string()),
                muted,
            ),
            Phase::Ready { scrambles } => {
                self.render_relay_attempt_view(scrambles, background, window, cx)
            }
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
                    .justify_between()
                    .items_center()
                    .gap_2()
                    .child(
                        div()
                            .text_lg()
                            .font_weight(FontWeight::SEMIBOLD)
                            .child(strings.nav_relays),
                    )
                    .when(matches!(self.relays.phase, Phase::Idle), |row| {
                        row.child(self.render_relay_controls(cx))
                    }),
            )
            .when(matches!(self.relays.phase, Phase::Idle), |column| {
                column.child(self.render_relay_stats(cx))
            })
            .child(body)
            .into_any_element()
    }

    /// The buttons that edit or delete an attempt, for the mouse; the keyboard
    /// has `Enter` and the delete key.
    fn render_relay_row_buttons(
        &self,
        index: usize,
        cx: &mut Context<Self>,
    ) -> impl IntoElement + use<> {
        let strings = self.strings();
        h_flex()
            .flex_none()
            .child(
                Button::new(("relay-edit", index))
                    .ghost()
                    .xsmall()
                    .icon(IconName::Pencil)
                    .tooltip(strings.relays_edit_title)
                    .accessibility_label(strings.relays_edit_title)
                    .tab_stop(false)
                    .on_click(cx.listener(move |app, _, window, cx| {
                        app.open_edit_relay(index, window, cx);
                    })),
            )
            .child(
                Button::new(("relay-delete", index))
                    .ghost()
                    .xsmall()
                    .icon(IconName::Trash)
                    .tooltip(strings.delete_solve)
                    .accessibility_label(strings.delete_solve)
                    .tab_stop(false)
                    .on_click(cx.listener(move |app, _, window, cx| {
                        app.relays.selected = Some(index);
                        app.confirm_delete_relay_attempt(window, cx);
                    })),
            )
    }

    /// The attempts at the chosen relay, newest first, each with the relay's
    /// name and total, and the time of each puzzle of the selected one.
    #[allow(clippy::too_many_lines, reason = "one list and one detail view")]
    fn render_relay_attempts(&self, cx: &mut Context<Self>) -> AnyElement {
        let strings = self.strings();
        let theme = cx.theme();
        let (background, muted, accent) = (theme.background, theme.muted_foreground, theme.accent);
        let border = theme.border;
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
                        .child(self.render_relay_row_buttons(index, cx))
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
                    .p_1()
                    .border_1()
                    .border_color(border)
                    .rounded_md()
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
                    // Every puzzle gets a row; one a hardware timer did not
                    // time says so instead of showing a time.
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
                                    .child(Self::relay_split_text(strings, attempt, puzzle)),
                            )
                            .into_any_element()
                    }))
            });

        v_flex()
            .flex_1()
            .min_h_0()
            .gap_2()
            .children(detail)
            .child(list)
            .into_any_element()
    }

    /// The button that switches what the timer key does during a relay.
    fn render_relay_mode_button(&self, cx: &mut Context<Self>) -> impl IntoElement + use<> {
        let strings = self.strings();
        let timer_key = self
            .keybinds()
            .label(Action::Timer)
            .unwrap_or_else(|| strings.unassigned.to_owned());
        let label = if self.settings.relay_key_splits {
            strings.relays_key_splits
        } else {
            strings.relays_key_stops
        }
        .replace("{key}", &timer_key);
        Button::new("relay-key-mode")
            .ghost()
            .small()
            .label(label)
            .tooltip(strings.action_toggle_relay_key_splits)
            .tab_stop(false)
            .on_click(cx.listener(|app, _, window, cx| {
                app.toggle_relay_key_splits(window, cx);
                app.focus.focus(window, cx);
            }))
    }

    /// Switches the timer key between stopping the relay and ending each
    /// puzzle, saving the setting.
    fn toggle_relay_key_splits(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if !self.timer.is_active() {
            self.update_settings(window, cx, |settings| {
                settings.relay_key_splits = !settings.relay_key_splits;
            });
        }
    }

    /// The scrambles of every puzzle on top and the timer under them, as on
    /// the timer screen. The same view serves before, during and after the
    /// start, so the key starts the relay from where it is.
    #[allow(clippy::too_many_lines, reason = "the scrambles and the digits")]
    fn render_relay_attempt_view(
        &self,
        scrambles: &[String],
        background: Hsla,
        window: &Window,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        let strings = self.strings();
        let colors = Palette::of(cx);
        let theme = cx.theme();
        let (foreground, muted, accent) = (theme.foreground, theme.muted_foreground, theme.accent);
        let mono = theme.mono_font_family.clone();
        let now = Instant::now();
        let events = &self.relays.current;
        let unassigned = || strings.unassigned.to_owned();
        let timer_key = self
            .keybinds()
            .label(Action::Timer)
            .unwrap_or_else(unassigned);
        let split_key = self
            .keybinds()
            .label(Action::OpenSolve)
            .unwrap_or_else(unassigned);

        let splits = *self.timer.splits();
        let active = self.timer.is_active();
        let running = matches!(self.timer.phase(), TimerPhase::Running { .. });
        let current = if running {
            splits.current().min(events.len() - 1)
        } else {
            0
        };
        let elapsed_ms = self.timer.elapsed(now).as_millis() as u64;
        // A hardware timer times the whole relay and nothing per puzzle, so
        // passing a puzzle only moves on to the next one.
        let timed_by_hardware = self.hardware_timing();
        let total_only = running && self.hardware_attempt;
        let done = if running && !total_only {
            splits.durations(elapsed_ms)
        } else {
            Vec::new()
        };

        let running_hint = if timed_by_hardware {
            strings.relays_running_hardware
        } else if self.settings.relay_key_splits {
            strings.relays_running
        } else {
            strings.relays_running_stop
        }
        .replace("{current}", &(current + 1).to_string())
        .replace("{total}", &events.len().to_string())
        .replace("{event}", events[current].name())
        .replace("{key}", &timer_key)
        .replace("{split}", &split_key);

        let (text, color, hint) = if !active && timed_by_hardware {
            (
                format_ms(0),
                muted,
                strings.relays_ready_hardware.to_owned(),
            )
        } else {
            match self.timer.phase() {
                TimerPhase::Running { .. } => (format_ms(elapsed_ms), foreground, running_hint),
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
                    Readiness::Released => (
                        format_ms(0),
                        foreground,
                        strings.hint_start.replace("{key}", &timer_key),
                    ),
                },
            }
        };

        let rows = FadeScroll::new("relay-scrambles", background)
            .flex_none()
            .max_h(relative(0.35))
            .gap_1()
            .children(scrambles.iter().enumerate().map(|(index, scramble)| {
                let finished = index < splits.marked() && running;
                let is_current = running && index == current;
                h_flex()
                    .flex_none()
                    .px_2()
                    .py_1()
                    .gap_3()
                    .items_start()
                    .rounded_sm()
                    .when(is_current, |row| row.bg(accent))
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
                    .child(
                        div()
                            .flex_1()
                            .min_w_0()
                            .font_family(mono.clone())
                            .text_color(if finished { muted } else { colors.scramble })
                            .child(scramble.clone()),
                    )
                    .when(finished && !total_only, |row| {
                        row.child(
                            div()
                                .flex_none()
                                .whitespace_nowrap()
                                .font_family(mono.clone())
                                .child(format_ms(done[index])),
                        )
                    })
                    .into_any_element()
            }));

        v_flex()
            .relative()
            .flex_1()
            .min_h_0()
            .gap_2()
            .child(v_flex().absolute().inset_0().child(digits_panel(
                text,
                color,
                &hint,
                muted,
                mono.clone(),
                window,
            )))
            .when(!active, |column| {
                column.child(
                    h_flex()
                        .flex_none()
                        .justify_end()
                        .gap_2()
                        .child(self.render_relay_mode_button(cx))
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
            })
            .child(rows)
            .into_any_element()
    }
}
