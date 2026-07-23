//! FIFO-fair serialization gate — a ticket-based mutex that guarantees
//! waiters are served in arrival order, preventing starvation under
//! heavy contention.
//!
//! Extracted from `foundation_core::valtron::executors::multi` so any
//! crate in the stack can serialize process-global resources (test
//! suites, singleton registries) without pulling in the full valtron
//! engine.

// `CondVarMutex` is `std::sync::Mutex` when std is available and this crate's
// own spin mutex otherwise, so `lock()` returns a *different* `PoisonError` type
// per feature set. The `|p| p.into_inner()` closures below are generic over
// both; naming either path (`std::sync::PoisonError::into_inner` or
// `crate::primitives::PoisonError::into_inner`) compiles under one feature set
// and fails under the other. Do not let clippy "simplify" them.
#![allow(clippy::redundant_closure_for_method_calls)]

use core::sync::atomic::{AtomicUsize, Ordering};

use super::condvar_comp::{CondVar, CondVarMutex};

pub struct FairGate {
    next_ticket: AtomicUsize,
    now_serving: AtomicUsize,
    inner: CondVarMutex<()>,
    cond: CondVar,
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
        let mut guard = self.inner.lock().unwrap_or_else(|p| p.into_inner());
        while self.now_serving.load(Ordering::SeqCst) != my_ticket {
            guard = self.cond.wait(guard).unwrap_or_else(|p| p.into_inner());
        }
        // keep `guard` alive so the condvar mutex stays held — prevents
        // spurious wake races where two waiters both see their ticket.
        drop(guard);
        FairGateGuard { gate: self }
    }
}

impl Default for FairGate {
    fn default() -> Self {
        Self::new()
    }
}

pub static SERIAL_TEST_GATE: FairGate = FairGate::new();

pub struct FairGateGuard {
    gate: &'static FairGate,
}

impl Drop for FairGateGuard {
    fn drop(&mut self) {
        let _guard = self.gate.inner.lock().unwrap_or_else(|p| p.into_inner());
        self.gate.now_serving.fetch_add(1, Ordering::SeqCst);
        self.gate.cond.notify_all();
    }
}
