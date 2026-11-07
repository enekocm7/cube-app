//! What the trainer saves: which cases are selected, the algorithms that
//! replace the defaults, and every attempt.

use std::collections::{BTreeMap, HashMap};
use std::hash::BuildHasher;

use serde::{Deserialize, Serialize};

use super::{AlgCase, AlgSet, CaseGroup};
use crate::solve::Modifier;

/// Time added by a `+2` penalty.
const PLUS_TWO_MS: u64 = 2_000;

/// How many of a case's latest timed attempts its recent average covers.
const RECENT_ATTEMPTS: usize = 5;

/// One timed attempt at a case.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Attempt {
    /// The id of the set.
    pub set: String,
    /// The name of the case.
    pub case: String,
    pub time_ms: u64,
    #[serde(default)]
    pub modifier: Modifier,
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub scramble: String,
    #[serde(default)]
    pub solved_at_unix_ms: u64,
}

impl Attempt {
    /// Returns the time that counts, or `None` for a DNF.
    pub const fn effective_ms(&self) -> Option<u64> {
        match self.modifier {
            Modifier::None => Some(self.time_ms),
            Modifier::PlusTwo => Some(self.time_ms + PLUS_TWO_MS),
            Modifier::DNF => None,
        }
    }

    /// Turns a penalty on, or off when it is already on.
    pub fn toggle_modifier(&mut self, modifier: Modifier) {
        self.modifier = if self.modifier == modifier {
            Modifier::None
        } else {
            modifier
        };
    }
}

/// How a case has gone so far. A DNF counts as an attempt and nothing else.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct CaseStats {
    pub attempts: usize,
    pub best_ms: Option<u64>,
    pub mean_ms: Option<u64>,
    /// The mean of the latest few timed attempts.
    pub recent_ms: Option<u64>,
}

/// The contents of `trainer.json`.
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default)]
pub struct TrainerData {
    /// The names of the selected cases, by set id.
    #[serde(skip_serializing_if = "BTreeMap::is_empty")]
    selections: BTreeMap<String, Vec<String>>,
    /// The algorithms that replace a case's default, by set id and case name.
    #[serde(skip_serializing_if = "BTreeMap::is_empty")]
    custom_algorithms: BTreeMap<String, BTreeMap<String, String>>,
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub attempts: Vec<Attempt>,
}

impl TrainerData {
    /// Returns whether the case called `name` is selected.
    pub fn is_selected(&self, set: &AlgSet, name: &str) -> bool {
        self.selections
            .get(set.id)
            .is_some_and(|names| names.iter().any(|selected| selected == name))
    }

    /// Returns the positions of the selected cases, in the order of the set.
    /// Names no longer in the set are left out.
    pub fn selected(&self, set: &AlgSet) -> Vec<usize> {
        let selected = |case: &&AlgCase| self.is_selected(set, case.name);
        let positions = set.cases.iter().enumerate();
        positions
            .filter(|(_, case)| selected(case))
            .map(|(index, _)| index)
            .collect()
    }

    /// Selects or clears the cases at `positions`.
    fn set_selected(&mut self, set: &AlgSet, positions: &[usize], selected: bool) {
        let names = self.selections.entry(set.id.to_owned()).or_default();
        for &position in positions {
            let name = set.cases[position].name;
            let found = names.iter().position(|other| other == name);
            match (found, selected) {
                (None, true) => names.push(name.to_owned()),
                (Some(index), false) => {
                    names.remove(index);
                }
                _ => {}
            }
        }
        if names.is_empty() {
            self.selections.remove(set.id);
        }
    }

    /// Selects exactly the cases at `positions`, clearing any others.
    pub fn select_only(&mut self, set: &AlgSet, positions: &[usize]) {
        self.selections.remove(set.id);
        self.set_selected(set, positions, true);
    }

    /// Selects the cases at `positions`, or clears them if all are selected.
    pub fn toggle_selected(&mut self, set: &AlgSet, positions: &[usize]) {
        let all = positions
            .iter()
            .all(|&position| self.is_selected(set, set.cases[position].name));
        self.set_selected(set, positions, !all);
    }

    /// Returns the algorithm to show for `case`.
    pub fn algorithm<'a>(&'a self, set: &AlgSet, case: &'a AlgCase) -> &'a str {
        self.custom_algorithm(set, case.name)
            .unwrap_or(case.algorithm)
    }

    /// Returns the algorithm replacing the default of the case called `name`.
    pub fn custom_algorithm(&self, set: &AlgSet, name: &str) -> Option<&str> {
        self.custom_algorithms
            .get(set.id)?
            .get(name)
            .map(String::as_str)
    }

    /// Replaces a case's algorithm, or brings back the default with `None`.
    /// Check it with [`check_algorithm`](super::check_algorithm) first.
    pub fn set_custom_algorithm(&mut self, set: &AlgSet, name: &str, algorithm: Option<String>) {
        match algorithm {
            Some(algorithm) => {
                let of_set = self.custom_algorithms.entry(set.id.to_owned()).or_default();
                of_set.insert(name.to_owned(), algorithm);
            }
            None => {
                if let Some(of_set) = self.custom_algorithms.get_mut(set.id) {
                    of_set.remove(name);
                    if of_set.is_empty() {
                        self.custom_algorithms.remove(set.id);
                    }
                }
            }
        }
    }

    /// Returns where the attempts at the case called `name` are in
    /// [`attempts`](Self::attempts), newest first.
    pub fn case_attempts(&self, set: &AlgSet, name: &str) -> Vec<usize> {
        let attempts = self.attempts.iter().enumerate().rev();
        attempts
            .filter(|(_, attempt)| attempt.set == set.id && attempt.case == name)
            .map(|(index, _)| index)
            .collect()
    }

    /// Returns how many of the latest attempts at a case went by without a
    /// DNF, counting back from the newest.
    pub fn clean_streak(&self, set: &AlgSet, name: &str) -> usize {
        let attempts = self.case_attempts(set, name);
        attempts
            .into_iter()
            .take_while(|&index| self.attempts[index].modifier != Modifier::DNF)
            .count()
    }

    /// Returns how each case of `set` has gone, by name.
    pub fn stats(&self, set: &AlgSet) -> HashMap<&str, CaseStats> {
        let mut by_case: HashMap<&str, (usize, Vec<u64>)> = HashMap::new();
        for attempt in self.attempts.iter().filter(|attempt| attempt.set == set.id) {
            let (attempts, timed) = by_case.entry(attempt.case.as_str()).or_default();
            *attempts += 1;
            timed.extend(attempt.effective_ms());
        }
        by_case
            .into_iter()
            .map(|(name, (attempts, timed))| {
                let recent = timed.len().saturating_sub(RECENT_ATTEMPTS);
                let stats = CaseStats {
                    attempts,
                    best_ms: timed.iter().copied().min(),
                    mean_ms: mean(&timed),
                    recent_ms: mean(&timed[recent..]),
                };
                (name, stats)
            })
            .collect()
    }
}

fn mean(times: &[u64]) -> Option<u64> {
    let total: u64 = times.iter().sum();
    (!times.is_empty()).then(|| total / times.len() as u64)
}

/// Lays the cases out slowest first, in one group. A case with no time yet
/// comes last, since nothing says it is slow.
pub fn slowest_first<S: BuildHasher>(
    set: &AlgSet,
    stats: &HashMap<&str, CaseStats, S>,
) -> Vec<CaseGroup> {
    let mut cases: Vec<usize> = (0..set.cases.len()).collect();
    let mean = |case: usize| {
        stats
            .get(set.cases[case].name)
            .and_then(|case| case.mean_ms)
    };
    cases.sort_by_key(|&case| std::cmp::Reverse(mean(case)));
    vec![CaseGroup { name: "", cases }]
}

/// Returns the cases to deal, with the ones that need the most work in
/// several times: three for a case that is much slower than the rest or has
/// only had DNFs, two for one that is slower or has not been tried, one for
/// the others. Everything is dealt once before anything comes back, so a fast
/// case still comes up in every round.
pub fn weighted_cases<S: BuildHasher>(
    set: &AlgSet,
    selected: &[usize],
    stats: &HashMap<&str, CaseStats, S>,
) -> Vec<usize> {
    let of = |case: usize| stats.get(set.cases[case].name);
    let times: Vec<u64> = selected
        .iter()
        .filter_map(|&case| of(case).and_then(|stats| stats.recent_ms))
        .collect();
    let average = mean(&times);
    let copies = |case: usize| match (of(case), average) {
        (None, _) => 2,
        (Some(stats), _) if stats.attempts == 0 => 2,
        (Some(stats), Some(average)) => match stats.recent_ms {
            None => 3,
            Some(time) if time * 10 >= average * 15 => 3,
            Some(time) if time * 10 >= average * 11 => 2,
            Some(_) => 1,
        },
        (Some(_), None) => 3,
    };
    // With nothing timed yet there is nothing to weigh.
    if times.is_empty() && selected.iter().all(|&case| of(case).is_none()) {
        return selected.to_vec();
    }
    selected
        .iter()
        .flat_map(|&case| std::iter::repeat_n(case, copies(case)))
        .collect()
}

#[cfg(test)]
mod tests {
    use super::super::{OLL, PLL};
    use super::*;

    const NONE: [usize; 0] = [];

    fn attempt(case: &str, time_ms: u64, modifier: Modifier) -> Attempt {
        Attempt {
            set: "pll".to_owned(),
            case: case.to_owned(),
            time_ms,
            modifier,
            scramble: "R U".to_owned(),
            solved_at_unix_ms: 1,
        }
    }

    fn with_attempts(attempts: Vec<Attempt>) -> TrainerData {
        TrainerData {
            attempts,
            ..TrainerData::default()
        }
    }

    #[test]
    fn nothing_is_selected_at_first() {
        assert_eq!(TrainerData::default().selected(&PLL), NONE);
    }

    #[test]
    fn toggling_a_case_selects_and_clears_it() {
        let mut data = TrainerData::default();
        data.toggle_selected(&PLL, &[2]);
        assert!(data.is_selected(&PLL, PLL.cases[2].name));
        assert_eq!(data.selected(&PLL), [2]);
        data.toggle_selected(&PLL, &[2]);
        assert_eq!(data.selected(&PLL), NONE);
    }

    #[test]
    fn toggling_a_group_fills_it_before_clearing_it() {
        let mut data = TrainerData::default();
        data.toggle_selected(&PLL, &[1]);
        data.toggle_selected(&PLL, &[0, 1, 2]);
        assert_eq!(data.selected(&PLL), [0, 1, 2]);
        data.toggle_selected(&PLL, &[0, 1, 2]);
        assert_eq!(data.selected(&PLL), NONE);
    }

    #[test]
    fn sets_keep_their_own_selection() {
        let mut data = TrainerData::default();
        data.toggle_selected(&PLL, &[0]);
        assert_eq!(data.selected(&OLL), NONE);
    }

    #[test]
    fn a_selected_name_no_longer_in_the_set_is_ignored() {
        let text = r#"{"selections":{"pll":["T","Gone"]}}"#;
        let data: TrainerData = serde_json::from_str(text).unwrap();
        assert_eq!(data.selected(&PLL), [PLL.position("T").unwrap()]);
    }

    #[test]
    fn a_custom_algorithm_replaces_the_default_until_removed() {
        let mut data = TrainerData::default();
        let case = PLL.case("T").unwrap();
        assert_eq!(data.algorithm(&PLL, case), case.algorithm);
        data.set_custom_algorithm(&PLL, "T", Some("R U".to_owned()));
        assert_eq!(data.algorithm(&PLL, case), "R U");
        data.set_custom_algorithm(&PLL, "T", None);
        assert_eq!(data.algorithm(&PLL, case), case.algorithm);
        assert_eq!(data, TrainerData::default());
    }

    #[test]
    fn stats_follow_the_attempts_of_each_case() {
        let data = with_attempts(vec![
            attempt("T", 3_000, Modifier::None),
            attempt("T", 1_000, Modifier::None),
            attempt("T", 2_000, Modifier::DNF),
            attempt("Z", 4_000, Modifier::PlusTwo),
        ]);
        let stats = data.stats(&PLL);
        assert_eq!(
            stats["T"],
            CaseStats {
                attempts: 3,
                best_ms: Some(1_000),
                mean_ms: Some(2_000),
                recent_ms: Some(2_000),
            }
        );
        assert_eq!(stats["Z"].best_ms, Some(6_000));
    }

    #[test]
    fn a_case_of_only_dnfs_has_no_times() {
        let data = with_attempts(vec![attempt("T", 1_000, Modifier::DNF)]);
        let stats = data.stats(&PLL);
        assert_eq!(stats["T"].attempts, 1);
        assert_eq!(stats["T"].mean_ms, None);
    }

    #[test]
    fn the_recent_average_covers_the_latest_attempts() {
        let data = with_attempts(
            (1..=7)
                .map(|second| attempt("T", second * 1_000, Modifier::None))
                .collect(),
        );
        assert_eq!(data.stats(&PLL)["T"].recent_ms, Some(5_000));
    }

    #[test]
    fn stats_stay_within_their_set() {
        let data = with_attempts(vec![attempt("T", 1_000, Modifier::None)]);
        assert!(data.stats(&OLL).is_empty());
    }

    #[test]
    fn the_slowest_case_comes_first_and_the_untimed_last() {
        let data = with_attempts(vec![
            attempt("Z", 1_000, Modifier::None),
            attempt("T", 3_000, Modifier::None),
        ]);
        let stats = data.stats(&PLL);
        let groups = slowest_first(&PLL, &stats);
        assert_eq!(groups.len(), 1);
        let cases = &groups[0].cases;
        assert_eq!(cases.len(), PLL.cases.len());
        assert_eq!(PLL.cases[cases[0]].name, "T");
        assert_eq!(PLL.cases[cases[1]].name, "Z");
    }

    #[test]
    fn a_case_lists_its_own_attempts_newest_first() {
        let mut other_set = attempt("T", 1_000, Modifier::None);
        other_set.set = "oll".to_owned();
        let data = with_attempts(vec![
            attempt("T", 1_000, Modifier::None),
            attempt("Z", 2_000, Modifier::None),
            other_set,
            attempt("T", 3_000, Modifier::DNF),
        ]);
        assert_eq!(data.case_attempts(&PLL, "T"), [3, 0]);
        assert_eq!(data.case_attempts(&PLL, "Y"), NONE);
    }

    #[test]
    fn penalties_toggle() {
        let mut attempt = attempt("T", 1_000, Modifier::None);
        attempt.toggle_modifier(Modifier::PlusTwo);
        assert_eq!(attempt.effective_ms(), Some(3_000));
        attempt.toggle_modifier(Modifier::DNF);
        assert_eq!(attempt.effective_ms(), None);
        attempt.toggle_modifier(Modifier::DNF);
        assert_eq!(attempt.effective_ms(), Some(1_000));
    }

    #[test]
    fn an_empty_file_loads_and_an_empty_save_stays_minimal() {
        let data: TrainerData = serde_json::from_str("{}").unwrap();
        assert_eq!(data, TrainerData::default());
        assert_eq!(serde_json::to_string(&data).unwrap(), "{}");
    }

    #[test]
    fn unknown_keys_are_tolerated() {
        let data: Result<TrainerData, _> = serde_json::from_str(r#"{"new_field":1}"#);
        assert!(data.is_ok());
    }

    fn timed(case: &str, time_ms: u64) -> Attempt {
        attempt(case, time_ms, Modifier::None)
    }

    #[test]
    fn selecting_only_some_cases_clears_the_rest() {
        let mut data = TrainerData::default();
        data.toggle_selected(&PLL, &[0, 1, 2]);
        data.select_only(&PLL, &[5, 6]);
        assert_eq!(data.selected(&PLL), [5, 6]);
        data.select_only(&PLL, &[]);
        assert_eq!(data, TrainerData::default());
    }

    #[test]
    fn the_clean_streak_stops_at_a_dnf() {
        let data = with_attempts(vec![
            timed("T", 1_000),
            attempt("T", 1_000, Modifier::DNF),
            timed("T", 1_000),
            timed("Z", 1_000),
            timed("T", 1_000),
        ]);
        assert_eq!(data.clean_streak(&PLL, "T"), 2);
        assert_eq!(data.clean_streak(&PLL, "H"), 0);
    }

    #[test]
    fn slower_cases_are_dealt_more_often() {
        let t = PLL.position("T").unwrap();
        let z = PLL.position("Z").unwrap();
        let h = PLL.position("H").unwrap();
        let mut attempts = Vec::new();
        attempts.extend((0..3).map(|_| timed("T", 4_000)));
        attempts.extend((0..3).map(|_| timed("Z", 1_000)));
        let data = with_attempts(attempts);
        let dealt = weighted_cases(&PLL, &[t, z, h], &data.stats(&PLL));
        let count = |case: usize| dealt.iter().filter(|&&other| other == case).count();
        assert_eq!(count(t), 3);
        assert_eq!(count(z), 1);
        // A case that has not been tried needs practice too.
        assert_eq!(count(h), 2);
    }

    #[test]
    fn a_dnf_only_case_counts_as_the_slowest() {
        let t = PLL.position("T").unwrap();
        let z = PLL.position("Z").unwrap();
        let data = with_attempts(vec![attempt("T", 1_000, Modifier::DNF), timed("Z", 1_000)]);
        let dealt = weighted_cases(&PLL, &[t, z], &data.stats(&PLL));
        assert_eq!(dealt.iter().filter(|&&case| case == t).count(), 3);
    }

    #[test]
    fn with_no_history_every_case_is_dealt_once() {
        let data = TrainerData::default();
        let stats = data.stats(&PLL);
        assert_eq!(weighted_cases(&PLL, &[3, 1, 2], &stats), [3, 1, 2]);
    }

    #[test]
    fn similar_times_are_all_dealt_once() {
        let t = PLL.position("T").unwrap();
        let z = PLL.position("Z").unwrap();
        let data = with_attempts(vec![timed("T", 1_000), timed("Z", 1_050)]);
        let dealt = weighted_cases(&PLL, &[t, z], &data.stats(&PLL));
        assert_eq!(dealt, [t, z]);
    }
}
