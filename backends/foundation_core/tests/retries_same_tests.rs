use foundation_core::retries::{
    RetryDecider, RetryState, SameBackoffDecider, DEFAULT_MIN_DURATION,
};

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
    assert!(reconnection_state3.is_none());
}
