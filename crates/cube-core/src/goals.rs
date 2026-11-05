//! Practice goals and the progress towards them.
//!
//! Progress is always derived from the solve history and never stored: solves
//! can be edited, deleted, restored or merged in from cube-tui, and a stored
//! count would drift. Only what the user asked for is saved, in its own file,
//! because `times.json` belongs to cube-tui.
//!
//! Sessions have no ids, and cube-tui rewrites the whole history, so a goal
//! limited to one session keeps a [`SessionAnchor`] and finds it the way
//! [`merge`](crate::merge) pairs sessions: by shared solves, then by name.

use std::borrow::Cow;
use std::collections::{HashMap, HashSet};

use chrono::{DateTime, Datelike as _, NaiveDate, TimeDelta, TimeZone};
use serde::{Deserialize, Serialize};

use crate::event::WcaEvent;
use crate::format::parse_time;
use crate::history::History;
use crate::solve::{Modifier, Solve};
use crate::stats::{self, AVERAGE_SIZES, Average};
use crate::validation::validate_duration;

/// The largest number of solves or days a goal may ask for.
pub const MAX_COUNT: u32 = 100_000;

/// The size of a target time that is a single rather than an average.
pub const SINGLE: usize = 1;

/// Every size a target time can have, in the order the form lists them.
pub const TARGET_SIZES: [usize; AVERAGE_SIZES.len() + 1] = {
    let mut sizes = [SINGLE; AVERAGE_SIZES.len() + 1];
    let mut index = 0;
    while index < AVERAGE_SIZES.len() {
        sizes[index + 1] = AVERAGE_SIZES[index];
        index += 1;
    }
    sizes
};

/// How many of a session's oldest solves an anchor remembers. New solves leave
/// the oldest alone, so the goals file is not rewritten after every solve.
const ANCHOR_SOLVES: usize = 16;

/// The stretch of time a volume goal counts solves in.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Period {
    Day,
    Week,
}

/// What a goal asks for.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum GoalKind {
    /// At least `solves` solves in every `period`.
    Volume { solves: u32, period: Period },
    /// A single, or an average of `size`, under `target_ms`.
    TargetTime { target_ms: u64, size: usize },
    /// `days` consecutive days with at least `solves_per_day` solves.
    Streak { days: u32, solves_per_day: u32 },
}

/// Why a goal cannot be used.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum GoalError {
    /// A number of solves or days is not between 1 and [`MAX_COUNT`].
    InvalidCount,
    /// A target time is not a valid duration.
    InvalidTime,
    /// A target time's size is not one of [`TARGET_SIZES`].
    InvalidSize,
}

impl GoalKind {
    /// Checks the numbers of a goal, from the form or from the goals file.
    pub fn validate(self) -> Result<(), GoalError> {
        let count = |count: u32| {
            (1..=MAX_COUNT)
                .contains(&count)
                .then_some(())
                .ok_or(GoalError::InvalidCount)
        };
        match self {
            Self::Volume { solves, .. } => count(solves),
            Self::TargetTime { target_ms, size } => {
                if !TARGET_SIZES.contains(&size) {
                    return Err(GoalError::InvalidSize);
                }
                validate_duration(target_ms, Modifier::None)
                    .then_some(())
                    .ok_or(GoalError::InvalidTime)
            }
            Self::Streak {
                days,
                solves_per_day,
            } => count(days).and(count(solves_per_day)),
        }
    }
}

/// Reads a number of solves or days typed into the form.
pub fn parse_count(text: &str) -> Result<u32, GoalError> {
    text.trim()
        .parse()
        .ok()
        .filter(|count| (1..=MAX_COUNT).contains(count))
        .ok_or(GoalError::InvalidCount)
}

/// Reads a target time typed into the form.
pub fn parse_target(text: &str) -> Result<u64, GoalError> {
    parse_time(text.trim())
        .filter(|&ms| validate_duration(ms, Modifier::None))
        .ok_or(GoalError::InvalidTime)
}

/// One solve as first recorded, which later edits do not change.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
struct SolveKey {
    event: WcaEvent,
    scramble: String,
    solved_at_unix_ms: u64,
    time_ms: u64,
}

impl SolveKey {
    fn of(solve: &Solve) -> Self {
        let (event, scramble, solved_at_unix_ms, time_ms) = solve.origin();
        Self {
            event,
            scramble: scramble.to_owned(),
            solved_at_unix_ms,
            time_ms,
        }
    }

    fn origin(&self) -> (WcaEvent, &str, u64, u64) {
        (
            self.event,
            &self.scramble,
            self.solved_at_unix_ms,
            self.time_ms,
        )
    }
}

/// What a goal remembers of the session it is limited to.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct SessionAnchor {
    #[serde(default, skip_serializing_if = "String::is_empty")]
    name: String,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    solves: Vec<SolveKey>,
    /// Where the session is while the app runs. This is what keeps an empty,
    /// unnamed session attached, since nothing in it could identify it.
    #[serde(skip)]
    index: Option<usize>,
}

impl SessionAnchor {
    /// Anchors a goal to the session at `index`.
    pub fn new(sessions: &[History], index: usize) -> Option<Self> {
        let session = sessions.get(index)?;
        Some(Self {
            name: session.name().to_owned(),
            solves: session
                .solves()
                .iter()
                .take(ANCHOR_SOLVES)
                .map(SolveKey::of)
                .collect(),
            index: Some(index),
        })
    }

    /// Returns the session's name as last seen, which may be empty.
    pub fn name(&self) -> &str {
        &self.name
    }

    fn same_as(&self, other: &Self) -> bool {
        self.name == other.name && self.solves == other.solves
    }

    /// Finds the session from what was saved, and whether it can be trusted
    /// enough for the anchor to follow it.
    ///
    /// Without solves in common a name is a guess, so an anchor found that way
    /// keeps its solves: undoing the session's deletion must still find it.
    fn locate(&self, sessions: &[History]) -> Option<(usize, bool)> {
        let known: HashSet<_> = self.solves.iter().map(SolveKey::origin).collect();
        let shared = sessions
            .iter()
            .enumerate()
            .map(|(index, session)| {
                let solves = session.solves().iter();
                (
                    index,
                    solves.filter(|s| known.contains(&s.origin())).count(),
                )
            })
            .filter(|&(_, shared)| shared > 0)
            // The first of equals, so the answer does not depend on the order
            // `max_by_key` happens to break ties in.
            .max_by_key(|&(index, shared)| (shared, std::cmp::Reverse(index)));
        if let Some((index, _)) = shared {
            return Some((index, true));
        }
        let index = if !self.name.is_empty() {
            sessions.iter().position(|s| s.name() == self.name)
        } else if self.solves.is_empty() {
            sessions
                .iter()
                .position(|s| s.name().is_empty() && s.is_empty())
        } else {
            // A session with solves and no name is gone, and every other
            // unnamed session would match by name.
            None
        };
        index.map(|index| (index, self.solves.is_empty()))
    }

    /// Returns the session at the moment, and whether the anchor follows it.
    fn find(&self, sessions: &[History]) -> Option<(usize, bool)> {
        self.index
            .filter(|&index| index < sessions.len())
            .map(|index| (index, true))
            .or_else(|| self.locate(sessions))
    }
}

/// Something to practice towards, for one event.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Goal {
    pub event: WcaEvent,
    pub kind: GoalKind,
    /// The session the goal is limited to; `None` counts every session.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub session: Option<SessionAnchor>,
}

/// How far along a goal is.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Measure {
    /// Solves in the current period, or days in the current streak.
    Count { done: u32, target: u32 },
    /// The best result so far, against the time to beat.
    Time {
        best: Option<Average>,
        target_ms: u64,
    },
}

/// A goal's progress, derived from the history.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct GoalProgress {
    /// The session the goal is limited to, if any and if it still exists.
    pub session: Option<usize>,
    /// Whether the goal's session is gone. The goal waits for it to come back.
    pub paused: bool,
    pub measure: Measure,
}

impl GoalProgress {
    /// Returns whether the goal is met. A target time must be beaten, not
    /// matched: sub-20 means under 20 seconds.
    pub const fn reached(&self) -> bool {
        if self.paused {
            return false;
        }
        match self.measure {
            Measure::Count { done, target } => done >= target,
            Measure::Time {
                best: Some(Average::Time(ms)),
                target_ms,
            } => ms < target_ms,
            Measure::Time { .. } => false,
        }
    }

    /// Returns how far along the goal is, from 0 to 100.
    pub fn percent(&self) -> f32 {
        if self.reached() {
            return 100.;
        }
        let share = match self.measure {
            Measure::Count { done, target } => f64::from(done) / f64::from(target.max(1)),
            Measure::Time {
                best: Some(Average::Time(ms)),
                target_ms,
            } => target_ms as f64 / ms.max(1) as f64,
            Measure::Time { .. } => 0.,
        };
        (share * 100.).clamp(0., 100.) as f32
    }
}

/// Returns the local day of a solve; old saves may not have a date.
fn solve_day<Tz: TimeZone>(solve: &Solve, zone: &Tz) -> Option<NaiveDate> {
    let unix_ms = i64::try_from(solve.solved_at_unix_ms()).ok()?;
    if unix_ms == 0 {
        return None;
    }
    let moment = DateTime::from_timestamp_millis(unix_ms)?;
    Some(moment.with_timezone(zone).date_naive())
}

/// Weeks start on Monday, as ISO 8601 has them.
fn week_start(day: NaiveDate) -> NaiveDate {
    day - TimeDelta::days(i64::from(day.weekday().num_days_from_monday()))
}

/// Counts the solves of each local day. A DNF is an attempt like any other:
/// volume is about practice, not results.
fn solves_by_day<'a, Tz: TimeZone>(
    solves: impl Iterator<Item = &'a Solve>,
    zone: &Tz,
) -> HashMap<NaiveDate, u32> {
    let mut days = HashMap::new();
    for day in solves.filter_map(|solve| solve_day(solve, zone)) {
        *days.entry(day).or_default() += 1;
    }
    days
}

/// Counts the consecutive days reaching `per_day`, up to today. Today only
/// adds to a streak: it can still be met, so it does not break one yet.
fn current_streak(days: &HashMap<NaiveDate, u32>, today: NaiveDate, per_day: u32) -> u32 {
    let met = |day: NaiveDate| days.get(&day).is_some_and(|&count| count >= per_day);
    let mut day = if met(today) {
        Some(today)
    } else {
        today.pred_opt()
    };
    let mut streak = 0;
    while let Some(current) = day.filter(|&current| met(current)) {
        streak += 1;
        day = current.pred_opt();
    }
    streak
}

/// The best single or average of `solves`, which belong to one session.
fn best_result(solves: &[Solve], size: usize) -> Option<Average> {
    if size == SINGLE {
        return solves
            .iter()
            .map(|solve| solve.effective_ms().map_or(Average::Dnf, Average::Time))
            .min();
    }
    stats::rolling(solves, size)
        .into_iter()
        .map(|(_, average)| average)
        .min()
}

impl Goal {
    /// Returns the solves of the goal's event, session by session. Averages
    /// stay inside a session, as they do everywhere else.
    fn solves<'a>(&self, sessions: &'a [History], limit: Option<usize>) -> Vec<Cow<'a, [Solve]>> {
        let event = self.event;
        let chosen: Vec<&History> = match (&self.session, limit) {
            (None, _) => sessions.iter().collect(),
            (Some(_), Some(index)) => sessions.get(index).into_iter().collect(),
            (Some(_), None) => Vec::new(),
        };
        chosen
            .into_iter()
            .map(|session| {
                let solves = session.solves();
                if solves.iter().all(|solve| solve.event() == event) {
                    Cow::Borrowed(solves)
                } else {
                    let same = solves.iter().filter(|solve| solve.event() == event);
                    Cow::Owned(same.cloned().collect())
                }
            })
            .collect()
    }

    /// Works out the goal's progress at `now`, whose time zone decides where
    /// days and weeks begin.
    pub fn progress<Tz: TimeZone>(&self, sessions: &[History], now: &DateTime<Tz>) -> GoalProgress {
        let session = self
            .session
            .as_ref()
            .and_then(|anchor| anchor.find(sessions))
            .map(|(index, _)| index);
        let solves = self.solves(sessions, session);
        let zone = now.timezone();
        let today = now.date_naive();
        let by_day = || solves_by_day(solves.iter().flat_map(|solves| solves.iter()), &zone);
        let measure = match self.kind {
            GoalKind::Volume {
                solves: target,
                period,
            } => {
                let days = by_day();
                let done = days.iter().filter(|&(&day, _)| match period {
                    Period::Day => day == today,
                    Period::Week => week_start(day) == week_start(today),
                });
                Measure::Count {
                    done: done.map(|(_, &count)| count).sum(),
                    target,
                }
            }
            GoalKind::TargetTime { target_ms, size } => Measure::Time {
                best: solves
                    .iter()
                    .filter_map(|solves| best_result(solves, size))
                    .min(),
                target_ms,
            },
            GoalKind::Streak {
                days: target,
                solves_per_day,
            } => Measure::Count {
                done: current_streak(&by_day(), today, solves_per_day),
                target,
            },
        };
        GoalProgress {
            session,
            paused: self.session.is_some() && session.is_none(),
            measure,
        }
    }

    /// Returns whether the timer on `event` and the session at `session` is
    /// working towards this goal.
    pub fn is_active_on(&self, progress: &GoalProgress, event: WcaEvent, session: usize) -> bool {
        self.event == event
            && !progress.paused
            && progress.session.is_none_or(|index| index == session)
    }
}

/// Works out every goal's progress at `now`.
pub fn progress_of<Tz: TimeZone>(
    goals: &[Goal],
    sessions: &[History],
    now: &DateTime<Tz>,
) -> Vec<GoalProgress> {
    goals
        .iter()
        .map(|goal| goal.progress(sessions, now))
        .collect()
}

/// Returns the goals reached in `after` that were not in `before`, for the same
/// list of goals.
pub fn newly_reached(before: &[GoalProgress], after: &[GoalProgress]) -> Vec<usize> {
    if before.len() != after.len() {
        return Vec::new();
    }
    let pairs = before.iter().zip(after).enumerate();
    pairs
        .filter(|(_, (before, after))| after.reached() && !before.reached())
        .map(|(index, _)| index)
        .collect()
}

/// Moves the selection in a list of `count` goals, starting from the top.
pub fn step_selection(selected: Option<usize>, count: usize, forward: bool) -> Option<usize> {
    let last = count.checked_sub(1)?;
    Some(match selected.map(|index| index.min(last)) {
        Some(index) if forward => (index + 1).min(last),
        Some(index) => index.saturating_sub(1),
        None => 0,
    })
}

/// Lets every anchor catch up with its session, so a rename or new solves are
/// remembered. Returns whether any changed and the goals need saving.
pub fn follow_sessions(goals: &mut [Goal], sessions: &[History]) -> bool {
    let mut changed = false;
    for anchor in goals.iter_mut().filter_map(|goal| goal.session.as_mut()) {
        match anchor.find(sessions) {
            Some((index, true)) => {
                if let Some(fresh) = SessionAnchor::new(sessions, index) {
                    changed |= !anchor.same_as(&fresh);
                    *anchor = fresh;
                }
            }
            Some((_, false)) | None => anchor.index = None,
        }
    }
    changed
}

/// Keeps the goals on their sessions after the one at `index` was deleted. A
/// goal on that session keeps its anchor, so undoing the deletion restores it.
pub fn session_removed(goals: &mut [Goal], index: usize) {
    for anchor in goals.iter_mut().filter_map(|goal| goal.session.as_mut()) {
        anchor.index = match anchor.index {
            Some(position) if position == index => None,
            Some(position) if position > index => Some(position - 1),
            position => position,
        };
    }
}

/// Keeps the goals on their sessions after one was inserted at `index`.
pub fn session_inserted(goals: &mut [Goal], index: usize) {
    for anchor in goals.iter_mut().filter_map(|goal| goal.session.as_mut()) {
        if let Some(position) = &mut anchor.index
            && *position >= index
        {
            *position += 1;
        }
    }
}

/// Forgets where every session was, after the list was replaced.
pub fn sessions_replaced(goals: &mut [Goal]) {
    for anchor in goals.iter_mut().filter_map(|goal| goal.session.as_mut()) {
        anchor.index = None;
    }
}

#[cfg(test)]
mod tests {
    use chrono::{FixedOffset, Utc};

    use super::*;
    use crate::merge::merge;

    const HOUR: u64 = 3_600_000;
    const DAY: u64 = 24 * HOUR;
    /// 2026-11-02 00:00 UTC, a Monday.
    const MONDAY: u64 = 1_793_577_600_000;

    const DAILY: GoalKind = GoalKind::Volume {
        solves: 2,
        period: Period::Day,
    };
    const WEEKLY: GoalKind = GoalKind::Volume {
        solves: 2,
        period: Period::Week,
    };

    fn solve_on(event: WcaEvent, ms: u64, at: u64, modifier: Modifier) -> Solve {
        Solve::new_with_meta(
            ms,
            event,
            format!("R{ms}-{at}"),
            at,
            modifier,
            String::new(),
        )
    }

    fn solve_at(ms: u64, at: u64) -> Solve {
        solve_on(WcaEvent::Cube3x3, ms, at, Modifier::None)
    }

    fn session(name: &str, solves: Vec<Solve>) -> History {
        let mut history = History::new();
        history.set_name(name.to_owned());
        for solve in solves {
            history.push(solve);
        }
        history
    }

    /// A session of `times` solved on [`MONDAY`], a second apart.
    fn times(name: &str, times: &[u64]) -> History {
        let solves = times.iter().enumerate();
        let solves = solves.map(|(index, &ms)| solve_at(ms, MONDAY + index as u64 * 1000));
        session(name, solves.collect())
    }

    fn goal(kind: GoalKind) -> Goal {
        Goal {
            event: WcaEvent::Cube3x3,
            kind,
            session: None,
        }
    }

    fn limited(kind: GoalKind, sessions: &[History], index: usize) -> Goal {
        Goal {
            session: SessionAnchor::new(sessions, index),
            ..goal(kind)
        }
    }

    fn utc(at: u64) -> DateTime<Utc> {
        DateTime::from_timestamp_millis(i64::try_from(at).unwrap()).unwrap()
    }

    fn zoned(at: u64, hours: i32) -> DateTime<FixedOffset> {
        utc(at).with_timezone(&FixedOffset::east_opt(hours * 3600).unwrap())
    }

    fn done(progress: GoalProgress) -> u32 {
        match progress.measure {
            Measure::Count { done, .. } => done,
            Measure::Time { .. } => panic!("expected a count"),
        }
    }

    fn best(progress: GoalProgress) -> Option<Average> {
        match progress.measure {
            Measure::Time { best, .. } => best,
            Measure::Count { .. } => panic!("expected a time"),
        }
    }

    /// As if the goals file had just been read at startup.
    fn reloaded(goal: &Goal) -> Goal {
        serde_json::from_str(&serde_json::to_string(goal).unwrap()).unwrap()
    }

    #[test]
    fn the_test_monday_is_a_monday() {
        let day = utc(MONDAY).date_naive();
        assert_eq!(day.weekday(), chrono::Weekday::Mon);
        assert_eq!(week_start(day), day);
        assert_eq!(MONDAY % DAY, 0);
    }

    #[test]
    fn an_empty_history_has_no_progress() {
        let now = utc(MONDAY);
        let sessions = [History::new()];
        assert_eq!(done(goal(DAILY).progress(&sessions, &now)), 0);
        assert_eq!(done(goal(DAILY).progress(&[], &now)), 0);
        let target = GoalKind::TargetTime {
            target_ms: 20_000,
            size: 12,
        };
        let progress = goal(target).progress(&sessions, &now);
        assert_eq!(best(progress), None);
        assert!(!progress.reached());
        assert!(progress.percent().abs() < f32::EPSILON);
    }

    #[test]
    fn a_day_ends_at_local_midnight() {
        // 23:30 and 00:30 UTC are the same day two hours east of Greenwich,
        // and neighbouring days in UTC.
        let late = solve_at(10_000, MONDAY - HOUR / 2);
        let early = solve_at(11_000, MONDAY + HOUR / 2);
        let sessions = [session("", vec![late, early])];
        let daily = goal(DAILY);
        assert_eq!(done(daily.progress(&sessions, &utc(MONDAY + HOUR))), 1);
        assert_eq!(done(daily.progress(&sessions, &zoned(MONDAY + HOUR, 2))), 2);
    }

    #[test]
    fn the_last_millisecond_of_a_day_counts_for_that_day() {
        let sessions = [session("", vec![solve_at(10_000, MONDAY + DAY - 1)])];
        let daily = goal(DAILY);
        assert_eq!(done(daily.progress(&sessions, &utc(MONDAY))), 1);
        assert_eq!(done(daily.progress(&sessions, &utc(MONDAY + DAY))), 0);
    }

    #[test]
    fn a_week_runs_from_monday_to_sunday() {
        let sunday = solve_at(10_000, MONDAY - 1);
        let monday = solve_at(11_000, MONDAY);
        let next_sunday = solve_at(12_000, MONDAY + 7 * DAY - 1);
        let sessions = [session("", vec![sunday, monday, next_sunday])];
        let weekly = goal(WEEKLY);
        assert_eq!(done(weekly.progress(&sessions, &utc(MONDAY + 3 * DAY))), 2);
        assert_eq!(done(weekly.progress(&sessions, &utc(MONDAY - DAY))), 1);
        assert_eq!(done(weekly.progress(&sessions, &utc(MONDAY + 7 * DAY))), 0);
    }

    #[test]
    fn a_week_boundary_follows_the_local_zone() {
        // Sunday 23:30 UTC is already Monday two hours east.
        let sessions = [session("", vec![solve_at(10_000, MONDAY - HOUR / 2)])];
        let weekly = goal(WEEKLY);
        assert_eq!(done(weekly.progress(&sessions, &utc(MONDAY + DAY))), 0);
        assert_eq!(done(weekly.progress(&sessions, &zoned(MONDAY + DAY, 2))), 1);
    }

    #[test]
    fn volume_counts_dnfs_but_not_other_events_or_undated_solves() {
        let solves = vec![
            solve_on(WcaEvent::Cube3x3, 0, MONDAY, Modifier::DNF),
            solve_at(10_000, MONDAY + 1),
            solve_on(WcaEvent::Cube2x2, 3_000, MONDAY + 2, Modifier::None),
            solve_at(11_000, 0),
        ];
        let sessions = [session("", solves)];
        let progress = goal(DAILY).progress(&sessions, &utc(MONDAY));
        assert_eq!(done(progress), 2);
        assert!(progress.reached());
    }

    #[test]
    fn volume_adds_up_every_session_unless_limited() {
        let sessions = [times("a", &[10_000]), times("b", &[11_000, 12_000])];
        let now = utc(MONDAY);
        assert_eq!(done(goal(DAILY).progress(&sessions, &now)), 3);
        let limited = limited(DAILY, &sessions, 1);
        assert_eq!(done(limited.progress(&sessions, &now)), 2);
    }

    #[test]
    fn a_streak_counts_back_from_today() {
        const STREAK: GoalKind = GoalKind::Streak {
            days: 3,
            solves_per_day: 2,
        };
        let on = |day: u64| {
            vec![
                solve_at(10_000, MONDAY + day * DAY),
                solve_at(11_000, MONDAY + day * DAY + 1),
            ]
        };
        // Monday is met, Tuesday falls short, then three days are met.
        let mut solves = on(0);
        solves.push(solve_at(12_000, MONDAY + DAY));
        solves.extend([2, 3, 4].into_iter().flat_map(on));
        let sessions = [session("", solves)];
        let streak = goal(STREAK);

        let progress = streak.progress(&sessions, &utc(MONDAY + 4 * DAY));
        assert_eq!(done(progress), 3);
        assert!(progress.reached());
        // Unmet so far today: yesterday's streak is still alive.
        assert_eq!(done(streak.progress(&sessions, &utc(MONDAY + 5 * DAY))), 3);
        // A whole day missed ends it.
        assert_eq!(done(streak.progress(&sessions, &utc(MONDAY + 6 * DAY))), 0);
    }

    #[test]
    fn a_streak_day_follows_the_local_zone() {
        const STREAK: GoalKind = GoalKind::Streak {
            days: 2,
            solves_per_day: 1,
        };
        // Both on Monday in UTC, on two days twelve hours west.
        let solves = vec![
            solve_at(10_000, MONDAY + HOUR),
            solve_at(11_000, MONDAY + 13 * HOUR),
        ];
        let sessions = [session("", solves)];
        let now = MONDAY + 14 * HOUR;
        assert_eq!(done(goal(STREAK).progress(&sessions, &utc(now))), 1);
        assert_eq!(done(goal(STREAK).progress(&sessions, &zoned(now, -12))), 2);
    }

    #[test]
    fn a_target_time_must_be_beaten() {
        let single = |target_ms| {
            goal(GoalKind::TargetTime {
                target_ms,
                size: SINGLE,
            })
        };
        let sessions = [times("", &[20_000, 25_000])];
        let now = utc(MONDAY);
        assert!(!single(20_000).progress(&sessions, &now).reached());
        assert!(single(20_001).progress(&sessions, &now).reached());
    }

    #[test]
    fn a_plus_two_counts_against_a_target_single() {
        let solves = vec![solve_on(
            WcaEvent::Cube3x3,
            19_000,
            MONDAY,
            Modifier::PlusTwo,
        )];
        let target = goal(GoalKind::TargetTime {
            target_ms: 20_000,
            size: SINGLE,
        });
        let progress = target.progress(&[session("", solves)], &utc(MONDAY));
        assert_eq!(best(progress), Some(Average::Time(21_000)));
    }

    #[test]
    fn dnfs_follow_the_rules_of_averages() {
        let dnf = |at| solve_on(WcaEvent::Cube3x3, 0, at, Modifier::DNF);
        let timed = |list: &[u64]| -> Vec<Solve> {
            list.iter().map(|&ms| solve_at(ms, MONDAY + ms)).collect()
        };
        let ao5 = goal(GoalKind::TargetTime {
            target_ms: 20_000,
            size: 5,
        });
        let now = utc(MONDAY);

        // One DNF is trimmed as the worst attempt.
        let mut solves = timed(&[10_000, 11_000, 12_000, 13_000]);
        solves.push(dnf(MONDAY + 1));
        let progress = ao5.progress(&[session("", solves)], &now);
        assert_eq!(best(progress), Some(Average::Time(12_000)));
        assert!(progress.reached());

        // Two make it a DNF, which reaches nothing.
        let mut solves = timed(&[10_000, 11_000, 12_000]);
        solves.extend([dnf(MONDAY + 1), dnf(MONDAY + 2)]);
        let progress = ao5.progress(&[session("", solves)], &now);
        assert_eq!(best(progress), Some(Average::Dnf));
        assert!(!progress.reached());
        assert!(progress.percent().abs() < f32::EPSILON);
    }

    #[test]
    fn a_single_dnf_is_no_time() {
        let solves = vec![solve_on(WcaEvent::Cube3x3, 0, MONDAY, Modifier::DNF)];
        let target = goal(GoalKind::TargetTime {
            target_ms: 20_000,
            size: SINGLE,
        });
        let progress = target.progress(&[session("", solves)], &utc(MONDAY));
        assert_eq!(best(progress), Some(Average::Dnf));
        assert!(!progress.reached());
    }

    #[test]
    fn averages_stay_inside_a_session_and_an_event() {
        let ao5 = goal(GoalKind::TargetTime {
            target_ms: 20_000,
            size: 5,
        });
        let now = utc(MONDAY);
        // Five solves, but no session has five of them.
        let split = [times("a", &[10_000; 3]), times("b", &[10_000; 2])];
        assert_eq!(best(ao5.progress(&split, &now)), None);

        // Another event's solve between them does not break the window.
        let mut solves: Vec<Solve> = (0..5).map(|n| solve_at(10_000, MONDAY + n)).collect();
        let other = solve_on(WcaEvent::Cube2x2, 1_000, MONDAY + 2, Modifier::None);
        solves.insert(2, other);
        let mixed = [session("", solves)];
        assert_eq!(
            best(ao5.progress(&mixed, &now)),
            Some(Average::Time(10_000))
        );
    }

    #[test]
    fn progress_follows_edits_deletions_and_restores() {
        let mut sessions = [times("", &[25_000, 26_000])];
        let now = utc(MONDAY);
        let single = goal(GoalKind::TargetTime {
            target_ms: 20_000,
            size: SINGLE,
        });
        let daily = goal(DAILY);
        assert!(!single.progress(&sessions, &now).reached());

        let mut edited = sessions[0].solves()[0].snapshot();
        edited.time_ms = 19_000;
        assert!(sessions[0].edit(0, edited));
        assert!(single.progress(&sessions, &now).reached());

        let removed = sessions[0].remove(0).unwrap();
        assert!(!single.progress(&sessions, &now).reached());
        assert_eq!(done(daily.progress(&sessions, &now)), 1);

        sessions[0].insert(0, removed);
        assert!(single.progress(&sessions, &now).reached());
        assert_eq!(done(daily.progress(&sessions, &now)), 2);
    }

    #[test]
    fn an_edited_date_moves_a_solve_to_its_new_day() {
        let mut sessions = [times("", &[10_000])];
        let daily = goal(DAILY);
        let mut edited = sessions[0].solves()[0].snapshot();
        edited.solved_at_unix_ms = MONDAY + DAY;
        assert!(sessions[0].edit(0, edited));
        assert_eq!(done(daily.progress(&sessions, &utc(MONDAY))), 0);
        assert_eq!(done(daily.progress(&sessions, &utc(MONDAY + DAY))), 1);
    }

    #[test]
    fn progress_includes_solves_merged_from_cube_tui() {
        let base = vec![times("a", &[10_000])];
        let mut ours = base.clone();
        let mut theirs = base.clone();
        theirs[0].push(solve_at(11_000, MONDAY + 5_000));
        let goal = limited(DAILY, &ours, 0);
        assert_eq!(done(goal.progress(&ours, &utc(MONDAY))), 1);
        merge(&base, &mut ours, &theirs);
        assert_eq!(done(goal.progress(&ours, &utc(MONDAY))), 2);
    }

    #[test]
    fn a_limited_goal_counts_only_its_session() {
        let sessions = [times("a", &[10_000, 11_000]), times("b", &[12_000])];
        let progress = limited(DAILY, &sessions, 1).progress(&sessions, &utc(MONDAY));
        assert_eq!(progress.session, Some(1));
        assert_eq!(done(progress), 1);
    }

    #[test]
    fn a_saved_anchor_finds_a_renamed_session_by_its_solves() {
        let mut sessions = vec![times("a", &[10_000]), times("b", &[11_000])];
        let goal = reloaded(&limited(DAILY, &sessions, 1));
        sessions[1].set_name("renamed elsewhere".to_owned());
        assert_eq!(goal.progress(&sessions, &utc(MONDAY)).session, Some(1));
    }

    #[test]
    fn a_saved_anchor_prefers_shared_solves_over_the_name() {
        let mut sessions = vec![times("a", &[10_000]), times("b", &[11_000])];
        let goal = reloaded(&limited(DAILY, &sessions, 1));
        sessions[0].set_name("b".to_owned());
        sessions[1].set_name("a".to_owned());
        assert_eq!(goal.progress(&sessions, &utc(MONDAY)).session, Some(1));
    }

    #[test]
    fn a_saved_anchor_falls_back_on_the_name() {
        let mut sessions = vec![times("a", &[10_000]), History::new()];
        sessions[1].set_name("b".to_owned());
        let goal = reloaded(&limited(DAILY, &sessions, 1));
        assert_eq!(goal.progress(&sessions, &utc(MONDAY)).session, Some(1));
    }

    #[test]
    fn a_goal_survives_a_cube_tui_rename_and_a_deletion_elsewhere() {
        let base = vec![times("a", &[10_000]), times("b", &[11_000])];
        let mut ours = base.clone();
        let mut theirs = base.clone();
        theirs[1].set_name("renamed in cube-tui".to_owned());
        let mut goals = [limited(DAILY, &ours, 1)];
        merge(&base, &mut ours, &theirs);
        assert_eq!(ours[1].name(), "renamed in cube-tui");

        assert!(follow_sessions(&mut goals, &ours));
        let anchor = goals[0].session.as_ref().unwrap();
        assert_eq!(anchor.name(), "renamed in cube-tui");

        ours.remove(0);
        session_removed(&mut goals, 0);
        let progress = goals[0].progress(&ours, &utc(MONDAY));
        assert_eq!(progress.session, Some(0));
        assert_eq!(done(progress), 1);
    }

    #[test]
    fn deleting_another_session_leaves_a_goal_on_its_empty_session() {
        let mut sessions = vec![times("", &[10_000]), History::new(), History::new()];
        let mut goals = [limited(DAILY, &sessions, 2)];
        sessions.remove(0);
        session_removed(&mut goals, 0);
        assert_eq!(goals[0].progress(&sessions, &utc(MONDAY)).session, Some(1));
    }

    #[test]
    fn a_goal_pauses_with_its_session_and_returns_when_undone() {
        let mut sessions = vec![times("a", &[10_000]), times("b", &[11_000, 12_000])];
        let mut goals = [limited(DAILY, &sessions, 1)];
        let now = utc(MONDAY);

        let deleted = sessions.remove(1);
        session_removed(&mut goals, 1);
        assert!(!follow_sessions(&mut goals, &sessions));
        let paused = goals[0].progress(&sessions, &now);
        assert!(paused.paused);
        assert_eq!(paused.session, None);
        assert_eq!(done(paused), 0);
        assert!(!paused.reached());

        sessions.insert(1, deleted);
        session_inserted(&mut goals, 1);
        follow_sessions(&mut goals, &sessions);
        let back = goals[0].progress(&sessions, &now);
        assert!(!back.paused);
        assert_eq!(back.session, Some(1));
        assert_eq!(done(back), 2);
    }

    #[test]
    fn a_deleted_unnamed_session_does_not_become_another_unnamed_one() {
        let mut sessions = vec![times("", &[10_000]), times("", &[11_000]), History::new()];
        let mut goals = [limited(DAILY, &sessions, 1)];
        sessions.remove(1);
        session_removed(&mut goals, 1);
        follow_sessions(&mut goals, &sessions);
        assert!(goals[0].progress(&sessions, &utc(MONDAY)).paused);
    }

    #[test]
    fn a_same_named_session_does_not_take_over_the_anchor() {
        let mut sessions = vec![times("oh", &[10_000]), times("oh", &[11_000])];
        let mut goals = [limited(DAILY, &sessions, 1)];
        let deleted = sessions.remove(1);
        session_removed(&mut goals, 1);

        // Found by name for now, but the anchor keeps its own solves.
        assert_eq!(goals[0].progress(&sessions, &utc(MONDAY)).session, Some(0));
        assert!(!follow_sessions(&mut goals, &sessions));

        sessions.insert(1, deleted);
        session_inserted(&mut goals, 1);
        assert_eq!(goals[0].progress(&sessions, &utc(MONDAY)).session, Some(1));
    }

    #[test]
    fn an_empty_unnamed_session_stays_attached_through_its_first_solve() {
        let mut sessions = vec![times("", &[10_000]), History::new()];
        let mut goals = [limited(DAILY, &sessions, 1)];
        sessions[1].push(solve_at(11_000, MONDAY + 1));
        assert!(follow_sessions(&mut goals, &sessions));

        let saved = reloaded(&goals[0]);
        let progress = saved.progress(&sessions, &utc(MONDAY));
        assert_eq!(progress.session, Some(1));
        assert_eq!(done(progress), 1);
    }

    #[test]
    fn losing_and_restoring_an_only_solve_keeps_the_session() {
        let mut sessions = vec![times("", &[10_000]), times("", &[11_000])];
        let mut goals = [limited(DAILY, &sessions, 1)];
        let solve = sessions[1].remove(0).unwrap();
        follow_sessions(&mut goals, &sessions);
        assert_eq!(goals[0].progress(&sessions, &utc(MONDAY)).session, Some(1));

        sessions[1].insert(0, solve);
        follow_sessions(&mut goals, &sessions);
        assert_eq!(done(goals[0].progress(&sessions, &utc(MONDAY))), 1);
    }

    #[test]
    fn renaming_an_empty_session_keeps_the_goal_on_it() {
        let mut sessions = vec![History::new()];
        let mut goals = [limited(DAILY, &sessions, 0)];
        sessions[0].set_name("named".to_owned());
        assert!(follow_sessions(&mut goals, &sessions));
        assert_eq!(goals[0].session.as_ref().unwrap().name(), "named");
    }

    #[test]
    fn replacing_the_sessions_finds_them_again_by_what_was_saved() {
        let sessions = vec![times("a", &[10_000])];
        let mut goals = [limited(DAILY, &sessions, 0)];
        sessions_replaced(&mut goals);
        let moved = vec![History::new(), sessions[0].clone()];
        assert_eq!(goals[0].progress(&moved, &utc(MONDAY)).session, Some(1));
    }

    #[test]
    fn new_solves_past_the_anchored_ones_do_not_need_saving() {
        let mut sessions = vec![times("a", &[10_000; ANCHOR_SOLVES])];
        let mut goals = [limited(DAILY, &sessions, 0)];
        sessions[0].push(solve_at(11_000, MONDAY + DAY));
        assert!(!follow_sessions(&mut goals, &sessions));
        let anchor = goals[0].session.as_ref().unwrap();
        assert_eq!(anchor.solves.len(), ANCHOR_SOLVES);
    }

    #[test]
    fn an_edited_solve_still_identifies_its_session() {
        let mut sessions = vec![History::new(), times("", &[10_000])];
        let saved = reloaded(&limited(DAILY, &sessions, 1));
        let mut edited = sessions[1].solves()[0].snapshot();
        edited.time_ms = 9_000;
        assert!(sessions[1].edit(0, edited));
        assert_eq!(saved.progress(&sessions, &utc(MONDAY)).session, Some(1));
    }

    #[test]
    fn only_goals_of_the_event_and_session_are_active_on_the_timer() {
        let sessions = vec![times("a", &[10_000]), times("b", &[11_000])];
        let now = utc(MONDAY);
        let everywhere = goal(DAILY);
        let progress = everywhere.progress(&sessions, &now);
        assert!(everywhere.is_active_on(&progress, WcaEvent::Cube3x3, 1));
        assert!(!everywhere.is_active_on(&progress, WcaEvent::Cube2x2, 1));

        let one = limited(DAILY, &sessions, 0);
        let progress = one.progress(&sessions, &now);
        assert!(one.is_active_on(&progress, WcaEvent::Cube3x3, 0));
        assert!(!one.is_active_on(&progress, WcaEvent::Cube3x3, 1));

        let paused = GoalProgress {
            session: None,
            paused: true,
            ..progress
        };
        assert!(!one.is_active_on(&paused, WcaEvent::Cube3x3, 0));
    }

    #[test]
    fn only_a_change_to_reached_is_announced() {
        let at = |done| GoalProgress {
            session: None,
            paused: false,
            measure: Measure::Count { done, target: 2 },
        };
        assert_eq!(newly_reached(&[at(1), at(2)], &[at(2), at(2)]), [0]);
        assert_eq!(newly_reached(&[at(2)], &[at(1)]), Vec::<usize>::new());
        assert_eq!(newly_reached(&[], &[at(2)]), Vec::<usize>::new());
    }

    #[test]
    fn percent_stays_in_range() {
        let at = |measure| GoalProgress {
            session: None,
            paused: false,
            measure,
        };
        let count = |done, target| at(Measure::Count { done, target }).percent();
        let time = |ms| {
            at(Measure::Time {
                best: Some(Average::Time(ms)),
                target_ms: 20_000,
            })
            .percent()
        };
        assert!((count(1, 4) - 25.).abs() < 0.01);
        assert!((count(9, 4) - 100.).abs() < 0.01);
        assert!((time(40_000) - 50.).abs() < 0.01);
        assert!((time(10_000) - 100.).abs() < 0.01);
    }

    #[test]
    fn the_selection_stops_at_both_ends_of_the_list() {
        assert_eq!(step_selection(None, 0, true), None);
        assert_eq!(step_selection(None, 3, true), Some(0));
        assert_eq!(step_selection(None, 3, false), Some(0));
        assert_eq!(step_selection(Some(0), 3, true), Some(1));
        assert_eq!(step_selection(Some(2), 3, true), Some(2));
        assert_eq!(step_selection(Some(0), 3, false), Some(0));
        assert_eq!(step_selection(Some(9), 3, false), Some(1));
    }

    #[test]
    fn typed_counts_must_be_positive_whole_numbers() {
        assert_eq!(parse_count(" 50 "), Ok(50));
        assert_eq!(parse_count("100000"), Ok(MAX_COUNT));
        for text in ["", "0", "-1", "1.5", "abc", "100001"] {
            assert_eq!(parse_count(text), Err(GoalError::InvalidCount), "{text}");
        }
    }

    #[test]
    fn typed_targets_use_the_notation_of_times() {
        assert_eq!(parse_target("20"), Ok(20_000));
        assert_eq!(parse_target(" 1:05.5 "), Ok(65_500));
        for text in ["", "0", "abc"] {
            assert_eq!(parse_target(text), Err(GoalError::InvalidTime), "{text}");
        }
    }

    #[test]
    fn a_goal_is_checked_field_by_field() {
        let volume = |solves| GoalKind::Volume {
            solves,
            period: Period::Day,
        };
        let target = |target_ms, size| GoalKind::TargetTime { target_ms, size };
        let streak = |days, solves_per_day| GoalKind::Streak {
            days,
            solves_per_day,
        };
        assert_eq!(volume(1).validate(), Ok(()));
        assert_eq!(volume(0).validate(), Err(GoalError::InvalidCount));
        assert_eq!(target(1, SINGLE).validate(), Ok(()));
        assert_eq!(target(1, 12).validate(), Ok(()));
        assert_eq!(target(0, 12).validate(), Err(GoalError::InvalidTime));
        assert_eq!(target(1, 7).validate(), Err(GoalError::InvalidSize));
        assert_eq!(streak(7, 1).validate(), Ok(()));
        assert_eq!(streak(0, 1).validate(), Err(GoalError::InvalidCount));
        assert_eq!(streak(7, 0).validate(), Err(GoalError::InvalidCount));
    }

    #[test]
    fn target_sizes_are_the_single_and_every_average() {
        assert_eq!(TARGET_SIZES, [1, 3, 5, 12, 50, 100]);
    }
}
