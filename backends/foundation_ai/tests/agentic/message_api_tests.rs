//! `MessageApi` integration tests (F08).
//!
//! Tests the append/flush/pub-sub surface over the in-memory `DocumentStore`.

#![allow(unused_must_use)]

use std::task::{Context, Poll, Waker};
use std::thread;

use foundation_ai::agentic::internals::{MessageApi, MessageApiConfig, MessageEvent};
use foundation_ai::types::{
    MemoryFact, MessageRole, Messages, ObservationEntry, ObservationKind, SessionId, SessionRecord,
    TextContent, UserModelContent,
};
use foundation_compact::ids::new_scru128;
use foundation_compact::SystemTime;
use foundation_core::valtron::valtron_test;
use foundation_db::MemoryDocumentStore;
use futures_lite::future::block_on;
use futures_lite::Future;

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
async fn memory_record_flush_is_idempotent() {
    let api = MessageApi::with_config(SessionId::new(), MemoryDocumentStore::new(), threshold(100));

    // The same record is appended twice (same id). The flush writes it once.
    let rec = working_record("idempotent memory");
    api.append(rec.clone());
    api.append(rec.clone());
    api.flush().await.unwrap();

    let all = api.all().await.unwrap();
    assert_eq!(
        all.len(),
        1,
        "the same memory record is stored once, not twice"
    );
}

#[test]
fn pub_sub_event_id_matches_memory_record_id() {
    let api = MessageApi::with_config(SessionId::new(), MemoryDocumentStore::new(), threshold(100));
    let rx = api.subscribe();

    let rec = working_record("event id matches");
    let rec_id = match &rec {
        SessionRecord::WorkingMemory { id, .. } => id.to_string(),
        _ => unreachable!("working_record returns a WorkingMemory record"),
    };

    // The returned id and the pub/sub event id must both be the record's own
    // id, which `write` also stores as the doc_id (append_promotable_with_id).
    let returned = api.append(rec);

    let events: Vec<_> = rx.try_iter().collect();
    assert_eq!(events.len(), 1);
    let event_id = match &events[0] {
        MessageEvent::Appended { id, .. } => id.clone(),
        _ => unreachable!("expected an Appended event"),
    };

    assert_eq!(returned, rec_id);
    assert_eq!(event_id, rec_id);
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
    let api = MessageApi::with_config(
        SessionId::new(),
        MemoryDocumentStore::new(),
        threshold(1000),
    );

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

// ---------------------------------------------------------------------------
// second_flush_waits_for_running_flush

mod blocking_store {
    use std::sync::atomic::{AtomicBool, Ordering};
    use std::sync::{Arc, Condvar, Mutex};
    use std::time::Duration;

    use async_trait::async_trait;
    use foundation_db::traits::{
        AsyncStorageItemStream, Document, DocumentStore, PromotableDocument,
    };
    use foundation_db::{MemoryDocumentStore, StorageError, StorageResult};
    use serde::de::DeserializeOwned;
    use serde::Serialize;

    /// A `MemoryDocumentStore` whose first `append_with_id` blocks until
    /// released, so a test can hold a running flush open while a second
    /// flush arrives.
    #[derive(Clone)]
    pub struct BlockingStore {
        inner: Arc<MemoryDocumentStore>,
        gate: Arc<(Mutex<bool>, Condvar)>,
        armed: Arc<AtomicBool>,
    }

    impl BlockingStore {
        pub fn new() -> Self {
            Self {
                inner: Arc::new(MemoryDocumentStore::new()),
                gate: Arc::new((Mutex::new(false), Condvar::new())),
                armed: Arc::new(AtomicBool::new(true)),
            }
        }

        /// Whether the first write has been entered and is now blocked.
        pub fn first_write_started(&self) -> bool {
            !self.armed.load(Ordering::SeqCst)
        }

        /// Unblock the first write.
        pub fn release(&self) {
            let (lock, cvar) = &*self.gate;
            *lock.lock().unwrap() = true;
            cvar.notify_all();
        }

        fn hold_first_write(&self) -> StorageResult<()> {
            if !self.armed.swap(false, Ordering::SeqCst) {
                return Ok(());
            }
            let (lock, cvar) = &*self.gate;
            let mut open = lock.lock().unwrap();
            while !*open {
                let (next, timed_out) = cvar
                    .wait_timeout(open, Duration::from_secs(10))
                    .unwrap_or_else(|e| e.into_inner());
                open = next;
                if timed_out.timed_out() {
                    return Err(StorageError::Backend("blocking store timed out".into()));
                }
            }
            Ok(())
        }
    }

    #[async_trait]
    impl DocumentStore for BlockingStore {
        async fn append<V: Serialize + Send + 'static>(
            &self,
            key: &str,
            content: V,
        ) -> StorageResult<Document> {
            self.inner.append(key, content).await
        }
        async fn append_with_id<V: Serialize + Send + 'static>(
            &self,
            key: &str,
            doc_id: &str,
            content: V,
        ) -> StorageResult<Document> {
            self.hold_first_write()?;
            self.inner.append_with_id(key, doc_id, content).await
        }
        async fn append_promotable<V: Serialize + PromotableDocument + Send + 'static>(
            &self,
            key: &str,
            content: V,
        ) -> StorageResult<Document> {
            self.inner.append_promotable(key, content).await
        }
        async fn append_promotable_with_id<V: Serialize + PromotableDocument + Send + 'static>(
            &self,
            key: &str,
            doc_id: &str,
            content: V,
        ) -> StorageResult<Document> {
            self.inner
                .append_promotable_with_id(key, doc_id, content)
                .await
        }
        async fn scan_documents(&self, key: &str, limit: usize) -> StorageResult<Vec<Document>> {
            self.inner.scan_documents(key, limit).await
        }
        async fn scan_documents_from(
            &self,
            key: &str,
            from_id: &str,
            limit: usize,
        ) -> StorageResult<Vec<Document>> {
            self.inner.scan_documents_from(key, from_id, limit).await
        }
        async fn scan<V: DeserializeOwned + Send + 'static>(
            &self,
            key: &str,
            limit: usize,
        ) -> StorageResult<AsyncStorageItemStream<'_, V>> {
            self.inner.scan(key, limit).await
        }
        async fn scan_all<V: DeserializeOwned + Send + 'static>(
            &self,
            key: &str,
        ) -> StorageResult<AsyncStorageItemStream<'_, V>> {
            self.inner.scan_all(key).await
        }
        async fn scan_from<V: DeserializeOwned + Send + 'static>(
            &self,
            key: &str,
            from_id: &str,
            limit: usize,
        ) -> StorageResult<AsyncStorageItemStream<'_, V>> {
            self.inner.scan_from(key, from_id, limit).await
        }
        async fn delete(&self, key: &str, doc_id: &str) -> StorageResult<()> {
            self.inner.delete(key, doc_id).await
        }
        async fn delete_all(&self, key: &str) -> StorageResult<u64> {
            self.inner.delete_all(key).await
        }
        async fn count(&self, key: &str) -> StorageResult<u64> {
            self.inner.count(key).await
        }
    }
}

use blocking_store::BlockingStore;

/// A flush that finds another flush already running must await its completion,
/// not return `Ok(0)` at once (which would make the agent loop re-enter
/// `Flushing` in a tight loop while the store is slow).
#[test]
fn second_flush_waits_for_running_flush() {
    let store = BlockingStore::new();
    let api = MessageApi::with_config(SessionId::new(), store.clone(), threshold(100));

    api.append(user_record("one"));

    // The first flush runs on its own thread and blocks on the store's first
    // write.
    let api1 = api.clone();
    let flush1 = thread::spawn(move || block_on(api1.flush()).unwrap());

    // Wait until the first flush has entered the blocked write.
    while !store.first_write_started() {
        thread::yield_now();
    }

    // The second flush must park while the first holds the store open. Poll it
    // many times with a noop waker: a wrong `Ok(0)` fast-path would resolve.
    let waker = Waker::noop();
    let mut cx = Context::from_waker(&waker);
    let mut second = Box::pin(api.flush());
    for _ in 0..2_000 {
        assert!(
            second.as_mut().poll(&mut cx).is_pending(),
            "second flush returned while the first flush was still running"
        );
    }

    store.release();
    assert_eq!(
        flush1.join().unwrap(),
        1,
        "the first flush wrote the record"
    );

    // Now the running flush has finished and drained the buffer, so the
    // waiting caller reports nothing left to write.
    match second.as_mut().poll(&mut cx) {
        Poll::Ready(Ok(0)) => {}
        Poll::Ready(Ok(n)) => panic!("waiter should report 0, got {n}"),
        Poll::Ready(Err(_)) => panic!("waiter should not fail"),
        Poll::Pending => panic!("second flush should complete once the first finishes"),
    }

    let all = block_on(api.all()).unwrap();
    assert_eq!(all.len(), 1, "the record is persisted exactly once");
}
