//! Native: the wake handle of one executor thread (spec 70 feature 80).
//!
//! WHY: a native executor with nothing runnable sleeps its thread for a back-off
//! (up to 100 ms). A task parked on `TaskStatus::Depends` becomes runnable when
//! its readiness fires, and the sleeping thread has to learn that at once, or
//! every wake costs the rest of the back-off. Until feature 80 the wake was one
//! process-wide function that interrupted every sleeping thread, so a wake for
//! one task made every executor look at all of its parked tasks (a wake storm).
//!
//! WHAT: every thread that runs an executor has one [`ExecutorWake`], found with
//! [`current`]. A task that parks takes the handle of the thread that parks it
//! (see `ReadinessWaker`), and its wake reaches that handle only: it marks the
//! task runnable and interrupts that thread's sleep, no other thread's.
//!
//! HOW: a thread is asleep in its handle only while it sleeps ([`park_idle`]), so
//! a wake interrupts the sleep it finds. A wake can also land between the
//! executor's "nothing runnable" check and the sleep: each executor loop calls
//! [`checking`] before it looks for work, and [`park_idle`] skips the sleep if a
//! wake arrived since then. Spurious wakes are harmless: the executor re-checks
//! its work and sleeps again.
//!
//! [`request_executor_resume`](crate::valtron::request_executor_resume) still
//! wakes every handle, for the callers that have not moved to a per-task waker.

use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Arc, Mutex, PoisonError, Weak};

type Interrupt = Arc<dyn Fn() + Send + Sync>;

/// Not checking: a sleep without a preceding [`checking`] never skips.
const NOT_CHECKING: u64 = u64::MAX;

/// The wake handle of one executor thread. See the module docs.
pub struct ExecutorWake {
    /// Wakes so far. A sleep is skipped if it changed since [`checking`].
    epoch: AtomicU64,
    /// The epoch when this thread's executor began its current check.
    checked_at: AtomicU64,
    /// Interrupts the thread's sleep; set only while it sleeps.
    sleeping: Mutex<Option<Interrupt>>,
    /// One-shot callbacks run by the next wake (an async consumer of a
    /// `DrivenStreamIterator`, which drives this thread's engine itself).
    listeners: Mutex<Vec<Box<dyn FnOnce() + Send>>>,
}

static HANDLES: Mutex<Vec<Weak<ExecutorWake>>> = Mutex::new(Vec::new());

thread_local! {
    static CURRENT: Arc<ExecutorWake> = ExecutorWake::new_registered();
}

impl ExecutorWake {
    fn new_registered() -> Arc<Self> {
        let handle = Arc::new(Self {
            epoch: AtomicU64::new(0),
            checked_at: AtomicU64::new(NOT_CHECKING),
            sleeping: Mutex::new(None),
            listeners: Mutex::new(Vec::new()),
        });
        let mut all = HANDLES.lock().unwrap_or_else(PoisonError::into_inner);
        all.retain(|weak| weak.strong_count() > 0);
        all.push(Arc::downgrade(&handle));
        handle
    }

    /// Wake this executor: run the wake listeners and interrupt its sleep.
    pub(crate) fn wake(&self) {
        self.epoch.fetch_add(1, Ordering::SeqCst);
        // Taken out of the lock first: a listener may register another one.
        let listeners = std::mem::take(
            &mut *self
                .listeners
                .lock()
                .unwrap_or_else(PoisonError::into_inner),
        );
        for listener in listeners {
            listener();
        }
        let interrupt = self
            .sleeping
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .clone();
        if let Some(interrupt) = interrupt {
            interrupt();
        }
    }

    /// Run `cb` once, at the next wake of this executor.
    ///
    /// WHY: an async consumer of a single-thread `DrivenStreamIterator` drives the
    /// engine itself and has no executor thread to interrupt. A parked task
    /// becomes ready when a foreign thread wakes its waker; registering the
    /// consumer's waker here makes that wake reach the consumer.
    ///
    /// The list is cleared when it fires, so it holds a callback only until the
    /// next wake. A caller that registers on every poll must keep its own
    /// "already registered" flag, or a long quiet period grows the list.
    // Used by the single-thread `DrivenStreamIterator` only.
    #[cfg_attr(feature = "multi", allow(dead_code))]
    pub(crate) fn on_wake(&self, cb: Box<dyn FnOnce() + Send>) {
        self.listeners
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .push(cb);
    }
}

/// The wake handle of the calling thread's executor.
pub(crate) fn current() -> Arc<ExecutorWake> {
    CURRENT.with(Arc::clone)
}

/// Wake every executor thread: some parked task may be ready.
///
/// This is the old process-wide wake, kept for callers that have not moved to a
/// per-task waker. A wake that belongs to one task must use that task's
/// `ReadinessWaker` instead.
pub fn resume_idle() {
    let handles: Vec<Arc<ExecutorWake>> = HANDLES
        .lock()
        .unwrap_or_else(PoisonError::into_inner)
        .iter()
        .filter_map(Weak::upgrade)
        .collect();
    for handle in handles {
        handle.wake();
    }
}

/// The executor loop is about to look for work: a wake from now on must not
/// be lost if the loop then decides to sleep.
pub fn checking() {
    let handle = current();
    handle
        .checked_at
        .store(handle.epoch.load(Ordering::SeqCst), Ordering::SeqCst);
}

/// Sleep the calling executor thread with `sleep`; a wake meanwhile runs
/// `interrupt`, which must make `sleep` return early.
pub fn park_idle(interrupt: Interrupt, sleep: impl FnOnce()) {
    let handle = current();
    let checked_at = handle.checked_at.swap(NOT_CHECKING, Ordering::SeqCst);
    *handle
        .sleeping
        .lock()
        .unwrap_or_else(PoisonError::into_inner) = Some(interrupt);
    // Registered: any later wake interrupts the sleep. A wake between the
    // check and the registration would be lost, so skip the sleep for it.
    if checked_at == NOT_CHECKING || handle.epoch.load(Ordering::SeqCst) == checked_at {
        sleep();
    }
    *handle
        .sleeping
        .lock()
        .unwrap_or_else(PoisonError::into_inner) = None;
}
