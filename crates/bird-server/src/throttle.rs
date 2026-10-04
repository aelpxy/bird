use std::collections::HashMap;
use std::sync::{Mutex, PoisonError};
use std::time::{Duration, Instant};

// failures before sign-ins slow down, then each further one doubles the wait; an address is shared
// by everyone behind the same network, so it gets more
const FREE_FAILURES_PER_USER: u32 = 5;
const FREE_FAILURES_PER_ADDRESS: u32 = 20;
const FIRST_WAIT: Duration = Duration::from_secs(30);
const LONGEST_WAIT: Duration = Duration::from_mins(15);
// a key quiet this long starts over, and is dropped when the map grows
const FORGET_AFTER: Duration = Duration::from_hours(1);
const MAX_KEYS: usize = 10_000;

struct Failures {
    count: u32,
    last: Instant,
}

// what failures are counted against
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub(crate) enum Key {
    User(String),
    Address(String),
}

impl Key {
    const fn free_failures(&self) -> u32 {
        match self {
            Self::User(_) => FREE_FAILURES_PER_USER,
            Self::Address(_) => FREE_FAILURES_PER_ADDRESS,
        }
    }
}

// counts failed sign-ins per user and per address, so guessing gets slow
#[derive(Default)]
pub(crate) struct LoginThrottle {
    failures: Mutex<HashMap<Key, Failures>>,
}

impl LoginThrottle {
    // how long until any of `keys` may try again
    pub(crate) fn wait(&self, keys: &[Key], now: Instant) -> Option<Duration> {
        let failures = self.failures.lock().unwrap_or_else(PoisonError::into_inner);
        keys.iter()
            .filter_map(|key| Some((key, failures.get(key)?)))
            .filter_map(|(key, entry)| {
                let until = entry.last + penalty(entry.count, key.free_failures())?;
                (until > now).then(|| until - now)
            })
            .max()
    }

    pub(crate) fn failed(&self, keys: &[Key], now: Instant) {
        let mut failures = self.failures.lock().unwrap_or_else(PoisonError::into_inner);
        if failures.len() >= MAX_KEYS {
            failures.retain(|_, entry| now.duration_since(entry.last) < FORGET_AFTER);
        }
        for key in keys {
            let entry = failures.entry(key.clone()).or_insert(Failures {
                count: 0,
                last: now,
            });
            if now.duration_since(entry.last) >= FORGET_AFTER {
                entry.count = 0;
            }
            entry.count = entry.count.saturating_add(1);
            entry.last = now;
        }
    }

    pub(crate) fn succeeded(&self, key: &Key) {
        self.failures
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .remove(key);
    }
}

fn penalty(count: u32, free: u32) -> Option<Duration> {
    let over = count.checked_sub(free)?;
    let doubled = FIRST_WAIT.saturating_mul(2_u32.saturating_pow(over.min(16)));
    Some(doubled.min(LONGEST_WAIT))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn slows_down_after_a_few_failures() {
        let throttle = LoginThrottle::default();
        let user = Key::User("ada".to_owned());
        let address = Key::Address("10.0.0.1".to_owned());
        let keys = [user.clone(), address.clone()];
        let start = Instant::now();
        for _ in 0..FREE_FAILURES_PER_USER - 1 {
            throttle.failed(&keys, start);
        }
        assert_eq!(throttle.wait(&keys, start), None);
        throttle.failed(&keys, start);
        assert_eq!(throttle.wait(&keys, start), Some(FIRST_WAIT));
        throttle.failed(&keys, start);
        assert_eq!(throttle.wait(&keys, start), Some(FIRST_WAIT * 2));
        assert_eq!(throttle.wait(&keys, start + FIRST_WAIT * 2), None);

        // the address still has room, and a success forgets the user's failures
        assert_eq!(throttle.wait(std::slice::from_ref(&address), start), None);
        throttle.succeeded(&user);
        assert_eq!(throttle.wait(std::slice::from_ref(&user), start), None);
        assert_eq!(
            penalty(FREE_FAILURES_PER_USER + 30, FREE_FAILURES_PER_USER),
            Some(LONGEST_WAIT)
        );
    }
}
