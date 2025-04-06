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

#[cfg(test)]
mod same_retry_test {
    use super::RetryState;

    use super::RetryDecider;
    use super::SameBackoffDecider;
    use super::DEFAULT_MIN_DURATION;

    #[test]
    fn can_generate_exponential_backoff() {
        let decider = SameBackoffDecider::default();

        let base = RetryState {
            total_allowed: 2,
            attempt: 0,
            wait: None,
        };

        let reconnection_state = decider.decide(base.clone()).expect("should get returned");
        assert!(matches!(
            reconnection_state.wait,
            Some(DEFAULT_MIN_DURATION)
        ));
        assert_eq!(reconnection_state.attempt, 1);

        let reconnection_state2 = decider
            .decide(reconnection_state.clone())
            .expect("should get returned");

        assert!(matches!(
            reconnection_state2.wait,
            Some(DEFAULT_MIN_DURATION)
        ));
        assert_eq!(reconnection_state2.attempt, 2);

        let reconnection_state3 = decider.decide(reconnection_state2.clone());
        assert!(matches!(reconnection_state3, None));
    }
}
