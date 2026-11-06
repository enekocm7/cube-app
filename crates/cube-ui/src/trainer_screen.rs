//! The trainer: the picker of cases and the drill of those selected.
//!
//! What a set, a case or a drill means lives in `cube_core::trainer`; this
//! module keeps the screen's state and draws it. The drill reuses the timer
//! panel, so timing works exactly as it does on the timer screen.

use std::cell::RefCell;
use std::collections::HashMap;
use std::ops::Range;
use std::path::PathBuf;
use std::rc::Rc;
use std::sync::Arc;
use std::time::{SystemTime, UNIX_EPOCH};

use cube_core::format::format_ms;
use cube_core::trainer::notation::AlgError;
use cube_core::trainer::{
    AlgCase, AlgSet, Attempt, CaseGroup, CaseStats, CustomAlgError, Drill, GridStep, PLL, PUZZLES,
    TrainerData, case_diagram, check_algorithm, grid_step, next_set, rows_before, set_event,
    slowest_first,
};
use cube_core::{Action, Modifier, TimerOutcome, WcaEvent, persistence};
use cube_locale::Strings;
use gpui_kit::assets::IconName;
use gpui_kit::component::button::{Button, ButtonVariants as _};
use gpui_kit::component::input::{Input, InputEvent, InputState};
use gpui_kit::component::notification::Notification;
use gpui_kit::component::scroll::Scrollbar;
use gpui_kit::component::{
    ActiveTheme as _, Disableable as _, Selectable as _, Sizable as _, WindowExt as _, h_flex,
    v_flex,
};
use gpui_kit::prelude::FluentBuilder as _;
use gpui_kit::*;

use crate::app::{CubeApp, LastResult, Screen};
use crate::details::{field_error, keep_focus};
use crate::dialog_size::{DialogSize, dialog_body_height, dialog_title, dialog_width};
use crate::dialog_submit::{DialogSubmit as _, Submit, close_action, on_press};
use crate::palette::Palette;
use crate::panel_resize::{PanelSide, render_grab};
use crate::scroll_fade::{FadeScroll, fading_scroll};
use crate::search_bar::{focus_on_search_key, new_search, query, search_field};
use crate::sidebar::number_width;
use crate::timer_view::result_text;

/// Width of the search field above the cases.
const SEARCH_WIDTH: Rems = Rems(14.);
/// Width of the summary of a case beside its attempts.
const SUMMARY_WIDTH: Rems = Rems(14.);
/// Width of the list of puzzles and sets.
const NAV_WIDTH: Rems = Rems(9.);
/// A case in the picker, which follows the text size.
const CARD_WIDTH: Rems = Rems(9.);
const CARD_HEIGHT: Rems = Rems(13.);
/// Height of a line of text on a case.
const CARD_LINE: Rems = Rems(1.);
const CARD_GAP: Rems = Rems(0.5);
const DIAGRAM_SIZE: Rems = Rems(5.5);
/// Height of a group's heading.
const HEADING_HEIGHT: Rems = Rems(2.);
/// The padding around the grid of cases.
const GRID_PADDING: Rems = Rems(1.);
/// Height of an attempt's row, which follows the text size.
const ROW_HEIGHT: Rems = Rems(1.75);
/// The drawing of the case when it is revealed during a drill.
const REVEALED_SIZE: Rems = Rems(7.);

/// Case drawings by set id and case name; `None` if one could not be drawn.
type Diagrams = HashMap<(&'static str, &'static str), Option<Arc<Image>>>;

/// A drill under way and what the screen shows about it.
struct ActiveDrill {
    drill: Drill,
    /// Where this drill's attempts start in the saved ones.
    first_attempt: usize,
    /// Whether the waiting case is shown.
    revealed: bool,
    /// The attempt selected with the keyboard, as an index into the saved ones.
    selected: Option<usize>,
}

/// Every attempt at one case, from any drill.
struct CaseAttempts {
    case: &'static AlgCase,
    /// The selected row; row `0` is the newest attempt.
    row: usize,
}

/// The trainer's state.
pub(crate) struct TrainerState {
    data: TrainerData,
    /// Where the data is saved; `None` keeps saving off.
    path: Option<PathBuf>,
    set: &'static AlgSet,
    /// The highlighted place in the picker, counting through its groups.
    cursor: usize,
    /// Whether the picker lists the slowest cases first.
    by_slowest: bool,
    search: Entity<InputState>,
    /// What the picker is searching for; empty shows every case.
    query: String,
    drill: Option<ActiveDrill>,
    /// The case whose attempts are showing in place of the picker.
    case_attempts: Option<CaseAttempts>,
    grid_scroll: ScrollHandle,
    history_scroll: UniformListScrollHandle,
    diagrams: RefCell<Diagrams>,
}

impl TrainerState {
    pub(crate) fn new(
        data: TrainerData,
        path: Option<PathBuf>,
        search: Entity<InputState>,
    ) -> Self {
        Self {
            data,
            path,
            set: &PLL,
            cursor: 0,
            by_slowest: false,
            search,
            query: String::new(),
            drill: None,
            case_attempts: None,
            grid_scroll: ScrollHandle::default(),
            history_scroll: UniformListScrollHandle::new(),
            diagrams: RefCell::default(),
        }
    }

    /// Lays the cases out the way the picker shows them.
    fn layout(&self) -> Vec<CaseGroup> {
        let layout = if self.by_slowest {
            slowest_first(self.set, &self.data.stats(self.set))
        } else {
            self.set.groups()
        };
        self.set.matching(layout, &self.query)
    }

    /// Returns the highlighted case, as a position in the set.
    fn cursor_case(&self) -> Option<usize> {
        let layout = self.layout();
        let mut cases = layout.iter().flat_map(|group| group.cases.iter());
        cases.nth(self.cursor).copied()
    }

    /// Returns the drawing of `case`, drawing it the first time.
    fn diagram(&self, set: &AlgSet, case: &'static AlgCase) -> Option<Arc<Image>> {
        let mut diagrams = self.diagrams.borrow_mut();
        let entry = diagrams.entry((set.id, case.name)).or_insert_with(|| {
            let svg = case_diagram(set, case)?;
            Some(Arc::new(Image::from_bytes(
                ImageFormat::Svg,
                svg.into_bytes(),
            )))
        });
        entry.clone()
    }

    /// Returns the attempts of the drill under way.
    fn drill_attempts(&self) -> &[Attempt] {
        let first = self.drill.as_ref().map_or(0, |active| active.first_attempt);
        self.data.attempts.get(first..).unwrap_or_default()
    }

    /// Returns where the attempts of the case on screen are, newest first.
    fn shown_case_attempts(&self) -> Vec<usize> {
        self.case_attempts.as_ref().map_or_else(Vec::new, |shown| {
            self.data.case_attempts(self.set, shown.case.name)
        })
    }

    /// Returns the attempt an action applies to: the selection, or during a
    /// drill the latest if nothing is selected.
    fn target_attempt(&self) -> Option<usize> {
        if let Some(shown) = &self.case_attempts {
            return self.shown_case_attempts().get(shown.row).copied();
        }
        let active = self.drill.as_ref()?;
        let last = self.data.attempts.len().checked_sub(1)?;
        let target = active.selected.unwrap_or(last).min(last);
        (target >= active.first_attempt).then_some(target)
    }
}

/// Creates the field that searches the cases. Enter hands the keyboard to the
/// cases it found.
pub(crate) fn new_trainer_search(
    strings: &'static Strings,
    window: &mut Window,
    cx: &mut Context<CubeApp>,
) -> Entity<InputState> {
    let search = new_search(strings, window, cx);
    cx.subscribe_in(
        &search,
        window,
        |app: &mut CubeApp, input, event: &InputEvent, window, cx| match event {
            InputEvent::Change => {
                app.trainer.query = query(input, cx);
                app.trainer.cursor = 0;
                app.trainer.grid_scroll.set_offset(Point::default());
            }
            InputEvent::PressEnter { .. } => app.focus.focus(window, cx),
            _ => {}
        },
    )
    .detach();
    search
}

/// How many cases fit side by side in the window.
fn grid_columns(window: &Window) -> usize {
    let rem = window.rem_size();
    let room =
        window.viewport_size().width - NAV_WIDTH.to_pixels(rem) - GRID_PADDING.to_pixels(rem) * 2.
            + CARD_GAP.to_pixels(rem);
    let card = CARD_WIDTH.to_pixels(rem) + CARD_GAP.to_pixels(rem);
    ((room / card).floor() as usize).max(1)
}

/// The time of an attempt, the way the timer shows it.
fn attempt_text(attempt: &Attempt) -> String {
    result_text(LastResult {
        time_ms: attempt.time_ms,
        modifier: attempt.modifier,
    })
}

/// One line of a case's figures.
fn stat_line(label: &'static str, value: String, muted: Hsla, mono: SharedString) -> Div {
    h_flex()
        .flex_none()
        .w_full()
        .h(CARD_LINE)
        .justify_between()
        .gap_1()
        .text_xs()
        .child(
            div()
                .flex_1()
                .min_w_0()
                .truncate()
                .text_color(muted)
                .child(label),
        )
        .child(
            div()
                .flex_none()
                .whitespace_nowrap()
                .font_family(mono)
                .child(value),
        )
}

impl CubeApp {
    /// Returns whether a drill is showing.
    pub(crate) fn drilling(&self) -> bool {
        self.screen == Screen::Trainer && self.trainer.drill.is_some()
    }

    /// Returns whether the picker of cases is showing.
    pub(crate) fn trainer_picking(&self) -> bool {
        self.screen == Screen::Trainer
            && self.trainer.drill.is_none()
            && self.trainer.case_attempts.is_none()
    }

    /// Returns whether the case the drill has waiting is being shown.
    pub(crate) fn trainer_revealed(&self) -> bool {
        self.drilling()
            && self
                .trainer
                .drill
                .as_ref()
                .is_some_and(|active| active.revealed)
    }

    /// Returns the scramble on screen: the drill's, or the timer's.
    pub(crate) fn shown_scramble(&self) -> Option<&str> {
        match &self.trainer.drill {
            Some(active) if self.drilling() => Some(active.drill.prompt().scramble.as_str()),
            _ => self.scramble.as_deref(),
        }
    }

    /// Returns the event whose sizes and drawings apply to what is on screen.
    pub(crate) fn shown_event(&self) -> WcaEvent {
        match &self.trainer.drill {
            Some(active) if self.drilling() => set_event(active.drill.set()),
            _ => self.event,
        }
    }

    /// Lets the search field have the keys while it is focused and takes `/`
    /// to reach it. Returns whether the key is dealt with.
    pub(crate) fn trainer_search_key(
        &mut self,
        event: &KeyDownEvent,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> bool {
        if !self.trainer_picking() {
            return false;
        }
        let search = self.trainer.search.clone();
        if search.read(cx).focus_handle(cx).is_focused(window) {
            match event.keystroke.key.as_str() {
                "escape" => {
                    self.clear_trainer_search(window, cx);
                    self.focus.focus(window, cx);
                    cx.stop_propagation();
                }
                "down" => {
                    self.focus.focus(window, cx);
                    cx.stop_propagation();
                }
                _ => {}
            }
            return true;
        }
        let keybinds = self.keybinds().clone();
        let taken = focus_on_search_key(event, &keybinds, &search, window, cx);
        if taken {
            cx.stop_propagation();
        }
        taken
    }

    /// Ends the search of the picker. Returns whether there was one.
    pub(crate) fn clear_trainer_search(
        &mut self,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> bool {
        if !self.trainer_picking() || self.trainer.query.is_empty() {
            return false;
        }
        self.trainer
            .search
            .update(cx, |search, cx| search.set_value("", window, cx));
        self.trainer.query.clear();
        self.trainer.cursor = 0;
        cx.notify();
        true
    }

    /// Writes the trainer's data to disk. Without a path, which means the file
    /// on disk could not be read, nothing is written over it.
    fn save_trainer(&self, window: &mut Window, cx: &mut Context<Self>) {
        if let Some(path) = &self.trainer.path
            && let Err(error) = persistence::save_trainer(path, &self.trainer.data)
        {
            self.report_error(self.strings().toast_trainer_save_failed, &error, window, cx);
        }
    }

    /// Runs a shortcut on the trainer screen. Returns `false` for those that
    /// work the same everywhere; the rest mean nothing here and do nothing.
    pub(crate) fn trainer_action(
        &mut self,
        action: Action,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> bool {
        if matches!(
            action,
            Action::OpenStats
                | Action::OpenCharts
                | Action::OpenGoals
                | Action::OpenTrainer
                | Action::OpenBluetooth
                | Action::OpenSettings
                | Action::ShowHelp
                | Action::FocusSearch
                | Action::ToggleScramblePreview
        ) {
            return false;
        }
        if self.drilling() {
            self.drill_action(action, window, cx);
        } else if self.trainer.case_attempts.is_some() {
            self.case_attempts_action(action, window, cx);
        } else {
            self.picker_action(action, window, cx);
        }
        true
    }

    fn picker_action(&mut self, action: Action, window: &mut Window, cx: &mut Context<Self>) {
        match action {
            Action::PreviousSolve => self.move_cursor(GridStep::Up, window, cx),
            Action::NextSolve => self.move_cursor(GridStep::Down, window, cx),
            Action::PreviousAverage => self.move_cursor(GridStep::Left, window, cx),
            Action::NextAverage => self.move_cursor(GridStep::Right, window, cx),
            Action::NextTrainerSet => self.select_trainer_set(next_set(self.trainer.set), cx),
            Action::ToggleTrainerCase => self.toggle_trainer_case(window, cx),
            Action::ToggleTrainerGroup => self.toggle_cursor_group(window, cx),
            Action::ToggleTrainerSet => self.toggle_trainer_set(window, cx),
            Action::StartTrainer | Action::OpenSolve => self.start_trainer(window, cx),
            Action::EditTrainerAlgorithm => {
                if let Some(case) = self.trainer.cursor_case() {
                    self.open_trainer_algorithm(case, window, cx);
                }
            }
            Action::SortTrainerCases => self.toggle_trainer_sort(cx),
            Action::TrainerCaseAttempts => {
                if let Some(case) = self.trainer.cursor_case() {
                    self.open_case_attempts(case, cx);
                }
            }
            _ => {}
        }
    }

    fn drill_action(&mut self, action: Action, window: &mut Window, cx: &mut Context<Self>) {
        match action {
            Action::NextScramble => self.skip_trainer_case(window, cx),
            Action::RevealTrainerCase => self.toggle_trainer_reveal(cx),
            Action::OpenSolve => {
                if let Some(index) = self.trainer.target_attempt() {
                    self.open_trainer_case(index, window, cx);
                }
            }
            Action::PreviousSolve => self.step_trainer_attempt(true, cx),
            Action::NextSolve => self.step_trainer_attempt(false, cx),
            _ => self.attempt_action(action, window, cx),
        }
    }

    fn case_attempts_action(
        &mut self,
        action: Action,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        match action {
            Action::PreviousSolve => self.step_case_attempt(false, cx),
            Action::NextSolve => self.step_case_attempt(true, cx),
            _ => self.attempt_action(action, window, cx),
        }
    }

    /// Runs what works on the target attempt, wherever it is listed.
    fn attempt_action(&mut self, action: Action, window: &mut Window, cx: &mut Context<Self>) {
        match action {
            Action::DeleteSolve => self.confirm_delete_trainer_attempt(window, cx),
            Action::PenaltyPlusTwo => self.toggle_attempt_penalty(Modifier::PlusTwo, window, cx),
            Action::PenaltyDnf => self.toggle_attempt_penalty(Modifier::DNF, window, cx),
            _ => {}
        }
    }

    /// Shows every attempt at a case in place of the picker.
    fn open_case_attempts(&mut self, case: usize, cx: &mut Context<Self>) {
        self.trainer.case_attempts = Some(CaseAttempts {
            case: &self.trainer.set.cases[case],
            row: 0,
        });
        self.trainer
            .history_scroll
            .scroll_to_item(0, ScrollStrategy::Top);
        cx.notify();
    }

    /// Goes back to the picker. Returns whether a case's attempts were showing.
    pub(crate) fn leave_case_attempts(&mut self, cx: &mut Context<Self>) -> bool {
        if self.screen != Screen::Trainer || self.trainer.case_attempts.is_none() {
            return false;
        }
        self.trainer.case_attempts = None;
        cx.notify();
        true
    }

    /// Moves the selection one row older (`older`) or newer.
    fn step_case_attempt(&mut self, older: bool, cx: &mut Context<Self>) {
        let last = self.trainer.shown_case_attempts().len().saturating_sub(1);
        let Some(shown) = &mut self.trainer.case_attempts else {
            return;
        };
        shown.row = if older {
            (shown.row + 1).min(last)
        } else {
            shown.row.saturating_sub(1)
        };
        let row = shown.row;
        self.trainer
            .history_scroll
            .scroll_to_item(row, ScrollStrategy::Nearest);
        cx.notify();
    }

    /// Moves the highlight and keeps it in view.
    fn move_cursor(&mut self, step: GridStep, window: &Window, cx: &mut Context<Self>) {
        let sizes: Vec<usize> = self
            .trainer
            .layout()
            .iter()
            .map(|group| group.cases.len())
            .collect();
        let columns = grid_columns(window);
        self.trainer.cursor = grid_step(&sizes, columns, self.trainer.cursor, step);

        let rem = window.rem_size();
        let (rows, groups) = rows_before(&sizes, columns, self.trainer.cursor);
        let headings = if self.trainer.by_slowest { 0 } else { groups };
        let row = CARD_HEIGHT.to_pixels(rem) + CARD_GAP.to_pixels(rem);
        let top = GRID_PADDING.to_pixels(rem)
            + HEADING_HEIGHT.to_pixels(rem) * headings as f32
            + row * rows as f32;
        let bottom = top + row;

        let scroll = &self.trainer.grid_scroll;
        let viewport = scroll.bounds().size.height;
        let offset = scroll.offset();
        let shown = -offset.y;
        if top < shown {
            scroll.set_offset(point(offset.x, -top));
        } else if bottom > shown + viewport {
            scroll.set_offset(point(offset.x, viewport - bottom));
        }
        cx.notify();
    }

    fn select_trainer_set(&mut self, set: &'static AlgSet, cx: &mut Context<Self>) {
        self.trainer.set = set;
        self.trainer.cursor = 0;
        self.trainer.grid_scroll.set_offset(Point::default());
        cx.notify();
    }

    /// Selects or clears the highlighted case.
    pub(crate) fn toggle_trainer_case(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if let Some(case) = self.trainer.cursor_case() {
            self.toggle_cases(&[case], window, cx);
        }
    }

    fn toggle_cases(&mut self, cases: &[usize], window: &mut Window, cx: &mut Context<Self>) {
        self.trainer.data.toggle_selected(self.trainer.set, cases);
        self.save_trainer(window, cx);
        cx.notify();
    }

    /// Selects or clears the group of the highlighted case. Sorted by slowest
    /// the groups are not on screen, but a case still belongs to one.
    fn toggle_cursor_group(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let Some(case) = self.trainer.cursor_case() else {
            return;
        };
        let set = self.trainer.set;
        let groups = set.matching(set.groups(), &self.trainer.query);
        if let Some(group) = groups.iter().find(|group| group.cases.contains(&case)) {
            self.toggle_cases(&group.cases, window, cx);
        }
    }

    /// Selects or clears the cases on screen, which a search can narrow down.
    fn toggle_trainer_set(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let layout = self.trainer.layout();
        let cases: Vec<usize> = layout.into_iter().flat_map(|group| group.cases).collect();
        self.toggle_cases(&cases, window, cx);
    }

    fn toggle_trainer_sort(&mut self, cx: &mut Context<Self>) {
        self.trainer.by_slowest = !self.trainer.by_slowest;
        self.trainer.cursor = 0;
        self.trainer.grid_scroll.set_offset(Point::default());
        cx.notify();
    }

    /// Starts drilling the selected cases.
    fn start_trainer(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let set = self.trainer.set;
        let cases = self.trainer.data.selected(set);
        let strings = self.strings();
        if cases.is_empty() {
            let notice = Notification::warning(strings.trainer_nothing_selected);
            window.push_notification(notice.placement(Anchor::BottomRight), cx);
            return;
        }
        match Drill::new(set, cases) {
            Ok(drill) => {
                self.trainer.drill = Some(ActiveDrill {
                    drill,
                    first_attempt: self.trainer.data.attempts.len(),
                    revealed: false,
                    selected: None,
                });
                self.last_result = None;
                cx.notify();
            }
            Err(error) => {
                self.report_error(strings.toast_trainer_scramble_failed, &error, window, cx);
            }
        }
    }

    /// Leaves the drill for the picker. Returns whether there was one showing.
    pub(crate) fn leave_trainer_drill(&mut self, cx: &mut Context<Self>) -> bool {
        if !self.drilling() || self.timer.is_active() {
            return false;
        }
        self.trainer.drill = None;
        self.last_result = None;
        cx.notify();
        true
    }

    /// Records a finished attempt and moves on to the next case.
    pub(crate) fn finish_trainer_attempt(
        &mut self,
        outcome: TimerOutcome,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let TimerOutcome { time_ms, modifier } = outcome;
        let Some(active) = &mut self.trainer.drill else {
            return;
        };
        let case = active.drill.case();
        let solved_at_unix_ms = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map_or(0, |elapsed| elapsed.as_millis() as u64);
        self.trainer.data.attempts.push(Attempt {
            set: active.drill.set().id.to_owned(),
            case: case.name.to_owned(),
            time_ms,
            modifier,
            scramble: active.drill.prompt().scramble.clone(),
            solved_at_unix_ms,
        });
        active.revealed = false;
        active.selected = None;
        let advanced = active.drill.advance();

        self.last_result = Some(LastResult { time_ms, modifier });
        self.trainer
            .history_scroll
            .scroll_to_item(0, ScrollStrategy::Top);
        self.save_trainer(window, cx);
        if let Err(error) = advanced {
            let template = self.strings().toast_trainer_scramble_failed;
            self.report_error(template, &error, window, cx);
        }
        cx.notify();
    }

    /// Moves on without recording anything; the case comes back this pass.
    fn skip_trainer_case(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let Some(active) = &mut self.trainer.drill else {
            return;
        };
        active.revealed = false;
        if let Err(error) = active.drill.skip() {
            let template = self.strings().toast_trainer_scramble_failed;
            self.report_error(template, &error, window, cx);
        }
        cx.notify();
    }

    fn toggle_trainer_reveal(&mut self, cx: &mut Context<Self>) {
        if let Some(active) = &mut self.trainer.drill {
            active.revealed = !active.revealed;
            cx.notify();
        }
    }

    /// Moves the attempt selection one step newer (`newer`) or older.
    fn step_trainer_attempt(&mut self, newer: bool, cx: &mut Context<Self>) {
        let Some(last) = self.trainer.data.attempts.len().checked_sub(1) else {
            return;
        };
        let Some(active) = &mut self.trainer.drill else {
            return;
        };
        let first = active.first_attempt;
        if first > last {
            return;
        }
        let index = match active.selected {
            Some(index) if newer => (index + 1).min(last),
            Some(index) => index.saturating_sub(1).max(first),
            None => last,
        };
        active.selected = Some(index);
        self.trainer
            .history_scroll
            .scroll_to_item(last - index, ScrollStrategy::Nearest);
        cx.notify();
    }

    fn toggle_attempt_penalty(
        &mut self,
        modifier: Modifier,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let Some(index) = self.trainer.target_attempt() else {
            return;
        };
        let attempt = &mut self.trainer.data.attempts[index];
        attempt.toggle_modifier(modifier);
        let (time_ms, modifier) = (attempt.time_ms, attempt.modifier);
        if index + 1 == self.trainer.data.attempts.len() && self.last_result.is_some() {
            self.last_result = Some(LastResult { time_ms, modifier });
        }
        self.save_trainer(window, cx);
        cx.notify();
    }

    /// Deletes the target attempt, asking first when confirmations are on.
    fn confirm_delete_trainer_attempt(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let Some(index) = self.trainer.target_attempt() else {
            return;
        };
        if !self.settings.confirm_delete {
            self.delete_trainer_attempt(index, window, cx);
            return;
        }
        let strings = self.strings();
        let on_confirm = cx.listener(move |app: &mut Self, (): &(), window, cx| {
            app.delete_trainer_attempt(index, window, cx);
            app.focus.focus(window, cx);
        });
        let message = strings.trainer_delete_confirm.to_owned();
        self.open_confirm(strings.delete_solve, message, on_confirm, window, cx);
    }

    fn delete_trainer_attempt(
        &mut self,
        index: usize,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if index >= self.trainer.data.attempts.len() {
            return;
        }
        let latest = index + 1 == self.trainer.data.attempts.len();
        self.trainer.data.attempts.remove(index);
        if latest {
            self.last_result = None;
        }
        if let Some(active) = &mut self.trainer.drill {
            active.selected = None;
        }
        // The row now holds the next attempt, so several can go in a row.
        let last = self.trainer.shown_case_attempts().len().saturating_sub(1);
        if let Some(shown) = &mut self.trainer.case_attempts {
            shown.row = shown.row.min(last);
        }
        self.save_trainer(window, cx);
        cx.notify();
    }

    /// Replaces a case's algorithm, or brings back the default with empty
    /// text. Returns why the text cannot be used, to show under the field.
    fn set_trainer_algorithm(
        &mut self,
        set: &'static AlgSet,
        case: &'static AlgCase,
        text: &str,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> Result<(), String> {
        let strings = self.strings();
        let algorithm = if text.is_empty() {
            None
        } else {
            check_algorithm(set, case, text).map_err(|error| match error {
                CustomAlgError::Notation(AlgError::InvalidMove(part)) => {
                    strings.trainer_algorithm_invalid.replace("{move}", &part)
                }
                CustomAlgError::Notation(AlgError::Empty) | CustomAlgError::WrongCase => strings
                    .trainer_algorithm_wrong_case
                    .replace("{case}", case.name),
            })?;
            Some(text.to_owned())
        };
        self.trainer
            .data
            .set_custom_algorithm(set, case.name, algorithm);
        self.save_trainer(window, cx);
        cx.notify();
        Ok(())
    }

    /// Opens the dialog that replaces a case's algorithm.
    fn open_trainer_algorithm(&self, case: usize, window: &mut Window, cx: &mut Context<Self>) {
        let strings = self.strings();
        let set = self.trainer.set;
        let case = &set.cases[case];
        let current = self
            .trainer
            .data
            .custom_algorithm(set, case.name)
            .unwrap_or_default()
            .to_owned();
        let input = cx.new(|cx| {
            InputState::new(window, cx)
                .default_value(current)
                .placeholder(case.algorithm)
        });
        let error = cx.new(|_| None::<String>);
        let entity = cx.entity().downgrade();
        let hint = strings
            .trainer_algorithm_hint
            .replace("{algorithm}", case.algorithm);
        let title = format!("{} · {}", strings.trainer_edit_algorithm, case.name);
        let first_build = std::cell::Cell::new(true);
        window.open_dialog(cx, move |dialog, window, cx| {
            if first_build.replace(false) {
                input.focus_handle(cx).focus(window, cx);
            }
            let theme = cx.theme();
            let muted = theme.muted_foreground;
            let red = Palette::of(cx).red;
            let message = error.read(cx).clone();
            let (input_for_save, error_for_save) = (input.clone(), error.clone());
            let entity = entity.clone();
            let save: Submit = Rc::new(move |window, cx| {
                let text = input_for_save.read(cx).value().trim().to_owned();
                let saved = entity.update(cx, |app, cx| {
                    let saved = app.set_trainer_algorithm(set, case, &text, window, cx);
                    if saved.is_ok() {
                        app.focus.focus(window, cx);
                    }
                    saved
                });
                match saved {
                    Ok(Ok(())) | Err(_) => window.close_dialog(cx),
                    Ok(Err(message)) => {
                        error_for_save.update(cx, |error, _| *error = Some(message));
                        window.refresh();
                    }
                }
            });
            dialog
                .title(dialog_title(title.clone(), strings.close))
                .close_button(false)
                .w(dialog_width(DialogSize::FORM, window))
                .submit_on_enter(&save)
                .child(
                    v_flex()
                        .gap_2()
                        .py_2()
                        .child(keep_focus(
                            "trainer-algorithm-field",
                            Input::new(&input).focus_bordered(false),
                        ))
                        .children(field_error(message.as_ref(), red))
                        .child(div().text_xs().text_color(muted).child(hint.clone())),
                )
                .footer(
                    h_flex()
                        .w_full()
                        .justify_end()
                        .gap_2()
                        .child(
                            Button::new("trainer-algorithm-cancel")
                                .outline()
                                .label(strings.cancel)
                                .on_click(|_, window, cx| window.close_dialog(cx)),
                        )
                        .child(
                            Button::new("trainer-algorithm-save")
                                .primary()
                                .label(strings.save)
                                .on_click(on_press(&save)),
                        ),
                )
        });
    }

    /// Opens the case of an attempt: its drawing, its algorithm and the
    /// scramble that attempt had.
    fn open_trainer_case(&self, index: usize, window: &mut Window, cx: &mut Context<Self>) {
        let Some(active) = &self.trainer.drill else {
            return;
        };
        let Some(attempt) = self.trainer.data.attempts.get(index) else {
            return;
        };
        let set = active.drill.set();
        let Some(case) = set.case(&attempt.case) else {
            return;
        };
        let strings = self.strings();
        let diagram = self.trainer.diagram(set, case);
        let algorithm = self.trainer.data.algorithm(set, case).to_owned();
        let scramble = attempt.scramble.clone();
        window.open_dialog(cx, move |dialog, window, cx| {
            let theme = cx.theme();
            let (muted, background) = (theme.muted_foreground, theme.background);
            let mono = theme.mono_font_family.clone();
            dialog
                .title(dialog_title(case.name, strings.close))
                .close_button(false)
                .w(dialog_width(DialogSize::FORM, window))
                .close_on_enter()
                .child(
                    FadeScroll::new("trainer-case", background)
                        .max_h(dialog_body_height(DialogSize::FORM, window))
                        .items_center()
                        .gap_3()
                        .py_2()
                        .children(
                            diagram
                                .clone()
                                .map(|image| img(image).size(REVEALED_SIZE).flex_none()),
                        )
                        .child(
                            div()
                                .text_center()
                                .font_family(mono.clone())
                                .child(algorithm.clone()),
                        )
                        .child(
                            div()
                                .text_sm()
                                .text_color(muted)
                                .child(strings.field_scramble),
                        )
                        .child(
                            div()
                                .text_center()
                                .font_family(mono)
                                .child(scramble.clone()),
                        ),
                )
                .footer(
                    h_flex().w_full().justify_end().child(
                        Button::new("trainer-case-close")
                            .primary()
                            .label(strings.close)
                            .on_click(on_press(&close_action())),
                    ),
                )
        });
    }

    /// The trainer screen: the drill under way, a case's attempts, or the
    /// picker.
    pub(crate) fn render_trainer(&self, window: &Window, cx: &mut Context<Self>) -> AnyElement {
        if self.trainer.drill.is_some() {
            self.render_drill(window, cx).into_any_element()
        } else if let Some(shown) = &self.trainer.case_attempts {
            self.render_case_attempts(shown.case, cx).into_any_element()
        } else {
            self.render_picker(window, cx).into_any_element()
        }
    }

    fn render_picker(&self, window: &Window, cx: &mut Context<Self>) -> impl IntoElement + use<> {
        h_flex()
            .flex_1()
            .min_h_0()
            .items_start()
            .child(self.render_set_list(cx))
            .child(
                v_flex()
                    .flex_1()
                    .min_w_0()
                    .h_full()
                    .child(self.render_picker_header(cx))
                    .child(self.render_case_grid(window, cx)),
            )
    }

    /// The puzzles and, under each, the sets that train it.
    fn render_set_list(&self, cx: &mut Context<Self>) -> impl IntoElement + use<> {
        let theme = cx.theme();
        let (border, muted) = (theme.border, theme.muted_foreground);
        let current = self.trainer.set.id;
        let mut list = v_flex()
            .w(NAV_WIDTH)
            .flex_none()
            .h_full()
            .p_2()
            .gap_1()
            .border_r_1()
            .border_color(border);
        for puzzle in PUZZLES {
            list = list.child(
                div()
                    .px_2()
                    .pt_1()
                    .text_xs()
                    .text_color(muted)
                    .child(puzzle.event.name()),
            );
            for &set in puzzle.sets {
                list = list.child(
                    Button::new(set.id)
                        .ghost()
                        .small()
                        .w_full()
                        .label(set.name)
                        .tab_stop(false)
                        .selected(set.id == current)
                        .on_click(cx.listener(move |app, _, window, cx| {
                            app.select_trainer_set(set, cx);
                            app.focus.focus(window, cx);
                        })),
                );
            }
        }
        list
    }

    /// The set's name, how much of it is selected, and what to do with that.
    fn render_picker_header(&self, cx: &mut Context<Self>) -> impl IntoElement + use<> {
        let strings = self.strings();
        let set = self.trainer.set;
        let language = self.language();
        let selected = self.trainer.data.selected(set).len();
        let theme = cx.theme();
        let (border, muted) = (theme.border, theme.muted_foreground);
        let count = strings
            .trainer_selected_count
            .replace("{count}", &language.format_count(selected as u64))
            .replace("{total}", &language.format_count(set.cases.len() as u64));
        h_flex()
            .flex_none()
            .w_full()
            .px_4()
            .py_2()
            .gap_2()
            .items_center()
            .border_b_1()
            .border_color(border)
            .child(
                div()
                    .flex_none()
                    .font_weight(FontWeight::SEMIBOLD)
                    .child(set.name),
            )
            .child(
                div()
                    .flex_1()
                    .min_w_0()
                    .truncate()
                    .text_sm()
                    .text_color(muted)
                    .child(count),
            )
            .child(
                div().flex_none().w(SEARCH_WIDTH).child(
                    search_field(
                        &self.trainer.search,
                        self.keybinds().label(Action::FocusSearch),
                        cx,
                    )
                    .small(),
                ),
            )
            .child(
                Button::new("trainer-toggle-all")
                    .ghost()
                    .small()
                    .icon(IconName::CircleCheck)
                    .tooltip(strings.trainer_toggle_all)
                    .accessibility_label(strings.trainer_toggle_all)
                    .tab_stop(false)
                    .on_click(cx.listener(|app, _, window, cx| {
                        app.toggle_trainer_set(window, cx);
                        app.focus.focus(window, cx);
                    })),
            )
            .child(
                Button::new("trainer-sort")
                    .ghost()
                    .small()
                    .icon(IconName::ArrowDownWideNarrow)
                    .selected(self.trainer.by_slowest)
                    .tooltip(strings.trainer_sort_slowest)
                    .accessibility_label(strings.trainer_sort_slowest)
                    .tab_stop(false)
                    .on_click(cx.listener(|app, _, window, cx| {
                        app.toggle_trainer_sort(cx);
                        app.focus.focus(window, cx);
                    })),
            )
            .child(
                Button::new("trainer-start")
                    .primary()
                    .small()
                    .label(strings.trainer_start)
                    .disabled(selected == 0)
                    .tab_stop(false)
                    .on_click(cx.listener(|app, _, window, cx| {
                        app.start_trainer(window, cx);
                        app.focus.focus(window, cx);
                    })),
            )
    }

    fn render_case_grid(
        &self,
        window: &Window,
        cx: &mut Context<Self>,
    ) -> impl IntoElement + use<> {
        let theme = cx.theme();
        let (background, muted) = (theme.background, theme.muted_foreground);
        let set = self.trainer.set;
        let stats = self.trainer.data.stats(set);
        let columns = grid_columns(window);
        let mut place = 0;
        let mut grid = FadeScroll::new("trainer-grid", background)
            .track(&self.trainer.grid_scroll)
            .flex_1()
            .min_h_0()
            .w_full()
            .p(GRID_PADDING);
        let layout = self.trainer.layout();
        if layout.is_empty() {
            grid = grid.child(
                div()
                    .text_sm()
                    .text_color(muted)
                    .child(self.strings().search_no_results),
            );
        }
        for group in layout {
            if !group.name.is_empty() {
                let cases = group.cases.clone();
                grid = grid.child(
                    h_flex()
                        .id(group.name)
                        .flex_none()
                        .h(HEADING_HEIGHT)
                        .items_center()
                        .text_sm()
                        .text_color(muted)
                        .cursor_pointer()
                        .child(group.name)
                        .on_click(cx.listener(move |app, _, window, cx| {
                            app.toggle_cases(&cases, window, cx);
                            app.focus.focus(window, cx);
                        })),
                );
            }
            for row in group.cases.chunks(columns) {
                let mut cards = h_flex().flex_none().gap(CARD_GAP).pb(CARD_GAP);
                for &case in row {
                    let stats = stats.get(set.cases[case].name).copied();
                    let stats = stats.unwrap_or_default();
                    cards = cards.child(self.render_case_card(case, place, stats, cx));
                    place += 1;
                }
                grid = grid.child(cards);
            }
        }
        grid
    }

    /// How a case has gone so far, one figure per line.
    fn case_figures(&self, stats: CaseStats, cx: &App) -> [Div; 4] {
        let strings = self.strings();
        let theme = cx.theme();
        let (muted, mono) = (theme.muted_foreground, theme.mono_font_family.clone());
        let time = |ms: Option<u64>| ms.map_or_else(|| "–".to_owned(), format_ms);
        let attempts = self.language().format_count(stats.attempts as u64);
        [
            stat_line(strings.trainer_attempts, attempts, muted, mono.clone()),
            stat_line(
                strings.trainer_best,
                time(stats.best_ms),
                muted,
                mono.clone(),
            ),
            stat_line(strings.stat_mean, time(stats.mean_ms), muted, mono.clone()),
            stat_line(strings.trainer_recent, time(stats.recent_ms), muted, mono),
        ]
    }

    fn render_case_card(
        &self,
        index: usize,
        place: usize,
        stats: CaseStats,
        cx: &mut Context<Self>,
    ) -> impl IntoElement + use<> {
        let strings = self.strings();
        let set = self.trainer.set;
        let case = &set.cases[index];
        let selected = self.trainer.data.is_selected(set, case.name);
        let highlighted = place == self.trainer.cursor;
        let theme = cx.theme();
        let (border, accent) = (theme.border, theme.ring);
        let (selected_bg, hover) = (theme.accent, theme.muted);

        v_flex()
            .id(("trainer-case", index))
            .relative()
            .flex_none()
            .w(CARD_WIDTH)
            .h(CARD_HEIGHT)
            .p_2()
            .gap_1()
            .items_center()
            .rounded_md()
            .border_1()
            .border_color(if selected { accent } else { border })
            .when(highlighted, |card| card.border_2().border_color(accent))
            .when(selected, |card| card.bg(selected_bg))
            .when(!selected, |card| card.hover(move |card| card.bg(hover)))
            .cursor_pointer()
            .on_click(cx.listener(move |app, _, window, cx| {
                app.trainer.cursor = place;
                app.toggle_trainer_case(window, cx);
                app.focus.focus(window, cx);
            }))
            .children(
                self.trainer
                    .diagram(set, case)
                    .map(|image| img(image).size(DIAGRAM_SIZE).flex_none()),
            )
            .child(
                div()
                    .flex_none()
                    .w_full()
                    .truncate()
                    .text_center()
                    .text_sm()
                    .font_weight(FontWeight::SEMIBOLD)
                    .child(case.name),
            )
            .children(self.case_figures(stats, cx))
            .child(
                div().absolute().top_1().left_1().child(
                    Button::new(("trainer-case-attempts", index))
                        .ghost()
                        .xsmall()
                        .icon(IconName::Timer)
                        .tooltip(strings.trainer_case_attempts)
                        .accessibility_label(strings.trainer_case_attempts)
                        .tab_stop(false)
                        .on_click(cx.listener(move |app, _, window, cx| {
                            cx.stop_propagation();
                            app.trainer.cursor = place;
                            app.open_case_attempts(index, cx);
                            app.focus.focus(window, cx);
                        })),
                ),
            )
            .child(
                div().absolute().top_1().right_1().child(
                    Button::new(("trainer-edit", index))
                        .ghost()
                        .xsmall()
                        .icon(IconName::Pencil)
                        .tooltip(strings.trainer_edit_algorithm)
                        .accessibility_label(strings.trainer_edit_algorithm)
                        .tab_stop(false)
                        .on_click(cx.listener(move |app, _, window, cx| {
                            cx.stop_propagation();
                            app.open_trainer_algorithm(index, window, cx);
                        })),
                ),
            )
    }

    /// A case and how it has gone, beside the list of all its attempts.
    fn render_case_attempts(
        &self,
        case: &'static AlgCase,
        cx: &mut Context<Self>,
    ) -> impl IntoElement + use<> {
        let strings = self.strings();
        let set = self.trainer.set;
        let theme = cx.theme();
        let (border, background) = (theme.border, theme.background);
        let mono = theme.mono_font_family.clone();
        let stats = self.trainer.data.stats(set);
        let stats = stats.get(case.name).copied().unwrap_or_default();

        let summary = FadeScroll::new("trainer-case-summary", background)
            .w(SUMMARY_WIDTH)
            .flex_none()
            .h_full()
            .p_3()
            .gap_2()
            .items_center()
            .children(
                self.trainer
                    .diagram(set, case)
                    .map(|image| img(image).size(REVEALED_SIZE).flex_none()),
            )
            .child(
                div()
                    .text_center()
                    .text_sm()
                    .font_family(mono)
                    .child(self.trainer.data.algorithm(set, case).to_owned()),
            )
            .children(self.case_figures(stats, cx));

        v_flex()
            .flex_1()
            .min_h_0()
            .w_full()
            .child(
                h_flex()
                    .flex_none()
                    .p_2()
                    .gap_1()
                    .items_center()
                    .border_b_1()
                    .border_color(border)
                    .child(
                        Button::new("trainer-attempts-back")
                            .ghost()
                            .small()
                            .icon(IconName::ChevronLeft)
                            .tooltip(strings.trainer_back)
                            .accessibility_label(strings.trainer_back)
                            .tab_stop(false)
                            .on_click(cx.listener(|app, _, window, cx| {
                                app.leave_case_attempts(cx);
                                app.focus.focus(window, cx);
                            })),
                    )
                    .child(
                        div()
                            .flex_1()
                            .min_w_0()
                            .truncate()
                            .font_weight(FontWeight::SEMIBOLD)
                            .child(format!("{} · {}", set.name, case.name)),
                    ),
            )
            .child(
                h_flex()
                    .flex_1()
                    .min_h_0()
                    .w_full()
                    .child(summary)
                    .child(div().flex_none().w_px().h_full().bg(border))
                    .child(self.render_case_attempt_list(cx)),
            )
    }

    /// Every attempt at the case on screen, or a note that there are none.
    fn render_case_attempt_list(&self, cx: &mut Context<Self>) -> AnyElement {
        let strings = self.strings();
        let theme = cx.theme();
        let (muted, background) = (theme.muted_foreground, theme.background);
        let count = self.trainer.shown_case_attempts().len();
        if count == 0 {
            div()
                .flex_1()
                .flex()
                .items_center()
                .justify_center()
                .p_3()
                .text_sm()
                .text_color(muted)
                .child(strings.trainer_no_attempts)
                .into_any_element()
        } else {
            let scroll = &self.trainer.history_scroll;
            div()
                .relative()
                .flex_1()
                .min_w_0()
                .h_full()
                .child(
                    fading_scroll(
                        uniform_list(
                            "trainer-case-attempts",
                            count,
                            cx.processor(Self::render_case_attempt_rows),
                        )
                        .size_full()
                        .track_scroll(scroll),
                        scroll,
                        background,
                    )
                    .size_full(),
                )
                .child(Scrollbar::vertical(scroll).mode(self.scrollbar_mode()))
                .into_any_element()
        }
    }

    /// Row `0` is the newest attempt. Each row says when it was.
    fn render_case_attempt_rows(
        &mut self,
        range: Range<usize>,
        _: &mut Window,
        cx: &mut Context<Self>,
    ) -> Vec<AnyElement> {
        let colors = Palette::of(cx);
        let theme = cx.theme();
        let (muted, selected_bg) = (theme.muted_foreground, theme.accent);
        let mono = theme.mono_font_family.clone();
        let language = self.language();
        let strings = self.strings();
        let attempts = self.trainer.shown_case_attempts();
        let total = attempts.len();
        let selected = self.trainer.case_attempts.as_ref().map(|shown| shown.row);
        let number = number_width(total, language);

        range
            .filter_map(|row| {
                let index = *attempts.get(row)?;
                let attempt = &self.trainer.data.attempts[index];
                let color = match attempt.modifier {
                    Modifier::DNF => Some(colors.red),
                    Modifier::PlusTwo => Some(colors.yellow),
                    Modifier::None => None,
                };
                Some(
                    h_flex()
                        .w_full()
                        .h(ROW_HEIGHT)
                        .px_3()
                        .gap_2()
                        .text_sm()
                        .when(selected == Some(row), |line| line.bg(selected_bg))
                        .child(
                            div()
                                .w(number)
                                .flex_none()
                                .whitespace_nowrap()
                                .font_family(mono.clone())
                                .text_xs()
                                .text_color(muted)
                                .child(language.format_count((total - row) as u64)),
                        )
                        .child(
                            div()
                                .flex_1()
                                .min_w_0()
                                .truncate()
                                .text_color(muted)
                                .child(language.format_date(attempt.solved_at_unix_ms)),
                        )
                        .child(
                            div()
                                .flex_none()
                                .whitespace_nowrap()
                                .font_family(mono.clone())
                                .when_some(color, gpui_kit::Styled::text_color)
                                .child(attempt_text(attempt)),
                        )
                        .child(
                            Button::new(("trainer-case-attempt-delete", index))
                                .ghost()
                                .xsmall()
                                .icon(IconName::Trash)
                                .tooltip(strings.delete_solve)
                                .accessibility_label(strings.delete_solve)
                                .tab_stop(false)
                                .on_click(cx.listener(move |app, _, window, cx| {
                                    cx.stop_propagation();
                                    app.select_case_attempt(row);
                                    app.focus.focus(window, cx);
                                    app.confirm_delete_trainer_attempt(window, cx);
                                })),
                        )
                        .into_any_element(),
                )
            })
            .collect()
    }

    fn select_case_attempt(&mut self, row: usize) {
        if let Some(shown) = &mut self.trainer.case_attempts {
            shown.row = row;
        }
    }

    /// The history of the drill beside the timer panel.
    fn render_drill(&self, window: &Window, cx: &mut Context<Self>) -> impl IntoElement + use<> {
        let attempt = self.timer.is_active();
        let (width, _) = self.panel_widths(window, cx);
        h_flex()
            .relative()
            .flex_1()
            .min_h_0()
            // A panel's edge can be dragged past the body, so the body hears
            // the drag and the release wherever they happen.
            .on_drag_move(cx.listener(Self::on_panel_drag))
            .on_mouse_up(MouseButton::Left, cx.listener(Self::on_panel_release))
            .on_mouse_up_out(MouseButton::Left, cx.listener(Self::on_panel_release))
            .when(!attempt, |row| {
                row.child(self.render_drill_history(width, cx))
            })
            .child(self.render_timer(window, cx))
            .when(!attempt, |row| {
                row.child(render_grab(PanelSide::Sidebar, width, cx))
            })
    }

    /// The way back to the picker, the set's name, and what can be done to
    /// the waiting case.
    fn render_drill_header(
        &self,
        name: &'static str,
        cx: &mut Context<Self>,
    ) -> impl IntoElement + use<> {
        let strings = self.strings();
        h_flex()
            .p_2()
            .gap_1()
            .items_center()
            .child(
                Button::new("trainer-back")
                    .ghost()
                    .small()
                    .icon(IconName::ChevronLeft)
                    .tooltip(strings.trainer_back)
                    .accessibility_label(strings.trainer_back)
                    .tab_stop(false)
                    .on_click(cx.listener(|app, _, window, cx| {
                        app.leave_trainer_drill(cx);
                        app.focus.focus(window, cx);
                    })),
            )
            .child(
                div()
                    .flex_1()
                    .min_w_0()
                    .truncate()
                    .font_weight(FontWeight::SEMIBOLD)
                    .child(name),
            )
            .child(
                Button::new("trainer-reveal")
                    .ghost()
                    .small()
                    .icon(IconName::Eye)
                    .selected(self.trainer_revealed())
                    .tooltip(strings.trainer_reveal)
                    .accessibility_label(strings.trainer_reveal)
                    .tab_stop(false)
                    .on_click(cx.listener(|app, _, window, cx| {
                        app.toggle_trainer_reveal(cx);
                        app.focus.focus(window, cx);
                    })),
            )
            .child(
                Button::new("trainer-skip")
                    .ghost()
                    .small()
                    .icon(IconName::Redo)
                    .tooltip(strings.trainer_skip)
                    .accessibility_label(strings.trainer_skip)
                    .tab_stop(false)
                    .on_click(cx.listener(|app, _, window, cx| {
                        app.skip_trainer_case(window, cx);
                        app.focus.focus(window, cx);
                    })),
            )
    }

    fn render_drill_history(
        &self,
        width: Pixels,
        cx: &mut Context<Self>,
    ) -> impl IntoElement + use<> {
        let strings = self.strings();
        let theme = cx.theme();
        let (border, muted, background) = (theme.border, theme.muted_foreground, theme.background);
        let count = self.trainer.drill_attempts().len();
        let language = self.language();
        let (name, progress) = self.trainer.drill.as_ref().map_or_else(
            || ("", String::new()),
            |active| {
                let (current, total) = active.drill.progress();
                (
                    active.drill.set().name,
                    strings
                        .trainer_progress
                        .replace("{current}", &language.format_count(current as u64))
                        .replace("{total}", &language.format_count(total as u64)),
                )
            },
        );

        v_flex()
            .w(width)
            .flex_shrink_0()
            .h_full()
            .border_r_1()
            .border_color(border)
            .child(self.render_drill_header(name, cx))
            .child(
                div()
                    .px_3()
                    .py_2()
                    .border_t_1()
                    .border_color(border)
                    .text_xs()
                    .text_color(muted)
                    .truncate()
                    .child(progress),
            )
            .child(if count == 0 {
                div()
                    .flex_1()
                    .flex()
                    .items_center()
                    .justify_center()
                    .p_3()
                    .text_sm()
                    .text_color(muted)
                    .child(strings.trainer_no_attempts)
                    .into_any_element()
            } else {
                let scroll = &self.trainer.history_scroll;
                div()
                    .relative()
                    .flex_1()
                    .min_h_0()
                    .child(
                        fading_scroll(
                            uniform_list(
                                "trainer-history",
                                count,
                                cx.processor(Self::render_attempt_rows),
                            )
                            .size_full()
                            .track_scroll(scroll),
                            scroll,
                            background,
                        )
                        .size_full(),
                    )
                    .child(Scrollbar::vertical(scroll).mode(self.scrollbar_mode()))
                    .into_any_element()
            })
    }

    /// Row `0` is the newest attempt. Each row names the case it was.
    fn render_attempt_rows(
        &mut self,
        range: Range<usize>,
        _: &mut Window,
        cx: &mut Context<Self>,
    ) -> Vec<AnyElement> {
        let colors = Palette::of(cx);
        let theme = cx.theme();
        let (muted, hover, selected_bg) = (theme.muted_foreground, theme.muted, theme.accent);
        let mono = theme.mono_font_family.clone();
        let language = self.language();
        let attempts = self.trainer.drill_attempts();
        let total = attempts.len();
        let first = self.trainer.data.attempts.len() - total;
        let selected = self.trainer.drill.as_ref().and_then(|drill| drill.selected);
        let number = number_width(total, language);

        range
            .filter_map(|row| {
                let place = total.checked_sub(row + 1)?;
                let attempt = &attempts[place];
                let index = first + place;
                let color = match attempt.modifier {
                    Modifier::DNF => Some(colors.red),
                    Modifier::PlusTwo => Some(colors.yellow),
                    Modifier::None => None,
                };
                Some(
                    h_flex()
                        .id(("trainer-attempt", index))
                        .w_full()
                        .h(ROW_HEIGHT)
                        .px_3()
                        .gap_2()
                        .text_sm()
                        .cursor_pointer()
                        .when(selected == Some(index), |row| row.bg(selected_bg))
                        .hover(move |row| row.bg(hover))
                        .child(
                            div()
                                .w(number)
                                .flex_none()
                                .whitespace_nowrap()
                                .font_family(mono.clone())
                                .text_xs()
                                .text_color(muted)
                                .child(language.format_count((place + 1) as u64)),
                        )
                        .child(
                            div()
                                .flex_1()
                                .min_w_0()
                                .truncate()
                                .child(attempt.case.clone()),
                        )
                        .child(
                            div()
                                .flex_none()
                                .whitespace_nowrap()
                                .font_family(mono.clone())
                                .when_some(color, gpui_kit::Styled::text_color)
                                .child(attempt_text(attempt)),
                        )
                        .on_click(cx.listener(move |app, _, window, cx| {
                            if let Some(active) = &mut app.trainer.drill {
                                active.selected = Some(index);
                                app.open_trainer_case(index, window, cx);
                                cx.notify();
                            }
                        }))
                        .into_any_element(),
                )
            })
            .collect()
    }

    /// What the timer panel adds during a drill: the waiting case once it is
    /// revealed. Nothing else, so no averages or stats.
    pub(crate) fn render_revealed_case(&self) -> Option<Div> {
        let active = self.trainer.drill.as_ref().filter(|_| self.drilling())?;
        active.revealed.then(|| {
            let case = active.drill.case();
            v_flex()
                .flex_none()
                .max_w_full()
                .items_center()
                .gap_1()
                .children(
                    self.trainer
                        .diagram(active.drill.set(), case)
                        .map(|image| img(image).size(REVEALED_SIZE).flex_none()),
                )
                .child(div().font_weight(FontWeight::SEMIBOLD).child(case.name))
        })
    }
}
