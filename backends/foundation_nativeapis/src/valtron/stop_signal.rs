/// A thread-safe stop signal for watcher tasks.
///
/// Shared between the task's `next_status()` and external code
/// (e.g., tests) that wants to terminate the watcher cleanly.
/// When `stop()` is called, `is_ready()` returns `true` so the executor
/// wakes the task, and the next `next_status()` call returns `None`.

use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::Duration;

use foundation_core::valtron::{EventReadiness, EventReadinessPtr};

#[derive(Clone)]
pub struct StopSignal(Arc<AtomicBool>);

impl StopSignal {
    /// Create a new stop signal (initially not stopped).
    pub fn new() -> Self {
        Self(Arc::new(AtomicBool::new(false)))
    }

    /// Signal the watcher to stop. The next `next_status()` call will
    /// return `None`, terminating the task.
    pub fn stop(&self) {
        self.0.store(true, Ordering::Relaxed);
    }

    /// Check if the stop signal has been set.
    pub fn is_stopped(&self) -> bool {
        self.0.load(Ordering::Relaxed)
    }
}

impl Default for StopSignal {
    fn default() -> Self {
        Self::new()
    }
}

/// Implements `EventReadiness` so the executor wakes the task when stopped.
/// When `stop()` is called, `is_ready()` returns `true` immediately, causing
/// the executor to reschedule the task. `next_status()` then checks
/// `is_stopped()` and returns `None`.
impl EventReadiness for StopSignal {
    fn is_ready(&self, _dur: Option<Duration>) -> bool {
        self.is_stopped()
    }
}

/// Combines two `EventReadiness` signals. Returns `true` if EITHER signal
/// is ready. Used to wake a parked task when the watcher has events OR
/// when `StopSignal.stop()` has been called.
pub struct CompositeReadiness {
    a: EventReadinessPtr,
    b: EventReadinessPtr,
}

impl CompositeReadiness {
    pub fn new(a: EventReadinessPtr, b: EventReadinessPtr) -> Self {
        Self { a, b }
    }
}

impl EventReadiness for CompositeReadiness {
    fn is_ready(&self, dur: Option<Duration>) -> bool {
        // Check both — if either is ready, return true.
        // Short-circuit: if `a` is ready, don't need to check `b`.
        self.a.is_ready(dur) || self.b.is_ready(dur)
    }
}

