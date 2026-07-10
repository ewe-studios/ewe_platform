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

// ── tests ───────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::Arc;
    use std::thread;

    #[test]
    fn timer_readiness_fires_after_interval() {
        let timer = TimerReadiness::new(Duration::from_millis(50));
        // First call: last_ready is now(), elapsed ≈ 0. Not ready yet.
        assert!(!timer.is_ready(None));

        // After the interval: should fire.
        thread::sleep(Duration::from_millis(60));
        assert!(timer.is_ready(None));

        // Resets on fire — next immediate call should be false.
        assert!(!timer.is_ready(None));
    }

    #[test]
    fn timer_readiness_implements_event_readiness() {
        let timer = Arc::new(TimerReadiness::new(Duration::from_millis(0)));
        // 0ms interval — always fires immediately.
        let ready: Arc<dyn EventReadiness + Send + Sync> = timer;
        assert!(ready.is_ready(None));
        // Also fires on second call — 0ms interval is always ready.
        assert!(ready.is_ready(None));
    }
}
