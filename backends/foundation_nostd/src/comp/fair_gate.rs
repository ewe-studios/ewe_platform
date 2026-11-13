//! FIFO-fair serialization gate — a ticket-based mutex that guarantees
//! waiters are served in arrival order, preventing starvation under
//! heavy contention.
//!
//! Extracted from `foundation_core::valtron::executors::multi` so any
//! crate in the stack can serialize process-global resources (test
//! suites, singleton registries) without pulling in the full valtron
//! engine.

use core::sync::atomic::{AtomicUsize, Ordering};

use super::condvar_comp::{CondVar, CondVarMutex};

// `CondVarMutex` is `std::sync::Mutex` only in the `std`-without-`js-event-loop`
// configuration; every other configuration uses the crate's own mutex. Import
// the matching `PoisonError` so `into_inner` works as a plain function.
#[cfg(any(not(feature = "std"), feature = "js-event-loop"))]
use crate::primitives::poison::PoisonError;
#[cfg(all(feature = "std", not(feature = "js-event-loop")))]
use std::sync::PoisonError;

pub struct FairGate {
    next_ticket: AtomicUsize,
    now_serving: AtomicUsize,
    inner: CondVarMutex<()>,
    cond: CondVar,
}

impl Default for FairGate {
    fn default() -> Self {
        Self::new()
    }
}

impl FairGate {
    #[must_use]
    pub const fn new() -> Self {
        Self {
            next_ticket: AtomicUsize::new(0),
            now_serving: AtomicUsize::new(0),
            inner: CondVarMutex::new(()),
            cond: CondVar::new(),
        }
    }

    pub fn acquire(&'static self) -> FairGateGuard {
        let my_ticket = self.next_ticket.fetch_add(1, Ordering::SeqCst);
        let mut guard = self.inner.lock().unwrap_or_else(PoisonError::into_inner);
        while self.now_serving.load(Ordering::SeqCst) != my_ticket {
            guard = self
                .cond
                .wait(guard)
                .unwrap_or_else(PoisonError::into_inner);
        }
        // keep `guard` alive so the condvar mutex stays held — prevents
        // spurious wake races where two waiters both see their ticket.
        drop(guard);
        FairGateGuard { gate: self }
    }
}

pub static SERIAL_TEST_GATE: FairGate = FairGate::new();

pub struct FairGateGuard {
    gate: &'static FairGate,
}

impl Drop for FairGateGuard {
    fn drop(&mut self) {
        let _guard = self
            .gate
            .inner
            .lock()
            .unwrap_or_else(PoisonError::into_inner);
        self.gate.now_serving.fetch_add(1, Ordering::SeqCst);
        self.gate.cond.notify_all();
    }
}
