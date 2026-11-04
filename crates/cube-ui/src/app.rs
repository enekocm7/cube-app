//! Application state and the operations on it. Views live in sibling modules.

use std::path::{Path, PathBuf};
use std::rc::Rc;
use std::sync::Arc;
use std::time::{Duration, Instant};

use cube_core::cstimer::{self, ImportReport};
use cube_core::scramble::{self, ScrambleQueue};
use cube_core::stats::{self, Average, SessionStats};
use cube_core::{
    History, Modifier, Settings, Solve, SolveSnapshot, Timer, TimerOutcome, TimerPhase, WcaEvent,
    format::format_ms, persistence,
};
use cube_locale::{Language, Strings};
use cube_updater::Release;
use gpui_kit::assets::IconName;
use gpui_kit::component::notification::Notification;
use gpui_kit::component::{ActiveTheme as _, TitleBar, WindowExt as _, h_flex, v_flex};
use gpui_kit::prelude::FluentBuilder as _;
use gpui_kit::*;

use crate::palette;
use crate::panel_resize::{PanelSide, render_grab};
use crate::stats_screen::ChartData;

/// The most sessions cube-tui can display.
pub const MAX_SESSIONS: usize = 99;

/// How often the display refreshes during an attempt.
const ACTIVE_TICK: Duration = Duration::from_millis(16);
/// How often background work is polled otherwise.
const IDLE_TICK: Duration = Duration::from_millis(100);

/// The timer key.
const TIMER_KEY: &str = "space";

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
    queue: ScrambleQueue,
    pub(crate) timer: Timer,
    /// Whether the timer key is physically down, to ignore key repeat.
    key_down: bool,
    pub(crate) last_result: Option<LastResult>,
    pub(crate) stats: SessionStats,
    chart_data: Option<Rc<ChartData>>,
    pub(crate) solves_scroll: UniformListScrollHandle,
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
            queue: ScrambleQueue::new(event),
            key_down: false,
            last_result: None,
            chart_data: None,
            solves_scroll: UniformListScrollHandle::new(),
            history_path,
            settings_path,
            saving_blocked: load_error.is_some(),
            available: None,
            update_checking: false,
            updating: false,
            update_ticker: None,
            panel_resizing: false,
        };
        if let Some(error) = load_error {
            let message = app.strings().toast_load_failed.replace("{error}", &error);
            // The notification layer belongs to the root view, which wraps this
            // one and does not exist until construction finishes.
            window.on_next_frame(move |window, cx| {
                window.push_notification(Notification::error(message).autohide(false), cx);
            });
        }
        app.start_ticking(window, cx);
        app.startup_update_tasks(window, cx);
        cx.observe_window_activation(window, Self::on_activation_changed)
            .detach();
        app
    }

    /// Returns the active language's strings.
    pub(crate) fn strings(&self) -> &'static Strings {
        Language::from_code(&self.settings.language)
            .or_else(|| sys_locale::get_locale().and_then(|code| Language::from_code(&code)))
            .unwrap_or_default()
            .strings()
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

    /// Drives time-based state: inspection expiry, display updates and scrambles.
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
        if self.scramble.is_none()
            && let Some(scramble) = self.queue.try_next()
        {
            self.scramble = Some(scramble);
            cx.notify();
        }
        if self.timer.is_active() {
            cx.notify();
            ACTIVE_TICK
        } else {
            IDLE_TICK
        }
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
        match event.keystroke.key.as_str() {
            TIMER_KEY => {
                cx.stop_propagation();
                if self.key_down || event.is_held || self.screen != Screen::Timer {
                    return;
                }
                self.key_down = true;
                self.press_timer(window, cx);
            }
            "escape" if self.timer.is_active() => {
                cx.stop_propagation();
                self.timer.cancel();
                self.toast(self.strings().toast_cancelled, window, cx);
                cx.notify();
            }
            _ => {}
        }
    }

    pub(crate) fn on_key_up(
        &mut self,
        event: &KeyUpEvent,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if event.keystroke.key != TIMER_KEY {
            return;
        }
        self.key_down = false;
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

    /// A key released while the window is inactive never reaches us, so a press
    /// in progress must not outlive the window's focus.
    fn on_activation_changed(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if !window.is_window_active() {
            self.key_down = false;
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
        self.last_result = Some(LastResult { time_ms, modifier });
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
            self.scramble = None;
            cx.notify();
        }
    }

    /// Switches event, discarding scrambles for the previous one.
    pub(crate) fn set_event(&mut self, event: WcaEvent, cx: &mut Context<Self>) {
        if event != self.event && !self.timer.is_active() {
            self.event = event;
            self.scramble = None;
            self.queue = ScrambleQueue::new(event);
            cx.notify();
        }
    }

    pub(crate) fn select_session(&mut self, index: usize, cx: &mut Context<Self>) {
        if index >= self.sessions.len() || self.timer.is_active() {
            return;
        }
        self.current = index;
        self.last_result = None;
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
        if index < self.sessions.len() {
            self.sessions.remove(index);
            let next = self.current.min(self.sessions.len() - 1);
            self.current = next;
            self.select_session(next, cx);
            self.save_history(window, cx);
        }
    }

    /// Toggles a penalty on a solve of the current session.
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
        if self.sessions[self.current].remove(index).is_some() {
            self.last_result = None;
            self.solves_changed(window, cx);
            self.toast(self.strings().solve_deleted, window, cx);
            cx.notify();
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

    /// Changes settings, then applies and saves them.
    pub(crate) fn update_settings(
        &mut self,
        window: &mut Window,
        cx: &mut Context<Self>,
        change: impl FnOnce(&mut Settings),
    ) {
        change(&mut self.settings);
        if !self.timer.is_active() {
            self.timer = self.settings.timer();
        }
        scramble::set_accurate_3x3(self.settings.accurate_3x3);
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

    fn render_title_bar(&self, cx: &mut Context<Self>) -> impl IntoElement + use<> {
        use gpui_kit::component::Selectable as _;
        use gpui_kit::component::Sizable as _;
        use gpui_kit::component::button::{Button, ButtonVariants as _};

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
                .w_full()
                .pr_2()
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
                .child(
                    div().occlude().child(
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
                    .child(self.render_timer(cx))
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
            .child(self.render_title_bar(cx))
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
