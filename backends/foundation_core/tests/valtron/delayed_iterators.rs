//! Tests for `SleepIterator` (a `DelayedIterator`).

use foundation_core::valtron::delayed_iterators::{Delayed, DelayedIterator, SleepIterator};
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
