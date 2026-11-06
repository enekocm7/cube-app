//! Spaced repetition: when a learned case should come back for review.
//!
//! This is the SM-2 family of schedules. Each review multiplies the gap by the
//! case's ease, and the ease itself drifts with how the reviews go, so cases
//! that keep being hard come back more often than those that never are.

use serde::{Deserialize, Serialize};

const DAY_MS: u64 = 24 * 60 * 60 * 1000;

/// The ease of a new case, in thousandths.
const START_EASE: u32 = 2_500;
/// The ease never drops below this, or a hard case would never spread out.
const MIN_EASE: u32 = 1_300;
/// The longest gap between reviews, in days.
const MAX_INTERVAL: u32 = 365;

/// How a review went.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Grade {
    /// It was forgotten.
    Again,
    /// It came back with effort.
    Hard,
    /// It came back.
    Good,
    /// It came back at once.
    Easy,
}

/// When a case is next due and how its reviews have gone.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Review {
    pub due_unix_ms: u64,
    pub interval_days: u32,
    /// How much the gap grows with each good review, in thousandths.
    pub ease: u32,
    /// How many times the case was forgotten.
    #[serde(default)]
    pub lapses: u32,
}

impl Review {
    /// Starts the reviews of a case just learned: the first is due tomorrow.
    pub const fn first(now_unix_ms: u64) -> Self {
        Self {
            due_unix_ms: now_unix_ms + DAY_MS,
            interval_days: 1,
            ease: START_EASE,
            lapses: 0,
        }
    }

    /// Returns whether the case is due at `now_unix_ms`.
    pub const fn is_due(&self, now_unix_ms: u64) -> bool {
        self.due_unix_ms <= now_unix_ms
    }

    /// Schedules the next review after one that went as `grade`.
    pub fn grade(&mut self, grade: Grade, now_unix_ms: u64) {
        let scaled = |factor: u32| {
            let days = u64::from(self.interval_days) * u64::from(factor) / 1_000;
            days.min(u64::from(MAX_INTERVAL)) as u32
        };
        let (interval, ease) = match grade {
            Grade::Again => {
                self.lapses += 1;
                (1, self.ease.saturating_sub(200))
            }
            Grade::Hard => (scaled(1_200), self.ease.saturating_sub(150)),
            Grade::Good => (scaled(self.ease), self.ease),
            Grade::Easy => (scaled(self.ease * 13 / 10), self.ease + 150),
        };
        // Any review that went well waits longer than the one before.
        let least = if grade == Grade::Again {
            1
        } else {
            (self.interval_days + 1).min(MAX_INTERVAL)
        };
        self.interval_days = interval.max(least);
        self.ease = ease.max(MIN_EASE);
        self.due_unix_ms = now_unix_ms + u64::from(self.interval_days) * DAY_MS;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const NOW: u64 = 1_000_000;

    #[test]
    fn a_new_case_is_due_tomorrow() {
        let review = Review::first(NOW);
        assert!(!review.is_due(NOW));
        assert!(!review.is_due(NOW + DAY_MS - 1));
        assert!(review.is_due(NOW + DAY_MS));
    }

    #[test]
    fn good_reviews_spread_out() {
        let mut review = Review::first(NOW);
        let mut gaps = Vec::new();
        for _ in 0..5 {
            review.grade(Grade::Good, NOW);
            gaps.push(review.interval_days);
        }
        assert_eq!(gaps, [2, 5, 12, 30, 75]);
        assert_eq!(review.ease, START_EASE);
    }

    #[test]
    fn the_next_review_is_counted_from_now() {
        let mut review = Review::first(NOW);
        review.grade(Grade::Good, NOW + DAY_MS);
        assert_eq!(review.due_unix_ms, NOW + 3 * DAY_MS);
    }

    #[test]
    fn forgetting_starts_over_and_is_counted() {
        let mut review = Review::first(NOW);
        for _ in 0..4 {
            review.grade(Grade::Good, NOW);
        }
        review.grade(Grade::Again, NOW);
        assert_eq!(review.interval_days, 1);
        assert_eq!(review.lapses, 1);
        assert_eq!(review.ease, START_EASE - 200);
        assert_eq!(review.due_unix_ms, NOW + DAY_MS);
    }

    #[test]
    fn easy_beats_good_and_good_beats_hard() {
        let after = |grade: Grade| {
            let mut review = Review::first(NOW);
            review.interval_days = 10;
            review.grade(grade, NOW);
            review
        };
        let (hard, good, easy) = (after(Grade::Hard), after(Grade::Good), after(Grade::Easy));
        assert!(hard.interval_days < good.interval_days);
        assert!(good.interval_days < easy.interval_days);
        assert!(hard.ease < good.ease && good.ease < easy.ease);
    }

    #[test]
    fn a_review_that_went_well_always_waits_longer() {
        let mut review = Review::first(NOW);
        review.ease = MIN_EASE;
        review.grade(Grade::Hard, NOW);
        assert_eq!(review.interval_days, 2);
    }

    #[test]
    fn the_ease_has_a_floor() {
        let mut review = Review::first(NOW);
        for _ in 0..30 {
            review.grade(Grade::Again, NOW);
        }
        assert_eq!(review.ease, MIN_EASE);
    }

    #[test]
    fn the_gap_has_a_ceiling() {
        let mut review = Review::first(NOW);
        for _ in 0..40 {
            review.grade(Grade::Easy, NOW);
        }
        assert_eq!(review.interval_days, MAX_INTERVAL);
    }
}
