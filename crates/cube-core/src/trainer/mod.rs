//! The algorithm trainer: drilling the cases of a set, such as PLL, one timed
//! attempt at a time.
//!
//! Everything here works from [`PUZZLES`], a catalog of puzzles and their
//! [`AlgSet`]s. A new set is its case data and one more line in the catalog;
//! the picker, the drill, the history and the stats follow from that. A set
//! that is solved differently also needs a [`Step`].
//!
//! Attempts live in `trainer.json` and never in `times.json`, so they stay out
//! of sessions, averages and charts.

mod cube;
mod data;
mod deck;
mod diagram;
mod drill;
mod grid;
pub mod notation;
mod three_by_three;

pub use cube::{CustomAlgError, check_algorithm, generate_scramble};
pub use data::{Attempt, CaseStats, TrainerData, slowest_first};
pub use deck::Deck;
pub use diagram::case_diagram;
pub use drill::{Drill, Prompt};
pub use grid::{GridStep, grid_step, rows_before};
pub use three_by_three::{OLL, PLL};

use crate::event::WcaEvent;

/// What solving a set's cases achieves, which decides how a case is set up,
/// when an algorithm counts as solving it, and how it is drawn.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Step {
    /// Every last layer piece ends up facing up, in any position.
    OrientLastLayer,
    /// The last layer starts oriented and ends up solved.
    PermuteLastLayer,
}

/// One case of a set. Its name is what saved data refers to it by, so it must
/// not change, and like an event's name it is the same in every language.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct AlgCase {
    pub name: &'static str,
    /// The group the case belongs to; those of a group sit together in `cases`.
    pub group: &'static str,
    /// The default algorithm, which also defines the case.
    pub algorithm: &'static str,
}

/// A set of cases to train. Its id is what saved data refers to it by.
#[derive(Debug)]
pub struct AlgSet {
    pub id: &'static str,
    pub name: &'static str,
    pub step: Step,
    pub cases: &'static [AlgCase],
}

impl AlgSet {
    /// Returns the position of the case called `name`.
    pub fn position(&self, name: &str) -> Option<usize> {
        self.cases.iter().position(|case| case.name == name)
    }

    /// Returns the case called `name`.
    pub fn case(&self, name: &str) -> Option<&'static AlgCase> {
        self.position(name).map(|index| &self.cases[index])
    }

    /// Lays the cases out in their groups, in the order of the set.
    pub fn groups(&self) -> Vec<CaseGroup> {
        let mut groups: Vec<CaseGroup> = Vec::new();
        for (index, case) in self.cases.iter().enumerate() {
            match groups.last_mut() {
                Some(group) if group.name == case.group => group.cases.push(index),
                _ => groups.push(CaseGroup {
                    name: case.group,
                    cases: vec![index],
                }),
            }
        }
        groups
    }

    /// Keeps the cases of `groups` that match `query`, leaving out any group
    /// with none left. Every word of the query must begin a word of the case's
    /// name or group, so `t` finds T and the T shapes, not every name with a t.
    /// An empty query keeps everything.
    pub fn matching(&self, mut groups: Vec<CaseGroup>, query: &str) -> Vec<CaseGroup> {
        let query = query.to_lowercase();
        let found = |case: &AlgCase| {
            let text = format!("{} {}", case.name, case.group).to_lowercase();
            query
                .split_whitespace()
                .all(|word| text.split_whitespace().any(|part| part.starts_with(word)))
        };
        for group in &mut groups {
            group.cases.retain(|&case| found(&self.cases[case]));
        }
        groups.retain(|group| !group.cases.is_empty());
        groups
    }
}

/// Cases shown together, as positions in the set.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct CaseGroup {
    pub name: &'static str,
    pub cases: Vec<usize>,
}

/// A puzzle and the sets that train it.
#[derive(Debug)]
pub struct TrainerPuzzle {
    /// The event whose scrambles and drawings the puzzle shares.
    pub event: WcaEvent,
    pub sets: &'static [&'static AlgSet],
}

/// Every puzzle the trainer knows, in the order they are listed.
pub static PUZZLES: &[TrainerPuzzle] = &[TrainerPuzzle {
    event: WcaEvent::Cube3x3,
    sets: &[&PLL, &OLL],
}];

/// Returns every set, in the order they are listed.
pub fn all_sets() -> impl Iterator<Item = &'static AlgSet> {
    PUZZLES
        .iter()
        .flat_map(|puzzle| puzzle.sets.iter().copied())
}

/// Returns the event of the puzzle `set` belongs to.
pub fn set_event(set: &AlgSet) -> WcaEvent {
    PUZZLES
        .iter()
        .find(|puzzle| puzzle.sets.iter().any(|other| other.id == set.id))
        .map_or(WcaEvent::Cube3x3, |puzzle| puzzle.event)
}

/// Returns the set that follows `set`, going back to the first after the last.
pub fn next_set(set: &AlgSet) -> &'static AlgSet {
    let sets: Vec<&'static AlgSet> = all_sets().collect();
    let index = sets.iter().position(|other| other.id == set.id);
    sets[index.map_or(0, |index| (index + 1) % sets.len())]
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::HashSet;

    fn found(set: &AlgSet, query: &str) -> Vec<&'static str> {
        let groups = set.matching(set.groups(), query);
        let cases = groups.iter().flat_map(|group| group.cases.iter());
        cases.map(|&case| set.cases[case].name).collect()
    }

    #[test]
    fn an_empty_search_keeps_every_case() {
        assert_eq!(PLL.matching(PLL.groups(), "  "), PLL.groups());
    }

    #[test]
    fn a_search_finds_a_case_by_name_ignoring_case() {
        assert_eq!(found(&PLL, "aA"), ["Aa"]);
        assert_eq!(found(&OLL, "oll 27"), ["OLL 27"]);
    }

    #[test]
    fn a_search_finds_a_whole_group() {
        assert_eq!(found(&PLL, "edges only"), ["Ua", "Ub", "H", "Z"]);
    }

    #[test]
    fn a_search_only_looks_at_the_start_of_words() {
        assert_eq!(found(&PLL, "t"), ["T"]);
        assert_eq!(found(&PLL, "dges"), [] as [&str; 0]);
    }

    #[test]
    fn groups_with_nothing_found_are_left_out() {
        let groups = PLL.matching(PLL.groups(), "ua");
        assert_eq!(groups.len(), 1);
        assert_eq!(groups[0].name, "Edges only");
        assert_eq!(PLL.matching(PLL.groups(), "nothing"), [] as [CaseGroup; 0]);
    }

    #[test]
    fn a_search_works_on_any_layout() {
        let everything = CaseGroup {
            name: "",
            cases: (0..PLL.cases.len()).collect(),
        };
        let groups = PLL.matching(vec![everything], "corners");
        assert_eq!(groups[0].cases.len(), 3);
    }

    #[test]
    fn set_ids_and_case_names_are_unique() {
        let mut ids = HashSet::new();
        for set in all_sets() {
            assert!(ids.insert(set.id), "{} is used twice", set.id);
            let mut names = HashSet::new();
            for case in set.cases {
                assert!(names.insert(case.name), "{} is used twice", case.name);
            }
        }
    }

    #[test]
    fn the_sets_hold_every_case() {
        assert_eq!(PLL.cases.len(), 21);
        assert_eq!(OLL.cases.len(), 57);
    }

    #[test]
    fn every_oll_number_is_there() {
        for number in 1..=57 {
            assert!(OLL.case(&format!("OLL {number}")).is_some());
        }
    }

    #[test]
    fn a_group_is_never_split() {
        for set in all_sets() {
            let groups = set.groups();
            let names: HashSet<&str> = groups.iter().map(|group| group.name).collect();
            assert_eq!(names.len(), groups.len(), "{} splits a group", set.name);
            let count: usize = groups.iter().map(|group| group.cases.len()).sum();
            assert_eq!(count, set.cases.len());
        }
    }

    #[test]
    fn sets_come_round_in_order() {
        assert_eq!(next_set(&PLL).id, "oll");
        assert_eq!(next_set(&OLL).id, "pll");
    }

    #[test]
    fn a_set_knows_its_event() {
        assert_eq!(set_event(&OLL), WcaEvent::Cube3x3);
    }
}
