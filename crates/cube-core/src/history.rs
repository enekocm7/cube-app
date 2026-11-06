//! A session: a named, chronological list of solves.

use serde::{Deserialize, Serialize};

use crate::solve::{Modifier, Solve, SolveSnapshot};
use crate::tags::TagChange;

/// One session. Field names are part of the save format shared with cube-tui.
#[derive(Clone, Debug, Default, Serialize, Deserialize)]
pub struct History {
    #[serde(default, skip_serializing_if = "String::is_empty")]
    session_name: String,
    times: Vec<Solve>,
}

impl History {
    pub const fn new() -> Self {
        Self {
            session_name: String::new(),
            times: Vec::new(),
        }
    }

    /// Returns the custom session name; empty for an unnamed session.
    pub fn name(&self) -> &str {
        &self.session_name
    }

    pub fn set_name(&mut self, name: String) {
        self.session_name = name;
    }

    /// Returns all solves in chronological order.
    pub fn solves(&self) -> &[Solve] {
        &self.times
    }

    pub const fn len(&self) -> usize {
        self.times.len()
    }

    pub const fn is_empty(&self) -> bool {
        self.times.is_empty()
    }

    /// Appends a solve, marking it when it beats the session's best single.
    pub fn push(&mut self, mut solve: Solve) {
        let best = self.times.iter().filter_map(Solve::effective_ms).min();
        solve.was_single_record = solve
            .effective_ms()
            .is_some_and(|ms| best.is_none_or(|best| ms < best));
        self.times.push(solve);
    }

    /// Applies an edit to the solve at `index`; returns whether anything changed.
    pub fn edit(&mut self, index: usize, after: SolveSnapshot) -> bool {
        let changed = self.times.get_mut(index).is_some_and(|s| s.edit(after));
        if changed {
            self.remark_single_records();
        }
        changed
    }

    /// Replaces the tags of the solve at `index`; returns whether they changed.
    pub fn set_tags(&mut self, index: usize, tags: Vec<String>) -> bool {
        match self.times.get_mut(index) {
            Some(solve) if solve.tags() != tags => {
                solve.set_tags(tags);
                true
            }
            _ => false,
        }
    }

    /// Applies a change to a tag on every solve that has it.
    pub fn change_tags(&mut self, change: &TagChange) {
        for solve in &mut self.times {
            solve.change_tags(change);
        }
    }

    /// Toggles a penalty on the solve at `index`, clearing it if already set.
    pub fn toggle_modifier(&mut self, index: usize, modifier: Modifier) -> bool {
        let Some(solve) = self.times.get(index) else {
            return false;
        };
        let mut after = solve.snapshot();
        after.modifier = if after.modifier == modifier {
            Modifier::None
        } else {
            modifier
        };
        self.edit(index, after)
    }

    /// Removes and returns the solve at `index`.
    pub fn remove(&mut self, index: usize) -> Option<Solve> {
        (index < self.times.len()).then(|| {
            let solve = self.times.remove(index);
            self.remark_single_records();
            solve
        })
    }

    /// Puts a removed solve back at `index` (clamped to the end).
    pub fn insert(&mut self, index: usize, solve: Solve) {
        self.times.insert(index.min(self.times.len()), solve);
        self.remark_single_records();
    }

    pub(crate) fn solves_mut(&mut self) -> &mut [Solve] {
        &mut self.times
    }

    /// Adds a solve after everything recorded no later than it, leaving the
    /// order of the existing solves alone.
    pub(crate) fn insert_chronologically(&mut self, solve: Solve) {
        let index = self
            .times
            .iter()
            .rposition(|other| other.solved_at_unix_ms() <= solve.solved_at_unix_ms())
            .map_or(0, |index| index + 1);
        self.times.insert(index, solve);
    }

    /// Recomputes every record marker after a change that can reorder results.
    pub(crate) fn remark_single_records(&mut self) {
        let mut best = None;
        for solve in &mut self.times {
            let ms = solve.effective_ms();
            solve.was_single_record = ms.is_some_and(|ms| best.is_none_or(|best| ms < best));
            if solve.was_single_record {
                best = ms;
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::event::WcaEvent;

    fn solve(ms: u64) -> Solve {
        Solve::new(ms, WcaEvent::Cube3x3, String::new(), Modifier::None)
    }

    #[test]
    fn insert_restores_a_removed_solve() {
        let mut history = History::new();
        for ms in [3000, 2000, 1000] {
            history.push(solve(ms));
        }
        let before = records(&history);
        let removed = history.remove(1).unwrap();
        history.insert(1, removed);
        assert_eq!(history.len(), 3);
        assert_eq!(history.solves()[1].effective_ms(), Some(2000));
        assert_eq!(records(&history), before);
        history.insert(99, solve(500));
        assert_eq!(history.solves()[3].effective_ms(), Some(500));
    }

    fn records(history: &History) -> Vec<bool> {
        history
            .solves()
            .iter()
            .map(Solve::was_single_record)
            .collect()
    }

    #[test]
    fn only_strict_improvements_are_records() {
        let mut history = History::new();
        for ms in [10_000, 10_000, 9_000, 9_500] {
            history.push(solve(ms));
        }
        assert_eq!(records(&history), [true, false, true, false]);
    }

    #[test]
    fn penalties_and_deletions_recompute_records() {
        let mut history = History::new();
        for ms in [10_000, 9_000, 9_500] {
            history.push(solve(ms));
        }
        assert!(history.toggle_modifier(1, Modifier::DNF));
        assert_eq!(records(&history), [true, false, true]);
        assert!(history.toggle_modifier(1, Modifier::DNF));
        assert_eq!(history.solves()[1].modifier(), Modifier::None);
        assert_eq!(records(&history), [true, true, false]);
        assert!(history.remove(1).is_some());
        assert_eq!(records(&history), [true, true]);
        assert!(history.remove(7).is_none());
    }
}
