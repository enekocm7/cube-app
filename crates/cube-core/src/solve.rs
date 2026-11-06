//! A single timed attempt and its edit history.

use std::fmt;
use std::time::{SystemTime, UNIX_EPOCH};

use serde::{Deserialize, Serialize};

use crate::event::WcaEvent;
use crate::format::format_ms;
use crate::tags::TagChange;

/// Time added by a `+2` penalty.
const PLUS_TWO_MS: u64 = 2_000;

/// A solve's penalty. Variant names are part of the save format.
#[allow(clippy::upper_case_acronyms)]
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub enum Modifier {
    #[default]
    None,
    PlusTwo,
    DNF,
}

/// One attempt. Field names are part of the save format shared with cube-tui.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Solve {
    #[serde(rename = "timestamp_in_millis")]
    time_ms: u64,
    event: WcaEvent,
    scramble: String,
    #[serde(default)]
    solved_at_unix_ms: u64,
    #[serde(default)]
    modifier: Modifier,
    #[serde(default, skip_serializing_if = "String::is_empty")]
    comment: String,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    changes: Vec<SolveChange>,
    /// Whether this single improved the session best when it was set.
    #[serde(default)]
    pub(crate) was_single_record: bool,
    /// Kept in `solve-extras.json`: cube-tui would drop a field it does not
    /// know from `times.json`.
    #[serde(skip)]
    tags: Vec<String>,
}

/// The editable fields of a solve, without its audit trail.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct SolveSnapshot {
    pub time_ms: u64,
    pub event: WcaEvent,
    pub scramble: String,
    pub solved_at_unix_ms: u64,
    pub modifier: Modifier,
    pub comment: String,
}

/// One entry of a solve's audit trail.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct SolveChange {
    pub changed_at_unix_ms: u64,
    pub before: SolveSnapshot,
    pub after: SolveSnapshot,
}

impl Solve {
    /// Creates a solve that finished just now.
    pub fn new(time_ms: u64, event: WcaEvent, scramble: String, modifier: Modifier) -> Self {
        Self::new_with_meta(
            time_ms,
            event,
            scramble,
            current_unix_ms(),
            modifier,
            String::new(),
        )
    }

    /// Creates a solve from stored data, such as one imported from csTimer.
    ///
    /// Imported solves have no audit trail: there is no earlier version to keep.
    pub fn new_with_meta(
        time_ms: u64,
        event: WcaEvent,
        scramble: String,
        solved_at_unix_ms: u64,
        modifier: Modifier,
        comment: String,
    ) -> Self {
        Self {
            time_ms,
            event,
            scramble,
            solved_at_unix_ms,
            modifier,
            comment,
            changes: Vec::new(),
            was_single_record: false,
            tags: Vec::new(),
        }
    }

    /// Returns the measured time, before any penalty.
    pub const fn raw_ms(&self) -> u64 {
        self.time_ms
    }

    /// Returns the penalty-adjusted time, or `None` for a DNF.
    pub const fn effective_ms(&self) -> Option<u64> {
        match self.modifier {
            Modifier::None => Some(self.time_ms),
            Modifier::PlusTwo => Some(self.time_ms + PLUS_TWO_MS),
            Modifier::DNF => None,
        }
    }

    pub const fn event(&self) -> WcaEvent {
        self.event
    }

    pub fn scramble(&self) -> &str {
        &self.scramble
    }

    /// Returns when the solve finished as Unix milliseconds; `0` when unknown.
    pub const fn solved_at_unix_ms(&self) -> u64 {
        self.solved_at_unix_ms
    }

    pub const fn modifier(&self) -> Modifier {
        self.modifier
    }

    pub fn comment(&self) -> &str {
        &self.comment
    }

    pub fn changes(&self) -> &[SolveChange] {
        &self.changes
    }

    pub const fn was_single_record(&self) -> bool {
        self.was_single_record
    }

    pub fn tags(&self) -> &[String] {
        &self.tags
    }

    pub fn set_tags(&mut self, tags: Vec<String>) {
        self.tags = tags;
    }

    /// Applies a change to a tag, if the solve has it.
    pub fn change_tags(&mut self, change: &TagChange) {
        change.apply(&mut self.tags);
    }

    /// Returns the solve's current editable fields.
    pub fn snapshot(&self) -> SolveSnapshot {
        SolveSnapshot {
            time_ms: self.time_ms,
            event: self.event,
            scramble: self.scramble.clone(),
            solved_at_unix_ms: self.solved_at_unix_ms,
            modifier: self.modifier,
            comment: self.comment.clone(),
        }
    }

    /// Returns the event, scramble, date and time the solve was first recorded
    /// with, which later edits do not change.
    pub(crate) fn origin(&self) -> (WcaEvent, &str, u64, u64) {
        match self.changes.first() {
            Some(change) => {
                let first = &change.before;
                (
                    first.event,
                    &first.scramble,
                    first.solved_at_unix_ms,
                    first.time_ms,
                )
            }
            None => (
                self.event,
                &self.scramble,
                self.solved_at_unix_ms,
                self.time_ms,
            ),
        }
    }

    /// Applies an edit and records it in the audit trail.
    ///
    /// Returns `false`, recording nothing, when `after` changes nothing.
    pub(crate) fn edit(&mut self, after: SolveSnapshot) -> bool {
        let before = self.snapshot();
        if before == after {
            return false;
        }
        self.time_ms = after.time_ms;
        self.event = after.event;
        self.scramble.clone_from(&after.scramble);
        self.solved_at_unix_ms = after.solved_at_unix_ms;
        self.modifier = after.modifier;
        self.comment.clone_from(&after.comment);
        self.changes.push(SolveChange {
            changed_at_unix_ms: current_unix_ms(),
            before,
            after,
        });
        true
    }
}

impl fmt::Display for Solve {
    /// Writes the effective time, marking `+2` solves and spelling out DNFs.
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self.effective_ms() {
            Some(ms) if self.modifier == Modifier::PlusTwo => write!(f, "{}+", format_ms(ms)),
            Some(ms) => f.write_str(&format_ms(ms)),
            None => f.write_str("DNF"),
        }
    }
}

fn current_unix_ms() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_or(0, |elapsed| elapsed.as_millis() as u64)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn solve(modifier: Modifier) -> Solve {
        Solve::new(10_000, WcaEvent::Cube3x3, "R U".into(), modifier)
    }

    #[test]
    fn penalties_change_the_effective_time_and_display() {
        assert_eq!(solve(Modifier::None).to_string(), "10.000");
        assert_eq!(solve(Modifier::PlusTwo).effective_ms(), Some(12_000));
        assert_eq!(solve(Modifier::PlusTwo).to_string(), "12.000+");
        assert_eq!(solve(Modifier::DNF).effective_ms(), None);
        assert_eq!(solve(Modifier::DNF).to_string(), "DNF");
    }

    #[test]
    fn edits_keep_both_versions_and_ignore_no_ops() {
        let mut solve = solve(Modifier::None);
        assert!(!solve.edit(solve.snapshot()));
        assert!(solve.changes().is_empty());

        let mut after = solve.snapshot();
        after.comment = "lucky".into();
        assert!(solve.edit(after));
        assert_eq!(solve.comment(), "lucky");
        assert_eq!(solve.changes()[0].before.comment, "");
        assert_eq!(solve.changes()[0].after.comment, "lucky");
    }
}
