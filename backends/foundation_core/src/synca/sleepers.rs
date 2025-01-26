// Implements a Sleeper register directory where a given set of
// Wakeable primitive can be notified after some expired duration
// registered with.

use std::{cell, time};

use super::{Entry, EntryList};

pub trait Waker {
    fn wake(&self);
}

pub struct Wakeable<T> {
    pub handle: T,
    pub from: time::Instant,
    pub how_long: time::Duration,
}

pub trait Timeable {
    fn remaining_duration(&self) -> Option<time::Duration>;
}

impl<T: Waker> Timeable for Wakeable<T> {
    fn remaining_duration(&self) -> Option<time::Duration> {
        self.remaining()
    }
}

pub trait Waiter {
    fn is_ready(&self) -> bool;
}

impl<T> Waiter for Wakeable<T> {
    fn is_ready(&self) -> bool {
        match self.try_is_ready() {
            Some(inner) => inner,
            None => false,
        }
    }
}

impl<T: Waker> Waker for Wakeable<T> {
    fn wake(&self) {
        self.handle.wake()
    }
}

impl<T> Wakeable<T> {
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
        match self.from.checked_add(self.how_long) {
            Some(when_ready) => Some(match when_ready.checked_duration_since(now) {
                Some(_) => false,
                None => true,
            }),
            None => None,
        }
    }
}

pub struct Sleepers<T: Waiter> {
    /// the list of wakers pending to be processed.
    sleepers: cell::RefCell<EntryList<T>>,
}

pub trait Timing {
    fn min_duration(&self) -> Option<time::Duration>;
    fn max_duration(&self) -> Option<time::Duration>;
}

impl<T: Timeable + Waiter> Timing for Sleepers<T> {
    /// Returns the minimum duration of time of all entries in the
    /// sleeper, providing you the minimum time when one of the task is
    /// guranteed to be ready for progress.
    fn min_duration(&self) -> Option<time::Duration> {
        match self
            .sleepers
            .borrow()
            .map_with(|item| item.remaining_duration())
            .iter()
            .max()
        {
            Some(item) => Some(item.clone()),
            None => None,
        }
    }

    /// Returns the maximum duration of time of all entries in the
    /// sleeper, providing you the maximum time to potentially wait
    /// for all tasks to be ready.
    fn max_duration(&self) -> Option<time::Duration> {
        match self
            .sleepers
            .borrow()
            .map_with(|item| item.remaining_duration())
            .iter()
            .max()
        {
            Some(item) => Some(item.clone()),
            None => None,
        }
    }
}

impl<T: Waker + Waiter> Waker for Sleepers<T> {
    fn wake(&self) {
        for sleeper in self
            .sleepers
            .borrow_mut()
            .select_take(|item| item.is_ready())
            .iter()
        {
            sleeper.wake();
        }
    }
}

impl<T: Waiter> Sleepers<T> {
    pub fn new() -> Self {
        Self {
            sleepers: cell::RefCell::new(EntryList::new()),
        }
    }
    /// Inserts a new Wakeable.
    pub fn insert(&self, wakeable: T) -> Entry {
        self.sleepers.borrow_mut().insert(wakeable)
    }

    /// Update an existing Wakeable returning the old handle used.
    pub fn update(&self, handle: &Entry, wakeable: T) -> Option<T> {
        self.sleepers.borrow_mut().update(handle, wakeable)
    }

    /// Removes a previously inserted sleeping ticker.
    ///
    /// Returns `true` if the ticker was notified.
    pub fn remove(&self, handle: &Entry) -> Option<T> {
        self.sleepers.borrow_mut().take(handle)
    }

    pub(crate) fn has_pending_tasks(&self) -> bool {
        self.sleepers.borrow().active_slots() > 0
    }

    pub(crate) fn count(&self) -> usize {
        self.sleepers.borrow().active_slots()
    }

    /// Returns the list of
    pub fn get_matured(&self) -> Vec<T> {
        self.sleepers
            .borrow_mut()
            .select_take(|item| item.is_ready())
    }
}
