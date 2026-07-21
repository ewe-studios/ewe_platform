//! Tests for the offline mutation queue with LWW conflict resolution.

use foundation_platform::*;

fn queue() -> MutationQueue {
    MutationQueue::in_memory()
}

// ── Enqueue ──────────────────────────────────────────────────

#[test]
fn enqueue_produces_uuid() {
    let q = queue();
    let id = q.enqueue("test", serde_json::Value::String("data".into()));
    assert!(!id.is_empty());
    assert_eq!(q.pending_count(), 1);
}

#[test]
fn enqueue_preserves_order() {
    let q = queue();
    let id1 = q.enqueue("t1", serde_json::Value::String("first".into()));
    let id2 = q.enqueue("t2", serde_json::Value::String("second".into()));
    let id3 = q.enqueue("t3", serde_json::Value::String("third".into()));

    let pending = q.pending_items();
    assert_eq!(pending.len(), 3);
    assert_eq!(pending[0].id, id1);
    assert_eq!(pending[1].id, id2);
    assert_eq!(pending[2].id, id3);
}

// ── Replay: success ──────────────────────────────────────────

#[test]
fn replay_applies_all_pending() {
    let q = queue();
    q.enqueue("t1", serde_json::Value::String("a".into()));
    q.enqueue("t2", serde_json::Value::String("b".into()));

    let result = q.replay(|_m| Ok(serde_json::Value::String("ok".into())));

    assert_eq!(result.applied.len(), 2);
    assert_eq!(result.conflicts.len(), 0);
    assert_eq!(result.failed.len(), 0);
    assert_eq!(q.pending_count(), 0); // all cleaned up
}

// ── Replay: conflict ─────────────────────────────────────────

#[test]
fn replay_handles_conflict_with_lww() {
    let q = queue();
    q.enqueue("update", serde_json::Value::String("local".into()));

    let result = q.replay(|m| {
        Err(ReplayError::Conflict(format!("server-state-for-{}", m.id)))
    });

    assert_eq!(result.applied.len(), 0);
    assert_eq!(result.conflicts.len(), 1);
    assert_eq!(q.pending_count(), 0); // resolved (LWW: server wins)
}

// ── Replay: non-retryable error ───────────────────────────────

#[test]
fn replay_stops_on_non_retryable_error() {
    let q = queue();
    q.enqueue("t1", serde_json::Value::String("a".into()));
    q.enqueue("t2", serde_json::Value::String("b".into())); // will fail
    q.enqueue("t3", serde_json::Value::String("c".into())); // never replayed

    let result = q.replay(|m| {
        if m.payload == serde_json::Value::String("b".into()) {
            Err(ReplayError::NonRetryable("bad data".into()))
        } else {
            Ok(serde_json::Value::String("ok".into()))
        }
    });

    assert_eq!(result.applied.len(), 1); // only t1 applied
    assert_eq!(result.failed.len(), 1);  // t2 failed
    // t3 still pending — replay stopped
    assert_eq!(q.pending_count(), 1);
}

// ── Replay: retryable error ──────────────────────────────────

#[test]
fn replay_keeps_retryable_pending() {
    let q = queue();
    q.enqueue("t1", serde_json::Value::String("a".into()));

    let result = q.replay(|_| Err(ReplayError::Retryable("timeout".into())));

    assert_eq!(result.applied.len(), 0);
    assert_eq!(q.pending_count(), 1); // still pending, retry later
}

// ── Replay: mixed results ────────────────────────────────────

#[test]
fn replay_mixed_results() {
    let q = queue();
    q.enqueue("apply", serde_json::Value::String("a".into()));
    q.enqueue("conflict", serde_json::Value::String("c".into()));
    q.enqueue("fail", serde_json::Value::String("f".into()));
    q.enqueue("never", serde_json::Value::String("n".into()));

    let result = q.replay(|m| match m.payload.as_str().unwrap() {
        "a" => Ok(serde_json::Value::String("ok".into())),
        "c" => Err(ReplayError::Conflict("server-state".into())),
        "f" => Err(ReplayError::NonRetryable("bad".into())),
        _ => Ok(serde_json::Value::String("ok".into())),
    });

    assert_eq!(result.applied.len(), 1);
    assert_eq!(result.conflicts.len(), 1);
    assert_eq!(result.failed.len(), 1);
    // "never" still pending (replay stopped at fail)
    assert_eq!(q.pending_count(), 1);
}

#[test]
fn replay_empty_queue_does_nothing() {
    let q = queue();
    let result = q.replay(|_| panic!("should not be called"));
    assert_eq!(result.applied.len(), 0);
}

// ── Mutation status tracking ─────────────────────────────────

#[test]
fn applied_mutations_are_deleted_from_queue() {
    let q = queue();
    let _id = q.enqueue("t1", serde_json::Value::String("a".into()));
    q.replay(|_| Ok(serde_json::Value::String("ok".into())));

    let all = q.all();
    assert!(all.is_empty()); // deleted after successful replay
}

#[test]
fn conflicted_mutations_preserve_status() {
    let q = queue();
    q.enqueue("t1", serde_json::Value::String("a".into()));
    q.replay(|_| Err(ReplayError::Conflict("server-data".into())));

    let all = q.all();
    assert_eq!(all.len(), 1);
    assert!(matches!(all[0].status, MutationStatus::Conflict(_)));
}

// ── UUID uniqueness ──────────────────────────────────────────

#[test]
fn each_mutation_gets_unique_uuid() {
    let q = queue();
    let id1 = q.enqueue("t", serde_json::Value::Null);
    let id2 = q.enqueue("t", serde_json::Value::Null);
    assert_ne!(id1, id2);
}

// ── Type and payload preservation ────────────────────────────

#[test]
fn mutation_preserves_type_and_payload() {
    let q = queue();
    let payload = serde_json::json!({"key": "value", "num": 42});
    q.enqueue("order_update", payload.clone());

    let pending = q.pending_items();
    assert_eq!(pending[0].mutation_type, "order_update");
    assert_eq!(pending[0].payload, payload);
}
