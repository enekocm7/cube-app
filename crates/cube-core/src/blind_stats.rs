//! Statistics for the blindfolded events.
//!
//! A trimmed average of 5 says little when many attempts are DNF, so these
//! events are summarized by how often they succeed, the best single, the mean
//! of 3 and how the time splits between memo and execution.

use crate::event::WcaEvent;
use crate::solve::Solve;
use crate::stats::{self, Average};

/// The attempts a mean covers.
const MEAN_SIZE: usize = 3;

/// The summary of one blindfolded event within a session.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct BlindStats {
    /// Every attempt, DNF or not.
    pub attempts: usize,
    /// The attempts that were not DNF.
    pub successes: usize,
    /// The fastest success and its index in the session.
    pub best: Option<(u64, usize)>,
    /// The latest mean of 3, if the session has three attempts of the event.
    pub current_mean: Option<Average>,
    /// The fastest mean of 3 and the session index of its last attempt. It is
    /// only a DNF when every mean is.
    pub best_mean: Option<(Average, usize)>,
    /// The mean memo of the successes timed in two parts.
    pub memo_mean: Option<u64>,
    /// The shortest memo among them and its index in the session.
    pub memo_best: Option<(u64, usize)>,
    /// The mean execution of the same successes.
    pub execution_mean: Option<u64>,
}

impl BlindStats {
    /// Summarizes the attempts of `event` among `solves`, a session in order.
    pub fn compute(solves: &[Solve], event: WcaEvent) -> Self {
        let positions: Vec<usize> = solves
            .iter()
            .enumerate()
            .filter(|(_, solve)| solve.event() == event)
            .map(|(index, _)| index)
            .collect();
        let times: Vec<Option<u64>> = positions
            .iter()
            .map(|&index| solves[index].effective_ms())
            .collect();

        let mut stats = Self {
            attempts: positions.len(),
            ..Self::default()
        };
        let mut memos = Vec::new();
        let mut executions = Vec::new();
        for &index in &positions {
            let solve = &solves[index];
            let Some(ms) = solve.effective_ms() else {
                continue;
            };
            stats.successes += 1;
            if stats.best.is_none_or(|(best, _)| ms < best) {
                stats.best = Some((ms, index));
            }
            if let (Some(memo), Some(execution)) = (solve.memo_ms(), solve.execution_ms()) {
                memos.push(memo);
                executions.push(execution);
                if stats.memo_best.is_none_or(|(best, _)| memo < best) {
                    stats.memo_best = Some((memo, index));
                }
            }
        }
        stats.memo_mean = mean(&memos);
        stats.execution_mean = mean(&executions);

        let means = times
            .windows(MEAN_SIZE)
            .enumerate()
            .filter_map(|(start, window)| {
                let average = stats::average_of(window.iter().copied())?;
                Some((start + MEAN_SIZE - 1, average))
            });
        for (end, average) in means {
            if end + 1 == times.len() {
                stats.current_mean = Some(average);
            }
            if stats.best_mean.is_none_or(|(best, _)| average < best) {
                stats.best_mean = Some((average, positions[end]));
            }
        }
        stats
    }

    /// Returns the share of attempts that succeeded as a whole percentage, or
    /// `None` before the first attempt.
    pub fn success_percent(&self) -> Option<u32> {
        (self.attempts > 0).then(|| {
            // Round half up in integers, so 1 of 8 reads 13 and not 12.
            ((self.successes * 200 + self.attempts) / (self.attempts * 2)) as u32
        })
    }
}

/// The mean of `values`, rounded down, or `None` when there are none.
fn mean(values: &[u64]) -> Option<u64> {
    let count = u128::try_from(values.len())
        .ok()
        .filter(|&count| count > 0)?;
    let sum: u128 = values.iter().map(|&value| u128::from(value)).sum();
    Some((sum / count) as u64)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::solve::Modifier;

    fn attempt(event: WcaEvent, ms: u64, modifier: Modifier, memo: Option<u64>) -> Solve {
        let mut solve =
            Solve::new_with_meta(ms, event, format!("R{ms}"), ms, modifier, String::new());
        solve.set_memo_ms(memo);
        solve
    }

    const BLD: WcaEvent = WcaEvent::Cube3x3Blind;

    fn ok(ms: u64, memo: u64) -> Solve {
        attempt(BLD, ms, Modifier::None, Some(memo))
    }

    fn dnf(ms: u64) -> Solve {
        attempt(BLD, ms, Modifier::DNF, Some(ms / 2))
    }

    #[test]
    fn an_empty_session_has_nothing_to_show() {
        let stats = BlindStats::compute(&[], BLD);
        assert_eq!(stats, BlindStats::default());
        assert_eq!(stats.success_percent(), None);
    }

    #[test]
    fn the_success_rate_counts_every_dnf_as_a_miss() {
        let solves = [ok(60_000, 30_000), dnf(50_000), ok(70_000, 40_000), dnf(1)];
        let stats = BlindStats::compute(&solves, BLD);
        assert_eq!((stats.attempts, stats.successes), (4, 2));
        assert_eq!(stats.success_percent(), Some(50));
    }

    #[test]
    fn the_percentage_rounds_half_up() {
        let mut solves = vec![ok(1_000, 500)];
        solves.extend((0..7).map(|_| dnf(1_000)));
        assert_eq!(
            BlindStats::compute(&solves, BLD).success_percent(),
            Some(13)
        );
        let thirds = [ok(1_000, 500), dnf(1), dnf(1)];
        assert_eq!(
            BlindStats::compute(&thirds, BLD).success_percent(),
            Some(33)
        );
    }

    #[test]
    fn the_best_single_ignores_dnfs_and_counts_penalties() {
        let solves = [
            dnf(10_000),
            ok(90_000, 40_000),
            attempt(BLD, 59_000, Modifier::PlusTwo, Some(30_000)),
            ok(80_000, 40_000),
        ];
        let stats = BlindStats::compute(&solves, BLD);
        assert_eq!(stats.best, Some((61_000, 2)));
    }

    #[test]
    fn the_mean_of_three_is_a_dnf_when_any_attempt_is() {
        let solves = [ok(60_000, 30_000), ok(90_000, 40_000), dnf(1)];
        let stats = BlindStats::compute(&solves, BLD);
        assert_eq!(stats.current_mean, Some(Average::Dnf));
        assert_eq!(stats.best_mean, Some((Average::Dnf, 2)));

        let solves = [
            dnf(1),
            ok(60_000, 30_000),
            ok(90_000, 40_000),
            ok(30_000, 20_000),
        ];
        let stats = BlindStats::compute(&solves, BLD);
        assert_eq!(stats.current_mean, Some(Average::Time(60_000)));
        assert_eq!(stats.best_mean, Some((Average::Time(60_000), 3)));
    }

    #[test]
    fn fewer_than_three_attempts_have_no_mean() {
        let stats = BlindStats::compute(&[ok(60_000, 30_000), ok(70_000, 30_000)], BLD);
        assert_eq!(stats.current_mean, None);
        assert_eq!(stats.best_mean, None);
    }

    #[test]
    fn memo_and_execution_come_from_the_successes_timed_in_two_parts() {
        let solves = [
            ok(100_000, 60_000),
            dnf(80_000),
            attempt(BLD, 50_000, Modifier::None, None),
            ok(80_000, 20_000),
        ];
        let stats = BlindStats::compute(&solves, BLD);
        assert_eq!(stats.memo_mean, Some(40_000));
        assert_eq!(stats.memo_best, Some((20_000, 3)));
        assert_eq!(stats.execution_mean, Some(50_000));
    }

    #[test]
    fn other_events_in_the_session_are_left_out() {
        let solves = [
            attempt(WcaEvent::Cube3x3, 9_000, Modifier::None, None),
            ok(60_000, 30_000),
            attempt(
                WcaEvent::Cube4x4Blind,
                200_000,
                Modifier::None,
                Some(100_000),
            ),
        ];
        let stats = BlindStats::compute(&solves, BLD);
        assert_eq!(stats.attempts, 1);
        assert_eq!(stats.best, Some((60_000, 1)));
        assert_eq!(
            BlindStats::compute(&solves, WcaEvent::Cube5x5Blind).attempts,
            0
        );
    }
}
