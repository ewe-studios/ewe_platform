// Implements a Sleeper register directory where a given set of
// Wakeable primitive can be notified after some expired duration
// registered with.

use std::{sync::Arc, time};

use super::{Entry, EntryList};

pub trait Waker {
    fn wake(&self);
}

pub struct Wakeable<T: Waker> {
    pub handle: Arc<T>,
    pub from: time::Instant,
    pub how_long: time::Duration,
}

impl<T: Waker> Waker for Wakeable<T> {
    fn wake(&self) {
        self.handle.wake()
    }
}

impl<T: Waker> Wakeable<T> {
    pub fn new(handle: Arc<T>, from: time::Instant, how_long: time::Duration) -> Self {
        Self {
            handle,
            from,
            how_long,
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
