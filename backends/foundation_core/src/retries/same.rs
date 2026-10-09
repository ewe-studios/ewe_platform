use std::time;

use super::{RetryDecider, RetryState, DEFAULT_MIN_DURATION};

#[derive(Clone, Debug)]
pub struct SameBackoffDecider(time::Duration);

impl Default for SameBackoffDecider {
    fn default() -> Self {
        Self::new(DEFAULT_MIN_DURATION)
    }
}

impl SameBackoffDecider {
    pub fn new(duration: impl Into<time::Duration>) -> Self {
        Self(duration.into())
    }
}

impl RetryDecider for SameBackoffDecider {
    fn decide(&self, state: RetryState) -> Option<RetryState> {
        let last_attempt = state.attempt;
        if last_attempt >= state.total_allowed {
            return None;
        }

        let next_attempt = last_attempt.saturating_add(1);
        Some(RetryState {
            wait: Some(self.0),
            attempt: next_attempt,
            total_allowed: state.total_allowed,
        })
    }
}
