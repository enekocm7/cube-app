//! Deadlines for short-lived notifications.
//!
//! The GUI owns one [`ToastSchedule`] and closes a toast when its deadline
//! passes. Keeping the clocks here lets the behaviour be tested without a
//! window: each toast gets its own deadline, so a new one cannot extend an
//! older one, and a toast that must stay on screen is simply never added.

use std::time::{Duration, Instant};

/// How long a short notification stays on screen.
pub const TOAST_TIMEOUT: Duration = Duration::from_secs(5);

/// The outstanding deadlines of the toasts that should disappear on their own.
#[derive(Debug, Default)]
pub struct ToastSchedule {
    next_id: u64,
    deadlines: Vec<(u64, Instant)>,
}

impl ToastSchedule {
    /// Schedules a toast at `now` and returns the key used to close it.
    pub fn push(&mut self, now: Instant) -> u64 {
        let id = self.next_id;
        self.next_id = self.next_id.wrapping_add(1);
        self.deadlines.push((id, now + TOAST_TIMEOUT));
        id
    }

    /// Returns whether there is nothing left to close.
    pub fn is_empty(&self) -> bool {
        self.deadlines.is_empty()
    }

    /// Forgets a toast that was dismissed before its deadline.
    pub fn cancel(&mut self, id: u64) {
        self.deadlines.retain(|(pending, _)| *pending != id);
    }

    /// Returns, and clears, the keys whose deadline has been reached.
    pub fn expired(&mut self, now: Instant) -> Vec<u64> {
        let mut expired = Vec::new();
        self.deadlines.retain(|(id, deadline)| {
            if *deadline <= now {
                expired.push(*id);
                false
            } else {
                true
            }
        });
        expired
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_toast_expires_five_seconds_after_it_was_pushed() {
        let now = Instant::now();
        let mut schedule = ToastSchedule::default();
        let id = schedule.push(now);

        assert_eq!(
            schedule.expired(now + Duration::from_millis(4_999)),
            Vec::<u64>::new()
        );
        assert_eq!(schedule.expired(now + TOAST_TIMEOUT), vec![id]);
        assert_eq!(schedule.expired(now + TOAST_TIMEOUT * 2), Vec::<u64>::new());
    }

    #[test]
    fn a_new_toast_does_not_restart_an_earlier_countdown() {
        let now = Instant::now();
        let mut schedule = ToastSchedule::default();
        let first = schedule.push(now);
        let second = schedule.push(now + Duration::from_secs(2));

        assert_eq!(schedule.expired(now + Duration::from_secs(5)), vec![first]);
        assert_eq!(schedule.expired(now + Duration::from_secs(7)), vec![second]);
        assert_eq!(
            schedule.expired(now + Duration::from_secs(8)),
            Vec::<u64>::new()
        );
    }

    #[test]
    fn dismissing_a_toast_cancels_only_its_own_deadline() {
        let now = Instant::now();
        let mut schedule = ToastSchedule::default();
        let dismissed = schedule.push(now);
        let kept = schedule.push(now + Duration::from_secs(1));
        schedule.cancel(dismissed);

        assert_eq!(schedule.expired(now + Duration::from_secs(6)), vec![kept]);
    }

    #[test]
    fn every_toast_gets_a_distinct_key() {
        let now = Instant::now();
        let mut schedule = ToastSchedule::default();
        let ids: Vec<_> = (0..100).map(|_| schedule.push(now)).collect();
        let unique: std::collections::HashSet<_> = ids.iter().copied().collect();

        assert_eq!(unique.len(), ids.len());
        assert_eq!(schedule.expired(now + TOAST_TIMEOUT).len(), ids.len());
    }

    #[test]
    fn toasts_that_must_stay_are_never_scheduled() {
        let mut schedule = ToastSchedule::default();

        assert_eq!(
            schedule.expired(Instant::now() + Duration::from_secs(60)),
            Vec::<u64>::new()
        );
    }
}
