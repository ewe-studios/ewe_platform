//! Ready-to-use stores: `TursoStorage::in_memory()` / `LibsqlStore::in_memory()`
//! open a migrated in-memory database (the async replacement for the old
//! blocking `Default` impls), and `StorageProvider::default()` is the
//! in-memory backend.

use foundation_core::valtron::valtron_test;
use foundation_db::traits::{AsyncDocumentStore, AsyncKeyValueStore};
use foundation_db::{SqlDocumentStore, StorageProvider, TursoStorage};

#[cfg(feature = "libsql")]
use foundation_db::LibsqlStore;

#[valtron_test]
async fn in_memory_turso_storage_is_a_migrated_key_value_store() {
    let storage = TursoStorage::in_memory().await.unwrap();

    storage.set_async("greeting", "hello").await.unwrap();
    let value: Option<String> = storage.get_async("greeting").await.unwrap();
    assert_eq!(value.as_deref(), Some("hello"));
}

#[valtron_test]
async fn sql_document_store_over_in_memory_turso_has_its_documents_table() {
    let store = SqlDocumentStore::new(TursoStorage::in_memory().await.unwrap());

    let doc = store.append_async("k", serde_json::json!({"n": 1})).await.unwrap();
    let docs = store.scan_documents_async("k", 10).await.unwrap();
    assert_eq!(docs.len(), 1);
    assert_eq!(docs[0].id, doc.id);
}

#[valtron_test]
async fn in_memory_turso_stores_are_independent() {
    let a = TursoStorage::in_memory().await.unwrap();
    let b = TursoStorage::in_memory().await.unwrap();

    a.set_async("only_in_a", 1_u32).await.unwrap();
    let in_b: Option<u32> = b.get_async("only_in_a").await.unwrap();
    assert_eq!(in_b, None, "each in-memory store is its own database");
}

#[cfg(feature = "libsql")]
#[valtron_test]
async fn in_memory_libsql_store_is_a_migrated_key_value_store() {
    let store = LibsqlStore::in_memory().await.unwrap();

    store.set_async("greeting", "hello").await.unwrap();
    let value: Option<String> = store.get_async("greeting").await.unwrap();
    assert_eq!(value.as_deref(), Some("hello"));
}

#[valtron_test]
async fn default_storage_provider_is_in_memory() {
    let provider = StorageProvider::default();

    provider.set_async("k", "v").await.unwrap();
    let value: Option<String> = provider.get_async("k").await.unwrap();
    assert_eq!(value.as_deref(), Some("v"));
}
