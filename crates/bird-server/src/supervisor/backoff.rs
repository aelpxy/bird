use std::time::{Duration, Instant};

const BASE_DELAY: Duration = Duration::from_secs(10);
const MAX_DELAY: Duration = Duration::from_mins(5);

#[derive(Debug, Default)]
pub(super) struct Backoff {
    failures: u32,
    next_attempt: Option<Instant>,
}

impl Backoff {
    pub(super) fn ready(&self, now: Instant) -> bool {
        self.next_attempt.is_none_or(|at| now >= at)
    }

    pub(super) fn fail(&mut self, now: Instant) -> Duration {
        let delay = delay_after(self.failures);
        self.failures = self.failures.saturating_add(1);
        self.next_attempt = Some(now + delay);
        delay
    }

    pub(super) fn reset(&mut self) {
        *self = Self::default();
    }
}

fn delay_after(failures: u32) -> Duration {
    BASE_DELAY
        .saturating_mul(2_u32.saturating_pow(failures))
        .min(MAX_DELAY)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn doubles_up_to_max() {
        let delays: Vec<u64> = (0..7).map(|n| delay_after(n).as_secs()).collect();
        assert_eq!(delays, [10, 20, 40, 80, 160, 300, 300]);
        assert_eq!(delay_after(u32::MAX), MAX_DELAY);
    }

    #[test]
    fn waits_after_failure_until_reset() {
        let now = Instant::now();
        let mut backoff = Backoff::default();
        assert!(backoff.ready(now));
        let delay = backoff.fail(now);
        assert!(!backoff.ready(now));
        assert!(backoff.ready(now + delay));
        backoff.reset();
        assert!(backoff.ready(now));
    }
}
