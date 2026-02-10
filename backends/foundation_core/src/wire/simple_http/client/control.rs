//! Implementation of request control signals

use std::sync::atomic::{AtomicU8, Ordering};
use std::sync::Arc;

// State constants for atomic coordination
const STATE_NOT_STARTED: u8 = 0;
const STATE_INTRO_READY: u8 = 1;
const STATE_BODY_REQUESTED: u8 = 2;
#[allow(dead_code)]
const STATE_COMPLETED: u8 = 3;

/// Shared control for coordinating between task and ClientRequest.
///
/// WHY: ClientRequest needs to signal when body reading is desired, and task
/// needs to wait for that signal. Atomic coordination enables lock-free
/// communication.
///
/// WHAT: Arc-wrapped atomic state that both task and ClientRequest can access.
///
/// HOW: Clone-able handle with atomic operations for state changes.
#[derive(Clone, Debug)]
pub struct RequestControl {
    state: Arc<AtomicU8>,
}

impl RequestControl {
    /// Creates a new request control in NOT_STARTED state.
    pub fn new() -> Self {
        Self {
            state: Arc::new(AtomicU8::new(STATE_NOT_STARTED)),
        }
    }

    /// Signals that intro/headers are ready.
    pub fn set_intro_ready(&self) {
        self.state.store(STATE_INTRO_READY, Ordering::Release);
    }

    /// Signals that body reading is requested.
    pub fn set_body_requested(&self) {
        self.state.store(STATE_BODY_REQUESTED, Ordering::Release);
    }

    /// Gets current state.
    pub fn get_state(&self) -> u8 {
        self.state.load(Ordering::Acquire)
    }
}

impl Default for RequestControl {
    fn default() -> Self {
        Self::new()
    }
}
