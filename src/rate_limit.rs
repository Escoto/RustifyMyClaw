use std::collections::{HashMap, VecDeque};
use std::sync::Mutex;
use std::time::{Duration, Instant};

/// Result of a rate-limit check for a single user.
#[derive(Debug, PartialEq)]
pub enum RateLimitResult {
    /// Request is within the allowed window — proceed.
    Allowed,
    /// User has exceeded their quota. Retry after the given `Duration`.
    LimitedFor(Duration),
}

/// Per-user sliding-window rate limiter.
///
/// Each user maintains a deque of `Instant`s representing their recent requests.
/// On every check, entries older than the window are pruned, then the count is tested
/// against `max_requests`. Checks consume a slot (push an `Instant`) on `Allowed`.
///
/// `std::sync::Mutex` is intentional: the critical section is prune + push, which
/// is sub-microsecond. Holding a sync mutex across this does not block the runtime.
pub struct RateLimiter {
    state: Mutex<HashMap<String, VecDeque<Instant>>>,
    max_requests: u32,
    window: Duration,
}

impl RateLimiter {
    pub fn new(max_requests: u32, window: Duration) -> Self {
        Self {
            state: Mutex::new(HashMap::new()),
            max_requests,
            window,
        }
    }

    /// Check whether `user_id` is within their rate limit.
    ///
    /// Returns `Allowed` and records the request, or `LimitedFor(remaining)` if the
    /// limit is already reached. `remaining` is the time until the oldest request in
    /// the window expires, freeing a slot.
    pub fn check(&self, user_id: &str) -> RateLimitResult {
        let now = Instant::now();
        let mut state = self.state.lock().unwrap();
        let deque = state.entry(user_id.to_string()).or_default();

        // Prune entries that have fallen outside the window.
        while deque
            .front()
            .map(|t| now.duration_since(*t) > self.window)
            .unwrap_or(false)
        {
            deque.pop_front();
        }

        if deque.len() >= self.max_requests as usize {
            // If max_requests is 0 or the deque is full, compute remaining wait.
            // When deque is empty (max_requests == 0), there is no expiring entry, so
            // return a zero-duration limit signal.
            let remaining = deque
                .front()
                .map(|oldest| {
                    let elapsed = now.duration_since(*oldest);
                    self.window.saturating_sub(elapsed)
                })
                .unwrap_or(Duration::ZERO);
            RateLimitResult::LimitedFor(remaining)
        } else {
            deque.push_back(now);
            RateLimitResult::Allowed
        }
    }
}

#[cfg(test)]
#[path = "tests/rate_limit_test.rs"]
mod tests;
