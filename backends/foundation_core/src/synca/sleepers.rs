// Implements a Sleeper register directory where a given set of
// Wakeable primitive can be notified after some expired duration
// registered with.

use std::{sync::Arc, time};

use super::{Entry, EntryList};
use foundation_nostd::comp::basic::RwLock;

pub trait Waker {
    fn wake(&self);
}

#[cfg_attr(feature = "std", derive(Debug))]
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

    pub(crate) fn has_pending_tasks(&self) -> bool {
        self.store.read().unwrap().active_slots() > 0
    }

    pub(crate) fn count(&self) -> usize {
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
            .max()
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
}

#[derive(Debug)]
pub struct Sleepers<T: Waiter> {
    /// the list of wakers pending to be processed.
    sleepers: Arc<RwLock<EntryList<T>>>,
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
        self.sleepers
            .read()
            .unwrap()
            .map_with(Timeable::remaining_duration)
            .iter()
            .max()
            .copied()
    }

    /// Returns the maximum duration of time of all entries in the
    /// sleeper, providing you the maximum time to potentially wait
    /// for all tasks to be ready.
    fn max_duration(&self) -> Option<time::Duration> {
        self.sleepers
            .read()
            .unwrap()
            .map_with(Timeable::remaining_duration)
            .iter()
            .max()
            .copied()
    }
}

impl<T: Waker + Waiter + std::fmt::Debug> Waker for Sleepers<T> {
    fn wake(&self) {
        for sleeper in &self.sleepers.write().unwrap().select_take(Waiter::is_ready) {
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
            sleepers: Arc::new(RwLock::new(EntryList::new())),
        }
    }

    /// Inserts a new Wakeable.
    pub fn insert(&self, wakeable: T) -> Entry {
        self.sleepers.write().unwrap().insert(wakeable)
    }

    /// Update an existing Wakeable returning the old handle used.
    pub fn update(&self, handle: &Entry, wakeable: T) -> Option<T> {
        self.sleepers.write().unwrap().update(handle, wakeable)
    }

    /// Removes a previously inserted sleeping ticker.
    ///
    /// Returns `true` if the ticker was notified.
    #[must_use]
    pub fn remove(&self, handle: &Entry) -> Option<T> {
        self.sleepers.write().unwrap().take(handle)
    }

    pub(crate) fn has_pending_tasks(&self) -> bool {
        self.sleepers.read().unwrap().active_slots() > 0
    }

    pub(crate) fn count(&self) -> usize {
        self.sleepers.read().unwrap().active_slots()
    }

    /// Returns the list of
    #[must_use]
    pub fn get_matured(&self) -> Vec<T> {
        self.sleepers.write().unwrap().select_take(Waiter::is_ready)
    }
}
#[cfg(test)]
mod test_duration_waker {
    use super::*;
    use std::sync::{Arc, Mutex};
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

        fn was_woken(&self) -> bool {
            *self.woken.lock().unwrap()
        }
    }

    impl Waker for MockWaker {
        fn wake(&self) {
            *self.woken.lock().unwrap() = true;
        }
    }

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
        assert!(!waker.is_ready());
    }

    #[test]
    fn test_duration_waker_is_ready_when_elapsed() {
        let handle = MockWaker::new();
        let start = time::Instant::now() - time::Duration::from_secs(2);
        let duration = time::Duration::from_secs(1);

        let waker = DurationWaker::new(handle, start, duration);

        assert_eq!(Some(true), waker.try_is_ready());
        assert!(waker.is_ready());
    }

    #[test]
    fn test_duration_waker_remaining_decreases() {
        let handle = MockWaker::new();
        let duration = time::Duration::from_millis(100);

        let waker = DurationWaker::from_now(handle, duration);

        let remaining1 = waker.remaining();
        thread::sleep(time::Duration::from_millis(20));
        let remaining2 = waker.remaining();

        assert!(remaining1 > remaining2);
    }

    #[test]
    fn test_duration_waker_remaining_none_when_elapsed() {
        let handle = MockWaker::new();
        let start = time::Instant::now() - time::Duration::from_secs(2);
        let duration = time::Duration::from_secs(1);

        let waker = DurationWaker::new(handle, start, duration);

        assert_eq!(None, waker.remaining());
    }

    #[test]
    fn test_duration_waker_wake() {
        let handle = MockWaker::new();
        let waker = DurationWaker::from_now(handle.clone(), time::Duration::from_secs(10));

        assert!(!handle.was_woken());
        waker.wake();
        assert!(handle.was_woken());
    }

    #[test]
    fn test_duration_waker_remaining_duration_trait() {
        let handle = MockWaker::new();
        let duration = time::Duration::from_secs(10);
        let waker = DurationWaker::from_now(handle, duration);

        let remaining = waker.remaining_duration();
        assert!(remaining.is_some());
        assert!(remaining.unwrap() <= duration);
    }
}

#[cfg(test)]
mod test_duration_store {
    use super::*;
    use std::sync::{Arc, Mutex};

    #[derive(Debug, Clone)]
    struct MockHandle {
        woken: Arc<Mutex<bool>>,
    }

    impl MockHandle {
        fn new() -> Self {
            Self {
                woken: Arc::new(Mutex::new(false)),
            }
        }
    }

    impl Waker for MockHandle {
        fn wake(&self) {
            *self.woken.lock().unwrap() = true;
        }
    }

    #[test]
    fn test_duration_store_new() {
        let store: DurationStore<MockHandle> = DurationStore::new();
        assert_eq!(0, store.count());
    }

    #[test]
    fn test_duration_store_default() {
        let store: DurationStore<MockHandle> = DurationStore::default();
        assert_eq!(0, store.count());
    }

    #[test]
    fn test_duration_store_insert() {
        let store = DurationStore::new();
        let handle = MockHandle::new();
        let waker = DurationWaker::from_now(handle, time::Duration::from_secs(1));

        let entry = store.insert(waker);
        assert_eq!(1, store.count());
        // Entry was successfully created
    }

    #[test]
    fn test_duration_store_remove() {
        let store = DurationStore::new();
        let handle = MockHandle::new();
        let waker = DurationWaker::from_now(handle, time::Duration::from_secs(1));

        let entry = store.insert(waker);
        assert_eq!(1, store.count());

        let removed = store.remove(&entry);
        assert!(removed.is_some());
        assert_eq!(0, store.count());
    }

    #[test]
    fn test_duration_store_update() {
        let store = DurationStore::new();
        let handle1 = MockHandle::new();
        let waker1 = DurationWaker::from_now(handle1, time::Duration::from_secs(1));

        let entry = store.insert(waker1);
        assert_eq!(1, store.count());

        let handle2 = MockHandle::new();
        let waker2 = DurationWaker::from_now(handle2, time::Duration::from_secs(2));

        let old = store.update(&entry, waker2);
        assert!(old.is_some());
        assert_eq!(1, store.count());
    }

    #[test]
    fn test_duration_store_get_matured_empty() {
        let store: DurationStore<MockHandle> = DurationStore::new();
        let matured = store.get_matured();
        assert_eq!(0, matured.len());
    }

    #[test]
    fn test_duration_store_get_matured_with_ready() {
        let store = DurationStore::new();

        // Insert an already-elapsed waker
        let handle1 = MockHandle::new();
        let start = time::Instant::now() - time::Duration::from_secs(2);
        let waker1 = DurationWaker::new(handle1, start, time::Duration::from_secs(1));
        store.insert(waker1);

        // Insert a not-yet-ready waker
        let handle2 = MockHandle::new();
        let waker2 = DurationWaker::from_now(handle2, time::Duration::from_secs(10));
        store.insert(waker2);

        assert_eq!(2, store.count());

        let matured = store.get_matured();
        assert_eq!(1, matured.len());
        assert_eq!(1, store.count()); // One removed, one remains
    }

    #[test]
    fn test_duration_store_has_pending_tasks() {
        let store: DurationStore<MockHandle> = DurationStore::new();
        assert!(!store.has_pending_tasks());

        let handle = MockHandle::new();
        let waker = DurationWaker::from_now(handle, time::Duration::from_secs(1));
        store.insert(waker);

        assert!(store.has_pending_tasks());
    }

    #[test]
    fn test_duration_store_clone() {
        let store = DurationStore::new();
        let handle = MockHandle::new();
        let waker = DurationWaker::from_now(handle, time::Duration::from_secs(1));

        store.insert(waker);
        assert_eq!(1, store.count());

        let store_clone = store.clone();
        assert_eq!(1, store_clone.count());
    }
}

#[cfg(test)]
mod test_sleepers {
    use super::*;
    use std::sync::{Arc, Mutex};

    #[derive(Debug, Clone)]
    struct TestWaker {
        id: usize,
        ready: Arc<Mutex<bool>>,
        woken: Arc<Mutex<bool>>,
    }

    impl TestWaker {
        fn new(id: usize, ready: bool) -> Self {
            Self {
                id,
                ready: Arc::new(Mutex::new(ready)),
                woken: Arc::new(Mutex::new(false)),
            }
        }

        fn set_ready(&self, value: bool) {
            *self.ready.lock().unwrap() = value;
        }

        fn was_woken(&self) -> bool {
            *self.woken.lock().unwrap()
        }
    }

    impl Waker for TestWaker {
        fn wake(&self) {
            *self.woken.lock().unwrap() = true;
        }
    }

    impl Waiter for TestWaker {
        fn is_ready(&self) -> bool {
            *self.ready.lock().unwrap()
        }
    }

    #[test]
    fn test_sleepers_new() {
        let sleepers: Sleepers<TestWaker> = Sleepers::new();
        assert_eq!(0, sleepers.count());
    }

    #[test]
    fn test_sleepers_default() {
        let sleepers: Sleepers<TestWaker> = Sleepers::default();
        assert_eq!(0, sleepers.count());
    }

    #[test]
    fn test_sleepers_insert() {
        let sleepers = Sleepers::new();
        let waker = TestWaker::new(1, false);

        let entry = sleepers.insert(waker);
        assert_eq!(1, sleepers.count());
        // Entry was successfully created
    }

    #[test]
    fn test_sleepers_remove() {
        let sleepers = Sleepers::new();
        let waker = TestWaker::new(1, false);

        let entry = sleepers.insert(waker);
        assert_eq!(1, sleepers.count());

        let removed = sleepers.remove(&entry);
        assert!(removed.is_some());
        assert_eq!(0, sleepers.count());
    }

    #[test]
    fn test_sleepers_update() {
        let sleepers = Sleepers::new();
        let waker1 = TestWaker::new(1, false);

        let entry = sleepers.insert(waker1);
        assert_eq!(1, sleepers.count());

        let waker2 = TestWaker::new(2, true);
        let old = sleepers.update(&entry, waker2);

        assert!(old.is_some());
        assert_eq!(1, old.unwrap().id);
        assert_eq!(1, sleepers.count());
    }

    #[test]
    fn test_sleepers_get_matured_empty() {
        let sleepers: Sleepers<TestWaker> = Sleepers::new();
        let matured = sleepers.get_matured();
        assert_eq!(0, matured.len());
    }

    #[test]
    fn test_sleepers_get_matured_filters_ready() {
        let sleepers = Sleepers::new();

        // Insert ready waker
        let waker1 = TestWaker::new(1, true);
        sleepers.insert(waker1);

        // Insert not-ready waker
        let waker2 = TestWaker::new(2, false);
        sleepers.insert(waker2);

        assert_eq!(2, sleepers.count());

        let matured = sleepers.get_matured();
        assert_eq!(1, matured.len());
        assert_eq!(1, matured[0].id);
        assert_eq!(1, sleepers.count()); // One removed, one remains
    }

    #[test]
    fn test_sleepers_has_pending_tasks() {
        let sleepers: Sleepers<TestWaker> = Sleepers::new();
        assert!(!sleepers.has_pending_tasks());

        let waker = TestWaker::new(1, false);
        sleepers.insert(waker);

        assert!(sleepers.has_pending_tasks());
    }

    #[test]
    fn test_sleepers_waker_trait() {
        let sleepers = Sleepers::new();

        // Insert multiple wakers, some ready
        let waker1 = TestWaker::new(1, true);
        sleepers.insert(waker1.clone());

        let waker2 = TestWaker::new(2, false);
        sleepers.insert(waker2.clone());

        let waker3 = TestWaker::new(3, true);
        sleepers.insert(waker3.clone());

        assert_eq!(3, sleepers.count());
        assert!(!waker1.was_woken());
        assert!(!waker3.was_woken());

        // Wake all ready wakers
        sleepers.wake();

        // Check that ready wakers were woken and removed
        assert!(waker1.was_woken());
        assert!(!waker2.was_woken());
        assert!(waker3.was_woken());
        assert_eq!(1, sleepers.count()); // Only non-ready remains
    }

    #[test]
    fn test_sleepers_clone() {
        let sleepers = Sleepers::new();
        let waker = TestWaker::new(1, false);

        sleepers.insert(waker);
        assert_eq!(1, sleepers.count());

        let sleepers_clone = sleepers.clone();
        assert_eq!(1, sleepers_clone.count());
    }

    #[test]
    fn test_sleepers_multiple_operations() {
        let sleepers = Sleepers::new();

        // Insert multiple wakers
        let waker1 = TestWaker::new(1, false);
        let _entry1 = sleepers.insert(waker1.clone());

        let waker2 = TestWaker::new(2, false);
        let entry2 = sleepers.insert(waker2.clone());

        let waker3 = TestWaker::new(3, false);
        let entry3 = sleepers.insert(waker3.clone());

        assert_eq!(3, sleepers.count());

        // Make waker1 ready
        waker1.set_ready(true);

        // Get matured should return waker1
        let matured = sleepers.get_matured();
        assert_eq!(1, matured.len());
        assert_eq!(1, matured[0].id);
        assert_eq!(2, sleepers.count());

        // Remove waker2
        let _ = sleepers.remove(&entry2);
        assert_eq!(1, sleepers.count());

        // Update waker3
        let new_waker = TestWaker::new(4, true);
        sleepers.update(&entry3, new_waker);

        // Get matured should return updated waker
        let matured = sleepers.get_matured();
        assert_eq!(1, matured.len());
        assert_eq!(4, matured[0].id);
        assert_eq!(0, sleepers.count());
    }
}
