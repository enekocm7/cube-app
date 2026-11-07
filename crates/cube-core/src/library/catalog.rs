//! The library's sets, read from the data files embedded in the binary.
//!
//! A new set needs no code: it is a block in its puzzle's file. A block is
//! lines of a keyword and its value:
//!
//! ```text
//! set zbll-t                a unique id, which saved data refers to
//! name ZBLL T
//! method ZB                 what the set is listed under
//! before all -layer:U       a region solved in every case of the set
//! after all                 a region an algorithm must solve
//! turn U                    turns allowed on either side of an algorithm
//! show all                  stickers to draw in color, by where they began
//! view top                  `top` or `solid`
//! step setup                only for a set the trainer drills: `orient`,
//!                           `permute`, `setup` or `random-ll`
//! source speedcubedb        where the algorithms came from
//! group T                   the group of the cases that follow
//! case T 1 | recognition | algorithm | algorithm
//! ```
//!
//! Regions are read by [`Model::region`](super::model::Model::region).

use std::sync::{LazyLock, OnceLock};

use crate::event::WcaEvent;
use crate::settings::Settings;
use crate::trainer::{AlgCase, AlgSet, Step};

/// A puzzle and the sets of its library.
#[derive(Debug)]
pub struct LibraryPuzzle {
    pub event: WcaEvent,
    pub sets: Vec<&'static AlgSet>,
}

impl LibraryPuzzle {
    /// Returns the puzzle's methods with their sets, each method once, in the
    /// order they first come up.
    #[must_use]
    pub fn methods(&self) -> Vec<(&'static str, Vec<&'static AlgSet>)> {
        let mut methods: Vec<(&'static str, Vec<&'static AlgSet>)> = Vec::new();
        for &set in &self.sets {
            match methods.iter_mut().find(|(name, _)| *name == set.method) {
                Some((_, sets)) => sets.push(set),
                None => methods.push((set.method, vec![set])),
            }
        }
        methods
    }
}

/// The data files, in the order the puzzles are listed.
const FILES: [(WcaEvent, &str); 12] = [
    (
        WcaEvent::Cube2x2,
        include_str!("../../data/library/2x2.txt"),
    ),
    (
        WcaEvent::Cube3x3,
        include_str!("../../data/library/3x3.txt"),
    ),
    (
        WcaEvent::Cube4x4,
        include_str!("../../data/library/4x4.txt"),
    ),
    (
        WcaEvent::Cube5x5,
        include_str!("../../data/library/5x5.txt"),
    ),
    (
        WcaEvent::Cube6x6,
        include_str!("../../data/library/6x6.txt"),
    ),
    (
        WcaEvent::Cube7x7,
        include_str!("../../data/library/7x7.txt"),
    ),
    (
        WcaEvent::Megaminx,
        include_str!("../../data/library/megaminx.txt"),
    ),
    (
        WcaEvent::Pyraminx,
        include_str!("../../data/library/pyraminx.txt"),
    ),
    (WcaEvent::Fto, include_str!("../../data/library/fto.txt")),
    (
        WcaEvent::Skewb,
        include_str!("../../data/library/skewb.txt"),
    ),
    (
        WcaEvent::Square1,
        include_str!("../../data/library/square1.txt"),
    ),
    (
        WcaEvent::Clock,
        include_str!("../../data/library/clock.txt"),
    ),
];

/// A set being read.
#[derive(Default)]
struct Draft {
    id: &'static str,
    name: &'static str,
    method: &'static str,
    before: &'static str,
    after: &'static str,
    turn: &'static str,
    show: &'static str,
    view: &'static str,
    step: &'static str,
    source: &'static str,
    cases: Vec<AlgCase>,
}

impl Draft {
    fn finish(self, event: WcaEvent) -> &'static AlgSet {
        let step = match self.step {
            "orient" => Step::OrientLastLayer,
            "permute" => Step::PermuteLastLayer,
            "setup" => Step::Setup,
            "random-ll" => Step::RandomLastLayer,
            _ => Step::Reference,
        };
        Box::leak(Box::new(AlgSet {
            id: self.id,
            name: self.name,
            method: self.method,
            event,
            step,
            before_text: self.before,
            after_text: self.after,
            turn_text: self.turn,
            show_text: self.show,
            solid_view: self.view == "solid",
            source: self.source,
            cases: Box::leak(self.cases.into_boxed_slice()),
            regions: OnceLock::new(),
        }))
    }
}

/// Reads one case line. The first algorithm is the one that defines the case.
fn read_case(line: &'static str, group: &'static str) -> AlgCase {
    let mut parts = line.split('|').map(str::trim);
    let name = parts.next().unwrap_or_default();
    let recognition = parts.next().unwrap_or_default();
    let algorithms: Vec<&'static str> = parts.filter(|part| !part.is_empty()).collect();
    let algorithms: &'static [&'static str] = Box::leak(algorithms.into_boxed_slice());
    AlgCase {
        name,
        group,
        recognition,
        algorithm: algorithms.first().copied().unwrap_or_default(),
        algorithms,
    }
}

/// Reads the sets of one data file.
fn read_file(event: WcaEvent, text: &'static str) -> Vec<&'static AlgSet> {
    let mut sets = Vec::new();
    let mut draft: Option<Draft> = None;
    let mut group = "";
    for line in text.lines().map(str::trim) {
        if line.is_empty() || line.starts_with('#') {
            continue;
        }
        let (keyword, value) = line.split_once(' ').unwrap_or((line, ""));
        let value = value.trim();
        if keyword == "set" {
            sets.extend(draft.take().map(|done| done.finish(event)));
            draft = Some(Draft {
                id: value,
                ..Draft::default()
            });
            group = "";
            continue;
        }
        let Some(set) = &mut draft else {
            continue;
        };
        match keyword {
            "name" => set.name = value,
            "method" => set.method = value,
            "before" => set.before = value,
            "after" => set.after = value,
            "turn" => set.turn = value,
            "show" => set.show = value,
            "view" => set.view = value,
            "step" => set.step = value,
            "source" => set.source = value,
            "group" => group = value,
            "case" => set.cases.push(read_case(value, group)),
            _ => {}
        }
    }
    sets.extend(draft.map(|done| done.finish(event)));
    sets
}

/// Returns every puzzle of the library, in the order they are listed.
pub fn catalog() -> &'static [LibraryPuzzle] {
    static CATALOG: LazyLock<Vec<LibraryPuzzle>> = LazyLock::new(|| {
        FILES
            .iter()
            .map(|&(event, text)| LibraryPuzzle {
                event,
                sets: read_file(event, text),
            })
            .collect()
    });
    &CATALOG
}

/// Returns every set of the library.
pub fn library_sets() -> impl Iterator<Item = &'static AlgSet> {
    let puzzles = catalog().iter();
    puzzles.flat_map(|puzzle| puzzle.sets.iter().copied())
}

/// Returns every puzzle with the sets shown in the library, leaving out a
/// puzzle whose every set is hidden.
#[must_use]
pub fn visible_catalog(settings: &Settings) -> Vec<LibraryPuzzle> {
    catalog()
        .iter()
        .filter_map(|puzzle| {
            let sets: Vec<&'static AlgSet> = puzzle
                .sets
                .iter()
                .copied()
                .filter(|set| settings.shows_set_in_library(set.id))
                .collect();
            (!sets.is_empty()).then_some(LibraryPuzzle {
                event: puzzle.event,
                sets,
            })
        })
        .collect()
}

/// Returns every set shown in the library.
pub fn visible_sets(settings: &Settings) -> impl Iterator<Item = &'static AlgSet> + '_ {
    catalog()
        .iter()
        .flat_map(move |puzzle| puzzle.sets.iter().copied())
        .filter(move |set| settings.shows_set_in_library(set.id))
}

/// Returns the first set shown in the library, if any.
pub fn first_visible_set(settings: &Settings) -> Option<&'static AlgSet> {
    visible_sets(settings).next()
}

/// Finds the cases of every set shown in the library whose names, groups, sets
/// or methods have all the words of `query`.
#[must_use]
pub fn search_visible_cases(
    settings: &Settings,
    query: &str,
) -> Vec<(&'static AlgSet, &'static AlgCase)> {
    search_cases_with(query, |set| settings.shows_set_in_library(set.id))
}

/// Finds the cases of every set whose names, groups, sets or methods have all
/// the words of `query`.
pub fn search_cases(query: &str) -> Vec<(&'static AlgSet, &'static AlgCase)> {
    search_cases_with(query, |_| true)
}

/// Finds the cases of the sets that `keep` accepts.
fn search_cases_with(
    query: &str,
    keep: impl Fn(&AlgSet) -> bool,
) -> Vec<(&'static AlgSet, &'static AlgCase)> {
    let mut found = Vec::new();
    for puzzle in catalog() {
        for &set in &puzzle.sets {
            if !keep(set) {
                continue;
            }
            let event = puzzle.event.name();
            for case in set.cases {
                let fields = [case.name, case.group, set.name, set.method, event];
                if crate::search::matches(query, &fields) {
                    found.push((set, case));
                }
            }
        }
    }
    found
}

/// Returns the set with the id `id`.
pub fn find_set(id: &str) -> Option<&'static AlgSet> {
    library_sets().find(|set| set.id == id)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_block_reads_as_a_set() {
        let text = "# A comment\nset demo\nname Demo\nmethod M\nbefore all\nafter all\nturn U\n\
                    show all\nview solid\nsource me\ngroup First\ncase One | Look | R U | U R\n\
                    group Second\ncase Two |  | R\n";
        let sets = read_file(WcaEvent::Cube3x3, text);
        assert_eq!(sets.len(), 1);
        let set = sets[0];
        assert_eq!((set.id, set.name, set.method), ("demo", "Demo", "M"));
        assert!(set.solid_view);
        assert_eq!(set.step, Step::Reference);
        assert_eq!(set.cases.len(), 2);
        assert_eq!(set.cases[0].name, "One");
        assert_eq!(set.cases[0].group, "First");
        assert_eq!(set.cases[0].recognition, "Look");
        assert_eq!(set.cases[0].algorithm, "R U");
        assert_eq!(set.cases[0].algorithms, ["R U", "U R"]);
        assert_eq!(set.cases[1].recognition, "");
        assert_eq!(set.cases[1].group, "Second");
    }

    #[test]
    fn every_block_starts_a_new_set() {
        let text = "set one\ngroup G\ncase A | | R\nset two\ncase B | | U\n";
        let sets = read_file(WcaEvent::Cube3x3, text);
        assert_eq!(sets.len(), 2);
        assert_eq!(sets[1].cases[0].group, "");
    }

    #[test]
    fn a_hidden_set_leaves_the_tree_search_and_neighbouring_puzzles() {
        let mut settings = Settings::default();
        settings.show_set_in_library("pll", false);
        let puzzles = visible_catalog(&settings);
        assert!(
            puzzles
                .iter()
                .flat_map(|puzzle| &puzzle.sets)
                .all(|set| set.id != "pll")
        );
        assert!(visible_sets(&settings).all(|set| set.id != "pll"));
        assert!(
            search_visible_cases(&settings, "pll")
                .iter()
                .all(|(set, _)| set.id != "pll")
        );
        // The rest of the puzzle is still there.
        assert!(
            puzzles
                .iter()
                .flat_map(|puzzle| &puzzle.sets)
                .any(|set| set.id == "oll")
        );
    }

    #[test]
    fn a_puzzle_with_every_set_hidden_is_left_out() {
        let mut settings = Settings::default();
        for set in &catalog()
            .iter()
            .find(|puzzle| puzzle.event == WcaEvent::Cube2x2)
            .unwrap()
            .sets
        {
            settings.show_set_in_library(set.id, false);
        }
        let puzzles = visible_catalog(&settings);
        assert!(
            puzzles
                .iter()
                .all(|puzzle| puzzle.event != WcaEvent::Cube2x2)
        );
        assert!(
            puzzles
                .iter()
                .any(|puzzle| puzzle.event == WcaEvent::Cube3x3)
        );
    }
}
