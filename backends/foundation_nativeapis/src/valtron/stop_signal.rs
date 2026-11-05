use std::sync::atomic::{AtomicBool, Ordering};
/// A thread-safe stop signal for watcher tasks.
///
/// Shared between the task's `next_status()` and external code
/// (e.g., tests) that wants to terminate the watcher cleanly.
/// When `stop()` is called, `is_ready()` returns `true` so the executor
/// wakes the task, and the next `next_status()` call returns `None`.
use std::sync::Arc;
use std::time::Duration;

use foundation_core::valtron::{
    EventReadiness, EventReadinessPtr, ReadinessNotifier, ReadinessWaker,
};

#[derive(Clone)]
pub struct StopSignal(Arc<StopSignalInner>);

struct StopSignalInner {
    flag: AtomicBool,
    /// The tasks parked on this signal, woken by `stop`.
    notifier: ReadinessNotifier,
}

impl StopSignal {
    /// Create a new stop signal (initially not stopped).
    pub fn new() -> Self {
        Self(Arc::new(StopSignalInner {
            flag: AtomicBool::new(false),
            notifier: ReadinessNotifier::new(),
        }))
    }

    /// Signal the watcher to stop. The next `next_status()` call will
    /// return `None`, terminating the task.
    pub fn stop(&self) {
        self.0.flag.store(true, Ordering::SeqCst);
        // Wake the tasks parked on this signal: they are not re-checked on a
        // timer (`register` returns `true`).
        self.0.notifier.notify();
    }

    /// Check if the stop signal has been set.
    pub fn is_stopped(&self) -> bool {
        self.0.flag.load(Ordering::SeqCst)
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

    /// `stop` wakes the registered task, so no timer re-check is needed.
    fn register(&self, waker: &ReadinessWaker) -> bool {
        self.0.notifier.register(waker)
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

    fn deadline(&self) -> Option<foundation_core::valtron::ReadinessInstant> {
        match (self.a.deadline(), self.b.deadline()) {
            (Some(a), Some(b)) => Some(a.min(b)),
            (a, b) => a.or(b),
        }
    }

    /// Registers the waker with both signals; `true` only if both wake.
    fn register(&self, waker: &ReadinessWaker) -> bool {
        let a = self.a.register(waker);
        let b = self.b.register(waker);
        a && b
    }
}
