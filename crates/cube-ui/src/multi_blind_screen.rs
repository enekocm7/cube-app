//! The Multi-Blind screen: several cubes memorized and solved in one attempt.
//!
//! What an attempt scores, and how the attempts are saved, lives in
//! `cube_core::multi_blind`; this module keeps the screen's state and draws it.
//! The attempt is timed by the same timer as every other solve, so holding the
//! key and releasing it work exactly as they do on the timer screen.

use std::path::PathBuf;
use std::rc::Rc;
use std::time::{Instant, SystemTime, UNIX_EPOCH};

use cube_core::format::{format_ms, parse_time};
use cube_core::multi_blind::{
    self, DnfReason, MAX_CUBES, MIN_CUBES, MultiBlindAttempt, MultiBlindData, MultiBlindResult,
};
use cube_core::{Action, Readiness, TimerOutcome, TimerPhase, persistence};
use gpui_kit::assets::IconName;
use gpui_kit::component::button::{Button, ButtonVariants as _};
use gpui_kit::component::input::{Input, InputState};
use gpui_kit::component::notification::Notification;
use gpui_kit::component::{ActiveTheme as _, Sizable as _, WindowExt as _, h_flex, v_flex};
use gpui_kit::prelude::FluentBuilder as _;
use gpui_kit::*;

use crate::app::{CubeApp, Screen};
use crate::details::{field_error, field_label, keep_focus};
use crate::dialog_size::{DialogSize, dialog_title, dialog_width};
use crate::dialog_submit::{DialogSubmit as _, Submit, on_press};
use crate::palette::Palette;
use crate::scroll_fade::FadeScroll;
use crate::stats_screen::empty_message;

/// Height of an attempt's row, which follows the text size.
const ROW_HEIGHT: Rems = Rems(1.75);
/// Width of the column numbering the scrambles.
const NUMBER_WIDTH: Rems = Rems(2.5);
/// The size of the digits.
const DIGITS_SIZE: Rems = Rems(5.);

/// Where an attempt stands.
enum Phase {
    /// Nothing under way: the attempts are showing.
    Idle,
    /// The scrambles are being prepared. `id` tells this request from an
    /// older one that was abandoned.
    Preparing { count: u32, id: u64 },
    /// The scrambles are ready, and the timer starts the attempt.
    Ready { scrambles: Vec<String> },
    /// The attempt has been timed and waits for how many cubes were solved.
    Finished {
        scrambles: Vec<String>,
        time_ms: u64,
    },
}

/// The screen's state.
pub(crate) struct MultiBlindState {
    data: MultiBlindData,
    /// Where the attempts are saved; `None` keeps saving off.
    path: Option<PathBuf>,
    phase: Phase,
    /// The number of cubes asked for last, to offer again.
    cubes: u32,
    /// Counts the requests for scrambles.
    requests: u64,
    /// The attempt selected with the keyboard, as an index into `data`.
    selected: Option<usize>,
}

impl MultiBlindState {
    pub(crate) fn new(data: MultiBlindData, path: Option<PathBuf>) -> Self {
        let cubes = data
            .attempts
            .last()
            .map_or(MIN_CUBES, |attempt| attempt.attempted);
        Self {
            data,
            path,
            phase: Phase::Idle,
            cubes,
            requests: 0,
            selected: None,
        }
    }

    /// The scrambles of the attempt under way, if it has them.
    fn scrambles(&self) -> Option<&[String]> {
        match &self.phase {
            Phase::Ready { scrambles } | Phase::Finished { scrambles, .. } => Some(scrambles),
            Phase::Idle | Phase::Preparing { .. } => None,
        }
    }

    /// The attempt an action applies to: the selection, or the newest.
    fn target(&self) -> Option<usize> {
        let last = self.data.attempts.len().checked_sub(1)?;
        Some(self.selected.unwrap_or(last).min(last))
    }
}

/// A field for a whole number, with the message about what it should be.
struct NumberPrompt {
    label: String,
    input: Entity<InputState>,
    error: Option<String>,
}

impl Render for NumberPrompt {
    fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let red = Palette::of(cx).red;
        let muted = cx.theme().muted_foreground;
        v_flex()
            .gap_2()
            .py_2()
            .child(div().text_sm().text_color(muted).child(self.label.clone()))
            .child(keep_focus(
                "multi-blind-field",
                Input::new(&self.input).focus_bordered(false),
            ))
            .children(field_error(self.error.as_ref(), red))
    }
}

/// The fields of the dialog that edits a saved attempt.
struct AttemptForm {
    attempted: Entity<InputState>,
    solved: Entity<InputState>,
    time: Entity<InputState>,
    /// The labels of the three fields, in order.
    labels: [&'static str; 3],
    error: Option<String>,
}

impl Render for AttemptForm {
    fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let red = Palette::of(cx).red;
        let muted = cx.theme().muted_foreground;
        let field = |id: &'static str, label: &'static str, input: &Entity<InputState>| {
            v_flex()
                .gap_1()
                .child(field_label(label, muted))
                .child(keep_focus(id, Input::new(input).focus_bordered(false)))
        };
        v_flex()
            .gap_3()
            .py_2()
            .child(field(
                "multi-blind-edit-attempted",
                self.labels[0],
                &self.attempted,
            ))
            .child(field(
                "multi-blind-edit-solved",
                self.labels[1],
                &self.solved,
            ))
            .child(field("multi-blind-edit-time", self.labels[2], &self.time))
            .children(field_error(self.error.as_ref(), red))
    }
}

/// What a number prompt asks and does.
pub(crate) struct Prompt {
    pub(crate) title: &'static str,
    pub(crate) label: String,
    pub(crate) default: String,
    pub(crate) button: &'static str,
}

/// Checks what was typed and acts on it, or says why it cannot.
pub(crate) type Accept =
    Rc<dyn Fn(&mut CubeApp, &str, &mut Window, &mut Context<CubeApp>) -> Result<(), String>>;

impl CubeApp {
    /// Returns whether the scrambles of an attempt are ready to be timed.
    pub(crate) fn multi_blind_ready(&self) -> bool {
        matches!(self.multi_blind.phase, Phase::Ready { .. })
    }

    /// Handles the timer key on this screen: it asks for a new attempt, starts
    /// the one that is ready, or asks for the result of the one just timed.
    pub(crate) fn multi_blind_timer_key(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        match self.multi_blind.phase {
            Phase::Idle => self.open_new_multi_blind(window, cx),
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
            Phase::Finished { .. } => self.open_multi_blind_result(window, cx),
            Phase::Preparing { .. } => {}
        }
    }

    /// Runs a shortcut on this screen. Returns `false` for those that work the
    /// same everywhere.
    pub(crate) fn multi_blind_action(
        &mut self,
        action: Action,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> bool {
        let idle = matches!(self.multi_blind.phase, Phase::Idle);
        match action {
            Action::OpenSolve => match self.multi_blind.phase {
                // As on the timer, Enter opens the selected attempt, or the
                // newest; the timer key starts a new one.
                Phase::Idle => match self.multi_blind.target() {
                    Some(index) => self.open_edit_multi_blind(index, window, cx),
                    None => self.open_new_multi_blind(window, cx),
                },
                Phase::Finished { .. } => self.open_multi_blind_result(window, cx),
                Phase::Preparing { .. } | Phase::Ready { .. } => {}
            },
            Action::PreviousSolve if idle => self.step_multi_blind_attempt(true, cx),
            Action::NextSolve if idle => self.step_multi_blind_attempt(false, cx),
            Action::DeleteSolve if idle => self.confirm_delete_multi_blind(window, cx),
            Action::PreviousSolve | Action::NextSolve | Action::DeleteSolve => {}
            _ => return false,
        }
        true
    }

    /// Moves the selection one attempt newer (`newer`) or older.
    fn step_multi_blind_attempt(&mut self, newer: bool, cx: &mut Context<Self>) {
        let count = self.multi_blind.data.attempts.len();
        let Some(current) = self.multi_blind.target() else {
            return;
        };
        let next = if newer {
            (current + 1).min(count - 1)
        } else {
            current.saturating_sub(1)
        };
        self.multi_blind.selected = Some(next);
        cx.notify();
    }

    /// Goes back to the attempts from scrambles that were never used. An
    /// attempt that was timed is kept until its result is entered.
    pub(crate) fn leave_multi_blind_attempt(&mut self, cx: &mut Context<Self>) -> bool {
        if self.screen != Screen::MultiBlind
            || !matches!(
                self.multi_blind.phase,
                Phase::Preparing { .. } | Phase::Ready { .. }
            )
        {
            return false;
        }
        self.multi_blind.phase = Phase::Idle;
        cx.notify();
        true
    }

    /// Asks how many cubes to attempt.
    fn open_new_multi_blind(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let strings = self.strings();
        if self.multi_blind.path.is_none() {
            let message = strings.multi_blind_unavailable;
            window.push_notification(Notification::warning(message), cx);
            return;
        }
        let bounds = |text: &str| {
            text.replace("{min}", &MIN_CUBES.to_string())
                .replace("{max}", &MAX_CUBES.to_string())
        };
        let invalid = bounds(strings.multi_blind_cubes_invalid);
        let prompt = Prompt {
            title: strings.multi_blind_new,
            label: bounds(strings.multi_blind_how_many),
            default: self.multi_blind.cubes.to_string(),
            button: strings.multi_blind_prepare,
        };
        let accept: Accept = Rc::new(move |app, text, window, cx| {
            let count = text
                .trim()
                .parse::<u32>()
                .ok()
                .filter(|&count| multi_blind::validate(count, 0).is_ok())
                .ok_or_else(|| invalid.clone())?;
            app.prepare_multi_blind(count, window, cx);
            Ok(())
        });
        self.open_number_prompt(prompt, accept, window, cx);
    }

    /// Asks how many of the cubes were solved.
    fn open_multi_blind_result(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let (Phase::Finished { scrambles, .. }, strings) =
            (&self.multi_blind.phase, self.strings())
        else {
            return;
        };
        let attempted = u32::try_from(scrambles.len()).unwrap_or(MAX_CUBES);
        let count = attempted.to_string();
        let invalid = strings
            .multi_blind_solved_invalid
            .replace("{count}", &count);
        let prompt = Prompt {
            title: strings.multi_blind_result_title,
            label: strings.multi_blind_solved_prompt.replace("{count}", &count),
            default: count,
            button: strings.save,
        };
        let accept: Accept = Rc::new(move |app, text, window, cx| {
            let solved =
                multi_blind::parse_solved(text, attempted).ok_or_else(|| invalid.clone())?;
            app.record_multi_blind(solved, window, cx);
            Ok(())
        });
        self.open_number_prompt(prompt, accept, window, cx);
    }

    /// Opens the dialog that edits a saved attempt.
    fn open_edit_multi_blind(&mut self, index: usize, window: &mut Window, cx: &mut Context<Self>) {
        let strings = self.strings();
        if self.multi_blind.path.is_none() {
            let message = strings.multi_blind_unavailable;
            window.push_notification(Notification::warning(message), cx);
            return;
        }
        let Some(attempt) = self.multi_blind.data.attempts.get(index) else {
            return;
        };
        let defaults = [
            attempt.attempted.to_string(),
            attempt.solved.to_string(),
            format_ms(attempt.time_ms),
        ];
        self.multi_blind.selected = Some(index);
        let [attempted, solved, time] =
            defaults.map(|value| cx.new(|cx| InputState::new(window, cx).default_value(value)));
        let first = attempted.clone();
        let view = cx.new(|_| AttemptForm {
            attempted: attempted.clone(),
            solved: solved.clone(),
            time: time.clone(),
            labels: [
                strings.multi_blind_field_attempted,
                strings.multi_blind_field_solved,
                strings.field_time,
            ],
            error: None,
        });
        let app = cx.entity().downgrade();
        let first_build = std::cell::Cell::new(true);
        window.open_dialog(cx, move |dialog, window, cx| {
            if first_build.replace(false) {
                first.focus_handle(cx).focus(window, cx);
            }
            let (app, form) = (app.clone(), view.clone());
            let (attempted, solved, time) = (attempted.clone(), solved.clone(), time.clone());
            let submit: Submit = Rc::new(move |window, cx| {
                let texts =
                    [&attempted, &solved, &time].map(|input| input.read(cx).value().to_string());
                let outcome = app
                    .update(cx, |app, cx| {
                        app.apply_multi_blind_edit(index, &texts, window, cx)
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
            dialog
                .title(dialog_title(strings.multi_blind_edit_title, strings.close))
                .close_button(false)
                .w(dialog_width(DialogSize::COMPACT, window))
                .submit_on_enter(&submit)
                .child(view.clone())
                .footer(
                    h_flex()
                        .w_full()
                        .justify_end()
                        .gap_2()
                        .child(
                            Button::new("multi-blind-edit-cancel")
                                .outline()
                                .label(strings.cancel)
                                .on_click(|_, window, cx| window.close_dialog(cx)),
                        )
                        .child(
                            Button::new("multi-blind-edit-save")
                                .primary()
                                .label(strings.save)
                                .on_click(on_press(&submit)),
                        ),
                )
        });
    }

    /// Checks the edited fields and, if they hold, replaces the numbers of the
    /// attempt. The message says what to fix otherwise.
    fn apply_multi_blind_edit(
        &mut self,
        index: usize,
        texts: &[String; 3],
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> Result<(), String> {
        let strings = self.strings();
        let attempted = texts[0]
            .trim()
            .parse::<u32>()
            .ok()
            .filter(|&count| multi_blind::validate(count, 0).is_ok())
            .ok_or_else(|| {
                strings
                    .multi_blind_cubes_invalid
                    .replace("{min}", &MIN_CUBES.to_string())
                    .replace("{max}", &MAX_CUBES.to_string())
            })?;
        let solved = multi_blind::parse_solved(&texts[1], attempted).ok_or_else(|| {
            strings
                .multi_blind_solved_invalid
                .replace("{count}", &attempted.to_string())
        })?;
        let time_ms = parse_time(&texts[2])
            .filter(|&ms| ms > 0)
            .ok_or_else(|| strings.invalid_time.to_owned())?;
        let Some(entry) = self.multi_blind.data.attempts.get_mut(index) else {
            return Ok(());
        };
        entry.attempted = attempted;
        entry.solved = solved;
        entry.time_ms = time_ms;
        // The scrambles belonged to the cubes first attempted.
        if entry.scrambles.len() != attempted as usize {
            entry.scrambles.clear();
        }
        self.save_multi_blind(window, cx);
        cx.notify();
        Ok(())
    }

    /// Opens a dialog with one field for a whole number. `accept` closes the
    /// dialog by returning `Ok`, and otherwise its message shows under the
    /// field.
    pub(crate) fn open_number_prompt(
        &self,
        prompt: Prompt,
        accept: Accept,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let strings = self.strings();
        let Prompt {
            title,
            label,
            default,
            button,
        } = prompt;
        let input = cx.new(|cx| InputState::new(window, cx).default_value(default));
        let view = cx.new(|_| NumberPrompt {
            label,
            input: input.clone(),
            error: None,
        });
        let app = cx.entity().downgrade();
        let first_build = std::cell::Cell::new(true);
        window.open_dialog(cx, move |dialog, window, cx| {
            if first_build.replace(false) {
                input.focus_handle(cx).focus(window, cx);
            }
            let (input, app, accept) = (input.clone(), app.clone(), Rc::clone(&accept));
            let shown = view.clone();
            let submit: Submit = Rc::new({
                let view = view.clone();
                move |window, cx| {
                    let text = input.read(cx).value().to_string();
                    let outcome = app
                        .update(cx, |app, cx| accept(app, &text, window, cx))
                        .unwrap_or(Ok(()));
                    match outcome {
                        Ok(()) => window.close_dialog(cx),
                        Err(message) => view.update(cx, |view, cx| {
                            view.error = Some(message);
                            cx.notify();
                        }),
                    }
                }
            });
            dialog
                .title(dialog_title(title, strings.close))
                .close_button(false)
                .w(dialog_width(DialogSize::COMPACT, window))
                .submit_on_enter(&submit)
                .child(shown)
                .footer(
                    h_flex()
                        .w_full()
                        .justify_end()
                        .gap_2()
                        .child(
                            Button::new("multi-blind-cancel")
                                .outline()
                                .label(strings.cancel)
                                .on_click(|_, window, cx| window.close_dialog(cx)),
                        )
                        .child(
                            Button::new("multi-blind-accept")
                                .primary()
                                .label(button)
                                .on_click(on_press(&submit)),
                        ),
                )
        });
    }

    /// Starts preparing the scrambles of a new attempt.
    fn prepare_multi_blind(&mut self, count: u32, window: &mut Window, cx: &mut Context<Self>) {
        self.multi_blind.requests += 1;
        let id = self.multi_blind.requests;
        self.multi_blind.cubes = count;
        self.multi_blind.phase = Phase::Preparing { count, id };
        cx.spawn_in(window, async move |this, cx| {
            let scrambles = cx
                .background_executor()
                .spawn(async move { multi_blind::generate_scrambles(count) })
                .await;
            this.update(cx, |app, cx| {
                // Only the latest request counts: this one may have been left.
                if matches!(app.multi_blind.phase, Phase::Preparing { id: current, .. } if current == id)
                {
                    app.multi_blind.phase = Phase::Ready { scrambles };
                    cx.notify();
                }
            })
            .ok();
        })
        .detach();
        cx.notify();
    }

    /// Takes the time of the attempt that just ended and asks for its result.
    pub(crate) fn finish_multi_blind_attempt(
        &mut self,
        outcome: TimerOutcome,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let phase = std::mem::replace(&mut self.multi_blind.phase, Phase::Idle);
        let Phase::Ready { scrambles } = phase else {
            self.multi_blind.phase = phase;
            return;
        };
        self.multi_blind.phase = Phase::Finished {
            scrambles,
            time_ms: outcome.time_ms,
        };
        self.open_multi_blind_result(window, cx);
        cx.notify();
    }

    /// Records the attempt that was timed, now that its result is known.
    fn record_multi_blind(&mut self, solved: u32, window: &mut Window, cx: &mut Context<Self>) {
        let phase = std::mem::replace(&mut self.multi_blind.phase, Phase::Idle);
        let Phase::Finished { scrambles, time_ms } = phase else {
            self.multi_blind.phase = phase;
            return;
        };
        let solved_at_unix_ms = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map_or(0, |elapsed| elapsed.as_millis() as u64);
        self.multi_blind.data.attempts.push(MultiBlindAttempt {
            attempted: u32::try_from(scrambles.len()).unwrap_or(MAX_CUBES),
            solved,
            time_ms,
            solved_at_unix_ms,
            scrambles,
        });
        self.multi_blind.selected = None;
        self.save_multi_blind(window, cx);
        cx.notify();
    }

    /// Writes the attempts to disk. Without a path, which means the file on
    /// disk could not be read, nothing is written over it.
    fn save_multi_blind(&self, window: &mut Window, cx: &mut Context<Self>) {
        if let Some(path) = &self.multi_blind.path
            && let Err(error) = persistence::save_multi_blind(path, &self.multi_blind.data)
        {
            let template = self.strings().toast_multi_blind_save_failed;
            self.report_error(template, &error, window, cx);
        }
    }

    fn confirm_delete_multi_blind(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let Some(index) = self.multi_blind.target() else {
            return;
        };
        if !self.settings.confirm_delete {
            self.delete_multi_blind(index, window, cx);
            return;
        }
        let strings = self.strings();
        let on_confirm = cx.listener(move |app: &mut Self, (): &(), window, cx| {
            app.delete_multi_blind(index, window, cx);
            app.focus.focus(window, cx);
        });
        let message = strings.trainer_delete_confirm.to_owned();
        self.open_confirm(strings.delete_solve, message, on_confirm, window, cx);
    }

    fn delete_multi_blind(&mut self, index: usize, window: &mut Window, cx: &mut Context<Self>) {
        if index >= self.multi_blind.data.attempts.len() {
            return;
        }
        self.multi_blind.data.attempts.remove(index);
        // The row now holds the next attempt, so several can go in a row.
        let last = self.multi_blind.data.attempts.len().checked_sub(1);
        self.multi_blind.selected = last.map(|last| index.min(last));
        self.save_multi_blind(window, cx);
        cx.notify();
    }

    /// Copies the scrambles of the attempt under way.
    pub(crate) fn copy_multi_blind_scrambles(
        &mut self,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if self.screen != Screen::MultiBlind {
            return;
        }
        let Some(scrambles) = self.multi_blind.scrambles() else {
            return;
        };
        let text = multi_blind::scrambles_text(scrambles);
        cx.write_to_clipboard(ClipboardItem::new_string(text));
        self.toast(self.strings().multi_blind_scrambles_copied, window, cx);
    }

    /// Saves the scrambles of the attempt under way to a text file, to print.
    pub(crate) fn save_multi_blind_scrambles(
        &mut self,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if self.screen != Screen::MultiBlind {
            return;
        }
        let Some(scrambles) = self.multi_blind.scrambles() else {
            return;
        };
        let text = multi_blind::scrambles_text(scrambles);
        let directory = dirs::document_dir()
            .or_else(dirs::home_dir)
            .unwrap_or_default();
        let receiver = cx.prompt_for_new_path(&directory, Some("multi-blind-scrambles.txt"));
        cx.spawn_in(window, async move |this, cx| {
            let saved = match receiver.await {
                Ok(Ok(Some(path))) => std::fs::write(&path, text)
                    .map(|()| path)
                    .map_err(|error| error.to_string()),
                Ok(Err(error)) => Err(format!("{error:#}")),
                // Cancelled, or the window closed while the picker was open.
                Ok(Ok(None)) | Err(_) => return,
            };
            this.update_in(cx, |app, window, cx| {
                let strings = app.strings();
                match saved {
                    Ok(path) => {
                        let path = path.display().to_string();
                        let message = strings.multi_blind_scrambles_saved.replace("{path}", &path);
                        app.toast(&message, window, cx);
                    }
                    Err(error) => {
                        let message = strings
                            .toast_multi_blind_scrambles_failed
                            .replace("{error}", &error);
                        window.push_notification(Notification::error(message), cx);
                    }
                }
            })
            .ok();
        })
        .detach();
    }

    /// The text for how an attempt came out.
    fn multi_blind_result_text(&self, attempt: &MultiBlindAttempt) -> String {
        let strings = self.strings();
        let (solved, attempted) = (attempt.solved.to_string(), attempt.attempted.to_string());
        match attempt.result() {
            MultiBlindResult::Scored { points, time_ms } => strings
                .multi_blind_result
                .replace("{solved}", &solved)
                .replace("{attempted}", &attempted)
                .replace("{points}", &points.to_string())
                .replace("{time}", &format_ms(time_ms)),
            MultiBlindResult::Dnf(reason) => {
                let text = match reason {
                    DnfReason::NoPoints => strings.multi_blind_dnf_no_points,
                    DnfReason::OverTime => strings.multi_blind_dnf_over_time,
                };
                let limit = format_ms(multi_blind::time_limit_ms(attempt.attempted));
                format!(
                    "{} · {}",
                    text.replace("{solved}", &solved)
                        .replace("{attempted}", &attempted)
                        .replace("{limit}", &limit),
                    format_ms(attempt.time_ms)
                )
            }
        }
    }

    /// The line summarizing every attempt.
    fn multi_blind_summary_text(&self) -> String {
        let summary = self.multi_blind.data.summary();
        let best = summary
            .best
            .and_then(|index| self.multi_blind.data.attempts.get(index))
            .map_or_else(
                || "–".to_owned(),
                |attempt| {
                    format!(
                        "{}/{} {}",
                        attempt.solved,
                        attempt.attempted,
                        format_ms(attempt.time_ms)
                    )
                },
            );
        self.strings()
            .multi_blind_summary
            .replace("{attempts}", &summary.attempts.to_string())
            .replace("{scored}", &summary.scored.to_string())
            .replace(
                "{rate}",
                &summary
                    .success_percent()
                    .map_or_else(|| "–".to_owned(), |rate| rate.to_string()),
            )
            .replace("{best}", &best)
    }

    /// The Multi-Blind screen.
    pub(crate) fn render_multi_blind(&self, window: &Window, cx: &mut Context<Self>) -> AnyElement {
        let strings = self.strings();
        let theme = cx.theme();
        let (background, muted) = (theme.background, theme.muted_foreground);
        let timer_key = self
            .keybinds()
            .label(Action::Timer)
            .unwrap_or_else(|| strings.unassigned.to_owned());
        let hint = |text: &str| text.replace("{key}", &timer_key);

        let body = match &self.multi_blind.phase {
            Phase::Idle => {
                self.render_multi_blind_attempts(&hint(strings.multi_blind_start_hint), cx)
            }
            Phase::Preparing { count, .. } => empty_message_owned(
                strings
                    .multi_blind_preparing
                    .replace("{count}", &count.to_string()),
                muted,
            ),
            Phase::Ready { scrambles } => {
                if self.timer.is_active() {
                    let running = if self.hardware_timing() {
                        strings.multi_blind_running_hardware
                    } else {
                        strings.multi_blind_running
                    };
                    self.render_multi_blind_digits(&hint(running), window, cx)
                } else {
                    let ready = if self.hardware_timing() {
                        strings.multi_blind_ready_hardware
                    } else {
                        strings.multi_blind_ready
                    };
                    self.render_multi_blind_scrambles(scrambles, &hint(ready), background, cx)
                }
            }
            Phase::Finished { scrambles, time_ms } => {
                self.render_multi_blind_finished(*time_ms, scrambles.len(), window, cx)
            }
        };

        let running = self.timer.is_active();
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
                    .child(
                        div()
                            .text_lg()
                            .font_weight(FontWeight::SEMIBOLD)
                            .child(strings.nav_multi_blind),
                    )
                    .when(
                        !running && matches!(self.multi_blind.phase, Phase::Idle),
                        |row| {
                            row.child(
                                Button::new("multi-blind-new")
                                    .primary()
                                    .small()
                                    .icon(IconName::Plus)
                                    .label(strings.multi_blind_new)
                                    .tab_stop(false)
                                    .on_click(cx.listener(|app, _, window, cx| {
                                        app.open_new_multi_blind(window, cx);
                                    })),
                            )
                        },
                    ),
            )
            .child(
                div()
                    .flex_none()
                    .text_sm()
                    .text_color(muted)
                    .child(self.multi_blind_summary_text()),
            )
            .child(body)
            .into_any_element()
    }

    /// The buttons that edit or delete an attempt, for the mouse; the keyboard
    /// has `Enter` and the delete key.
    fn render_multi_blind_row_buttons(
        &self,
        index: usize,
        cx: &mut Context<Self>,
    ) -> impl IntoElement + use<> {
        let strings = self.strings();
        h_flex()
            .flex_none()
            .child(
                Button::new(("multi-blind-edit", index))
                    .ghost()
                    .xsmall()
                    .icon(IconName::Pencil)
                    .tooltip(strings.multi_blind_edit_title)
                    .accessibility_label(strings.multi_blind_edit_title)
                    .tab_stop(false)
                    .on_click(cx.listener(move |app, _, window, cx| {
                        app.open_edit_multi_blind(index, window, cx);
                    })),
            )
            .child(
                Button::new(("multi-blind-delete", index))
                    .ghost()
                    .xsmall()
                    .icon(IconName::Trash)
                    .tooltip(strings.delete_solve)
                    .accessibility_label(strings.delete_solve)
                    .tab_stop(false)
                    .on_click(cx.listener(move |app, _, window, cx| {
                        app.multi_blind.selected = Some(index);
                        app.confirm_delete_multi_blind(window, cx);
                    })),
            )
    }

    /// The saved attempts, newest first, under a hint about starting another.
    fn render_multi_blind_attempts(&self, hint: &str, cx: &mut Context<Self>) -> AnyElement {
        let strings = self.strings();
        let theme = cx.theme();
        let (background, muted, accent) = (theme.background, theme.muted_foreground, theme.accent);
        let mono = theme.mono_font_family.clone();
        let language = self.language();
        let target = self.multi_blind.target();
        let attempts = &self.multi_blind.data.attempts;

        let list: AnyElement = if attempts.is_empty() {
            let message = if self.multi_blind.path.is_some() {
                strings.multi_blind_empty
            } else {
                strings.multi_blind_unavailable
            };
            empty_message(message, muted)
        } else {
            FadeScroll::new("multi-blind-attempts", background)
                .flex_1()
                .min_h_0()
                .children(attempts.iter().enumerate().rev().map(|(index, attempt)| {
                    h_flex()
                        .id(("multi-blind-attempt", index))
                        .flex_none()
                        .h(ROW_HEIGHT)
                        .px_3()
                        .gap_3()
                        .rounded_sm()
                        .text_sm()
                        .when(target == Some(index), |row| row.bg(accent))
                        .on_click(cx.listener(move |app, _, window, cx| {
                            app.multi_blind.selected = Some(index);
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
                                .child((index + 1).to_string()),
                        )
                        .child(
                            div()
                                .flex_1()
                                .min_w_0()
                                .truncate()
                                .font_family(mono.clone())
                                .child(self.multi_blind_result_text(attempt)),
                        )
                        .child(
                            div()
                                .flex_none()
                                .whitespace_nowrap()
                                .text_color(muted)
                                .child(language.format_date(attempt.solved_at_unix_ms)),
                        )
                        .child(self.render_multi_blind_row_buttons(index, cx))
                        .into_any_element()
                }))
                .into_any_element()
        };

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
            .child(list)
            .into_any_element()
    }

    /// The scrambles of the attempt, with the way to copy or save them.
    fn render_multi_blind_scrambles(
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
        let attempted = u32::try_from(scrambles.len()).unwrap_or(MAX_CUBES);
        let limit = strings
            .multi_blind_time_limit
            .replace("{limit}", &format_ms(multi_blind::time_limit_ms(attempted)));

        v_flex()
            .flex_1()
            .min_h_0()
            .gap_2()
            .child(
                h_flex()
                    .flex_none()
                    .justify_between()
                    .gap_2()
                    .child(
                        v_flex()
                            .min_w_0()
                            .child(div().text_sm().text_color(muted).child(hint.to_owned()))
                            .child(div().text_sm().text_color(muted).child(limit)),
                    )
                    .child(
                        h_flex()
                            .flex_none()
                            .gap_2()
                            .child(
                                Button::new("multi-blind-copy")
                                    .outline()
                                    .small()
                                    .icon(IconName::Copy)
                                    .label(strings.action_copy_multi_blind_scrambles)
                                    .tab_stop(false)
                                    .on_click(cx.listener(|app, _, window, cx| {
                                        app.copy_multi_blind_scrambles(window, cx);
                                        app.focus.focus(window, cx);
                                    })),
                            )
                            .child(
                                Button::new("multi-blind-save")
                                    .outline()
                                    .small()
                                    .label(strings.action_save_multi_blind_scrambles)
                                    .tab_stop(false)
                                    .on_click(cx.listener(|app, _, window, cx| {
                                        app.save_multi_blind_scrambles(window, cx);
                                        app.focus.focus(window, cx);
                                    })),
                            ),
                    ),
            )
            .child(
                FadeScroll::new("multi-blind-scrambles", background)
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
                            .child(div().flex_1().min_w_0().child(scramble.clone()))
                            .into_any_element()
                    })),
            )
            .into_any_element()
    }

    /// The digits of the attempt while it is held or running.
    fn render_multi_blind_digits(
        &self,
        hint: &str,
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
        let (text, color, hint) = match self.timer.phase() {
            TimerPhase::Running { .. } => (
                format_ms(self.timer.elapsed(now).as_millis() as u64),
                foreground,
                hint.to_owned(),
            ),
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
                Readiness::Released => (format_ms(0), foreground, hint.to_owned()),
            },
        };
        digits_panel(text, color, &hint, muted, mono, window)
    }

    /// The time of the attempt, waiting for its result.
    fn render_multi_blind_finished(
        &self,
        time_ms: u64,
        attempted: usize,
        window: &Window,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        let theme = cx.theme();
        let (foreground, muted) = (theme.foreground, theme.muted_foreground);
        let mono = theme.mono_font_family.clone();
        // The attempt is over; what is left is saying how it went.
        let prompt = self
            .strings()
            .multi_blind_solved_prompt
            .replace("{count}", &attempted.to_string());
        digits_panel(format_ms(time_ms), foreground, &prompt, muted, mono, window)
    }
}

/// Centers `text` as big digits with a line of `hint` under them.
pub(crate) fn digits_panel(
    text: String,
    color: Hsla,
    hint: &str,
    muted: Hsla,
    mono: SharedString,
    window: &Window,
) -> AnyElement {
    let size = DIGITS_SIZE.to_pixels(window.rem_size());
    v_flex()
        .flex_1()
        .items_center()
        .justify_center()
        .gap_3()
        .child(
            div()
                .font_family(mono)
                .text_size(size)
                .line_height(size)
                .whitespace_nowrap()
                .font_weight(FontWeight::MEDIUM)
                .text_color(color)
                .child(text),
        )
        .child(
            div()
                .text_sm()
                .text_color(muted)
                .whitespace_nowrap()
                .child(hint.to_owned()),
        )
        .into_any_element()
}

/// A centered message built at run time.
pub(crate) fn empty_message_owned(message: String, color: Hsla) -> AnyElement {
    v_flex()
        .flex_1()
        .items_center()
        .justify_center()
        .text_color(color)
        .child(message)
        .into_any_element()
}
