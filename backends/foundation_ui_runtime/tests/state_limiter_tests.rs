use core::time::Duration;

use foundation_ui_runtime::state::{
    FailureKey, FailureLimiter, FailureLimits, InMemoryFailureLimiter, StateError,
};

use super::support::{test_ctx, WALL};

fn key(session: &str, address: &str) -> FailureKey {
    FailureKey {
        session: session.into(),
        address: address.into(),
    }
}

#[test]
fn block_after_max_failures() {
    let (ctx, _) = test_ctx(WALL);
    let limits = FailureLimits::default();
    assert_eq!(limits.failure_count(), 5);
    assert_eq!(limits.window_value(), Duration::from_secs(600));
    assert_eq!(limits.block_duration(), Duration::from_secs(600));
    let limiter = InMemoryFailureLimiter::new(limits);
    let key = key("sess-A", "10.0.0.1");
    for _ in 0..limits.failure_count() {
        assert_eq!(limiter.check(&ctx, &key), Ok(()));
        limiter.record_failure(&ctx, &key);
    }
    assert_eq!(limiter.check(&ctx, &key), Err(StateError::RateLimited));
}

#[test]
fn other_key_is_not_blocked() {
    let (ctx, _) = test_ctx(WALL);
    let limiter = InMemoryFailureLimiter::new(FailureLimits::default().max_failures(1));
    limiter.record_failure(&ctx, &key("sess-A", "10.0.0.1"));
    assert_eq!(limiter.check(&ctx, &key("sess-B", "10.0.0.2")), Ok(()));
    assert_eq!(limiter.check(&ctx, &key("sess-A", "10.0.0.2")), Ok(()));
    assert_eq!(limiter.check(&ctx, &key("sess-B", "10.0.0.1")), Ok(()));
}

#[test]
fn block_ends_after_block_time() {
    let (ctx, clock) = test_ctx(WALL);
    let limiter = InMemoryFailureLimiter::new(FailureLimits::default().max_failures(1));
    let key = key("sess-A", "10.0.0.1");
    limiter.record_failure(&ctx, &key);
    clock.set_elapsed(599);
    assert_eq!(limiter.check(&ctx, &key), Err(StateError::RateLimited));
    limiter.record_failure(&ctx, &key);
    clock.set_elapsed(600);
    assert_eq!(limiter.check(&ctx, &key), Ok(()));
}

#[test]
fn window_resets_count() {
    let (ctx, clock) = test_ctx(WALL);
    let limits = FailureLimits::default()
        .max_failures(2)
        .window(Duration::from_secs(3))
        .block_for(Duration::from_secs(7));
    let limiter = InMemoryFailureLimiter::new(limits);
    let key = key("sess-A", "10.0.0.1");
    limiter.record_failure(&ctx, &key);
    clock.set_elapsed(3);
    limiter.record_failure(&ctx, &key);
    assert_eq!(limiter.check(&ctx, &key), Ok(()));
    limiter.record_failure(&ctx, &key);
    assert_eq!(limiter.check(&ctx, &key), Err(StateError::RateLimited));
    clock.set_elapsed(10);
    assert_eq!(limiter.check(&ctx, &key), Ok(()));
}
