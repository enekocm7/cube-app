//! What a newly recorded solve beat, so the app can celebrate it.
//!
//! Only singles and ao5s count, and only once there is enough history for a
//! best to mean something: the first solves of a session beat each other all
//! the time.

use std::f32::consts::PI;

use crate::event::WcaEvent;
use crate::history::History;
use crate::solve::Solve;
use crate::stats::average_of;

/// The rolling average that can be celebrated.
const AVERAGE_SIZE: usize = 5;

/// How far a best reaches. Later variants are bigger.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub enum BestScope {
    Session,
    AllTime,
}

impl BestScope {
    /// How long the effect on the digits lasts, in milliseconds.
    pub const fn effect_ms(self) -> u64 {
        match self {
            Self::Session => 900,
            Self::AllTime => 1800,
        }
    }

    /// How many times the digits flash during the effect.
    pub const fn flashes(self) -> u32 {
        match self {
            Self::Session => 1,
            Self::AllTime => 3,
        }
    }
}

/// What was beaten. Later variants are bigger within a scope.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub enum BestKind {
    Single,
    Ao5,
}

/// The biggest best set by one solve.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct NewBest {
    pub scope: BestScope,
    pub kind: BestKind,
    pub ms: u64,
}

/// The results to beat in some part of the history.
#[derive(Clone, Copy, Debug, Default)]
struct Bests {
    count: usize,
    single: Option<u64>,
    ao5: Option<u64>,
}

impl Bests {
    /// Adds a run of solves. An average never spans two runs.
    fn add(&mut self, times: &[Option<u64>]) {
        self.count += times.len();
        let single = times.iter().copied().flatten().min();
        self.single = lowest(self.single, single);
        let ao5 = times
            .windows(AVERAGE_SIZE)
            .filter_map(|window| average_of(window.iter().copied())?.ms())
            .min();
        self.ao5 = lowest(self.ao5, ao5);
    }

    /// The biggest result of `candidate` that strictly beats these, once they
    /// hold `min_solves`. There must be something to beat: a first result is
    /// not a best.
    fn beaten_by(&self, candidate: Candidate, min_solves: u32) -> Option<(BestKind, u64)> {
        if self.count < min_solves as usize {
            return None;
        }
        let beats = |new: Option<u64>, old: Option<u64>| {
            new.zip(old)
                .filter(|(new, old)| new < old)
                .map(|(new, _)| new)
        };
        beats(candidate.ao5, self.ao5)
            .map(|ms| (BestKind::Ao5, ms))
            .or_else(|| beats(Some(candidate.single), self.single).map(|ms| (BestKind::Single, ms)))
    }

    /// Whether `candidate` improves on these, whatever the minimum says. A
    /// result that does not cannot be a best of anything larger either.
    fn improved_by(&self, candidate: Candidate) -> bool {
        self.single.is_none_or(|single| candidate.single < single)
            || candidate
                .ao5
                .is_some_and(|ao5| self.ao5.is_none_or(|best| ao5 < best))
    }
}

/// The results the newest solve produced.
#[derive(Clone, Copy, Debug)]
struct Candidate {
    single: u64,
    ao5: Option<u64>,
}

fn lowest(a: Option<u64>, b: Option<u64>) -> Option<u64> {
    a.into_iter().chain(b).min()
}

/// The effective times of the solves of `event`, where `None` is a DNF.
fn times(solves: &[Solve], event: WcaEvent) -> Vec<Option<u64>> {
    solves
        .iter()
        .filter(|solve| solve.event() == event)
        .map(Solve::effective_ms)
        .collect()
}

/// Returns the biggest best set by the newest solve of session `current`.
///
/// Call it for a solve that was just recorded, and for nothing else: every
/// other way into the history, like imports and merges, must not celebrate.
/// All-time bests outrank session bests, and an ao5 outranks a single. A tie is
/// not a best, and neither is a DNF. A scope must already hold `min_solves`
/// solves of the event before a best in it counts.
pub fn new_best(sessions: &[History], current: usize, min_solves: u32) -> Option<NewBest> {
    let (latest, earlier) = sessions.get(current)?.solves().split_last()?;
    let single = latest.effective_ms()?;
    let event = latest.event();

    let mut recent = times(earlier, event);
    let mut session = Bests::default();
    session.add(&recent);

    recent.push(Some(single));
    let ao5 = recent
        .last_chunk::<AVERAGE_SIZE>()
        .and_then(|window| average_of(window.iter().copied())?.ms());
    let candidate = Candidate { single, ao5 };

    // Most solves stop here, sparing a read of the whole history.
    if !session.improved_by(candidate) {
        return None;
    }

    let mut all_time = session;
    for (index, history) in sessions.iter().enumerate() {
        if index != current {
            all_time.add(&times(history.solves(), event));
        }
    }

    let found = |scope, (kind, ms)| NewBest { scope, kind, ms };
    all_time
        .beaten_by(candidate, min_solves)
        .map(|best| found(BestScope::AllTime, best))
        .or_else(|| {
            session
                .beaten_by(candidate, min_solves)
                .map(|best| found(BestScope::Session, best))
        })
}

/// How strongly the digits glow at `progress` through an effect, from 0 to 1.
///
/// It starts at full strength, so the flash lands with the result, and fades
/// to nothing by the end.
pub fn glow(progress: f32, flashes: u32) -> f32 {
    let progress = progress.clamp(0., 1.);
    let wave = (progress * flashes as f32 * PI).cos().powi(2);
    wave * (1. - progress)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::settings::DEFAULT_BEST_MIN_SOLVES as MIN;
    use crate::solve::Modifier;

    const CUBE: WcaEvent = WcaEvent::Cube3x3;

    fn solve(ms: u64) -> Solve {
        Solve::new(ms, CUBE, String::new(), Modifier::None)
    }

    fn dnf(ms: u64) -> Solve {
        Solve::new(ms, CUBE, String::new(), Modifier::DNF)
    }

    /// A session of `count` solves of 20 seconds.
    fn session(count: usize) -> History {
        let mut history = History::new();
        for _ in 0..count {
            history.push(solve(20_000));
        }
        history
    }

    fn after(mut history: History, solves: impl IntoIterator<Item = Solve>) -> Option<NewBest> {
        for solve in solves {
            history.push(solve);
        }
        new_best(&[history], 0, MIN)
    }

    fn best(scope: BestScope, kind: BestKind, ms: u64) -> NewBest {
        NewBest { scope, kind, ms }
    }

    #[test]
    fn a_single_beating_a_long_session_is_an_all_time_best() {
        assert_eq!(
            after(session(100), [solve(10_000)]),
            Some(best(BestScope::AllTime, BestKind::Single, 10_000)),
        );
    }

    #[test]
    fn nothing_counts_below_the_minimum() {
        assert_eq!(after(session(99), [solve(10_000)]), None);
        assert!(after(session(100), [solve(10_000)]).is_some());
    }

    #[test]
    fn the_minimum_is_the_callers_choice() {
        let mut history = session(10);
        history.push(solve(10_000));
        let sessions = [history];
        assert_eq!(new_best(&sessions, 0, 11), None);
        assert_eq!(
            new_best(&sessions, 0, 10),
            Some(best(BestScope::AllTime, BestKind::Single, 10_000)),
        );
    }

    #[test]
    fn a_minimum_of_zero_still_needs_something_to_beat() {
        let mut history = History::new();
        history.push(solve(10_000));
        assert_eq!(new_best(&[history], 0, 0), None);
    }

    #[test]
    fn a_new_session_stays_quiet_about_its_own_bests() {
        let mut old = session(200);
        old.push(solve(5_000));
        let mut fresh = History::new();
        fresh.push(solve(20_000));
        fresh.push(solve(10_000));
        assert_eq!(new_best(&[old, fresh], 1, MIN), None);
    }

    #[test]
    fn an_all_time_best_counts_in_a_new_session() {
        let mut fresh = History::new();
        fresh.push(solve(4_000));
        assert_eq!(
            new_best(&[session(200), fresh], 1, MIN),
            Some(best(BestScope::AllTime, BestKind::Single, 4_000)),
        );
    }

    #[test]
    fn a_long_session_celebrates_a_best_that_is_not_all_time() {
        let mut old = session(10);
        old.push(solve(5_000));
        let mut current = session(100);
        current.push(solve(10_000));
        assert_eq!(
            new_best(&[old, current], 1, MIN),
            Some(best(BestScope::Session, BestKind::Single, 10_000)),
        );
    }

    #[test]
    fn an_all_time_single_outranks_a_session_ao5() {
        let mut old = session(100);
        for _ in 0..5 {
            old.push(solve(15_000));
        }
        let mut current = session(100);
        for _ in 0..4 {
            current.push(solve(18_000));
        }
        // A session ao5 of 18 s, but only the single is all-time.
        current.push(solve(14_000));
        assert_eq!(
            new_best(&[old, current], 1, MIN),
            Some(best(BestScope::AllTime, BestKind::Single, 14_000)),
        );
    }

    #[test]
    fn an_ao5_outranks_a_single_of_the_same_scope() {
        let solves = [15_000, 15_000, 15_000, 14_000, 13_000].map(solve);
        assert_eq!(
            after(session(100), solves),
            Some(best(BestScope::AllTime, BestKind::Ao5, 14_666)),
        );
    }

    #[test]
    fn an_ao5_counts_without_a_new_single() {
        let mut history = session(100);
        history.push(solve(10_000));
        for ms in [15_000, 15_000, 15_000] {
            history.push(solve(ms));
        }
        assert_eq!(
            after(history, [solve(12_000)]),
            Some(best(BestScope::AllTime, BestKind::Ao5, 14_000)),
        );
    }

    #[test]
    fn ties_are_not_bests() {
        assert_eq!(after(session(100), [solve(20_000)]), None);
    }

    #[test]
    fn a_dnf_never_counts() {
        assert_eq!(after(session(100), [dnf(1_000)]), None);
    }

    #[test]
    fn a_plus_two_is_judged_by_its_effective_time() {
        let slower = Solve::new(19_000, CUBE, String::new(), Modifier::PlusTwo);
        assert_eq!(after(session(100), [slower]), None);
        let faster = Solve::new(17_000, CUBE, String::new(), Modifier::PlusTwo);
        assert_eq!(
            after(session(100), [faster]),
            Some(best(BestScope::AllTime, BestKind::Single, 19_000)),
        );
    }

    #[test]
    fn other_events_neither_count_nor_compete() {
        let other = |ms| Solve::new(ms, WcaEvent::Cube2x2, String::new(), Modifier::None);
        assert_eq!(after(session(100), [other(10_000)]), None);

        let mut mixed = session(100);
        mixed.push(other(1_000));
        assert_eq!(
            after(mixed, [solve(10_000)]),
            Some(best(BestScope::AllTime, BestKind::Single, 10_000)),
        );
    }

    #[test]
    fn an_ao5_never_spans_two_sessions() {
        let mut old = session(100);
        for _ in 0..4 {
            old.push(solve(10_000));
        }
        let mut current = History::new();
        current.push(solve(15_000));
        // Slower than the old session's best single, and no window of five.
        assert_eq!(new_best(&[old, current], 1, MIN), None);
    }

    #[test]
    fn nothing_to_report_without_solves() {
        assert_eq!(new_best(&[History::new()], 0, MIN), None);
        assert_eq!(new_best(&[], 0, MIN), None);
    }

    #[test]
    fn all_time_effects_are_stronger() {
        assert!(BestScope::AllTime > BestScope::Session);
        assert!(BestScope::AllTime.effect_ms() > BestScope::Session.effect_ms());
        assert!(BestScope::AllTime.flashes() > BestScope::Session.flashes());
    }

    #[test]
    fn the_glow_starts_full_and_ends_at_nothing() {
        for flashes in [1, 3] {
            assert!((glow(0., flashes) - 1.).abs() < f32::EPSILON);
            assert!(glow(1., flashes).abs() < f32::EPSILON);
            assert!((0..=100).all(|step| (0. ..=1.).contains(&glow(step as f32 / 100., flashes))));
        }
    }
}
