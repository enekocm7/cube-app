//! The algorithm trainer: drilling the cases of a set, such as PLL, one timed
//! attempt at a time.
//!
//! A set is a block of the library's data files, which is the one copy of the
//! cases: see [`crate::library`]. The trainer drills those with a [`Step`] that
//! says how to scramble into a case, and the picker, the drill, the history and
//! the stats follow from that.
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
mod small;
mod stage;
mod three_by_three;

pub use cube::{CustomAlgError, check_algorithm, generate_scramble};
pub use data::{Attempt, CaseStats, TrainerData, slowest_first, weighted_cases};
pub use deck::Deck;
pub(crate) use diagram::sticker_fills;
pub use diagram::{case_diagram, case_thumbnail, diagram_ratio};
pub use drill::{Drill, Prompt};
pub use grid::{GridStep, grid_step, rows_before};
pub use stage::STAGES;
pub use three_by_three::{OLL, PLL};

use std::sync::{LazyLock, OnceLock};

use crate::event::WcaEvent;
use crate::library::catalog;
use crate::library::model::{Model, Region};
use crate::settings::Settings;

/// What solving a set's cases achieves, which decides how a case is set up,
/// when an algorithm counts as solving it, and how it is drawn.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Step {
    /// Every last layer piece ends up facing up, in any position.
    OrientLastLayer,
    /// The last layer starts oriented and ends up solved.
    PermuteLastLayer,
    /// A case is set up exactly: the algorithm's inverse with a free turn on
    /// either side. It suits any set whose cases say all there is to know.
    Setup,
    /// Like [`Setup`](Self::Setup), with the last layer scrambled first, for
    /// cases such as an F2L pair that work however the last layer is.
    RandomLastLayer,
    /// Each case is a stage of the solve with no algorithm: a random state
    /// with everything before that stage solved.
    Stage,
    /// Listed in the library, with no way yet to drill it.
    Reference,
}

/// One case of a set. Its name is what saved data refers to it by, so it must
/// not change, and like an event's name it is the same in every language.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct AlgCase {
    pub name: &'static str,
    /// The group the case belongs to; those of a group sit together in `cases`.
    pub group: &'static str,
    /// How to spot the case.
    pub recognition: &'static str,
    /// The default algorithm, which also defines the case.
    pub algorithm: &'static str,
    /// Every algorithm the data lists, the default first.
    pub algorithms: &'static [&'static str],
}

/// A set of cases to train. Its id is what saved data refers to it by.
#[derive(Debug)]
pub struct AlgSet {
    pub id: &'static str,
    pub name: &'static str,
    /// What the set is listed under in the library, such as CFOP.
    pub method: &'static str,
    /// The event whose puzzle the set belongs to.
    pub event: WcaEvent,
    pub step: Step,
    /// What is solved in every case, as a [`Region`].
    pub before_text: &'static str,
    /// What an algorithm of the set must solve, as a [`Region`].
    pub after_text: &'static str,
    /// The turns allowed on either side of an algorithm.
    pub turn_text: &'static str,
    /// Which stickers are drawn in color, by where they began, as a [`Region`].
    pub show_text: &'static str,
    /// Whether cases are drawn as a solid and not from above.
    pub solid_view: bool,
    /// Where the algorithms came from.
    pub source: &'static str,
    pub cases: &'static [AlgCase],
    pub(crate) regions: OnceLock<[Region; 3]>,
}

impl AlgSet {
    /// Returns whether the set is made of stages and not of algorithm cases.
    pub fn is_stage(&self) -> bool {
        self.step == Step::Stage
    }

    /// Returns the rules of the set's puzzle.
    pub fn model(&self) -> &'static Model {
        Model::of(self.event)
    }

    /// Reads the set's regions. The tests rule out one that can't be read.
    fn regions(&self) -> &[Region; 3] {
        self.regions.get_or_init(|| {
            let read = |text: &str| {
                self.model()
                    .region(text)
                    .unwrap_or_else(|| panic!("{} has a bad region: {text}", self.id))
            };
            [
                read(self.before_text),
                read(self.after_text),
                read(self.show_text),
            ]
        })
    }

    /// Returns what is solved in every case of the set.
    pub fn before(&self) -> &Region {
        &self.regions()[0]
    }

    /// Returns what an algorithm of the set must solve.
    pub fn after(&self) -> &Region {
        &self.regions()[1]
    }

    /// Returns which stickers are drawn in color.
    pub fn show(&self) -> &Region {
        &self.regions()[2]
    }

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
    pub sets: Vec<&'static AlgSet>,
}

impl TrainerPuzzle {
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

/// Returns every puzzle the trainer can drill, in the order they are listed.
pub fn puzzles() -> &'static [TrainerPuzzle] {
    static PUZZLES: LazyLock<Vec<TrainerPuzzle>> = LazyLock::new(|| {
        let drilled = catalog().iter().map(|puzzle| {
            let sets = puzzle.sets.iter().copied();
            let mut sets: Vec<&'static AlgSet> =
                sets.filter(|set| set.step != Step::Reference).collect();
            // The shortest set is the one to start from.
            sets.sort_by_key(|set| set.cases.len());
            // The stages of a solve come after the sets of cases.
            if puzzle.event == WcaEvent::Cube3x3 {
                sets.push(*STAGES);
            }
            TrainerPuzzle {
                event: puzzle.event,
                sets,
            }
        });
        drilled.filter(|puzzle| !puzzle.sets.is_empty()).collect()
    });
    &PUZZLES
}

/// Returns every set the trainer can drill, in the order they are listed.
pub fn all_sets() -> impl Iterator<Item = &'static AlgSet> {
    puzzles()
        .iter()
        .flat_map(|puzzle| puzzle.sets.iter().copied())
}

/// Returns the puzzles and sets the trainer offers, without the sets the user
/// left out of training.
#[must_use]
pub fn puzzles_for(settings: &Settings) -> Vec<TrainerPuzzle> {
    puzzles()
        .iter()
        .filter_map(|puzzle| {
            let sets: Vec<&'static AlgSet> = puzzle
                .sets
                .iter()
                .copied()
                .filter(|set| settings.includes_set_in_training(set.id))
                .collect();
            (!sets.is_empty()).then_some(TrainerPuzzle {
                event: puzzle.event,
                sets,
            })
        })
        .collect()
}

/// Returns every set the trainer offers.
pub fn all_sets_for(settings: &Settings) -> impl Iterator<Item = &'static AlgSet> + '_ {
    puzzles_for(settings)
        .into_iter()
        .flat_map(|puzzle| puzzle.sets.into_iter())
}

/// Returns the first set the trainer offers, if any.
pub fn first_trainable_set(settings: &Settings) -> Option<&'static AlgSet> {
    all_sets_for(settings).next()
}

/// Returns the set that follows `set` in the trainer, going back to the first
/// after the last. `None` when the trainer offers no set.
#[must_use]
pub fn next_set_for(settings: &Settings, set: &AlgSet) -> Option<&'static AlgSet> {
    let sets: Vec<&'static AlgSet> = all_sets_for(settings).collect();
    if sets.is_empty() {
        return None;
    }
    let index = sets.iter().position(|other| other.id == set.id);
    Some(sets[index.map_or(0, |index| (index + 1) % sets.len())])
}

/// Returns the event of the puzzle `set` belongs to.
pub const fn set_event(set: &AlgSet) -> WcaEvent {
    set.event
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
        assert_eq!(found(&PLL, "epll"), ["H", "Ua", "Ub", "Z"]);
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
        assert_eq!(groups[0].name, "EPLL");
        assert_eq!(PLL.matching(PLL.groups(), "nothing"), [] as [CaseGroup; 0]);
    }

    #[test]
    fn a_search_works_on_any_layout() {
        let everything = CaseGroup {
            name: "",
            cases: (0..PLL.cases.len()).collect(),
        };
        let groups = PLL.matching(vec![everything], "epll");
        assert_eq!(groups[0].cases.len(), 4);
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
        let sets: Vec<&AlgSet> = all_sets().collect();
        for (index, set) in sets.iter().enumerate() {
            let next = sets[(index + 1) % sets.len()];
            assert_eq!(next_set(set).id, next.id);
        }
    }

    #[test]
    fn the_stages_are_the_last_set_of_the_cube() {
        let cube = puzzles()
            .iter()
            .find(|puzzle| puzzle.event == WcaEvent::Cube3x3);
        let sets = &cube.unwrap().sets;
        assert!(sets.last().unwrap().is_stage());
        assert_eq!(sets.iter().filter(|set| set.is_stage()).count(), 1);
    }

    #[test]
    fn the_trainer_offers_a_second_puzzle() {
        let events: Vec<WcaEvent> = puzzles().iter().map(|puzzle| puzzle.event).collect();
        assert!(events.contains(&WcaEvent::Cube3x3));
        assert!(events.contains(&WcaEvent::Cube2x2));
    }

    #[test]
    fn every_generated_scramble_produces_the_chosen_case() {
        let mut random = tnoodle::java::JavaRandom::new(41);
        for set in all_sets().filter(|set| !set.is_stage()) {
            let model = set.model();
            for case in set.cases {
                let algorithm = model.parse(case.algorithm).unwrap();
                for _ in 0..2 {
                    let scramble = generate_scramble(set, case, &mut random)
                        .unwrap_or_else(|error| panic!("{} {}: {error}", set.id, case.name));
                    let mut state = model.solved();
                    let ops = model.parse(&scramble).unwrap();
                    assert!(model.apply(&mut state, &ops));
                    assert!(
                        crate::library::solves_from(set, &state, &algorithm),
                        "{} {}: {scramble}",
                        set.id,
                        case.name
                    );
                }
            }
        }
    }

    #[test]
    fn a_set_knows_its_event() {
        assert_eq!(set_event(&OLL), WcaEvent::Cube3x3);
    }

    #[test]
    fn a_set_left_out_of_training_is_not_offered() {
        let mut settings = Settings::default();
        settings.include_set_in_training("pll", false);
        let offered: Vec<&str> = all_sets_for(&settings).map(|set| set.id).collect();
        assert!(!offered.contains(&"pll"));
        assert!(offered.contains(&"oll"));
        let puzzles = puzzles_for(&settings);
        assert!(
            puzzles
                .iter()
                .flat_map(|puzzle| &puzzle.sets)
                .all(|set| set.id != "pll")
        );
        // Moving on from OLL skips PLL, wherever the two sit in the list.
        let oll = crate::library::find_set("oll").unwrap();
        let sets: Vec<&AlgSet> = all_sets_for(&settings).collect();
        let at = sets.iter().position(|set| set.id == "oll").unwrap();
        assert_eq!(next_set_for(&settings, oll).unwrap().id, sets[at + 1].id);
        assert_ne!(next_set_for(&settings, oll).unwrap().id, "pll");
    }

    #[test]
    fn moving_on_with_every_set_in_training_keeps_coming_round() {
        let settings = Settings::default();
        let sets: Vec<&AlgSet> = all_sets_for(&settings).collect();
        for (index, set) in sets.iter().enumerate() {
            let next = sets[(index + 1) % sets.len()];
            assert_eq!(next_set_for(&settings, set).unwrap().id, next.id);
        }
    }
}
