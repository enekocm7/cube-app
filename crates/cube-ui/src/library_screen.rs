//! The algorithm library: every puzzle's sets to browse and search, a case's
//! drawing and algorithms, what the user has learned, and the review queue.
//!
//! What it all means lives in `cube_core::library`; this only shows it.

use std::cell::RefCell;
use std::collections::{HashMap, HashSet};
use std::ops::Range;
use std::path::PathBuf;
use std::rc::Rc;
use std::sync::Arc;
use std::time::{SystemTime, UNIX_EPOCH};

use cube_core::library::{
    Grade, LibraryData, LibraryPuzzle, Status, check, find_set, first_visible_set, recognition,
    search_visible_cases, visible_catalog,
};
use cube_core::trainer::notation::AlgError;
use cube_core::trainer::{
    AlgCase, AlgSet, CustomAlgError, PLL, Step, case_diagram, case_thumbnail, diagram_ratio,
};
use cube_core::{Action, Settings, WcaEvent, persistence};
use cube_locale::Strings;
use gpui_kit::assets::IconName;
use gpui_kit::component::button::{Button, ButtonVariants as _};
use gpui_kit::component::input::{Input, InputEvent, InputState};
use gpui_kit::component::menu::{DropdownMenu as _, PopupMenuItem};
use gpui_kit::component::scroll::Scrollbar;
use gpui_kit::component::{ActiveTheme as _, Icon, Sizable as _, WindowExt as _, h_flex, v_flex};
use gpui_kit::prelude::FluentBuilder as _;
use gpui_kit::*;

use crate::app::{CubeApp, Screen};
use crate::details::{field_error, keep_focus};
use crate::dialog_size::{DialogSize, dialog_title, dialog_width};
use crate::dialog_submit::{DialogSubmit as _, Submit, on_press};
use crate::palette::Palette;
use crate::scroll_fade::{FadeScroll, fading_scroll};
use crate::search_bar::{focus_on_search_key, new_search, query, search_field};

/// Width of the list of puzzles and sets.
pub(crate) const NAV_WIDTH: Rems = Rems(14.);
/// A row of that list, which follows the text size.
const TREE_ROW: Rems = Rems(1.9);
/// How far what a dropdown holds sits in from it.
const TREE_INDENT: Rems = Rems(0.95);
/// Width of the panel about the highlighted case.
const DETAIL_WIDTH: Rems = Rems(22.);
const SEARCH_WIDTH: Rems = Rems(14.);
/// A case in the list, which follows the text size.
const ROW_HEIGHT: Rems = Rems(4.75);
const ROW_DIAGRAM: Rems = Rems(4.25);
/// The drawing of the highlighted case.
const DETAIL_DIAGRAM: Rems = Rems(9.);
/// The drawing of the case under review.
const REVIEW_DIAGRAM: Rems = Rems(12.);
/// Room kept for the case list between the library's side panels.
const MIN_LIST_WIDTH: Rems = Rems(16.);

/// `base` scaled by the library size setting, kept in rems so it still follows
/// the text size.
fn library_rem(base: Rems, scale: f32) -> Rems {
    Rems(base.0 * scale)
}

/// The nav and detail panel widths at the chosen library size, narrowed
/// together when the window cannot hold them and a readable case list.
fn library_panel_widths(window: &Window, scale: f32) -> (Rems, Rems) {
    let (nav, detail) = (
        library_rem(NAV_WIDTH, scale),
        library_rem(DETAIL_WIDTH, scale),
    );
    let rem = f32::from(window.rem_size()).max(1.);
    let room = f32::from(window.viewport_size().width) / rem - MIN_LIST_WIDTH.0;
    let total = nav.0 + detail.0;
    if room <= 0. || total <= room {
        return (nav, detail);
    }
    let fit = room / total;
    (Rems(nav.0 * fit), Rems(detail.0 * fit))
}

/// A case's drawing at `height`, as wide as the set's drawings are, or as the
/// space it is placed in.
fn full_diagram(image: Arc<Image>, set: &AlgSet, height: Rems) -> impl IntoElement + use<> {
    img(image)
        .h(height)
        .w(Rems(height.0 * diagram_ratio(set) as f32))
        .max_w_full()
        .object_fit(ObjectFit::Contain)
        .flex_none()
}

/// Case drawings by set id and case name; `None` if one could not be drawn.
type Diagrams = HashMap<(&'static str, &'static str), Option<Arc<Image>>>;

/// A case and the set it is in.
type Row = (&'static AlgSet, &'static AlgCase);

/// The cases left to review, the current one first.
struct ReviewRun {
    queue: Vec<Row>,
    /// Whether the algorithm of the current case is showing.
    revealed: bool,
}

/// The library's state.
pub(crate) struct LibraryState {
    pub(crate) data: LibraryData,
    /// Where the data is saved; `None` keeps saving off.
    path: Option<PathBuf>,
    set: &'static AlgSet,
    /// The puzzles open in the list.
    open_puzzles: HashSet<WcaEvent>,
    /// The methods open in the list, under their puzzles.
    open_methods: HashSet<(WcaEvent, &'static str)>,
    search: Entity<InputState>,
    /// What is being searched for, in every set; empty shows the set.
    query: String,
    /// The cases listed.
    rows: Vec<Row>,
    /// The highlighted row.
    cursor: usize,
    /// The highlighted algorithm of that row's case.
    algorithm: usize,
    review: Option<ReviewRun>,
    list_scroll: UniformListScrollHandle,
    diagrams: RefCell<Diagrams>,
    thumbnails: RefCell<Diagrams>,
}

impl LibraryState {
    pub(crate) fn new(
        data: LibraryData,
        path: Option<PathBuf>,
        search: Entity<InputState>,
        settings: &Settings,
    ) -> Self {
        let mut state = Self {
            data,
            path,
            set: if settings.shows_set_in_library(PLL.id) {
                &PLL
            } else {
                first_visible_set(settings).unwrap_or(&PLL)
            },
            open_puzzles: HashSet::new(),
            open_methods: HashSet::new(),
            search,
            query: String::new(),
            rows: Vec::new(),
            cursor: 0,
            algorithm: 0,
            review: None,
            list_scroll: UniformListScrollHandle::new(),
            diagrams: RefCell::default(),
            thumbnails: RefCell::default(),
        };
        state.refresh_rows(settings);
        state.reveal_set();
        state
    }

    /// Opens the way to the set, closing the other puzzles, so that moving
    /// through the sets from the keyboard keeps the list short.
    fn reveal_set(&mut self) {
        let set = self.set;
        self.open_puzzles.clear();
        self.open_puzzles.insert(set.event);
        self.open_methods.insert((set.event, set.method));
    }

    /// Drops cases of sets that the library no longer shows, then moves off a
    /// hidden set.
    pub(crate) fn adopt_visibility(&mut self, settings: &Settings) {
        let empty = if let Some(review) = &mut self.review {
            review
                .queue
                .retain(|(set, _)| settings.shows_set_in_library(set.id));
            review.queue.is_empty()
        } else {
            false
        };
        if empty {
            self.review = None;
        }
        if settings.shows_set_in_library(self.set.id) {
            self.refresh_rows(settings);
            return;
        }
        if let Some(set) = first_visible_set(settings) {
            self.set = set;
            self.reveal_set();
        }
        self.refresh_rows(settings);
    }

    /// Opens or closes a puzzle. Opening one shows the way to the current set,
    /// and a method with no others to choose from.
    fn toggle_puzzle(&mut self, event: WcaEvent, only_method: Option<&'static str>) {
        if self.open_puzzles.remove(&event) {
            return;
        }
        self.open_puzzles.insert(event);
        if let Some(method) = only_method {
            self.open_methods.insert((event, method));
        }
        if self.set.event == event {
            self.open_methods.insert((event, self.set.method));
        }
    }

    fn toggle_method(&mut self, event: WcaEvent, method: &'static str) {
        if !self.open_methods.remove(&(event, method)) {
            self.open_methods.insert((event, method));
        }
    }

    /// Lists what the search or the set calls for.
    fn refresh_rows(&mut self, settings: &Settings) {
        self.rows = if self.query.trim().is_empty() {
            let set = self.set;
            set.cases.iter().map(|case| (set, case)).collect()
        } else {
            search_visible_cases(settings, &self.query)
        };
        self.cursor = 0;
        self.algorithm = 0;
    }

    fn selected(&self) -> Option<Row> {
        self.rows.get(self.cursor).copied()
    }

    /// Returns the drawing of `case`, drawing it the first time.
    fn diagram(&self, set: &AlgSet, case: &'static AlgCase) -> Option<Arc<Image>> {
        let mut diagrams = self.diagrams.borrow_mut();
        let entry = diagrams
            .entry((set.id, case.name))
            .or_insert_with(|| case_diagram(set, case).map(svg_image));
        entry.clone()
    }

    /// Returns the drawing of `case` for the list.
    fn thumbnail(&self, set: &AlgSet, case: &'static AlgCase) -> Option<Arc<Image>> {
        let mut thumbnails = self.thumbnails.borrow_mut();
        let entry = thumbnails
            .entry((set.id, case.name))
            .or_insert_with(|| case_thumbnail(set, case).map(svg_image));
        entry.clone()
    }

    /// Returns the cases due now that the library still has and shows.
    fn due(&self, settings: &Settings) -> Vec<Row> {
        let due = self.data.due(now_unix_ms(), settings);
        due.iter()
            .filter_map(|due| {
                let set = find_set(&due.set)?;
                Some((set, set.case(&due.case)?))
            })
            .collect()
    }
}

/// Creates the search field, which looks through every set.
pub(crate) fn new_library_search(
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
                app.library.query = query(input, cx);
                app.library.refresh_rows(&app.settings);
            }
            InputEvent::PressEnter { .. } => app.focus.focus(window, cx),
            _ => {}
        },
    )
    .detach();
    search
}

fn svg_image(svg: String) -> Arc<Image> {
    Arc::new(Image::from_bytes(ImageFormat::Svg, svg.into_bytes()))
}

fn now_unix_ms() -> u64 {
    let now = SystemTime::now().duration_since(UNIX_EPOCH);
    now.map_or(0, |elapsed| {
        u64::try_from(elapsed.as_millis()).unwrap_or(u64::MAX)
    })
}

fn status_label(strings: &'static Strings, status: Status) -> &'static str {
    match status {
        Status::NotStarted => strings.library_status_not_started,
        Status::Learning => strings.library_status_learning,
        Status::Learned => strings.library_status_learned,
    }
}

fn grade_label(strings: &'static Strings, grade: Grade) -> &'static str {
    match grade {
        Grade::Again => strings.library_grade_again,
        Grade::Hard => strings.library_grade_hard,
        Grade::Good => strings.library_grade_good,
        Grade::Easy => strings.library_grade_easy,
    }
}

/// Says what is wrong with an algorithm the user gave for `case`.
fn algorithm_error(strings: &'static Strings, case: &AlgCase, error: CustomAlgError) -> String {
    match error {
        CustomAlgError::Notation(AlgError::InvalidMove(part)) => {
            strings.trainer_algorithm_invalid.replace("{move}", &part)
        }
        CustomAlgError::Notation(AlgError::Empty) | CustomAlgError::WrongCase => strings
            .trainer_algorithm_wrong_case
            .replace("{case}", case.name),
    }
}

/// A row of the list of puzzles.
pub(crate) fn tree_row(id: impl Into<ElementId>, selected: bool, cx: &App) -> Stateful<Div> {
    let theme = cx.theme();
    let (accent, hover) = (theme.accent, theme.accent.opacity(0.5));
    h_flex()
        .id(id)
        .w_full()
        .h(TREE_ROW)
        .flex_none()
        .px_2()
        .gap_1p5()
        .rounded(px(6.))
        .cursor_pointer()
        .hover(move |row| row.bg(hover))
        .when(selected, |row| row.bg(accent))
}

/// What a dropdown holds, hanging from a line under it.
pub(crate) fn tree_children(cx: &App) -> Div {
    v_flex()
        .ml(TREE_INDENT)
        .pl_1()
        .gap_0p5()
        .border_l_1()
        .border_color(cx.theme().border)
}

pub(crate) fn chevron(open: bool, cx: &App) -> Icon {
    let icon = if open {
        IconName::ChevronDown
    } else {
        IconName::ChevronRight
    };
    Icon::new(icon)
        .xsmall()
        .text_color(cx.theme().muted_foreground)
}

/// How much of a row's cases are learned, at the end of the row.
pub(crate) fn tree_count(count: String, cx: &App) -> Div {
    let theme = cx.theme();
    div()
        .flex_none()
        .whitespace_nowrap()
        .text_xs()
        .font_family(theme.mono_font_family.clone())
        .text_color(theme.muted_foreground)
        .child(count)
}

/// Which of the library's two text dialogs is open.
#[derive(Clone, Copy, PartialEq, Eq)]
enum TextField {
    Algorithm,
    Note,
}

impl CubeApp {
    /// Returns whether a review is showing.
    pub(crate) fn library_reviewing(&self) -> bool {
        self.screen == Screen::Library && self.library.review.is_some()
    }

    /// Writes the library's data to disk. Without a path, which means the file
    /// on disk could not be read, nothing is written over it.
    pub(crate) fn save_library(&self, window: &mut Window, cx: &mut Context<Self>) {
        if let Some(path) = &self.library.path
            && let Err(error) = persistence::save_library(path, &self.library.data)
        {
            self.report_error(self.strings().toast_library_save_failed, &error, window, cx);
        }
    }

    /// Lets the search field have the keys while it is focused and takes `/`
    /// to reach it. Returns whether the key is dealt with.
    pub(crate) fn library_search_key(
        &mut self,
        event: &KeyDownEvent,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> bool {
        if self.screen != Screen::Library || self.library.review.is_some() {
            return false;
        }
        let search = self.library.search.clone();
        if search.read(cx).focus_handle(cx).is_focused(window) {
            match event.keystroke.key.as_str() {
                "escape" => {
                    self.clear_library_search(window, cx);
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

    /// Ends the search. Returns whether there was one.
    pub(crate) fn clear_library_search(
        &mut self,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> bool {
        if self.screen != Screen::Library || self.library.query.is_empty() {
            return false;
        }
        self.library
            .search
            .update(cx, |search, cx| search.set_value("", window, cx));
        self.library.query.clear();
        self.library.refresh_rows(&self.settings);
        cx.notify();
        true
    }

    /// Ends a review. Returns whether there was one.
    pub(crate) fn leave_library_review(&mut self, cx: &mut Context<Self>) -> bool {
        if self.screen != Screen::Library || self.library.review.is_none() {
            return false;
        }
        self.library.review = None;
        cx.notify();
        true
    }

    /// Runs a shortcut on the library. Returns `false` for those that work the
    /// same everywhere; the rest mean nothing here and do nothing.
    pub(crate) fn library_action(
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
                | Action::OpenLibrary
                | Action::OpenMultiBlind
                | Action::OpenBluetooth
                | Action::OpenSettings
                | Action::ShowHelp
                | Action::FocusSearch
        ) {
            return false;
        }
        if self.library.review.is_some() {
            self.review_action(action, window, cx);
        } else {
            self.browse_action(action, window, cx);
        }
        true
    }

    fn browse_action(&mut self, action: Action, window: &mut Window, cx: &mut Context<Self>) {
        match action {
            Action::PreviousSolve => self.move_library_cursor(false, cx),
            Action::NextSolve => self.move_library_cursor(true, cx),
            Action::PreviousAverage => self.move_library_algorithm(false, cx),
            Action::NextAverage => self.move_library_algorithm(true, cx),
            Action::NextTrainerSet => self.next_library_set(window, cx),
            Action::NextLibraryPuzzle => self.next_library_puzzle(window, cx),
            Action::CycleCaseStatus => self.cycle_library_status(window, cx),
            Action::ToggleFavouriteAlgorithm => self.toggle_library_favourite(window, cx),
            Action::AddLibraryAlgorithm => self.open_library_text(TextField::Algorithm, window, cx),
            Action::EditLibraryNote => self.open_library_text(TextField::Note, window, cx),
            Action::DeleteSolve => self.remove_library_algorithm(window, cx),
            Action::StartLibraryReview => self.start_library_review(window, cx),
            Action::TrainLibraryCase => self.train_library_case(window, cx),
            Action::ToggleSetInLibrary => self.toggle_set_in_library(window, cx),
            Action::ToggleSetInTraining => self.toggle_set_in_training(window, cx),
            _ => {}
        }
    }

    /// Shows or hides the library's set in the library.
    fn toggle_set_in_library(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let set = self.library.set;
        let show = !self.settings.shows_set_in_library(set.id);
        self.update_settings(window, cx, |settings| {
            settings.show_set_in_library(set.id, show);
        });
        cx.notify();
    }

    /// Includes or leaves out the library's set in the trainer.
    fn toggle_set_in_training(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let set = self.library.set;
        let include = !self.settings.includes_set_in_training(set.id);
        self.update_settings(window, cx, |settings| {
            settings.include_set_in_training(set.id, include);
        });
        cx.notify();
    }

    /// Starts training the highlighted case in the trainer.
    fn train_library_case(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if let Some((set, case)) = self.library.selected() {
            self.train_case(set, case, window, cx);
        }
    }

    fn review_action(&mut self, action: Action, window: &mut Window, cx: &mut Context<Self>) {
        match action {
            Action::RevealTrainerCase | Action::OpenSolve => self.toggle_library_reveal(cx),
            Action::GradeAgain => self.grade_library_case(Grade::Again, window, cx),
            Action::GradeHard => self.grade_library_case(Grade::Hard, window, cx),
            Action::GradeGood => self.grade_library_case(Grade::Good, window, cx),
            Action::GradeEasy => self.grade_library_case(Grade::Easy, window, cx),
            _ => {}
        }
    }

    fn move_library_cursor(&mut self, down: bool, cx: &mut Context<Self>) {
        let last = self.library.rows.len().saturating_sub(1);
        let cursor = self.library.cursor;
        self.library.cursor = if down {
            (cursor + 1).min(last)
        } else {
            cursor.saturating_sub(1)
        };
        self.library.algorithm = 0;
        self.library
            .list_scroll
            .scroll_to_item(self.library.cursor, ScrollStrategy::Nearest);
        cx.notify();
    }

    fn move_library_algorithm(&mut self, next: bool, cx: &mut Context<Self>) {
        let Some((set, case)) = self.library.selected() else {
            return;
        };
        let last = self
            .library
            .data
            .algorithms(set, case)
            .len()
            .saturating_sub(1);
        let algorithm = self.library.algorithm;
        self.library.algorithm = if next {
            (algorithm + 1).min(last)
        } else {
            algorithm.saturating_sub(1)
        };
        cx.notify();
    }

    /// Shows `case` of `set` in the library.
    pub(crate) fn open_in_library(
        &mut self,
        set: &'static AlgSet,
        case: &'static AlgCase,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        self.select_library_set(set, window, cx);
        self.library.reveal_set();
        if let Some(at) = self
            .library
            .rows
            .iter()
            .position(|row| row.1.name == case.name)
        {
            self.library.cursor = at;
            self.library
                .list_scroll
                .scroll_to_item(at, ScrollStrategy::Center);
        }
        self.show_screen(Screen::Library, cx);
    }

    fn select_library_set(
        &mut self,
        set: &'static AlgSet,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        self.library.set = set;
        if !self.clear_library_search(window, cx) {
            self.library.refresh_rows(&self.settings);
        }
        self.library
            .list_scroll
            .scroll_to_item(0, ScrollStrategy::Top);
        cx.notify();
    }

    /// Moves on to the next set of the puzzle, going around.
    fn next_library_set(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let current = self.library.set;
        let puzzles = visible_catalog(&self.settings);
        let Some(puzzle) = puzzles.iter().find(|puzzle| puzzle.event == current.event) else {
            if let Some(set) = first_visible_set(&self.settings) {
                self.select_library_set(set, window, cx);
            }
            return;
        };
        let at = puzzle.sets.iter().position(|set| set.id == current.id);
        let next = at.map_or(0, |at| (at + 1) % puzzle.sets.len());
        self.select_library_set(puzzle.sets[next], window, cx);
        self.library.reveal_set();
    }

    /// Moves on to the first set of the next puzzle, going around.
    fn next_library_puzzle(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let puzzles = visible_catalog(&self.settings);
        if puzzles.is_empty() {
            return;
        }
        let event = self.library.set.event;
        let at = puzzles.iter().position(|puzzle| puzzle.event == event);
        let next = at.map_or(0, |at| (at + 1) % puzzles.len());
        self.select_library_set(puzzles[next].sets[0], window, cx);
        self.library.reveal_set();
    }

    fn cycle_library_status(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let Some((set, case)) = self.library.selected() else {
            return;
        };
        self.library
            .data
            .cycle_status(set, case.name, now_unix_ms());
        self.save_library(window, cx);
        cx.notify();
    }

    fn set_library_status(
        &mut self,
        row: Row,
        status: Status,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let (set, case) = row;
        self.library
            .data
            .set_status(set, case.name, status, now_unix_ms());
        self.save_library(window, cx);
        cx.notify();
    }

    /// Chooses the highlighted algorithm, or clears the choice.
    fn toggle_library_favourite(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let Some((set, case)) = self.library.selected() else {
            return;
        };
        let algorithms = self.library.data.algorithms(set, case);
        let Some(algorithm) = algorithms.get(self.library.algorithm) else {
            return;
        };
        let algorithm = (*algorithm).to_owned();
        self.library
            .data
            .toggle_favourite(set, case.name, &algorithm);
        self.after_library_choice(set, case, window, cx);
    }

    /// Removes the highlighted algorithm if it is one of the user's.
    fn remove_library_algorithm(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let Some((set, case)) = self.library.selected() else {
            return;
        };
        let algorithms = self.library.data.algorithms(set, case);
        let Some(algorithm) = algorithms.get(self.library.algorithm) else {
            return;
        };
        if self.library.algorithm < case.algorithms.len() {
            return;
        }
        let algorithm = (*algorithm).to_owned();
        self.library.data.remove_custom(set, case.name, &algorithm);
        self.library.algorithm = self.library.algorithm.saturating_sub(1);
        self.after_library_choice(set, case, window, cx);
    }

    /// Saves a change of algorithm, which the trainer's drills follow.
    fn after_library_choice(
        &mut self,
        set: &'static AlgSet,
        case: &'static AlgCase,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        self.save_library(window, cx);
        if set.step != Step::Reference {
            let chosen = self.library.data.chosen(set, case);
            let custom = (chosen != case.algorithm).then(|| chosen.to_owned());
            self.follow_library_algorithm(set, case, custom, window, cx);
        }
        cx.notify();
    }

    /// Saves what a dialog was given. An algorithm must solve the case.
    fn set_library_text(
        &mut self,
        field: TextField,
        row: Row,
        text: &str,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> Result<(), String> {
        let (set, case) = row;
        match field {
            TextField::Algorithm => {
                check(set, case, text)
                    .map_err(|error| algorithm_error(self.strings(), case, error))?;
                self.library.data.add_custom(set, case, text);
                let algorithms = self.library.data.algorithms(set, case);
                let at = algorithms.iter().position(|known| *known == text);
                self.library.algorithm = at.unwrap_or(0);
                self.after_library_choice(set, case, window, cx);
            }
            TextField::Note => {
                self.library.data.set_note(set, case.name, text.to_owned());
                self.save_library(window, cx);
                cx.notify();
            }
        }
        Ok(())
    }

    /// Opens the dialog that adds an algorithm to the highlighted case, or
    /// edits its note.
    fn open_library_text(&self, field: TextField, window: &mut Window, cx: &mut Context<Self>) {
        let Some(row) = self.library.selected() else {
            return;
        };
        let (set, case) = row;
        let strings = self.strings();
        let is_note = field == TextField::Note;
        let current = if is_note {
            self.library.data.note(set, case.name).to_owned()
        } else {
            String::new()
        };
        let input = cx.new(|cx| {
            InputState::new(window, cx)
                .default_value(current)
                .placeholder(if is_note {
                    strings.library_note_placeholder
                } else {
                    case.algorithm
                })
        });
        let error = cx.new(|_| None::<String>);
        let entity = cx.entity().downgrade();
        let heading = if is_note {
            strings.library_edit_note
        } else {
            strings.library_add_algorithm
        };
        let title = format!("{heading} · {}", case.name);
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
                    let saved = app.set_library_text(field, row, &text, window, cx);
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
                            "library-text-field",
                            Input::new(&input).focus_bordered(false),
                        ))
                        .children(field_error(message.as_ref(), red))
                        .when(!is_note, |form| {
                            form.child(
                                div()
                                    .text_xs()
                                    .text_color(muted)
                                    .child(strings.library_add_hint),
                            )
                        }),
                )
                .footer(
                    h_flex()
                        .w_full()
                        .justify_end()
                        .gap_2()
                        .child(
                            Button::new("library-text-cancel")
                                .outline()
                                .label(strings.cancel)
                                .on_click(|_, window, cx| window.close_dialog(cx)),
                        )
                        .child(
                            Button::new("library-text-save")
                                .primary()
                                .label(strings.save)
                                .on_click(on_press(&save)),
                        ),
                )
        });
    }

    fn start_library_review(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let queue = self.library.due(&self.settings);
        if queue.is_empty() {
            self.toast(self.strings().library_review_empty, window, cx);
            return;
        }
        self.library.review = Some(ReviewRun {
            queue,
            revealed: false,
        });
        cx.notify();
    }

    pub(crate) fn toggle_library_reveal(&mut self, cx: &mut Context<Self>) {
        if let Some(review) = &mut self.library.review {
            review.revealed = !review.revealed;
            cx.notify();
        }
    }

    /// Grades the current case once its algorithm has been looked at, and
    /// moves on.
    fn grade_library_case(&mut self, grade: Grade, window: &mut Window, cx: &mut Context<Self>) {
        let Some(review) = &mut self.library.review else {
            return;
        };
        if !review.revealed || review.queue.is_empty() {
            return;
        }
        let (set, case) = review.queue.remove(0);
        review.revealed = false;
        let finished = review.queue.is_empty();
        self.library
            .data
            .grade(set.id, case.name, grade, now_unix_ms());
        self.save_library(window, cx);
        if finished {
            self.library.review = None;
            self.toast(self.strings().library_review_done, window, cx);
        }
        cx.notify();
    }

    /// The library screen: a review, or the sets to browse.
    pub(crate) fn render_library(&self, window: &mut Window, cx: &mut Context<Self>) -> AnyElement {
        if self.library.review.is_some() {
            return self.render_library_review(cx).into_any_element();
        }
        let (nav, detail) = library_panel_widths(window, self.settings.library_scale());
        h_flex()
            .flex_1()
            .min_h_0()
            .items_start()
            .child(self.render_library_nav(nav, cx))
            .child(
                v_flex()
                    .flex_1()
                    .min_w_0()
                    .h_full()
                    .child(self.render_library_header(cx))
                    .child(self.render_library_list(cx)),
            )
            .child(self.render_library_detail(detail, cx))
            .into_any_element()
    }

    /// The puzzles, each a dropdown of its methods and those of their sets.
    fn render_library_nav(&self, width: Rems, cx: &mut Context<Self>) -> impl IntoElement + use<> {
        let theme = cx.theme();
        let (border, background) = (theme.border, theme.background);
        let puzzles = visible_catalog(&self.settings);
        let items: Vec<AnyElement> = puzzles
            .iter()
            .enumerate()
            .map(|(index, puzzle)| self.render_nav_puzzle(index, puzzle, cx))
            .collect();
        FadeScroll::new("library-nav", background)
            .w(width)
            .flex_none()
            .h_full()
            .p_2()
            .gap_0p5()
            .border_r_1()
            .border_color(border)
            .children(items)
    }

    /// A puzzle, with how much of it is learned and, when open, its methods.
    fn render_nav_puzzle(
        &self,
        index: usize,
        puzzle: &LibraryPuzzle,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        let language = self.language();
        let open = self.library.open_puzzles.contains(&puzzle.event);
        let event = puzzle.event;
        let methods = puzzle.methods();
        let only_method = match methods.as_slice() {
            [(only, _)] => Some(*only),
            _ => None,
        };
        let learned: usize = puzzle
            .sets
            .iter()
            .map(|set| self.library.data.progress(set).learned)
            .sum();
        let total: usize = puzzle.sets.iter().map(|set| set.cases.len()).sum();
        let count = format!(
            "{}/{}",
            language.format_count(learned as u64),
            language.format_count(total as u64)
        );
        let row = tree_row(("library-puzzle", index), false, cx)
            .child(chevron(open, cx))
            .child(
                div()
                    .flex_1()
                    .min_w_0()
                    .truncate()
                    .font_weight(FontWeight::MEDIUM)
                    .child(puzzle.event.name()),
            )
            .child(tree_count(count, cx))
            .on_click(cx.listener(move |app, _, window, cx| {
                app.library.toggle_puzzle(event, only_method);
                app.focus.focus(window, cx);
                cx.notify();
            }));
        let methods = open.then(|| {
            let rows: Vec<AnyElement> = methods
                .into_iter()
                .enumerate()
                .map(|(at, (method, sets))| {
                    self.render_nav_method(index * 16 + at, event, method, &sets, cx)
                })
                .collect();
            tree_children(cx).children(rows)
        });
        v_flex()
            .w_full()
            .gap_0p5()
            .child(row)
            .children(methods)
            .into_any_element()
    }

    /// A method and, when open, its sets.
    fn render_nav_method(
        &self,
        id: usize,
        event: WcaEvent,
        method: &'static str,
        sets: &[&'static AlgSet],
        cx: &mut Context<Self>,
    ) -> AnyElement {
        let muted = cx.theme().muted_foreground;
        let open = self.library.open_methods.contains(&(event, method));
        let holds_current = sets.iter().any(|set| set.id == self.library.set.id);
        let row = tree_row(("library-method", id), false, cx)
            .text_sm()
            .child(chevron(open, cx))
            .child(
                div()
                    .flex_1()
                    .min_w_0()
                    .truncate()
                    .when(!holds_current, |name| name.text_color(muted))
                    .child(method),
            )
            .on_click(cx.listener(move |app, _, window, cx| {
                app.library.toggle_method(event, method);
                app.focus.focus(window, cx);
                cx.notify();
            }));
        let rows = open.then(|| {
            let rows: Vec<AnyElement> = sets
                .iter()
                .enumerate()
                .map(|(at, &set)| self.render_nav_set(id * 128 + at, set, cx))
                .collect();
            tree_children(cx).children(rows)
        });
        v_flex()
            .w_full()
            .gap_0p5()
            .child(row)
            .children(rows)
            .into_any_element()
    }

    /// A set, with how many of its cases are learned.
    fn render_nav_set(
        &self,
        id: usize,
        set: &'static AlgSet,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        let language = self.language();
        let current = self.library.query.is_empty() && set.id == self.library.set.id;
        let learned = self.library.data.progress(set).learned;
        let count = format!(
            "{}/{}",
            language.format_count(learned as u64),
            language.format_count(set.cases.len() as u64)
        );
        tree_row(("library-set", id), current, cx)
            .text_sm()
            .child(div().flex_1().min_w_0().truncate().child(set.name))
            .child(tree_count(count, cx))
            .on_click(cx.listener(move |app, _, window, cx| {
                app.select_library_set(set, window, cx);
                app.focus.focus(window, cx);
            }))
            .into_any_element()
    }

    /// The set's name, how much of it is learned, the reviews due and the
    /// search.
    fn render_library_header(&self, cx: &mut Context<Self>) -> impl IntoElement + use<> {
        let strings = self.strings();
        let language = self.language();
        let set = self.library.set;
        let theme = cx.theme();
        let (muted, border) = (theme.muted_foreground, theme.border);
        let progress = self.library.data.progress(set);
        let summary = strings
            .library_progress
            .replace("{learned}", &language.format_count(progress.learned as u64))
            .replace("{total}", &language.format_count(set.cases.len() as u64));
        let due = self.library.due(&self.settings).len();
        let due_label = strings
            .library_due
            .replace("{count}", &language.format_count(due as u64));
        let hint = self.keybinds().label(Action::FocusSearch);
        h_flex()
            .w_full()
            .flex_none()
            .p_2()
            .gap_3()
            .border_b_1()
            .border_color(border)
            .child(
                v_flex()
                    .flex_1()
                    .min_w_0()
                    .child(div().truncate().child(set.name))
                    .child(div().truncate().text_xs().text_color(muted).child(summary)),
            )
            .child(
                Button::new("library-review")
                    .outline()
                    .small()
                    .label(due_label)
                    .tab_stop(false)
                    .on_click(cx.listener(|app, _, window, cx| {
                        app.start_library_review(window, cx);
                        app.focus.focus(window, cx);
                    })),
            )
            .child(
                div()
                    .w(SEARCH_WIDTH)
                    .flex_none()
                    .child(search_field(&self.library.search, hint, cx).small()),
            )
    }

    /// The cases, as rows that are drawn only when they scroll into view.
    fn render_library_list(&self, cx: &mut Context<Self>) -> AnyElement {
        let strings = self.strings();
        let theme = cx.theme();
        let (muted, background) = (theme.muted_foreground, theme.background);
        let count = self.library.rows.len();
        if count == 0 {
            return div()
                .flex_1()
                .flex()
                .items_center()
                .justify_center()
                .p_3()
                .text_sm()
                .text_color(muted)
                .child(strings.library_no_cases)
                .into_any_element();
        }
        let scroll = &self.library.list_scroll;
        div()
            .relative()
            .flex_1()
            .min_h_0()
            .w_full()
            .child(
                fading_scroll(
                    uniform_list(
                        "library-cases",
                        count,
                        cx.processor(Self::render_library_rows),
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

    fn render_library_rows(
        &mut self,
        range: Range<usize>,
        _: &mut Window,
        cx: &mut Context<Self>,
    ) -> Vec<AnyElement> {
        let colors = Palette::of(cx);
        let theme = cx.theme();
        let (muted, selected_bg) = (theme.muted_foreground, theme.accent);
        let mono = theme.mono_font_family.clone();
        let scale = self.settings.library_scale();
        let searching = !self.library.query.is_empty();
        range
            .filter_map(|row| {
                let (set, case) = *self.library.rows.get(row)?;
                let status = self.library.data.status(set, case.name);
                let color = match status {
                    Status::NotStarted => muted,
                    Status::Learning => colors.yellow,
                    Status::Learned => colors.green,
                };
                let place = if searching {
                    format!("{} · {} · {}", set.event.name(), set.name, case.group)
                } else {
                    case.group.to_owned()
                };
                Some(
                    h_flex()
                        .id(("library-case", row))
                        .w_full()
                        .h(library_rem(ROW_HEIGHT, scale))
                        .px_2()
                        .gap_2()
                        .when(self.library.cursor == row, |line| line.bg(selected_bg))
                        .on_click(cx.listener(move |app, _, window, cx| {
                            app.library.cursor = row;
                            app.library.algorithm = 0;
                            app.focus.focus(window, cx);
                            cx.notify();
                        }))
                        .children(self.library.thumbnail(set, case).map(|image| {
                            img(image).size(library_rem(ROW_DIAGRAM, scale)).flex_none()
                        }))
                        .child(
                            v_flex()
                                .flex_1()
                                .min_w_0()
                                .child(
                                    h_flex()
                                        .gap_2()
                                        .child(div().truncate().text_sm().child(case.name))
                                        .child(
                                            div()
                                                .flex_none()
                                                .whitespace_nowrap()
                                                .text_xs()
                                                .text_color(muted)
                                                .child(place),
                                        ),
                                )
                                .child(
                                    div()
                                        .truncate()
                                        .text_xs()
                                        .font_family(mono.clone())
                                        .text_color(muted)
                                        .child(self.library.data.chosen(set, case).to_owned()),
                                ),
                        )
                        .child(self.render_status_menu(row, (set, case), status, color, cx))
                        .into_any_element(),
                )
            })
            .collect()
    }

    /// A case's status, as a dropdown of the others.
    fn render_status_menu(
        &self,
        id: usize,
        row: Row,
        status: Status,
        color: Hsla,
        cx: &mut Context<Self>,
    ) -> impl IntoElement + use<> {
        let strings = self.strings();
        let entity = cx.entity().downgrade();
        Button::new(("library-case-status", id))
            .ghost()
            .xsmall()
            .tab_stop(false)
            .child(
                h_flex()
                    .gap_1()
                    .items_center()
                    .text_xs()
                    .whitespace_nowrap()
                    .text_color(color)
                    .child(status_label(strings, status))
                    .child(Icon::new(IconName::ChevronDown).size(px(12.))),
            )
            .dropdown_menu(move |mut menu, _, _| {
                for option in [Status::NotStarted, Status::Learning, Status::Learned] {
                    let entity = entity.clone();
                    menu = menu.item(
                        PopupMenuItem::new(status_label(strings, option))
                            .checked(option == status)
                            .on_click(move |_, window, cx| {
                                entity
                                    .update(cx, |app, cx| {
                                        app.set_library_status(row, option, window, cx);
                                        app.focus.focus(window, cx);
                                    })
                                    .ok();
                            }),
                    );
                }
                menu
            })
    }

    /// The highlighted case: its drawing, what to look for, its algorithms
    /// and the user's note.
    fn render_library_detail(
        &self,
        width: Rems,
        cx: &mut Context<Self>,
    ) -> impl IntoElement + use<> {
        let strings = self.strings();
        let language = self.language();
        let theme = cx.theme();
        let (border, muted, background) = (theme.border, theme.muted_foreground, theme.background);
        let scale = self.settings.library_scale();
        let panel = FadeScroll::new("library-detail", background)
            .w(width)
            .flex_none()
            .h_full()
            .p_3()
            .gap_2()
            .border_l_1()
            .border_color(border);
        let Some((set, case)) = self.library.selected() else {
            return panel;
        };
        let note = self.library.data.note(set, case.name).to_owned();
        let hint = if case.recognition.is_empty() {
            recognition(set, case).map(|found| {
                strings
                    .library_recognition_counts
                    .replace("{up}", &language.format_count(found.facing_up as u64))
                    .replace("{total}", &language.format_count(found.top_stickers as u64))
                    .replace("{moved}", &language.format_count(found.moved as u64))
            })
        } else {
            Some(case.recognition.to_owned())
        };
        panel
            .items_center()
            .children(
                self.library
                    .diagram(set, case)
                    .map(|image| full_diagram(image, set, library_rem(DETAIL_DIAGRAM, scale))),
            )
            .child(div().text_center().child(case.name))
            .children(hint.map(|hint| {
                div()
                    .w_full()
                    .text_center()
                    .text_xs()
                    .text_color(muted)
                    .child(hint)
            }))
            .child(self.render_library_algorithms(set, case, cx))
            .child(
                h_flex()
                    .w_full()
                    .gap_1()
                    .flex_wrap()
                    .child(
                        Button::new("library-add")
                            .ghost()
                            .xsmall()
                            .icon(IconName::Plus)
                            .label(strings.library_add_algorithm)
                            .tab_stop(false)
                            .on_click(cx.listener(|app, _, window, cx| {
                                app.open_library_text(TextField::Algorithm, window, cx);
                            })),
                    )
                    .child(
                        Button::new("library-train")
                            .ghost()
                            .xsmall()
                            .icon(IconName::Timer)
                            .label(strings.library_train)
                            .tab_stop(false)
                            .on_click(cx.listener(|app, _, window, cx| {
                                app.train_library_case(window, cx);
                            })),
                    )
                    .child(
                        Button::new("library-note")
                            .ghost()
                            .xsmall()
                            .icon(IconName::Pencil)
                            .label(strings.library_edit_note)
                            .tab_stop(false)
                            .on_click(cx.listener(|app, _, window, cx| {
                                app.open_library_text(TextField::Note, window, cx);
                            })),
                    ),
            )
            .when(!note.is_empty(), |panel| {
                panel.child(div().w_full().text_sm().child(note))
            })
    }

    /// Every algorithm of a case, each with what can be done to it.
    fn render_library_algorithms(
        &self,
        set: &'static AlgSet,
        case: &'static AlgCase,
        cx: &mut Context<Self>,
    ) -> impl IntoElement + use<> {
        let strings = self.strings();
        let theme = cx.theme();
        let selected_bg = theme.accent;
        let mono = theme.mono_font_family.clone();
        let chosen = self.library.data.chosen(set, case).to_owned();
        let algorithms: Vec<String> = self
            .library
            .data
            .algorithms(set, case)
            .iter()
            .map(|&algorithm| algorithm.to_owned())
            .collect();
        let bundled = case.algorithms.len();
        let highlighted = self.library.algorithm;
        v_flex()
            .w_full()
            .gap_1()
            .children(
                algorithms
                    .into_iter()
                    .enumerate()
                    .map(|(index, algorithm)| {
                        let is_chosen = algorithm == chosen;
                        h_flex()
                            .id(("library-algorithm", index))
                            .w_full()
                            .gap_1()
                            .p_1()
                            .rounded(px(4.))
                            .when(index == highlighted, |line| line.bg(selected_bg))
                            .on_click(cx.listener(move |app, _, window, cx| {
                                app.library.algorithm = index;
                                app.focus.focus(window, cx);
                                cx.notify();
                            }))
                            .child(
                                Button::new(("library-favourite", index))
                                    .ghost()
                                    .xsmall()
                                    .icon(IconName::Star)
                                    .when(
                                        is_chosen,
                                        gpui_kit::component::button::ButtonVariants::primary,
                                    )
                                    .tooltip(strings.library_favourite)
                                    .accessibility_label(strings.library_favourite)
                                    .tab_stop(false)
                                    .on_click(cx.listener(move |app, _, window, cx| {
                                        cx.stop_propagation();
                                        app.library.algorithm = index;
                                        app.toggle_library_favourite(window, cx);
                                        app.focus.focus(window, cx);
                                    })),
                            )
                            .child(
                                div()
                                    .flex_1()
                                    .min_w_0()
                                    .text_sm()
                                    .font_family(mono.clone())
                                    .child(algorithm),
                            )
                            .when(index >= bundled, |line| {
                                line.child(
                                    Button::new(("library-remove", index))
                                        .ghost()
                                        .xsmall()
                                        .icon(IconName::Trash)
                                        .tooltip(strings.library_remove_algorithm)
                                        .accessibility_label(strings.library_remove_algorithm)
                                        .tab_stop(false)
                                        .on_click(cx.listener(move |app, _, window, cx| {
                                            cx.stop_propagation();
                                            app.library.algorithm = index;
                                            app.remove_library_algorithm(window, cx);
                                            app.focus.focus(window, cx);
                                        })),
                                )
                            })
                    }),
            )
    }

    /// The case under review. Its algorithm stays hidden until asked for, and
    /// only then can it be graded.
    fn render_library_review(&self, cx: &mut Context<Self>) -> impl IntoElement + use<> {
        let strings = self.strings();
        let language = self.language();
        let theme = cx.theme();
        let (muted, background) = (theme.muted_foreground, theme.background);
        let mono = theme.mono_font_family.clone();
        let scale = self.settings.library_scale();
        let review = self.library.review.as_ref();
        let current = review.and_then(|review| review.queue.first().copied());
        let revealed = review.is_some_and(|review| review.revealed);
        let left = review.map_or(0, |review| review.queue.len());
        let page = FadeScroll::new("library-review-page", background)
            .flex_1()
            .min_h_0()
            .w_full()
            .items_center()
            .gap_3()
            .p_4();
        let Some((set, case)) = current else {
            return page;
        };
        let grades = [Grade::Again, Grade::Hard, Grade::Good, Grade::Easy].map(|grade| {
            Button::new(("library-grade", grade as usize))
                .outline()
                .label(grade_label(strings, grade))
                .tab_stop(false)
                .on_click(cx.listener(move |app, _, window, cx| {
                    app.grade_library_case(grade, window, cx);
                    app.focus.focus(window, cx);
                }))
        });
        page.child(
            div().text_sm().text_color(muted).child(
                strings
                    .library_due
                    .replace("{count}", &language.format_count(left as u64)),
            ),
        )
        .children(
            self.library
                .diagram(set, case)
                .map(|image| full_diagram(image, set, library_rem(REVIEW_DIAGRAM, scale))),
        )
        .child(div().text_center().child(case.name))
        .child(div().text_xs().text_color(muted).child(format!(
            "{} · {}",
            set.event.name(),
            set.name
        )))
        .when(revealed, |page| {
            let note = self.library.data.note(set, case.name).to_owned();
            page.child(
                div()
                    .text_center()
                    .font_family(mono)
                    .child(self.library.data.chosen(set, case).to_owned()),
            )
            .when(!note.is_empty(), |page| {
                page.child(div().text_sm().text_color(muted).child(note))
            })
            .child(
                h_flex()
                    .gap_2()
                    .flex_wrap()
                    .justify_center()
                    .children(grades),
            )
        })
        .when(!revealed, |page| {
            page.child(
                Button::new("library-reveal")
                    .primary()
                    .label(strings.trainer_reveal)
                    .tab_stop(false)
                    .on_click(cx.listener(|app, _, window, cx| {
                        app.toggle_library_reveal(cx);
                        app.focus.focus(window, cx);
                    })),
            )
        })
    }
}
