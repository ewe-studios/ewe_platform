//! Unit coverage for `CircuitBreaker` and `ErrorPolicy::classify`.
//!
//! These are pure state machines — no model needed — and drive the fallback and
//! error-classification decisions the loop relies on. Matrix rows 5.3-5.6.

use foundation_ai::agentic::errors::{AgentAction, AgenticError, CircuitBreaker, ErrorPolicy};
use foundation_ai::types::ModelId;

fn model(name: &str) -> ModelId {
    ModelId::Name(name.into(), None)
}

// ---------------------------------------------------------------------------
// CircuitBreaker — matrix 5.4/5.5/5.6

#[test]
fn breaker_does_not_trip_before_threshold() {
    let mut b = CircuitBreaker::new(3, vec![model("fallback")]);
    assert!(b.on_failure().is_none(), "1st failure must not trip");
    assert!(b.on_failure().is_none(), "2nd failure must not trip");
}

#[test]
fn breaker_trips_at_threshold_and_returns_fallback() {
    let mut b = CircuitBreaker::new(2, vec![model("fallback-1")]);
    assert!(b.on_failure().is_none());
    assert_eq!(
        b.on_failure(),
        Some(model("fallback-1")),
        "the breaker must return the fallback once the threshold is reached"
    );
}

#[test]
fn breaker_walks_fallbacks_in_order_then_exhausts() {
    // threshold 1 so each failure trips immediately.
    let mut b = CircuitBreaker::new(1, vec![model("a"), model("b")]);
    assert_eq!(b.on_failure(), Some(model("a")));
    assert_eq!(b.on_failure(), Some(model("b")));
    assert_eq!(
        b.on_failure(),
        None,
        "past the last fallback the breaker returns None (caller terminates)"
    );
}

#[test]
fn breaker_resets_on_success() {
    let mut b = CircuitBreaker::new(2, vec![model("fallback")]);
    assert!(b.on_failure().is_none());
    b.on_success();
    // After reset, one more failure must NOT trip (count restarted).
    assert!(
        b.on_failure().is_none(),
        "on_success must reset the failure count"
    );
}

#[test]
fn breaker_threshold_zero_is_clamped_to_one() {
    // new() clamps threshold to at least 1, so a 0 threshold still needs one
    // failure to trip rather than tripping with none.
    let mut b = CircuitBreaker::new(0, vec![model("fallback")]);
    assert_eq!(b.on_failure(), Some(model("fallback")));
}

// ---------------------------------------------------------------------------
// ErrorPolicy::classify — matrix 5.3

#[test]
fn classify_loop_detected_continues() {
    let policy = ErrorPolicy::new();
    let action = policy.classify(AgenticError::LoopDetected(
        foundation_ai::agentic::errors::LoopDetection {
            kind: "ExactLoop".into(),
            occurrences: 3,
        },
    ));
    assert!(
        matches!(action, AgentAction::Continue),
        "a loop-detected error should let the loop continue (F17 owns escalation)"
    );
}

#[test]
fn classify_unexpected_terminates() {
    let policy = ErrorPolicy::new();
    let action = policy.classify(AgenticError::Unexpected("boom".into()));
    assert!(
        matches!(action, AgentAction::Terminate(_)),
        "an unexpected error must terminate the turn"
    );
}

// ---------------------------------------------------------------------------
// AgenticError classification, Display, and FailedAction projection.
// These are pure and cover the from_generation / Display / into_failed_action
// paths that dominated errors.rs's uncovered regions.

use foundation_ai::errors::GenerationError;
use foundation_ai::types::SessionRecord;

fn gen_err(msg: &str) -> GenerationError {
    GenerationError::Generic(msg.to_string())
}

#[test]
fn from_generation_detects_rate_limit() {
    let e = AgenticError::from_generation(&gen_err("Rate limit exceeded (429)"), None, 4096);
    // A rate-limit classifies so ErrorPolicy switches model.
    assert!(
        matches!(ErrorPolicy::new().classify(e), AgentAction::SwitchModel),
        "a 429 must classify as a rate limit -> SwitchModel"
    );
}

#[test]
fn from_generation_defaults_to_provider_kind() {
    let e = AgenticError::from_generation(&gen_err("weird internal failure"), None, 4096);
    // A generic provider failure terminates.
    assert!(matches!(
        ErrorPolicy::new().classify(e),
        AgentAction::Terminate(_)
    ));
}

#[test]
fn display_renders_each_common_variant() {
    // Exercises the Display arms — each must be non-empty and self-describing.
    let cases = vec![
        AgenticError::Routing("no provider".into()),
        AgenticError::Session("bad state".into()),
        AgenticError::Queue("full".into()),
        AgenticError::Memory("io".into()),
        AgenticError::Unexpected("boom".into()),
        AgenticError::Budget { limit: 100 },
    ];
    for e in cases {
        let s = format!("{e}");
        assert!(!s.is_empty(), "Display must render {e:?}");
    }
}

#[test]
fn into_failed_action_carries_the_error_and_a_trace() {
    let err = AgenticError::Unexpected("kaboom".into());
    match err.into_failed_action() {
        SessionRecord::FailedAction { error, trace } => {
            assert!(format!("{error}").contains("kaboom"));
            assert!(
                !format!("{trace:?}").is_empty(),
                "a structured trace must be attached"
            );
        }
        other => panic!("expected FailedAction, got {other:?}"),
    }
}
