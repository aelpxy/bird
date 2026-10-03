use std::time::{Duration, Instant};

const DEFAULT_BASE: Duration = Duration::from_secs(10);
const DEFAULT_MAX: Duration = Duration::from_mins(5);

#[derive(Debug)]
pub(crate) struct Backoff {
    base: Duration,
    max: Duration,
    failures: u32,
    next_attempt: Option<Instant>,
}

impl Default for Backoff {
    fn default() -> Self {
        Self::new(DEFAULT_BASE, DEFAULT_MAX)
    }
}

impl Backoff {
    pub(crate) const fn new(base: Duration, max: Duration) -> Self {
        Self {
            base,
            max,
            failures: 0,
            next_attempt: None,
        }
    }

    pub(crate) fn ready(&self, now: Instant) -> bool {
        self.next_attempt.is_none_or(|at| now >= at)
    }

    pub(crate) fn next_attempt(&self) -> Option<Instant> {
        self.next_attempt
    }

    pub(crate) fn fail(&mut self, now: Instant) -> Duration {
        let delay = self.delay_after(self.failures);
        self.failures = self.failures.saturating_add(1);
        self.next_attempt = Some(now + delay);
        delay
    }

    pub(crate) fn reset(&mut self) {
        self.failures = 0;
        self.next_attempt = None;
    }

    fn delay_after(&self, failures: u32) -> Duration {
        self.base
            .saturating_mul(2_u32.saturating_pow(failures))
            .min(self.max)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn doubles_up_to_max() {
        let backoff = Backoff::default();
        let delays: Vec<u64> = (0..7).map(|n| backoff.delay_after(n).as_secs()).collect();
        assert_eq!(delays, [10, 20, 40, 80, 160, 300, 300]);
        assert_eq!(backoff.delay_after(u32::MAX), DEFAULT_MAX);
    }

    #[test]
    fn respects_custom_limits() {
        let backoff = Backoff::new(Duration::from_mins(5), Duration::from_hours(12));
        let delays: Vec<u64> = (0..3).map(|n| backoff.delay_after(n).as_secs()).collect();
        assert_eq!(delays, [300, 600, 1200]);
        assert_eq!(backoff.delay_after(20), Duration::from_hours(12));
    }

    #[test]
    fn waits_after_failure_until_reset() {
        let now = Instant::now();
        let mut backoff = Backoff::default();
        assert!(backoff.ready(now));
        let delay = backoff.fail(now);
        assert!(!backoff.ready(now));
        assert_eq!(backoff.next_attempt(), Some(now + delay));
        assert!(backoff.ready(now + delay));
        backoff.reset();
        assert!(backoff.ready(now));
        assert_eq!(backoff.next_attempt(), None);
    }
}
