// Implements a Sleeper register directory where a given set of
// Wakeable primitive can be notified after some expired duration
// registered with.

use std::{collections::HashMap, sync::Arc, time};

use super::{Entry, EntryList};
use foundation_nostd::comp::basic::RwLock;

pub trait Waker {
    fn wake(&self);
}

#[derive(Debug)]
pub struct DurationWaker<T> {
    pub handle: T,
    pub from: time::Instant,
    pub how_long: time::Duration,
}

#[derive(Debug)]
pub struct DurationStore<T> {
    store: Arc<RwLock<EntryList<DurationWaker<T>>>>,
}

impl<T> Clone for DurationStore<T> {
    fn clone(&self) -> Self {
        Self {
            store: self.store.clone(),
        }
    }
}

// --- constructors

impl<T> Default for DurationStore<T> {
    fn default() -> Self {
        Self::new()
    }
}

impl<T> DurationStore<T> {
    #[must_use]
    pub fn new() -> Self {
        Self {
            store: Arc::new(RwLock::new(EntryList::new())),
        }
    }
}

// --- core implementation methods

#[allow(unused)]
impl<T: std::fmt::Debug> DurationStore<T> {
    /// Inserts a new Wakeable.
    pub fn insert(&self, wakeable: DurationWaker<T>) -> Entry {
        self.store.write().unwrap().insert(wakeable)
    }

    /// Update an existing Wakeable returning the old handle used.
    pub fn update(&self, handle: &Entry, wakeable: DurationWaker<T>) -> Option<DurationWaker<T>> {
        self.store.write().unwrap().update(handle, wakeable)
    }

    /// Removes a previously inserted sleeping ticker.
    ///
    /// Returns `true` if the ticker was notified.
    #[must_use]
    pub fn remove(&self, handle: &Entry) -> Option<DurationWaker<T>> {
        self.store.write().unwrap().take(handle)
    }

    #[must_use]
    pub fn has_pending_tasks(&self) -> bool {
        self.store.read().unwrap().active_slots() > 0
    }

    #[must_use]
    pub fn count(&self) -> usize {
        self.store.read().unwrap().active_slots()
    }

    /// Returns the list of
    #[must_use]
    pub fn get_matured(&self) -> Vec<DurationWaker<T>> {
        self.store.write().unwrap().select_take(Waiter::is_ready)
    }

    /// Returns the minimum duration of time of all entries in the
    /// sleeper, providing you the minimum time when one of the task is
    /// guaranteed to be ready for progress.
    fn min_duration(&self) -> Option<time::Duration> {
        self.store
            .read()
            .unwrap()
            .map_with(DurationWaker::remaining)
            .iter()
            .min()
            .copied()
    }

    /// Returns the maximum duration of time of all entries in the
    /// sleeper, providing you the maximum time to potentially wait
    /// for all tasks to be ready.
    fn max_duration(&self) -> Option<time::Duration> {
        self.store
            .read()
            .unwrap()
            .map_with(DurationWaker::remaining)
            .iter()
            .max()
            .copied()
    }
}

pub trait Timeable {
    fn remaining_duration(&self) -> Option<time::Duration>;
}

impl<T: Waker> Timeable for DurationWaker<T> {
    fn remaining_duration(&self) -> Option<time::Duration> {
        self.remaining()
    }
}

pub trait Waiter {
    fn is_ready(&self) -> bool;
}

impl<T> Waiter for DurationWaker<T> {
    fn is_ready(&self) -> bool {
        self.try_is_ready().unwrap_or(false)
    }
}

impl<T: Waker> Waker for DurationWaker<T> {
    fn wake(&self) {
        self.handle.wake();
    }
}

impl<T> DurationWaker<T> {
    pub fn new(handle: T, from: time::Instant, how_long: time::Duration) -> Self {
        Self {
            handle,
            from,
            how_long,
        }
    }

    pub fn from_now(handle: T, how_long: time::Duration) -> Self {
        Self::new(handle, time::Instant::now(), how_long)
    }

    pub fn remaining(&self) -> Option<time::Duration> {
        let now = std::time::Instant::now();
        match self.from.checked_add(self.how_long) {
            Some(when_ready) => when_ready.checked_duration_since(now),
            None => None,
        }
    }

    pub fn try_is_ready(&self) -> Option<bool> {
        let now = std::time::Instant::now();
        self.from
            .checked_add(self.how_long)
            .map(|when_ready| when_ready.checked_duration_since(now).is_none())
    }

    /// Returns the deadline when this waker will be ready.
    pub fn deadline(&self) -> time::Instant {
        self.from + self.how_long
    }
}

/// Sleepers manages sleeping tasks keyed by their task Entry.
/// Uses HashMap to avoid entry ID collisions from EntryList.
///
/// # Design Note
/// Previously used `EntryList<T>` which generated new entry IDs for each insert,
/// causing collisions where sleeper entries (e.g., 3/0) could conflict with
/// task entries (e.g., 3/0). When task 3/0 completed, removing its sleeper would
/// accidentally remove task 5/0's sleeper if both shared the same generated ID.
///
/// The HashMap-based approach keys sleepers directly by the task's Entry,
/// eliminating this collision issue entirely.
#[derive(Debug)]
pub struct Sleepers<T: Waiter> {
    /// Map from task entry to sleeper data.
    sleepers: Arc<RwLock<HashMap<Entry, T>>>,
}

pub trait Timing {
    fn min_duration(&self) -> Option<time::Duration>;
    fn max_duration(&self) -> Option<time::Duration>;
}

impl<T: Timeable + Waiter + std::fmt::Debug> Timing for Sleepers<T> {
    /// Returns the minimum duration of time of all entries in the
    /// sleeper, providing you the minimum time when one of the task is
    /// guaranteed to be ready for progress.
    fn min_duration(&self) -> Option<time::Duration> {
        let sleepers = self.sleepers.read().unwrap();
        sleepers
            .values()
            .filter_map(|sleeper| sleeper.remaining_duration())
            .max()
    }

    /// Returns the maximum duration of time of all entries in the
    /// sleeper, providing you the maximum time to potentially wait
    /// for all tasks to be ready.
    fn max_duration(&self) -> Option<time::Duration> {
        let sleepers = self.sleepers.read().unwrap();
        sleepers
            .values()
            .filter_map(|sleeper| sleeper.remaining_duration())
            .max()
    }
}

impl<T: Waker + Waiter + std::fmt::Debug> Waker for Sleepers<T> {
    fn wake(&self) {
        for sleeper in self.get_matured() {
            sleeper.wake();
        }
    }
}

impl<T: Waiter + std::fmt::Debug> Clone for Sleepers<T> {
    fn clone(&self) -> Self {
        Self {
            sleepers: self.sleepers.clone(),
        }
    }
}

impl<T: Waiter + std::fmt::Debug> Default for Sleepers<T> {
    fn default() -> Self {
        Self::new()
    }
}

impl<T: Waiter + std::fmt::Debug> Sleepers<T> {
    #[must_use]
    pub fn new() -> Self {
        Self {
            sleepers: Arc::new(RwLock::new(HashMap::new())),
        }
    }

    /// Inserts a new Wakeable keyed by the task entry.
    /// The entry is the task's entry, not a new sleeper entry.
    pub fn insert(&self, entry: Entry, wakeable: T) {
        self.sleepers.write().unwrap().insert(entry, wakeable);
    }

    /// Update an existing Wakeable returning the old handle used.
    pub fn update(&self, handle: &Entry, wakeable: T) -> Option<T> {
        self.sleepers.write().unwrap().insert(*handle, wakeable)
    }

    /// Removes a previously inserted sleeping ticker.
    #[must_use]
    pub fn remove(&self, handle: &Entry) -> Option<T> {
        self.sleepers.write().unwrap().remove(handle)
    }

    /// Returns true if a sleeper exists for the given entry.
    #[must_use]
    pub fn contains(&self, handle: &Entry) -> bool {
        self.sleepers.read().unwrap().contains_key(handle)
    }

    #[must_use]
    pub fn has_pending_tasks(&self) -> bool {
        !self.sleepers.read().unwrap().is_empty()
    }

    #[must_use]
    pub fn count(&self) -> usize {
        self.sleepers.read().unwrap().len()
    }

    /// Returns the list of matured (ready) sleepers.
    #[must_use]
    pub fn get_matured(&self) -> Vec<T> {
        let mut sleepers = self.sleepers.write().unwrap();
        let matured_entries: Vec<Entry> = sleepers
            .iter()
            .filter(|(_, sleeper)| sleeper.is_ready())
            .map(|(entry, _)| *entry)
            .collect();
        let mut matured = Vec::new();
        for entry in matured_entries {
            if let Some(sleeper) = sleepers.remove(&entry) {
                matured.push(sleeper);
            }
        }
        matured
    }

    /// Returns the earliest deadline among all Timable sleepers.
    ///
    /// Used to calculate how long a thread should sleep when no work is available.
    /// Returns None if no sleepers have deadlines (e.g., all are Atomic).
    pub fn minimum_deadline(&self) -> Option<time::Instant>
    where
        T: crate::synca::sleepers::Timeable,
    {
        let sleepers = self.sleepers.read().unwrap();
        sleepers
            .values()
            .filter_map(|sleeper| sleeper.remaining_duration())
            .map(|remaining| time::Instant::now() + remaining)
            .min()
    }

    /// Returns the duration until the earliest sleeper deadline.
    ///
    /// Returns None if no sleepers have deadlines.
    /// Returns Duration::ZERO if a sleeper is already past its deadline.
    pub fn time_until_next(&self) -> Option<time::Duration>
    where
        T: crate::synca::sleepers::Timeable,
    {
        self.minimum_deadline().map(|deadline| {
            let now = time::Instant::now();
            if deadline > now {
                deadline - now
            } else {
                time::Duration::ZERO
            }
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::thread;

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
        fn is_ready(&self) -> bool {
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

        sleepers.insert(
            Entry { id: 1, gen: 0 },
            MockSleeper { ready: false },
        );
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

        store.insert(DurationWaker::from_now(handle1, time::Duration::from_millis(100)));
        store.insert(DurationWaker::from_now(handle2, time::Duration::from_millis(50)));

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

        store.insert(DurationWaker::from_now(handle1, time::Duration::from_millis(100)));
        store.insert(DurationWaker::from_now(handle2, time::Duration::from_millis(50)));

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
}
