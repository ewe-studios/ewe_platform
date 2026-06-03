/// A thread-safe stop signal for watcher tasks.
///
/// Shared between the task's `next_status()` and external code
/// (e.g., tests) that wants to terminate the watcher cleanly.
/// When `stop()` is called, the next `next_status()` call returns `None`,
/// removing the task from the executor.

use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};

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
