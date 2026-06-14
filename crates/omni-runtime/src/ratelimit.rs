//! Per-session input rate limiting: a token bucket that drops floods instead
//! of queueing them, per the security rules.

use std::time::Instant;

/// Sustained events per second a session may deliver. Generous: real input
/// peaks around a few hundred events per second.
pub const EVENTS_PER_SECOND: u32 = 2_000;
/// Burst headroom on top of the sustained rate.
pub const BURST: u32 = 4_000;

/// A token bucket. Each event costs one token; an empty bucket drops events.
#[derive(Debug)]
pub struct RateLimiter {
    capacity: f64,
    tokens: f64,
    refill_per_second: f64,
    last: Instant,
}

impl RateLimiter {
    pub fn new(events_per_second: u32, burst: u32) -> Self {
        Self {
            capacity: burst as f64,
            tokens: burst as f64,
            refill_per_second: events_per_second as f64,
            last: Instant::now(),
        }
    }

    /// Spends one token if available. `false` means: drop the event.
    pub fn allow(&mut self) -> bool {
        self.allow_at(Instant::now())
    }

    fn allow_at(&mut self, now: Instant) -> bool {
        let elapsed = now.duration_since(self.last).as_secs_f64();
        self.last = now;
        self.tokens = (self.tokens + elapsed * self.refill_per_second).min(self.capacity);
        if self.tokens >= 1.0 {
            self.tokens -= 1.0;
            true
        } else {
            false
        }
    }
}

impl Default for RateLimiter {
    fn default() -> Self {
        Self::new(EVENTS_PER_SECOND, BURST)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::Duration;

    #[test]
    fn allows_up_to_the_burst_then_drops() {
        let mut limiter = RateLimiter::new(10, 5);
        let now = Instant::now();

        for _ in 0..5 {
            assert!(limiter.allow_at(now));
        }
        assert!(!limiter.allow_at(now), "burst exhausted, must drop");
    }

    #[test]
    fn refills_over_time() {
        let mut limiter = RateLimiter::new(10, 5);
        let start = Instant::now();
        for _ in 0..5 {
            assert!(limiter.allow_at(start));
        }
        assert!(!limiter.allow_at(start));

        // One second refills ten tokens (capped at the burst of five).
        let later = start + Duration::from_secs(1);
        for _ in 0..5 {
            assert!(limiter.allow_at(later));
        }
        assert!(!limiter.allow_at(later));
    }
}
