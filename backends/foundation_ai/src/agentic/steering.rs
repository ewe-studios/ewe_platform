//! Steering queues & cancel signals (F13).
//!
//! WHY: The agent loop must be interruptible — the user can inject high-priority
//! messages (stop, pause, redirect) that should preempt current tool execution.
//!
//! WHAT: Two `Arc<ConcurrentQueue<Messages>>` queues (priority for immediate
//! interrupt, follow-up for deferred injection) and an `Arc<AtomicU32>` cancel
//! signal carrying `CancelCode as u32`. Queue waits use `TaskStatus::Depends`
//! with `QueueReadiness` so the loop never spins on Pending/Delayed.

use std::sync::atomic::{AtomicU32, Ordering};
use std::sync::Arc;

use concurrent_queue::ConcurrentQueue;
use foundation_core::valtron::{EventReadiness, QueueReadiness};

use crate::types::Messages;

// ---------------------------------------------------------------------------
// CancelCode

/// Cancellation/steering signal carried by an `Arc<AtomicU32>`.
/// The atomic holds one of these values as `u32`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(u32)]
pub enum CancelCode {
    /// No cancellation requested; normal execution.
    None = 0,
    /// Pause current work and handle priority messages first.
    PauseForPriority = 1,
    /// Abort current session entirely.
    Abort = 2,
}

impl CancelCode {
    /// Read the current cancel code from an atomic signal.
    pub fn load(signal: &Arc<AtomicU32>) -> Self {
        match signal.load(Ordering::SeqCst) {
            1 => CancelCode::PauseForPriority,
            2 => CancelCode::Abort,
            _ => CancelCode::None,
        }
    }

    /// Store this cancel code into an atomic signal.
    pub fn store(self, signal: &Arc<AtomicU32>) {
        signal.store(self as u32, Ordering::SeqCst);
    }

    /// Reset the signal to `None`.
    pub fn reset(signal: &Arc<AtomicU32>) {
        CancelCode::None.store(signal);
    }

    /// Check if any cancellation is pending.
    #[must_use]
    pub fn is_active(self) -> bool {
        self != CancelCode::None
    }
}

// ---------------------------------------------------------------------------
// SteeringQueues

/// The two steering queues used to inject messages into the agent loop.
pub struct SteeringQueues {
    /// High-priority queue — messages here should interrupt current work.
    pub priority: Arc<ConcurrentQueue<Messages>>,
    /// Follow-up queue — messages here are appended after current work completes.
    pub follow_up: Arc<ConcurrentQueue<Messages>>,
    /// Atomic cancel signal read by the agent loop.
    pub cancel_signal: Arc<AtomicU32>,
}

impl SteeringQueues {
    /// Create new empty steering queues.
    #[must_use]
    pub fn new() -> Self {
        Self {
            priority: Arc::new(ConcurrentQueue::unbounded()),
            follow_up: Arc::new(ConcurrentQueue::unbounded()),
            cancel_signal: Arc::new(AtomicU32::new(CancelCode::None as u32)),
        }
    }

    /// Wrap existing shared handles — used when the session holds Arc handles
    /// and the `AgentLoop` needs its own `SteeringQueues` view.
    #[must_use]
    pub fn from_shared(
        priority: Arc<ConcurrentQueue<Messages>>,
        follow_up: Arc<ConcurrentQueue<Messages>>,
        cancel_signal: Arc<AtomicU32>,
    ) -> Self {
        Self {
            priority,
            follow_up,
            cancel_signal,
        }
    }

    /// Push a message to the priority queue (interrupts current work).
    pub fn push_priority(&self, msg: Messages) {
        let _ = self.priority.push(msg);
        CancelCode::PauseForPriority.store(&self.cancel_signal);
    }

    /// Push a message to the follow-up queue (processed after current work).
    pub fn push_follow_up(&self, msg: Messages) {
        let _ = self.follow_up.push(msg);
    }

    /// Pop a priority message, if any.
    #[must_use]
    pub fn pop_priority(&self) -> Option<Messages> {
        self.priority.pop().ok()
    }

    /// Pop a follow-up message, if any.
    #[must_use]
    pub fn pop_follow_up(&self) -> Option<Messages> {
        self.follow_up.pop().ok()
    }

    /// Check if the priority queue has messages.
    #[must_use]
    pub fn has_priority(&self) -> bool {
        !self.priority.is_empty()
    }

    /// Check if the follow-up queue has messages.
    #[must_use]
    pub fn has_follow_up(&self) -> bool {
        !self.follow_up.is_empty()
    }

    /// Get the current cancel code.
    #[must_use]
    pub fn cancel_code(&self) -> CancelCode {
        CancelCode::load(&self.cancel_signal)
    }

    /// Reset the cancel signal after handling.
    pub fn reset_cancel(&self) {
        CancelCode::reset(&self.cancel_signal);
    }

    /// Check if any steering is pending (priority messages or active cancel).
    #[must_use]
    pub fn is_active(&self) -> bool {
        self.has_priority() || self.cancel_code().is_active()
    }

    /// Readiness signal for `TaskStatus::Depends` — parks the agent task
    /// until a priority message arrives. Zero CPU spin.
    #[must_use]
    pub fn priority_readiness(&self) -> Arc<dyn EventReadiness> {
        Arc::new(QueueReadiness::new(self.priority.clone()))
    }

    /// Readiness signal for `TaskStatus::Depends` — parks the agent task
    /// until a follow-up message arrives.
    #[must_use]
    pub fn followup_readiness(&self) -> Arc<dyn EventReadiness> {
        Arc::new(QueueReadiness::new(self.follow_up.clone()))
    }

    /// Drain all priority messages (session end: caller persists, then clears).
    #[must_use]
    pub fn drain_priority(&self) -> Vec<Messages> {
        let mut out = Vec::new();
        while let Ok(msg) = self.priority.pop() {
            out.push(msg);
        }
        out
    }

    /// Drain all follow-up messages (session end: caller persists, then clears).
    #[must_use]
    pub fn drain_follow_up(&self) -> Vec<Messages> {
        let mut out = Vec::new();
        while let Ok(msg) = self.follow_up.pop() {
            out.push(msg);
        }
        out
    }
}

impl Default for SteeringQueues {
    fn default() -> Self {
        Self::new()
    }
}
