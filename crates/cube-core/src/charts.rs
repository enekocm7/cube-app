//! The data behind the charts screen: which solves a filter keeps, the series
//! drawn from them, the calendar of solves per day, the comparison of sessions
//! and the all-time bests.

use std::collections::BTreeMap;
use std::ops::Range;

use chrono::{DateTime, Datelike, Days, Local, NaiveDate, TimeDelta};

use crate::event::WcaEvent;
use crate::history::History;
use crate::solve::Solve;
use crate::stats::{self, AVERAGE_SIZES, SessionStats};
use crate::tags;

/// The rolling averages drawn over the solve times, and compared between
/// sessions.
pub const OVERLAY_SIZES: [usize; 3] = [5, 12, 100];

/// How many values a [`CompareRow`] holds: the single, the mean and each of
/// [`OVERLAY_SIZES`].
pub const COMPARE_COLUMNS: usize = 2 + OVERLAY_SIZES.len();

/// The fewest solves the times chart zooms in to.
pub const MIN_ZOOM_SOLVES: f64 = 5.;

/// The strongest shade of a calendar day.
pub const MAX_HEAT: u8 = 4;

const DATE_FORMAT: &str = "%Y-%m-%d";

/// The views of the charts screen.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum ChartView {
    #[default]
    Trends,
    Calendar,
    Compare,
    Tags,
    Bests,
}

impl ChartView {
    /// Every view, in tab order.
    pub const ALL: [Self; 5] = [
        Self::Trends,
        Self::Calendar,
        Self::Compare,
        Self::Tags,
        Self::Bests,
    ];

    /// Returns the view after this one, wrapping to the first.
    #[must_use]
    pub fn next(self) -> Self {
        let index = Self::ALL.iter().position(|&view| view == self).unwrap_or(0);
        Self::ALL[(index + 1) % Self::ALL.len()]
    }
}

/// Which part of a session's history the charts show.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum ChartRange {
    #[default]
    All,
    LastSolves(usize),
    /// The last `n` days, counting today.
    LastDays(u32),
    /// Every day from `from` to `to`, both included.
    Dates {
        from: NaiveDate,
        to: NaiveDate,
    },
}

impl ChartRange {
    /// The ranges offered without typing a date, in menu order.
    pub const PRESETS: [Self; 8] = [
        Self::All,
        Self::LastSolves(50),
        Self::LastSolves(100),
        Self::LastSolves(500),
        Self::LastDays(7),
        Self::LastDays(30),
        Self::LastDays(90),
        Self::LastDays(365),
    ];

    /// Returns the preset after this range; custom dates move to the first.
    #[must_use]
    pub fn next(self) -> Self {
        Self::PRESETS
            .iter()
            .position(|&range| range == self)
            .map_or(Self::PRESETS[0], |index| {
                Self::PRESETS[(index + 1) % Self::PRESETS.len()]
            })
    }

    /// Reads a range typed as two `YYYY-MM-DD` dates. Returns `None` when
    /// either is malformed or the range ends before it starts.
    pub fn from_dates(from: &str, to: &str) -> Option<Self> {
        let (from, to) = (parse_day(from)?, parse_day(to)?);
        (from <= to).then_some(Self::Dates { from, to })
    }

    /// Returns whether a solve finished on `day` is inside a date range.
    /// Ranges that do not depend on dates keep every day.
    fn keeps_day(self, day: Option<NaiveDate>, today: NaiveDate) -> bool {
        match self {
            Self::All | Self::LastSolves(_) => true,
            Self::LastDays(days) => {
                let first = today
                    .checked_sub_days(Days::new(u64::from(days.saturating_sub(1))))
                    .unwrap_or(NaiveDate::MIN);
                day.is_some_and(|day| day >= first)
            }
            Self::Dates { from, to } => day.is_some_and(|day| (from..=to).contains(&day)),
        }
    }
}

/// What the charts screen keeps of each session.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct ChartFilter {
    /// The event to show, or `None` for every event.
    pub event: Option<WcaEvent>,
    pub range: ChartRange,
    /// Whether to show every session merged, not just the current one. The
    /// comparison always lists every session, so it ignores this.
    pub all_sessions: bool,
    /// The tag a solve must have, or `None` for every solve. The stats table
    /// follows this too.
    pub tag: Option<String>,
}

/// One position of the times chart.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct ChartPoint {
    /// The index of the solve's session.
    pub session: usize,
    /// The solve's 1-based number in its session, as in the stats table.
    pub number: usize,
    /// What the x axis calls this point: the session number, or the place in
    /// the merged history when the chart spans sessions.
    pub axis: usize,
    /// The penalty-adjusted time, or `None` for a DNF.
    pub ms: Option<u64>,
    /// The averages of [`OVERLAY_SIZES`] ending at this solve. `None` while
    /// there are too few solves, and for a DNF average.
    pub averages: [Option<u64>; OVERLAY_SIZES.len()],
    /// Whether this single beat every earlier one of the filtered history.
    pub personal_best: bool,
}

/// A solve of a [`Selection`], with where it came from.
struct Entry {
    session: usize,
    /// The solve's 1-based number in its session.
    number: usize,
    solve: Solve,
}

/// The solves of one session, or of all of them, after a filter.
///
/// Averages and personal bests are worked out on every solve of the event and
/// only then cut to the range, so a short range still shows its ao100.
pub struct Selection {
    /// The solves of the event, oldest first.
    entries: Vec<Entry>,
    /// Positions in `entries` of those inside the range.
    visible: Vec<usize>,
    /// Whether `entries` merges several sessions.
    merged: bool,
}

impl Selection {
    /// Selects from the solves of a single session.
    pub fn new(solves: &[Solve], filter: &ChartFilter, today: NaiveDate) -> Self {
        Self::build([(0, solves)], false, filter, today)
    }

    /// Selects what `filter` shows: the session at `current`, or every session
    /// merged by date. Solves of unknown date come first.
    pub fn across(
        sessions: &[History],
        current: usize,
        filter: &ChartFilter,
        today: NaiveDate,
    ) -> Self {
        let chosen = sessions
            .iter()
            .enumerate()
            .filter(|&(index, _)| filter.all_sessions || index == current)
            .map(|(index, session)| (index, session.solves()));
        Self::build(chosen, filter.all_sessions, filter, today)
    }

    fn build<'a>(
        sessions: impl IntoIterator<Item = (usize, &'a [Solve])>,
        merged: bool,
        filter: &ChartFilter,
        today: NaiveDate,
    ) -> Self {
        let mut entries: Vec<Entry> = sessions
            .into_iter()
            .flat_map(|(session, solves)| {
                solves
                    .iter()
                    .enumerate()
                    .filter(|(_, solve)| filter.event.is_none_or(|event| solve.event() == event))
                    .filter(|(_, solve)| tags::keeps(solve, filter.tag.as_deref()))
                    .map(move |(index, solve)| Entry {
                        session,
                        number: index + 1,
                        solve: solve.clone(),
                    })
            })
            .collect();
        if merged {
            entries.sort_by_key(|entry| entry.solve.solved_at_unix_ms());
        }
        let skipped = match filter.range {
            ChartRange::LastSolves(count) => entries.len().saturating_sub(count),
            _ => 0,
        };
        let visible = entries
            .iter()
            .enumerate()
            .skip(skipped)
            .filter(|(_, entry)| filter.range.keeps_day(solve_day(&entry.solve), today))
            .map(|(position, _)| position)
            .collect();
        Self {
            entries,
            visible,
            merged,
        }
    }

    /// Returns the solves inside the range, oldest first.
    pub fn visible(&self) -> Vec<Solve> {
        self.visible
            .iter()
            .map(|&position| self.entries[position].solve.clone())
            .collect()
    }

    /// Returns the times chart's series, one point per solve inside the range.
    pub fn points(&self) -> Vec<ChartPoint> {
        let averages = self.overlays();
        let records = self.personal_bests();
        self.visible
            .iter()
            .map(|&position| {
                let entry = &self.entries[position];
                ChartPoint {
                    session: entry.session,
                    number: entry.number,
                    axis: if self.merged {
                        position + 1
                    } else {
                        entry.number
                    },
                    ms: entry.solve.effective_ms(),
                    averages: averages[position],
                    personal_best: records[position],
                }
            })
            .collect()
    }

    /// Works out the overlays of each entry. An average never spans two
    /// sessions, so these are the same values as in that session's stats.
    fn overlays(&self) -> Vec<[Option<u64>; OVERLAY_SIZES.len()]> {
        let mut sessions: BTreeMap<usize, Vec<usize>> = BTreeMap::new();
        for (position, entry) in self.entries.iter().enumerate() {
            sessions.entry(entry.session).or_default().push(position);
        }
        let mut averages = vec![[None; OVERLAY_SIZES.len()]; self.entries.len()];
        for positions in sessions.values_mut() {
            positions.sort_by_key(|&position| self.entries[position].number);
            let solves: Vec<Solve> = positions
                .iter()
                .map(|&position| self.entries[position].solve.clone())
                .collect();
            for (column, size) in OVERLAY_SIZES.into_iter().enumerate() {
                for (end, average) in stats::rolling(&solves, size) {
                    averages[positions[end]][column] = average.ms();
                }
            }
        }
        averages
    }

    /// Marks each solve strictly faster than every solve before it.
    fn personal_bests(&self) -> Vec<bool> {
        let mut best: Option<u64> = None;
        self.entries
            .iter()
            .map(|entry| {
                let ms = entry.solve.effective_ms();
                let record = ms.is_some_and(|ms| best.is_none_or(|best| ms < best));
                if record {
                    best = ms;
                }
                record
            })
            .collect()
    }
}

/// The stretch of a series the times chart shows.
///
/// It is fractional so that the small steps of a trackpad add up instead of
/// rounding away.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct ChartZoom {
    start: f64,
    len: f64,
}

impl ChartZoom {
    /// The whole of a series of `count` points.
    pub fn full(count: usize) -> Self {
        Self {
            start: 0.,
            len: count as f64,
        }
    }

    /// Returns whether every one of `count` points is showing.
    pub fn is_full(self, count: usize) -> bool {
        self.len >= count as f64
    }

    /// Scales the stretch by `factor`, below `1` to zoom in. The point at
    /// `anchor`, a share of the stretch from `0` to `1`, stays where it is.
    #[must_use]
    pub fn scaled(self, count: usize, factor: f64, anchor: f64) -> Self {
        let total = count as f64;
        let len = (self.len * factor).clamp(MIN_ZOOM_SOLVES.min(total), total);
        let start = self.start + (self.len - len) * anchor.clamp(0., 1.);
        Self { start, len }.within(total)
    }

    /// Moves the stretch by `share` of its own length, forward when positive.
    #[must_use]
    pub fn panned(self, count: usize, share: f64) -> Self {
        let start = self.start + self.len * share;
        Self { start, ..self }.within(count as f64)
    }

    fn within(self, total: f64) -> Self {
        Self {
            start: self.start.clamp(0., (total - self.len).max(0.)),
            ..self
        }
    }

    /// Returns the indices showing of a series of `count` points.
    pub fn range(self, count: usize) -> Range<usize> {
        let start = (self.start.round() as usize).min(count);
        let end = (start + self.len.round() as usize).min(count);
        start..end
    }
}

/// Solves per local day.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Calendar {
    days: BTreeMap<NaiveDate, u32>,
}

impl Calendar {
    /// Counts `solves` by day. Solves with an unknown date have no day.
    pub fn new(solves: &[Solve]) -> Self {
        let mut days = BTreeMap::new();
        for day in solves.iter().filter_map(solve_day) {
            *days.entry(day).or_insert(0) += 1;
        }
        Self { days }
    }

    /// Returns how many solves finished on `day`.
    pub fn count(&self, day: NaiveDate) -> u32 {
        self.days.get(&day).copied().unwrap_or(0)
    }

    /// Returns the latest day with a solve.
    pub fn latest(&self) -> Option<NaiveDate> {
        self.days.keys().next_back().copied()
    }

    /// Returns the totals of one year.
    pub fn year(&self, year: i32) -> YearSummary {
        let mut summary = YearSummary::default();
        let days = self
            .days
            .iter()
            .filter(|(day, _)| day.year() == year)
            .map(|(&day, &count)| (day, count));
        for (day, count) in days {
            summary.solves += count;
            summary.active_days += 1;
            if summary.busiest.is_none_or(|(_, most)| count > most) {
                summary.busiest = Some((day, count));
            }
        }
        summary
    }
}

/// The totals of one calendar year.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct YearSummary {
    pub solves: u32,
    pub active_days: u32,
    /// The earliest day with the most solves, and how many.
    pub busiest: Option<(NaiveDate, u32)>,
}

/// Returns how strongly to shade a day of `count` solves in a year whose
/// busiest day had `most`: `0` for none, up to [`MAX_HEAT`].
pub fn heat_level(count: u32, most: u32) -> u8 {
    if count == 0 || most == 0 {
        return 0;
    }
    let level = (u64::from(count) * u64::from(MAX_HEAT)).div_ceil(u64::from(most));
    level.clamp(1, u64::from(MAX_HEAT)) as u8
}

/// Lays `year` out in weeks from Monday to Sunday. Days of the neighbouring
/// years stay empty.
pub fn year_weeks(year: i32) -> Vec<[Option<NaiveDate>; 7]> {
    let Some(first) = NaiveDate::from_ymd_opt(year, 1, 1) else {
        return Vec::new();
    };
    let mut weeks = Vec::new();
    let mut week = [None; 7];
    for day in first.iter_days().take_while(|day| day.year() == year) {
        let slot = day.weekday().num_days_from_monday() as usize;
        week[slot] = Some(day);
        if slot == 6 {
            weeks.push(week);
            week = [None; 7];
        }
    }
    if week.iter().any(Option::is_some) {
        weeks.push(week);
    }
    weeks
}

/// Moves `day` by `delta` days, stopping at `today`.
pub fn step_day(day: NaiveDate, delta: i64, today: NaiveDate) -> NaiveDate {
    day.checked_add_signed(TimeDelta::days(delta))
        .unwrap_or(day)
        .min(today)
}

/// Moves `day` by `delta` years, stopping at `today`. The 29th of February
/// becomes the 28th in a year without one.
pub fn step_year(day: NaiveDate, delta: i32, today: NaiveDate) -> NaiveDate {
    let year = day.year().saturating_add(delta);
    day.with_year(year)
        .or_else(|| day.pred_opt()?.with_year(year))
        .unwrap_or(day)
        .min(today)
}

/// One session's results under a filter.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct CompareRow {
    /// The session's index.
    pub session: usize,
    pub count: usize,
    /// The best single, the mean and the best of each of [`OVERLAY_SIZES`].
    pub values: [Option<u64>; COMPARE_COLUMNS],
}

/// The best single, the mean and the best of each of [`OVERLAY_SIZES`].
fn compare_values(stats: &SessionStats) -> [Option<u64>; COMPARE_COLUMNS] {
    let mut values = [None; COMPARE_COLUMNS];
    values[0] = stats.best.map(|(ms, _)| ms);
    values[1] = stats.mean;
    for (column, size) in OVERLAY_SIZES.into_iter().enumerate() {
        values[2 + column] = best_average(stats, size);
    }
    values
}

impl CompareRow {
    fn new(session: usize, solves: &[Solve]) -> Self {
        let stats = SessionStats::compute(solves);
        Self {
            session,
            count: stats.count,
            values: compare_values(&stats),
        }
    }

    /// Returns the session's mean.
    pub const fn mean(&self) -> Option<u64> {
        self.values[1]
    }
}

/// Summarizes every session with solves under `filter`.
pub fn compare(sessions: &[History], filter: &ChartFilter, today: NaiveDate) -> Vec<CompareRow> {
    sessions
        .iter()
        .enumerate()
        .map(|(index, session)| {
            let selection = Selection::new(session.solves(), filter, today);
            CompareRow::new(index, &selection.visible())
        })
        .filter(|row| row.count > 0)
        .collect()
}

/// One side of a comparison of tags.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct TagSide {
    pub count: usize,
    /// The best single, the mean and the best of each of [`OVERLAY_SIZES`].
    pub values: [Option<u64>; COMPARE_COLUMNS],
}

impl TagSide {
    /// Summarizes the solves `filter` shows with `tag`, or all of them.
    ///
    /// Across sessions the solves count as one run, oldest first, so that
    /// what is compared is the tag and not where its solves happen to be.
    pub fn new(
        sessions: &[History],
        current: usize,
        filter: &ChartFilter,
        tag: Option<&str>,
        today: NaiveDate,
    ) -> Self {
        let filter = ChartFilter {
            tag: tag.map(str::to_owned),
            ..filter.clone()
        };
        let visible = Selection::across(sessions, current, &filter, today).visible();
        let stats = SessionStats::compute(&visible);
        Self {
            count: stats.count,
            values: compare_values(&stats),
        }
    }
}

/// Returns the fastest value of each column of `rows`.
pub fn fastest(rows: &[CompareRow]) -> [Option<u64>; COMPARE_COLUMNS] {
    std::array::from_fn(|column| rows.iter().filter_map(|row| row.values[column]).min())
}

/// A best result and the session it belongs to.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Record {
    pub ms: u64,
    pub session: usize,
}

/// One event's best results across every session.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct EventBests {
    pub event: WcaEvent,
    pub count: usize,
    pub single: Option<Record>,
    /// The best of each of [`AVERAGE_SIZES`].
    pub averages: [Option<Record>; AVERAGE_SIZES.len()],
}

/// Returns the best results of every event with solves, across all sessions.
///
/// An average never spans two sessions or two events. On a tie the earlier
/// session keeps the record.
pub fn all_time_bests(sessions: &[History]) -> Vec<EventBests> {
    WcaEvent::ALL
        .into_iter()
        .filter_map(|event| {
            let mut bests = EventBests {
                event,
                count: 0,
                single: None,
                averages: [None; AVERAGE_SIZES.len()],
            };
            for (session, history) in sessions.iter().enumerate() {
                let solves: Vec<Solve> = history
                    .solves()
                    .iter()
                    .filter(|solve| solve.event() == event)
                    .cloned()
                    .collect();
                let stats = SessionStats::compute(&solves);
                bests.count += stats.count;
                improve(&mut bests.single, stats.best.map(|(ms, _)| ms), session);
                for (column, size) in AVERAGE_SIZES.into_iter().enumerate() {
                    improve(
                        &mut bests.averages[column],
                        best_average(&stats, size),
                        session,
                    );
                }
            }
            (bests.count > 0).then_some(bests)
        })
        .collect()
}

fn improve(record: &mut Option<Record>, ms: Option<u64>, session: usize) {
    if let Some(ms) = ms
        && record.is_none_or(|record| ms < record.ms)
    {
        *record = Some(Record { ms, session });
    }
}

/// Returns the best timed average of `size`.
fn best_average(stats: &SessionStats, size: usize) -> Option<u64> {
    let window = stats.windows.iter().find(|window| window.size == size)?;
    window.best?.0.ms()
}

/// Returns the events with at least one solve, in navigation order.
pub fn events(sessions: &[History]) -> Vec<WcaEvent> {
    WcaEvent::ALL
        .into_iter()
        .filter(|&event| {
            sessions
                .iter()
                .any(|session| session.solves().iter().any(|solve| solve.event() == event))
        })
        .collect()
}

/// Returns the event filter after `current`: every event, then each of
/// `available` in turn, then every event again.
pub fn next_event(current: Option<WcaEvent>, available: &[WcaEvent]) -> Option<WcaEvent> {
    match current {
        None => available.first().copied(),
        Some(event) => {
            let index = available.iter().position(|&item| item == event)?;
            available.get(index + 1).copied()
        }
    }
}

/// Returns the current local day.
pub fn today() -> NaiveDate {
    Local::now().date_naive()
}

/// Reads a `YYYY-MM-DD` date.
pub fn parse_day(text: &str) -> Option<NaiveDate> {
    NaiveDate::parse_from_str(text.trim(), DATE_FORMAT).ok()
}

/// Writes a date as `YYYY-MM-DD`, the form [`parse_day`] reads.
pub fn format_day(day: NaiveDate) -> String {
    day.format(DATE_FORMAT).to_string()
}

/// Returns the local day a solve finished on, or `None` when it is unknown.
fn solve_day(solve: &Solve) -> Option<NaiveDate> {
    let unix_ms = i64::try_from(solve.solved_at_unix_ms())
        .ok()
        .filter(|&ms| ms > 0)?;
    let date = DateTime::from_timestamp_millis(unix_ms)?;
    Some(date.with_timezone(&Local).date_naive())
}

#[cfg(test)]
mod tests {
    use chrono::TimeZone;

    use super::*;
    use crate::solve::Modifier;

    fn day(year: i32, month: u32, date: u32) -> NaiveDate {
        NaiveDate::from_ymd_opt(year, month, date).unwrap()
    }

    fn solve_at(ms: u64, event: WcaEvent, unix_ms: u64, modifier: Modifier) -> Solve {
        Solve::new_with_meta(ms, event, String::new(), unix_ms, modifier, String::new())
    }

    /// A solve at noon local time on `date`.
    fn on(date: NaiveDate, ms: u64, event: WcaEvent) -> Solve {
        let noon = Local
            .from_local_datetime(&date.and_hms_opt(12, 0, 0).unwrap())
            .unwrap();
        solve_at(ms, event, noon.timestamp_millis() as u64, Modifier::None)
    }

    /// A 3x3 solve with an unknown date.
    fn solve(ms: u64) -> Solve {
        solve_at(ms, WcaEvent::Cube3x3, 0, Modifier::None)
    }

    fn dnf() -> Solve {
        solve_at(1000, WcaEvent::Cube3x3, 0, Modifier::DNF)
    }

    fn session(solves: Vec<Solve>) -> History {
        let mut history = History::new();
        for solve in solves {
            history.push(solve);
        }
        history
    }

    fn filter(event: Option<WcaEvent>, range: ChartRange) -> ChartFilter {
        ChartFilter {
            event,
            range,
            all_sessions: false,
            tag: None,
        }
    }

    fn everything(event: Option<WcaEvent>, range: ChartRange) -> ChartFilter {
        ChartFilter {
            all_sessions: true,
            ..filter(event, range)
        }
    }

    fn numbers(points: &[ChartPoint]) -> Vec<usize> {
        points.iter().map(|point| point.number).collect()
    }

    #[test]
    fn views_and_ranges_wrap_around() {
        assert_eq!(ChartView::Trends.next(), ChartView::Calendar);
        assert_eq!(ChartView::Bests.next(), ChartView::Trends);
        assert_eq!(ChartRange::All.next(), ChartRange::LastSolves(50));
        assert_eq!(ChartRange::LastDays(365).next(), ChartRange::All);
        let custom = ChartRange::from_dates("2026-01-01", "2026-01-02").unwrap();
        assert_eq!(custom.next(), ChartRange::All);
    }

    #[test]
    fn typed_ranges_need_two_ordered_dates() {
        assert_eq!(
            ChartRange::from_dates(" 2026-01-01", "2026-01-01 "),
            Some(ChartRange::Dates {
                from: day(2026, 1, 1),
                to: day(2026, 1, 1),
            })
        );
        assert_eq!(ChartRange::from_dates("2026-01-02", "2026-01-01"), None);
        assert_eq!(ChartRange::from_dates("", "2026-01-01"), None);
        assert_eq!(ChartRange::from_dates("2026-02-30", "2026-03-01"), None);
        assert_eq!(
            parse_day(&format_day(day(2026, 3, 4))),
            Some(day(2026, 3, 4))
        );
    }

    fn tagged(day: NaiveDate, ms: u64, tag: &str) -> Solve {
        let mut solve = on(day, ms, WcaEvent::Cube3x3);
        solve.set_tags(vec![tag.to_owned()]);
        solve
    }

    #[test]
    fn the_tag_filter_keeps_session_numbers_and_ignores_case() {
        let today = day(2026, 6, 1);
        let solves = [
            tagged(today, 1000, "GAN"),
            on(today, 2000, WcaEvent::Cube3x3),
            tagged(today, 3000, "gan"),
        ];
        let filter = ChartFilter {
            tag: Some("Gan".to_owned()),
            ..ChartFilter::default()
        };
        let points = Selection::new(&solves, &filter, today).points();
        let numbers: Vec<usize> = points.iter().map(|point| point.number).collect();
        assert_eq!(numbers, [1, 3]);
    }

    #[test]
    fn the_tag_filter_averages_only_the_tagged_solves() {
        let today = day(2026, 6, 1);
        let mut solves = Vec::new();
        for ms in [1000, 2000, 3000] {
            solves.push(tagged(today, ms, "GAN"));
            solves.push(on(today, 60_000, WcaEvent::Cube3x3));
        }
        let filter = ChartFilter {
            tag: Some("GAN".to_owned()),
            ..ChartFilter::default()
        };
        let visible = Selection::new(&solves, &filter, today).visible();
        assert_eq!(visible.len(), 3);
        assert_eq!(stats::average(&visible), Some(stats::Average::Time(2000)));
    }

    #[test]
    fn comparing_tags_uses_the_same_rules_as_the_stats() {
        let today = day(2026, 6, 1);
        let mut first = History::new();
        let mut second = History::new();
        for ms in [1000, 2000, 3000, 4000, 5000] {
            first.push(tagged(today, ms, "GAN"));
            second.push(tagged(today, ms * 2, "Moyu"));
        }
        let mut dnf = tagged(today, 500, "GAN");
        let after = crate::solve::SolveSnapshot {
            modifier: Modifier::DNF,
            ..dnf.snapshot()
        };
        dnf.edit(after);
        first.push(dnf);
        let sessions = [first, second];
        let filter = ChartFilter {
            all_sessions: true,
            ..ChartFilter::default()
        };

        let gan = TagSide::new(&sessions, 0, &filter, Some("gan"), today);
        let moyu = TagSide::new(&sessions, 0, &filter, Some("moyu"), today);
        let everything = TagSide::new(&sessions, 0, &filter, None, today);

        assert_eq!((gan.count, moyu.count, everything.count), (6, 5, 11));
        // The single and mean skip the DNF, which the ao5 counts as the worst.
        assert_eq!(gan.values[0], Some(1000));
        assert_eq!(gan.values[1], Some(3000));
        assert_eq!(gan.values[2], Some(3000));
        assert_eq!(moyu.values[0], Some(2000));
        assert_eq!(moyu.values[2], Some(6000));
    }

    #[test]
    fn comparing_tags_stays_in_the_session_unless_told_otherwise() {
        let today = day(2026, 6, 1);
        let mut first = History::new();
        let mut second = History::new();
        first.push(tagged(today, 1000, "GAN"));
        second.push(tagged(today, 2000, "GAN"));
        let sessions = [first, second];
        let filter = ChartFilter::default();
        let side = TagSide::new(&sessions, 1, &filter, Some("GAN"), today);
        assert_eq!(side.count, 1);
        assert_eq!(side.values[0], Some(2000));
    }

    #[test]
    fn the_event_filter_keeps_session_numbers() {
        let today = day(2026, 6, 1);
        let solves = [
            on(today, 1000, WcaEvent::Cube3x3),
            on(today, 2000, WcaEvent::Cube2x2),
            on(today, 3000, WcaEvent::Cube3x3),
        ];
        let filter = filter(Some(WcaEvent::Cube3x3), ChartRange::All);
        let points = Selection::new(&solves, &filter, today).points();
        assert_eq!(numbers(&points), [1, 3]);
    }

    #[test]
    fn last_solves_counts_after_the_event_filter() {
        let today = day(2026, 6, 1);
        let solves = [
            on(today, 1000, WcaEvent::Cube3x3),
            on(today, 2000, WcaEvent::Cube3x3),
            on(today, 3000, WcaEvent::Cube2x2),
            on(today, 4000, WcaEvent::Cube3x3),
        ];
        let range = ChartRange::LastSolves(2);
        let filter = filter(Some(WcaEvent::Cube3x3), range);
        let points = Selection::new(&solves, &filter, today).points();
        assert_eq!(numbers(&points), [2, 4]);
    }

    #[test]
    fn date_ranges_include_their_ends_and_skip_unknown_dates() {
        let today = day(2026, 6, 10);
        let solves = [
            on(day(2026, 6, 3), 1000, WcaEvent::Cube3x3),
            on(day(2026, 6, 4), 1000, WcaEvent::Cube3x3),
            solve(1000),
            on(today, 1000, WcaEvent::Cube3x3),
        ];
        let points = |range| Selection::new(&solves, &filter(None, range), today).points();
        assert_eq!(numbers(&points(ChartRange::LastDays(7))), [2, 4]);
        assert_eq!(numbers(&points(ChartRange::LastDays(1))), [4]);
        let dates = ChartRange::Dates {
            from: day(2026, 6, 3),
            to: day(2026, 6, 4),
        };
        assert_eq!(numbers(&points(dates)), [1, 2]);
        assert_eq!(numbers(&points(ChartRange::All)), [1, 2, 3, 4]);
    }

    #[test]
    fn averages_reach_back_past_the_range() {
        let solves: Vec<Solve> = [1000, 2000, 3000, 4000, 5000, 6000]
            .into_iter()
            .map(solve)
            .collect();
        let filter = filter(None, ChartRange::LastSolves(1));
        let points = Selection::new(&solves, &filter, day(2026, 1, 1)).points();
        assert_eq!(points.len(), 1);
        assert_eq!(points[0].averages[0], Some(4000));
        assert_eq!(points[0].averages[1], None);
    }

    #[test]
    fn dnf_averages_leave_a_gap() {
        let mut solves = vec![dnf(), dnf()];
        solves.extend([1000, 2000, 3000].map(solve));
        let points = Selection::new(&solves, &ChartFilter::default(), day(2026, 1, 1)).points();
        assert_eq!(points[4].averages[0], None);
        assert_eq!(points[0].ms, None);
    }

    #[test]
    fn personal_bests_are_strict_and_survive_the_range() {
        let solves = [solve(3000), dnf(), solve(3000), solve(2000), solve(2500)];
        let all = Selection::new(&solves, &ChartFilter::default(), day(2026, 1, 1)).points();
        let marks: Vec<bool> = all.iter().map(|point| point.personal_best).collect();
        assert_eq!(marks, [true, false, false, true, false]);

        // The 2500 is the fastest of the range, but an earlier solve beat it.
        let last = filter(None, ChartRange::LastSolves(1));
        let cut = Selection::new(&solves, &last, day(2026, 1, 1)).points();
        assert!(!cut[0].personal_best);
    }

    #[test]
    fn personal_bests_are_per_event() {
        let today = day(2026, 1, 1);
        let solves = [
            on(today, 2000, WcaEvent::Cube2x2),
            on(today, 9000, WcaEvent::Cube3x3),
        ];
        let filter = filter(Some(WcaEvent::Cube3x3), ChartRange::All);
        assert!(Selection::new(&solves, &filter, today).points()[0].personal_best);
    }

    #[test]
    fn one_session_is_shown_by_default() {
        let today = day(2026, 6, 1);
        let sessions = [
            session(vec![on(today, 1000, WcaEvent::Cube3x3)]),
            session(vec![on(today, 2000, WcaEvent::Cube3x3); 2]),
        ];
        let points = Selection::across(&sessions, 1, &ChartFilter::default(), today).points();
        assert_eq!(numbers(&points), [1, 2]);
        assert!(points.iter().all(|point| point.session == 1));
        assert_eq!(points[1].axis, 2);
    }

    #[test]
    fn all_sessions_merge_by_date() {
        let today = day(2026, 6, 10);
        let sessions = [
            session(vec![
                on(day(2026, 6, 1), 1000, WcaEvent::Cube3x3),
                on(day(2026, 6, 3), 3000, WcaEvent::Cube3x3),
            ]),
            session(vec![
                solve(9000),
                on(day(2026, 6, 2), 2000, WcaEvent::Cube3x3),
            ]),
        ];
        let filter = everything(None, ChartRange::All);
        let points = Selection::across(&sessions, 0, &filter, today).points();
        let times: Vec<Option<u64>> = points.iter().map(|point| point.ms).collect();
        assert_eq!(times, [Some(9000), Some(1000), Some(2000), Some(3000)]);
        let origins: Vec<(usize, usize)> = points
            .iter()
            .map(|point| (point.session, point.number))
            .collect();
        assert_eq!(origins, [(1, 1), (0, 1), (1, 2), (0, 2)]);
        let axis: Vec<usize> = points.iter().map(|point| point.axis).collect();
        assert_eq!(axis, [1, 2, 3, 4]);
    }

    #[test]
    fn merged_averages_stay_inside_their_session() {
        let today = day(2026, 6, 10);
        let older = day(2026, 6, 1);
        let sessions = [
            session(vec![on(older, 1000, WcaEvent::Cube3x3); 4]),
            session(vec![on(today, 9000, WcaEvent::Cube3x3); 5]),
        ];
        let filter = everything(None, ChartRange::All);
        let points = Selection::across(&sessions, 0, &filter, today).points();
        // Four solves of the first session make no ao5, and neither do the
        // first four of the second, though nine solves have gone by.
        assert!(points[..8].iter().all(|point| point.averages[0].is_none()));
        assert_eq!(points[8].averages[0], Some(9000));
    }

    #[test]
    fn merged_personal_bests_span_sessions() {
        let today = day(2026, 6, 10);
        let sessions = [
            session(vec![on(day(2026, 6, 1), 1000, WcaEvent::Cube3x3)]),
            session(vec![on(today, 2000, WcaEvent::Cube3x3)]),
        ];
        let all = Selection::across(&sessions, 0, &everything(None, ChartRange::All), today);
        assert!(!all.points()[1].personal_best);
        let one = Selection::across(&sessions, 1, &ChartFilter::default(), today);
        assert!(one.points()[0].personal_best);
    }

    #[test]
    fn last_solves_counts_across_merged_sessions() {
        let today = day(2026, 6, 10);
        let sessions = [
            session(vec![on(day(2026, 6, 1), 1000, WcaEvent::Cube3x3); 3]),
            session(vec![on(today, 2000, WcaEvent::Cube3x3); 3]),
        ];
        let filter = everything(None, ChartRange::LastSolves(4));
        let points = Selection::across(&sessions, 0, &filter, today).points();
        let axis: Vec<usize> = points.iter().map(|point| point.axis).collect();
        assert_eq!(axis, [3, 4, 5, 6]);
    }

    #[test]
    fn compare_lists_each_session_whatever_the_scope() {
        let today = day(2026, 6, 10);
        let sessions = [
            session(vec![on(today, 1000, WcaEvent::Cube3x3); 3]),
            session(vec![on(today, 2000, WcaEvent::Cube3x3); 2]),
        ];
        let rows = compare(&sessions, &everything(None, ChartRange::All), today);
        let counts: Vec<usize> = rows.iter().map(|row| row.count).collect();
        assert_eq!(counts, [3, 2]);
    }

    #[test]
    fn zooming_keeps_the_anchor_in_place() {
        let zoom = ChartZoom::full(100);
        assert_eq!(zoom.range(100), 0..100);
        assert_eq!(zoom.scaled(100, 0.5, 0.).range(100), 0..50);
        assert_eq!(zoom.scaled(100, 0.5, 1.).range(100), 50..100);
        assert_eq!(zoom.scaled(100, 0.5, 0.5).range(100), 25..75);
    }

    #[test]
    fn zooming_stops_at_both_ends() {
        let tight = ChartZoom::full(100).scaled(100, 0.001, 0.5);
        assert_eq!(tight.range(100).len(), 5);
        let wide = tight.scaled(100, 1000., 0.5);
        assert!(wide.is_full(100));
        assert_eq!(wide.range(100), 0..100);
        // A series shorter than the limit cannot zoom at all.
        assert!(ChartZoom::full(3).scaled(3, 0.5, 0.5).is_full(3));
    }

    #[test]
    fn small_zoom_steps_add_up() {
        let mut zoom = ChartZoom::full(100);
        for _ in 0..100 {
            zoom = zoom.scaled(100, 0.99, 0.5);
        }
        assert!(zoom.range(100).len() < 40);
    }

    #[test]
    fn zooming_out_stays_inside_the_series() {
        let edge = ChartZoom::full(100).scaled(100, 0.2, 1.);
        assert_eq!(edge.scaled(100, 2., 1.).range(100), 60..100);
        assert_eq!(edge.scaled(100, 2., 0.).range(100), 60..100);
    }

    #[test]
    fn panning_stops_at_the_ends() {
        let zoom = ChartZoom::full(100).scaled(100, 0.5, 0.);
        assert_eq!(zoom.panned(100, 0.5).range(100), 25..75);
        assert_eq!(zoom.panned(100, -1.).range(100), 0..50);
        assert_eq!(zoom.panned(100, 9.).range(100), 50..100);
    }

    #[test]
    fn a_stale_zoom_never_passes_the_series() {
        let zoom = ChartZoom::full(100).scaled(100, 0.2, 1.);
        assert_eq!(zoom.range(90), 80..90);
        assert_eq!(zoom.range(10), 10..10);
    }

    #[test]
    fn the_calendar_counts_days_and_ignores_unknown_dates() {
        let (first, second) = (day(2025, 12, 31), day(2026, 1, 2));
        let calendar = Calendar::new(&[
            on(first, 1000, WcaEvent::Cube3x3),
            on(second, 1000, WcaEvent::Cube3x3),
            on(second, 1000, WcaEvent::Cube3x3),
            solve(1000),
        ]);
        assert_eq!(calendar.count(first), 1);
        assert_eq!(calendar.count(second), 2);
        assert_eq!(calendar.count(day(2026, 1, 1)), 0);
        assert_eq!(calendar.latest(), Some(second));
        assert_eq!(
            calendar.year(2026),
            YearSummary {
                solves: 2,
                active_days: 1,
                busiest: Some((second, 2)),
            }
        );
        assert_eq!(calendar.year(2024), YearSummary::default());
    }

    #[test]
    fn heat_scales_with_the_busiest_day() {
        assert_eq!(heat_level(0, 10), 0);
        assert_eq!(heat_level(0, 0), 0);
        assert_eq!(heat_level(1, 100), 1);
        assert_eq!(heat_level(50, 100), 2);
        assert_eq!(heat_level(100, 100), MAX_HEAT);
        assert_eq!(heat_level(1, 1), MAX_HEAT);
    }

    #[test]
    fn a_year_is_laid_out_from_monday() {
        // 2026 starts and ends on a Thursday.
        let weeks = year_weeks(2026);
        assert_eq!(weeks.len(), 53);
        assert_eq!(weeks[0][..3], [None, None, None]);
        assert_eq!(weeks[0][3], Some(day(2026, 1, 1)));
        assert_eq!(weeks[52][3], Some(day(2026, 12, 31)));
        assert_eq!(weeks[52][4], None);
        assert_eq!(weeks.iter().flatten().flatten().count(), 365);
        assert_eq!(year_weeks(2024).iter().flatten().flatten().count(), 366);
    }

    #[test]
    fn stepping_days_stops_at_today() {
        let today = day(2026, 6, 10);
        assert_eq!(step_day(day(2026, 1, 1), -1, today), day(2025, 12, 31));
        assert_eq!(step_day(day(2026, 6, 9), 7, today), today);
        assert_eq!(step_year(day(2025, 6, 11), 1, today), today);
        assert_eq!(step_year(day(2026, 6, 1), -1, today), day(2025, 6, 1));
        assert_eq!(step_year(day(2024, 2, 29), -1, today), day(2023, 2, 28));
    }

    #[test]
    fn compare_skips_empty_sessions_and_finds_the_fastest() {
        let today = day(2026, 1, 1);
        let sessions = [
            session(vec![on(today, 5000, WcaEvent::Cube3x3); 5]),
            session(vec![on(today, 1000, WcaEvent::Cube2x2)]),
            session(vec![on(today, 4000, WcaEvent::Cube3x3); 2]),
        ];
        let filter = filter(Some(WcaEvent::Cube3x3), ChartRange::All);
        let rows = compare(&sessions, &filter, today);
        let indices: Vec<usize> = rows.iter().map(|row| row.session).collect();
        assert_eq!(indices, [0, 2]);
        assert_eq!(
            rows[0].values,
            [Some(5000), Some(5000), Some(5000), None, None]
        );
        assert_eq!(rows[1].mean(), Some(4000));
        assert_eq!(
            fastest(&rows),
            [Some(4000), Some(4000), Some(5000), None, None]
        );
    }

    #[test]
    fn compare_ignores_dnf_only_averages() {
        let rows = compare(
            &[session(vec![dnf(); 5])],
            &ChartFilter::default(),
            day(2026, 1, 1),
        );
        assert_eq!(rows[0].count, 5);
        assert_eq!(rows[0].values, [None; COMPARE_COLUMNS]);
    }

    #[test]
    fn all_time_bests_span_sessions_but_not_events() {
        let today = day(2026, 1, 1);
        let sessions = [
            session(vec![on(today, 9000, WcaEvent::Cube3x3); 3]),
            session(vec![
                on(today, 8000, WcaEvent::Cube3x3),
                on(today, 1000, WcaEvent::Cube2x2),
                on(today, 8000, WcaEvent::Cube3x3),
                on(today, 8000, WcaEvent::Cube3x3),
            ]),
        ];
        let bests = all_time_bests(&sessions);
        let events: Vec<WcaEvent> = bests.iter().map(|best| best.event).collect();
        assert_eq!(events, [WcaEvent::Cube2x2, WcaEvent::Cube3x3]);
        let cube = bests[1];
        assert_eq!(cube.count, 6);
        let record = Record {
            ms: 8000,
            session: 1,
        };
        assert_eq!(cube.single, Some(record));
        assert_eq!(cube.averages[0], Some(record));
        assert_eq!(cube.averages[1], None);
    }

    #[test]
    fn ties_stay_with_the_earlier_session() {
        let today = day(2026, 1, 1);
        let sessions = [
            session(vec![on(today, 8000, WcaEvent::Cube3x3)]),
            session(vec![on(today, 8000, WcaEvent::Cube3x3)]),
        ];
        assert_eq!(all_time_bests(&sessions)[0].single.unwrap().session, 0);
    }

    #[test]
    fn event_filters_cycle_through_what_exists() {
        let today = day(2026, 1, 1);
        let sessions = [
            session(vec![on(today, 1000, WcaEvent::Cube3x3)]),
            session(vec![on(today, 1000, WcaEvent::Cube2x2)]),
        ];
        let available = events(&sessions);
        assert_eq!(available, [WcaEvent::Cube2x2, WcaEvent::Cube3x3]);
        let first = next_event(None, &available);
        assert_eq!(first, Some(WcaEvent::Cube2x2));
        assert_eq!(next_event(first, &available), Some(WcaEvent::Cube3x3));
        assert_eq!(next_event(Some(WcaEvent::Cube3x3), &available), None);
        // An event whose solves are gone falls back to every event.
        assert_eq!(next_event(Some(WcaEvent::Clock), &available), None);
    }
}
