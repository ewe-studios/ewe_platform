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
        let last_attempt = state.attempt.clone();
        if last_attempt >= state.total_allowed {
            return None;
        }

        let next_attempt = last_attempt.saturating_add(1);

        // create exponential duraton
        let exponent = self.factor.saturating_pow(next_attempt as u32);
        let duration = self.min_duration.saturating_mul(exponent as u32);

        // Apply jitter - use multiples of 100 to prevent rely on floats.
        let jitter_factor = (self.jitter * 100f32) as u32;
        let random = self.rng.borrow_mut().u32(0..jitter_factor * 2);

        let mut duration = duration.saturating_mul(100);
        if random < jitter_factor {
            let jitter = duration.saturating_mul(random as u32) / 100;
            duration = duration.saturating_sub(jitter)
        } else {
            let jitter = duration.saturating_mul((random as u32) / 2) / 100;
            duration = duration.saturating_add(jitter)
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

#[cfg(test)]
mod exponential_retry_test {
    use super::ExponentialBackoffDecider;
    use super::RetryDecider;
    use super::RetryState;

    #[test]
    fn can_generate_exponential_backoff() {
        let decider = ExponentialBackoffDecider::default();

        let base = RetryState {
            total_allowed: 2,
            attempt: 0,
            wait: None,
        };

        let reconnection_state = decider.decide(base.clone()).expect("should get returned");
        assert!(matches!(reconnection_state.wait, Some(_)));
        assert_eq!(reconnection_state.attempt, 1);

        let reconnection_state2 = decider
            .decide(reconnection_state.clone())
            .expect("should get returned");
        assert!(matches!(reconnection_state2.wait, Some(_)));
        assert_eq!(reconnection_state2.attempt, 2);

        let reconnection_state3 = decider.decide(reconnection_state2.clone());
        dbg!(&reconnection_state3);
        assert!(matches!(reconnection_state3, None));
    }
}
