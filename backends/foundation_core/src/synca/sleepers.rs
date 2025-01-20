// Implements a Sleeper register directory where a given set of
// Wakeable primitive can be notified after some expired duration
// registered with.

use std::time;

use super::{Entry, EntryList};

pub trait Waker {
    fn wake(&self);
}

pub struct Wakeable<T: Waker> {
    pub handle: T,
    pub from: time::Instant,
    pub how_long: time::Duration,
}

impl<T: Waker> Waker for Wakeable<T> {
    fn wake(&self) {
        self.handle.wake()
    }
}

impl<T: Waker> Wakeable<T> {
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

    pub fn wake(&self) {
        self.handle.wake();
    }

    pub fn is_ready(&self) -> bool {
        self.try_is_ready().expect("should have result")
    }

    pub fn try_is_ready(&self) -> Option<bool> {
        let now = std::time::Instant::now();
        match self.from.checked_add(self.how_long) {
            Some(when_ready) => Some(match when_ready.checked_duration_since(now) {
                Some(_) => false,
                None => true,
            }),
            None => None,
        }
    }
}

pub struct Sleepers<T: Waker> {
    /// the list of wakers pending to be processed.
    sleepers: EntryList<Wakeable<T>>,
}

impl<T: Waker> Sleepers<T> {
    pub fn new() -> Self {
        Self {
            sleepers: EntryList::new(),
        }
    }
    /// Inserts a new Wakeable.
    pub fn insert(&mut self, wakeable: Wakeable<T>) -> Entry {
        self.sleepers.insert(wakeable)
    }

    /// Returns the minimum duration of time of all entries in the
    /// sleeper, providing you the minimum time when one of the task is
    /// guranteed to be ready for progress.
    pub fn min_duration(&self) -> Option<time::Duration> {
        match self.sleepers.map_with(|item| item.remaining()).iter().max() {
            Some(item) => Some(item.clone()),
            None => None,
        }
    }

    /// Returns the maximum duration of time of all entries in the
    /// sleeper, providing you the maximum time to potentially wait
    /// for all tasks to be ready.
    pub fn max_duration(&self) -> Option<time::Duration> {
        match self.sleepers.map_with(|item| item.remaining()).iter().max() {
            Some(item) => Some(item.clone()),
            None => None,
        }
    }

    /// Update an existing Wakeable returning the old handle used.
    pub fn update(&mut self, handle: &Entry, wakeable: Wakeable<T>) -> Option<Wakeable<T>> {
        self.sleepers.update(handle, wakeable)
    }

    /// Removes a previously inserted sleeping ticker.
    ///
    /// Returns `true` if the ticker was notified.
    pub fn remove(&mut self, handle: &Entry) -> Option<Wakeable<T>> {
        self.sleepers.take(handle)
    }

    /// notify_ready will go through all sleepers to see who is ready
    /// to be woken having expired it's sleeping time (considered ready)
    /// to be a woken up.
    pub fn notify_ready(&mut self) {
        for sleeper in self
            .sleepers
            .select_take(|item| match item.try_is_ready() {
                Some(inner) => inner,
                None => true,
            })
            .iter()
        {
            sleeper.wake();
        }
    }
}
