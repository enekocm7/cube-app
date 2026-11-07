//! The timer state machine.
//!
//! Every transition takes the current [`Instant`], so timing logic can be tested
//! without sleeping and the UI stays free of rules.

use std::time::{Duration, Instant};

use crate::solve::Modifier;

/// Extra time allowed after the limit before an inspection becomes a DNF.
const PENALTY_WINDOW: Duration = Duration::from_secs(2);

/// When the first inspection warning plays.
const EIGHT_SECOND_CUE: Duration = Duration::from_secs(8);

/// When the second inspection warning plays.
const TWELVE_SECOND_CUE: Duration = Duration::from_secs(12);

/// How long the timer key must be held by default before a release starts the solve.
pub const DEFAULT_HOLD: Duration = Duration::from_millis(300);

/// The most puzzles one attempt may be split into.
pub const MAX_PUZZLES: usize = 16;

/// The ends of the puzzles of a relay, marked while its attempt runs.
///
/// A relay is one attempt over several puzzles. The timer key marks the end of
/// each puzzle but the last, which ends the attempt as it does any other. Marks
/// are kept as times since the attempt started, so the splits always add up to
/// the total.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Splits {
    puzzles: u8,
    marked: u8,
    at_ms: [u64; MAX_PUZZLES],
}

impl Splits {
    /// An attempt of a single puzzle, which has nothing to mark.
    pub const NONE: Self = Self::new(1);

    /// Splits for an attempt of `puzzles` puzzles, from 1 to [`MAX_PUZZLES`].
    pub const fn new(puzzles: usize) -> Self {
        let puzzles = if puzzles < 1 {
            1
        } else if puzzles > MAX_PUZZLES {
            MAX_PUZZLES
        } else {
            puzzles
        };
        Self {
            puzzles: puzzles as u8,
            marked: 0,
            at_ms: [0; MAX_PUZZLES],
        }
    }

    /// Returns how many puzzles the attempt has.
    pub const fn puzzles(&self) -> usize {
        self.puzzles as usize
    }

    /// Returns how many puzzles have been marked finished so far.
    pub const fn marked(&self) -> usize {
        self.marked as usize
    }

    /// Returns whether the next press marks the end of a puzzle rather than
    /// the end of the attempt.
    pub const fn pending(&self) -> bool {
        self.marked + 1 < self.puzzles
    }

    /// Returns the puzzle being solved, counting from zero.
    pub const fn current(&self) -> usize {
        self.marked as usize
    }

    fn mark(&mut self, at_ms: u64) {
        let previous = match self.marked {
            0 => 0,
            marked => self.at_ms[marked as usize - 1],
        };
        self.at_ms[self.marked as usize] = at_ms.max(previous);
        self.marked += 1;
    }

    /// Returns the time spent on each puzzle, given the total of the attempt.
    ///
    /// The result always has one entry per puzzle and adds up to `total_ms`,
    /// so a press that never came leaves the puzzles it would have marked at
    /// zero, with the remainder on the one being solved.
    pub fn durations(&self, total_ms: u64) -> Vec<u64> {
        let mut durations = Vec::with_capacity(self.puzzles());
        let mut start = 0;
        for index in 0..self.puzzles() {
            let end = if index < self.marked() {
                self.at_ms[index].min(total_ms)
            } else {
                total_ms
            };
            let end = end.max(start);
            durations.push(end - start);
            start = end;
        }
        durations
    }
}

/// What the timer is doing.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum TimerPhase {
    Idle,
    /// The key is down with inspection off; `since` is when it was pressed.
    Holding {
        since: Instant,
    },
    /// The inspection count-up is running.
    Inspecting {
        started: Instant,
        /// When the key was pressed to start the solve, if it is still held.
        held_since: Option<Instant>,
        /// Whether the 8-second warning has already played.
        eight_cue_played: bool,
        /// Whether the 12-second warning has already played.
        twelve_cue_played: bool,
    },
    /// Hands are on a hardware timer; `ready` once lifting them starts it.
    Armed {
        ready: bool,
    },
    Running {
        started: Instant,
        modifier: Modifier,
        /// How long into the attempt a blindfolded solver marked the end of
        /// the memo, once they have.
        memo: Option<Duration>,
    },
}

/// What a hardware timer reports. It does the timing itself, so these replace
/// the timer key and the app only follows along.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum HardwareState {
    /// The timer was reset.
    Idle,
    /// Hands are on the timer, but not for long enough to start.
    HandsOn,
    /// Lifting the hands now starts the timer.
    GetSet,
    /// Hands left the timer before it was ready.
    HandsOff,
    Running,
    /// The timer stopped; [`Self::Finished`] carries the time.
    Stopped,
    /// The attempt is over, with its time in milliseconds.
    Finished(u64),
}

/// A finished attempt, ready to be recorded.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct TimerOutcome {
    pub time_ms: u64,
    pub modifier: Modifier,
    /// The part of `time_ms` spent on the memo, when the attempt marked one.
    pub memo_ms: Option<u64>,
}

/// Whether the held key would start the solve if released now.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Readiness {
    /// The key is not held.
    Released,
    /// The key is held, but not for long enough yet.
    Waiting,
    Ready,
}

/// An audio warning that plays during inspection.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum InspectionCue {
    /// The warning that plays 8 seconds into inspection.
    EightSeconds,
    /// The warning that plays 12 seconds into inspection.
    TwelveSeconds,
}

/// The timer's configuration and current phase.
#[derive(Clone, Copy, Debug)]
pub struct Timer {
    phase: TimerPhase,
    pub inspection: bool,
    pub inspection_limit: Duration,
    pub hold: Duration,
    /// Whether the attempt is blindfolded: there is no inspection whatever
    /// `inspection` says, and the first press of a running attempt marks the
    /// end of the memo instead of stopping it.
    pub blindfolded: bool,
    /// How many puzzles the next attempt has. More than one makes it a relay:
    /// there is one inspection for all of them, and the first presses of a
    /// running attempt mark the end of each puzzle but the last.
    pub puzzles: usize,
    splits: Splits,
}

impl Timer {
    pub const fn new(inspection: bool, inspection_limit: Duration, hold: Duration) -> Self {
        Self {
            phase: TimerPhase::Idle,
            inspection,
            inspection_limit,
            hold,
            blindfolded: false,
            puzzles: 1,
            splits: Splits::NONE,
        }
    }

    /// Returns the ends of the puzzles marked in the current or last attempt.
    pub const fn splits(&self) -> &Splits {
        &self.splits
    }

    /// Returns whether the next attempt starts with an inspection.
    pub const fn inspects(&self) -> bool {
        self.inspection && !self.blindfolded
    }

    pub const fn phase(&self) -> TimerPhase {
        self.phase
    }

    /// Returns whether an attempt is under way, so the UI must not interrupt it.
    pub const fn is_active(&self) -> bool {
        !matches!(self.phase, TimerPhase::Idle)
    }

    /// Handles the timer key going down. A running timer stops and reports its time.
    pub fn press(&mut self, now: Instant) -> Option<TimerOutcome> {
        match self.phase {
            TimerPhase::Idle if self.inspects() => {
                self.splits = Splits::new(self.puzzles);
                self.phase = TimerPhase::Inspecting {
                    started: now,
                    held_since: None,
                    eight_cue_played: false,
                    twelve_cue_played: false,
                };
                None
            }
            TimerPhase::Idle => {
                self.splits = Splits::new(self.puzzles);
                self.phase = TimerPhase::Holding { since: now };
                None
            }
            TimerPhase::Holding { .. } | TimerPhase::Armed { .. } => None,
            TimerPhase::Inspecting {
                started,
                eight_cue_played,
                twelve_cue_played,
                ..
            } => {
                self.phase = TimerPhase::Inspecting {
                    started,
                    held_since: Some(now),
                    eight_cue_played,
                    twelve_cue_played,
                };
                None
            }
            TimerPhase::Running { started, .. } if self.splits.pending() => {
                self.splits
                    .mark(now.duration_since(started).as_millis() as u64);
                None
            }
            TimerPhase::Running {
                started,
                modifier,
                memo: None,
            } if self.blindfolded => {
                self.phase = TimerPhase::Running {
                    started,
                    modifier,
                    memo: Some(now.duration_since(started)),
                };
                None
            }
            TimerPhase::Running {
                started,
                modifier,
                memo,
            } => {
                self.phase = TimerPhase::Idle;
                let time_ms = now.duration_since(started).as_millis() as u64;
                Some(TimerOutcome {
                    time_ms,
                    modifier,
                    memo_ms: memo.map(|memo| (memo.as_millis() as u64).min(time_ms)),
                })
            }
        }
    }

    /// Handles the timer key going up.
    ///
    /// Returns an outcome only when the release ends the attempt, which happens
    /// when inspection ran out while the key was held.
    pub fn release(&mut self, now: Instant) -> Option<TimerOutcome> {
        match self.phase {
            TimerPhase::Holding { since } => {
                self.phase = if now.duration_since(since) >= self.hold {
                    TimerPhase::Running {
                        started: now,
                        modifier: Modifier::None,
                        memo: None,
                    }
                } else {
                    TimerPhase::Idle
                };
                None
            }
            TimerPhase::Inspecting {
                started,
                held_since: Some(since),
                eight_cue_played,
                twelve_cue_played,
            } => {
                if now.duration_since(since) < self.hold {
                    self.phase = TimerPhase::Inspecting {
                        started,
                        held_since: None,
                        eight_cue_played,
                        twelve_cue_played,
                    };
                    return None;
                }
                match self.inspection_modifier(now.duration_since(started)) {
                    Modifier::DNF => Some(self.expire()),
                    modifier => {
                        self.phase = TimerPhase::Running {
                            started: now,
                            modifier,
                            memo: None,
                        };
                        None
                    }
                }
            }
            TimerPhase::Idle
            | TimerPhase::Armed { .. }
            | TimerPhase::Inspecting { .. }
            | TimerPhase::Running { .. } => None,
        }
    }

    /// Advances time-based transitions, ending an inspection past the DNF limit.
    pub fn tick(&mut self, now: Instant) -> Option<TimerOutcome> {
        match self.phase {
            TimerPhase::Inspecting { started, .. }
                if self.inspection_modifier(now.duration_since(started)) == Modifier::DNF =>
            {
                Some(self.expire())
            }
            _ => None,
        }
    }

    /// Follows a hardware timer, returning the attempt it just finished.
    ///
    /// Only an attempt seen running is reported, so one cancelled here stays
    /// cancelled when the hardware finishes it. A running attempt ends only by
    /// finishing or a reset: hands go back on the timer to stop it, and that
    /// must not lose the time still on its way.
    pub fn apply_hardware(&mut self, state: HardwareState, now: Instant) -> Option<TimerOutcome> {
        let running = matches!(self.phase, TimerPhase::Running { .. });
        match state {
            HardwareState::Finished(time_ms) => {
                let outcome = match self.phase {
                    TimerPhase::Running { modifier, .. } => Some(TimerOutcome {
                        time_ms,
                        modifier,
                        memo_ms: None,
                    }),
                    _ => None,
                };
                self.phase = TimerPhase::Idle;
                return outcome;
            }
            HardwareState::Idle => self.phase = TimerPhase::Idle,
            HardwareState::Stopped => {}
            _ if running => {}
            HardwareState::HandsOff => self.phase = TimerPhase::Idle,
            HardwareState::HandsOn => self.phase = TimerPhase::Armed { ready: false },
            HardwareState::GetSet => self.phase = TimerPhase::Armed { ready: true },
            HardwareState::Running => {
                self.splits = Splits::NONE;
                self.phase = TimerPhase::Running {
                    started: now,
                    modifier: Modifier::None,
                    memo: None,
                };
            }
        }
        None
    }

    /// Abandons the attempt without recording anything.
    pub const fn cancel(&mut self) {
        self.phase = TimerPhase::Idle;
    }

    /// Returns how long the current attempt or inspection has been running.
    pub fn elapsed(&self, now: Instant) -> Duration {
        match self.phase {
            TimerPhase::Inspecting { started, .. } | TimerPhase::Running { started, .. } => {
                now.duration_since(started)
            }
            TimerPhase::Idle | TimerPhase::Holding { .. } | TimerPhase::Armed { .. } => {
                Duration::ZERO
            }
        }
    }

    /// Returns how long the memo took, once a blindfolded attempt marked its
    /// end.
    pub const fn memo(&self) -> Option<Duration> {
        match self.phase {
            TimerPhase::Running { memo, .. } => memo,
            _ => None,
        }
    }

    /// Returns the penalty an inspection of this length would earn.
    pub fn inspection_modifier(&self, elapsed: Duration) -> Modifier {
        if elapsed < self.inspection_limit {
            Modifier::None
        } else if elapsed < self.inspection_limit + PENALTY_WINDOW {
            Modifier::PlusTwo
        } else {
            Modifier::DNF
        }
    }

    /// Returns the inspection warnings that have become due since the last
    /// call, in the order they should play.
    ///
    /// Each warning is reported once per inspection.
    pub fn take_inspection_cues(&mut self, now: Instant) -> [Option<InspectionCue>; 2] {
        let TimerPhase::Inspecting {
            started,
            eight_cue_played,
            twelve_cue_played,
            ..
        } = &mut self.phase
        else {
            return [None, None];
        };

        let elapsed = now.duration_since(*started);
        let mut cues = [None, None];
        if elapsed >= EIGHT_SECOND_CUE && !*eight_cue_played {
            *eight_cue_played = true;
            cues[0] = Some(InspectionCue::EightSeconds);
        }
        if elapsed >= TWELVE_SECOND_CUE && !*twelve_cue_played {
            *twelve_cue_played = true;
            cues[1] = Some(InspectionCue::TwelveSeconds);
        }
        cues
    }

    /// Returns whether the key is held, and for long enough to start the solve.
    pub fn readiness(&self, now: Instant) -> Readiness {
        if let TimerPhase::Armed { ready } = self.phase {
            return if ready {
                Readiness::Ready
            } else {
                Readiness::Waiting
            };
        }
        let (TimerPhase::Holding { since }
        | TimerPhase::Inspecting {
            held_since: Some(since),
            ..
        }) = self.phase
        else {
            return Readiness::Released;
        };
        if now.duration_since(since) >= self.hold {
            Readiness::Ready
        } else {
            Readiness::Waiting
        }
    }

    /// Ends an expired inspection with the zero-length DNF cube-tui also records.
    const fn expire(&mut self) -> TimerOutcome {
        self.phase = TimerPhase::Idle;
        TimerOutcome {
            time_ms: 0,
            modifier: Modifier::DNF,
            memo_ms: None,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const LIMIT: Duration = Duration::from_secs(15);
    const HOLD: Duration = Duration::from_millis(300);

    fn ms(value: u64) -> Duration {
        Duration::from_millis(value)
    }

    #[test]
    fn inspection_boundaries_match_the_wca_rules() {
        let timer = Timer::new(true, LIMIT, HOLD);
        assert_eq!(timer.inspection_modifier(ms(14_999)), Modifier::None);
        assert_eq!(timer.inspection_modifier(ms(15_000)), Modifier::PlusTwo);
        assert_eq!(timer.inspection_modifier(ms(16_999)), Modifier::PlusTwo);
        assert_eq!(timer.inspection_modifier(ms(17_000)), Modifier::DNF);
    }

    fn hardware_timer() -> Timer {
        Timer::new(false, LIMIT, HOLD)
    }

    #[test]
    fn a_hardware_attempt_follows_the_timer_and_keeps_its_time() {
        let mut timer = hardware_timer();
        let now = Instant::now();
        assert_eq!(timer.apply_hardware(HardwareState::HandsOn, now), None);
        assert_eq!(timer.readiness(now), Readiness::Waiting);
        assert_eq!(timer.apply_hardware(HardwareState::GetSet, now), None);
        assert_eq!(timer.readiness(now), Readiness::Ready);
        assert_eq!(timer.apply_hardware(HardwareState::Running, now), None);
        assert!(matches!(timer.phase(), TimerPhase::Running { .. }));

        let outcome = timer.apply_hardware(HardwareState::Finished(8_980), now + ms(9_000));
        assert_eq!(
            outcome,
            Some(TimerOutcome {
                time_ms: 8_980,
                modifier: Modifier::None,
                memo_ms: None
            })
        );
        assert_eq!(timer.phase(), TimerPhase::Idle);
    }

    #[test]
    fn hands_back_on_a_running_hardware_timer_do_not_lose_the_solve() {
        let mut timer = hardware_timer();
        let now = Instant::now();
        timer.apply_hardware(HardwareState::Running, now);
        for state in [
            HardwareState::Stopped,
            HardwareState::HandsOn,
            HardwareState::GetSet,
            HardwareState::HandsOff,
        ] {
            assert_eq!(timer.apply_hardware(state, now), None);
            assert!(matches!(timer.phase(), TimerPhase::Running { .. }));
        }
        assert!(
            timer
                .apply_hardware(HardwareState::Finished(1_000), now)
                .is_some()
        );
    }

    #[test]
    fn a_repeated_running_report_keeps_the_start() {
        let mut timer = hardware_timer();
        let now = Instant::now();
        timer.apply_hardware(HardwareState::Running, now);
        timer.apply_hardware(HardwareState::Running, now + ms(500));
        assert_eq!(timer.elapsed(now + ms(1_000)), ms(1_000));
    }

    #[test]
    fn a_cancelled_hardware_attempt_is_not_recorded_when_it_finishes() {
        let mut timer = hardware_timer();
        let now = Instant::now();
        timer.apply_hardware(HardwareState::Running, now);
        timer.cancel();
        assert_eq!(
            timer.apply_hardware(HardwareState::Finished(1_000), now),
            None
        );
    }

    #[test]
    fn hands_off_and_a_reset_end_a_hardware_attempt_without_a_result() {
        let mut timer = hardware_timer();
        let now = Instant::now();
        timer.apply_hardware(HardwareState::HandsOn, now);
        assert_eq!(timer.apply_hardware(HardwareState::HandsOff, now), None);
        assert_eq!(timer.phase(), TimerPhase::Idle);

        timer.apply_hardware(HardwareState::Running, now);
        assert_eq!(timer.apply_hardware(HardwareState::Idle, now), None);
        assert_eq!(timer.phase(), TimerPhase::Idle);
    }

    #[test]
    fn the_timer_key_does_nothing_while_hands_are_on_the_hardware() {
        let mut timer = hardware_timer();
        let now = Instant::now();
        timer.apply_hardware(HardwareState::GetSet, now);
        assert_eq!(timer.press(now), None);
        assert_eq!(timer.release(now + ms(1_000)), None);
        assert_eq!(timer.phase(), TimerPhase::Armed { ready: true });
    }

    #[test]
    fn inspection_audio_cues_fire_once_each() {
        let mut timer = Timer::new(true, LIMIT, HOLD);
        let start = Instant::now();
        timer.press(start);

        assert_eq!(timer.take_inspection_cues(start + ms(7_999)), [None, None]);
        assert_eq!(
            timer.take_inspection_cues(start + ms(8_000)),
            [Some(InspectionCue::EightSeconds), None]
        );
        assert_eq!(timer.take_inspection_cues(start + ms(8_500)), [None, None]);
        assert_eq!(
            timer.take_inspection_cues(start + ms(12_000)),
            [None, Some(InspectionCue::TwelveSeconds)]
        );
        assert_eq!(timer.take_inspection_cues(start + ms(14_000)), [None, None]);
    }

    #[test]
    fn a_new_inspection_plays_its_cues_again() {
        let mut timer = Timer::new(true, LIMIT, HOLD);
        let start = Instant::now();
        timer.press(start);
        timer.take_inspection_cues(start + ms(8_000));
        timer.cancel();

        let next = start + ms(20_000);
        timer.press(next);
        assert_eq!(
            timer.take_inspection_cues(next + ms(8_000)),
            [Some(InspectionCue::EightSeconds), None]
        );
    }

    #[test]
    fn a_solve_without_inspection_needs_a_full_hold() {
        let mut timer = Timer::new(false, LIMIT, HOLD);
        let start = Instant::now();

        timer.press(start);
        assert_eq!(timer.readiness(start + ms(100)), Readiness::Waiting);
        timer.release(start + ms(100));
        assert_eq!(timer.phase(), TimerPhase::Idle);

        timer.press(start);
        assert_eq!(timer.readiness(start + HOLD), Readiness::Ready);
        timer.release(start + HOLD);
        let outcome = timer.press(start + HOLD + ms(9_000));
        assert_eq!(
            outcome,
            Some(TimerOutcome {
                time_ms: 9_000,
                modifier: Modifier::None,
                memo_ms: None
            })
        );
        assert_eq!(timer.phase(), TimerPhase::Idle);
    }

    #[test]
    fn inspection_starts_on_press_and_survives_a_short_hold() {
        let mut timer = Timer::new(true, LIMIT, HOLD);
        let start = Instant::now();
        timer.press(start);
        timer.release(start + ms(50));
        assert!(matches!(timer.phase(), TimerPhase::Inspecting { .. }));

        timer.press(start + ms(1_000));
        timer.release(start + ms(1_100));
        assert!(matches!(
            timer.phase(),
            TimerPhase::Inspecting {
                held_since: None,
                ..
            }
        ));
    }

    #[test]
    fn starting_late_carries_a_plus_two_into_the_solve() {
        let mut timer = Timer::new(true, LIMIT, HOLD);
        let start = Instant::now();
        timer.press(start);
        timer.press(start + ms(15_500));
        timer.release(start + ms(16_000));
        let outcome = timer.press(start + ms(26_000)).unwrap();
        assert_eq!(outcome.modifier, Modifier::PlusTwo);
        assert_eq!(outcome.time_ms, 10_000);
    }

    #[test]
    fn inspection_running_out_records_a_zero_length_dnf() {
        let mut timer = Timer::new(true, LIMIT, HOLD);
        let start = Instant::now();
        timer.press(start);
        assert_eq!(timer.tick(start + ms(16_999)), None);
        assert_eq!(
            timer.tick(start + ms(17_000)),
            Some(TimerOutcome {
                time_ms: 0,
                modifier: Modifier::DNF,
                memo_ms: None
            })
        );
        assert_eq!(timer.phase(), TimerPhase::Idle);
    }

    fn blindfolded_timer() -> Timer {
        let mut timer = Timer::new(true, LIMIT, HOLD);
        timer.blindfolded = true;
        timer
    }

    /// Starts a blindfolded attempt at `start`.
    fn start_blindfolded(timer: &mut Timer, start: Instant) {
        timer.press(start);
        timer.release(start + HOLD);
    }

    #[test]
    fn a_blindfolded_attempt_skips_inspection_whatever_the_setting() {
        let mut timer = blindfolded_timer();
        assert!(timer.inspection);
        assert!(!timer.inspects());
        let start = Instant::now();
        timer.press(start);
        assert!(matches!(timer.phase(), TimerPhase::Holding { .. }));
        assert_eq!(timer.readiness(start + HOLD), Readiness::Ready);
        timer.release(start + HOLD);
        assert!(matches!(timer.phase(), TimerPhase::Running { .. }));
        assert_eq!(timer.tick(start + ms(60_000)), None);
    }

    #[test]
    fn the_first_press_marks_the_memo_and_the_second_stops() {
        let mut timer = blindfolded_timer();
        let start = Instant::now();
        start_blindfolded(&mut timer, start);
        let began = start + HOLD;

        assert_eq!(timer.press(began + ms(12_340)), None);
        assert!(matches!(timer.phase(), TimerPhase::Running { .. }));
        assert_eq!(timer.memo(), Some(ms(12_340)));

        let outcome = timer.press(began + ms(40_000)).unwrap();
        assert_eq!(outcome.time_ms, 40_000);
        assert_eq!(outcome.memo_ms, Some(12_340));
        assert_eq!(outcome.modifier, Modifier::None);
        assert_eq!(timer.phase(), TimerPhase::Idle);
        assert_eq!(timer.memo(), None);
    }

    #[test]
    fn the_memo_is_never_longer_than_the_attempt() {
        let mut timer = blindfolded_timer();
        let start = Instant::now();
        start_blindfolded(&mut timer, start);
        let began = start + HOLD;
        timer.press(began + ms(5_000));
        // The clock cannot run backwards, but the rule does not rely on it.
        let outcome = timer.press(began + ms(5_000)).unwrap();
        assert_eq!(outcome.memo_ms, Some(5_000));
        assert_eq!(outcome.time_ms, 5_000);
    }

    #[test]
    fn an_ordinary_attempt_has_no_memo() {
        let mut timer = Timer::new(false, LIMIT, HOLD);
        let start = Instant::now();
        start_blindfolded(&mut timer, start);
        let outcome = timer.press(start + HOLD + ms(9_000)).unwrap();
        assert_eq!(outcome.memo_ms, None);
    }

    #[test]
    fn cancelling_a_blindfolded_attempt_records_nothing() {
        let mut timer = blindfolded_timer();
        let start = Instant::now();
        start_blindfolded(&mut timer, start);
        timer.press(start + HOLD + ms(3_000));
        timer.cancel();
        assert!(!timer.is_active());
        assert_eq!(timer.memo(), None);
    }

    #[test]
    fn cancelling_records_nothing() {
        let mut timer = Timer::new(true, LIMIT, HOLD);
        timer.press(Instant::now());
        timer.cancel();
        assert!(!timer.is_active());
    }

    fn start_relay(timer: &mut Timer, puzzles: usize, start: Instant) {
        timer.puzzles = puzzles;
        timer.press(start);
        timer.release(start + HOLD);
    }

    #[test]
    fn a_relay_marks_each_puzzle_and_the_last_press_stops_it() {
        let mut timer = Timer::new(false, LIMIT, HOLD);
        let start = Instant::now();
        start_relay(&mut timer, 3, start);
        let begun = start + HOLD;
        assert!(timer.splits().pending());
        assert_eq!(timer.press(begun + ms(1_000)), None);
        assert_eq!(timer.splits().current(), 1);
        assert_eq!(timer.press(begun + ms(3_500)), None);
        assert!(!timer.splits().pending());
        let outcome = timer.press(begun + ms(6_000)).expect("last press stops");
        assert_eq!(outcome.time_ms, 6_000);
        assert_eq!(
            timer.splits().durations(outcome.time_ms),
            vec![1_000, 2_500, 2_500]
        );
        assert!(!timer.is_active());
    }

    #[test]
    fn relay_inspection_is_taken_once_for_all_the_puzzles() {
        let mut timer = Timer::new(true, LIMIT, HOLD);
        let start = Instant::now();
        timer.puzzles = 2;
        timer.press(start);
        timer.press(start + ms(15_500));
        timer.release(start + ms(15_900));
        // A late start earns the +2 of the whole relay, not one per puzzle.
        let begun = start + ms(15_900);
        assert_eq!(timer.press(begun + ms(2_000)), None);
        let outcome = timer.press(begun + ms(5_000)).expect("stops");
        assert_eq!(outcome.modifier, Modifier::PlusTwo);
        assert_eq!(
            timer.splits().durations(outcome.time_ms),
            vec![2_000, 3_000]
        );
    }

    #[test]
    fn splits_always_add_up_to_the_total() {
        let mut splits = Splits::new(4);
        splits.mark(2_000);
        splits.mark(1_000);
        let durations = splits.durations(9_000);
        assert_eq!(durations.len(), 4);
        assert_eq!(durations.iter().sum::<u64>(), 9_000);
        // A mark cannot go back in time.
        assert_eq!(durations, vec![2_000, 0, 7_000, 0]);
        // Marks beyond the total are held to it.
        assert_eq!(splits.durations(1_500).iter().sum::<u64>(), 1_500);
    }

    #[test]
    fn a_single_puzzle_has_nothing_to_mark() {
        let mut timer = Timer::new(false, LIMIT, HOLD);
        let start = Instant::now();
        timer.press(start);
        timer.release(start + HOLD);
        assert!(!timer.splits().pending());
        assert!(timer.press(start + HOLD + ms(500)).is_some());
    }

    #[test]
    fn a_relay_cancelled_early_marks_nothing_for_the_next_attempt() {
        let mut timer = Timer::new(false, LIMIT, HOLD);
        let start = Instant::now();
        start_relay(&mut timer, 3, start);
        timer.press(start + HOLD + ms(500));
        timer.cancel();
        timer.puzzles = 1;
        timer.press(start + ms(5_000));
        assert_eq!(timer.splits().puzzles(), 1);
        assert_eq!(timer.splits().marked(), 0);
    }
}
