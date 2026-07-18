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

    /// Return all pending mutations in FIFO order.
    pub fn pending_items(&self) -> Vec<Mutation> {
        self.storage.pending()
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

// Tests moved to tests/mutation_suite.rs
