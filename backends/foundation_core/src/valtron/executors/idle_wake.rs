//! Native: wake idle executor threads when a parked task becomes ready.
//!
//! WHY: a native executor with nothing runnable sleeps its thread for a back-off
//! (up to 100 ms). A task parked on `TaskStatus::Depends` becomes runnable when
//! its signal flips or its future's waker fires, but nothing told the sleeping
//! thread, so every such wake cost the rest of that back-off — a consumer that
//! parks three times per item spent ~300 ms per item.
//!
//! WHAT: an executor thread registers an interrupt just before it sleeps
//! ([`park_idle`]); [`resume_idle`] — called through
//! `request_executor_resume` by `WakeSignal` and by every bridged future's
//! waker — runs the registered interrupts.
//!
//! HOW: a sleeping thread is in [`SLEEPING`] only while it sleeps, so a wake
//! interrupts exactly the threads asleep at that moment. A wake can also land
//! between the executor's "nothing runnable" check and its registration: each
//! executor loop iteration calls [`checking`] before it looks for work, and
//! [`park_idle`] skips the sleep if a wake arrived since then. Only wakes in
//! that short window skip a sleep, so unrelated wake traffic elsewhere in the
//! process cannot spin an idle executor. (An earlier version skipped the sleep
//! for any wake since the thread's last sleep; under heavy traffic that
//! skipped nearly every sleep.) Spurious wakes are harmless: the executor
//! re-checks its work and sleeps again.

use std::cell::Cell;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Arc, Mutex, PoisonError};

type Interrupt = Arc<dyn Fn() + Send + Sync>;

static NEXT_ID: AtomicU64 = AtomicU64::new(0);
static RESUMES: AtomicU64 = AtomicU64::new(0);

/// Not checking: a sleep without a preceding [`checking`] never skips.
const NOT_CHECKING: u64 = u64::MAX;

thread_local! {
    /// Set while this thread runs a consumer wake (see [`waking_consumer`]).
    static WAKING_CONSUMER: Cell<bool> = const { Cell::new(false) };

    /// The resume count when this thread's executor began its current check.
    static CHECKED_AT: Cell<u64> = const { Cell::new(NOT_CHECKING) };
}

/// The executor loop is about to look for work: a wake from now on must not
/// be lost if the loop then decides to sleep.
pub fn checking() {
    CHECKED_AT.with(|c| c.set(RESUMES.load(Ordering::SeqCst)));
}
static SLEEPING: Mutex<Vec<(u64, Interrupt)>> = Mutex::new(Vec::new());

/// One-shot callbacks run by the next [`resume_idle`].
static LISTENERS: Mutex<Vec<Box<dyn FnOnce() + Send>>> = Mutex::new(Vec::new());

/// Run `cb` once, on the thread that next calls [`resume_idle`].
///
/// WHY: an async consumer of a single-thread `DrivenStreamIterator` drives the
/// engine itself and has no executor thread to interrupt. A parked task becomes
/// ready when a foreign thread flips its readiness and calls
/// `request_executor_resume`; the consumer's waker is registered here so that
/// call also wakes the consumer.
///
/// The list is cleared when it fires, so it holds a callback only until the next
/// resume. A caller that registers on every poll must keep its own "already
/// registered" flag, or a long quiet period grows the list.
// Used by the single-thread `DrivenStreamIterator` only.
#[cfg_attr(feature = "multi", allow(dead_code))]
pub fn register_resume_listener(cb: Box<dyn FnOnce() + Send>) {
    LISTENERS
        .lock()
        .unwrap_or_else(PoisonError::into_inner)
        .push(cb);
}

/// Run `wake`, a consumer wake (a `Waker::wake` of a `DrivenStreamIterator`
/// consumer), so that any [`resume_idle`] it causes on this thread does not fire
/// the resume listeners.
///
/// WHY: a consumer whose waker belongs to a valtron future (a bridged future, a
/// `block_on_future`) calls `request_executor_resume` when it is woken. Without
/// this guard that resume would fire every other consumer's listener; each of
/// those wakes calls resume again, and the consumers wake one another without
/// end (thousands of readiness re-checks a second). Sleeping executor threads
/// are still interrupted: only the listeners are skipped. A real flip of a
/// readiness calls `request_executor_resume` outside this guard, so it still
/// reaches every listener.
pub fn waking_consumer(wake: impl FnOnce()) {
    let outer = WAKING_CONSUMER.with(|c| c.replace(true));
    wake();
    WAKING_CONSUMER.with(|c| c.set(outer));
}

/// Wake every sleeping executor thread: a parked task may be ready.
pub fn resume_idle() {
    RESUMES.fetch_add(1, Ordering::SeqCst);
    // Taken out of the lock first: a listener may register another one. Not
    // taken at all while this thread is waking a consumer (see
    // [`waking_consumer`]).
    if !WAKING_CONSUMER.with(Cell::get) {
        let listeners =
            std::mem::take(&mut *LISTENERS.lock().unwrap_or_else(PoisonError::into_inner));
        for listener in listeners {
            listener();
        }
    }
    let sleeping: Vec<Interrupt> = SLEEPING
        .lock()
        .unwrap_or_else(PoisonError::into_inner)
        .iter()
        .map(|(_, interrupt)| Arc::clone(interrupt))
        .collect();
    for interrupt in sleeping {
        interrupt();
    }
}

/// Sleep the calling executor thread with `sleep`; a wake meanwhile
/// ([`resume_idle`]) runs `interrupt`, which must make `sleep` return early.
pub fn park_idle(interrupt: Interrupt, sleep: impl FnOnce()) {
    let checked_at = CHECKED_AT.with(|c| c.replace(NOT_CHECKING));
    let id = NEXT_ID.fetch_add(1, Ordering::Relaxed);
    SLEEPING
        .lock()
        .unwrap_or_else(PoisonError::into_inner)
        .push((id, interrupt));
    // Registered: any later wake interrupts the sleep. A wake between the
    // check and the registration would be lost, so skip the sleep for it.
    if checked_at == NOT_CHECKING || RESUMES.load(Ordering::SeqCst) == checked_at {
        sleep();
    }
    SLEEPING
        .lock()
        .unwrap_or_else(PoisonError::into_inner)
        .retain(|(entry, _)| *entry != id);
}
