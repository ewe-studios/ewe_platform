use foundation_core::retries::{ExponentialBackoffDecider, RetryDecider, RetryState};

#[test]
fn can_generate_exponential_backoff() {
    let decider = ExponentialBackoffDecider::default();

    let base = RetryState {
        total_allowed: 2,
        attempt: 0,
        wait: None,
    };

    let reconnection_state = decider.decide(base.clone()).expect("should get returned");
    assert!(reconnection_state.wait.is_some());
    assert_eq!(reconnection_state.attempt, 1);

    let reconnection_state2 = decider
        .decide(reconnection_state.clone())
        .expect("should get returned");
    assert!(reconnection_state2.wait.is_some());
    assert_eq!(reconnection_state2.attempt, 2);

    let reconnection_state3 = decider.decide(reconnection_state2.clone());
    dbg!(&reconnection_state3);
    assert!(reconnection_state3.is_none());
}
