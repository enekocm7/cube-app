//! Multi-Blind: several cubes memorized and solved in one attempt.
//!
//! An attempt has one scramble per cube and one time, and its result is points
//! rather than a time, so it does not fit a [`Solve`](crate::solve::Solve).
//! Attempts live in `multi-blind.json`, never in `times.json`, which is
//! cube-tui's file and would not know what to do with them.
//!
//! The scoring follows the WCA regulations (9f): points are the cubes solved
//! minus the cubes unsolved, the time limit is 10 minutes per cube attempted
//! up to an hour, and an attempt that scores zero or less, or that runs out of
//! time, is a DNF.

use std::cmp::Ordering;
use std::fmt::Write as _;

use serde::{Deserialize, Serialize};

use crate::event::WcaEvent;
use crate::scramble;

/// The fewest cubes an attempt may have.
pub const MIN_CUBES: u32 = 2;

/// The most cubes an attempt may have. The WCA sets no limit; this keeps the
/// scrambles quick to prepare and the list readable.
pub const MAX_CUBES: u32 = 100;

/// The time allowed for each cube attempted.
const LIMIT_PER_CUBE_MS: u64 = 10 * 60 * 1_000;

/// The longest time limit, however many cubes are attempted.
const LONGEST_LIMIT_MS: u64 = 60 * 60 * 1_000;

/// The event whose scrambles a Multi-Blind attempt uses.
pub const EVENT: WcaEvent = WcaEvent::Cube3x3Blind;

/// Returns the time limit of an attempt at `attempted` cubes, in milliseconds.
pub const fn time_limit_ms(attempted: u32) -> u64 {
    let limit = LIMIT_PER_CUBE_MS * attempted as u64;
    if limit > LONGEST_LIMIT_MS {
        LONGEST_LIMIT_MS
    } else {
        limit
    }
}

/// Why an attempt is not scored.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum DnfReason {
    /// More cubes were unsolved than solved, or as many.
    NoPoints,
    /// The attempt took longer than its time limit.
    OverTime,
}

/// What an attempt came to.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum MultiBlindResult {
    /// Points were scored, `solved - (attempted - solved)`, within the time.
    Scored {
        points: u32,
        time_ms: u64,
    },
    Dnf(DnfReason),
}

impl MultiBlindResult {
    /// Returns whether the attempt was scored.
    pub const fn is_scored(self) -> bool {
        matches!(self, Self::Scored { .. })
    }
}

/// Orders results from best to worst: more points first, then less time, and
/// any scored result before a DNF.
pub fn compare(first: MultiBlindResult, second: MultiBlindResult) -> Ordering {
    match (first, second) {
        (
            MultiBlindResult::Scored {
                points: first_points,
                time_ms: first_time,
            },
            MultiBlindResult::Scored {
                points: second_points,
                time_ms: second_time,
            },
        ) => second_points
            .cmp(&first_points)
            .then(first_time.cmp(&second_time)),
        (MultiBlindResult::Scored { .. }, MultiBlindResult::Dnf(_)) => Ordering::Less,
        (MultiBlindResult::Dnf(_), MultiBlindResult::Scored { .. }) => Ordering::Greater,
        (MultiBlindResult::Dnf(_), MultiBlindResult::Dnf(_)) => Ordering::Equal,
    }
}

/// Scores an attempt.
///
/// `solved` cannot exceed `attempted`; the caller checks with [`validate`].
pub fn score(attempted: u32, solved: u32, time_ms: u64) -> MultiBlindResult {
    let unsolved = attempted.saturating_sub(solved);
    let Some(points) = solved.checked_sub(unsolved).filter(|&points| points > 0) else {
        return MultiBlindResult::Dnf(DnfReason::NoPoints);
    };
    if time_ms > time_limit_ms(attempted) {
        return MultiBlindResult::Dnf(DnfReason::OverTime);
    }
    MultiBlindResult::Scored { points, time_ms }
}

/// Why the numbers of an attempt cannot be right.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Invalid {
    /// Fewer than [`MIN_CUBES`] or more than [`MAX_CUBES`] cubes.
    Cubes,
    /// More cubes solved than attempted.
    Solved,
}

/// Checks the number of cubes attempted and solved.
pub const fn validate(attempted: u32, solved: u32) -> Result<(), Invalid> {
    if attempted < MIN_CUBES || attempted > MAX_CUBES {
        Err(Invalid::Cubes)
    } else if solved > attempted {
        Err(Invalid::Solved)
    } else {
        Ok(())
    }
}

/// Reads the number of cubes solved as typed, or `None` when it is not a
/// whole number from zero to `attempted`.
pub fn parse_solved(text: &str, attempted: u32) -> Option<u32> {
    let text = text.trim();
    if text.is_empty() || !text.bytes().all(|byte| byte.is_ascii_digit()) {
        return None;
    }
    text.parse().ok().filter(|&solved| solved <= attempted)
}

/// One attempt at several cubes.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct MultiBlindAttempt {
    pub attempted: u32,
    pub solved: u32,
    /// The whole attempt, memo included, in milliseconds.
    pub time_ms: u64,
    #[serde(default)]
    pub solved_at_unix_ms: u64,
    /// The scrambles of the cubes, in order.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub scrambles: Vec<String>,
}

impl MultiBlindAttempt {
    /// Returns what the attempt came to.
    pub fn result(&self) -> MultiBlindResult {
        score(self.attempted, self.solved, self.time_ms)
    }
}

/// The contents of `multi-blind.json`.
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct MultiBlindData {
    /// Every attempt, oldest first.
    pub attempts: Vec<MultiBlindAttempt>,
}

impl MultiBlindData {
    /// Checks that every attempt has numbers the app could have recorded, so a
    /// file it could not save back unchanged is refused rather than rewritten.
    pub fn validate(&self) -> Result<(), usize> {
        match self
            .attempts
            .iter()
            .position(|attempt| validate(attempt.attempted, attempt.solved).is_err())
        {
            Some(index) => Err(index),
            None => Ok(()),
        }
    }

    /// Summarizes every attempt.
    pub fn summary(&self) -> MultiBlindSummary {
        let mut summary = MultiBlindSummary {
            attempts: self.attempts.len(),
            ..MultiBlindSummary::default()
        };
        for (index, attempt) in self.attempts.iter().enumerate() {
            summary.cubes_attempted += u64::from(attempt.attempted);
            summary.cubes_solved += u64::from(attempt.solved);
            let result = attempt.result();
            if result.is_scored() {
                summary.scored += 1;
                let better = summary
                    .best
                    .is_none_or(|best| compare(result, self.attempts[best].result()).is_lt());
                if better {
                    summary.best = Some(index);
                }
            }
        }
        summary
    }
}

/// The figures of a [`MultiBlindData`].
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct MultiBlindSummary {
    pub attempts: usize,
    /// The attempts that were not DNF.
    pub scored: usize,
    pub cubes_attempted: u64,
    pub cubes_solved: u64,
    /// The index of the best attempt, if any was scored.
    pub best: Option<usize>,
}

impl MultiBlindSummary {
    /// Returns the share of attempts that scored, as a whole percentage.
    pub fn success_percent(&self) -> Option<u32> {
        percent(self.scored as u64, self.attempts as u64)
    }

    /// Returns the share of cubes that were solved, as a whole percentage.
    pub fn cube_percent(&self) -> Option<u32> {
        percent(self.cubes_solved, self.cubes_attempted)
    }
}

/// `part` of `whole` as a percentage rounded half up; `None` when `whole` is
/// zero.
fn percent(part: u64, whole: u64) -> Option<u32> {
    (whole > 0).then(|| ((part * 200 + whole) / (whole * 2)) as u32)
}

/// Prepares `count` scrambles for an attempt, which takes a while.
pub fn generate_scrambles(count: u32) -> Vec<String> {
    (0..count).map(|_| scramble::generate(EVENT)).collect()
}

/// Writes the scrambles as a numbered list, one cube to a line, ready to copy
/// or print.
pub fn scrambles_text(scrambles: &[String]) -> String {
    let width = scrambles.len().to_string().len();
    let mut text = String::new();
    for (index, scramble) in scrambles.iter().enumerate() {
        let _ = writeln!(text, "{:>width$}. {}", index + 1, scramble.trim());
    }
    text
}

#[cfg(test)]
mod tests {
    use super::*;

    const MINUTE: u64 = 60_000;

    #[test]
    fn points_are_the_cubes_solved_minus_the_cubes_unsolved() {
        assert_eq!(
            score(3, 3, 20 * MINUTE),
            MultiBlindResult::Scored {
                points: 3,
                time_ms: 20 * MINUTE
            }
        );
        assert_eq!(
            score(4, 3, 20 * MINUTE),
            MultiBlindResult::Scored {
                points: 2,
                time_ms: 20 * MINUTE
            }
        );
        assert_eq!(
            score(10, 8, MINUTE),
            MultiBlindResult::Scored {
                points: 6,
                time_ms: MINUTE
            }
        );
        // Two of three is the lowest score that counts as a result.
        assert_eq!(
            score(3, 2, MINUTE),
            MultiBlindResult::Scored {
                points: 1,
                time_ms: MINUTE
            }
        );
    }

    #[test]
    fn zero_or_fewer_points_is_a_dnf() {
        // Zero points: as many cubes unsolved as solved.
        assert_eq!(
            score(2, 1, MINUTE),
            MultiBlindResult::Dnf(DnfReason::NoPoints)
        );
        assert_eq!(
            score(6, 3, MINUTE),
            MultiBlindResult::Dnf(DnfReason::NoPoints)
        );
        // Fewer than zero.
        assert_eq!(
            score(3, 1, MINUTE),
            MultiBlindResult::Dnf(DnfReason::NoPoints)
        );
        assert_eq!(
            score(5, 0, MINUTE),
            MultiBlindResult::Dnf(DnfReason::NoPoints)
        );
        assert_eq!(
            score(2, 0, MINUTE),
            MultiBlindResult::Dnf(DnfReason::NoPoints)
        );
    }

    #[test]
    fn the_time_limit_is_ten_minutes_a_cube_up_to_an_hour() {
        assert_eq!(time_limit_ms(2), 20 * MINUTE);
        assert_eq!(time_limit_ms(5), 50 * MINUTE);
        assert_eq!(time_limit_ms(6), 60 * MINUTE);
        assert_eq!(time_limit_ms(40), 60 * MINUTE);
        assert_eq!(time_limit_ms(MAX_CUBES), 60 * MINUTE);
    }

    #[test]
    fn going_over_the_limit_is_a_dnf_but_the_limit_itself_is_not() {
        let limit = time_limit_ms(4);
        assert!(score(4, 4, limit).is_scored());
        assert_eq!(
            score(4, 4, limit + 1),
            MultiBlindResult::Dnf(DnfReason::OverTime)
        );
        assert!(score(10, 10, 60 * MINUTE).is_scored());
        assert_eq!(
            score(10, 10, 60 * MINUTE + 1),
            MultiBlindResult::Dnf(DnfReason::OverTime)
        );
    }

    #[test]
    fn no_points_outranks_running_out_of_time() {
        assert_eq!(
            score(2, 0, 90 * MINUTE),
            MultiBlindResult::Dnf(DnfReason::NoPoints)
        );
    }

    #[test]
    fn more_points_win_and_less_time_breaks_a_tie() {
        let scored = |points, time_ms| MultiBlindResult::Scored { points, time_ms };
        let dnf = MultiBlindResult::Dnf(DnfReason::OverTime);
        assert_eq!(compare(scored(5, 50), scored(4, 10)), Ordering::Less);
        assert_eq!(compare(scored(4, 10), scored(4, 20)), Ordering::Less);
        assert_eq!(compare(scored(4, 20), scored(4, 20)), Ordering::Equal);
        assert_eq!(compare(scored(1, 99), dnf), Ordering::Less);
        assert_eq!(compare(dnf, scored(1, 99)), Ordering::Greater);
        assert_eq!(
            compare(dnf, MultiBlindResult::Dnf(DnfReason::NoPoints)),
            Ordering::Equal
        );
    }

    #[test]
    fn the_numbers_of_an_attempt_are_checked() {
        assert_eq!(validate(MIN_CUBES, 0), Ok(()));
        assert_eq!(validate(MAX_CUBES, MAX_CUBES), Ok(()));
        assert_eq!(validate(1, 0), Err(Invalid::Cubes));
        assert_eq!(validate(0, 0), Err(Invalid::Cubes));
        assert_eq!(validate(MAX_CUBES + 1, 0), Err(Invalid::Cubes));
        assert_eq!(validate(3, 4), Err(Invalid::Solved));
    }

    #[test]
    fn the_solved_count_must_be_a_whole_number_within_the_attempt() {
        assert_eq!(parse_solved("0", 5), Some(0));
        assert_eq!(parse_solved(" 5 ", 5), Some(5));
        assert_eq!(parse_solved("6", 5), None);
        assert_eq!(parse_solved("", 5), None);
        assert_eq!(parse_solved("-1", 5), None);
        assert_eq!(parse_solved("2.5", 5), None);
        assert_eq!(parse_solved("99999999999999999999", 5), None);
    }

    fn attempt(attempted: u32, solved: u32, time_ms: u64) -> MultiBlindAttempt {
        MultiBlindAttempt {
            attempted,
            solved,
            time_ms,
            solved_at_unix_ms: 0,
            scrambles: Vec::new(),
        }
    }

    #[test]
    fn the_summary_finds_the_best_attempt_and_the_success_rates() {
        let data = MultiBlindData {
            attempts: vec![
                attempt(4, 2, 30 * MINUTE), // zero points
                attempt(4, 3, 30 * MINUTE), // 2 points
                attempt(3, 3, 25 * MINUTE), // 3 points
                attempt(3, 3, 20 * MINUTE), // 3 points, faster
                attempt(2, 2, 25 * MINUTE), // over the limit
            ],
        };
        let summary = data.summary();
        assert_eq!(summary.attempts, 5);
        assert_eq!(summary.scored, 3);
        assert_eq!(summary.best, Some(3));
        assert_eq!(summary.success_percent(), Some(60));
        assert_eq!((summary.cubes_attempted, summary.cubes_solved), (16, 13));
        assert_eq!(summary.cube_percent(), Some(81));
    }

    #[test]
    fn a_history_without_a_scored_attempt_has_no_best() {
        let data = MultiBlindData {
            attempts: vec![attempt(2, 0, MINUTE)],
        };
        let summary = data.summary();
        assert_eq!(summary.best, None);
        assert_eq!(summary.success_percent(), Some(0));
        assert_eq!(MultiBlindData::default().summary().success_percent(), None);
    }

    #[test]
    fn scrambles_are_listed_one_cube_to_a_line() {
        let scrambles: Vec<String> = (1..=10).map(|n| format!("R{n} U ")).collect();
        let text = scrambles_text(&scrambles);
        let lines: Vec<&str> = text.lines().collect();
        assert_eq!(lines.len(), 10);
        assert_eq!(lines[0], " 1. R1 U");
        assert_eq!(lines[9], "10. R10 U");
        assert_eq!(scrambles_text(&[]), "");
    }

    #[test]
    fn data_round_trips_and_older_files_load() {
        let mut data = MultiBlindData::default();
        data.attempts.push(MultiBlindAttempt {
            scrambles: vec!["R U Rw".to_owned(), "F D".to_owned()],
            solved_at_unix_ms: 1_760_000_000_000,
            ..attempt(2, 2, 15 * MINUTE)
        });
        let text = serde_json::to_string(&data).unwrap();
        assert_eq!(serde_json::from_str::<MultiBlindData>(&text).unwrap(), data);

        let loaded: MultiBlindData =
            serde_json::from_str(r#"{"attempts":[{"attempted":3,"solved":2,"time_ms":5}]}"#)
                .unwrap();
        assert_eq!(loaded.attempts[0].scrambles, Vec::<String>::new());
        assert_eq!(loaded.attempts[0].solved_at_unix_ms, 0);
        assert_eq!(
            serde_json::from_str::<MultiBlindData>("{}").unwrap(),
            MultiBlindData::default()
        );
    }

    #[test]
    fn impossible_numbers_in_a_file_are_found() {
        let data = MultiBlindData {
            attempts: vec![attempt(3, 3, 1), attempt(3, 4, 1)],
        };
        assert_eq!(data.validate(), Err(1));
        assert_eq!(MultiBlindData::default().validate(), Ok(()));
    }
}
