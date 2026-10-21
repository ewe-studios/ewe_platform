//! `Blocking<S>`: each sync storage trait round-trips through an async store.
//!
//! This binary never initializes a valtron pool, on purpose: the in-memory
//! stores' futures are ready on the first poll, so `Blocking` must answer
//! without an executor. A store that really waits must then fail with a
//! `StorageError` (not panic). The executor-driven path is covered by
//! `blocking_executor_tests.rs`.

use foundation_compact::ids::new_scru128_string;
use foundation_core::valtron::Stream;
use foundation_db::{
    AsyncKeyValueStore, AsyncListStream, BlobStore, Blocking, DocumentStore, KeyValueStore,
    MemoryDocumentStore, MemoryStorage, PromotableDocument, QueryStore, RateLimiterStore, StorageError,
    StorageItemStream, StorageResult,
};
use serde::{de::DeserializeOwned, Serialize};

fn collect<T>(stream: StorageItemStream<'_, T>) -> Vec<T> {
    stream
        .map(|item| match item {
            Stream::Next(Ok(v)) => v,
            Stream::Next(Err(e)) => panic!("stream error: {e}"),
            _ => panic!("expected only `Stream::Next` items from an in-memory store"),
        })
        .collect()
}

#[test]
fn key_value_store_round_trips() {
    let store = Blocking::new(MemoryStorage::new());

    store.set("a:1", "one").unwrap();
    store.set("a:2", 2_u32).unwrap();
    store.set("b:1", true).unwrap();

    let one: Option<String> = store.get("a:1").unwrap();
    assert_eq!(one.as_deref(), Some("one"));
    let two: Option<u32> = store.get("a:2").unwrap();
    assert_eq!(two, Some(2));
    assert!(store.exists("b:1").unwrap());
    assert!(!store.exists("missing").unwrap());

    let mut keys = collect(store.list_keys(Some("a:")).unwrap());
    keys.sort();
    assert_eq!(keys, vec!["a:1".to_string(), "a:2".to_string()]);
    assert_eq!(collect(store.list_keys(None).unwrap()).len(), 3);

    store.delete("a:1").unwrap();
    let gone: Option<String> = store.get("a:1").unwrap();
    assert_eq!(gone, None);
}

#[test]
fn blob_store_round_trips() {
    let store = Blocking::new(MemoryStorage::new());

    store.put_blob("blob", &[1, 2, 3]).unwrap();
    assert!(store.blob_exists("blob").unwrap());
    assert_eq!(store.get_blob("blob").unwrap(), Some(vec![1, 2, 3]));

    store.delete_blob("blob").unwrap();
    assert!(!store.blob_exists("blob").unwrap());
    assert_eq!(store.get_blob("blob").unwrap(), None);
}

#[test]
fn rate_limiter_store_round_trips() {
    let store = Blocking::new(MemoryStorage::new());

    assert!(store.check_rate_limit("ip", 2, 60).unwrap());
    assert_eq!(store.record_rate_limit("ip").unwrap(), 1);
    assert_eq!(store.record_rate_limit("ip").unwrap(), 2);
    assert!(!store.check_rate_limit("ip", 2, 60).unwrap(), "limit reached");

    store.reset_rate_limit("ip").unwrap();
    assert!(store.check_rate_limit("ip", 2, 60).unwrap(), "reset clears the count");
}

/// `MemoryStorage` has no SQL engine; its async `QueryStore` reports that and
/// `Blocking` passes the error through unchanged.
#[test]
fn query_store_passes_errors_through() {
    let store = Blocking::new(MemoryStorage::new());

    assert!(matches!(store.query("SELECT 1", &[]), Err(StorageError::Generic(_))));
    assert!(matches!(store.execute("DELETE FROM t", &[]), Err(StorageError::Generic(_))));
    assert!(matches!(store.execute_batch("SELECT 1"), Err(StorageError::Generic(_))));
}

#[derive(serde::Serialize, serde::Deserialize, Debug, PartialEq)]
struct Note {
    n: u32,
}

impl PromotableDocument for Note {
    fn record_type(&self) -> Option<String> {
        Some("note".into())
    }
    fn summary(&self) -> Option<String> {
        Some(format!("note {}", self.n))
    }
}

#[test]
fn document_store_round_trips() {
    let store = Blocking::new(MemoryDocumentStore::new());

    let first = store.append("k", Note { n: 0 }).unwrap();
    let second = store.append_promotable("k", Note { n: 1 }).unwrap();
    assert_eq!(second.record_type.as_deref(), Some("note"));
    let third_id = new_scru128_string();
    let third = store.append_with_id("k", &third_id, Note { n: 2 }).unwrap();
    assert_eq!(third.id, third_id);
    let fourth_id = new_scru128_string();
    let fourth = store
        .append_promotable_with_id("k", &fourth_id, Note { n: 3 })
        .unwrap();
    assert_eq!(fourth.summary.as_deref(), Some("note 3"));

    assert_eq!(store.count("k").unwrap(), 4);

    let newest_two: Vec<Note> = collect(store.scan("k", 2).unwrap());
    assert_eq!(newest_two, vec![Note { n: 3 }, Note { n: 2 }]);

    let all: Vec<Note> = collect(store.scan_all("k").unwrap());
    assert_eq!(all.iter().map(|n| n.n).collect::<Vec<_>>(), vec![0, 1, 2, 3]);

    let from_second: Vec<Note> = collect(store.scan_from("k", &second.id, 0).unwrap());
    assert_eq!(from_second.iter().map(|n| n.n).collect::<Vec<_>>(), vec![1, 2, 3]);

    let docs = store.scan_documents("k", 10).unwrap();
    assert_eq!(docs.len(), 4);
    assert_eq!(docs[0].id, fourth.id, "newest first");
    let docs_from = store.scan_documents_from("k", &third.id, 0).unwrap();
    assert_eq!(docs_from.len(), 2);

    store.delete("k", &first.id).unwrap();
    assert_eq!(store.count("k").unwrap(), 3);
    assert_eq!(store.delete_all("k").unwrap(), 3);
    assert_eq!(store.count("k").unwrap(), 0);
}

#[test]
fn blocking_is_cheap_to_clone_and_shares_the_store() {
    let store = Blocking::new(MemoryStorage::new());
    let clone = store.clone();
    store.set("shared", 7_u8).unwrap();
    let seen: Option<u8> = clone.get("shared").unwrap();
    assert_eq!(seen, Some(7));

    // `Deref` keeps the inner store's async API reachable.
    let direct: Option<u8> = futures_lite::future::block_on(clone.get_async("shared")).unwrap();
    assert_eq!(direct, Some(7));
}

/// A key-value store whose futures really wait once before finishing (like a
/// network store), so `Blocking` has to hand them to the executor.
struct Waits(MemoryStorage);

#[async_trait::async_trait]
impl AsyncKeyValueStore for Waits {
    async fn get_async<V: DeserializeOwned + Send + 'static>(
        &self,
        key: &str,
    ) -> StorageResult<Option<V>> {
        futures_lite::future::yield_now().await;
        self.0.get_async(key).await
    }
    async fn set_async<V: Serialize + Send + 'static>(&self, key: &str, value: V) -> StorageResult<()> {
        futures_lite::future::yield_now().await;
        self.0.set_async(key, value).await
    }
    async fn delete_async(&self, key: &str) -> StorageResult<()> {
        futures_lite::future::yield_now().await;
        self.0.delete_async(key).await
    }
    async fn exists_async(&self, key: &str) -> StorageResult<bool> {
        futures_lite::future::yield_now().await;
        self.0.exists_async(key).await
    }
    async fn list_keys_async(&self, prefix: Option<&str>) -> StorageResult<AsyncListStream> {
        futures_lite::future::yield_now().await;
        self.0.list_keys_async(prefix).await
    }
}

/// Without a valtron pool, a future that has to wait can't be driven: the sync
/// call fails with a `StorageError` instead of panicking or hanging.
#[test]
fn waiting_without_an_executor_is_an_error_not_a_panic() {
    let store = Blocking::new(Waits(MemoryStorage::new()));
    let err = store.set("k", 1_u8).unwrap_err();
    assert!(
        matches!(&err, StorageError::Backend(msg) if msg.contains("valtron executor")),
        "got {err:?}"
    );
}
