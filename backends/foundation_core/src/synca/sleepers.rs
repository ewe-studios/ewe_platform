// Implements a Sleeper register directory where a given set of
// Wakeable primitive can be notified after some expired duration
// registered with.

use std::{collections::HashMap, sync::Arc, time};

use super::{Entry, EntryList};
use foundation_nostd::comp::basic::RwLock;

static DEFAULT_READINESS_WAIT: std::time::Duration = std::time::Duration::from_millis(10);

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
        self.store
            .write()
            .unwrap()
            .select_take(|t| t.is_ready(None))
    }

    /// Returns the minimum duration of time of all entries in the
    /// sleeper, providing you the minimum time when one of the task is
    /// guaranteed to be ready for progress.
    ///
    /// This is how long a caller can sleep before the next entry matures
    /// (the store-level counterpart of [`Timing::min_duration`] on
    /// [`Sleepers`]). Returns `None` when the store is empty.
    #[must_use]
    pub fn min_duration(&self) -> Option<time::Duration> {
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
    ///
    /// Store-level counterpart of [`Timing::max_duration`] on [`Sleepers`].
    /// Returns `None` when the store is empty.
    #[must_use]
    pub fn max_duration(&self) -> Option<time::Duration> {
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
    fn is_ready(&self, dur: Option<std::time::Duration>) -> bool;
}

impl<T> Waiter for DurationWaker<T> {
    fn is_ready(&self, _dur: Option<std::time::Duration>) -> bool {
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
    /// max wait for readiness checks
    max_readiness_wait: std::time::Duration,
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
            .min()
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
            max_readiness_wait: self.max_readiness_wait,
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
            max_readiness_wait: DEFAULT_READINESS_WAIT,
        }
    }

    #[must_use]
    pub fn with_max_readiness_wait(dur: std::time::Duration) -> Self {
        Self {
            sleepers: Arc::new(RwLock::new(HashMap::new())),
            max_readiness_wait: dur,
        }
    }

    pub fn set_max_readiness_wait(&mut self, dur: std::time::Duration) {
        self.max_readiness_wait = dur;
    }

    /// The maximum wait a readiness check may block (see
    /// [`Self::get_matured`]).
    #[must_use]
    pub fn max_readiness_wait(&self) -> std::time::Duration {
        self.max_readiness_wait
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
            .filter(|(_, sleeper)| sleeper.is_ready(Some(self.max_readiness_wait)))
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
