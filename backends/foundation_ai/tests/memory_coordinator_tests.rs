//! `MemoryCoordinator` integration tests (F07): dual-write to the audit
//! `DocumentStore` + the latest-memory cache, and the cache-miss fallback that
//! rebuilds from the audit log.

use foundation_ai::agentic::{KvMemoryStore, MemoryCoordinator, MemoryStore, MemoryTier};
use foundation_ai::types::{
    ObservationEntry, ObservationKind, ReflectionEntry, SessionId, SessionRecord,
};
use foundation_compact::SystemTime;
use foundation_db::{MemoryDocumentStore, MemoryStorage};
use futures_lite::future::block_on;

fn observation(content: &str) -> SessionRecord {
    SessionRecord::Observation {
        observations: vec![ObservationEntry {
            kind: ObservationKind::Assertion,
            content: content.into(),
            timestamp: SystemTime::UNIX_EPOCH,
            source_message_id: None,
            scope: None,
        }],
        token_count: 4,
        timestamp: SystemTime::UNIX_EPOCH,
    }
}

fn reflection(summary: &str) -> SessionRecord {
    SessionRecord::Reflection {
        reflections: vec![ReflectionEntry {
            summary: summary.into(),
            time_range: None,
            observation_refs: vec![],
            importance: 0.5,
        }],
        generated_at: SystemTime::UNIX_EPOCH,
        observation_token_count_before: 10,
        reflection_token_count_after: 4,
    }
}

fn new_coordinator() -> MemoryCoordinator<KvMemoryStore<MemoryStorage>, MemoryDocumentStore> {
    MemoryCoordinator::new(
        KvMemoryStore::new(MemoryStorage::new()),
        MemoryDocumentStore::new(),
    )
}

#[test]
fn dual_write_then_hydrate_returns_each_tier() {
    block_on(async {
        let coord = new_coordinator();
        let session = SessionId::new();

        let obs = observation("user prefers rust");
        let refl = reflection("the user is a systems programmer");
        coord.record_async(&session, &obs).await.unwrap();
        coord.record_async(&session, &refl).await.unwrap();

        let memory = coord.hydrate_async(&session).await.unwrap();
        assert_eq!(memory.observation.as_ref(), Some(&obs));
        assert_eq!(memory.reflection.as_ref(), Some(&refl));
        assert_eq!(memory.working, None);
    });
}

#[test]
fn set_overwrites_with_latest_per_tier() {
    block_on(async {
        let coord = new_coordinator();
        let session = SessionId::new();
        coord.record_async(&session, &observation("first")).await.unwrap();
        let latest = observation("second");
        coord.record_async(&session, &latest).await.unwrap();

        let memory = coord.hydrate_async(&session).await.unwrap();
        assert_eq!(memory.observation.as_ref(), Some(&latest));
    });
}

#[test]
fn hydrate_falls_back_to_audit_and_repopulates_cache() {
    block_on(async {
        let coord = new_coordinator();
        let session = SessionId::new();
        let obs = observation("durable observation");
        coord.record_async(&session, &obs).await.unwrap();

        // Simulate a cache loss (crash before the cache write / eviction).
        coord.memory().clear_async(&session).await.unwrap();
        assert_eq!(
            coord.memory().get_async(&session, MemoryTier::Observation).await.unwrap(),
            None,
            "cache is empty after clear"
        );

        // Hydrate must rebuild from the audit DocumentStore...
        let memory = coord.hydrate_async(&session).await.unwrap();
        assert_eq!(memory.observation.as_ref(), Some(&obs));

        // ...and re-populate the cache so the next resume is O(1).
        assert_eq!(
            coord.memory().get_async(&session, MemoryTier::Observation).await.unwrap(),
            Some(obs),
            "fallback re-populated the cache"
        );
    });
}

#[test]
fn sessions_are_isolated() {
    block_on(async {
        let coord = new_coordinator();
        let a = SessionId::new();
        let b = SessionId::new();
        coord.record_async(&a, &observation("for a")).await.unwrap();

        assert!(coord.hydrate_async(&b).await.unwrap().observation.is_none());
        assert!(coord.hydrate_async(&a).await.unwrap().observation.is_some());
    });
}
