//! `MessageApi` integration tests (F08).
//!
//! Tests the append/flush/pub-sub surface over the in-memory `DocumentStore`.

#![allow(unused_must_use)]

use foundation_ai::agentic::{MessageApi, MessageEvent};
use foundation_ai::types::{
    MemoryFact, MessageRole, Messages, ObservationEntry, ObservationKind, SessionId, SessionRecord,
    TextContent, UserModelContent,
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

    let _id1 = api.append(user_record("first"));
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

#[test]
fn all_returns_oldest_first() {
    let api = MessageApi::with_config(SessionId::new(), MemoryDocumentStore::new(), 100, 256);

    let _ = api.append(user_record("first"));
    let _ = api.append(user_record("second"));
    let _ = api.append(user_record("third"));
    api.flush().unwrap();

    let records = api.all().unwrap();
    assert_eq!(records.len(), 3);

    let texts: Vec<&str> = records
        .iter()
        .filter_map(|r| match r {
            SessionRecord::Conversation {
                message: Messages::User { content, .. },
            } => match content {
                UserModelContent::Text(tc) => Some(tc.content.as_str()),
                _ => None,
            },
            _ => None,
        })
        .collect();
    assert_eq!(texts, vec!["first", "second", "third"]);
}

#[test]
fn flush_on_end_loses_nothing() {
    let api = MessageApi::with_config(SessionId::new(), MemoryDocumentStore::new(), 1000, 256);

    for i in 0..20 {
        api.append(user_record(&format!("msg-{i}")));
    }
    // Buffer is not auto-flushed (threshold=1000).
    // Explicit flush simulates session-end flush.
    let count = api.flush().unwrap();
    assert_eq!(count, 20);

    let all = api.all().unwrap();
    assert_eq!(all.len(), 20);
}

#[test]
fn concurrent_append_and_read() {
    let api = MessageApi::with_config(SessionId::new(), MemoryDocumentStore::new(), 5, 256);

    let writers: Vec<_> = (0..4)
        .map(|t| {
            let api = api.clone();
            std::thread::spawn(move || {
                for i in 0..10 {
                    api.append(user_record(&format!("t{t}-{i}")));
                }
            })
        })
        .collect();

    for w in writers {
        w.join().unwrap();
    }

    api.flush().unwrap();
    let all = api.all().unwrap();
    assert_eq!(all.len(), 40);
}

#[test]
fn pub_sub_flush_event_includes_count() {
    let api = MessageApi::with_config(SessionId::new(), MemoryDocumentStore::new(), 100, 256);
    let rx = api.subscribe();

    api.append(user_record("a"));
    api.append(user_record("b"));
    api.flush().unwrap();

    let events: Vec<_> = rx.try_iter().collect();
    // 2 Appended + 1 Flushed
    assert_eq!(events.len(), 3);
    assert!(matches!(&events[2], MessageEvent::Flushed { count: 2 }));
}

#[test]
fn clear_removes_all_records() {
    let api = MessageApi::with_config(SessionId::new(), MemoryDocumentStore::new(), 100, 256);
    api.append(user_record("will be gone"));
    api.flush().unwrap();
    assert_eq!(api.recent(10).unwrap().len(), 1);

    api.clear().unwrap();
    assert_eq!(api.recent(10).unwrap().len(), 0);
}

#[test]
fn mixed_record_types_roundtrip() {
    let api = MessageApi::with_config(SessionId::new(), MemoryDocumentStore::new(), 100, 256);

    api.append(user_record("hello"));
    api.append(working_record("user preference"));
    api.append(observation_record("noted pattern"));
    api.flush().unwrap();

    let all = api.all().unwrap();
    assert_eq!(all.len(), 3);
    assert!(matches!(&all[0], SessionRecord::Conversation { .. }));
    assert!(matches!(&all[1], SessionRecord::WorkingMemory { .. }));
    assert!(matches!(&all[2], SessionRecord::Observation { .. }));
}
