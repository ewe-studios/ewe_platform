//! libsql storage backend integration tests.
#![cfg(feature = "libsql")]

use foundation_core::valtron::valtron_test;
use foundation_db::{KeyValueStore, LibsqlStore};
use futures_lite::StreamExt;
use tempfile::TempDir;

/// Open and initialise a KV store.
async fn open(url: &str) -> LibsqlStore {
    let storage = LibsqlStore::new_kv(url, None).await.unwrap();
    storage.init_kv().await.unwrap();
    storage
}

async fn keys(storage: &LibsqlStore, prefix: Option<&str>) -> Vec<String> {
    storage
        .list_keys_async(prefix)
        .await
        .unwrap()
        .map(Result::unwrap)
        .collect()
        .await
}

#[valtron_test]
async fn test_libsql_storage_basic() {
    let temp_dir = TempDir::new().unwrap();
    let db_path = temp_dir.path().join("test.db");
    let storage = open(db_path.to_str().unwrap()).await;

    storage.set_async("test_key", "test_value".to_string()).await.unwrap();

    let value: Option<String> = storage.get_async("test_key").await.unwrap();
    assert_eq!(value, Some("test_value".to_string()));

    assert!(storage.exists_async("test_key").await.unwrap());
    assert!(!storage.exists_async("nonexistent").await.unwrap());

    storage.delete_async("test_key").await.unwrap();
    assert!(!storage.exists_async("test_key").await.unwrap());
}

#[valtron_test]
async fn test_libsql_storage_list_keys() {
    let temp_dir = TempDir::new().unwrap();
    let db_path = temp_dir.path().join("test.db");
    let storage = open(db_path.to_str().unwrap()).await;

    storage.set_async("prefix:key1", "value1".to_string()).await.unwrap();
    storage.set_async("prefix:key2", "value2".to_string()).await.unwrap();
    storage.set_async("other:key3", "value3".to_string()).await.unwrap();

    assert_eq!(keys(&storage, None).await.len(), 3);
    assert_eq!(keys(&storage, Some("prefix:")).await, vec!["prefix:key1", "prefix:key2"]);
}
