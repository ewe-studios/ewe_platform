//! Contract tests for the Cloudflare wasm stores, on the REAL bindings.
//!
//! WHY: the D1, R2 and KV stores call the Workers bindings, whose type rules
//! differ from the REST clients. These tests run inside workerd under
//! `wrangler dev --local`, against real local D1, R2 and KV storage. No stub.
//! WHAT: the same get/set/list/batch/take/blob range/conditional put contract
//! the native stores have, plus the D1 integer type and precision rules.
//! HOW: the testbed worker sets `globalThis.__CF_TEST_ENV__` to the worker
//! `env`; the helpers in this crate build each store over `env.DB`, `env.BUCKET`
//! and `env.KV`.

#![cfg(target_arch = "wasm32")]

use std::sync::atomic::{AtomicU32, Ordering};
use std::sync::Arc;

use foundation_db::traits::{BlobPutCondition, BlobRange};
use foundation_db::{
    BlobStore, CfD1R2DocumentStore, D1WasmStorage, DataValue, DocumentStore, KVWasmStorage,
    KeyValueStore, QueryStore, R2WasmStorage, StorageBackend, StorageProvider,
};
use foundation_db_wasm_cf_tests::{d1, kv, r2};
use futures_lite::StreamExt;
use wasm_bindgen_test::wasm_bindgen_test;

/// A fresh identifier per call: the local D1 and R2 state persists between runs.
fn unique(prefix: &str) -> String {
    static NEXT: AtomicU32 = AtomicU32::new(0);
    let count = NEXT.fetch_add(1, Ordering::Relaxed);
    format!("{prefix}_{}_{count}", js_sys::Date::now() as u64)
}

async fn d1_store(prefix: &str) -> D1WasmStorage {
    let db = Arc::new(d1("DB"));
    let provider = StorageProvider::new(StorageBackend::D1Wasm {
        db: Arc::clone(&db),
        table_prefix: prefix.to_string(),
    })
    .await
    .expect("D1 provider");
    provider.init_schema_async().await.expect("D1 schema");
    D1WasmStorage::new(db, prefix)
}

async fn collect_blob_keys(store: &R2WasmStorage, prefix: &str) -> Vec<String> {
    let mut keys = Vec::new();
    let mut stream = store
        .list_blobs_after(prefix, None, None)
        .await
        .expect("R2 list");
    while let Some(key) = stream.next().await {
        keys.push(key.expect("R2 listed key"));
    }
    keys
}

#[wasm_bindgen_test]
async fn d1_kv_contract() {
    let prefix = unique("d1kv");
    let store = d1_store(&prefix).await;

    assert!(store.get::<String>("missing").await.unwrap().is_none());

    store.set("k1", "v1".to_string()).await.unwrap();
    store.set("k2", "v2".to_string()).await.unwrap();
    assert_eq!(store.get::<String>("k1").await.unwrap(), Some("v1".into()));
    assert!(store.exists("k1").await.unwrap());
    assert!(!store.exists("missing").await.unwrap());

    let mut keys = store.list_keys(Some("k")).await.unwrap();
    let mut seen = Vec::new();
    while let Some(key) = keys.next().await {
        seen.push(key.unwrap());
    }
    assert_eq!(seen, vec!["k1".to_string(), "k2".to_string()]);

    // `take` is one statement, so exactly one concurrent caller gets the value.
    assert_eq!(
        store.take::<String>("k1").await.unwrap(),
        Some("v1".to_string())
    );
    assert!(store.get::<String>("k1").await.unwrap().is_none());

    store.delete("k2").await.unwrap();
    assert!(!store.exists("k2").await.unwrap());
}

#[wasm_bindgen_test]
async fn d1_blob_contract() {
    let prefix = unique("d1blob");
    let store = d1_store(&prefix).await;
    let data = b"hello d1 blob".to_vec();

    store.put_blob("blob", &data).await.unwrap();
    assert!(store.blob_exists("blob").await.unwrap());
    assert_eq!(store.get_blob("blob").await.unwrap(), Some(data.clone()));

    // The default range read slices the whole object.
    let mut range = store
        .get_blob_range(
            "blob",
            BlobRange {
                start: 6,
                end: Some(8),
            },
        )
        .await
        .unwrap();
    let mut slice = Vec::new();
    while let Some(chunk) = range.next().await {
        slice.extend(chunk.unwrap());
    }
    assert_eq!(slice, b"d1".to_vec());

    store.delete_blob("blob").await.unwrap();
    assert!(store.get_blob("blob").await.unwrap().is_none());
}

/// E1: an integer parameter is stored as INTEGER in an untyped column, and the
/// full `i64` range round-trips exactly. Read it back with `CAST(v AS TEXT)`,
/// because the binding returns an INTEGER column as a JS number (a double).
#[wasm_bindgen_test]
async fn d1_integer_types_and_i64_precision() {
    let store = d1_store(&unique("d1int")).await;
    let table = unique("ints");
    store
        .execute_batch(&format!("CREATE TABLE {table} (v)"))
        .await
        .unwrap();

    let values = [i64::MAX, i64::MIN, (1_i64 << 53) + 1, 42];
    for value in values {
        store
            .execute(
                &format!("INSERT INTO {table} (v) VALUES (?)"),
                &[DataValue::Integer(value)],
            )
            .await
            .unwrap();
    }

    let mut rows = store
        .query(
            &format!("SELECT typeof(v) AS kind, CAST(v AS TEXT) AS text FROM {table} ORDER BY rowid"),
            &[],
        )
        .await
        .unwrap();
    let mut seen = Vec::new();
    while let Some(row) = rows.next().await {
        let row = row.unwrap();
        let kind: String = row.get_by_name("kind").unwrap();
        let text: String = row.get_by_name("text").unwrap();
        seen.push((kind, text));
    }
    let expected: Vec<(String, String)> = values
        .iter()
        .map(|value| ("integer".to_string(), value.to_string()))
        .collect();
    assert_eq!(seen, expected);
}

#[wasm_bindgen_test]
async fn r2_blob_contract() {
    let store = R2WasmStorage::new(r2("BUCKET"), &unique("r2blob"));
    let data = b"hello r2 blob".to_vec();

    store.put_blob("blob", &data).await.unwrap();
    assert!(store.blob_exists("blob").await.unwrap());
    assert_eq!(store.get_blob("blob").await.unwrap(), Some(data.clone()));

    store.put_blob("other", b"second").await.unwrap();
    let mut keys = collect_blob_keys(&store, "").await;
    keys.sort();
    assert_eq!(keys, vec!["blob".to_string(), "other".to_string()]);

    let etag = store.blob_etag("blob").await.unwrap();
    assert!(etag.is_some(), "R2 exposes an ETag");

    let mut range = store
        .get_blob_range(
            "blob",
            BlobRange {
                start: 6,
                end: Some(8),
            },
        )
        .await
        .unwrap();
    let mut slice = Vec::new();
    while let Some(chunk) = range.next().await {
        slice.extend(chunk.unwrap());
    }
    assert_eq!(slice, b"r2".to_vec());

    store.delete_blob("blob").await.unwrap();
    assert!(store.get_blob("blob").await.unwrap().is_none());
}

/// R2's conditional put contract, on the real binding. The store uses the
/// trait's check-then-write default (non-atomic) and reports a failed
/// precondition, because workerd's local R2 binding enforces `onlyIf` but
/// resolves the promise instead of rejecting when the condition fails.
#[wasm_bindgen_test]
async fn r2_conditional_put_and_etag() {
    let store = R2WasmStorage::new(r2("BUCKET"), &unique("r2cond"));
    let first = b"first".to_vec();

    store
        .put_blob_conditionally("object", &first, BlobPutCondition::IfNoneMatch)
        .await
        .unwrap();
    assert!(
        store
            .put_blob_conditionally("object", b"second", BlobPutCondition::IfNoneMatch)
            .await
            .is_err(),
        "an existing object fails IfNoneMatch"
    );

    let etag = store.blob_etag("object").await.unwrap().expect("ETag");
    store
        .put_blob_conditionally(
            "object",
            b"second",
            BlobPutCondition::IfMatch(etag.clone()),
        )
        .await
        .unwrap();
    assert_eq!(
        store.get_blob("object").await.unwrap(),
        Some(b"second".to_vec())
    );

    assert!(
        store
            .put_blob_conditionally("object", b"stale", BlobPutCondition::IfMatch(etag))
            .await
            .is_err(),
        "a stale ETag fails IfMatch"
    );
    assert_eq!(
        store.get_blob("object").await.unwrap(),
        Some(b"second".to_vec())
    );
}

#[wasm_bindgen_test]
async fn kv_contract() {
    let store = KVWasmStorage::new(kv("KV"), &unique("kv"));
    assert!(store.get::<String>("missing").await.unwrap().is_none());

    store.set("k1", "v1".to_string()).await.unwrap();
    store.set("k2", "v2".to_string()).await.unwrap();
    assert_eq!(store.get::<String>("k1").await.unwrap(), Some("v1".into()));
    assert!(store.exists("k1").await.unwrap());

    let mut keys = store.list_keys(Some("k")).await.unwrap();
    let mut seen = Vec::new();
    while let Some(key) = keys.next().await {
        seen.push(key.unwrap());
    }
    seen.sort();
    assert_eq!(seen, vec!["k1".to_string(), "k2".to_string()]);

    // KV has no conditional delete, so `take` is a documented read-then-delete.
    assert_eq!(
        store.take::<String>("k1").await.unwrap(),
        Some("v1".to_string())
    );
    store.delete("k2").await.unwrap();
    assert!(!store.exists("k2").await.unwrap());
}

#[wasm_bindgen_test]
async fn d1r2_document_store_round_trip() {
    let table = unique("docs");
    let d1 = d1_store(&unique("cfdocs")).await;
    let r2 = R2WasmStorage::new(r2("BUCKET"), &unique("cfdocs_bucket"));
    d1.execute_batch(&format!(
        "CREATE TABLE {table} (\
            id INTEGER PRIMARY KEY AUTOINCREMENT, \
            collection_key TEXT NOT NULL, \
            doc_id TEXT NOT NULL, \
            content TEXT NOT NULL, \
            metadata TEXT NOT NULL DEFAULT '{{}}', \
            created_at TIMESTAMP DEFAULT CURRENT_TIMESTAMP, \
            title TEXT, summary TEXT, record_type TEXT, r2_key TEXT); \
         CREATE UNIQUE INDEX idx_{table}_collection_doc_id \
            ON {table}(collection_key, doc_id)"
    ))
    .await
    .unwrap();
    let store = CfD1R2DocumentStore::new(d1, r2).with_table(&table);

    let key = unique("collection");
    store.append(&key, serde_json::json!({ "n": 0 })).await.unwrap();
    store.append(&key, serde_json::json!({ "n": 1 })).await.unwrap();

    let docs = store.scan_documents(&key, 10).await.unwrap();
    assert_eq!(docs.len(), 2);
    // `scan_documents` is newest-first.
    let newest: serde_json::Value = serde_json::from_str(&docs[0].content).unwrap();
    let oldest: serde_json::Value = serde_json::from_str(&docs[1].content).unwrap();
    assert_eq!(newest, serde_json::json!({ "n": 1 }));
    assert_eq!(oldest, serde_json::json!({ "n": 0 }));

    // Past the 4 KB page threshold the content moves to R2 and reads back.
    let big = "x".repeat(5 * 1024);
    let appended = store
        .append(&key, serde_json::json!({ "big": big }))
        .await
        .unwrap();
    let read = store
        .scan_documents_from(&key, &appended.id, 1)
        .await
        .unwrap();
    assert_eq!(read.len(), 1);
    let content: serde_json::Value = serde_json::from_str(&read[0].content).unwrap();
    assert_eq!(content, serde_json::json!({ "big": big }));
}
