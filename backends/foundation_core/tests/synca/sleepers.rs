//! Tests for `DurationWaker`, `Sleepers` and `DurationStore`.

use foundation_core::synca::{
    DurationStore, DurationWaker, Entry, Sleepers, Timeable, Timing, Waiter, Waker,
};
use std::sync::Arc;
use std::thread;
use std::time;

#[derive(Debug, Clone)]
struct MockWaker {
    woken: Arc<Mutex<bool>>,
}

impl MockWaker {
    fn new() -> Self {
        Self {
            woken: Arc::new(Mutex::new(false)),
        }
    }

    #[allow(dead_code)]
    fn was_woken(&self) -> bool {
        *self.woken.lock().unwrap()
    }
}

impl Waker for MockWaker {
    fn wake(&self) {
        *self.woken.lock().unwrap() = true;
    }
}

#[derive(Debug)]
struct MockSleeper {
    ready: bool,
}

impl Waiter for MockSleeper {
    fn is_ready(&self, _dur: Option<time::Duration>) -> bool {
        self.ready
    }
}

impl Timeable for MockSleeper {
    fn remaining_duration(&self) -> Option<time::Duration> {
        if self.ready {
            Some(time::Duration::ZERO)
        } else {
            Some(time::Duration::from_secs(1))
        }
    }
}

use std::sync::Mutex;

#[test]
fn test_duration_waker_new() {
    let handle = MockWaker::new();
    let start = time::Instant::now();
    let duration = time::Duration::from_millis(100);

    let waker = DurationWaker::new(handle, start, duration);

    assert!(waker.remaining().is_some());
}

#[test]
fn test_duration_waker_from_now() {
    let handle = MockWaker::new();
    let duration = time::Duration::from_millis(100);

    let waker = DurationWaker::from_now(handle, duration);

    assert!(waker.remaining().is_some());
}

#[test]
fn test_duration_waker_is_ready_when_not_elapsed() {
    let handle = MockWaker::new();
    let duration = time::Duration::from_secs(10);

    let waker = DurationWaker::from_now(handle, duration);

    assert_eq!(Some(false), waker.try_is_ready());
}

#[test]
fn test_duration_waker_is_ready_when_elapsed() {
    let handle = MockWaker::new();
    let duration = time::Duration::from_millis(10);

    let waker = DurationWaker::from_now(handle, duration);

    thread::sleep(time::Duration::from_millis(50));

    assert_eq!(Some(true), waker.try_is_ready());
}

#[test]
fn test_duration_waker_deadline() {
    let handle = MockWaker::new();
    let duration = time::Duration::from_millis(100);
    let before = time::Instant::now();

    let waker = DurationWaker::from_now(handle, duration);
    let deadline = waker.deadline();

    let expected = before + duration;
    // Allow for small timing differences
    assert!(deadline >= expected - time::Duration::from_millis(10));
    assert!(deadline <= expected + time::Duration::from_millis(10));
}

#[test]
fn test_sleepers_insert_and_remove() {
    let sleepers = Sleepers::new();
    let entry = Entry { id: 1, gen: 0 };
    let sleeper = MockSleeper { ready: false };

    sleepers.insert(entry, sleeper);
    assert_eq!(sleepers.count(), 1);

    let removed = sleepers.remove(&entry);
    assert!(removed.is_some());
    assert_eq!(sleepers.count(), 0);
}

#[test]
fn test_sleepers_remove_nonexistent() {
    let sleepers: Sleepers<MockSleeper> = Sleepers::new();
    let entry = Entry { id: 1, gen: 0 };

    let removed = sleepers.remove(&entry);
    assert!(removed.is_none());
}

#[test]
fn test_sleepers_get_matured_returns_ready_sleepers() {
    let sleepers = Sleepers::new();
    let entry1 = Entry { id: 1, gen: 0 };
    let entry2 = Entry { id: 2, gen: 0 };

    sleepers.insert(entry1, MockSleeper { ready: true });
    sleepers.insert(entry2, MockSleeper { ready: false });

    let matured = sleepers.get_matured();
    assert_eq!(matured.len(), 1);
    assert_eq!(sleepers.count(), 1); // Only the not-ready one remains
}

#[test]
fn test_sleepers_get_matured_empty_when_none_ready() {
    let sleepers = Sleepers::new();
    let entry1 = Entry { id: 1, gen: 0 };
    let entry2 = Entry { id: 2, gen: 0 };

    sleepers.insert(entry1, MockSleeper { ready: false });
    sleepers.insert(entry2, MockSleeper { ready: false });

    let matured = sleepers.get_matured();
    assert!(matured.is_empty());
    assert_eq!(sleepers.count(), 2);
}

#[test]
fn test_sleepers_get_matured_empty_when_no_sleepers() {
    let sleepers: Sleepers<MockSleeper> = Sleepers::new();
    let matured = sleepers.get_matured();
    assert!(matured.is_empty());
}

#[test]
fn test_sleepers_count() {
    let sleepers = Sleepers::new();
    assert_eq!(sleepers.count(), 0);

    sleepers.insert(Entry { id: 1, gen: 0 }, MockSleeper { ready: false });
    assert_eq!(sleepers.count(), 1);

    sleepers.insert(Entry { id: 2, gen: 0 }, MockSleeper { ready: false });
    assert_eq!(sleepers.count(), 2);

    let _ = sleepers.remove(&Entry { id: 1, gen: 0 });
    assert_eq!(sleepers.count(), 1);
}

#[test]
fn test_sleepers_has_pending_tasks() {
    let sleepers = Sleepers::new();
    assert!(!sleepers.has_pending_tasks());

    sleepers.insert(Entry { id: 1, gen: 0 }, MockSleeper { ready: false });
    assert!(sleepers.has_pending_tasks());

    let _ = sleepers.remove(&Entry { id: 1, gen: 0 });
    assert!(!sleepers.has_pending_tasks());
}

#[test]
fn test_sleepers_update_existing() {
    let sleepers = Sleepers::new();
    let entry = Entry { id: 1, gen: 0 };

    sleepers.insert(entry, MockSleeper { ready: false });
    let old = sleepers.update(&entry, MockSleeper { ready: true });

    assert!(old.is_some());
    let matured = sleepers.get_matured();
    assert_eq!(matured.len(), 1);
}

#[test]
fn test_sleepers_update_nonexistent() {
    let sleepers = Sleepers::new();
    let entry = Entry { id: 1, gen: 0 };

    let old = sleepers.update(&entry, MockSleeper { ready: true });
    assert!(old.is_none());
    assert_eq!(sleepers.count(), 1);
}

#[test]
fn test_sleepers_clone() {
    let sleepers = Sleepers::new();
    let entry = Entry { id: 1, gen: 0 };

    sleepers.insert(entry, MockSleeper { ready: false });
    let cloned = sleepers.clone();

    assert_eq!(cloned.count(), 1);
    let _ = cloned.remove(&entry);
    assert_eq!(sleepers.count(), 0); // Shared state
}

#[test]
fn test_sleepers_default() {
    let sleepers: Sleepers<MockSleeper> = Sleepers::default();
    assert_eq!(sleepers.count(), 0);
    assert!(!sleepers.has_pending_tasks());
}

#[test]
fn test_sleepers_timing_min_duration() {
    let sleepers = Sleepers::new();
    let entry = Entry { id: 1, gen: 0 };

    sleepers.insert(entry, MockSleeper { ready: false });
    let min = sleepers.min_duration();
    assert!(min.is_some());
}

#[test]
fn test_sleepers_timing_max_duration() {
    let sleepers = Sleepers::new();
    let entry = Entry { id: 1, gen: 0 };

    sleepers.insert(entry, MockSleeper { ready: false });
    let max = sleepers.max_duration();
    assert!(max.is_some());
}

/// WHY: the local executor sleeps for `min_duration()` so the NEAREST
/// sleeper wakes on time; returning the furthest made short timers fire late.
/// WHAT: with a ready (0s) and a pending (1s) sleeper, min is 0s and max is 1s.
#[test]
fn test_sleepers_timing_min_and_max_differ() {
    let sleepers = Sleepers::new();
    sleepers.insert(Entry { id: 1, gen: 0 }, MockSleeper { ready: true });
    sleepers.insert(Entry { id: 2, gen: 0 }, MockSleeper { ready: false });

    assert_eq!(sleepers.min_duration(), Some(time::Duration::ZERO));
    assert_eq!(sleepers.max_duration(), Some(time::Duration::from_secs(1)));
}

#[test]
fn test_sleepers_minimum_deadline() {
    let sleepers = Sleepers::new();
    let entry1 = Entry { id: 1, gen: 0 };
    let entry2 = Entry { id: 2, gen: 0 };

    sleepers.insert(entry1, MockSleeper { ready: false });
    sleepers.insert(entry2, MockSleeper { ready: true });

    let deadline = sleepers.minimum_deadline();
    // MockSleeper doesn't have a real deadline, just verifies the API works
    assert!(deadline.is_some());
}

#[test]
fn test_sleepers_time_until_next() {
    let sleepers = Sleepers::new();
    let entry = Entry { id: 1, gen: 0 };

    sleepers.insert(entry, MockSleeper { ready: false });
    let remaining = sleepers.time_until_next();
    assert!(remaining.is_some());
}

#[test]
fn test_sleepers_time_until_next_when_past_deadline() {
    let sleepers = Sleepers::new();
    let entry = Entry { id: 1, gen: 0 };

    // A ready sleeper has zero remaining time
    sleepers.insert(entry, MockSleeper { ready: true });
    let remaining = sleepers.time_until_next();
    assert_eq!(remaining, Some(time::Duration::ZERO));
}

#[test]
fn test_sleepers_timing_min_duration_empty() {
    let sleepers: Sleepers<MockSleeper> = Sleepers::new();
    assert!(sleepers.min_duration().is_none());
}

#[test]
fn test_sleepers_timing_max_duration_empty() {
    let sleepers: Sleepers<MockSleeper> = Sleepers::new();
    assert!(sleepers.max_duration().is_none());
}

#[test]
fn test_sleepers_minimum_deadline_empty() {
    let sleepers: Sleepers<MockSleeper> = Sleepers::new();
    assert!(sleepers.minimum_deadline().is_none());
}

#[test]
fn test_sleepers_time_until_next_empty() {
    let sleepers: Sleepers<MockSleeper> = Sleepers::new();
    assert!(sleepers.time_until_next().is_none());
}

#[test]
fn test_duration_store_insert() {
    let store = DurationStore::new();
    let handle = MockWaker::new();
    let waker = DurationWaker::from_now(handle, time::Duration::from_millis(100));

    let entry = store.insert(waker);
    assert!(store.has_pending_tasks());
    assert_eq!(store.count(), 1);

    // Can retrieve by the returned entry
    let removed = store.remove(&entry);
    assert!(removed.is_some());
}

#[test]
fn test_duration_store_update() {
    let store = DurationStore::new();
    let handle = MockWaker::new();
    let waker = DurationWaker::from_now(handle, time::Duration::from_millis(100));

    let entry = store.insert(waker);
    let new_waker = DurationWaker::from_now(MockWaker::new(), time::Duration::from_millis(200));

    let old = store.update(&entry, new_waker);
    assert!(old.is_some());
}

#[test]
fn test_duration_store_get_matured() {
    let store = DurationStore::new();
    let handle = MockWaker::new();
    let waker = DurationWaker::from_now(handle, time::Duration::from_millis(10));

    store.insert(waker);

    // Wait for the waker to mature
    thread::sleep(time::Duration::from_millis(50));

    let matured = store.get_matured();
    assert_eq!(matured.len(), 1);
}

#[test]
fn test_duration_store_get_matured_empty_when_not_ready() {
    let store = DurationStore::new();
    let handle = MockWaker::new();
    let waker = DurationWaker::from_now(handle, time::Duration::from_secs(10));

    store.insert(waker);

    let matured = store.get_matured();
    assert!(matured.is_empty());
}

#[test]
fn test_duration_store_min_duration() {
    let store = DurationStore::new();
    let handle1 = MockWaker::new();
    let handle2 = MockWaker::new();

    store.insert(DurationWaker::from_now(
        handle1,
        time::Duration::from_millis(100),
    ));
    store.insert(DurationWaker::from_now(
        handle2,
        time::Duration::from_millis(50),
    ));

    let min = store.min_duration();
    assert!(min.is_some());
    // Should be around 50ms
    assert!(min.unwrap() <= time::Duration::from_millis(60));
}

#[test]
fn test_duration_store_max_duration() {
    let store = DurationStore::new();
    let handle1 = MockWaker::new();
    let handle2 = MockWaker::new();

    store.insert(DurationWaker::from_now(
        handle1,
        time::Duration::from_millis(100),
    ));
    store.insert(DurationWaker::from_now(
        handle2,
        time::Duration::from_millis(50),
    ));

    let max = store.max_duration();
    assert!(max.is_some());
    // Should be around 100ms
    assert!(max.unwrap() <= time::Duration::from_millis(100));
}

#[test]
fn test_duration_store_min_duration_empty() {
    let store: DurationStore<MockWaker> = DurationStore::new();
    assert!(store.min_duration().is_none());
}

#[test]
fn test_duration_store_max_duration_empty() {
    let store: DurationStore<MockWaker> = DurationStore::new();
    assert!(store.max_duration().is_none());
}
