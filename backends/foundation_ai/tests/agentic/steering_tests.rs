#![allow(unused_imports)]
use foundation_ai::agentic::{CancelCode, SteeringQueues};
use foundation_ai::types::{base_types::TextContent, MessageRole, Messages, UserModelContent};
use std::sync::atomic::{AtomicU32, Ordering};
use std::sync::Arc;

fn user_msg(text: &str) -> Messages {
    Messages::User {
        id: foundation_compact::ids::new_scru128(),
        role: MessageRole::User,
        content: UserModelContent::Text(TextContent {
            content: text.into(),
            signature: None,
        }),
        signature: None,
    }
}

// ---------------------------------------------------------------------------
// CancelCode
// ---------------------------------------------------------------------------

#[test]
fn cancel_code_repr_u32_values() {
    assert_eq!(CancelCode::None as u32, 0);
    assert_eq!(CancelCode::PauseForPriority as u32, 1);
    assert_eq!(CancelCode::Abort as u32, 2);
}

#[test]
fn cancel_code_roundtrip_through_atomic() {
    let signal = Arc::new(AtomicU32::new(0));

    assert_eq!(CancelCode::load(&signal), CancelCode::None);
    assert!(!CancelCode::None.is_active());

    CancelCode::PauseForPriority.store(&signal);
    assert_eq!(signal.load(Ordering::SeqCst), 1);
    assert_eq!(CancelCode::load(&signal), CancelCode::PauseForPriority);
    assert!(CancelCode::PauseForPriority.is_active());

    CancelCode::Abort.store(&signal);
    assert_eq!(signal.load(Ordering::SeqCst), 2);
    assert_eq!(CancelCode::load(&signal), CancelCode::Abort);
    assert!(CancelCode::Abort.is_active());

    CancelCode::reset(&signal);
    assert_eq!(CancelCode::load(&signal), CancelCode::None);
}

#[test]
fn cancel_code_unknown_value_maps_to_none() {
    let signal = Arc::new(AtomicU32::new(99));
    assert_eq!(CancelCode::load(&signal), CancelCode::None);
}

// ---------------------------------------------------------------------------
// SteeringQueues — basic push/pop
// ---------------------------------------------------------------------------

#[test]
fn new_queues_are_empty() {
    let q = SteeringQueues::new();
    assert!(!q.has_priority());
    assert!(!q.has_follow_up());
    assert!(!q.is_active());
    assert_eq!(q.cancel_code(), CancelCode::None);
}

#[test]
fn push_priority_sets_cancel_signal() {
    let q = SteeringQueues::new();
    q.push_priority(user_msg("stop"));

    assert!(q.has_priority());
    assert_eq!(q.cancel_code(), CancelCode::PauseForPriority);
    assert!(q.is_active());
}

#[test]
fn push_follow_up_does_not_set_cancel_signal() {
    let q = SteeringQueues::new();
    q.push_follow_up(user_msg("later"));

    assert!(q.has_follow_up());
    assert_eq!(q.cancel_code(), CancelCode::None);
    assert!(!q.is_active());
}

#[test]
fn pop_priority_returns_fifo() {
    let q = SteeringQueues::new();
    q.push_priority(user_msg("first"));
    q.push_priority(user_msg("second"));

    let m1 = q.pop_priority().expect("should have first");
    let m2 = q.pop_priority().expect("should have second");
    assert!(q.pop_priority().is_none());

    if let Messages::User { content, .. } = &m1 {
        if let UserModelContent::Text(tc) = content {
            assert_eq!(tc.content, "first");
        }
    }
    if let Messages::User { content, .. } = &m2 {
        if let UserModelContent::Text(tc) = content {
            assert_eq!(tc.content, "second");
        }
    }
}

#[test]
fn pop_follow_up_returns_fifo() {
    let q = SteeringQueues::new();
    q.push_follow_up(user_msg("a"));
    q.push_follow_up(user_msg("b"));

    let m1 = q.pop_follow_up().expect("should have a");
    assert!(q.pop_follow_up().is_some());
    assert!(q.pop_follow_up().is_none());

    if let Messages::User { content, .. } = &m1 {
        if let UserModelContent::Text(tc) = content {
            assert_eq!(tc.content, "a");
        }
    }
}

#[test]
fn reset_cancel_clears_signal() {
    let q = SteeringQueues::new();
    q.push_priority(user_msg("interrupt"));
    assert_eq!(q.cancel_code(), CancelCode::PauseForPriority);

    q.reset_cancel();
    assert_eq!(q.cancel_code(), CancelCode::None);
    // Queue still has the message
    assert!(q.has_priority());
}

// ---------------------------------------------------------------------------
// Drain
// ---------------------------------------------------------------------------

#[test]
fn drain_priority_returns_all_and_empties() {
    let q = SteeringQueues::new();
    q.push_priority(user_msg("1"));
    q.push_priority(user_msg("2"));
    q.push_priority(user_msg("3"));

    let drained = q.drain_priority();
    assert_eq!(drained.len(), 3);
    assert!(!q.has_priority());
    assert!(q.drain_priority().is_empty());
}

#[test]
fn drain_follow_up_returns_all_and_empties() {
    let q = SteeringQueues::new();
    q.push_follow_up(user_msg("x"));
    q.push_follow_up(user_msg("y"));

    let drained = q.drain_follow_up();
    assert_eq!(drained.len(), 2);
    assert!(!q.has_follow_up());
    assert!(q.drain_follow_up().is_empty());
}

#[test]
fn drain_empty_queue_returns_empty_vec() {
    let q = SteeringQueues::new();
    assert!(q.drain_priority().is_empty());
    assert!(q.drain_follow_up().is_empty());
}

// ---------------------------------------------------------------------------
// Readiness — EventReadiness / QueueReadiness
// ---------------------------------------------------------------------------

#[test]
fn priority_readiness_false_when_empty() {
    let q = SteeringQueues::new();
    let ready = q.priority_readiness();
    assert!(!ready.is_ready(None));
}

#[test]
fn priority_readiness_true_after_push() {
    let q = SteeringQueues::new();
    let ready = q.priority_readiness();

    q.push_priority(user_msg("wake up"));
    assert!(ready.is_ready(None));
}

#[test]
fn priority_readiness_false_after_drain() {
    let q = SteeringQueues::new();
    let ready = q.priority_readiness();

    q.push_priority(user_msg("msg"));
    assert!(ready.is_ready(None));

    q.drain_priority();
    assert!(!ready.is_ready(None));
}

#[test]
fn followup_readiness_false_when_empty() {
    let q = SteeringQueues::new();
    let ready = q.followup_readiness();
    assert!(!ready.is_ready(None));
}

#[test]
fn followup_readiness_true_after_push() {
    let q = SteeringQueues::new();
    let ready = q.followup_readiness();

    q.push_follow_up(user_msg("later"));
    assert!(ready.is_ready(None));
}

// ---------------------------------------------------------------------------
// Multi-producer: multiple pushes from different "sources"
// ---------------------------------------------------------------------------

#[test]
fn multi_producer_priority() {
    let q = Arc::new(SteeringQueues::new());

    let handles: Vec<_> = (0..4)
        .map(|i| {
            let q = q.clone();
            std::thread::spawn(move || {
                q.push_priority(user_msg(&format!("producer-{i}")));
            })
        })
        .collect();

    for h in handles {
        h.join().unwrap();
    }

    let drained = q.drain_priority();
    assert_eq!(drained.len(), 4);
}

#[test]
fn multi_producer_follow_up() {
    let q = Arc::new(SteeringQueues::new());

    let handles: Vec<_> = (0..4)
        .map(|i| {
            let q = q.clone();
            std::thread::spawn(move || {
                q.push_follow_up(user_msg(&format!("followup-{i}")));
            })
        })
        .collect();

    for h in handles {
        h.join().unwrap();
    }

    let drained = q.drain_follow_up();
    assert_eq!(drained.len(), 4);
}

// ---------------------------------------------------------------------------
// Default trait
// ---------------------------------------------------------------------------

#[test]
fn default_same_as_new() {
    let q = SteeringQueues::default();
    assert!(!q.has_priority());
    assert!(!q.has_follow_up());
    assert_eq!(q.cancel_code(), CancelCode::None);
}
