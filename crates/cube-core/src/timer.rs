//! The timer state machine.
//!
//! Every transition takes the current [`Instant`], so timing logic can be tested
//! without sleeping and the UI stays free of rules.

use std::time::{Duration, Instant};

use crate::solve::Modifier;

/// Extra time allowed after the limit before an inspection becomes a DNF.
const PENALTY_WINDOW: Duration = Duration::from_secs(2);

/// How long the timer key must be held by default before a release starts the solve.
pub const DEFAULT_HOLD: Duration = Duration::from_millis(300);

/// What the timer is doing.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum TimerPhase {
    Idle,
    /// The key is down with inspection off; `since` is when it was pressed.
    Holding { since: Instant },
    /// The inspection countdown is running.
    Inspecting {
        started: Instant,
        /// When the key was pressed to start the solve, if it is still held.
        held_since: Option<Instant>,
    },
    Running {
        started: Instant,
        modifier: Modifier,
    },
}

/// A finished attempt, ready to be recorded.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct TimerOutcome {
    pub time_ms: u64,
    pub modifier: Modifier,
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

/// The timer's configuration and current phase.
#[derive(Clone, Copy, Debug)]
pub struct Timer {
    phase: TimerPhase,
    pub inspection: bool,
    pub inspection_limit: Duration,
    pub hold: Duration,
}

impl Timer {
    pub const fn new(inspection: bool, inspection_limit: Duration, hold: Duration) -> Self {
        Self {
            phase: TimerPhase::Idle,
            inspection,
            inspection_limit,
            hold,
        }
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
            TimerPhase::Idle if self.inspection => {
                self.phase = TimerPhase::Inspecting {
                    started: now,
                    held_since: None,
                };
                None
            }
            TimerPhase::Idle => {
                self.phase = TimerPhase::Holding { since: now };
                None
            }
            TimerPhase::Holding { .. } => None,
            TimerPhase::Inspecting { started, .. } => {
                self.phase = TimerPhase::Inspecting {
                    started,
                    held_since: Some(now),
                };
                None
            }
            TimerPhase::Running { started, modifier } => {
                self.phase = TimerPhase::Idle;
                Some(TimerOutcome {
                    time_ms: now.duration_since(started).as_millis() as u64,
                    modifier,
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
                    }
                } else {
                    TimerPhase::Idle
                };
                None
            }
            TimerPhase::Inspecting {
                started,
                held_since: Some(since),
            } => {
                if now.duration_since(since) < self.hold {
                    self.phase = TimerPhase::Inspecting {
                        started,
                        held_since: None,
                    };
                    return None;
                }
                match self.inspection_modifier(now.duration_since(started)) {
                    Modifier::DNF => Some(self.expire()),
                    modifier => {
                        self.phase = TimerPhase::Running {
                            started: now,
                            modifier,
                        };
                        None
                    }
                }
            }
            TimerPhase::Idle | TimerPhase::Inspecting { .. } | TimerPhase::Running { .. } => None,
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
            TimerPhase::Idle | TimerPhase::Holding { .. } => Duration::ZERO,
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

    /// Returns whether the key is held, and for long enough to start the solve.
    pub fn readiness(&self, now: Instant) -> Readiness {
        let since = match self.phase {
            TimerPhase::Holding { since }
            | TimerPhase::Inspecting {
                held_since: Some(since),
                ..
            } => since,
            _ => return Readiness::Released,
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
                modifier: Modifier::None
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
                modifier: Modifier::DNF
            })
        );
        assert_eq!(timer.phase(), TimerPhase::Idle);
    }

    #[test]
    fn cancelling_records_nothing() {
        let mut timer = Timer::new(true, LIMIT, HOLD);
        timer.press(Instant::now());
        timer.cancel();
        assert!(!timer.is_active());
    }
}
