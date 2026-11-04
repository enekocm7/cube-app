//! Application state and the operations on it. Views live in sibling modules.

use std::path::{Path, PathBuf};
use std::rc::Rc;
use std::sync::Arc;
use std::time::{Duration, Instant};

use cube_core::cstimer::{self, ImportReport};
use cube_core::scramble::{self, ScrambleQueue};
use cube_core::stats::{self, Average, SessionStats};
use cube_core::window_state::WindowState;
use cube_core::{
    Action, BindingError, History, KeyBinding, KeyPress, Keybinds, ManualEntry, Modifier, Settings,
    Solve, SolveSnapshot, Timer, TimerOutcome, TimerPhase, WcaEvent, format::format_ms,
    persistence,
};
use cube_locale::{Language, Strings};
use cube_updater::Release;
use gpui_kit::assets::IconName;
use gpui_kit::component::Selectable as _;
use gpui_kit::component::Sizable as _;
use gpui_kit::component::button::{Button, ButtonVariants as _};
use gpui_kit::component::menu::{DropdownMenu as _, PopupMenuItem};
use gpui_kit::component::notification::Notification;
use gpui_kit::component::{ActiveTheme as _, Icon, TitleBar, WindowExt as _, h_flex, v_flex};
use gpui_kit::prelude::FluentBuilder as _;
use gpui_kit::*;

use crate::palette;
use crate::panel_resize::{PanelSide, render_grab};
use crate::preview::PreviewCache;
use crate::stats_screen::ChartData;

/// The most sessions cube-tui can display.
pub const MAX_SESSIONS: usize = 99;

/// How often the display refreshes during an attempt.
const ACTIVE_TICK: Duration = Duration::from_millis(16);
/// How often time-based state is checked otherwise. Scrambles do not wait for
/// this: they arrive through `ScrambleSignal`.
const IDLE_TICK: Duration = Duration::from_millis(100);
/// How long a scramble may take before the placeholder message shows.
const GENERATING_DELAY: Duration = Duration::from_millis(500);

/// The top-level screens.
#[derive(Clone, Copy, PartialEq, Eq)]
pub enum Screen {
    Timer,
    Stats,
}

/// Everything read from disk at startup.
pub struct Loaded {
    pub settings: Settings,
    sessions: Vec<History>,
    history_path: Option<PathBuf>,
    settings_path: Option<PathBuf>,
    /// Why the history could not be loaded. Saving stays off in that case, so a
    /// file this version cannot read is never overwritten.
    load_error: Option<String>,
}

impl Loaded {
    pub fn load() -> Self {
        let history_path = persistence::history_file().ok();
        let settings_path = persistence::settings_file().ok();
        // cube-tui keeps its themes here; creating the default one gives both
        // apps a theme to start from.
        let _ = persistence::ensure_default_theme();
        let (sessions, load_error) = match history_path.as_deref().map(persistence::load_history) {
            Some(Ok(sessions)) => (sessions.unwrap_or_default(), None),
            Some(Err(error)) => (Vec::new(), Some(format!("{error:#}"))),
            None => (
                Vec::new(),
                Some("The data directory is unavailable.".to_owned()),
            ),
        };
        let settings = settings_path
            .as_deref()
            .and_then(|path| persistence::load_settings(path).ok())
            .unwrap_or_default();
        Self {
            settings,
            sessions,
            history_path,
            settings_path,
            load_error,
        }
    }
}

/// The result shown on the timer once an attempt ends.
#[derive(Clone, Copy)]
pub struct LastResult {
    pub time_ms: u64,
    pub modifier: Modifier,
}

/// The application window's root view.
#[allow(
    clippy::struct_excessive_bools,
    reason = "independent timer, saving and update states"
)]
pub struct CubeApp {
    pub(crate) focus: FocusHandle,
    pub(crate) screen: Screen,
    pub(crate) settings: Settings,
    pub(crate) sessions: Vec<History>,
    pub(crate) current: usize,
    pub(crate) event: WcaEvent,
    pub(crate) scramble: Option<String>,
    /// The time being typed while manual mode is on.
    pub(crate) manual: Option<ManualEntry>,
    /// The drawing of `scramble`, kept while the same one is on screen.
    pub(crate) preview: PreviewCache,
    /// Whether the wait for a scramble has outlasted `GENERATING_DELAY`, so
    /// the placeholder message may show.
    pub(crate) generating_visible: bool,
    /// Reveals the placeholder after the delay. Replacing or dropping it
    /// cancels the pending reveal, so an older wait cannot outlive a newer one.
    generating_timer: Option<Task<()>>,
    queue: ScrambleQueue,
    pub(crate) timer: Timer,
    /// Whether the timer key is physically down, to ignore key repeat.
    key_down: bool,
    /// Whether the mouse or a touch is holding the digits, the pointer twin of
    /// `key_down`.
    pointer_down: bool,
    /// The key that stopped a running timer, so its held repeats do not also
    /// trigger that key's action until it is released.
    stop_repeat_key: Option<String>,
    pub(crate) last_result: Option<LastResult>,
    pub(crate) stats: SessionStats,
    chart_data: Option<Rc<ChartData>>,
    pub(crate) solves_scroll: UniformListScrollHandle,
    /// The solve selected with the keyboard, as an index into the session.
    pub(crate) selected_solve: Option<usize>,
    keybinds: Keybinds,
    /// Deletions Undo can restore, oldest first.
    undo_stack: Vec<Deleted>,
    /// Undone deletions Redo can repeat, until a new deletion clears them.
    redo_stack: Vec<Deleted>,
    history_path: Option<PathBuf>,
    settings_path: Option<PathBuf>,
    saving_blocked: bool,
    /// The newer release the update notification offers, if any.
    pub(crate) available: Option<Arc<Release>>,
    pub(crate) update_checking: bool,
    pub(crate) updating: bool,
    pub(crate) update_ticker: Option<Task<()>>,
    /// Whether a panel edge has been dragged since the last release, so the
    /// new widths are saved once the mouse goes up.
    pub(crate) panel_resizing: bool,
    /// Pending save of the window bounds, replaced while the window is dragged.
    window_save: Option<Task<()>>,
}

/// The padding `TitleBar` leaves before its content (`gpui-kit` keeps it private).
#[cfg(target_os = "macos")]
const TITLE_BAR_LEFT_PADDING: Pixels = px(80.);
#[cfg(not(target_os = "macos"))]
const TITLE_BAR_LEFT_PADDING: Pixels = px(12.);

/// How many deletions Undo remembers.
const UNDO_LIMIT: usize = 100;

/// Something deleted that can still be brought back.
enum Deleted {
    Solve {
        session: usize,
        index: usize,
        solve: Solve,
    },
    Session {
        index: usize,
        history: History,
    },
}

impl CubeApp {
    pub fn new(loaded: Loaded, window: &mut Window, cx: &mut Context<Self>) -> Self {
        let Loaded {
            mut settings,
            mut sessions,
            history_path,
            settings_path,
            load_error,
        } = loaded;
        settings.adopt_existing_history(&sessions);
        if sessions.is_empty() {
            sessions.push(History::new());
        }
        scramble::set_accurate_3x3(settings.accurate_3x3);
        let keybinds = settings.keybinds();

        let event = session_event(&sessions[0]);
        let focus = cx.focus_handle();
        focus.focus(window, cx);
        let mut app = Self {
            focus,
            screen: Screen::Timer,
            timer: settings.timer(),
            stats: SessionStats::compute(sessions[0].solves()),
            settings,
            sessions,
            current: 0,
            event,
            scramble: None,
            manual: None,
            preview: PreviewCache::default(),
            generating_visible: false,
            generating_timer: None,
            queue: ScrambleQueue::new(event),
            key_down: false,
            pointer_down: false,
            stop_repeat_key: None,
            last_result: None,
            chart_data: None,
            solves_scroll: UniformListScrollHandle::new(),
            selected_solve: None,
            undo_stack: Vec::new(),
            redo_stack: Vec::new(),
            keybinds,
            history_path,
            settings_path,
            saving_blocked: load_error.is_some(),
            available: None,
            update_checking: false,
            updating: false,
            update_ticker: None,
            panel_resizing: false,
            window_save: None,
        };
        if let Some(error) = load_error {
            let message = app.strings().toast_load_failed.replace("{error}", &error);
            // The notification layer belongs to the root view, which wraps this
            // one and does not exist until construction finishes.
            window.on_next_frame(move |window, cx| {
                window.push_notification(Notification::error(message).autohide(false), cx);
            });
        }
        app.next_scramble(cx);
        app.listen_for_scrambles(cx);
        app.start_ticking(window, cx);
        app.startup_update_tasks(window, cx);
        cx.observe_window_activation(window, Self::on_activation_changed)
            .detach();
        cx.observe_window_bounds(window, Self::on_window_bounds_changed)
            .detach();
        app
    }

    /// Remembers the window's bounds. Dragging reports every frame, so the file
    /// is written once the window has stopped changing.
    fn on_window_bounds_changed(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let (bounds, maximized) = match window.window_bounds() {
            WindowBounds::Windowed(bounds) | WindowBounds::Fullscreen(bounds) => (bounds, false),
            WindowBounds::Maximized(bounds) => (bounds, true),
        };
        let state = WindowState {
            x: f32::from(bounds.origin.x).round() as i32,
            y: f32::from(bounds.origin.y).round() as i32,
            width: f32::from(bounds.size.width).round().max(0.) as u32,
            height: f32::from(bounds.size.height).round().max(0.) as u32,
            maximized,
        };
        if self.settings.window == Some(state) {
            return;
        }
        self.settings.window = Some(state);
        self.window_save = Some(cx.spawn_in(window, async move |this, cx| {
            cx.background_executor()
                .timer(Duration::from_millis(500))
                .await;
            this.update_in(cx, |this, window, cx| this.save_settings(window, cx))
                .ok();
        }));
    }

    /// Returns the active language's strings.
    pub(crate) fn strings(&self) -> &'static Strings {
        Language::from_code(&self.settings.language)
            .or_else(|| sys_locale::get_locale().and_then(|code| Language::from_code(&code)))
            .unwrap_or_default()
            .strings()
    }

    /// Returns the effective keyboard bindings.
    pub(crate) fn keybinds(&self) -> &Keybinds {
        &self.keybinds
    }

    pub(crate) fn session(&self) -> &History {
        &self.sessions[self.current]
    }

    /// Returns the name to show for the session at `index`.
    pub(crate) fn session_label(&self, index: usize) -> String {
        match self.sessions[index].name() {
            "" => self
                .strings()
                .session_default_name
                .replace("{number}", &(index + 1).to_string()),
            name => name.to_owned(),
        }
    }

    /// Drives time-based state: inspection expiry and display updates.
    fn start_ticking(&self, window: &mut Window, cx: &mut Context<Self>) {
        cx.spawn_in(window, async move |this, cx| {
            let mut delay = IDLE_TICK;
            loop {
                cx.background_executor().timer(delay).await;
                let updated = this.update_in(cx, CubeApp::tick);
                match updated {
                    Ok(next_delay) => delay = next_delay,
                    Err(_) => break,
                }
            }
        })
        .detach();
    }

    fn tick(&mut self, window: &mut Window, cx: &mut Context<Self>) -> Duration {
        let now = Instant::now();
        self.play_inspection_cues(now, window, cx);
        if let Some(outcome) = self.timer.tick(now) {
            self.finish_attempt(outcome, window, cx);
        }
        if self.timer.is_active() {
            cx.notify();
            ACTIVE_TICK
        } else {
            IDLE_TICK
        }
    }

    /// Shows a scramble the moment the worker delivers one, if none is showing.
    fn listen_for_scrambles(&self, cx: &mut Context<Self>) {
        let mut signal = self.queue.signal();
        cx.spawn(async move |this, cx| {
            while signal.changed().await {
                let updated = this.update(cx, |app, cx| {
                    if app.scramble.is_none()
                        && let Some(scramble) = app.queue.try_next()
                    {
                        app.scramble = Some(scramble);
                        app.generating_visible = false;
                        app.generating_timer = None;
                        cx.notify();
                    }
                });
                if updated.is_err() {
                    break;
                }
            }
        })
        .detach();
    }

    /// Moves to the next scramble, right away if one is prepared. Otherwise the
    /// line stays blank until `listen_for_scrambles` delivers one, and only if
    /// none arrives within `GENERATING_DELAY` does the placeholder appear.
    fn next_scramble(&mut self, cx: &mut Context<Self>) {
        self.scramble = self.queue.try_next();
        self.generating_visible = false;
        self.generating_timer = self.scramble.is_none().then(|| {
            cx.spawn(async move |this, cx| {
                cx.background_executor().timer(GENERATING_DELAY).await;
                this.update(cx, |app, cx| {
                    if app.scramble.is_none() {
                        app.generating_visible = true;
                        cx.notify();
                    }
                })
                .ok();
            })
        });
    }

    /// Plays any inspection warnings whose time has come, without interrupting
    /// the attempt.
    fn play_inspection_cues(&mut self, now: Instant, window: &mut Window, cx: &mut Context<Self>) {
        for cue in self.timer.take_inspection_cues(now).into_iter().flatten() {
            if !self.settings.inspection_audio {
                continue;
            }
            if let Err(error) = crate::audio::play(cue) {
                let message = self
                    .strings()
                    .toast_audio_failed
                    .replace("{error}", &format!("{error:#}"));
                self.toast_warning(&message, window, cx);
            }
        }
    }

    pub(crate) fn on_key_down(
        &mut self,
        event: &KeyDownEvent,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        // Dialogs handle their own keys, including ones typed into inputs.
        if window.has_active_dialog(cx) {
            return;
        }
        let Some(press) = key_press(&event.keystroke) else {
            return;
        };
        // Holding the key that stopped the timer must not trigger its action.
        if event.is_held && self.stop_repeat_key.as_deref() == Some(press.key()) {
            return;
        }

        // Many cubers expect any key to stop the timer, not just the one that
        // started it. Modifiers alone are ignored so the stop is deliberate.
        if matches!(self.timer.phase(), TimerPhase::Running { .. }) {
            if event.keystroke.is_modifier_key() || event.is_held {
                return;
            }
            cx.stop_propagation();
            self.key_down = false;
            self.stop_repeat_key = Some(press.key().to_owned());
            if self.keybinds.action_for(&press) == Some(Action::Cancel) {
                self.cancel(window, cx);
            } else if let Some(outcome) = self.timer.press(Instant::now()) {
                self.finish_attempt(outcome, window, cx);
            }
            return;
        }

        if self.manual_typing(&press, &event.keystroke, event.is_held, window, cx) {
            cx.stop_propagation();
            return;
        }

        let Some(action) = self.keybinds.action_for(&press) else {
            return;
        };
        cx.stop_propagation();
        if action == Action::Timer {
            // Manual mode types times in instead of timing them.
            if self.manual.is_some() {
                return;
            }
            if self.key_down || self.pointer_down || event.is_held || self.screen != Screen::Timer {
                return;
            }
            self.key_down = true;
            self.press_timer(window, cx);
            return;
        }
        if action == Action::Cancel {
            self.cancel(window, cx);
            return;
        }
        if self.timer.is_active() || (event.is_held && !action.repeats()) {
            return;
        }
        match action {
            Action::PreviousSolve
            | Action::NextSolve
            | Action::OpenSolve
            | Action::DeleteSolve
            | Action::PenaltyPlusTwo
            | Action::PenaltyDnf
                if self.screen != Screen::Timer => {}
            Action::NextScramble => self.skip_scramble(cx),
            Action::ToggleInspection => self.toggle_inspection(window, cx),
            Action::ToggleScramblePreview => self.toggle_scramble_preview(window, cx),
            Action::ManualEntry => self.toggle_manual_entry(cx),
            Action::PreviousSolve => self.select_solve(true, cx),
            Action::NextSolve => self.select_solve(false, cx),
            Action::OpenSolve => {
                if let Some(index) = self.target_solve() {
                    self.open_solve_details(index, window, cx);
                }
            }
            Action::DeleteSolve => {
                if let Some(index) = self.target_solve() {
                    self.confirm_delete_solve(index, window, cx);
                }
            }
            Action::PenaltyPlusTwo => self.toggle_target_penalty(Modifier::PlusTwo, window, cx),
            Action::PenaltyDnf => self.toggle_target_penalty(Modifier::DNF, window, cx),
            Action::NextEvent => self.cycle_event(1, cx),
            Action::PreviousEvent => self.cycle_event(-1, cx),
            Action::NextSession => self.cycle_session(1, cx),
            Action::PreviousSession => self.cycle_session(-1, cx),
            Action::NewSession => self.add_session(window, cx),
            Action::RenameSession => self.open_rename_session(window, cx),
            Action::DeleteSession => self.confirm_delete_session(window, cx),
            Action::UndoDelete => self.undo_delete(window, cx),
            Action::RedoDelete => self.redo_delete(window, cx),
            Action::OpenStats => {
                self.screen = if self.screen == Screen::Stats {
                    Screen::Timer
                } else {
                    Screen::Stats
                };
                cx.notify();
            }
            Action::OpenSettings => self.open_settings(window, cx),
            Action::ShowHelp => self.open_help(window, cx),
            Action::Timer | Action::Cancel => {}
        }
    }

    pub(crate) fn on_key_up(
        &mut self,
        event: &KeyUpEvent,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let Some(press) = key_press(&event.keystroke) else {
            return;
        };
        if self.stop_repeat_key.as_deref() == Some(press.key()) {
            self.stop_repeat_key = None;
        }
        if self.keybinds.action_for(&press) != Some(Action::Timer) {
            return;
        }
        self.key_down = false;
        self.release_timer(window, cx);
    }

    /// Starts or stops the timer with the mouse or a touch on the digits, the
    /// same way the timer key does.
    pub(crate) fn on_pointer_down(
        &mut self,
        _event: &MouseDownEvent,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if window.has_active_dialog(cx)
            || self.pointer_down
            || self.key_down
            || self.manual.is_some()
        {
            return;
        }
        // Stopping needs no matching release, so a press that stopped the
        // timer is not tracked and its release is ignored.
        self.pointer_down = !matches!(self.timer.phase(), TimerPhase::Running { .. });
        self.press_timer(window, cx);
        // Keep keyboard shortcuts working after a click.
        window.focus(&self.focus, cx);
        cx.stop_propagation();
    }

    pub(crate) fn on_pointer_up(
        &mut self,
        _event: &MouseUpEvent,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if !self.pointer_down {
            return;
        }
        self.pointer_down = false;
        self.release_timer(window, cx);
    }

    /// Handles the timer key or pointer going up.
    fn release_timer(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let was_inspecting = matches!(self.timer.phase(), TimerPhase::Inspecting { .. });
        let outcome = self.timer.release(Instant::now());
        if matches!(self.timer.phase(), TimerPhase::Running { .. }) {
            self.mark_start_hint_seen(window, cx);
        }
        if let Some(outcome) = outcome {
            self.finish_attempt(outcome, window, cx);
        } else if was_inspecting
            && let TimerPhase::Running {
                modifier: Modifier::PlusTwo,
                ..
            } = self.timer.phase()
        {
            self.toast(self.strings().toast_plus_two, window, cx);
        }
        cx.notify();
    }

    /// Cancels the current attempt, or leaves the stats screen when idle.
    fn cancel(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if self.timer.is_active() {
            self.timer.cancel();
            self.toast(self.strings().toast_cancelled, window, cx);
            cx.notify();
        } else if self.screen != Screen::Timer {
            self.screen = Screen::Timer;
            cx.notify();
        }
    }

    /// A key released while the window is inactive never reaches us, so a press
    /// in progress must not outlive the window's focus.
    fn on_activation_changed(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if !window.is_window_active() {
            self.key_down = false;
            self.pointer_down = false;
            self.stop_repeat_key = None;
            if matches!(self.timer.phase(), TimerPhase::Holding { .. }) {
                self.timer.cancel();
                cx.notify();
            }
        }
    }

    fn press_timer(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        // An attempt needs a scramble to record against.
        if !self.timer.is_active() && self.scramble.is_none() {
            return;
        }
        if !self.timer.is_active() {
            self.last_result = None;
        }
        if let Some(outcome) = self.timer.press(Instant::now()) {
            self.finish_attempt(outcome, window, cx);
        }
        cx.notify();
    }

    /// Records a finished attempt and moves on to the next scramble.
    fn finish_attempt(
        &mut self,
        outcome: TimerOutcome,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let TimerOutcome { time_ms, modifier } = outcome;
        let scramble = self.scramble.take().unwrap_or_default();
        let solve = Solve::new(time_ms, self.event, scramble, modifier);
        self.sessions[self.current].push(solve);
        self.next_scramble(cx);
        self.last_result = Some(LastResult { time_ms, modifier });
        self.selected_solve = None;
        self.solves_changed(window, cx);
        self.solves_scroll.scroll_to_item(0, ScrollStrategy::Top);

        if time_ms == 0 && modifier == Modifier::DNF {
            self.toast(self.strings().toast_dnf, window, cx);
        }
        self.announce_records(window, cx);
        cx.notify();
    }

    /// Reports every statistic the newest solve improved.
    fn announce_records(&self, window: &mut Window, cx: &mut Context<Self>) {
        let solves = self.session().solves();
        let Some(latest_index) = solves.len().checked_sub(1) else {
            return;
        };
        let strings = self.strings();
        let announce = |label: String, ms: u64, window: &mut Window, cx: &mut Context<Self>| {
            let message = strings
                .toast_new_best
                .replace("{label}", &label)
                .replace("{time}", &format_ms(ms));
            self.toast(&message, window, cx);
        };

        if solves[latest_index].was_single_record()
            && let Some(ms) = solves[latest_index].effective_ms()
        {
            announce(strings.stat_single.to_lowercase(), ms, window, cx);
        }
        for window_stats in &self.stats.windows {
            // `best` keeps the earliest of equal windows, so a tie never lands here.
            if let Some((Average::Time(ms), index)) = window_stats.best
                && index == latest_index
            {
                announce(average_label(window_stats.size), ms, window, cx);
            }
        }
    }

    pub(crate) fn toast(&self, message: &str, window: &mut Window, cx: &mut Context<Self>) {
        if self.settings.toasts {
            let toast = Notification::info(message.to_owned()).placement(Anchor::BottomRight);
            window.push_notification(toast, cx);
        }
    }

    /// Shows a warning without changing what the timer is doing.
    pub(crate) fn toast_warning(&self, message: &str, window: &mut Window, cx: &mut Context<Self>) {
        if self.settings.toasts {
            let toast = Notification::warning(message.to_owned()).placement(Anchor::BottomRight);
            window.push_notification(toast, cx);
        }
    }

    /// Replaces the scramble without recording anything.
    pub(crate) fn skip_scramble(&mut self, cx: &mut Context<Self>) {
        if !self.timer.is_active() {
            self.next_scramble(cx);
            cx.notify();
        }
    }

    /// Switches event, keeping the previous one's scramble for coming back.
    pub(crate) fn set_event(&mut self, event: WcaEvent, cx: &mut Context<Self>) {
        if event != self.event && !self.timer.is_active() {
            self.event = event;
            self.queue.switch_event(event, self.scramble.take());
            self.next_scramble(cx);
            cx.notify();
        }
    }

    /// Moves to the next (`delta` 1) or previous (`delta` -1) event.
    pub(crate) fn cycle_event(&mut self, delta: isize, cx: &mut Context<Self>) {
        if self.timer.is_active() {
            return;
        }
        let count = WcaEvent::ALL.len();
        let current = WcaEvent::ALL
            .iter()
            .position(|&event| event == self.event)
            .unwrap_or(0);
        let step = delta.unsigned_abs() % count;
        let index = if delta >= 0 {
            (current + step) % count
        } else {
            (current + count - step) % count
        };
        self.set_event(WcaEvent::ALL[index], cx);
    }

    /// Moves to the next (`delta` 1) or previous (`delta` -1) session.
    pub(crate) fn cycle_session(&mut self, delta: isize, cx: &mut Context<Self>) {
        if self.sessions.len() <= 1 || self.timer.is_active() {
            return;
        }
        let count = self.sessions.len();
        let step = delta.unsigned_abs() % count;
        let index = if delta >= 0 {
            (self.current + step) % count
        } else {
            (self.current + count - step) % count
        };
        self.select_session(index, cx);
    }

    /// Turns WCA inspection on or off, saving the setting.
    pub(crate) fn toggle_inspection(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if !self.timer.is_active() {
            self.update_settings(window, cx, |settings| {
                settings.inspection = !settings.inspection;
            });
        }
    }

    /// Shows or hides the scramble preview, saving the setting.
    pub(crate) fn toggle_scramble_preview(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        self.update_settings(window, cx, |settings| {
            settings.scramble_preview = !settings.scramble_preview;
        });
    }

    /// Turns manual mode on or off. It starts only from the idle timer
    /// screen, and the scramble stays until a typed time is recorded.
    pub(crate) fn toggle_manual_entry(&mut self, cx: &mut Context<Self>) {
        if self.manual.take().is_none() {
            if self.timer.is_active() || self.screen != Screen::Timer {
                return;
            }
            self.manual = Some(ManualEntry::default());
        }
        cx.notify();
    }

    /// Feeds a key to the time being typed in manual mode. Returns whether the
    /// key was used; digits, `:`, `.`, Backspace, paste and Enter (once
    /// something is typed) are, so every other binding keeps working.
    fn manual_typing(
        &mut self,
        press: &KeyPress,
        keystroke: &Keystroke,
        held: bool,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> bool {
        let typing = self.screen == Screen::Timer && !self.timer.is_active();
        let Some(entry) = self.manual.as_mut().filter(|_| typing) else {
            return false;
        };
        if self.keybinds.action_for(press) == Some(Action::ManualEntry) {
            return false;
        }
        let modifiers = keystroke.modifiers;
        let changed = if modifiers.control || modifiers.platform || modifiers.alt {
            if (modifiers.control || modifiers.platform) && press.key() == "v" {
                cx.read_from_clipboard()
                    .and_then(|item| item.text())
                    .is_some_and(|text| entry.push_text(&text))
            } else {
                return false;
            }
        } else {
            match press.key() {
                "backspace" => entry.backspace(),
                "enter" if !entry.is_empty() => {
                    self.record_manual(window, cx);
                    return true;
                }
                _ if held => return false,
                _ => keystroke
                    .key_char
                    .as_deref()
                    .is_some_and(|text| entry.push_text(text)),
            }
        };
        if changed {
            cx.notify();
        }
        changed
    }

    /// Records the typed time like a timed solve, and stays in manual mode for
    /// the next one. Penalties then apply to it like to any other solve.
    fn record_manual(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        // A scramble is still being prepared: keep the text for the next Enter.
        if self.scramble.is_none() {
            return;
        }
        let Some(time_ms) = self.manual.as_mut().and_then(ManualEntry::commit) else {
            cx.notify();
            return;
        };
        self.finish_attempt(
            TimerOutcome {
                time_ms,
                modifier: Modifier::None,
            },
            window,
            cx,
        );
        if let Some(entry) = &mut self.manual {
            entry.clear();
        }
    }

    /// Moves the solve selection one step newer (`newer`) or older.
    pub(crate) fn select_solve(&mut self, newer: bool, cx: &mut Context<Self>) {
        let count = self.session().len();
        if count == 0 {
            return;
        }
        let index = match self.selected_solve.map(|index| index.min(count - 1)) {
            Some(index) if newer => (index + 1).min(count - 1),
            Some(index) => index.saturating_sub(1),
            None => count - 1,
        };
        self.selected_solve = Some(index);
        self.solves_scroll
            .scroll_to_item(count - 1 - index, ScrollStrategy::Nearest);
        cx.notify();
    }

    /// Returns the solve an action applies to: the selection, or the latest.
    pub(crate) fn target_solve(&self) -> Option<usize> {
        let count = self.session().len();
        (count > 0).then(|| self.selected_solve.unwrap_or(count - 1).min(count - 1))
    }

    pub(crate) fn select_session(&mut self, index: usize, cx: &mut Context<Self>) {
        if index >= self.sessions.len() || self.timer.is_active() {
            return;
        }
        self.current = index;
        self.last_result = None;
        self.selected_solve = None;
        self.refresh_stats();
        let event = session_event(self.session());
        self.set_event(event, cx);
        cx.notify();
    }

    pub(crate) fn add_session(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if self.sessions.len() >= MAX_SESSIONS {
            self.toast(self.strings().max_sessions, window, cx);
            return;
        }
        self.sessions.push(History::new());
        self.select_session(self.sessions.len() - 1, cx);
        self.save_history(window, cx);
    }

    pub(crate) fn rename_session(
        &mut self,
        index: usize,
        name: &str,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if let Some(session) = self.sessions.get_mut(index) {
            session.set_name(name.trim().to_owned());
            self.save_history(window, cx);
            cx.notify();
        }
    }

    pub(crate) fn delete_session(
        &mut self,
        index: usize,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if self.sessions.len() <= 1 {
            self.toast(self.strings().last_session, window, cx);
            return;
        }
        if let Some(history) = self.remove_session(index, window, cx) {
            self.record_deletion(Deleted::Session { index, history }, window, cx);
        }
    }

    /// Removes a session and fixes the selection; the last one is kept.
    fn remove_session(
        &mut self,
        index: usize,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> Option<History> {
        if self.sessions.len() <= 1 || index >= self.sessions.len() {
            return None;
        }
        let history = self.sessions.remove(index);
        // Removing an entry above the open one shifts it up a place.
        let shifted = if index < self.current {
            self.current - 1
        } else {
            self.current
        };
        let next = shifted.min(self.sessions.len() - 1);
        self.current = next;
        self.select_session(next, cx);
        self.save_history(window, cx);
        Some(history)
    }

    /// Toggles a penalty on a solve of the current session.
    /// Toggles a penalty on the selected solve, or the latest.
    fn toggle_target_penalty(
        &mut self,
        modifier: Modifier,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if let Some(index) = self.target_solve() {
            self.toggle_penalty(index, modifier, window, cx);
        }
    }

    pub(crate) fn toggle_penalty(
        &mut self,
        index: usize,
        modifier: Modifier,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if self.sessions[self.current].toggle_modifier(index, modifier) {
            self.solves_changed(window, cx);
            self.sync_last_result();
            cx.notify();
        }
    }

    /// Applies an edit to a solve of the current session.
    pub(crate) fn edit_solve(
        &mut self,
        index: usize,
        after: SolveSnapshot,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if self.sessions[self.current].edit(index, after) {
            self.solves_changed(window, cx);
            self.sync_last_result();
            cx.notify();
        }
    }

    pub(crate) fn delete_solve(
        &mut self,
        index: usize,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let session = self.current;
        if let Some(solve) = self.remove_solve(session, index, window, cx) {
            self.record_deletion(
                Deleted::Solve {
                    session,
                    index,
                    solve,
                },
                window,
                cx,
            );
        }
    }

    /// Removes a solve and fixes the selection.
    fn remove_solve(
        &mut self,
        session: usize,
        index: usize,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> Option<Solve> {
        let solve = self.sessions.get_mut(session)?.remove(index)?;
        if session != self.current {
            self.select_session(session, cx);
        }
        self.last_result = None;
        let count = self.session().len();
        self.selected_solve = match self.selected_solve {
            Some(selected) if selected > index => Some(selected - 1),
            Some(selected) if selected == index => (count > 0).then(|| index.min(count - 1)),
            selected => selected,
        };
        self.solves_changed(window, cx);
        cx.notify();
        Some(solve)
    }

    /// Remembers a new deletion, drops the redo history and offers Undo.
    fn record_deletion(&mut self, deleted: Deleted, window: &mut Window, cx: &mut Context<Self>) {
        self.redo_stack.clear();
        self.push_undo(deleted, window, cx);
    }

    fn push_undo(&mut self, deleted: Deleted, window: &mut Window, cx: &mut Context<Self>) {
        let strings = self.strings();
        let message = match deleted {
            Deleted::Solve { .. } => strings.solve_deleted,
            Deleted::Session { .. } => strings.session_deleted,
        };
        self.undo_stack.push(deleted);
        if self.undo_stack.len() > UNDO_LIMIT {
            self.undo_stack.remove(0);
        }
        self.offer_undo(message, window, cx);
    }

    /// Shows a toast with an Undo button.
    ///
    /// Shown even with toasts turned off: it is the only way back besides
    /// the keyboard shortcut.
    fn offer_undo(&self, message: &str, window: &mut Window, cx: &mut Context<Self>) {
        let strings = self.strings();
        let app = cx.entity();
        let toast = Notification::info(message.to_owned())
            .placement(Anchor::BottomRight)
            .autohide(true)
            .action(move |_, _, cx| {
                let (app, note) = (app.clone(), cx.entity());
                gpui_kit::component::button::Button::new("undo-delete")
                    .label(strings.undo)
                    .on_click(move |_, window, cx| {
                        app.update(cx, |app, cx| app.undo_delete(window, cx));
                        note.update(cx, |note, cx| note.dismiss(window, cx));
                    })
            });
        window.push_notification(toast, cx);
    }

    /// Restores the most recent deletion that can still be restored.
    pub(crate) fn undo_delete(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        while let Some(deleted) = self.undo_stack.pop() {
            match deleted {
                Deleted::Solve {
                    session,
                    index,
                    solve,
                } => {
                    if session >= self.sessions.len() {
                        continue;
                    }
                    let index = index.min(self.sessions[session].len());
                    self.sessions[session].insert(index, solve.clone());
                    if session != self.current {
                        self.select_session(session, cx);
                    }
                    self.selected_solve = Some(index);
                    self.redo_stack.push(Deleted::Solve {
                        session,
                        index,
                        solve,
                    });
                    self.solves_changed(window, cx);
                }
                Deleted::Session { index, history } => {
                    if self.sessions.len() >= MAX_SESSIONS {
                        self.undo_stack.push(Deleted::Session { index, history });
                        self.toast(self.strings().max_sessions, window, cx);
                        return;
                    }
                    let index = index.min(self.sessions.len());
                    self.redo_stack.push(Deleted::Session {
                        index,
                        history: history.clone(),
                    });
                    self.sessions.insert(index, history);
                    self.select_session(index, cx);
                    self.save_history(window, cx);
                }
            }
            self.sync_last_result();
            cx.notify();
            return;
        }
    }

    /// Repeats the most recently undone deletion.
    pub(crate) fn redo_delete(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        while let Some(deleted) = self.redo_stack.pop() {
            let redone = match deleted {
                Deleted::Solve { session, index, .. } => self
                    .remove_solve(session, index, window, cx)
                    .map(|solve| Deleted::Solve {
                        session,
                        index,
                        solve,
                    }),
                Deleted::Session { index, .. } => self
                    .remove_session(index, window, cx)
                    .map(|history| Deleted::Session { index, history }),
            };
            if let Some(redone) = redone {
                self.push_undo(redone, window, cx);
                return;
            }
        }
    }

    /// Imports a csTimer export, adding its sessions to the history.
    pub(crate) fn import_cstimer(
        &mut self,
        path: &Path,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        match cstimer::import(path) {
            Ok(report) => self.add_imported(report, window, cx),
            Err(error) => self.report_error(self.strings().toast_import_failed, &error, window, cx),
        }
    }

    /// Writes every session to a csTimer export.
    pub(crate) fn export_cstimer(&self, path: &Path, window: &mut Window, cx: &mut Context<Self>) {
        match cstimer::export(path, &self.sessions) {
            Ok(path) => {
                let message = self
                    .strings()
                    .toast_export_done
                    .replace("{path}", &path.display().to_string());
                self.toast(&message, window, cx);
            }
            Err(error) => self.report_error(self.strings().toast_export_failed, &error, window, cx),
        }
    }

    /// Appends imported sessions, replacing an untouched default session.
    fn add_imported(&mut self, report: ImportReport, window: &mut Window, cx: &mut Context<Self>) {
        let ImportReport {
            sessions,
            mut imported,
            mut skipped,
        } = report;
        if !sessions.is_empty() {
            // An untouched first session is only the empty default; replacing
            // it keeps imported sessions from starting at "Session 2".
            let first = if self.sessions.len() == 1
                && self.sessions[0].is_empty()
                && self.sessions[0].name().is_empty()
            {
                self.sessions.clear();
                0
            } else {
                self.sessions.len()
            };
            let mut added = false;
            for (index, session) in sessions.into_iter().enumerate() {
                if first + index < MAX_SESSIONS {
                    self.sessions.push(session);
                    added = true;
                } else {
                    imported = imported.saturating_sub(session.len());
                    skipped += session.len();
                }
            }
            if added {
                self.current = first.min(self.sessions.len() - 1);
                self.last_result = None;
                self.selected_solve = None;
                self.refresh_stats();
                let event = session_event(self.session());
                self.set_event(event, cx);
                self.save_history(window, cx);
            }
        }

        let message = self
            .strings()
            .toast_import_done
            .replace("{imported}", &imported.to_string())
            .replace("{skipped}", &skipped.to_string());
        self.toast(&message, window, cx);
        cx.notify();
    }

    /// Reports a failed file picker.
    pub(crate) fn report_import_error(
        &self,
        error: &anyhow::Error,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        self.report_error(self.strings().toast_import_failed, error, window, cx);
    }

    /// Reports a failed file picker.
    pub(crate) fn report_export_error(
        &self,
        error: &anyhow::Error,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        self.report_error(self.strings().toast_export_failed, error, window, cx);
    }

    /// Keeps the timer display in step with an edit to the newest solve.
    fn sync_last_result(&mut self) {
        if self.last_result.is_some()
            && let Some(solve) = self.session().solves().last()
        {
            self.last_result = Some(LastResult {
                time_ms: solve.raw_ms(),
                modifier: solve.modifier(),
            });
        }
    }

    /// Remembers that the start hint is no longer needed, from the first
    /// attempt on.
    fn mark_start_hint_seen(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if self.settings.start_hint_seen {
            return;
        }
        self.settings.start_hint_seen = true;
        if let Some(path) = &self.settings_path
            && let Err(error) = persistence::save_settings(path, &self.settings)
        {
            self.report_save_error(&error, window, cx);
        }
    }

    /// Replaces an action's keyboard binding, or removes it when `None`.
    ///
    /// The change is validated first, so a conflicting or unusable key is
    /// rejected before anything is saved.
    pub(crate) fn set_keybinding(
        &mut self,
        action: Action,
        binding: Option<KeyBinding>,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> Result<(), BindingError> {
        match &binding {
            Some(binding) => self.keybinds.validate(action, binding)?,
            None => self.keybinds.validate_removal(action)?,
        }
        let text = binding.map_or_else(String::new, |binding| binding.to_string());
        self.update_settings(window, cx, |settings| {
            settings.keybindings.insert(action.name().to_owned(), text);
        });
        Ok(())
    }

    /// Changes settings, then applies and saves them.
    pub(crate) fn update_settings(
        &mut self,
        window: &mut Window,
        cx: &mut Context<Self>,
        change: impl FnOnce(&mut Settings),
    ) {
        let accurate_3x3 = self.settings.accurate_3x3;
        change(&mut self.settings);
        self.keybinds = self.settings.keybinds();
        if !self.timer.is_active() {
            self.timer = self.settings.timer();
        }
        if self.settings.accurate_3x3 != accurate_3x3 {
            scramble::set_accurate_3x3(self.settings.accurate_3x3);
            self.queue.discard(WcaEvent::Cube3x3);
        }
        palette::apply(&self.settings, cx);
        self.save_settings(window, cx);
        window.refresh();
        cx.notify();
    }

    /// Writes the current settings to disk.
    pub(crate) fn save_settings(&self, window: &mut Window, cx: &mut Context<Self>) {
        if let Some(path) = &self.settings_path
            && let Err(error) = persistence::save_settings(path, &self.settings)
        {
            self.report_save_error(&error, window, cx);
        }
    }

    fn solves_changed(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        self.refresh_stats();
        self.save_history(window, cx);
    }

    fn refresh_stats(&mut self) {
        self.stats = SessionStats::compute(self.session().solves());
        self.chart_data = None;
    }

    /// Returns the current session's chart series, building them on first use.
    pub(crate) fn chart_data(&mut self) -> Rc<ChartData> {
        let solves = self.sessions[self.current].solves();
        Rc::clone(
            self.chart_data
                .get_or_insert_with(|| Rc::new(ChartData::new(solves))),
        )
    }

    fn save_history(&self, window: &mut Window, cx: &mut Context<Self>) {
        if self.saving_blocked {
            return;
        }
        if let Some(path) = &self.history_path
            && let Err(error) = persistence::save_history(path, &self.sessions)
        {
            self.report_save_error(&error, window, cx);
        }
    }

    fn report_save_error(
        &self,
        error: &anyhow::Error,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        self.report_error(self.strings().toast_save_failed, error, window, cx);
    }

    /// Shows `template`, with `{error}` filled in, as an error notification.
    fn report_error(
        &self,
        template: &'static str,
        error: &anyhow::Error,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let message = template.replace("{error}", &format!("{error:#}"));
        window.push_notification(Notification::error(message), cx);
    }

    /// The current event and the control to skip its scramble.
    pub(crate) fn render_event_picker(&self, cx: &mut Context<Self>) -> impl IntoElement + use<> {
        let strings = self.strings();
        let entity = cx.entity().downgrade();
        let event = self.event;

        // The picker alone is centered; the scramble button hangs off its right edge
        // so it does not pull the picker off the middle.
        // Full bar height, so the scramble button centers on the bar and not on the
        // picker's own box.
        h_flex()
            .relative()
            .flex_none()
            .h_full()
            .items_center()
            .child(
                div().occlude().flex_none().child(
                    Button::new("event")
                        .text()
                        // An equal spacer balances the arrow, so the label itself is what
                        // sits in the middle.
                        .child(
                            h_flex()
                                .gap_1()
                                .items_center()
                                .text_base()
                                .child(div().w(px(14.)))
                                .child(event.name())
                                .child(Icon::new(IconName::ChevronDown).size(px(14.))),
                        )
                        .tab_stop(false)
                        .dropdown_menu(move |mut menu, _, _| {
                            for item in WcaEvent::ALL {
                                let entity = entity.clone();
                                menu = menu.item(
                                    PopupMenuItem::new(item.name())
                                        .checked(item == event)
                                        .on_click(move |_, _, cx| {
                                            entity
                                                .update(cx, |app, cx| app.set_event(item, cx))
                                                .ok();
                                        }),
                                );
                            }
                            menu
                        }),
                ),
            )
            .child(
                div()
                    .absolute()
                    .left_full()
                    .top_0()
                    .bottom_0()
                    .ml_1()
                    .child(
                        h_flex().occlude().h_full().items_center().child(
                            Button::new("next-scramble")
                                .ghost()
                                .small()
                                .flex_none()
                                .icon(IconName::Redo)
                                .tooltip(strings.new_scramble)
                                .tab_stop(false)
                                .on_click(cx.listener(|app, _, window, cx| {
                                    app.skip_scramble(cx);
                                    app.focus.focus(window, cx);
                                })),
                        ),
                    ),
            )
    }

    fn render_title_bar(
        &self,
        window: &Window,
        cx: &mut Context<Self>,
    ) -> impl IntoElement + use<> {
        let strings = self.strings();
        let tab =
            |id: &'static str, label: &'static str, screen: Screen, cx: &mut Context<Self>| {
                Button::new(id)
                    .ghost()
                    .small()
                    .label(label)
                    .tab_stop(false)
                    .selected(self.screen == screen)
                    .on_click(cx.listener(move |app, _, window, cx| {
                        if !app.timer.is_active() {
                            app.screen = screen;
                            cx.notify();
                        }
                        app.focus.focus(window, cx);
                    }))
            };
        TitleBar::new().child(
            h_flex()
                .relative()
                .w_full()
                .justify_between()
                .child(
                    h_flex()
                        .gap_2()
                        .child(img(logo()).size(px(18.)))
                        .child(
                            div()
                                .text_sm()
                                .font_weight(FontWeight::SEMIBOLD)
                                .child("cube"),
                        )
                        .child(
                            // The title bar is a drag region; only occluded children get clicks.
                            h_flex()
                                .occlude()
                                .ml_4()
                                .gap_1()
                                .child(tab("nav-timer", strings.nav_timer, Screen::Timer, cx))
                                .child(tab("nav-stats", strings.nav_stats, Screen::Stats, cx)),
                        ),
                )
                // Centered on the window, not on the space the tabs and the window
                // controls leave: the overlay starts where the title bar's own left
                // padding ends and is as wide as the window. It lets clicks through;
                // only the group is occluded.
                .child(
                    h_flex()
                        .absolute()
                        .top_0()
                        .bottom_0()
                        .left(-TITLE_BAR_LEFT_PADDING)
                        .w(window.viewport_size().width)
                        .justify_center()
                        .items_center()
                        .child(self.render_event_picker(cx)),
                )
                .child(
                    h_flex()
                        .occlude()
                        .mr_2()
                        .gap_1()
                        .child(
                            Button::new("open-help")
                                .ghost()
                                .small()
                                .icon(IconName::Keyboard)
                                .tooltip(strings.help_title)
                                .accessibility_label(strings.help_title)
                                .tab_stop(false)
                                .on_click(cx.listener(|app, _, window, cx| {
                                    if app.timer.is_active() {
                                        app.focus.focus(window, cx);
                                    } else {
                                        app.open_help(window, cx);
                                    }
                                })),
                        )
                        .child(
                            Button::new("open-settings")
                                .ghost()
                                .small()
                                .icon(IconName::Settings)
                                .tooltip(strings.settings)
                                .accessibility_label(strings.settings)
                                .tab_stop(false)
                                .on_click(cx.listener(|app, _, window, cx| {
                                    if app.timer.is_active() {
                                        app.focus.focus(window, cx);
                                    } else {
                                        app.open_settings(window, cx);
                                    }
                                })),
                        ),
                ),
        )
    }
}

impl Render for CubeApp {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let attempt = self.timer.is_active();
        let body = match self.screen {
            Screen::Timer => {
                let (sidebar_width, stats_width) = self.panel_widths(window);
                h_flex()
                    .relative()
                    .flex_1()
                    .min_h_0()
                    // A panel's edge can be dragged past the body, so the body hears the
                    // drag and the release wherever they happen.
                    .on_drag_move(cx.listener(Self::on_panel_drag))
                    .on_mouse_up(MouseButton::Left, cx.listener(Self::on_panel_release))
                    .on_mouse_up_out(MouseButton::Left, cx.listener(Self::on_panel_release))
                    .when(!attempt, |row| {
                        row.child(self.render_sidebar(sidebar_width, cx))
                    })
                    .child(self.render_timer(window, cx))
                    .when(!attempt, |row| {
                        row.child(self.render_stats_panel(stats_width, cx))
                            // The grips go last so they sit above the panels they straddle.
                            .child(render_grab(PanelSide::Sidebar, sidebar_width, cx))
                            .child(render_grab(PanelSide::Stats, stats_width, cx))
                    })
                    .into_any_element()
            }
            Screen::Stats => self.render_stats_screen(cx).into_any_element(),
        };
        let theme = cx.theme();
        v_flex()
            .id("cube")
            .track_focus(&self.focus)
            .size_full()
            .bg(theme.background)
            .text_color(theme.foreground)
            .on_key_down(cx.listener(Self::on_key_down))
            .on_key_up(cx.listener(Self::on_key_up))
            .child(self.render_title_bar(window, cx))
            .child(body)
    }
}

/// Returns the short name of an average: `mo3` for three, `aoN` otherwise.
pub(crate) fn average_label(size: usize) -> String {
    if size == 3 {
        "mo3".to_owned()
    } else {
        format!("ao{size}")
    }
}

/// Formats a mean or average, using an en dash when there is none yet.
pub(crate) fn format_average(average: Option<Average>) -> String {
    match average {
        Some(Average::Time(ms)) => format_ms(ms),
        Some(Average::Dnf) => "DNF".to_owned(),
        None => "–".to_owned(),
    }
}

/// Returns the rolling average of `size` ending at `index`, formatted.
pub(crate) fn format_average_at(solves: &[Solve], index: usize, size: usize) -> String {
    format_average(stats::average_at(solves, index, size))
}

/// Converts a platform keystroke into the shared, normalized model.
pub(crate) fn key_press(keystroke: &Keystroke) -> Option<KeyPress> {
    KeyPress::new(
        &keystroke.key,
        keystroke.modifiers.control || keystroke.modifiers.platform,
        keystroke.modifiers.alt,
        keystroke.modifiers.shift,
    )
}

/// A session resumes on the event of its latest solve.
fn session_event(history: &History) -> WcaEvent {
    history
        .solves()
        .last()
        .map_or(WcaEvent::Cube3x3, Solve::event)
}

/// The app logo, from the same SVG as the executable's icon.
fn logo() -> std::sync::Arc<Image> {
    static LOGO: std::sync::OnceLock<std::sync::Arc<Image>> = std::sync::OnceLock::new();
    std::sync::Arc::clone(LOGO.get_or_init(|| {
        std::sync::Arc::new(Image::from_bytes(
            ImageFormat::Svg,
            include_bytes!("../../../assets/logo.svg").to_vec(),
        ))
    }))
}
