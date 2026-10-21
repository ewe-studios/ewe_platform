//! libsql storage backend integration tests.
#![cfg(feature = "libsql")]

use std::sync::Mutex;

use foundation_core::valtron::{initialize_pool, BlockingFutureExt, PoolGuard};
use foundation_db::{Blocking, KeyValueStore, LibsqlStore, StorageError};
use tempfile::TempDir;

/// Open + init a KV store (async constructors) and wrap it in the sync view.
fn open(url: &str) -> Blocking<LibsqlStore> {
    let url = url.to_owned();
    let storage = BlockingFutureExt::block(async move {
        let storage = LibsqlStore::new_kv(&url, None).await?;
        storage.init_kv().await?;
        Ok::<_, StorageError>(storage)
    })
    .unwrap();
    Blocking::new(storage)
}

/// Initialize the Valtron executor for tests (shared across the binary).
fn init_valtron() {
    static POOL_GUARD: Mutex<Option<PoolGuard>> = Mutex::new(None);
    let mut guard = POOL_GUARD.lock().unwrap();
    if guard.is_none() {
        *guard = Some(initialize_pool(42, None));
    }
}

#[test]
fn test_libsql_storage_basic() {
    init_valtron();
    let temp_dir = TempDir::new().unwrap();
    let db_path = temp_dir.path().join("test.db");
    let url = db_path.to_str().unwrap();

    let storage = open(url);

    storage.set::<String>("test_key", "test_value".to_string()).unwrap();

    let value: Option<String> = storage.get("test_key").unwrap();
    assert_eq!(value, Some("test_value".to_string()));

    let exists: bool = storage.exists("test_key").unwrap();
    assert!(exists);

    let not_exists: bool = storage.exists("nonexistent").unwrap();
    assert!(!not_exists);

    storage.delete("test_key").unwrap();

    let deleted: bool = storage.exists("test_key").unwrap();
    assert!(!deleted);
}

#[test]
fn test_libsql_storage_list_keys() {
    init_valtron();
    let temp_dir = TempDir::new().unwrap();
    let db_path = temp_dir.path().join("test.db");
    let url = db_path.to_str().unwrap();

    let storage = open(url);

    storage.set::<String>("prefix:key1", "value1".to_string()).unwrap();
    storage.set::<String>("prefix:key2", "value2".to_string()).unwrap();
    storage.set::<String>("other:key3", "value3".to_string()).unwrap();

    let keys: Vec<String> = storage
        .list_keys(None)
        .unwrap()
        .flat_map(|stream_item| match stream_item {
            foundation_core::valtron::Stream::Next(Ok(result)) => vec![result],
            _ => vec![],
        })
        .collect();
    assert_eq!(keys.len(), 3);

    let keys: Vec<String> = storage
        .list_keys(Some("prefix:"))
        .unwrap()
        .flat_map(|stream_item| match stream_item {
            foundation_core::valtron::Stream::Next(Ok(result)) => vec![result],
            _ => vec![],
        })
        .collect();
    assert_eq!(keys.len(), 2);
}

