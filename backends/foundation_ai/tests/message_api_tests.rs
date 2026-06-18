//! `MessageApi` integration tests (F08).
//!
//! Tests the append/flush/pub-sub surface over the in-memory `DocumentStore`.

use foundation_ai::agentic::{MessageApi, MessageEvent};
use foundation_ai::types::{
    MemoryFact, MessageRole, Messages, ObservationEntry, ObservationKind, SessionId,
    SessionRecord, TextContent, UserModelContent,
};
use foundation_compact::ids::new_scru128;
use foundation_compact::SystemTime;
use foundation_db::MemoryDocumentStore;

fn user_record(text: &str) -> SessionRecord {
    SessionRecord::Conversation {
        message: Messages::User {
            id: new_scru128(),
            role: MessageRole::User,
            content: UserModelContent::Text(TextContent {
                content: text.into(),
                signature: None,
            }),
            signature: None,
        },
    }
}

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

#[test]
fn append_returns_id_and_flush_persists() {
    let store = MemoryDocumentStore::new();
    let api = MessageApi::new(SessionId::new(), store);

    let id = api.append(user_record("hello"));
    assert_eq!(id.len(), 25);

    // High threshold means buffer isn't auto-flushed.
    let api2 = MessageApi::with_config(SessionId::new(), MemoryDocumentStore::new(), 50, 256);
    api2.append(user_record("test"));
    api2.flush().unwrap();
    let recent = api2.recent(10).unwrap();
    assert_eq!(recent.len(), 1);
}

#[test]
fn append_triggers_flush_at_threshold() {
    let store = MemoryDocumentStore::new();
    let api = MessageApi::with_config(SessionId::new(), store, 3, 256);

    api.append(user_record("one"));
    api.append(user_record("two"));
    api.append(user_record("three"));

    let recent = api.recent(10).unwrap();
    assert_eq!(recent.len(), 3);
}

#[test]
fn scan_from_returns_from_id() {
    let store = MemoryDocumentStore::new();
    let api = MessageApi::with_config(SessionId::new(), store, 2, 256);

    let id1 = api.append(user_record("first"));
    let id2 = api.append(user_record("second"));
    let _id3 = api.append(user_record("third"));
    api.flush().unwrap();

    let from_second = api.scan_from(&id2, 10).unwrap();
    assert_eq!(from_second.len(), 2); // second + third
}

#[test]
fn pub_sub_delivers_events() {
    let store = MemoryDocumentStore::new();
    let api = MessageApi::new(SessionId::new(), store);

    let rx = api.subscribe();
    api.append(user_record("hello"));

    let events: Vec<_> = rx.try_iter().collect();
    assert_eq!(events.len(), 1);
    assert!(
        matches!(&events[0], MessageEvent::Appended { variant, .. } if *variant == "conversation")
    );
}

#[test]
fn memory_records_use_promotable_columns() {
    let sid = SessionId::new();
    let api = MessageApi::with_config(sid, MemoryDocumentStore::new(), 2, 256);

    api.append(working_record("user likes rust"));
    api.append(observation_record("observed preference"));
    api.flush().unwrap();

    let recent = api.recent(10).unwrap();
    assert_eq!(recent.len(), 2);
    let is_memory = recent.iter().filter(|r| {
        matches!(
            r,
            SessionRecord::WorkingMemory { .. }
                | SessionRecord::Observation { .. }
                | SessionRecord::Reflection { .. }
        )
    });
    assert_eq!(is_memory.count(), 2);
}
