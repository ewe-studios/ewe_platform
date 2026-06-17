//! Async SQL `AsyncDocumentStore` conformance (F22b), against the real Turso
//! (`SQLite`) backend — proves `AsyncSqlDocumentStore<TursoStorage>` matches the
//! in-memory + sync-SQL semantics (scru128 ordering, inclusive `scan_from`,
//! promoted columns) and that scans are pulled lazily via `.next().await`.

use foundation_db::traits::AsyncDocumentStore;
use foundation_db::{AsyncSqlDocumentStore, TursoStorage};
use futures_lite::StreamExt;
use std::sync::Mutex;
use tempfile::TempDir;

/// Shared Valtron pool guard — `TursoStorage::new`/`init_schema` drive async work
/// through valtron, so the pool must exist before they're called.
static POOL_GUARD: Mutex<Option<foundation_core::valtron::PoolGuard>> = Mutex::new(None);

fn init_valtron() {
    let mut guard = POOL_GUARD.lock().unwrap();
    if guard.is_none() {
        *guard = Some(foundation_core::valtron::initialize_pool(42, None));
    }
}

/// Migrated async store over a fresh temp `SQLite` db (migrations 020/021 applied
/// via the real `init_schema`).
fn make_store() -> (TempDir, AsyncSqlDocumentStore<TursoStorage>) {
    init_valtron();
    let dir = TempDir::new().unwrap();
    let url = dir.path().join("docs.db");
    let storage = TursoStorage::new(url.to_str().unwrap()).unwrap();
    storage.init_schema().unwrap();
    (dir, AsyncSqlDocumentStore::new(storage))
}

#[derive(serde::Serialize)]
struct Note {
    kind: String,
    text: String,
}
impl foundation_db::traits::PromotableDocument for Note {
    fn record_type(&self) -> Option<String> {
        Some(self.kind.clone())
    }
    fn summary(&self) -> Option<String> {
        Some(self.text.clone())
    }
}

#[test]
fn append_async_orders_by_scru128_and_counts() {
    let (_dir, store) = make_store();
    futures_lite::future::block_on(async {
        let a = store.append_async("k", serde_json::json!({"n": 0})).await.unwrap();
        let b = store.append_async("k", serde_json::json!({"n": 1})).await.unwrap();
        assert_eq!(a.id.len(), 25);
        assert!(b.id > a.id, "later async append → greater scru128 id");
        assert_eq!(store.count_async("k").await.unwrap(), 2);
    });
}

#[test]
fn scan_from_async_streams_inclusive_oldest_first() {
    let (_dir, store) = make_store();
    futures_lite::future::block_on(async {
        let mut ids = Vec::new();
        for n in 0..5 {
            ids.push(store.append_async("k", serde_json::json!({ "n": n })).await.unwrap().id);
        }

        // Pull the stream item-by-item (no Vec materialization).
        let mut stream = store
            .scan_from_async::<serde_json::Value>("k", &ids[2], 0)
            .await
            .unwrap();
        let mut got = Vec::new();
        while let Some(item) = stream.next().await {
            got.push(item.unwrap());
        }
        assert_eq!(got.len(), 3);
        assert_eq!(got[0]["n"], 2);
        assert_eq!(got[2]["n"], 4);

        // limit caps the stream.
        let limited: Vec<_> = store
            .scan_from_async::<serde_json::Value>("k", &ids[0], 2)
            .await
            .unwrap()
            .collect()
            .await;
        assert_eq!(limited.len(), 2);
    });
}

#[test]
fn promotable_round_trips_and_scan_documents() {
    let (_dir, store) = make_store();
    futures_lite::future::block_on(async {
        let doc = store
            .append_promotable_async(
                "k",
                Note { kind: "observation".into(), text: "saw a cat".into() },
            )
            .await
            .unwrap();
        assert_eq!(doc.record_type.as_deref(), Some("observation"));
        assert_eq!(doc.summary.as_deref(), Some("saw a cat"));
        assert_eq!(doc.title, None);

        let docs = store.scan_documents_async("k", 10).await.unwrap();
        assert_eq!(docs.len(), 1);
        assert_eq!(docs[0].record_type.as_deref(), Some("observation"));
        assert_eq!(docs[0].summary.as_deref(), Some("saw a cat"));
        assert_eq!(docs[0].title, None);
    });
}

#[test]
fn plain_append_async_leaves_promoted_columns_null() {
    let (_dir, store) = make_store();
    futures_lite::future::block_on(async {
        store.append_async("k", serde_json::json!({"n": 1})).await.unwrap();
        let docs = store.scan_documents_async("k", 10).await.unwrap();
        assert_eq!(docs[0].record_type, None);
        assert_eq!(docs[0].title, None);
        assert_eq!(docs[0].summary, None);
    });
}

#[test]
fn delete_async_removes() {
    let (_dir, store) = make_store();
    futures_lite::future::block_on(async {
        let a = store.append_async("k", serde_json::json!({"n": 0})).await.unwrap();
        store.append_async("k", serde_json::json!({"n": 1})).await.unwrap();
        store.delete_async("k", &a.id).await.unwrap();
        assert_eq!(store.count_async("k").await.unwrap(), 1);
    });
}
