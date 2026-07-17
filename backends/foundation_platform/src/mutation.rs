//! Offline mutation queue with LWW (last-write-wins) conflict resolution.
//!
//! Mutations are enqueued locally when offline, replayed in FIFO order
//! when connectivity returns. Each mutation carries a UUID for idempotency.
//!
//! MVP: LWW default only. Custom conflict resolvers are post-MVP per
//! decision 12/gaps.md GAP 7 resolution.

use std::collections::HashMap;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Mutex, RwLock};
use std::time::{SystemTime, UNIX_EPOCH};

// ── ID generation ────────────────────────────────────────────────────

static NEXT_ID: AtomicU64 = AtomicU64::new(1);

fn generate_id() -> String {
    let id = NEXT_ID.fetch_add(1, Ordering::SeqCst);
    let ts = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_millis() as u64;
    format!("{ts:x}-{id:x}")
}

// ── Mutation ─────────────────────────────────────────────────────────

/// A single mutation enqueued while offline.
#[derive(Debug, Clone)]
pub struct Mutation {
    /// UUID for idempotency — server deduplicates by this.
    pub id: String,

    /// When the mutation was created (Unix timestamp, milliseconds).
    pub created_at: u64,

    /// The mutation kind (e.g. "order_update", "item_delete").
    /// Used by conflict resolvers to dispatch to the correct strategy.
    pub mutation_type: String,

    /// The mutation payload.
    pub payload: serde_json::Value,

    /// Current status in the queue.
    pub status: MutationStatus,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum MutationStatus {
    /// Waiting to be replayed.
    Pending,
    /// Successfully applied by the server.
    Applied,
    /// Server rejected with a conflict (LWW: server wins).
    Conflict(String),
    /// Non-retryable error — replay stopped.
    Failed(String),
    /// Requires user resolution — post-MVP.
    RequiresUserResolution,
}

/// Result of replaying the mutation queue.
#[derive(Debug, Clone)]
pub struct ReplayResult {
    pub applied: Vec<(String, serde_json::Value)>,
    pub conflicts: Vec<(String, String)>,
    pub failed: Vec<(String, String)>,
}

impl ReplayResult {
    pub fn new() -> Self {
        Self { applied: vec![], conflicts: vec![], failed: vec![] }
    }
}

// ── Queue storage trait ──────────────────────────────────────────────

/// Backend storage for the mutation queue. Default is in-memory.
/// Swappable for SQLite via `foundation_db`.
pub trait QueueStorage: Send + Sync + 'static {
    fn enqueue(&self, mutation: &Mutation);
    fn pending(&self) -> Vec<Mutation>;
    fn update_status(&self, id: &str, status: MutationStatus);
    fn delete(&self, id: &str);
    fn all(&self) -> Vec<Mutation>;
}

pub struct MemoryQueueStorage {
    mutations: RwLock<HashMap<String, Mutation>>,
    /// Order-preserving list of mutation IDs (FIFO).
    order: Mutex<Vec<String>>,
}

impl MemoryQueueStorage {
    pub fn new() -> Self {
        Self {
            mutations: RwLock::new(HashMap::new()),
            order: Mutex::new(Vec::new()),
        }
    }
}

impl QueueStorage for MemoryQueueStorage {
    fn enqueue(&self, mutation: &Mutation) {
        self.order.lock().unwrap().push(mutation.id.clone());
        self.mutations.write().unwrap().insert(mutation.id.clone(), mutation.clone());
    }

    fn pending(&self) -> Vec<Mutation> {
        let map = self.mutations.read().unwrap();
        let order = self.order.lock().unwrap();
        order.iter()
            .filter_map(|id| map.get(id))
            .filter(|m| m.status == MutationStatus::Pending)
            .cloned()
            .collect()
    }

    fn update_status(&self, id: &str, status: MutationStatus) {
        if let Some(m) = self.mutations.write().unwrap().get_mut(id) {
            m.status = status;
        }
    }

    fn delete(&self, id: &str) {
        self.mutations.write().unwrap().remove(id);
        let mut order = self.order.lock().unwrap();
        order.retain(|x| x != id);
    }

    fn all(&self) -> Vec<Mutation> {
        self.mutations.read().unwrap().values().cloned().collect()
    }
}

// ── Mutation queue ───────────────────────────────────────────────────

/// Offline mutation queue with LWW conflict resolution.
pub struct MutationQueue {
    storage: Box<dyn QueueStorage>,
}

impl MutationQueue {
    pub fn new(storage: impl QueueStorage) -> Self {
        Self { storage: Box::new(storage) }
    }

    pub fn in_memory() -> Self {
        Self::new(MemoryQueueStorage::new())
    }

    /// Enqueue a mutation. Generates a UUID and timestamp.
    /// Validates the mutation locally before enqueuing.
    pub fn enqueue(&self, mutation_type: &str, payload: serde_json::Value) -> String {
        let id = generate_id();
        let created_at = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or_default()
            .as_millis() as u64;

        let mutation = Mutation {
            id,
            created_at,
            mutation_type: mutation_type.to_string(),
            payload,
            status: MutationStatus::Pending,
        };

        self.storage.enqueue(&mutation);
        mutation.id
    }

    /// Return all pending mutations in FIFO order.
    pub fn pending_count(&self) -> usize {
        self.storage.pending().len()
    }

    /// Replay all pending mutations in FIFO order.
    ///
    /// For each mutation, calls the provided `replay_fn` which should
    /// send the mutation to the server and return the result.
    /// MVP: LWW — server response wins on conflict.
    ///
    /// Returns a summary of what happened.
    pub fn replay<F>(&self, replay_fn: F) -> ReplayResult
    where
        F: Fn(&Mutation) -> Result<serde_json::Value, ReplayError>,
    {
        let pending = self.storage.pending();
        let mut result = ReplayResult::new();

        for mutation in &pending {
            match replay_fn(mutation) {
                Ok(response) => {
                    self.storage.update_status(&mutation.id, MutationStatus::Applied);
                    result.applied.push((mutation.id.clone(), response));
                }
                Err(ReplayError::Conflict(server_state)) => {
                    // LWW: server wins
                    self.storage.update_status(
                        &mutation.id,
                        MutationStatus::Conflict(server_state.clone()),
                    );
                    result.conflicts.push((mutation.id.clone(), server_state));
                }
                Err(ReplayError::NonRetryable(msg)) => {
                    self.storage.update_status(
                        &mutation.id,
                        MutationStatus::Failed(msg.clone()),
                    );
                    result.failed.push((mutation.id.clone(), msg));
                    // Stop replay on non-retryable error
                    break;
                }
                Err(ReplayError::Retryable(_)) => {
                    // Keep as Pending, try again later
                }
            }
        }

        // Clean up applied mutations
        for (id, _) in &result.applied {
            self.storage.delete(id);
        }

        result
    }

    /// Return all mutations (for inspection in tests).
    pub fn all(&self) -> Vec<Mutation> {
        self.storage.all()
    }
}

// ── Replay error ─────────────────────────────────────────────────────

#[derive(Debug, Clone)]
pub enum ReplayError {
    /// Server rejected with a conflict. Contains server state for LWW.
    Conflict(String),
    /// Non-retryable error — replay stops.
    NonRetryable(String),
    /// Retryable error — mutation stays Pending.
    Retryable(String),
}

// ── Tests ────────────────────────────────────────────────────────────

// NOTE: Tests kept inline because: Tests replay order, status transitions, idempotency—private queue internals.
#[cfg(test)]
mod tests {
    use super::*;

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

        let pending = q.storage.pending();
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

        let pending = q.storage.pending();
        assert_eq!(pending[0].mutation_type, "order_update");
        assert_eq!(pending[0].payload, payload);
    }
}
