//! Session statistics: WCA averages, their rolling values and session summaries.

use std::cmp::Ordering;

use crate::format::format_ms;
use crate::solve::Solve;

/// The rolling windows shown for every session.
pub const AVERAGE_SIZES: [usize; 5] = [3, 5, 12, 50, 100];

/// The index in [`AVERAGE_SIZES`] of the window used until another is chosen.
pub const DEFAULT_AVERAGE: usize = 1;

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
    average_of(attempts.iter().map(Solve::effective_ms))
}

/// The average of the effective times, where `None` is a DNF.
fn average_of(times: impl ExactSizeIterator<Item = Option<u64>>) -> Option<Average> {
    let n = times.len();
    if n < 3 {
        return None;
    }
    let mut sum = 0_u128;
    let mut best = u64::MAX;
    let mut worst = 0;
    let mut dnfs = 0;
    for time in times {
        match time {
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

/// The spread of one column of an [`AverageTable`].
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
struct ColumnRange {
    fastest: u64,
    median: u64,
    slowest: u64,
}

impl ColumnRange {
    fn new(mut times: Vec<u64>) -> Option<Self> {
        times.sort_unstable();
        Some(Self {
            fastest: *times.first()?,
            median: times[times.len() / 2],
            slowest: *times.last()?,
        })
    }
}

/// Every solve's rolling averages, one column per [`AVERAGE_SIZES`].
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct AverageTable {
    rows: Vec<[Option<Average>; AVERAGE_SIZES.len()]>,
    ranges: [Option<ColumnRange>; AVERAGE_SIZES.len()],
}

impl AverageTable {
    pub fn new(solves: &[Solve]) -> Self {
        let mut rows = vec![[None; AVERAGE_SIZES.len()]; solves.len()];
        let mut ranges = [None; AVERAGE_SIZES.len()];
        for (column, size) in AVERAGE_SIZES.into_iter().enumerate() {
            let mut times = Vec::new();
            for (index, average) in rolling(solves, size) {
                rows[index][column] = Some(average);
                times.extend(average.ms());
            }
            ranges[column] = ColumnRange::new(times);
        }
        Self { rows, ranges }
    }

    /// Returns the averages ending at the solve `index`. A window the session
    /// had not reached by then is `None`.
    pub fn row(&self, index: usize) -> Option<&[Option<Average>; AVERAGE_SIZES.len()]> {
        self.rows.get(index)
    }

    /// Places an average within its column: `-1.0` for the fastest, `0.0` for
    /// the median and `1.0` for the slowest. Measuring from the median keeps
    /// one bad average from making every other one look fast. A DNF has no
    /// place.
    pub fn heat(&self, column: usize, average: Average) -> Option<f32> {
        let ms = average.ms()?;
        let range = (*self.ranges.get(column)?)?;
        let share = |distance: u64, span: u64| (distance as f32 / span as f32).min(1.0);
        Some(match ms.cmp(&range.median) {
            Ordering::Less => -share(range.median - ms, range.median - range.fastest),
            Ordering::Equal => 0.0,
            Ordering::Greater => share(ms - range.median, range.slowest - range.median),
        })
    }

    /// Returns whether no average in the column beats this one.
    pub fn is_best(&self, column: usize, average: Average) -> bool {
        let range = self.ranges.get(column).copied().flatten();
        average.ms().is_some() && average.ms() == range.map(|range| range.fastest)
    }
}

/// Moves a choice of window one step along [`AVERAGE_SIZES`], stopping at
/// either end. Nothing chosen yet starts from [`DEFAULT_AVERAGE`].
pub fn step_average(selected: Option<usize>, forward: bool) -> usize {
    let current = selected.unwrap_or(DEFAULT_AVERAGE);
    if forward {
        (current + 1).min(AVERAGE_SIZES.len() - 1)
    } else {
        current.saturating_sub(1)
    }
}

/// Returns the positions of the attempts an average drops, best then worst.
///
/// A DNF is the worst attempt. Of equal times the earliest is the best and the
/// latest the worst, so the two are always different attempts. A mean keeps
/// every attempt, so it returns `None`, as does a window too short to average.
pub fn trimmed(attempts: &[Solve]) -> Option<(usize, usize)> {
    if attempts.len() <= 3 {
        return None;
    }
    let rank = |&position: &usize| attempts[position].effective_ms().unwrap_or(u64::MAX);
    let best = (0..attempts.len()).min_by_key(rank)?;
    let worst = (0..attempts.len()).max_by_key(rank)?;
    Some((best, worst))
}

/// Why an attempt does not count towards its average.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Trim {
    Best,
    Worst,
}

/// One rolling average and the attempts behind it.
#[derive(Clone, Copy, Debug)]
pub struct AverageDetail<'a> {
    attempts: &'a [Solve],
    first: usize,
    average: Average,
    trimmed: Option<(usize, usize)>,
    /// The largest distance from the average among the attempts that count.
    reach: u64,
}

impl<'a> AverageDetail<'a> {
    /// Returns the average of the `size` solves ending at `end`, inclusive.
    pub fn at(solves: &'a [Solve], end: usize, size: usize) -> Option<Self> {
        let first = (end + 1).checked_sub(size)?;
        let attempts = solves.get(first..=end)?;
        let mut detail = Self {
            attempts,
            first,
            average: average(attempts)?,
            trimmed: trimmed(attempts),
            reach: 0,
        };
        detail.reach = (0..attempts.len())
            .filter(|&position| detail.trim(position).is_none())
            .filter_map(|position| detail.deviation(position))
            .map(i64::unsigned_abs)
            .max()
            .unwrap_or(0);
        Some(detail)
    }

    /// Returns the attempts, oldest first.
    pub const fn attempts(&self) -> &'a [Solve] {
        self.attempts
    }

    /// Returns the session index of the oldest attempt.
    pub const fn first(&self) -> usize {
        self.first
    }

    pub const fn average(&self) -> Average {
        self.average
    }

    /// Returns why the attempt at `position` is dropped, if it is.
    pub fn trim(&self, position: usize) -> Option<Trim> {
        let (best, worst) = self.trimmed?;
        if position == best {
            Some(Trim::Best)
        } else if position == worst {
            Some(Trim::Worst)
        } else {
            None
        }
    }

    /// Returns how far the attempt at `position` is from the average, in
    /// milliseconds: negative when faster. A DNF on either side has none.
    pub fn deviation(&self, position: usize) -> Option<i64> {
        let average = self.average.ms()?;
        let time = self.attempts.get(position)?.effective_ms()?;
        Some(i64::try_from(time).ok()? - i64::try_from(average).ok()?)
    }

    /// Places the attempt at `position` around the average, from `-1.0` to
    /// `1.0`, with the attempts that count reaching the ends.
    ///
    /// Trimmed attempts stop at the ends, so one bad solve cannot flatten the
    /// rest, and a DNF sits at the slow end. A DNF average has no centre, so
    /// nothing has a place.
    pub fn offset(&self, position: usize) -> Option<f32> {
        self.average.ms()?;
        let Some(deviation) = self.deviation(position) else {
            return Some(1.0);
        };
        if self.reach == 0 {
            return Some(if deviation == 0 {
                0.0
            } else {
                deviation.signum() as f32
            });
        }
        Some((deviation as f32 / self.reach as f32).clamp(-1.0, 1.0))
    }

    /// Writes the attempt at `position`, in parentheses when it is dropped.
    pub fn attempt_text(&self, position: usize) -> String {
        let time = self.attempts[position].to_string();
        if self.trim(position).is_some() {
            format!("({time})")
        } else {
            time
        }
    }

    /// Writes the average as text to share: `title` and the result, then every
    /// attempt with its scramble.
    pub fn share_text(&self, title: &str) -> String {
        let result = match self.average {
            Average::Time(ms) => format_ms(ms),
            Average::Dnf => "DNF".to_owned(),
        };
        let mut text = format!("{title}: {result}\n");
        for (position, solve) in self.attempts.iter().enumerate() {
            let line = format!(
                "\n{}. {}   {}",
                position + 1,
                self.attempt_text(position),
                solve.scramble()
            );
            text.push_str(line.trim_end());
        }
        text
    }
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
#[derive(Clone, Debug, PartialEq, Eq)]
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
    /// The effective time of every solve, so [`Self::update`] can resume from
    /// the first solve that differs instead of reading the session again.
    times: Vec<Option<u64>>,
    /// How many of `times` count towards the mean, and their totals.
    counted: usize,
    sum: u128,
    squares: u128,
    /// One entry per [`AVERAGE_SIZES`], in the same order.
    caches: Vec<WindowCache>,
}

/// The averages of one window that beat every earlier one.
///
/// Keeping only the records makes the best average cheap to extend and to cut
/// back: truncating the solves pops the records past the new end.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
struct WindowCache {
    /// The best average so far and its last solve, one entry per new record.
    records: Vec<(Average, usize)>,
}

impl Default for SessionStats {
    fn default() -> Self {
        Self {
            count: 0,
            dnf_count: 0,
            best: None,
            worst: None,
            mean: None,
            std_dev: None,
            windows: AVERAGE_SIZES
                .into_iter()
                .map(|size| WindowStats {
                    size,
                    current: None,
                    best: None,
                })
                .collect(),
            times: Vec::new(),
            counted: 0,
            sum: 0,
            squares: 0,
            caches: vec![WindowCache::default(); AVERAGE_SIZES.len()],
        }
    }
}

impl SessionStats {
    /// Summarizes `solves` with every window in [`AVERAGE_SIZES`].
    pub fn compute(solves: &[Solve]) -> Self {
        let mut stats = Self::default();
        stats.update(solves);
        stats
    }

    /// Brings the summary up to date with `solves`.
    ///
    /// Only the solves from the first one that differs from the previous
    /// update are read again, so recording an attempt costs the longest window
    /// instead of the whole session. Changing or removing a solve costs one
    /// pass over the solves after it, which keeps a 100k solve history cheap.
    pub fn update(&mut self, solves: &[Solve]) {
        let common = (0..solves.len().min(self.times.len()))
            .take_while(|&index| self.times[index] == solves[index].effective_ms())
            .count();
        if common == solves.len() && common == self.times.len() {
            return;
        }
        self.rewind_to(common);
        for (index, solve) in solves.iter().enumerate().skip(common) {
            self.push(index, solve.effective_ms());
        }
        self.refresh();
    }

    /// Drops every solve from `len` on, keeping what came before.
    fn rewind_to(&mut self, len: usize) {
        for ms in self.times.drain(len..).flatten() {
            let ms = u128::from(ms);
            self.sum = self.sum.saturating_sub(ms);
            self.squares = self.squares.saturating_sub(ms * ms);
            self.counted -= 1;
        }
        for cache in &mut self.caches {
            while cache.records.last().is_some_and(|&(_, index)| index >= len) {
                cache.records.pop();
            }
        }
        self.count = len;
        self.dnf_count = len - self.counted;
        let dropped = |found: Option<(u64, usize)>| found.is_some_and(|(_, index)| index >= len);
        if dropped(self.best) || dropped(self.worst) {
            self.best = None;
            self.worst = None;
            for index in 0..self.times.len() {
                if let Some(ms) = self.times[index] {
                    self.rank_single(index, ms);
                }
            }
        }
    }

    /// Adds the solve at `index`, which must follow every solve already kept.
    fn push(&mut self, index: usize, time: Option<u64>) {
        self.times.push(time);
        self.count += 1;
        if let Some(ms) = time {
            let wide = u128::from(ms);
            self.counted += 1;
            self.sum += wide;
            self.squares += wide * wide;
            self.rank_single(index, ms);
        } else {
            self.dnf_count += 1;
        }
        for (column, &size) in AVERAGE_SIZES.iter().enumerate() {
            let Some(average) = self.average_ending_at(index, size) else {
                continue;
            };
            let records = &mut self.caches[column].records;
            if records.last().is_none_or(|&(best, _)| average < best) {
                records.push((average, index));
            }
        }
    }

    /// Notes a solve towards the fastest and slowest singles.
    ///
    /// A tie keeps the earliest fastest and the latest slowest, matching the
    /// scan of `Iterator::min_by_key` and `Iterator::max_by_key`.
    fn rank_single(&mut self, index: usize, ms: u64) {
        if self.best.is_none_or(|(best, _)| ms < best) {
            self.best = Some((ms, index));
        }
        if self.worst.is_none_or(|(worst, _)| ms >= worst) {
            self.worst = Some((ms, index));
        }
    }

    /// Returns the average of the `size` solves ending at `end`, inclusive.
    fn average_ending_at(&self, end: usize, size: usize) -> Option<Average> {
        let start = (end + 1).checked_sub(size)?;
        let times = self.times.get(start..=end)?;
        average_of(times.iter().copied())
    }

    /// Rebuilds the figures derived from every solve, which is cheap because
    /// they come from the running totals.
    fn refresh(&mut self) {
        self.mean = None;
        self.std_dev = None;
        if self.counted > 0 {
            let n = self.counted as u128;
            let mean = (self.sum / n) as u64;
            let centre = u128::from(mean);
            // sum((ms - mean)^2), kept in integers so it stays exact. Writing
            // the remainder of the division separately keeps every step
            // positive.
            let remainder = self.sum - n * centre;
            let spread = self
                .squares
                .saturating_sub(n * centre * centre)
                .saturating_sub(2 * centre * remainder);
            self.mean = Some(mean);
            self.std_dev = Some((spread as f64 / n as f64).sqrt().round() as u64);
        }
        self.windows = AVERAGE_SIZES
            .iter()
            .enumerate()
            .map(|(column, &size)| WindowStats {
                size,
                current: self
                    .times
                    .len()
                    .checked_sub(1)
                    .and_then(|end| self.average_ending_at(end, size)),
                best: self.caches[column].records.last().copied(),
            })
            .collect();
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

    fn assert_up_to_date(attempts: &[Solve], stats: &SessionStats) {
        assert_eq!(stats, &SessionStats::compute(attempts));
    }

    #[test]
    fn updating_agrees_with_computing_from_scratch() {
        let mut attempts = solves(&[5_000, 1_000, 9_000, 4_000, 3_000, 7_000, 2_000, 8_000]);
        let mut stats = SessionStats::compute(&attempts);
        assert_up_to_date(&attempts, &stats);

        // Recording an attempt only adds to the end.
        attempts.push(solve(6_000));
        stats.update(&attempts);
        assert_up_to_date(&attempts, &stats);

        // A penalty on an old solve, a slower time in the middle, and a
        // deletion all rewrite what comes after them.
        attempts[3] = dnf();
        stats.update(&attempts);
        assert_up_to_date(&attempts, &stats);
        attempts[2] = solve(500);
        stats.update(&attempts);
        assert_up_to_date(&attempts, &stats);
        attempts.remove(0);
        stats.update(&attempts);
        assert_up_to_date(&attempts, &stats);

        // Cutting the history back and filling it with DNFs.
        attempts.truncate(3);
        stats.update(&attempts);
        assert_up_to_date(&attempts, &stats);
        attempts.extend([dnf(), dnf(), dnf()]);
        stats.update(&attempts);
        assert_up_to_date(&attempts, &stats);

        // A different session that shares nothing with the last one.
        let mut replaced = solves(&[3_000, 3_000, 3_000, 2_000]);
        stats.update(&replaced);
        assert_up_to_date(&replaced, &stats);
        replaced.clear();
        stats.update(&replaced);
        assert_up_to_date(&replaced, &stats);
    }

    #[test]
    fn updating_without_a_change_keeps_the_same_summary() {
        let attempts = solves(&[5_000, 1_000, 9_000, 4_000, 3_000]);
        let mut stats = SessionStats::compute(&attempts);
        let before = stats.clone();
        stats.update(&attempts);
        assert_eq!(stats, before);
    }

    #[test]
    fn the_slowest_tie_moves_back_when_the_newest_goes() {
        let mut attempts = solves(&[2_000, 5_000, 5_000]);
        let mut stats = SessionStats::compute(&attempts);
        assert_eq!(stats.worst, Some((5_000, 2)));
        attempts.pop();
        stats.update(&attempts);
        assert_eq!(stats.worst, Some((5_000, 1)));
        assert_up_to_date(&attempts, &stats);
    }

    #[test]
    fn removing_the_best_average_falls_back_to_the_one_before_it() {
        let mut attempts = solves(&[5_000, 5_000, 1_000, 1_000, 1_000]);
        let mut stats = SessionStats::compute(&attempts);
        assert_eq!(stats.windows[0].best, Some((Average::Time(1_000), 4)));
        attempts.truncate(4);
        stats.update(&attempts);
        assert_eq!(stats.windows[0].best, Some((Average::Time(2_333), 3)));
        assert_up_to_date(&attempts, &stats);
    }

    #[test]
    fn an_empty_session_still_lists_every_window() {
        let stats = SessionStats::compute(&[]);
        assert_eq!(stats.windows.len(), AVERAGE_SIZES.len());
        assert!(stats.windows.iter().all(|window| window.best.is_none()));
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
    fn the_table_has_a_row_per_solve_and_blanks_for_short_windows() {
        let table = AverageTable::new(&solves(&[3_000, 3_000, 3_000, 6_000, 3_000]));
        assert_eq!(table.row(1), Some(&[None; AVERAGE_SIZES.len()]));
        let third = table.row(2).unwrap();
        assert_eq!(third[0], Some(Average::Time(3_000)));
        assert_eq!(third[1], None);
        assert_eq!(table.row(3).unwrap()[0], Some(Average::Time(4_000)));
        assert_eq!(table.row(4).unwrap()[1], Some(Average::Time(3_000)));
        assert_eq!(table.row(4).unwrap()[2], None);
        assert_eq!(table.row(5), None);
    }

    #[test]
    fn the_table_agrees_with_the_averages_it_opens() {
        let attempts = solves(&[5_000, 1_000, 9_000, 4_000, 3_000, 7_000, 2_000]);
        let table = AverageTable::new(&attempts);
        for index in 0..attempts.len() {
            for (column, &size) in AVERAGE_SIZES.iter().enumerate() {
                let detail = AverageDetail::at(&attempts, index, size);
                assert_eq!(
                    table.row(index).unwrap()[column],
                    detail.map(|detail| detail.average())
                );
            }
        }
    }

    #[test]
    fn heat_runs_from_the_fastest_through_the_median_to_the_slowest() {
        let table = AverageTable::new(&solves(&[1_000, 1_000, 1_000, 4_000, 7_000]));
        assert_eq!(table.heat(0, Average::Time(1_000)), Some(-1.0));
        assert_eq!(table.heat(0, Average::Time(1_500)), Some(-0.5));
        assert_eq!(table.heat(0, Average::Time(2_000)), Some(0.0));
        assert_eq!(table.heat(0, Average::Time(3_000)), Some(0.5));
        assert_eq!(table.heat(0, Average::Time(4_000)), Some(1.0));
        assert_eq!(table.heat(0, Average::Dnf), None);
    }

    #[test]
    fn one_slow_average_leaves_the_rest_around_the_median() {
        let mut attempts = solves(&[2_000; 12]);
        attempts[1] = solve(1_000);
        attempts.extend(solves(&[90_000; 3]));
        let table = AverageTable::new(&attempts);
        assert_eq!(table.heat(0, Average::Time(2_000)), Some(0.0));
    }

    #[test]
    fn a_column_of_equal_times_has_no_extremes() {
        let table = AverageTable::new(&solves(&[2_000; 4]));
        assert_eq!(table.heat(0, Average::Time(2_000)), Some(0.0));
    }

    #[test]
    fn the_best_of_a_column_ignores_dnfs() {
        let table = AverageTable::new(&[solve(1_000), solve(2_000), solve(3_000), dnf()]);
        assert!(table.is_best(0, Average::Time(2_000)));
        assert!(!table.is_best(0, Average::Time(3_000)));
        assert!(!table.is_best(0, Average::Dnf));
        assert!(!table.is_best(1, Average::Time(2_000)));
    }

    #[test]
    fn stepping_the_average_stops_at_both_ends() {
        assert_eq!(step_average(None, true), DEFAULT_AVERAGE + 1);
        assert_eq!(step_average(None, false), DEFAULT_AVERAGE - 1);
        assert_eq!(step_average(Some(0), false), 0);
        let last = AVERAGE_SIZES.len() - 1;
        assert_eq!(step_average(Some(last), true), last);
    }

    #[test]
    fn a_mean_trims_nothing() {
        assert_eq!(trimmed(&solves(&[1_000, 2_000, 6_000])), None);
    }

    #[test]
    fn an_average_trims_its_best_and_worst() {
        let attempts = solves(&[3_000, 1_000, 9_000, 4_000, 5_000]);
        assert_eq!(trimmed(&attempts), Some((1, 2)));
    }

    #[test]
    fn penalties_count_when_trimming() {
        let mut attempts = solves(&[1_000, 3_000, 4_000, 5_000]);
        attempts[0] = Solve::new(4_500, WcaEvent::Cube3x3, String::new(), Modifier::PlusTwo);
        assert_eq!(trimmed(&attempts), Some((1, 0)));
    }

    #[test]
    fn a_dnf_is_trimmed_as_the_worst() {
        let mut attempts = solves(&[3_000, 1_000, 4_000, 5_000]);
        attempts.insert(2, dnf());
        assert_eq!(trimmed(&attempts), Some((1, 2)));
    }

    #[test]
    fn equal_times_trim_two_different_attempts() {
        assert_eq!(trimmed(&solves(&[2_000; 5])), Some((0, 4)));
        assert_eq!(trimmed(&[dnf(), dnf(), dnf(), dnf()]), Some((0, 3)));
    }

    #[test]
    fn a_detail_covers_the_window_ending_at_its_solve() {
        let attempts = solves(&[9_000, 3_000, 1_000, 9_000, 4_000, 5_000]);
        let detail = AverageDetail::at(&attempts, 5, 5).unwrap();
        assert_eq!(detail.first(), 1);
        assert_eq!(detail.attempts().len(), 5);
        assert_eq!(detail.average(), Average::Time(4_000));
        assert_eq!(detail.trim(0), None);
        assert_eq!(detail.trim(1), Some(Trim::Best));
        assert_eq!(detail.trim(2), Some(Trim::Worst));
        assert_eq!(detail.trim(3), None);
        assert_eq!(detail.attempt_text(0), "3.000");
        assert_eq!(detail.attempt_text(1), "(1.000)");
    }

    #[test]
    fn deviations_are_measured_from_the_average() {
        let attempts = solves(&[3_000, 1_000, 9_000, 4_000, 5_000]);
        let detail = AverageDetail::at(&attempts, 4, 5).unwrap();
        assert_eq!(detail.deviation(0), Some(-1_000));
        assert_eq!(detail.deviation(3), Some(0));
        assert_eq!(detail.deviation(4), Some(1_000));
    }

    #[test]
    fn counting_attempts_reach_the_ends_and_trimmed_ones_stop_there() {
        let attempts = solves(&[3_000, 1_000, 9_000, 4_000, 5_000]);
        let detail = AverageDetail::at(&attempts, 4, 5).unwrap();
        assert_eq!(detail.offset(0), Some(-1.0));
        assert_eq!(detail.offset(3), Some(0.0));
        assert_eq!(detail.offset(4), Some(1.0));
        assert_eq!(detail.offset(1), Some(-1.0));
        assert_eq!(detail.offset(2), Some(1.0));
    }

    #[test]
    fn a_dnf_sits_at_the_slow_end() {
        let attempts = [
            solve(1_000),
            solve(2_000),
            solve(3_000),
            solve(4_000),
            dnf(),
        ];
        let detail = AverageDetail::at(&attempts, 4, 5).unwrap();
        assert_eq!(detail.deviation(4), None);
        assert_eq!(detail.offset(4), Some(1.0));
    }

    #[test]
    fn a_dnf_average_has_no_centre() {
        let attempts = [solve(1_000), dnf(), solve(2_000), dnf()];
        let detail = AverageDetail::at(&attempts, 3, 4).unwrap();
        assert_eq!(detail.deviation(0), None);
        assert_eq!(detail.offset(0), None);
    }

    #[test]
    fn equal_times_all_sit_on_the_average() {
        let attempts = solves(&[2_000; 5]);
        let detail = AverageDetail::at(&attempts, 4, 5).unwrap();
        assert_eq!(detail.offset(0), Some(0.0));
        assert_eq!(detail.offset(4), Some(0.0));
    }

    #[test]
    fn a_detail_needs_a_complete_window() {
        let attempts = solves(&[1_000, 2_000, 3_000]);
        assert!(AverageDetail::at(&attempts, 1, 3).is_none());
        assert!(AverageDetail::at(&attempts, 3, 3).is_none());
        assert!(AverageDetail::at(&attempts, 2, 5).is_none());
    }

    #[test]
    fn shared_text_lists_every_attempt_with_its_scramble() {
        let scrambled = |ms, scramble: &str, modifier| {
            Solve::new(ms, WcaEvent::Cube3x3, scramble.to_owned(), modifier)
        };
        let attempts = [
            scrambled(2_000, "R U", Modifier::None),
            scrambled(1_000, "F2", Modifier::None),
            scrambled(3_000, "L'", Modifier::PlusTwo),
            scrambled(4_000, "D", Modifier::DNF),
        ];
        let detail = AverageDetail::at(&attempts, 3, 4).unwrap();
        assert_eq!(
            detail.share_text("Average of 4"),
            "Average of 4: 3.500\n\n\
             1. 2.000   R U\n\
             2. (1.000)   F2\n\
             3. 5.000+   L'\n\
             4. (DNF)   D"
        );
    }

    #[test]
    fn shared_text_leaves_no_trailing_space_without_a_scramble() {
        let attempts = solves(&[1_000, 2_000, 3_000]);
        let detail = AverageDetail::at(&attempts, 2, 3).unwrap();
        assert_eq!(
            detail.share_text("Mean of 3"),
            "Mean of 3: 2.000\n\n1. 1.000\n2. 2.000\n3. 3.000"
        );
    }

    #[test]
    fn a_dnf_average_is_shared_as_dnf() {
        let attempts = [solve(1_000), dnf(), solve(2_000), dnf()];
        let detail = AverageDetail::at(&attempts, 3, 4).unwrap();
        assert!(
            detail
                .share_text("Average of 4")
                .starts_with("Average of 4: DNF\n")
        );
    }

    #[test]
    fn an_empty_session_has_no_stats() {
        let stats = SessionStats::compute(&[]);
        assert_eq!(stats.best, None);
        assert_eq!(stats.mean, None);
        assert!(stats.windows.iter().all(|window| window.current.is_none()));
    }
}
