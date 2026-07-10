//! Health probe consecutive-count state machine (`ProbeState`).

use foundation_proxy::health::ProbeState;

/// WHY: A healthy backend must not be ejected by a single failed probe; it takes
/// `unhealthy_threshold` consecutive failures.
/// WHAT: With threshold 3, two failures do nothing; the third flips to unhealthy.
#[test]
fn test_ejects_only_after_unhealthy_threshold() {
    let mut state = ProbeState::new(2, 3, true);
    assert_eq!(state.record(false), None);
    assert_eq!(state.record(false), None);
    assert_eq!(state.record(false), Some(false), "third failure ejects");
    assert!(!state.is_healthy());
}

/// WHY: A recovered backend rejoins only after `healthy_threshold` consecutive
/// successes.
/// WHAT: After ejection, one success does nothing; the second readmits.
#[test]
fn test_readmits_only_after_healthy_threshold() {
    let mut state = ProbeState::new(2, 3, true);
    state.record(false);
    state.record(false);
    state.record(false); // now unhealthy

    assert_eq!(state.record(true), None, "one success is not enough");
    assert_eq!(state.record(true), Some(true), "second success readmits");
    assert!(state.is_healthy());
}

/// WHY: The counts are *consecutive* — a break resets the opposing run.
/// WHAT: Failures interrupted by a success never reach the unhealthy threshold.
#[test]
fn test_intermittent_failures_reset_the_run() {
    let mut state = ProbeState::new(2, 3, true);
    assert_eq!(state.record(false), None);
    assert_eq!(state.record(false), None);
    assert_eq!(state.record(true), None, "success resets the failure run");
    assert_eq!(state.record(false), None);
    assert_eq!(state.record(false), None);
    assert!(state.is_healthy(), "never hit 3 consecutive failures");
}

/// WHY: No transition should fire while already in the target state.
/// WHAT: Repeated successes on a healthy backend return `None`.
#[test]
fn test_no_transition_when_already_in_state() {
    let mut state = ProbeState::new(2, 3, true);
    assert_eq!(state.record(true), None);
    assert_eq!(state.record(true), None);
    assert!(state.is_healthy());
}

/// WHY: A zero threshold would divide traffic by an impossible bar; it is
/// clamped to 1.
/// WHAT: With `unhealthy_threshold` given as 0, one failure ejects.
#[test]
fn test_thresholds_clamped_to_at_least_one() {
    let mut state = ProbeState::new(0, 0, true);
    assert_eq!(state.record(false), Some(false), "clamped threshold of 1 ejects on first failure");
}
