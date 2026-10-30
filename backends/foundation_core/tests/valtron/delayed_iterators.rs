//! Tests for `SleepIterator` (a `DelayedIterator`).

use foundation_core::valtron::delayed_iterators::{
    remaining, Delayed, DelayedIterator, SleepIterator,
};
use std::{
    thread,
    time::{Duration, Instant},
};

#[test]
fn zero_duration_sleep_iterator_finishes_immediately() {
    let now = Instant::now();
    let wait = Duration::from_secs(0);
    let mut sleeper = SleepIterator::new(now, wait, ());
    assert_eq!(sleeper.next(), Some(Delayed::Done(())));
}

#[test]
fn remaining_classifies_a_deadline_at_the_boundary() {
    let now = Instant::now();

    // Deadline is exactly now: done.
    assert_eq!(remaining(Some(now), now), None);

    // Deadline is in the future: still pending, with the exact time left.
    assert_eq!(
        remaining(Some(now + Duration::from_millis(1)), now),
        Some(Duration::from_millis(1))
    );

    // Deadline is in the past: done.
    assert_eq!(remaining(Some(now - Duration::from_millis(1)), now), None);

    // Deadline overflowed: done.
    assert_eq!(remaining(None, now), None);
}

#[test]
fn can_sleep_thread_and_get_final() {
    let now = Instant::now();
    let wait = Duration::from_secs(3);
    let mut sleeper = SleepIterator::new(now, wait, ());

    assert!(matches!(sleeper.next(), Some(Delayed::Pending(_, _, _))));
    assert!(matches!(sleeper.next(), Some(Delayed::Pending(_, _, _))));
    assert!(matches!(sleeper.next(), Some(Delayed::Pending(_, _, _))));

    thread::sleep(Duration::from_secs(1));

    assert!(matches!(sleeper.next(), Some(Delayed::Pending(_, _, _))));

    thread::sleep(Duration::from_secs(2));
    assert_eq!(sleeper.next(), Some(Delayed::Done(())));
}
