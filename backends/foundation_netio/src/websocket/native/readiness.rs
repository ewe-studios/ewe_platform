//! `EventReadiness` impls for WebSocket tasks (F38).
//!
//! WHY: WebSocket tasks park on socket readiness. When a kernel reactor is
//! available the caller wraps the fd in `RegisteredFd`. When no reactor is
//! available, `TimerReadiness` provides a fallback — it returns true every
//! N milliseconds, giving the same `Depends` interface without a real kernel fd.
//!
//! WHAT: [`TimerReadiness`] — a simple `EventReadiness` impl backed by a wall-clock
//! timer. The task returns `TaskStatus::Depends(timer)` which parks the task
//! until the next tick. No epoll, no fd, no kernel — just a Mutex<Instant>.

use std::sync::Mutex;
use std::time::{Duration, Instant};

use foundation_core::valtron::EventReadiness;

/// A wall-clock `EventReadiness` fallback for when no kernel reactor is available.
///
/// Returns `true` every `interval`. Used by WS tasks to park via `Depends`
/// instead of polling with `Delayed`. The interval is the maximum time the task
/// stays parked before being re-polled — a shorter interval means lower latency
/// but more executor wake-ups.
#[derive(Debug)]
pub struct TimerReadiness {
    interval: Duration,
    last_ready: Mutex<Instant>,
}

impl TimerReadiness {
    /// Create a timer that returns `true` every `interval`.
    #[must_use]
    pub fn new(interval: Duration) -> Self {
        Self {
            interval,
            last_ready: Mutex::new(Instant::now()),
        }
    }
}

impl EventReadiness for TimerReadiness {
    fn is_ready(&self, _dur: Option<Duration>) -> bool {
        let mut last = self.last_ready.lock().unwrap();
        if last.elapsed() >= self.interval {
            *last = Instant::now();
            true
        } else {
            false
        }
    }
}
