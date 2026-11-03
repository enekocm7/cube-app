//! A session: a named, chronological list of solves.

use serde::{Deserialize, Serialize};

use crate::solve::{Modifier, Solve, SolveSnapshot};

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

    /// Recomputes every record marker after a change that can reorder results.
    fn remark_single_records(&mut self) {
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
