//! Tests for `IdleMan` / `SleepyMan` idle and backoff accounting.

use foundation_core::retries::ExponentialBackoffDecider;
use foundation_core::synca::{IdleMan, SleepyMan};
use std::time;

#[test]
fn can_increment_for_max_allowed_idleness() {
    let sl = SleepyMan::new(3, ExponentialBackoffDecider::default());
    let mut idle = IdleMan::new(3, None, sl);

    assert_eq!(idle.increment(), None);
    assert_eq!(idle.increment(), None);
    assert_eq!(idle.increment(), None);
    assert_eq!(idle.increment(), None);
}

#[test]
fn increment_to_max_leads_to_sleep_duration() {
    let sl = SleepyMan::new(3, ExponentialBackoffDecider::default());
    let mut idle = IdleMan::new(3, None, sl);

    assert!(idle.increment().is_none());
    assert!(idle.increment().is_none());
    assert!(idle.increment().is_none());
    assert!(idle.increment().is_none());

    assert!(idle.increment().is_some());
    assert!(idle.increment().is_some());
    assert!(idle.increment().is_some());

    // end of random sleep
    assert!(idle.increment().is_none());
}

#[test]
fn increment_to_max_resets_sleeper() {
    let dfs = Some(time::Duration::from_millis(100));
    let sl = SleepyMan::new(3, ExponentialBackoffDecider::default());
    let mut idle = IdleMan::new(3, dfs, sl);

    assert_eq!(idle.increment(), dfs);
    assert_eq!(idle.increment(), dfs);
    assert_eq!(idle.increment(), dfs);
    assert_eq!(idle.increment(), dfs);

    assert!(idle.increment().is_some());
    assert!(idle.increment().is_some());
    assert!(idle.increment().is_some());

    assert_eq!(idle.increment(), dfs);
    assert_eq!(idle.increment(), dfs);
    assert_eq!(idle.increment(), dfs);
    assert_eq!(idle.increment(), dfs);

    assert!(idle.increment().is_some());
    assert!(idle.increment().is_some());
    assert!(idle.increment().is_some());
    assert!(idle.increment().is_some());
}
