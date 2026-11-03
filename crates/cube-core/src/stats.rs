//! Session statistics: WCA averages, their rolling values and session summaries.

use crate::solve::Solve;

/// The rolling windows shown for every session.
pub const AVERAGE_SIZES: [usize; 5] = [3, 5, 12, 50, 100];

/// The outcome of a mean or average. Any time orders before a DNF.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub enum Average {
    Time(u64),
    Dnf,
}

impl Average {
    /// Returns the time in milliseconds, or `None` for a DNF average.
    pub const fn ms(self) -> Option<u64> {
        match self {
            Self::Time(ms) => Some(ms),
            Self::Dnf => None,
        }
    }
}

/// Calculates a mean of three, or a WCA trimmed average for any longer window.
///
/// A mean is DNF when any attempt is. An average drops its best and worst
/// attempts, a single DNF counting as the worst, and is DNF with two or more.
/// Returns `None` for windows too short to average.
pub fn average(attempts: &[Solve]) -> Option<Average> {
    let n = attempts.len();
    if n < 3 {
        return None;
    }
    let mut sum = 0_u128;
    let mut best = u64::MAX;
    let mut worst = 0;
    let mut dnfs = 0;
    for solve in attempts {
        match solve.effective_ms() {
            Some(ms) => {
                sum += u128::from(ms);
                best = best.min(ms);
                worst = worst.max(ms);
            }
            None => dnfs += 1,
        }
    }

    if n == 3 {
        return Some(if dnfs > 0 {
            Average::Dnf
        } else {
            Average::Time((sum / 3) as u64)
        });
    }
    if dnfs >= 2 {
        return Some(Average::Dnf);
    }
    sum -= u128::from(best);
    if dnfs == 0 {
        sum -= u128::from(worst);
    }
    Some(Average::Time((sum / (n as u128 - 2)) as u64))
}

/// Returns the average of the `size` solves ending at `end`, inclusive.
pub fn average_at(solves: &[Solve], end: usize, size: usize) -> Option<Average> {
    let start = (end + 1).checked_sub(size)?;
    average(solves.get(start..=end)?)
}

/// Returns every complete rolling average of `size`, keyed by its last solve.
pub fn rolling(solves: &[Solve], size: usize) -> Vec<(usize, Average)> {
    if size == 0 {
        return Vec::new();
    }
    solves
        .windows(size)
        .enumerate()
        .filter_map(|(start, window)| Some((start + size - 1, average(window)?)))
        .collect()
}

/// Splits the non-DNF solves into `buckets` equal-width ranges between the
/// extremes, returning each range's start and how many solves fall in it.
pub fn histogram(solves: &[Solve], buckets: usize) -> Vec<(u64, u32)> {
    let times: Vec<u64> = solves.iter().filter_map(Solve::effective_ms).collect();
    let (Some(&min), Some(&max)) = (times.iter().min(), times.iter().max()) else {
        return Vec::new();
    };
    if buckets == 0 {
        return Vec::new();
    }
    let width = (max - min) / buckets as u64 + 1;
    let mut counts = vec![0_u32; buckets];
    for ms in times {
        counts[((ms - min) / width) as usize] += 1;
    }
    counts
        .into_iter()
        .enumerate()
        .map(|(index, count)| (min + index as u64 * width, count))
        .collect()
}

/// The current and best values for one rolling window.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct WindowStats {
    pub size: usize,
    /// The latest complete average, if the session is long enough.
    pub current: Option<Average>,
    /// The fastest average and the index of its last solve. It is only a DNF
    /// when every window is.
    pub best: Option<(Average, usize)>,
}

/// A summary of a whole session.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct SessionStats {
    pub count: usize,
    pub dnf_count: usize,
    /// The fastest single and its index.
    pub best: Option<(u64, usize)>,
    /// The slowest non-DNF single and its index.
    pub worst: Option<(u64, usize)>,
    /// The mean of every non-DNF solve.
    pub mean: Option<u64>,
    /// The population standard deviation of every non-DNF solve.
    pub std_dev: Option<u64>,
    pub windows: Vec<WindowStats>,
}

impl SessionStats {
    /// Summarizes `solves` with every window in [`AVERAGE_SIZES`].
    pub fn compute(solves: &[Solve]) -> Self {
        let times: Vec<(usize, u64)> = solves
            .iter()
            .enumerate()
            .filter_map(|(index, solve)| Some((index, solve.effective_ms()?)))
            .collect();
        let mean = (!times.is_empty()).then(|| {
            let sum: u128 = times.iter().map(|&(_, ms)| u128::from(ms)).sum();
            (sum / times.len() as u128) as u64
        });
        let std_dev = mean.map(|mean| {
            let variance = times
                .iter()
                .map(|&(_, ms)| (ms as f64 - mean as f64).powi(2))
                .sum::<f64>()
                / times.len() as f64;
            variance.sqrt().round() as u64
        });

        Self {
            count: solves.len(),
            dnf_count: solves.len() - times.len(),
            best: times
                .iter()
                .min_by_key(|&&(_, ms)| ms)
                .map(|&(index, ms)| (ms, index)),
            worst: times
                .iter()
                .max_by_key(|&&(_, ms)| ms)
                .map(|&(index, ms)| (ms, index)),
            mean,
            std_dev,
            windows: AVERAGE_SIZES
                .into_iter()
                .map(|size| window_stats(solves, size))
                .collect(),
        }
    }
}

fn window_stats(solves: &[Solve], size: usize) -> WindowStats {
    let averages = rolling(solves, size);
    WindowStats {
        size,
        current: averages.last().map(|&(_, average)| average),
        best: averages
            .iter()
            .min_by_key(|&&(_, average)| average)
            .map(|&(index, average)| (average, index)),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::event::WcaEvent;
    use crate::solve::Modifier;

    fn solve(ms: u64) -> Solve {
        Solve::new(ms, WcaEvent::Cube3x3, String::new(), Modifier::None)
    }

    fn dnf() -> Solve {
        Solve::new(1, WcaEvent::Cube3x3, String::new(), Modifier::DNF)
    }

    fn solves(times: &[u64]) -> Vec<Solve> {
        times.iter().copied().map(solve).collect()
    }

    #[test]
    fn means_of_three_keep_every_attempt_and_reject_dnfs() {
        assert_eq!(
            average(&solves(&[1_000, 2_000, 6_000])),
            Some(Average::Time(3_000))
        );
        assert_eq!(average(&[solve(1), solve(2), dnf()]), Some(Average::Dnf));
    }

    #[test]
    fn averages_trim_the_best_and_worst() {
        assert_eq!(
            average(&solves(&[1_000, 2_000, 3_000, 4_000, 90_000])),
            Some(Average::Time(3_000))
        );
    }

    #[test]
    fn one_dnf_counts_as_the_worst_and_two_make_a_dnf() {
        let mut attempts = solves(&[1_000, 2_000, 3_000, 4_000]);
        attempts.push(dnf());
        assert_eq!(average(&attempts), Some(Average::Time(3_000)));
        attempts[0] = dnf();
        assert_eq!(average(&attempts), Some(Average::Dnf));
    }

    #[test]
    fn short_windows_have_no_average() {
        assert_eq!(average(&solves(&[1, 2])), None);
        assert_eq!(average_at(&solves(&[1, 2, 3]), 1, 3), None);
        assert_eq!(average_at(&solves(&[1, 2, 3]), 9, 3), None);
    }

    #[test]
    fn rolling_averages_are_keyed_by_their_last_solve() {
        let rolled = rolling(&solves(&[3_000, 3_000, 3_000, 6_000]), 3);
        assert_eq!(
            rolled,
            [(2, Average::Time(3_000)), (3, Average::Time(4_000))]
        );
    }

    #[test]
    fn session_stats_ignore_dnfs_but_count_them() {
        let mut attempts = solves(&[2_000, 4_000]);
        attempts.push(dnf());
        let stats = SessionStats::compute(&attempts);
        assert_eq!(stats.count, 3);
        assert_eq!(stats.dnf_count, 1);
        assert_eq!(stats.best, Some((2_000, 0)));
        assert_eq!(stats.worst, Some((4_000, 1)));
        assert_eq!(stats.mean, Some(3_000));
        assert_eq!(stats.std_dev, Some(1_000));
    }

    #[test]
    fn best_window_prefers_times_over_dnfs() {
        let attempts = [solve(1_000), solve(1_000), solve(1_000), dnf()];
        let stats = SessionStats::compute(&attempts);
        let mo3 = stats.windows[0];
        assert_eq!(mo3.current, Some(Average::Dnf));
        assert_eq!(mo3.best, Some((Average::Time(1_000), 2)));
    }

    #[test]
    fn the_histogram_counts_every_finished_solve_once() {
        let mut attempts = solves(&[1_000, 1_000, 9_000]);
        attempts.push(dnf());
        let buckets = histogram(&attempts, 12);
        assert_eq!(buckets.len(), 12);
        assert_eq!(buckets.iter().map(|&(_, count)| count).sum::<u32>(), 3);
        assert_eq!(buckets[0], (1_000, 2));
        assert_eq!(buckets[11].1, 1);
    }

    #[test]
    fn the_histogram_handles_identical_times_and_no_times() {
        assert_eq!(histogram(&solves(&[1_000, 1_000]), 4)[0], (1_000, 2));
        assert_eq!(histogram(&[dnf()], 4), []);
    }

    #[test]
    fn an_empty_session_has_no_stats() {
        let stats = SessionStats::compute(&[]);
        assert_eq!(stats.best, None);
        assert_eq!(stats.mean, None);
        assert!(stats.windows.iter().all(|window| window.current.is_none()));
    }
}
