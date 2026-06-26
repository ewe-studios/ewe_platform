use std::{cell, time};

use super::{RetryDecider, RetryState, DEFAULT_MIN_DURATION};

#[derive(Clone, Debug)]
pub struct ExponentialBackoffDecider {
    pub factor: u32,
    pub jitter: f32,
    pub min_duration: time::Duration,
    pub max_duration: time::Duration,
    pub rng: cell::RefCell<fastrand::Rng>,
}

const DEFAULT_JITTER: f32 = 0.6;
const DEFAULT_FACTOR: u32 = 3;

impl Default for ExponentialBackoffDecider {
    /// Returns a `ExponentialBackoffDecider` which uses the default `DEFAULT_FACTOR`
    /// and `DEFAULT_JITTER` and `DEFAULT_MIN_DURATION` values the diferent arguments
    /// required for creating an exponential backoff retry decider.
    fn default() -> Self {
        Self::new(DEFAULT_FACTOR, DEFAULT_JITTER, DEFAULT_MIN_DURATION, None)
    }
}

impl ExponentialBackoffDecider {
    pub fn new(
        factor: u32,
        jitter: f32,
        min_duration: time::Duration,
        max_duration: impl Into<Option<time::Duration>>,
    ) -> Self {
        assert!(
            jitter > 0f32 && jitter < 1f32,
            "<exponential-backoff>: jitter must be between 0 and 1."
        );
        Self {
            factor,
            jitter,
            min_duration,
            rng: cell::RefCell::new(fastrand::Rng::new()),
            max_duration: max_duration.into().unwrap_or(time::Duration::MAX),
        }
    }

    pub fn from_duration(
        min_duration: time::Duration,
        max_duration: impl Into<Option<time::Duration>>,
    ) -> Self {
        Self::new(DEFAULT_FACTOR, DEFAULT_JITTER, min_duration, max_duration)
    }
}

impl RetryDecider for ExponentialBackoffDecider {
    fn decide(&self, state: RetryState) -> Option<RetryState> {
        let last_attempt = state.attempt;
        if last_attempt >= state.total_allowed {
            return None;
        }

        let next_attempt = last_attempt.saturating_add(1);

        // create exponential duraton
        let exponent = self.factor.saturating_pow(next_attempt);
        let duration = self.min_duration.saturating_mul(exponent);

        // Apply jitter - use multiples of 100 to prevent rely on floats.
        let jitter_factor = (self.jitter * 100f32) as u32;
        let random = self.rng.borrow_mut().u32(0..jitter_factor * 2);

        let mut duration = duration.saturating_mul(100);
        if random < jitter_factor {
            let jitter = duration.saturating_mul(random) / 100;
            duration = duration.saturating_sub(jitter);
        } else {
            let jitter = duration.saturating_mul(random / 2) / 100;
            duration = duration.saturating_add(jitter);
        }

        duration /= 100;

        // keep within boundaries
        duration = duration.clamp(self.min_duration, self.max_duration);

        Some(RetryState {
            wait: Some(duration),
            attempt: next_attempt,
            total_allowed: state.total_allowed,
        })
    }
}
