//! `SqlDocumentStore` over libsql (F22b): the generic SQL document store runs
//! on `LibsqlStore`, not just Turso.
//!
//! The full behavioural suite (promoted columns, limits) is the Turso one
//! (`document_store_tests.rs`, `async_sql_document_store_tests.rs`); every SQL
//! backend shares the same statement builder, so this checks the libsql
//! connection end to end: migrated schema, append, ordered scans, count,
//! delete.
//!
//! Gated behind the `libsql` feature.
#![cfg(feature = "libsql")]

use foundation_core::valtron::valtron_test;
use foundation_db::{DocumentStore, LibsqlStore, SqlDocumentStore};
use futures_lite::StreamExt;

async fn store() -> SqlDocumentStore<LibsqlStore> {
    SqlDocumentStore::new(LibsqlStore::in_memory().await.unwrap())
}

#[valtron_test]
async fn append_scan_count_and_delete_over_libsql() {
    let store = store().await;

    let mut ids = Vec::new();
    for n in 0..3 {
        ids.push(store.append_async("k", serde_json::json!({ "n": n })).await.unwrap().id);
    }
    assert_eq!(store.count_async("k").await.unwrap(), 3);

    // Newest first.
    let docs = store.scan_documents_async("k", 10).await.unwrap();
    let got: Vec<&str> = docs.iter().map(|d| d.id.as_str()).collect();
    assert_eq!(got, vec![ids[2].as_str(), ids[1].as_str(), ids[0].as_str()]);

    // Inclusive `scan_from`, oldest first.
    let from_second: Vec<serde_json::Value> = store
        .scan_from_async::<serde_json::Value>("k", &ids[1], 0)
        .await
        .unwrap()
        .map(Result::unwrap)
        .collect()
        .await;
    assert_eq!(from_second, vec![serde_json::json!({"n": 1}), serde_json::json!({"n": 2})]);

    store.delete_async("k", &ids[0]).await.unwrap();
    assert_eq!(store.count_async("k").await.unwrap(), 2);
    assert_eq!(store.delete_all_async("k").await.unwrap(), 2);
    assert_eq!(store.count_async("k").await.unwrap(), 0);
}
