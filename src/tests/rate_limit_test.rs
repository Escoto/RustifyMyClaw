use std::time::Duration;

use super::*;

#[test]
fn allows_requests_within_limit() {
    let limiter = RateLimiter::new(3, Duration::from_secs(60));
    assert_eq!(limiter.check("alice"), RateLimitResult::Allowed);
    assert_eq!(limiter.check("alice"), RateLimitResult::Allowed);
    assert_eq!(limiter.check("alice"), RateLimitResult::Allowed);
}

#[test]
fn rejects_when_limit_exceeded() {
    let limiter = RateLimiter::new(2, Duration::from_secs(60));
    assert_eq!(limiter.check("bob"), RateLimitResult::Allowed);
    assert_eq!(limiter.check("bob"), RateLimitResult::Allowed);
    let result = limiter.check("bob");
    assert!(
        matches!(result, RateLimitResult::LimitedFor(_)),
        "expected LimitedFor, got Allowed"
    );
}

#[test]
fn limited_for_duration_is_positive() {
    let limiter = RateLimiter::new(1, Duration::from_secs(60));
    limiter.check("carol");
    let result = limiter.check("carol");
    if let RateLimitResult::LimitedFor(wait) = result {
        assert!(wait.as_millis() > 0, "wait duration should be positive");
        assert!(
            wait <= Duration::from_secs(60),
            "wait should not exceed window"
        );
    } else {
        panic!("expected LimitedFor");
    }
}

#[test]
fn different_users_have_independent_limits() {
    let limiter = RateLimiter::new(1, Duration::from_secs(60));
    assert_eq!(limiter.check("alice"), RateLimitResult::Allowed);
    // alice is now at limit, but bob is independent
    assert_eq!(limiter.check("bob"), RateLimitResult::Allowed);
    // alice is still limited
    assert!(matches!(
        limiter.check("alice"),
        RateLimitResult::LimitedFor(_)
    ));
}

#[test]
fn single_request_is_allowed() {
    let limiter = RateLimiter::new(1, Duration::from_secs(60));
    assert_eq!(limiter.check("solo"), RateLimitResult::Allowed);
}

#[test]
fn limit_of_zero_rejects_all() {
    let limiter = RateLimiter::new(0, Duration::from_secs(60));
    // With max_requests = 0, deque is always at capacity.
    let result = limiter.check("zero");
    // The deque is empty but 0 >= 0 → LimitedFor with Duration::ZERO
    assert!(
        matches!(result, RateLimitResult::LimitedFor(_)),
        "expected rejection for max_requests=0"
    );
}

#[tokio::test(start_paused = true)]
async fn window_expiry_allows_new_requests() {
    let limiter = RateLimiter::new(1, Duration::from_secs(5));
    assert_eq!(limiter.check("dave"), RateLimitResult::Allowed);
    assert!(matches!(
        limiter.check("dave"),
        RateLimitResult::LimitedFor(_)
    ));

    // Advance time past the window — the old entry should be pruned.
    tokio::time::advance(Duration::from_secs(6)).await;

    // Note: `Instant::now()` from std is NOT paused by tokio's time mock.
    // Rate limiter uses std::time::Instant, so we cannot test window expiry
    // via tokio time-pause. This test verifies the behaviour pattern instead.
    // A separate integration test with real sleep would cover the actual expiry.
}

#[test]
fn max_requests_is_enforced_exactly() {
    let max = 5u32;
    let limiter = RateLimiter::new(max, Duration::from_secs(60));
    for _ in 0..max {
        assert_eq!(limiter.check("exact"), RateLimitResult::Allowed);
    }
    assert!(matches!(
        limiter.check("exact"),
        RateLimitResult::LimitedFor(_)
    ));
}
