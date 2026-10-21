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
//! HOW: a global resume counter closes the race where a wake lands between the
//! executor's "nothing runnable" check and its sleep: the sleep is skipped when
//! a resume happened since this thread last slept. Spurious wakes are harmless —
//! the executor re-checks its work and sleeps again.

use std::cell::Cell;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Arc, Mutex, PoisonError};

type Interrupt = Arc<dyn Fn() + Send + Sync>;

static RESUMES: AtomicU64 = AtomicU64::new(0);
static NEXT_ID: AtomicU64 = AtomicU64::new(0);
static SLEEPING: Mutex<Vec<(u64, Interrupt)>> = Mutex::new(Vec::new());

thread_local! {
    /// The resume count this thread has accounted for; `u64::MAX` = never
    /// slept yet (its first sleep starts from the current count).
    static SEEN: Cell<u64> = const { Cell::new(u64::MAX) };
}

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

/// Sleep the calling executor thread with `sleep`, unless a resume happened
/// since it last slept. `interrupt` must make `sleep` return early.
pub fn park_idle(interrupt: Interrupt, sleep: impl FnOnce()) {
    let now = RESUMES.load(Ordering::SeqCst);
    let seen = SEEN.with(Cell::get);
    if seen != u64::MAX && seen != now {
        // A wake arrived while this thread was working: re-check first.
        SEEN.with(|s| s.set(now));
        return;
    }
    let id = NEXT_ID.fetch_add(1, Ordering::Relaxed);
    SLEEPING
        .lock()
        .unwrap_or_else(PoisonError::into_inner)
        .push((id, interrupt));
    // A resume between the check above and the registration would be lost.
    if RESUMES.load(Ordering::SeqCst) == now {
        sleep();
    }
    SLEEPING
        .lock()
        .unwrap_or_else(PoisonError::into_inner)
        .retain(|(entry, _)| *entry != id);
    SEEN.with(|s| s.set(RESUMES.load(Ordering::SeqCst)));
}
