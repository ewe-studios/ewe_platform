use foundation_ai::agentic::memory_store::*;
use foundation_ai::types::{
    MemoryFact, MessageRole, Messages, ObservationEntry, ObservationKind, ReflectionEntry,
    SessionId, SessionRecord, TextContent, UserModelContent,
};
use foundation_compact::SystemTime;
use foundation_db::MemoryStorage;

fn working_record(fact: &str) -> SessionRecord {
    SessionRecord::WorkingMemory {
        id: foundation_compact::ids::new_scru128(),
        facts: vec![MemoryFact {
            fact: fact.into(),
            asserted_at: SystemTime::UNIX_EPOCH,
            source_message_id: foundation_compact::ids::new_scru128(),
            confidence: 0.9,
        }],
        version: 1,
        timestamp: SystemTime::UNIX_EPOCH,
    }
}

fn observation_record(content: &str) -> SessionRecord {
    SessionRecord::Observation {
        id: foundation_compact::ids::new_scru128(),
        observations: vec![ObservationEntry {
            kind: ObservationKind::Assertion,
            content: content.into(),
            timestamp: SystemTime::UNIX_EPOCH,
            source_message_id: foundation_compact::ids::new_scru128(),
            scope: None,
        }],
        token_count: 5,
        timestamp: SystemTime::UNIX_EPOCH,
    }
}

fn reflection_record(summary: &str) -> SessionRecord {
    SessionRecord::Reflection {
        id: foundation_compact::ids::new_scru128(),
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
    let conv = Messages::User {
        id: foundation_compact::ids::new_scru128(),
        role: MessageRole::User,
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
    let working = working_record("user likes rust");
    let obs = observation_record("observed preference");
    let mut mem = SessionMemory::default();
    mem = mem.set(MemoryTier::Working, working.clone());
    mem = mem.set(MemoryTier::Observation, obs);
    assert_eq!(mem.working.as_ref().unwrap(), &working);
    assert!(mem.reflection.is_none());
}

#[test]
fn kv_memory_store_set_get_hydrate_clear() {
    use futures_lite::future::block_on;
    block_on(async {
        let store = KvMemoryStore::new(MemoryStorage::new());
        let sid = session_id();

        assert!(store
            .get_async(&sid, MemoryTier::Working)
            .await
            .unwrap()
            .is_none());
        assert!(store.hydrate_async(&sid).await.unwrap().working.is_none());

        let rec = working_record("test fact");
        store.set_async(&sid, &rec).await.unwrap();

        let got = store
            .get_async(&sid, MemoryTier::Working)
            .await
            .unwrap()
            .unwrap();
        assert_eq!(got, rec);

        let conv = Messages::User {
            id: foundation_compact::ids::new_scru128(),
            role: MessageRole::User,
            content: UserModelContent::Text(TextContent {
                content: "hi".into(),
                signature: None,
            }),
            signature: None,
        };
        let non_mem = SessionRecord::Conversation { message: conv };
        assert!(store.set_async(&sid, &non_mem).await.is_err());

        store
            .set_async(&sid, &observation_record("obs"))
            .await
            .unwrap();
        let mem = store.hydrate_async(&sid).await.unwrap();
        assert!(mem.working.is_some());
        assert!(mem.observation.is_some());
        assert!(mem.reflection.is_none());

        let updated = working_record("updated");
        store.set_async(&sid, &updated).await.unwrap();
        let got = store
            .get_async(&sid, MemoryTier::Working)
            .await
            .unwrap()
            .unwrap();
        assert_eq!(got, updated);

        store.clear_async(&sid).await.unwrap();
        assert!(store.hydrate_async(&sid).await.unwrap().working.is_none());
    })
}

#[test]
fn kv_memory_store_exercises_async_surface() {
    kv_memory_store_set_get_hydrate_clear();
}
