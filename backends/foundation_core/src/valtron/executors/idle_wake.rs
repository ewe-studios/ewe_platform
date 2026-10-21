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
    /// The resume count when this thread's executor began its current check.
    static CHECKED_AT: Cell<u64> = const { Cell::new(NOT_CHECKING) };
}

/// The executor loop is about to look for work: a wake from now on must not
/// be lost if the loop then decides to sleep.
pub fn checking() {
    CHECKED_AT.with(|c| c.set(RESUMES.load(Ordering::SeqCst)));
}
static SLEEPING: Mutex<Vec<(u64, Interrupt)>> = Mutex::new(Vec::new());

/// Wake every sleeping executor thread: a parked task may be ready.
pub fn resume_idle() {
    RESUMES.fetch_add(1, Ordering::SeqCst);
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
