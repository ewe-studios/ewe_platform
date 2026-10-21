//! `Blocking<S>` over stores that really wait: their futures and streams go
//! `Pending`, so `Blocking` hands them to the valtron executor (initialized
//! here) and waits. Covers single values, lazily pulled streams, the real
//! Turso backend, and `StorageCleanup` (a blanket over the sync `QueryStore`).

use std::pin::Pin;
use std::sync::Mutex;
use std::task::{Context, Poll};
use std::vec::IntoIter;

use foundation_core::valtron::{initialize_pool, BlockingFutureExt, PoolGuard, Stream};
use foundation_db::{
    AsyncKeyValueStore, AsyncListStream, Blocking, DataValue, KeyValueStore, MemoryStorage,
    QueryStore, StorageCleanup, StorageError, StorageItemStream, StorageResult, TursoStorage,
};
use serde::{de::DeserializeOwned, Serialize};

static POOL_GUARD: Mutex<Option<PoolGuard>> = Mutex::new(None);

fn init_valtron() {
    let mut guard = POOL_GUARD.lock().unwrap();
    if guard.is_none() {
        *guard = Some(initialize_pool(42, None));
    }
}

fn collect<T>(stream: StorageItemStream<'_, T>) -> Vec<T> {
    stream
        .filter_map(|item| match item {
            Stream::Next(Ok(v)) => Some(v),
            Stream::Next(Err(e)) => panic!("stream error: {e}"),
            _ => None,
        })
        .collect()
}

/// A stream that is `Pending` once before every item (waking itself), like a
/// network cursor.
struct SlowKeys {
    keys: IntoIter<String>,
    ready: bool,
}

impl futures_core::Stream for SlowKeys {
    type Item = StorageResult<String>;

    fn poll_next(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Option<Self::Item>> {
        if self.ready {
            self.ready = false;
            Poll::Ready(self.keys.next().map(Ok))
        } else {
            self.ready = true;
            cx.waker().wake_by_ref();
            Poll::Pending
        }
    }
}

/// A key-value store whose calls wait once and whose key listing is slow.
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
        let keys = self.0.list_keys_async(prefix).await?.collect_all().await?;
        Ok(AsyncListStream::new(SlowKeys {
            keys: keys.into_iter(),
            ready: false,
        }))
    }
}

#[test]
fn waiting_futures_and_streams_go_through_the_executor() {
    init_valtron();
    let store = Blocking::new(Waits(MemoryStorage::new()));

    store.set("p:1", "a").unwrap();
    store.set("p:2", "b").unwrap();
    store.set("q:1", "c").unwrap();
    let a: Option<String> = store.get("p:1").unwrap();
    assert_eq!(a.as_deref(), Some("a"));
    assert!(store.exists("q:1").unwrap());

    let mut keys = collect(store.list_keys(Some("p:")).unwrap());
    keys.sort();
    assert_eq!(keys, vec!["p:1".to_string(), "p:2".to_string()]);

    store.delete("q:1").unwrap();
    assert!(!store.exists("q:1").unwrap());
}

fn turso() -> Blocking<TursoStorage> {
    let storage = async {
        let storage = TursoStorage::new(":memory:").await?;
        storage.init_schema().await?;
        Ok::<_, StorageError>(storage)
    }
    .block()
    .unwrap();
    Blocking::new(storage)
}

#[test]
fn turso_round_trips_through_the_sync_traits() {
    init_valtron();
    let store = turso();

    store.set("greeting", "hello").unwrap();
    let value: Option<String> = store.get("greeting").unwrap();
    assert_eq!(value.as_deref(), Some("hello"));

    store
        .execute_batch("CREATE TABLE t (id INTEGER PRIMARY KEY, name TEXT)")
        .unwrap();
    for (id, name) in [(1, "one"), (2, "two"), (3, "three")] {
        let changed = store
            .execute(
                "INSERT INTO t (id, name) VALUES (?, ?)",
                &[DataValue::Integer(id), DataValue::Text(name.into())],
            )
            .unwrap();
        assert_eq!(changed, 1);
    }
    let rows = collect(store.query("SELECT name FROM t ORDER BY id", &[]).unwrap());
    let names: Vec<String> = rows.iter().map(|r| r.get::<String>(0).unwrap()).collect();
    assert_eq!(names, vec!["one", "two", "three"]);

    // Turso's row stream is lazy: a bad statement surfaces as the stream's
    // first item, an `Err`.
    let mut bad = store.query("SELECT * FROM no_such_table", &[]).unwrap();
    assert!(matches!(bad.next(), Some(Stream::Next(Err(_)))));
}

#[test]
fn storage_cleanup_runs_over_blocking_turso() {
    init_valtron();
    let store = turso();

    // An expired and a live JWT row; cleanup removes only the expired one.
    store
        .execute(
            "INSERT INTO jwt_tokens (id, access_token_encrypted, expires_at) VALUES (?, ?, ?)",
            &[
                DataValue::Text("old".into()),
                DataValue::Text("h1".into()),
                DataValue::Integer(10),
            ],
        )
        .unwrap();
    store
        .execute(
            "INSERT INTO jwt_tokens (id, access_token_encrypted, expires_at) VALUES (?, ?, ?)",
            &[
                DataValue::Text("new".into()),
                DataValue::Text("h2".into()),
                DataValue::Integer(i64::MAX),
            ],
        )
        .unwrap();

    assert_eq!(store.cleanup_expired_jwt_tokens(100).unwrap(), 1);
    let left = collect(store.query("SELECT id FROM jwt_tokens", &[]).unwrap());
    assert_eq!(left.len(), 1);
    assert_eq!(left[0].get::<String>(0).unwrap(), "new");
}
