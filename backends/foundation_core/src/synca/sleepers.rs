// Implements a Sleeper register directory where a given set of
// Wakeable primitive can be notified after some expired duration
// registered with.

use std::{sync::Arc, time};

use foundation_nostd::comp::RwLock;
use super::{Entry, EntryList};

pub trait Waker {
    fn wake(&self);
}

pub struct DurationWaker<T> {
    pub handle: T,
    pub from: time::Instant,
    pub how_long: time::Duration,
}

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
impl<T> DurationStore<T> {
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

pub struct Sleepers<T: Waiter> {
    /// the list of wakers pending to be processed.
    sleepers: Arc<RwLock<EntryList<T>>>,
}

pub trait Timing {
    fn min_duration(&self) -> Option<time::Duration>;
    fn max_duration(&self) -> Option<time::Duration>;
}

impl<T: Timeable + Waiter> Timing for Sleepers<T> {
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

impl<T: Waker + Waiter> Waker for Sleepers<T> {
    fn wake(&self) {
        for sleeper in &self.sleepers.write().unwrap().select_take(Waiter::is_ready) {
            sleeper.wake();
        }
    }
}

impl<T: Waiter> Clone for Sleepers<T> {
    fn clone(&self) -> Self {
        Self {
            sleepers: self.sleepers.clone(),
        }
    }
}

impl<T: Waiter> Default for Sleepers<T> {
    fn default() -> Self {
        Self::new()
    }
}

impl<T: Waiter> Sleepers<T> {
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
