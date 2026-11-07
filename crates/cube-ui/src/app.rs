//! Application state and the operations on it. Views live in sibling modules.

use std::borrow::Cow;
use std::cell::Cell;
use std::path::{Path, PathBuf};
use std::rc::Rc;
use std::sync::Arc;
use std::time::{Duration, Instant};

use chrono::{Local, NaiveDate};
use cube_bluetooth::Client;
use cube_core::bests::{self, BestScope};
use cube_core::blind_stats::BlindStats;
use cube_core::charts::{ChartFilter, ChartView, ChartZoom};
use cube_core::cstimer::{self, ImportReport};
use cube_core::goals;
use cube_core::history_writer::{HistoryWriter, Merge, WriterEvent};
use cube_core::library::LibraryData;
use cube_core::multi_blind::MultiBlindData;
use cube_core::persistence::{self, HistoryStore};
use cube_core::relay::RelayData;
use cube_core::scramble::{self, ScrambleQueue};
use cube_core::stats::{self, Average, SessionStats};
use cube_core::tags::{self, TagUsage};
use cube_core::trainer::TrainerData;
use cube_core::window_state::WindowState;
use cube_core::{
    Action, BindingError, Goal, GoalProgress, History, KeyBinding, KeyPress, Keybinds, ManualEntry,
    MergeSummary, Modifier, Settings, Solve, SolveSnapshot, TagChange, Timer, TimerOutcome,
    TimerPhase, WcaEvent, format::format_ms,
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

use crate::bluetooth::TimerLink;
use crate::charts_screen::{ChartData, KEY_ZOOM};
use crate::library_screen::{LibraryState, new_library_search};
use crate::multi_blind_screen::MultiBlindState;
use crate::palette;
use crate::panel_resize::{PanelSide, render_grab};
use crate::preview::PreviewCache;
use crate::relays_screen::RelayState;
use crate::scoped_rem::ScopedRem;
use crate::stats_screen::StatsView;
use crate::trainer_screen::{TrainerState, new_trainer_search};

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
    Charts,
    Goals,
    Trainer,
    Library,
    MultiBlind,
    Relays,
}

/// Everything read from disk at startup.
pub struct Loaded {
    pub settings: Settings,
    sessions: Vec<History>,
    /// `None` when the history could not be loaded, which keeps saving off so
    /// a file this version cannot read is never overwritten.
    history: Option<HistoryStore>,
    settings_path: Option<PathBuf>,
    problems: LoadProblems,
    goals: Vec<Goal>,
    /// `None` when the goals could not be loaded, which keeps saving off so a
    /// file this version cannot read is never overwritten.
    goals_path: Option<PathBuf>,
    /// Whether changes to tags can be saved.
    tags_available: bool,
    trainer: TrainerData,
    /// `None` when the trainer's data could not be loaded, which keeps saving
    /// off so a file this version cannot read is never overwritten.
    trainer_path: Option<PathBuf>,
    library: LibraryData,
    /// `None` when the library's data could not be loaded, which keeps saving
    /// off so a file this version cannot read is never overwritten.
    library_path: Option<PathBuf>,
    multi_blind: MultiBlindData,
    /// `None` when the Multi-Blind attempts could not be loaded, which keeps
    /// saving off so a file this version cannot read is never overwritten.
    multi_blind_path: Option<PathBuf>,
    relays: RelayData,
    /// `None` when the relays could not be loaded, which keeps saving off so
    /// a file this version cannot read is never overwritten.
    relays_path: Option<PathBuf>,
}

/// What went wrong reading from disk at startup.
struct LoadProblems {
    /// Why the history could not be loaded.
    history: Option<String>,
    /// The newest backup, offered when the history could not be loaded.
    backup: Option<PathBuf>,
    /// Where unreadable settings are, now that the defaults replace them.
    settings_aside: Option<PathBuf>,
    /// Why the goals could not be loaded.
    goals: Option<String>,
    /// Why the tags could not be loaded.
    tags: Option<String>,
    /// Why the trainer's data could not be loaded.
    trainer: Option<String>,
    /// Why the library's data could not be loaded.
    library: Option<String>,
    /// Why the Multi-Blind attempts could not be loaded.
    multi_blind: Option<String>,
    /// Why the relays could not be loaded.
    relays: Option<String>,
}

impl LoadProblems {
    /// Tells the user, once the window can show notifications.
    fn report(self, strings: &Strings, window: &Window) {
        if let Some(error) = self.history {
            let mut message = strings.toast_load_failed.replace("{error}", &error);
            if let Some(backup) = self.backup {
                let path = backup.display().to_string();
                message.push(' ');
                message.push_str(&strings.toast_backup_available.replace("{path}", &path));
            }
            notify_when_ready(Notification::error(message), window);
        }
        if let Some(error) = self.goals {
            let message = strings.toast_goals_load_failed.replace("{error}", &error);
            notify_when_ready(Notification::error(message), window);
        }
        if let Some(error) = self.tags {
            let message = strings.toast_tags_load_failed.replace("{error}", &error);
            notify_when_ready(Notification::error(message), window);
        }
        if let Some(error) = self.library {
            let message = strings.toast_library_load_failed.replace("{error}", &error);
            notify_when_ready(Notification::error(message), window);
        }
        if let Some(error) = self.trainer {
            let message = strings.toast_trainer_load_failed.replace("{error}", &error);
            notify_when_ready(Notification::error(message), window);
        }
        if let Some(error) = self.multi_blind {
            let message = strings
                .toast_multi_blind_load_failed
                .replace("{error}", &error);
            notify_when_ready(Notification::error(message), window);
        }
        if let Some(error) = self.relays {
            let message = strings.toast_relay_load_failed.replace("{error}", &error);
            notify_when_ready(Notification::error(message), window);
        }
        if let Some(aside) = self.settings_aside {
            let path = aside.display().to_string();
            let message = strings.toast_settings_reset.replace("{path}", &path);
            notify_when_ready(Notification::warning(message), window);
        }
    }
}

impl Loaded {
    pub fn load() -> Self {
        let history_path = persistence::history_file().ok();
        let settings_path = persistence::settings_file().ok();
        // cube-tui keeps its themes here; creating the default one gives both
        // apps a theme to start from.
        let _ = persistence::ensure_default_theme();
        let (history, sessions, load_error, backup) = match history_path.as_deref() {
            Some(path) => match HistoryStore::open(path) {
                Ok((store, sessions)) => (Some(store), sessions, None, None),
                Err(error) => (
                    None,
                    Vec::new(),
                    Some(format!("{error:#}")),
                    persistence::latest_backup(path),
                ),
            },
            None => (
                None,
                Vec::new(),
                Some("The data directory is unavailable.".to_owned()),
                None,
            ),
        };
        let (settings, settings_path, settings_aside) = load_settings(settings_path);
        let mut history = history;
        if let Some(store) = &mut history {
            store.set_user_backups(settings.user_backups());
        }
        let (goals, goals_path, goals_error) = load_goals();
        let (trainer, trainer_path, trainer_error) = load_trainer();
        let (mut library, library_path, library_error) = load_library();
        library.adopt_trainer(&trainer);
        let (multi_blind, multi_blind_path, multi_blind_error) = load_multi_blind();
        let (relays, relays_path, relays_error) = load_relays();
        let tags_available = history.as_ref().is_some_and(HistoryStore::tags_available);
        let tags_error = history
            .as_mut()
            .and_then(HistoryStore::take_extras_error)
            .map(|error| format!("{error:#}"));
        Self {
            settings,
            sessions,
            history,
            settings_path,
            problems: LoadProblems {
                history: load_error,
                backup,
                settings_aside,
                goals: goals_error,
                tags: tags_error,
                trainer: trainer_error,
                library: library_error,
                multi_blind: multi_blind_error,
                relays: relays_error,
            },
            goals,
            goals_path,
            tags_available,
            trainer,
            trainer_path,
            library,
            library_path,
            multi_blind,
            multi_blind_path,
            relays,
            relays_path,
        }
    }
}

/// Loads the goals. Unlike settings there are no defaults worth saving over a
/// file that cannot be read, so it stays where it is and saving turns off.
fn load_goals() -> (Vec<Goal>, Option<PathBuf>, Option<String>) {
    let Ok(path) = persistence::goals_file() else {
        return (Vec::new(), None, None);
    };
    match persistence::load_goals(&path) {
        Ok(goals) => (goals, Some(path), None),
        Err(error) => (Vec::new(), None, Some(format!("{error:#}"))),
    }
}

/// Loads the trainer's data. A file that cannot be read stays where it is and
/// saving turns off, as for the goals.
fn load_trainer() -> (TrainerData, Option<PathBuf>, Option<String>) {
    let Ok(path) = persistence::trainer_file() else {
        return (TrainerData::default(), None, None);
    };
    match persistence::load_trainer(&path) {
        Ok(data) => (data, Some(path), None),
        Err(error) => (TrainerData::default(), None, Some(format!("{error:#}"))),
    }
}

/// Loads the library's data. A file that cannot be read stays where it is and
/// saving turns off, as for the goals.
fn load_library() -> (LibraryData, Option<PathBuf>, Option<String>) {
    let Ok(path) = persistence::library_file() else {
        return (LibraryData::default(), None, None);
    };
    match persistence::load_library(&path) {
        Ok(data) => (data, Some(path), None),
        Err(error) => (LibraryData::default(), None, Some(format!("{error:#}"))),
    }
}

/// Loads the Multi-Blind attempts. A file that cannot be read stays where it
/// is and saving turns off, as for the goals.
fn load_multi_blind() -> (MultiBlindData, Option<PathBuf>, Option<String>) {
    let Ok(path) = persistence::multi_blind_file() else {
        return (MultiBlindData::default(), None, None);
    };
    match persistence::load_multi_blind(&path) {
        Ok(data) => (data, Some(path), None),
        Err(error) => (MultiBlindData::default(), None, Some(format!("{error:#}"))),
    }
}

/// Loads the relays. A file that cannot be read stays where it is and saving
/// turns off, as for the goals.
fn load_relays() -> (RelayData, Option<PathBuf>, Option<String>) {
    let Ok(path) = persistence::relays_file() else {
        return (RelayData::default(), None, None);
    };
    match persistence::load_relays(&path) {
        Ok(data) => (data, Some(path), None),
        Err(error) => (RelayData::default(), None, Some(format!("{error:#}"))),
    }
}

/// Loads the settings, moving a file that cannot be read aside so saving the
/// defaults does not destroy it. When it cannot be moved, the returned path is
/// `None`, which keeps settings from being saved over it.
fn load_settings(path: Option<PathBuf>) -> (Settings, Option<PathBuf>, Option<PathBuf>) {
    let Some(path) = path else {
        return (Settings::default(), None, None);
    };
    match persistence::load_settings(&path) {
        Ok(settings) => (settings, Some(path), None),
        Err(_) => match persistence::set_aside(&path) {
            Ok(aside) => (Settings::default(), Some(path), Some(aside)),
            Err(_) => (Settings::default(), None, Some(path)),
        },
    }
}

/// Returns the language `settings` ask for, or else the system's.
fn language_of(settings: &Settings) -> Language {
    Language::from_code(&settings.language)
        .or_else(|| sys_locale::get_locale().and_then(|code| Language::from_code(&code)))
        .unwrap_or_default()
}

/// Shows a notification that stays until dismissed, on the next frame: the
/// notification layer belongs to the root view, which wraps this one and does
/// not exist until construction finishes.
fn notify_when_ready(notification: Notification, window: &Window) {
    window.on_next_frame(move |window, cx| {
        window.push_notification(notification.autohide(false), cx);
    });
}

/// The effect playing on the digits for a new best.
#[derive(Clone, Copy)]
pub struct Celebration {
    pub scope: BestScope,
    pub started: Instant,
    /// The session's length at that solve, so that back-to-back bests each
    /// start their own animation.
    pub solve: usize,
}

/// The result shown on the timer once an attempt ends.
#[derive(Clone, Copy)]
pub struct LastResult {
    pub time_ms: u64,
    pub modifier: Modifier,
    /// How much of `time_ms` was the memo, for a blindfolded attempt.
    pub memo_ms: Option<u64>,
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
    pub(crate) bluetooth: Client,
    /// The Bluetooth timer doing the timing, once one is chosen.
    pub(crate) timer_link: Option<TimerLink>,
    /// Whether the timer key is physically down, to ignore key repeat.
    pub(crate) key_down: bool,
    /// Whether the mouse or a touch is holding the digits, the pointer twin of
    /// `key_down`.
    pointer_down: bool,
    /// The key that stopped a running timer, so its held repeats do not also
    /// trigger that key's action until it is released.
    stop_repeat_key: Option<String>,
    pub(crate) last_result: Option<LastResult>,
    /// The effect on the digits for the latest solve, if it set a best.
    pub(crate) celebration: Option<Celebration>,
    pub(crate) stats: SessionStats,
    /// The summary of the current event within the session, while it is a
    /// blindfolded one.
    pub(crate) blind_stats: Option<BlindStats>,
    chart_data: Option<Rc<ChartData>>,
    pub(crate) chart_view: ChartView,
    /// What the charts keep of each session. It lasts until the app closes.
    pub(crate) chart_filter: ChartFilter,
    /// The day selected on the calendar; the latest with solves until one is.
    pub(crate) chart_day: Option<NaiveDate>,
    pub(crate) chart_scroll: ScrollHandle,
    /// The stretch of the times chart showing; `None` shows all of it.
    pub(crate) chart_zoom: Option<ChartZoom>,
    /// Where the times chart was last painted, to place the wheel's cursor.
    pub(crate) chart_plot_bounds: Rc<Cell<Option<Bounds<Pixels>>>>,
    stats_table: Option<Rc<StatsView>>,
    pub(crate) stats_table_scroll: UniformListScrollHandle,
    pub(crate) solves_scroll: UniformListScrollHandle,
    /// The solve selected with the keyboard, as an index into the session.
    pub(crate) selected_solve: Option<usize>,
    /// The average selected with the keyboard, as an index into
    /// `stats::AVERAGE_SIZES`.
    pub(crate) selected_average: Option<usize>,
    /// Whether a share card is being drawn, so a second press waits for it.
    pub(crate) drawing_card: Cell<bool>,
    pub(crate) goals: Vec<Goal>,
    /// Where the goals are saved; `None` keeps saving off.
    pub(crate) goals_path: Option<PathBuf>,
    /// Each goal's progress, in the order of `goals`.
    pub(crate) goal_progress: Vec<GoalProgress>,
    /// Whether the solves changed since `goal_progress` was worked out.
    pub(crate) goals_stale: bool,
    /// The day `goal_progress` was worked out on, since a new one resets it.
    pub(crate) goals_day: NaiveDate,
    /// The goal selected with the keyboard, as an index into `goals`.
    pub(crate) selected_goal: Option<usize>,
    pub(crate) goals_scroll: UniformListScrollHandle,
    /// The algorithm trainer's screen and data.
    pub(crate) trainer: TrainerState,
    /// The algorithm library's screen and data.
    pub(crate) library: LibraryState,
    /// The Multi-Blind screen and its attempts.
    pub(crate) multi_blind: MultiBlindState,
    /// The Relays screen and its attempts.
    pub(crate) relays: RelayState,
    /// Whether changes to tags can be saved.
    pub(crate) tags_available: bool,
    /// Every tag and how many solves have it, sorted.
    pub(crate) tag_usage: Vec<TagUsage>,
    /// What the tag comparison sets against `chart_filter.tag`; `None` is
    /// every solve.
    pub(crate) compare_tag: Option<String>,
    keybinds: Keybinds,
    /// Deletions Undo can restore, oldest first.
    undo_stack: Vec<Deleted>,
    /// Undone deletions Redo can repeat, until a new deletion clears them.
    redo_stack: Vec<Deleted>,
    /// Saves the history off the UI thread; `None` keeps saving off.
    history_writer: Option<Arc<HistoryWriter>>,
    /// A background merge waiting for the timer and any dialog to finish.
    pending_merge: Option<Merge>,
    settings_path: Option<PathBuf>,
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

/// How much wider the title bar's left column is than its right one, or the
/// other way round, so the event picker between them sits on the window's
/// center. `TitleBar` takes 80 px on the left on macOS for the traffic lights;
/// elsewhere it takes 12 px on the left and three 34 px window controls on the
/// right (`gpui-kit` keeps these private).
#[cfg(target_os = "macos")]
const TITLE_BAR_COLUMN_BASES: (Pixels, Pixels) = (px(0.), px(80.));
#[cfg(not(target_os = "macos"))]
const TITLE_BAR_COLUMN_BASES: (Pixels, Pixels) = (px(90.), px(0.));

/// Room kept beside the event picker for the scramble button hanging off it.
const EVENT_PICKER_HANG: Rems = Rems(2.25);

/// The largest rem the title bar follows. The bar has a fixed height and must
/// fit the smallest window, with its window controls, in every language, so
/// past the largest preset it stops growing with the rest of the interface.
const TITLE_BAR_MAX_REM: Pixels = px(22.);

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

impl Deleted {
    /// Applies a change to a tag, so undoing brings back what is current.
    fn change_tags(&mut self, change: &TagChange) {
        match self {
            Self::Solve { solve, .. } => solve.change_tags(change),
            Self::Session { history, .. } => history.change_tags(change),
        }
    }
}

impl CubeApp {
    #[allow(clippy::too_many_lines, reason = "one line per field of the app")]
    pub fn new(loaded: Loaded, window: &mut Window, cx: &mut Context<Self>) -> Self {
        let Loaded {
            mut settings,
            mut sessions,
            history,
            settings_path,
            problems,
            goals,
            goals_path,
            tags_available,
            trainer,
            trainer_path,
            library,
            library_path,
            multi_blind,
            multi_blind_path,
            relays,
            relays_path,
        } = loaded;
        let strings = language_of(&settings).strings();
        let trainer_search = new_trainer_search(strings, window, cx);
        let library_search = new_library_search(strings, window, cx);
        settings.adopt_existing_history(&sessions);
        if sessions.is_empty() {
            sessions.push(History::new());
        }
        scramble::set_accurate_3x3(settings.accurate_3x3);
        let keybinds = settings.keybinds();
        let now = Local::now();
        let goal_progress = goals::progress_of(&goals, &sessions, &now);
        let tag_usage = tags::usage(&sessions, &settings.active_tags);

        let event = session_event(&sessions[0]);
        let focus = cx.focus_handle();
        focus.focus(window, cx);
        let mut app = Self {
            focus,
            screen: Screen::Timer,
            timer: settings.timer(),
            bluetooth: Client::default(),
            timer_link: None,
            stats: SessionStats::compute(sessions[0].solves()),
            blind_stats: None,
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
            celebration: None,
            chart_data: None,
            chart_view: ChartView::default(),
            chart_filter: ChartFilter::default(),
            chart_day: None,
            chart_scroll: ScrollHandle::default(),
            chart_zoom: None,
            chart_plot_bounds: Rc::default(),
            stats_table: None,
            stats_table_scroll: UniformListScrollHandle::new(),
            solves_scroll: UniformListScrollHandle::new(),
            selected_solve: None,
            selected_average: None,
            drawing_card: Cell::new(false),
            goals,
            goals_path,
            goal_progress,
            goals_stale: true,
            goals_day: now.date_naive(),
            selected_goal: None,
            goals_scroll: UniformListScrollHandle::new(),
            trainer: TrainerState::new(trainer, trainer_path, trainer_search),
            library: LibraryState::new(library, library_path, library_search),
            multi_blind: MultiBlindState::new(multi_blind, multi_blind_path),
            relays: RelayState::new(relays, relays_path),
            tags_available,
            tag_usage,
            compare_tag: None,
            undo_stack: Vec::new(),
            redo_stack: Vec::new(),
            keybinds,
            history_writer: history.map(|store| Arc::new(HistoryWriter::new(store))),
            pending_merge: None,
            settings_path,
            available: None,
            update_checking: false,
            updating: false,
            update_ticker: None,
            panel_resizing: false,
            window_save: None,
        };
        problems.report(app.strings(), window);
        app.refresh_blind_stats();
        app.next_scramble(cx);
        app.listen_for_scrambles(cx);
        app.start_ticking(window, cx);
        app.startup_update_tasks(window, cx);
        cx.observe_window_activation(window, Self::on_activation_changed)
            .detach();
        cx.observe_window_bounds(window, Self::on_window_bounds_changed)
            .detach();
        // Closing the window quits the app; hold it until the last save lands.
        if let Some(writer) = app.history_writer.clone() {
            cx.on_app_quit(move |_, cx| {
                let writer = Arc::clone(&writer);
                cx.background_executor()
                    .spawn(async move { writer.flush() })
            })
            .detach();
        }
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

    /// Returns the active interface language.
    pub(crate) fn language(&self) -> Language {
        language_of(&self.settings)
    }

    /// Returns the active language's strings.
    pub(crate) fn strings(&self) -> &'static Strings {
        self.language().strings()
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
            "" => self.strings().session_default_name.replace(
                "{number}",
                &self.language().format_count((index + 1) as u64),
            ),
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
        self.handle_writer_events(window, cx);
        self.play_inspection_cues(now, window, cx);
        if let Some(outcome) = self.timer.tick(now) {
            self.finish_attempt(outcome, window, cx);
        }
        if self.timer.is_active() {
            cx.notify();
            return ACTIVE_TICK;
        }
        self.refresh_goals(window, cx);
        IDLE_TICK
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
        if self.trainer_search_key(event, window, cx) || self.library_search_key(event, window, cx)
        {
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
            let action = self.keybinds.action_for(&press);
            if action == Some(Action::Cancel) {
                self.cancel(window, cx);
            } else if self.screen == Screen::Relays
                && !self.timer.mark_on_press
                && action == Some(Action::OpenSolve)
            {
                // The key stops the relay, so this one ends the puzzle.
                self.timer.mark(Instant::now());
                self.stop_repeat_key = None;
                cx.notify();
            } else if !self.hardware_timing()
                && let Some(outcome) = self.timer.press(Instant::now())
            {
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
            // With no timer on the picker, the key picks the highlighted case.
            if self.trainer_picking() {
                if !event.is_held {
                    self.toggle_trainer_case(window, cx);
                }
                return;
            }
            // With no timer on a review, the key shows the algorithm.
            if self.library_reviewing() {
                if !event.is_held {
                    self.toggle_library_reveal(cx);
                }
                return;
            }
            // Multi-Blind starts an attempt with the key, then times it.
            if self.screen == Screen::MultiBlind {
                if !event.is_held && !self.key_down && !self.pointer_down {
                    self.multi_blind_timer_key(window, cx);
                }
                return;
            }
            // Relays start with the key, then split with it.
            if self.screen == Screen::Relays {
                if !event.is_held && !self.key_down && !self.pointer_down {
                    self.relay_timer_key(window, cx);
                }
                return;
            }
            // Manual mode types times in instead of timing them, and a
            // Bluetooth timer is started and stopped by hand.
            if self.manual_mode() || self.hardware_timing() {
                return;
            }
            let timer_showing = self.screen == Screen::Timer || self.drilling();
            if self.key_down || self.pointer_down || event.is_held || !timer_showing {
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
        self.run_action(action, window, cx);
    }

    /// Runs a shortcut other than the timer's and Cancel, which depend on the
    /// key going down and up.
    #[allow(clippy::too_many_lines, reason = "one arm per action")]
    fn run_action(&mut self, action: Action, window: &mut Window, cx: &mut Context<Self>) {
        if self.screen == Screen::Trainer && self.trainer_action(action, window, cx) {
            return;
        }
        if self.screen == Screen::Library && self.library_action(action, window, cx) {
            return;
        }
        if self.screen == Screen::MultiBlind && self.multi_blind_action(action, window, cx) {
            return;
        }
        if self.screen == Screen::Relays && self.relay_action(action, window, cx) {
            return;
        }
        match action {
            Action::PreviousSolve | Action::NextSolve if self.screen == Screen::Goals => {
                self.step_goal(action == Action::NextSolve, cx);
            }
            Action::OpenSolve if self.screen == Screen::Goals => {
                self.open_target_goal(window, cx);
            }
            Action::DeleteSolve if self.screen == Screen::Goals => {
                self.delete_target_goal(window, cx);
            }
            Action::NewGoal if self.screen != Screen::Goals => {}
            Action::EditTags if matches!(self.screen, Screen::Charts | Screen::Goals) => {}
            Action::NextTagFilter if !self.tag_filter_shown() => {}
            Action::NextCompareTag
                if self.screen != Screen::Charts || self.chart_view != ChartView::Tags => {}
            Action::DeleteSolve | Action::PenaltyPlusTwo | Action::PenaltyDnf
                if self.screen != Screen::Timer => {}
            Action::ShareSolve | Action::ShareAverage | Action::CopySolveLink
                if matches!(self.screen, Screen::Charts | Screen::Goals) => {}
            Action::NextChartView
            | Action::NextChartEvent
            | Action::NextChartRange
            | Action::ChartDates
            | Action::ToggleChartSessions
            | Action::ChartZoomIn
            | Action::ChartZoomOut
            | Action::ChartResetZoom
                if self.screen != Screen::Charts => {}
            Action::OpenSolve if self.screen == Screen::Charts => {}
            Action::PreviousSolve
            | Action::NextSolve
            | Action::PreviousAverage
            | Action::NextAverage
                if self.screen == Screen::Charts =>
            {
                self.chart_arrow(action, cx);
            }
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
            Action::OpenStats => self.toggle_screen(Screen::Stats, cx),
            Action::OpenCharts => self.toggle_screen(Screen::Charts, cx),
            Action::OpenBluetooth => self.open_bluetooth(window, cx),
            Action::OpenGoals => self.toggle_screen(Screen::Goals, cx),
            Action::OpenTrainer => self.toggle_screen(Screen::Trainer, cx),
            Action::OpenLibrary => self.toggle_screen(Screen::Library, cx),
            Action::OpenMultiBlind => self.toggle_screen(Screen::MultiBlind, cx),
            Action::OpenRelays => self.toggle_screen(Screen::Relays, cx),
            Action::CopyMultiBlindScrambles => self.copy_multi_blind_scrambles(window, cx),
            Action::SaveMultiBlindScrambles => self.save_multi_blind_scrambles(window, cx),
            Action::NewGoal => self.open_goal_form(None, window, cx),
            Action::EditTags => {
                if let Some(index) = self.target_solve() {
                    self.open_solve_tags(index, window, cx);
                }
            }
            Action::ActiveTags => self.open_active_tags(window, cx),
            Action::ManageTags => self.open_manage_tags(window, cx),
            Action::NextTagFilter => self.next_tag_filter(cx),
            Action::NextCompareTag => self.next_compare_tag(cx),
            Action::NextChartView => self.next_chart_view(cx),
            Action::NextChartEvent => self.next_chart_event(cx),
            Action::NextChartRange => self.next_chart_range(cx),
            Action::ChartDates => self.open_chart_dates(window, cx),
            Action::ToggleChartSessions => self.toggle_chart_sessions(cx),
            Action::ChartZoomIn => self.zoom_chart(KEY_ZOOM, 0.5, cx),
            Action::ChartZoomOut => self.zoom_chart(KEY_ZOOM.recip(), 0.5, cx),
            Action::ChartResetZoom => self.reset_chart_zoom(cx),
            Action::PreviousAverage => self.step_selected_average(false, cx),
            Action::NextAverage => self.step_selected_average(true, cx),
            Action::OpenAverage => self.open_selected_average(window, cx),
            Action::OpenBestAverage => self.open_best_average(window, cx),
            Action::CopyAverage => self.copy_selected_average(window, cx),
            Action::ShareSolve => self.share_target_solve(window, cx),
            Action::ShareAverage => self.share_selected_average(window, cx),
            Action::CopySolveLink => self.copy_target_solve_link(window, cx),
            Action::OpenSettings => self.open_settings(window, cx),
            Action::ShowHelp => self.open_help(window, cx),
            // The relay actions only mean something on the Relays screen.
            Action::Timer
            | Action::NextRelay
            | Action::NewRelay
            | Action::DeleteRelay
            | Action::CopyRelayScrambles
            | Action::ToggleRelayKeySplits
            | Action::Cancel
            | Action::NextTrainerSet
            | Action::ToggleTrainerCase
            | Action::ToggleTrainerGroup
            | Action::ToggleTrainerSet
            | Action::StartTrainer
            | Action::RevealTrainerCase
            | Action::EditTrainerAlgorithm
            | Action::SortTrainerCases
            | Action::FocusSearch
            | Action::NextLibraryPuzzle
            | Action::CycleCaseStatus
            | Action::ToggleFavouriteAlgorithm
            | Action::AddLibraryAlgorithm
            | Action::EditLibraryNote
            | Action::StartLibraryReview
            | Action::TrainLibraryCase
            | Action::GradeAgain
            | Action::GradeHard
            | Action::GradeGood
            | Action::GradeEasy
            | Action::TrainerCaseAttempts
            | Action::SelectLearningCases
            | Action::SelectDueCases => {}
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
    /// same way the timer key does. Only when `touch_to_start` is on.
    pub(crate) fn on_pointer_down(
        &mut self,
        _event: &MouseDownEvent,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        // Keep keyboard shortcuts working after a click, even while touching
        // the digits does nothing.
        if !self.settings.touch_to_start {
            window.focus(&self.focus, cx);
            return;
        }
        if window.has_active_dialog(cx)
            || self.pointer_down
            || self.key_down
            || self.manual_mode()
            || self.hardware_timing()
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
        // `pointer_down` only becomes true while `touch_to_start` is on, so a
        // release always finishes its own hold even if the setting changed.
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
        } else if self.leave_multi_blind_attempt(cx)
            || self.leave_relay_attempt(cx)
            || self.leave_trainer_drill(cx)
            || self.leave_case_attempts(cx)
            || self.clear_trainer_search(window, cx)
            || self.leave_library_review(cx)
            || self.clear_library_search(window, cx)
        {
            // Back on the whole picker; another press leaves the screen.
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
        } else if !self.timer.is_active() && !window.has_active_dialog(cx) {
            // Coming back from cube-tui, say. An open dialog may hold a solve's
            // index, so with one open the next save does the merging instead.
            self.refresh_history();
        }
    }

    pub(crate) fn press_timer(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        // An attempt needs a scramble to record against. Multi-Blind keeps its
        // own, a whole list of them.
        if !self.timer.is_active()
            && !matches!(self.screen, Screen::MultiBlind | Screen::Relays)
            && self.shown_scramble().is_none()
        {
            return;
        }
        if !self.timer.is_active() {
            self.last_result = None;
            // A drill times one case, and Multi-Blind one long attempt; neither
            // has an inspection, nor a memo of its own to mark.
            let separate = self.drilling() || self.screen == Screen::MultiBlind;
            let relaying = self.screen == Screen::Relays;
            self.timer.puzzles = if relaying { self.relay_puzzles() } else { 1 };
            self.timer.mark_on_press = self.settings.relay_key_splits;
            self.timer.inspection = self.settings.timer().inspection && !separate;
            self.timer.blindfolded = self.shown_event().is_blindfolded() && !separate;
        }
        if let Some(outcome) = self.timer.press(Instant::now()) {
            self.finish_attempt(outcome, window, cx);
        }
        cx.notify();
    }

    /// Records a finished attempt and moves on to the next scramble.
    pub(crate) fn finish_attempt(
        &mut self,
        outcome: TimerOutcome,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if self.drilling() {
            self.finish_trainer_attempt(outcome, window, cx);
            return;
        }
        if self.screen == Screen::MultiBlind {
            self.finish_multi_blind_attempt(outcome, window, cx);
            return;
        }
        if self.screen == Screen::Relays {
            self.finish_relay_attempt(outcome, window, cx);
            return;
        }
        let TimerOutcome {
            time_ms,
            modifier,
            memo_ms,
        } = outcome;
        let scramble = self.scramble.take().unwrap_or_default();
        let mut solve = Solve::new(time_ms, self.event, scramble, modifier);
        solve.set_memo_ms(memo_ms);
        if self.tags_available {
            solve.set_tags(self.settings.active_tags.clone());
        }
        self.sessions[self.current].push(solve);
        self.next_scramble(cx);
        self.last_result = Some(LastResult {
            time_ms,
            modifier,
            memo_ms,
        });
        self.selected_solve = None;
        self.solves_changed(window, cx);
        self.solves_scroll.scroll_to_item(0, ScrollStrategy::Top);

        if time_ms == 0 && modifier == Modifier::DNF {
            self.toast(self.strings().toast_dnf, window, cx);
        }
        self.announce_records(window, cx);
        self.celebrate(window, cx);
        cx.notify();
    }

    /// Starts the effect and the sound if the newest solve set a best. Only
    /// [`Self::finish_attempt`] may call this, so that nothing but a live solve
    /// celebrates.
    fn celebrate(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        self.celebration = None;
        let (effect, sound) = (self.settings.celebrate_bests, self.settings.celebrate_sound);
        if !effect && !sound {
            return;
        }
        let min_solves = self.settings.best_min_solves;
        let Some(best) = bests::new_best(&self.sessions, self.current, min_solves) else {
            return;
        };
        if effect {
            self.celebration = Some(Celebration {
                scope: best.scope,
                started: Instant::now(),
                solve: self.session().len(),
            });
        }
        if sound && let Err(error) = crate::audio::play_personal_best() {
            let message = self
                .strings()
                .toast_celebration_audio_failed
                .replace("{error}", &format!("{error:#}"));
            self.toast_warning(&message, window, cx);
        }
    }

    /// Reports every statistic the newest solve improved, once the session is
    /// long enough for that to mean something.
    fn announce_records(&self, window: &mut Window, cx: &mut Context<Self>) {
        let solves = self.session().solves();
        if solves.len() <= self.settings.best_min_solves as usize {
            return;
        }
        let latest_index = solves.len() - 1;
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
            self.refresh_blind_stats();
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
        // Moving between sessions on the charts is asking to see one of them.
        if self.screen == Screen::Charts {
            self.chart_filter.all_sessions = false;
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

    /// Shows `screen`, keeping the target solve in view on the stats table.
    pub(crate) fn show_screen(&mut self, screen: Screen, cx: &mut Context<Self>) {
        // The digits belong to one side or the other, never to both.
        if (self.screen == Screen::Trainer) != (screen == Screen::Trainer) {
            self.last_result = None;
        }
        self.screen = screen;
        if screen == Screen::Stats
            && let Some(index) = self.target_solve()
        {
            self.stats_table_scroll
                .scroll_to_item(self.table_row(index), ScrollStrategy::Nearest);
        }
        cx.notify();
    }

    /// Shows `screen`, or returns to the timer when it is already showing.
    pub(crate) fn toggle_screen(&mut self, screen: Screen, cx: &mut Context<Self>) {
        let next = if self.screen == screen {
            Screen::Timer
        } else {
            screen
        };
        self.show_screen(next, cx);
    }

    /// Returns whether manual mode applies to what is on screen. It stays on
    /// for the timer while the trainer, which only times attempts, is showing.
    pub(crate) fn manual_mode(&self) -> bool {
        self.manual.is_some()
            && !matches!(
                self.screen,
                Screen::Trainer | Screen::Library | Screen::MultiBlind | Screen::Relays
            )
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
                memo_ms: None,
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
        if let Some(tag) = self.stats_tag() {
            let kept = tags::positions(self.session().solves(), Some(tag));
            if let Some(index) = tags::step(&kept, self.selected_solve, newer) {
                self.selected_solve = Some(index);
                self.stats_table_scroll
                    .scroll_to_item(self.table_row(index), ScrollStrategy::Nearest);
                cx.notify();
            }
            return;
        }
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
        self.stats_table_scroll
            .scroll_to_item(count - 1 - index, ScrollStrategy::Nearest);
        cx.notify();
    }

    /// Returns the solve an action applies to: the selection, or the latest.
    pub(crate) fn target_solve(&self) -> Option<usize> {
        let last = self.session().len().checked_sub(1)?;
        let target = self.selected_solve.unwrap_or(last).min(last);
        match self.stats_tag() {
            Some(tag) => {
                let kept = tags::positions(self.session().solves(), Some(tag));
                tags::nearest(&kept, target)
            }
            None => Some(target),
        }
    }

    /// Returns the tag the stats table is filtered by, while it is showing.
    pub(crate) fn stats_tag(&self) -> Option<&str> {
        let tag = self.chart_filter.tag.as_deref();
        tag.filter(|_| self.screen == Screen::Stats)
    }

    /// Returns which row of the stats table shows the solve at `index`.
    fn table_row(&self, index: usize) -> usize {
        let kept = tags::positions(self.session().solves(), self.stats_tag());
        let place = kept.partition_point(|&position| position < index);
        kept.len().saturating_sub(place + 1)
    }

    /// Returns the solves the averages work on: the session's, or on the stats
    /// table those its filter keeps.
    pub(crate) fn average_solves(&self) -> Cow<'_, [Solve]> {
        let solves = self.session().solves();
        match self.stats_tag() {
            Some(tag) => Cow::Owned(tags::filtered(solves, Some(tag))),
            None => Cow::Borrowed(solves),
        }
    }

    /// Returns the place in `average_solves` of the solve at `index`.
    pub(crate) fn average_place(&self, index: usize) -> usize {
        self.stats_tag().map_or(index, |tag| {
            let kept = tags::positions(self.session().solves(), Some(tag));
            kept.partition_point(|&position| position < index)
        })
    }

    /// Returns which solve of the session is at `place` of `average_solves`.
    pub(crate) fn session_index(&self, place: usize) -> usize {
        self.stats_tag().map_or(place, |tag| {
            let kept = tags::positions(self.session().solves(), Some(tag));
            kept.get(place).copied().unwrap_or(place)
        })
    }

    /// Returns every tag, sorted.
    pub(crate) fn known_tags(&self) -> Vec<String> {
        let usage = self.tag_usage.iter();
        usage.map(|usage| usage.tag.clone()).collect()
    }

    pub(crate) fn select_session(&mut self, index: usize, cx: &mut Context<Self>) {
        if index >= self.sessions.len() || self.timer.is_active() {
            return;
        }
        self.current = index;
        self.last_result = None;
        self.selected_solve = None;
        self.chart_day = None;
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
            self.goals_stale = true;
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
        goals::session_removed(&mut self.goals, index);
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

    /// Applies an edit to a solve of the current session, and new tags when
    /// there are any.
    pub(crate) fn edit_solve(
        &mut self,
        index: usize,
        after: SolveSnapshot,
        extras: Option<(Vec<String>, String)>,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let session = &mut self.sessions[self.current];
        let edited = session.edit(index, after);
        let extended = extras.is_some_and(|(tags, solution)| {
            let tagged = session.set_tags(index, tags);
            session.set_solution(index, solution) || tagged
        });
        if edited || extended {
            self.solves_changed(window, cx);
            self.sync_last_result();
            cx.notify();
        }
    }

    /// Replaces the tags of a solve of the current session.
    pub(crate) fn set_solve_tags(
        &mut self,
        index: usize,
        tags: Vec<String>,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if self.sessions[self.current].set_tags(index, tags) {
            self.solves_changed(window, cx);
            cx.notify();
        }
    }

    /// Sets the tags the next solves get, saving the setting.
    pub(crate) fn set_active_tags(
        &mut self,
        tags: Vec<String>,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        self.update_settings(window, cx, |settings| settings.active_tags = tags);
        self.tag_usage = tags::usage(&self.sessions, &self.settings.active_tags);
    }

    /// Renames or deletes a tag everywhere: on every solve, what undo can
    /// bring back, the next solves and the filters.
    pub(crate) fn apply_tag_change(
        &mut self,
        change: TagChange,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        for session in &mut self.sessions {
            session.change_tags(&change);
        }
        for deleted in self.undo_stack.iter_mut().chain(&mut self.redo_stack) {
            deleted.change_tags(&change);
        }
        self.chart_filter.tag = change.follow(self.chart_filter.tag.take());
        self.compare_tag = change.follow(self.compare_tag.take());
        self.update_settings(window, cx, |settings| {
            change.apply(&mut settings.active_tags);
        });
        self.refresh_stats();
        if let Some(writer) = &self.history_writer {
            writer.change_tags(change, self.sessions.clone());
        }
        cx.notify();
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
                    goals::session_inserted(&mut self.goals, index);
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
                goals::sessions_replaced(&mut self.goals);
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
            .replace("{imported}", &self.language().format_count(imported as u64))
            .replace("{skipped}", &self.language().format_count(skipped as u64));
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
                memo_ms: solve.memo_ms(),
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
        let user_backups = self.settings.user_backups();
        change(&mut self.settings);
        if let Some(writer) = &self.history_writer
            && self.settings.user_backups() != user_backups
        {
            writer.set_user_backups(self.settings.user_backups());
        }
        self.keybinds = self.settings.keybinds();
        if !self.timer.is_active() {
            self.timer = self.settings.timer();
        }
        if self.settings.accurate_3x3 != accurate_3x3 {
            scramble::set_accurate_3x3(self.settings.accurate_3x3);
            self.queue.discard(WcaEvent::Cube3x3);
            self.queue.discard(WcaEvent::Cube3x3OneHanded);
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

    /// Summarizes the current event if it is blindfolded, which gets its own
    /// figures in place of the trimmed averages.
    pub(crate) fn refresh_blind_stats(&mut self) {
        let event = self.event;
        self.blind_stats = event
            .is_blindfolded()
            .then(|| BlindStats::compute(self.sessions[self.current].solves(), event));
    }

    fn solves_changed(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        self.refresh_stats();
        self.save_history(window, cx);
    }

    fn refresh_stats(&mut self) {
        let stats = &mut self.stats;
        stats.update(self.sessions[self.current].solves());
        self.refresh_blind_stats();
        self.goals_stale = true;
        self.tag_usage = tags::usage(&self.sessions, &self.settings.active_tags);
        self.clear_chart_data();
        self.stats_table = None;
    }

    /// Drops the stats table, so the next use builds it again.
    pub(crate) fn clear_stats_table(&mut self) {
        self.stats_table = None;
    }

    /// Drops the chart data, so the next use builds it again. The zoom goes
    /// with it, since it points into the old series.
    pub(crate) fn clear_chart_data(&mut self) {
        self.chart_data = None;
        self.chart_zoom = None;
    }

    /// Returns the data behind the charts, building it on first use.
    pub(crate) fn chart_data(&mut self) -> Rc<ChartData> {
        let (sessions, current) = (&self.sessions, self.current);
        let (filter, compare_tag) = (&self.chart_filter, self.compare_tag.as_deref());
        Rc::clone(
            self.chart_data.get_or_insert_with(|| {
                Rc::new(ChartData::new(sessions, current, filter, compare_tag))
            }),
        )
    }

    /// Returns the current session's table of averages, building it on first
    /// use.
    pub(crate) fn stats_table(&mut self) -> Rc<StatsView> {
        let solves = self.sessions[self.current].solves();
        let tag = self.chart_filter.tag.as_deref();
        Rc::clone(
            self.stats_table
                .get_or_insert_with(|| Rc::new(StatsView::new(solves, tag))),
        )
    }

    fn save_history(&self, _window: &mut Window, _cx: &mut Context<Self>) {
        if let Some(writer) = &self.history_writer {
            writer.save(self.sessions.clone());
        }
    }

    /// Picks up what another app wrote to the history since the last save.
    fn refresh_history(&self) {
        if let Some(writer) = &self.history_writer {
            writer.refresh(self.sessions.clone());
        }
    }

    /// Handles what the background writer finished since the last tick.
    fn handle_writer_events(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let Some(writer) = self.history_writer.clone() else {
            return;
        };
        for event in writer.take_events() {
            match event {
                WriterEvent::Merged(merge) => self.pending_merge = Some(merge),
                WriterEvent::SaveFailed(error) => {
                    let error = anyhow::anyhow!(error);
                    self.report_save_error(&error, window, cx);
                }
                WriterEvent::BackupFailed(error) => {
                    self.report_backup_error(&anyhow::anyhow!(error), window, cx);
                }
                WriterEvent::BackedUp(path) => {
                    let path = path.display().to_string();
                    let message = self.strings().toast_backup_done.replace("{path}", &path);
                    self.toast(&message, window, cx);
                }
            }
        }
        // An attempt in progress and an open dialog both rely on the solves
        // staying where they are. If either saves first, this merge is stale
        // and the writer sends a new one.
        if self.timer.is_active() || window.has_active_dialog(cx) {
            return;
        }
        if let Some(merge) = self.pending_merge.take()
            && let Some((sessions, summary)) = writer.adopt(merge)
        {
            self.sessions = sessions;
            self.history_merged(summary, window, cx);
        }
    }

    /// Copies the history into the user's backup folder.
    pub(crate) fn back_up_now(&self, window: &mut Window, cx: &mut Context<Self>) {
        if let Some(writer) = &self.history_writer {
            writer.refresh(self.sessions.clone());
            writer.back_up();
        } else {
            let error = anyhow::anyhow!("Your solves are not loaded.");
            self.report_backup_error(&error, window, cx);
        }
    }

    pub(crate) fn report_backup_error(
        &self,
        error: &anyhow::Error,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        self.report_error(self.strings().toast_backup_failed, error, window, cx);
    }

    /// Catches up after a merge moved solves around under the session.
    ///
    /// Session indices survive a merge but solve indices may not, so what
    /// points at a solve by position is dropped. Shown even with toasts turned
    /// off: the history changed without the user doing anything.
    fn history_merged(
        &mut self,
        summary: MergeSummary,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        self.selected_solve = None;
        self.redo_stack.clear();
        self.refresh_stats();
        cx.notify();
        if !summary.is_empty() {
            let message = self
                .strings()
                .toast_history_merged
                .replace(
                    "{added}",
                    &self.language().format_count(summary.added as u64),
                )
                .replace(
                    "{updated}",
                    &self.language().format_count(summary.updated as u64),
                )
                .replace("{kept}", &self.language().format_count(summary.kept as u64));
            let toast = Notification::warning(message).placement(Anchor::BottomRight);
            window.push_notification(toast, cx);
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
    pub(crate) fn report_error(
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
                        .child(
                            h_flex()
                                .gap_1()
                                .items_center()
                                .text_base()
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
            .children(self.render_active_tags(cx).map(|button| {
                div()
                    .absolute()
                    .right_full()
                    .top_0()
                    .bottom_0()
                    .mr_1()
                    .child(h_flex().occlude().h_full().items_center().child(button))
            }))
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
        let rem = window.rem_size().min(TITLE_BAR_MAX_REM);
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
                            app.show_screen(screen, cx);
                        }
                        app.focus.focus(window, cx);
                    }))
            };
        let (left_basis, right_basis) = TITLE_BAR_COLUMN_BASES;
        // The columns share the free room equally, which puts the picker on the
        // window's center. A column never gets narrower than what it holds, so
        // when the text is too large for that the picker moves aside rather
        // than landing on the tabs.
        ScopedRem::new(
            rem,
            TitleBar::new().child(
                h_flex()
                    .w_full()
                    .h_full()
                    .child(
                        h_flex()
                            .flex_grow(1.)
                            .flex_basis(left_basis)
                            .gap_2()
                            .child(img(logo()).size(px(18.)).flex_none())
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
                                    .child(tab("nav-stats", strings.nav_stats, Screen::Stats, cx))
                                    .child(tab(
                                        "nav-charts",
                                        strings.nav_charts,
                                        Screen::Charts,
                                        cx,
                                    ))
                                    .child(tab(
                                        "nav-trainer",
                                        strings.nav_trainer,
                                        Screen::Trainer,
                                        cx,
                                    ))
                                    .child(tab(
                                        "nav-library",
                                        strings.nav_library,
                                        Screen::Library,
                                        cx,
                                    ))
                                    .child(tab(
                                        "nav-multi-blind",
                                        strings.nav_multi_blind,
                                        Screen::MultiBlind,
                                        cx,
                                    ))
                                    .child(tab(
                                        "nav-relays",
                                        strings.nav_relays,
                                        Screen::Relays,
                                        cx,
                                    )),
                            ),
                    )
                    // The trainer's and library's sets say what is being practised
                    // instead, and Multi-Blind has only one event.
                    .children(
                        (!matches!(
                            self.screen,
                            Screen::Trainer | Screen::Library | Screen::MultiBlind | Screen::Relays
                        ))
                        .then(|| self.render_event_picker(cx)),
                    )
                    .child(
                        h_flex()
                            .flex_grow(1.)
                            .flex_basis(right_basis)
                            .justify_end()
                            .child(self.render_title_bar_buttons(cx)),
                    ),
            ),
        )
    }

    /// The buttons at the right of the title bar.
    fn render_title_bar_buttons(&self, cx: &mut Context<Self>) -> impl IntoElement + use<> {
        let strings = self.strings();
        h_flex()
            .occlude()
            .ml(EVENT_PICKER_HANG)
            .mr_2()
            .gap_1()
            .child(self.render_bluetooth_button(cx))
            .child(self.render_goals_button(cx))
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
            )
    }
}

impl Render for CubeApp {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let attempt = self.timer.is_active();
        let body = match self.screen {
            Screen::Timer => {
                let (sidebar_width, stats_width) = self.panel_widths(window, cx);
                let stacked = cube_core::panels::stats_rows_stack(
                    f32::from(stats_width) as u32,
                    self.panel_content(window, cx).stats,
                );
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
                        row.child(self.render_stats_panel(stats_width, stacked, cx))
                            // The grips go last so they sit above the panels they straddle.
                            .child(render_grab(PanelSide::Sidebar, sidebar_width, cx))
                            .child(render_grab(PanelSide::Stats, stats_width, cx))
                    })
                    .into_any_element()
            }
            Screen::Stats => self.render_table(cx),
            Screen::Charts => self.render_charts(cx),
            Screen::Goals => self.render_goals(cx),
            Screen::Trainer => self.render_trainer(window, cx),
            Screen::Library => self.render_library(window, cx),
            Screen::MultiBlind => self.render_multi_blind(window, cx),
            Screen::Relays => self.render_relays(window, cx),
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
