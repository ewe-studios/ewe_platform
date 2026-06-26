//! `D1R2DocumentStore` conformance (F23) — exercises the target-agnostic
//! D1+R2 store logic over real, in-process backends with no external infra.
//!
//! WHY: `D1R2DocumentStore<Q, B>` is the Cloudflare DocumentStore; on Workers it
//! runs over `D1WasmStorage` + `R2WasmStorage`, and the gated
//! `d1r2_document_store_tests` proves the native D1/R2 REST clients against a
//! miniflare worker. This suite proves the *store logic itself* — 4 KB offload
//! threshold, `r2_key` bookkeeping, transparent read-through, scru128 ordering,
//! inclusive `scan_from`, and delete cleanup — using `TursoStorage` (real
//! in-memory SQLite, an `AsyncQueryStore`) as the index and `MemoryStorage` (an
//! `AsyncBlobStore`) as the blob backend. It runs everywhere, including CI.
//!
//! HOW: `TursoStorage::init_schema` applies the F06 `documents` migrations incl.
//! the F23 `r2_key` column (022); async methods are driven with
//! `futures_lite::future::block_on`, with valtron initialized for Turso.

use std::sync::Mutex;

use foundation_db::traits::{AsyncBlobStore, AsyncDocumentStore};
use foundation_db::{D1R2DocumentStore, MemoryStorage, TursoStorage};
use futures_lite::StreamExt;

static POOL_GUARD: Mutex<Option<foundation_core::valtron::PoolGuard>> = Mutex::new(None);

fn init_valtron() {
    let mut guard = POOL_GUARD.lock().unwrap();
    if guard.is_none() {
        *guard = Some(foundation_core::valtron::initialize_pool(42, None));
    }
}

/// A `D1R2DocumentStore` over in-memory SQLite (index) + in-memory blobs (R2),
/// plus a handle to the blob store so tests can assert on offload directly.
fn make_store() -> (D1R2DocumentStore<TursoStorage, MemoryStorage>, MemoryStorage) {
    init_valtron();
    let turso = TursoStorage::new(":memory:").unwrap();
    turso.init_schema().unwrap();
    let blobs = MemoryStorage::new();
    let store = D1R2DocumentStore::new(turso, blobs.clone());
    (store, blobs)
}

fn blob_key(collection: &str, doc_id: &str) -> String {
    format!("doc/{collection}/{doc_id}")
}

#[test]
fn append_orders_by_scru128_and_counts() {
    let (store, _blobs) = make_store();
    futures_lite::future::block_on(async {
        let a = store.append_async("k", serde_json::json!({"n": 0})).await.unwrap();
        let b = store.append_async("k", serde_json::json!({"n": 1})).await.unwrap();
        let c = store.append_async("k", serde_json::json!({"n": 2})).await.unwrap();
        assert_eq!(a.id.len(), 25, "scru128 id");
        assert!(b.id > a.id && c.id > b.id, "appends produce ascending scru128 ids");
        assert_eq!(store.count_async("k").await.unwrap(), 3);

        // scan_documents_async returns newest-first (DESC).
        let docs = store.scan_documents_async("k", 10).await.unwrap();
        let ids: Vec<&str> = docs.iter().map(|d| d.id.as_str()).collect();
        assert_eq!(ids, vec![c.id.as_str(), b.id.as_str(), a.id.as_str()]);
    });
}

#[test]
fn scan_from_is_inclusive_and_ascending() {
    let (store, _blobs) = make_store();
    futures_lite::future::block_on(async {
        let _a = store.append_async("k", serde_json::json!({"n": 0})).await.unwrap();
        let b = store.append_async("k", serde_json::json!({"n": 1})).await.unwrap();
        let _c = store.append_async("k", serde_json::json!({"n": 2})).await.unwrap();

        let from_b: Vec<serde_json::Value> = store
            .scan_from_async::<serde_json::Value>("k", &b.id, 0)
            .await
            .unwrap()
            .map(Result::unwrap)
            .collect()
            .await;
        assert_eq!(from_b.len(), 2, "inclusive of b, then c");
        assert_eq!(from_b[0]["n"], 1);
        assert_eq!(from_b[1]["n"], 2);
    });
}

#[test]
fn small_documents_stay_inline_no_offload() {
    let (store, blobs) = make_store();
    futures_lite::future::block_on(async {
        let doc = store
            .append_async("k", serde_json::json!({"body": "x".repeat(16)}))
            .await
            .unwrap();
        // Below the 4 KB threshold → no blob written.
        assert!(
            !blobs.blob_exists_async(&blob_key("k", &doc.id)).await.unwrap(),
            "small doc must not offload to the blob store"
        );
        let back: Vec<serde_json::Value> = store
            .scan_all_async::<serde_json::Value>("k")
            .await
            .unwrap()
            .map(Result::unwrap)
            .collect()
            .await;
        assert_eq!(back.len(), 1);
        assert_eq!(back[0]["body"], "x".repeat(16));
    });
}

#[test]
fn large_documents_offload_and_read_through() {
    let (store, blobs) = make_store();
    futures_lite::future::block_on(async {
        let body = "y".repeat(8192); // > 4 KB → offload
        let doc = store
            .append_async("k", serde_json::json!({"tag": "big", "body": body.clone()}))
            .await
            .unwrap();

        // The blob landed in the blob store under the expected key.
        let key = blob_key("k", &doc.id);
        assert!(
            blobs.blob_exists_async(&key).await.unwrap(),
            "large doc must offload to the blob store"
        );

        // Transparent read-through: the full content comes back via scan despite
        // the SQL row holding only a placeholder.
        let back: Vec<serde_json::Value> = store
            .scan_all_async::<serde_json::Value>("k")
            .await
            .unwrap()
            .map(Result::unwrap)
            .collect()
            .await;
        assert_eq!(back.len(), 1);
        assert_eq!(back[0]["tag"], "big");
        assert_eq!(back[0]["body"].as_str().unwrap().len(), 8192);
    });
}

#[test]
fn mixed_inline_and_offloaded_round_trip_in_order() {
    let (store, blobs) = make_store();
    futures_lite::future::block_on(async {
        let small = store.append_async("k", serde_json::json!({"i": 0, "b": "s"})).await.unwrap();
        let big = store
            .append_async("k", serde_json::json!({"i": 1, "b": "L".repeat(5000)}))
            .await
            .unwrap();

        assert!(!blobs.blob_exists_async(&blob_key("k", &small.id)).await.unwrap());
        assert!(blobs.blob_exists_async(&blob_key("k", &big.id)).await.unwrap());

        // Ascending scan_from over both — inline + read-through interleave cleanly.
        let all: Vec<serde_json::Value> = store
            .scan_from_async::<serde_json::Value>("k", &small.id, 0)
            .await
            .unwrap()
            .map(Result::unwrap)
            .collect()
            .await;
        assert_eq!(all.len(), 2);
        assert_eq!(all[0]["i"], 0);
        assert_eq!(all[1]["i"], 1);
        assert_eq!(all[1]["b"].as_str().unwrap().len(), 5000);
    });
}

#[test]
fn delete_removes_row_and_offloaded_blob() {
    let (store, blobs) = make_store();
    futures_lite::future::block_on(async {
        let doc = store
            .append_async("k", serde_json::json!({"body": "z".repeat(8192)}))
            .await
            .unwrap();
        let key = blob_key("k", &doc.id);
        assert!(blobs.blob_exists_async(&key).await.unwrap());

        store.delete_async("k", &doc.id).await.unwrap();
        assert_eq!(store.count_async("k").await.unwrap(), 0);
        assert!(
            !blobs.blob_exists_async(&key).await.unwrap(),
            "delete must remove the offloaded blob"
        );
    });
}

#[test]
fn delete_all_clears_collection_and_blobs() {
    let (store, blobs) = make_store();
    futures_lite::future::block_on(async {
        let d0 = store.append_async("k", serde_json::json!({"b": "a".repeat(6000)})).await.unwrap();
        let d1 = store.append_async("k", serde_json::json!({"b": "b".repeat(6000)})).await.unwrap();
        store.append_async("other", serde_json::json!({"b": "keep"})).await.unwrap();

        let removed = store.delete_all_async("k").await.unwrap();
        assert_eq!(removed, 2);
        assert_eq!(store.count_async("k").await.unwrap(), 0);
        assert_eq!(store.count_async("other").await.unwrap(), 1, "other collection untouched");
        assert!(!blobs.blob_exists_async(&blob_key("k", &d0.id)).await.unwrap());
        assert!(!blobs.blob_exists_async(&blob_key("k", &d1.id)).await.unwrap());
    });
}
