// Implements a Sleeper register directory where a given set of
// Wakeable primitive can be notified after some expired duration
// registered with.

use std::{collections::HashMap, sync::Arc, time};

use super::{Entry, EntryList};
use crate::valtron::Clock;
use foundation_nostd::comp::basic::RwLock;

static DEFAULT_READINESS_WAIT: std::time::Duration = std::time::Duration::from_millis(10);

pub trait Waker {
    fn wake(&self);
}

pub struct DurationWaker<T> {
    pub handle: T,
    pub from: foundation_compact::Instant,
    pub how_long: time::Duration,
    /// The clock that says when this waker matures.
    clock: Arc<dyn Clock>,
}

impl<T: core::fmt::Debug> core::fmt::Debug for DurationWaker<T> {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        f.debug_struct("DurationWaker")
            .field("handle", &self.handle)
            .field("from", &self.from)
            .field("how_long", &self.how_long)
            .finish_non_exhaustive()
    }
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
    ///
    /// # Panics
    /// Panics if the internal lock is poisoned.
    /// Inserts a new Wakeable.
    pub fn insert(&self, wakeable: DurationWaker<T>) -> Entry {
        self.store.write().unwrap().insert(wakeable)
    }

    ///
    /// # Panics
    /// Panics if the internal lock is poisoned.
    /// Update an existing Wakeable returning the old handle used.
    pub fn update(&self, handle: &Entry, wakeable: DurationWaker<T>) -> Option<DurationWaker<T>> {
        self.store.write().unwrap().update(handle, wakeable)
    }

    ///
    /// # Panics
    /// Panics if the internal lock is poisoned.
    /// Removes a previously inserted sleeping ticker.
    ///
    /// Returns `true` if the ticker was notified.
    #[must_use]
    pub fn remove(&self, handle: &Entry) -> Option<DurationWaker<T>> {
        self.store.write().unwrap().take(handle)
    }

    /// # Panics
    /// Panics if the internal lock is poisoned.
    #[must_use]
    pub fn has_pending_tasks(&self) -> bool {
        self.store.read().unwrap().active_slots() > 0
    }

    /// # Panics
    /// Panics if the internal lock is poisoned.
    #[must_use]
    pub fn count(&self) -> usize {
        self.store.read().unwrap().active_slots()
    }

    ///
    /// # Panics
    /// Panics if the internal lock is poisoned.
    /// Returns the list of
    #[must_use]
    pub fn get_matured(&self) -> Vec<DurationWaker<T>> {
        self.store
            .write()
            .unwrap()
            .select_take(|t| t.is_ready(None))
    }

    ///
    /// # Panics
    /// Panics if the internal lock is poisoned.
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

    ///
    /// # Panics
    /// Panics if the internal lock is poisoned.
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

    /// Whether the sleeper must be re-checked on a timer because it can become
    /// ready without a wake. Defaults to `false`: a pure timer needs none.
    fn needs_polling(&self) -> bool {
        false
    }
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
    pub fn new(
        handle: T,
        from: foundation_compact::Instant,
        how_long: time::Duration,
        clock: Arc<dyn Clock>,
    ) -> Self {
        Self {
            handle,
            from,
            how_long,
            clock,
        }
    }

    pub fn from_now(handle: T, how_long: time::Duration, clock: Arc<dyn Clock>) -> Self {
        let from = clock.now();
        Self::new(handle, from, how_long, clock)
    }

    pub fn remaining(&self) -> Option<time::Duration> {
        let now = self.clock.now();
        // A deadline that has already passed is due NOW, not "no deadline".
        // Returning `None` here made `Sleepers::time_until_next` drop a past-due
        // timer, so a worker picked the next far deadline and slept past a task
        // that was already runnable (spec 70 feature 121: a 10 ms H2 pump poll
        // next to a 60 s idle deadline slept for 60 s).
        self.from
            .checked_add(self.how_long)
            .map(|when_ready| when_ready.saturating_duration_since(now))
    }

    pub fn try_is_ready(&self) -> Option<bool> {
        let now = self.clock.now();
        self.from
            .checked_add(self.how_long)
            .map(|when_ready| when_ready.checked_duration_since(now).is_none())
    }

    /// Returns the deadline when this waker will be ready.
    pub fn deadline(&self) -> foundation_compact::Instant {
        self.from + self.how_long
    }
}

/// Sleepers manages sleeping tasks keyed by their task Entry.
/// Uses `HashMap` to avoid entry ID collisions from `EntryList`.
///
/// # Design Note
/// Previously used `EntryList<T>` which generated new entry IDs for each insert,
/// causing collisions where sleeper entries (e.g., 3/0) could conflict with
/// task entries (e.g., 3/0). When task 3/0 completed, removing its sleeper would
/// accidentally remove task 5/0's sleeper if both shared the same generated ID.
///
/// The HashMap-based approach keys sleepers directly by the task's Entry,
/// eliminating this collision issue entirely.
#[allow(
    clippy::struct_field_names,
    reason = "the field names the directory that stores the sleepers"
)]
pub struct Sleepers<T: Waiter> {
    /// Map from task entry to sleeper data.
    sleepers: Arc<RwLock<HashMap<Entry, T>>>,
    /// max wait for readiness checks
    max_readiness_wait: std::time::Duration,
    /// The clock that says when a sleeper matures.
    clock: Arc<dyn Clock>,
}

impl<T: Waiter + core::fmt::Debug> core::fmt::Debug for Sleepers<T> {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        f.debug_struct("Sleepers")
            .field("max_readiness_wait", &self.max_readiness_wait)
            .finish_non_exhaustive()
    }
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
            .filter_map(Timeable::remaining_duration)
            .min()
    }

    /// Returns the maximum duration of time of all entries in the
    /// sleeper, providing you the maximum time to potentially wait
    /// for all tasks to be ready.
    fn max_duration(&self) -> Option<time::Duration> {
        let sleepers = self.sleepers.read().unwrap();
        sleepers
            .values()
            .filter_map(Timeable::remaining_duration)
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
            clock: self.clock.clone(),
        }
    }
}

impl<T: Waiter + std::fmt::Debug> Sleepers<T> {
    #[must_use]
    pub fn new(clock: Arc<dyn Clock>) -> Self {
        Self {
            sleepers: Arc::new(RwLock::new(HashMap::new())),
            max_readiness_wait: DEFAULT_READINESS_WAIT,
            clock,
        }
    }

    #[must_use]
    pub fn with_max_readiness_wait(dur: std::time::Duration, clock: Arc<dyn Clock>) -> Self {
        Self {
            sleepers: Arc::new(RwLock::new(HashMap::new())),
            max_readiness_wait: dur,
            clock,
        }
    }

    /// The clock this directory reads to decide when a sleeper matures.
    #[must_use]
    pub fn clock(&self) -> Arc<dyn Clock> {
        self.clock.clone()
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

    ///
    /// # Panics
    /// Panics if the internal lock is poisoned.
    /// Inserts a new Wakeable keyed by the task entry.
    /// The entry is the task's entry, not a new sleeper entry.
    pub fn insert(&self, entry: Entry, wakeable: T) {
        self.sleepers.write().unwrap().insert(entry, wakeable);
    }

    ///
    /// # Panics
    /// Panics if the internal lock is poisoned.
    /// Update an existing Wakeable returning the old handle used.
    pub fn update(&self, handle: &Entry, wakeable: T) -> Option<T> {
        self.sleepers.write().unwrap().insert(*handle, wakeable)
    }

    ///
    /// # Panics
    /// Panics if the internal lock is poisoned.
    /// Removes a previously inserted sleeping ticker.
    #[must_use]
    pub fn remove(&self, handle: &Entry) -> Option<T> {
        self.sleepers.write().unwrap().remove(handle)
    }

    ///
    /// # Panics
    /// Panics if the internal lock is poisoned.
    /// Returns true if a sleeper exists for the given entry.
    #[must_use]
    pub fn contains(&self, handle: &Entry) -> bool {
        self.sleepers.read().unwrap().contains_key(handle)
    }

    /// # Panics
    /// Panics if the internal lock is poisoned.
    #[must_use]
    pub fn has_pending_tasks(&self) -> bool {
        !self.sleepers.read().unwrap().is_empty()
    }

    /// # Panics
    /// Panics if the internal lock is poisoned.
    #[must_use]
    pub fn count(&self) -> usize {
        self.sleepers.read().unwrap().len()
    }

    ///
    /// # Panics
    /// Panics if the internal lock is poisoned.
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

    ///
    /// # Panics
    /// Panics if the internal lock is poisoned.
    /// Returns the earliest deadline among all Timable sleepers.
    ///
    /// Used to calculate how long a thread should sleep when no work is available.
    /// Returns None if no sleepers have deadlines (e.g., all are Atomic).
    #[must_use]
    pub fn minimum_deadline(&self) -> Option<foundation_compact::Instant>
    where
        T: crate::synca::sleepers::Timeable,
    {
        let sleepers = self.sleepers.read().unwrap();
        let now = self.clock.now();
        sleepers
            .values()
            .filter_map(Timeable::remaining_duration)
            .map(|remaining| now + remaining)
            .min()
    }

    ///
    /// # Panics
    /// Panics if the internal lock is poisoned.
    /// Returns the duration until the earliest sleeper deadline.
    ///
    /// Whether any sleeper must be re-checked on a timer (see
    /// [`Timeable::needs_polling`]).
    pub fn needs_polling(&self) -> bool
    where
        T: crate::synca::sleepers::Timeable,
    {
        self.sleepers
            .read()
            .unwrap()
            .values()
            .any(Timeable::needs_polling)
    }

    /// Returns None if no sleepers have deadlines.
    /// Returns `Duration::ZERO` if a sleeper is already past its deadline.
    #[must_use]
    pub fn time_until_next(&self) -> Option<time::Duration>
    where
        T: crate::synca::sleepers::Timeable,
    {
        self.minimum_deadline().map(|deadline| {
            let now = self.clock.now();
            if deadline > now {
                deadline - now
            } else {
                time::Duration::ZERO
            }
        })
    }
}
