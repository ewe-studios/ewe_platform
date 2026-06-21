//! `MemoryStore` — the fast "latest memory per tier" cache (F07).
//!
//! On session resume the agent needs Working / Observation / Reflection memory
//! fast — it should not scan the whole audit log. `MemoryStore` keeps a direct,
//! overwriting pointer to the latest memory `SessionRecord` of each tier per
//! session. `hydrate` is one KV get (single-key bundle) → `SessionMemory`.
//!
//! `MemoryStore` is a **dumb cache** — it does not know about `DocumentStore`.
//! A `MemoryCoordinator` (in `memory_coordinator.rs`) owns both stores, does
//! dual-write, and the `record_type` fallback.

use foundation_db::traits::KeyValueStore;
use foundation_db::{StorageError, StorageResult};
use serde::{Deserialize, Serialize};

use crate::types::{SessionId, SessionRecord};

// ---------------------------------------------------------------------------
// MemoryTier

/// Which memory layer a record belongs to.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum MemoryTier {
    /// Permanent curated facts (Tier 1).
    Working,
    /// Time-scoped structured observations (Tier 2).
    Observation,
    /// Condensed reflections over observations (Tier 3).
    Reflection,
}

impl MemoryTier {
    /// Extract the tier from a memory `SessionRecord`.
    #[must_use]
    pub fn of(record: &SessionRecord) -> Option<Self> {
        match record {
            SessionRecord::WorkingMemory { .. } => Some(MemoryTier::Working),
            SessionRecord::Observation { .. } => Some(MemoryTier::Observation),
            SessionRecord::Reflection { .. } => Some(MemoryTier::Reflection),
            _ => None, // conversation, summary, failed_action are not memory tiers
        }
    }
}

// ---------------------------------------------------------------------------
// SessionMemory

/// The three latest memory records per session — one struct, one KV value.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, Default)]
pub struct SessionMemory {
    /// Latest `WorkingMemory` record, if any.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub working: Option<SessionRecord>,
    /// latest Observation record, if any.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub observation: Option<SessionRecord>,
    /// latest Reflection record, if any.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub reflection: Option<SessionRecord>,
}

impl SessionMemory {
    /// Read the tier that matches the given record, if it is a memory variant.
    fn get(&self, tier: MemoryTier) -> Option<&SessionRecord> {
        match tier {
            MemoryTier::Working => self.working.as_ref(),
            MemoryTier::Observation => self.observation.as_ref(),
            MemoryTier::Reflection => self.reflection.as_ref(),
        }
    }

    /// Set a tier and return the updated `SessionMemory`.
    fn set(mut self, tier: MemoryTier, record: SessionRecord) -> Self {
        match tier {
            MemoryTier::Working => self.working = Some(record),
            MemoryTier::Observation => self.observation = Some(record),
            MemoryTier::Reflection => self.reflection = Some(record),
        }
        self
    }

    /// Build the key under which this session's bundle is stored.
    #[must_use]
    pub fn key(session: &SessionId) -> String {
        format!("memory:{session}")
    }
}

// ---------------------------------------------------------------------------
// MemoryStore trait

/// Latest-memory cache: the newest memory `SessionRecord` of each tier, per
/// session. Overwriting (not append-only): `set` replaces the prior record of
/// that tier.
///
/// `MemoryStore` is a dumb fast cache — it does NOT know about `DocumentStore`.
/// Use `MemoryCoordinator` for dual-write + fallback.
#[async_trait::async_trait]
pub trait MemoryStore: Send + Sync {
    /// Latest memory record of a tier (None if never written).
    async fn get_async(
        &self,
        session: &SessionId,
        tier: MemoryTier,
    ) -> StorageResult<Option<SessionRecord>>;

    /// Overwrite the latest record for that session+tier. `record` MUST be a
    /// memory variant (WorkingMemory/Observation/Reflection); non-memory
    /// variants are rejected.
    async fn set_async(&self, session: &SessionId, record: &SessionRecord) -> StorageResult<()>;

    /// Load all tiers at once (resume fast-path) — one get on the KV backend.
    async fn hydrate_async(&self, session: &SessionId) -> StorageResult<SessionMemory>;

    /// Clear a session's cached memory.
    async fn clear_async(&self, session: &SessionId) -> StorageResult<()>;

    /// Synchronous hydrate for the `AgentLoop`'s `TaskIterator` (F19).
    /// Default returns empty — concrete stores override when sync is cheap.
    fn hydrate_sync(&self, session: &SessionId) -> StorageResult<SessionMemory> {
        let _ = session;
        Ok(SessionMemory::default())
    }
}

// ---------------------------------------------------------------------------
// KvMemoryStore — universal impl over any KeyValueStore

/// `MemoryStore` backed by a `KeyValueStore`. Stores the full `SessionMemory`
/// bundle under one key per session (`memory:{session_id}`), so `hydrate` is
/// a single KV get.
pub struct KvMemoryStore<K> {
    kv: K,
}

impl<K> KvMemoryStore<K> {
    /// Create a new KV-backed `MemoryStore`.
    pub fn new(kv: K) -> Self {
        Self { kv }
    }
}

impl<K: Default> Default for KvMemoryStore<K> {
    fn default() -> Self {
        Self { kv: K::default() }
    }
}

#[async_trait::async_trait]
impl<K: KeyValueStore> MemoryStore for KvMemoryStore<K> {
    async fn get_async(
        &self,
        session: &SessionId,
        tier: MemoryTier,
    ) -> StorageResult<Option<SessionRecord>> {
        let key = SessionMemory::key(session);
        let mem: Option<SessionMemory> = self.kv.get(&key)?;
        Ok(mem.and_then(|m| m.get(tier).cloned()))
    }

    async fn set_async(&self, session: &SessionId, record: &SessionRecord) -> StorageResult<()> {
        let tier = MemoryTier::of(record).ok_or_else(|| {
            StorageError::Backend(
                "set_async: record is not a memory variant (Working/Observation/Reflection)".into(),
            )
        })?;
        let key = SessionMemory::key(session);
        let mem: SessionMemory = self.kv.get(&key)?.unwrap_or_default();
        self.kv.set(&key, mem.set(tier, record.clone()))
    }

    async fn hydrate_async(&self, session: &SessionId) -> StorageResult<SessionMemory> {
        let key = SessionMemory::key(session);
        self.kv
            .get(&key)
            .map(std::option::Option::unwrap_or_default)
    }

    async fn clear_async(&self, session: &SessionId) -> StorageResult<()> {
        self.kv.delete(&SessionMemory::key(session))
    }

    fn hydrate_sync(&self, session: &SessionId) -> StorageResult<SessionMemory> {
        let key = SessionMemory::key(session);
        self.kv
            .get(&key)
            .map(std::option::Option::unwrap_or_default)
    }
}

// ---------------------------------------------------------------------------
// Tests

#[cfg(test)]
mod tests {
    use super::*;
    use crate::types::{
        MemoryFact, ObservationEntry, ObservationKind, ReflectionEntry, TextContent,
        UserModelContent,
    };
    use foundation_compact::SystemTime;
    use foundation_db::MemoryStorage;

    fn working_record(fact: &str) -> SessionRecord {
        SessionRecord::WorkingMemory {
            facts: vec![MemoryFact {
                fact: fact.into(),
                asserted_at: SystemTime::UNIX_EPOCH,
                source_message_id: None,
                confidence: 0.9,
            }],
            version: 1,
            timestamp: SystemTime::UNIX_EPOCH,
        }
    }

    fn observation_record(content: &str) -> SessionRecord {
        SessionRecord::Observation {
            observations: vec![ObservationEntry {
                kind: ObservationKind::Assertion,
                content: content.into(),
                timestamp: SystemTime::UNIX_EPOCH,
                source_message_id: None,
                scope: None,
            }],
            token_count: 5,
            timestamp: SystemTime::UNIX_EPOCH,
        }
    }

    fn reflection_record(summary: &str) -> SessionRecord {
        SessionRecord::Reflection {
            reflections: vec![ReflectionEntry {
                summary: summary.into(),
                time_range: None,
                observation_refs: vec![],
                importance: 0.8,
            }],
            generated_at: SystemTime::UNIX_EPOCH,
            observation_token_count_before: 5,
            reflection_token_count_after: 2,
        }
    }

    fn session_id() -> SessionId {
        SessionId::new()
    }

    #[test]
    fn memory_tier_of_detects_memory_variants() {
        assert_eq!(
            MemoryTier::of(&working_record("x")),
            Some(MemoryTier::Working)
        );
        assert_eq!(
            MemoryTier::of(&observation_record("x")),
            Some(MemoryTier::Observation)
        );
        assert_eq!(
            MemoryTier::of(&reflection_record("x")),
            Some(MemoryTier::Reflection)
        );
        // non-memory variants return None
        let conv = crate::types::Messages::User {
            id: foundation_compact::ids::new_scru128(),
            role: crate::types::MessageRole::User,
            content: UserModelContent::Text(TextContent {
                content: "hi".into(),
                signature: None,
            }),
            signature: None,
        };
        assert!(MemoryTier::of(&SessionRecord::Conversation { message: conv }).is_none());
    }

    #[test]
    fn session_memory_round_trips() {
        let mut mem = SessionMemory::default();
        mem = mem.set(MemoryTier::Working, working_record("user likes rust"));
        mem = mem.set(
            MemoryTier::Observation,
            observation_record("observed preference"),
        );
        assert_eq!(
            mem.working.as_ref().unwrap(),
            &working_record("user likes rust")
        );
        assert!(mem.reflection.is_none());
    }

    #[test]
    fn kv_memory_store_set_get_hydrate_clear() {
        use futures_lite::future::block_on;
        block_on(async {
            let store = KvMemoryStore::new(MemoryStorage::new());
            let sid = session_id();

            // Empty store returns None.
            assert!(store
                .get_async(&sid, MemoryTier::Working)
                .await
                .unwrap()
                .is_none());
            assert!(store.hydrate_async(&sid).await.unwrap().working.is_none());

            // Set working memory.
            store
                .set_async(&sid, &working_record("test fact"))
                .await
                .unwrap();

            // Get single tier.
            let got = store
                .get_async(&sid, MemoryTier::Working)
                .await
                .unwrap()
                .unwrap();
            assert_eq!(got, working_record("test fact"));

            // Non-memory record is rejected.
            let conv = crate::types::Messages::User {
                id: foundation_compact::ids::new_scru128(),
                role: crate::types::MessageRole::User,
                content: UserModelContent::Text(TextContent {
                    content: "hi".into(),
                    signature: None,
                }),
                signature: None,
            };
            let non_mem = SessionRecord::Conversation { message: conv };
            assert!(store.set_async(&sid, &non_mem).await.is_err());

            // Hydrate returns all tiers.
            store
                .set_async(&sid, &observation_record("obs"))
                .await
                .unwrap();
            let mem = store.hydrate_async(&sid).await.unwrap();
            assert!(mem.working.is_some());
            assert!(mem.observation.is_some());
            assert!(mem.reflection.is_none());

            // Overwrite updates the tier.
            store
                .set_async(&sid, &working_record("updated"))
                .await
                .unwrap();
            let got = store
                .get_async(&sid, MemoryTier::Working)
                .await
                .unwrap()
                .unwrap();
            assert_eq!(got, working_record("updated"));

            // Clear removes all.
            store.clear_async(&sid).await.unwrap();
            assert!(store.hydrate_async(&sid).await.unwrap().working.is_none());
        })
    }

    #[test]
    fn kv_memory_store_exercises_async_surface() {
        kv_memory_store_set_get_hydrate_clear();
    }
}
