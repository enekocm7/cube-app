//! What the library saves: each case's status, the user's algorithms and
//! notes, and when it is next due for review.

use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};

use super::review::{Grade, Review};
use crate::trainer::{AlgCase, AlgSet, TrainerData, puzzles};

/// How far along learning a case is.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Status {
    #[default]
    NotStarted,
    Learning,
    Learned,
}

impl Status {
    /// Returns the next status, going back to the first after the last.
    #[must_use]
    pub const fn next(self) -> Self {
        match self {
            Self::NotStarted => Self::Learning,
            Self::Learning => Self::Learned,
            Self::Learned => Self::NotStarted,
        }
    }

    #[allow(
        clippy::trivially_copy_pass_by_ref,
        reason = "serde's skip_serializing_if takes a reference"
    )]
    const fn is_default(&self) -> bool {
        matches!(self, Self::NotStarted)
    }
}

/// What the user keeps about one case. A case with nothing here is not saved.
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default)]
pub struct CaseEntry {
    #[serde(skip_serializing_if = "Status::is_default")]
    pub status: Status,
    /// The algorithm the user chose, from the library or their own.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub favourite: Option<String>,
    /// The user's own algorithms.
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub custom: Vec<String>,
    #[serde(skip_serializing_if = "String::is_empty")]
    pub note: String,
    /// Set while the case is learned.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub review: Option<Review>,
}

impl CaseEntry {
    fn is_empty(&self) -> bool {
        *self == Self::default()
    }
}

/// A case due for review, by set id and case name.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct DueCase {
    pub set: String,
    pub case: String,
    pub due_unix_ms: u64,
}

/// How much of a set has been learned.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Progress {
    pub learning: usize,
    pub learned: usize,
}

/// The contents of `library.json`.
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default)]
pub struct LibraryData {
    /// The entries by set id and case name.
    #[serde(skip_serializing_if = "BTreeMap::is_empty")]
    cases: BTreeMap<String, BTreeMap<String, CaseEntry>>,
}

impl LibraryData {
    /// Returns what is kept about a case, if anything.
    pub fn entry(&self, set: &AlgSet, name: &str) -> Option<&CaseEntry> {
        self.cases.get(set.id)?.get(name)
    }

    /// Changes a case's entry, dropping it if nothing is left in it.
    fn update<T>(&mut self, set: &str, name: &str, change: impl FnOnce(&mut CaseEntry) -> T) -> T {
        let cases = self.cases.entry(set.to_owned()).or_default();
        let entry = cases.entry(name.to_owned()).or_default();
        let result = change(entry);
        if entry.is_empty() {
            cases.remove(name);
        }
        if cases.is_empty() {
            self.cases.remove(set);
        }
        result
    }

    pub fn status(&self, set: &AlgSet, name: &str) -> Status {
        self.entry(set, name)
            .map_or(Status::default(), |entry| entry.status)
    }

    /// Sets a case's status. Learning a case starts its reviews, and anything
    /// else ends them.
    pub fn set_status(&mut self, set: &AlgSet, name: &str, status: Status, now_unix_ms: u64) {
        self.update(set.id, name, |entry| {
            entry.status = status;
            if status == Status::Learned {
                entry.review.get_or_insert(Review::first(now_unix_ms));
            } else {
                entry.review = None;
            }
        });
    }

    /// Moves a case on to its next status and returns it.
    pub fn cycle_status(&mut self, set: &AlgSet, name: &str, now_unix_ms: u64) -> Status {
        let status = self.status(set, name).next();
        self.set_status(set, name, status, now_unix_ms);
        status
    }

    /// Returns every algorithm of a case: the library's, then the user's.
    pub fn algorithms<'a>(&'a self, set: &AlgSet, case: &'a AlgCase) -> Vec<&'a str> {
        let mut algorithms: Vec<&str> = case.algorithms.to_vec();
        if let Some(entry) = self.entry(set, case.name) {
            algorithms.extend(entry.custom.iter().map(String::as_str));
        }
        algorithms
    }

    /// Returns the algorithm to show for a case: the user's choice, or else
    /// the first in the library.
    pub fn chosen<'a>(&'a self, set: &AlgSet, case: &'a AlgCase) -> &'a str {
        self.entry(set, case.name)
            .and_then(|entry| entry.favourite.as_deref())
            .unwrap_or(case.algorithm)
    }

    /// Chooses an algorithm, or clears the choice if it was the chosen one.
    /// Returns whether it is now chosen.
    pub fn toggle_favourite(&mut self, set: &AlgSet, name: &str, algorithm: &str) -> bool {
        self.update(set.id, name, |entry| {
            if entry.favourite.as_deref() == Some(algorithm) {
                entry.favourite = None;
                false
            } else {
                entry.favourite = Some(algorithm.to_owned());
                true
            }
        })
    }

    /// Adds one of the user's algorithms, which is then the chosen one. An
    /// algorithm already in the library is only chosen, not added twice.
    /// Check it with [`check`](super::check) first.
    pub fn add_custom(&mut self, set: &AlgSet, case: &AlgCase, algorithm: &str) {
        let known = case.algorithms.contains(&algorithm);
        self.update(set.id, case.name, |entry| {
            if !known && !entry.custom.iter().any(|other| other == algorithm) {
                entry.custom.push(algorithm.to_owned());
            }
            entry.favourite = Some(algorithm.to_owned());
        });
    }

    /// Takes on the trainer's algorithm for a case: `None` is its default,
    /// which clears the choice. The trainer checks what it is given.
    pub fn adopt(&mut self, set: &AlgSet, case: &AlgCase, algorithm: Option<&str>) {
        match algorithm {
            Some(algorithm) => self.add_custom(set, case, algorithm),
            None => self.update(set.id, case.name, |entry| entry.favourite = None),
        }
    }

    /// Picks up the algorithms chosen in the trainer, for cases with no choice
    /// here yet.
    pub fn adopt_trainer(&mut self, trainer: &TrainerData) {
        for puzzle in puzzles() {
            for &set in &puzzle.sets {
                for case in set.cases {
                    let chosen = self.entry(set, case.name);
                    if chosen.is_none_or(|entry| entry.favourite.is_none())
                        && let Some(algorithm) = trainer.custom_algorithm(set, case.name)
                    {
                        self.add_custom(set, case, algorithm);
                    }
                }
            }
        }
    }

    /// Removes one of the user's algorithms, and the choice of it.
    pub fn remove_custom(&mut self, set: &AlgSet, name: &str, algorithm: &str) {
        self.update(set.id, name, |entry| {
            entry.custom.retain(|other| other != algorithm);
            if entry.favourite.as_deref() == Some(algorithm) {
                entry.favourite = None;
            }
        });
    }

    pub fn note(&self, set: &AlgSet, name: &str) -> &str {
        self.entry(set, name)
            .map_or("", |entry| entry.note.as_str())
    }

    pub fn set_note(&mut self, set: &AlgSet, name: &str, note: String) {
        self.update(set.id, name, |entry| entry.note = note);
    }

    /// Counts how much of `set` is under way.
    pub fn progress(&self, set: &AlgSet) -> Progress {
        let mut progress = Progress::default();
        for case in set.cases {
            match self.status(set, case.name) {
                Status::NotStarted => {}
                Status::Learning => progress.learning += 1,
                Status::Learned => progress.learned += 1,
            }
        }
        progress
    }

    /// Returns the learned cases due at `now_unix_ms`, the most overdue first.
    pub fn due(&self, now_unix_ms: u64) -> Vec<DueCase> {
        let mut due = Vec::new();
        for (set, cases) in &self.cases {
            for (case, entry) in cases {
                if let Some(review) = entry.review.filter(|review| review.is_due(now_unix_ms)) {
                    due.push(DueCase {
                        set: set.clone(),
                        case: case.clone(),
                        due_unix_ms: review.due_unix_ms,
                    });
                }
            }
        }
        due.sort_by_key(|case| case.due_unix_ms);
        due
    }

    /// Records how a review went. A forgotten case stays learned and simply
    /// comes back sooner.
    pub fn grade(&mut self, set: &str, name: &str, grade: Grade, now_unix_ms: u64) {
        self.update(set, name, |entry| {
            if let Some(review) = &mut entry.review {
                review.grade(grade, now_unix_ms);
            }
        });
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::trainer::{OLL, PLL};

    const NOW: u64 = 1_000_000;

    #[test]
    fn the_library_picks_up_what_the_trainer_chose() {
        let algorithm = "U R U R' U' R' F R2 U' R' U' R U R' F'";
        let case = PLL.case("T").unwrap();
        let mut trainer = TrainerData::default();
        trainer.set_custom_algorithm(&PLL, "T", Some(algorithm.to_owned()));
        let mut data = LibraryData::default();
        data.adopt_trainer(&trainer);
        assert_eq!(data.chosen(&PLL, case), algorithm);

        data.adopt(&PLL, case, None);
        assert_eq!(data.chosen(&PLL, case), case.algorithm);
    }

    #[test]
    fn a_choice_made_in_the_library_is_kept_over_the_trainer_s() {
        let case = PLL.case("T").unwrap();
        let mut trainer = TrainerData::default();
        trainer.set_custom_algorithm(&PLL, "T", Some("R U R'".to_owned()));
        let mut data = LibraryData::default();
        data.toggle_favourite(&PLL, "T", case.algorithms[1]);
        data.adopt_trainer(&trainer);
        assert_eq!(data.chosen(&PLL, case), case.algorithms[1]);
    }
    const DAY_MS: u64 = 24 * 60 * 60 * 1000;

    #[test]
    fn a_new_library_is_saved_as_nothing() {
        let data: LibraryData = serde_json::from_str("{}").unwrap();
        assert_eq!(data, LibraryData::default());
        assert_eq!(serde_json::to_string(&data).unwrap(), "{}");
    }

    #[test]
    fn unknown_keys_are_tolerated() {
        let text = r#"{"future":1,"cases":{"pll":{"T":{"status":"learning","future":2}}}}"#;
        let data: LibraryData = serde_json::from_str(text).unwrap();
        assert_eq!(data.status(&PLL, "T"), Status::Learning);
    }

    #[test]
    fn the_status_comes_round() {
        let mut data = LibraryData::default();
        assert_eq!(data.status(&PLL, "T"), Status::NotStarted);
        assert_eq!(data.cycle_status(&PLL, "T", NOW), Status::Learning);
        assert_eq!(data.cycle_status(&PLL, "T", NOW), Status::Learned);
        assert_eq!(data.cycle_status(&PLL, "T", NOW), Status::NotStarted);
        assert_eq!(data, LibraryData::default());
    }

    #[test]
    fn only_learned_cases_are_reviewed() {
        let mut data = LibraryData::default();
        data.set_status(&PLL, "T", Status::Learning, NOW);
        data.set_status(&PLL, "Z", Status::Learned, NOW);
        let due = data.due(NOW + DAY_MS);
        assert_eq!(due.len(), 1);
        assert_eq!(due[0].case, "Z");
        assert_eq!(data.due(NOW), Vec::new());
    }

    #[test]
    fn leaving_learned_ends_the_reviews() {
        let mut data = LibraryData::default();
        data.set_status(&PLL, "T", Status::Learned, NOW);
        data.set_status(&PLL, "T", Status::Learning, NOW);
        assert_eq!(data.due(NOW + 10 * DAY_MS), Vec::new());
    }

    #[test]
    fn staying_learned_keeps_the_schedule() {
        let mut data = LibraryData::default();
        data.set_status(&PLL, "T", Status::Learned, NOW);
        data.grade("pll", "T", Grade::Good, NOW);
        let before = data.entry(&PLL, "T").unwrap().review;
        data.set_status(&PLL, "T", Status::Learned, NOW + DAY_MS);
        assert_eq!(data.entry(&PLL, "T").unwrap().review, before);
    }

    #[test]
    fn the_most_overdue_case_comes_first() {
        let mut data = LibraryData::default();
        data.set_status(&PLL, "T", Status::Learned, NOW + 5);
        data.set_status(&OLL, "OLL 27", Status::Learned, NOW);
        let due = data.due(NOW + 2 * DAY_MS);
        let names: Vec<&str> = due.iter().map(|case| case.case.as_str()).collect();
        assert_eq!(names, ["OLL 27", "T"]);
    }

    #[test]
    fn a_graded_case_leaves_the_queue() {
        let mut data = LibraryData::default();
        data.set_status(&PLL, "T", Status::Learned, NOW);
        let later = NOW + DAY_MS;
        assert_eq!(data.due(later).len(), 1);
        data.grade("pll", "T", Grade::Good, later);
        assert_eq!(data.due(later), Vec::new());
    }

    #[test]
    fn the_chosen_algorithm_starts_as_the_first_in_the_library() {
        let mut data = LibraryData::default();
        let case = PLL.case("T").unwrap();
        assert_eq!(data.chosen(&PLL, case), case.algorithm);
        assert!(data.toggle_favourite(&PLL, "T", "R U"));
        assert_eq!(data.chosen(&PLL, case), "R U");
        assert!(!data.toggle_favourite(&PLL, "T", "R U"));
        assert_eq!(data.chosen(&PLL, case), case.algorithm);
        assert_eq!(data, LibraryData::default());
    }

    #[test]
    fn the_users_own_algorithm_is_listed_and_chosen() {
        let mut data = LibraryData::default();
        let case = PLL.case("T").unwrap();
        data.add_custom(&PLL, case, "R U");
        data.add_custom(&PLL, case, "R U");
        assert_eq!(data.algorithms(&PLL, case).last(), Some(&"R U"));
        let count = data.algorithms(&PLL, case).len();
        assert_eq!(count, case.algorithms.len() + 1);
        assert_eq!(data.chosen(&PLL, case), "R U");
        data.remove_custom(&PLL, "T", "R U");
        assert_eq!(data.chosen(&PLL, case), case.algorithm);
        assert_eq!(data, LibraryData::default());
    }

    #[test]
    fn an_algorithm_already_in_the_library_is_not_listed_twice() {
        let mut data = LibraryData::default();
        let case = PLL.case("T").unwrap();
        data.add_custom(&PLL, case, case.algorithms[0]);
        assert_eq!(data.algorithms(&PLL, case).len(), case.algorithms.len());
    }

    #[test]
    fn notes_are_kept_until_emptied() {
        let mut data = LibraryData::default();
        data.set_note(&PLL, "T", "Fingertrick the U".to_owned());
        assert_eq!(data.note(&PLL, "T"), "Fingertrick the U");
        data.set_note(&PLL, "T", String::new());
        assert_eq!(data, LibraryData::default());
    }

    #[test]
    fn progress_counts_each_status() {
        let mut data = LibraryData::default();
        data.set_status(&PLL, "T", Status::Learned, NOW);
        data.set_status(&PLL, "Z", Status::Learning, NOW);
        data.set_status(&PLL, "No longer a case", Status::Learned, NOW);
        let progress = data.progress(&PLL);
        assert_eq!((progress.learning, progress.learned), (1, 1));
    }

    #[test]
    fn saved_data_loads_back() {
        let mut data = LibraryData::default();
        data.set_status(&PLL, "T", Status::Learned, NOW);
        data.add_custom(&PLL, PLL.case("T").unwrap(), "R U");
        data.set_note(&PLL, "Z", "note".to_owned());
        let text = serde_json::to_string(&data).unwrap();
        assert_eq!(serde_json::from_str::<LibraryData>(&text).unwrap(), data);
    }
}
