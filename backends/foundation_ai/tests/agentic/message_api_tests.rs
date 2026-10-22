//! `MessageApi` integration tests (F08).
//!
//! Tests the append/flush/pub-sub surface over the in-memory `DocumentStore`.

#![allow(unused_must_use)]

use std::thread;

use foundation_core::valtron::valtron_test;
use foundation_ai::agentic::internals::{MessageApi, MessageApiConfig, MessageEvent};
use foundation_ai::types::{
    MemoryFact, MessageRole, Messages, ObservationEntry, ObservationKind, SessionId, SessionRecord,
    TextContent, UserModelContent,
};
use foundation_compact::ids::new_scru128;
use foundation_compact::SystemTime;
use foundation_db::MemoryDocumentStore;

fn threshold(flush_threshold: usize) -> MessageApiConfig {
    MessageApiConfig::default().with_flush_threshold(flush_threshold)
}

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

#[valtron_test]
async fn append_returns_id_and_flush_persists() {
    let store = MemoryDocumentStore::new();
    let api = MessageApi::new(SessionId::new(), store);

    let id = api.append(user_record("hello"));
    assert_eq!(id.len(), 25);

    // High threshold means buffer isn't auto-flushed.
    let api2 = MessageApi::with_config(SessionId::new(), MemoryDocumentStore::new(), threshold(50));
    api2.append(user_record("test"));
    api2.flush().await.unwrap();
    let recent = api2.recent(10).await.unwrap();
    assert_eq!(recent.len(), 1);
}

#[valtron_test]
async fn needs_flush_reports_the_threshold_and_clears_after_a_flush() {
    let store = MemoryDocumentStore::new();
    let api = MessageApi::with_config(SessionId::new(), store, threshold(3));
    assert_eq!(api.flush_threshold(), 3);

    // `append` only buffers; the owner of the log awaits the flush.
    api.append(user_record("one"));
    api.append(user_record("two"));
    assert!(!api.needs_flush(), "below the threshold no flush is due");
    api.append(user_record("three"));
    assert!(api.needs_flush(), "at the threshold a flush is due");
    assert_eq!(api.unflushed(), 3, "append must not write the store itself");

    assert_eq!(api.flush().await.unwrap(), 3);
    assert!(!api.needs_flush());
    assert_eq!(api.unflushed(), 0);
    assert_eq!(api.recent(10).await.unwrap().len(), 3);
}

#[test]
fn message_api_config_defaults() {
    let config = MessageApiConfig::default();
    assert_eq!(config.flush_threshold, 50);
    assert_eq!(config.subscriber_capacity, 256);
    let tuned = config.with_flush_threshold(7).with_subscriber_capacity(9);
    assert_eq!(tuned.flush_threshold, 7);
    assert_eq!(tuned.subscriber_capacity, 9);
}

#[valtron_test]
async fn scan_from_returns_from_id() {
    let store = MemoryDocumentStore::new();
    let api = MessageApi::with_config(SessionId::new(), store, threshold(2));

    let _id1 = api.append(user_record("first"));
    let id2 = api.append(user_record("second"));
    let _id3 = api.append(user_record("third"));
    api.flush().await.unwrap();

    let from_second = api.scan_from(&id2, 10).await.unwrap();
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

#[valtron_test]
async fn memory_records_use_promotable_columns() {
    let sid = SessionId::new();
    let api = MessageApi::with_config(sid, MemoryDocumentStore::new(), threshold(2));

    api.append(working_record("user likes rust"));
    api.append(observation_record("observed preference"));
    api.flush().await.unwrap();

    let recent = api.recent(10).await.unwrap();
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

#[valtron_test]
async fn all_returns_oldest_first() {
    let api = MessageApi::with_config(SessionId::new(), MemoryDocumentStore::new(), threshold(100));

    let _ = api.append(user_record("first"));
    let _ = api.append(user_record("second"));
    let _ = api.append(user_record("third"));
    api.flush().await.unwrap();

    let records = api.all().await.unwrap();
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

#[valtron_test]
async fn flush_on_end_loses_nothing() {
    let api = MessageApi::with_config(SessionId::new(), MemoryDocumentStore::new(), threshold(1000));

    for i in 0..20 {
        api.append(user_record(&format!("msg-{i}")));
    }
    // Buffer is not auto-flushed (threshold=1000).
    // Explicit flush simulates session-end flush.
    let count = api.flush().await.unwrap();
    assert_eq!(count, 20);

    let all = api.all().await.unwrap();
    assert_eq!(all.len(), 20);
}

#[valtron_test]
async fn concurrent_append_and_read() {
    let api = MessageApi::with_config(SessionId::new(), MemoryDocumentStore::new(), threshold(5));

    let writers: Vec<_> = (0..4)
        .map(|t| {
            let api = api.clone();
            thread::spawn(move || {
                for i in 0..10 {
                    api.append(user_record(&format!("t{t}-{i}")));
                }
            })
        })
        .collect();

    for w in writers {
        w.join().unwrap();
    }

    api.flush().await.unwrap();
    let all = api.all().await.unwrap();
    assert_eq!(all.len(), 40);
}

#[valtron_test]
async fn pub_sub_flush_event_includes_count() {
    let api = MessageApi::with_config(SessionId::new(), MemoryDocumentStore::new(), threshold(100));
    let rx = api.subscribe();

    api.append(user_record("a"));
    api.append(user_record("b"));
    api.flush().await.unwrap();

    let events: Vec<_> = rx.try_iter().collect();
    // 2 Appended + 1 Flushed
    assert_eq!(events.len(), 3);
    assert!(matches!(&events[2], MessageEvent::Flushed { count: 2 }));
}

#[valtron_test]
async fn clear_removes_all_records() {
    let api = MessageApi::with_config(SessionId::new(), MemoryDocumentStore::new(), threshold(100));
    api.append(user_record("will be gone"));
    api.flush().await.unwrap();
    assert_eq!(api.recent(10).await.unwrap().len(), 1);

    api.clear().await.unwrap();
    assert_eq!(api.recent(10).await.unwrap().len(), 0);
}

#[valtron_test]
async fn mixed_record_types_roundtrip() {
    let api = MessageApi::with_config(SessionId::new(), MemoryDocumentStore::new(), threshold(100));

    api.append(user_record("hello"));
    api.append(working_record("user preference"));
    api.append(observation_record("noted pattern"));
    api.flush().await.unwrap();

    let all = api.all().await.unwrap();
    assert_eq!(all.len(), 3);
    assert!(matches!(&all[0], SessionRecord::Conversation { .. }));
    assert!(matches!(&all[1], SessionRecord::WorkingMemory { .. }));
    assert!(matches!(&all[2], SessionRecord::Observation { .. }));
}
