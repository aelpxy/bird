use std::collections::HashMap;
use std::net::IpAddr;
use std::sync::{Mutex, PoisonError};
use std::time::{Duration, Instant};

const SHARDS: u8 = 32;
// bounds memory at about 32 * 4096 visitors per app; past that new visitors go untracked
const MAX_VISITORS_PER_SHARD: usize = 4096;
const NANOS_PER_SEC: u64 = 1_000_000_000;

/// Requests per visitor with a burst, as a generic cell rate: one timestamp per visitor
#[derive(Debug)]
pub struct RateLimiter {
    interval_ns: u64,
    tolerance_ns: u64,
    epoch: Instant,
    shards: Box<[Mutex<HashMap<u128, u64>>]>,
}

impl RateLimiter {
    #[must_use]
    pub fn new(per_second: u32, burst: u32) -> Self {
        let interval_ns = NANOS_PER_SEC / u64::from(per_second.max(1));
        Self {
            interval_ns,
            tolerance_ns: interval_ns.saturating_mul(u64::from(burst.max(1))),
            epoch: Instant::now(),
            shards: (0..SHARDS).map(|_| Mutex::default()).collect(),
        }
    }

    /// Counts a request, or says how long the visitor must wait
    pub fn check(&self, client: IpAddr) -> Result<(), Duration> {
        self.check_at(client, self.now_ns())
    }

    fn check_at(&self, client: IpAddr, now: u64) -> Result<(), Duration> {
        let key = visitor(client);
        let shard = usize::try_from(key % u128::from(SHARDS)).unwrap_or_default();
        let Some(shard) = self.shards.get(shard) else {
            return Ok(());
        };
        let mut visitors = shard.lock().unwrap_or_else(PoisonError::into_inner);
        let arrival = visitors.get(&key).copied().unwrap_or(now).max(now);
        let next = arrival.saturating_add(self.interval_ns);
        let allowed_at = next.saturating_sub(self.tolerance_ns);
        if allowed_at > now {
            return Err(Duration::from_nanos(allowed_at - now));
        }
        if visitors.len() >= MAX_VISITORS_PER_SHARD && !visitors.contains_key(&key) {
            // visitors whose bucket refilled carry no state worth keeping
            visitors.retain(|_, &mut at| at > now);
            if visitors.len() >= MAX_VISITORS_PER_SHARD {
                return Ok(());
            }
        }
        visitors.insert(key, next);
        Ok(())
    }

    fn now_ns(&self) -> u64 {
        u64::try_from(self.epoch.elapsed().as_nanos()).unwrap_or(u64::MAX)
    }
}

// one IPv6 visitor usually holds a whole /64, so that is what is counted
fn visitor(client: IpAddr) -> u128 {
    match client {
        IpAddr::V4(v4) => u128::from(v4.to_bits()),
        IpAddr::V6(v6) => v6.to_bits() & !u128::from(u64::MAX) | (1 << 127),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const SECOND: u64 = NANOS_PER_SEC;

    fn ip(raw: &str) -> IpAddr {
        raw.parse().unwrap()
    }

    #[test]
    fn allows_the_burst_then_the_sustained_rate() {
        let limiter = RateLimiter::new(2, 3);
        let visitor = ip("203.0.113.7");
        for _ in 0..3 {
            assert!(limiter.check_at(visitor, 10 * SECOND).is_ok());
        }
        let wait = limiter.check_at(visitor, 10 * SECOND).unwrap_err();
        assert_eq!(wait, Duration::from_millis(500));
        assert!(limiter.check_at(visitor, 10 * SECOND + SECOND / 2).is_ok());
        assert!(limiter.check_at(visitor, 10 * SECOND + SECOND / 2).is_err());
        // a quiet visitor gets the whole burst back
        for _ in 0..3 {
            assert!(limiter.check_at(visitor, 20 * SECOND).is_ok());
        }
    }

    #[test]
    fn counts_visitors_apart_and_ipv6_per_network() {
        let limiter = RateLimiter::new(1, 1);
        assert!(limiter.check_at(ip("203.0.113.7"), SECOND).is_ok());
        assert!(limiter.check_at(ip("203.0.113.8"), SECOND).is_ok());
        assert!(limiter.check_at(ip("203.0.113.7"), SECOND).is_err());
        assert!(limiter.check_at(ip("2001:db8::1"), SECOND).is_ok());
        assert!(limiter.check_at(ip("2001:db8::ffff"), SECOND).is_err());
        assert!(limiter.check_at(ip("2001:db8:0:1::1"), SECOND).is_ok());
    }
}
