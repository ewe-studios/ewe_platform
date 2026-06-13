//! libsql storage backend integration tests.
#![cfg(feature = "libsql")]

use foundation_db::{KeyValueStore, LibsqlStore, QueryStore};
use tempfile::TempDir;

/// Initialize the Valtron executor for tests.
fn init_valtron() {
    foundation_core::valtron::single::initialize_pool(42);
}

fn test_libsql_storage_basic() {
    init_valtron();
    let temp_dir = TempDir::new().unwrap();
    let db_path = temp_dir.path().join("test.db");
    let url = db_path.to_str().unwrap();

    let storage = LibsqlStore::new_kv(url, None).unwrap();
    storage.init_kv().unwrap();

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

fn test_libsql_storage_list_keys() {
    init_valtron();
    let temp_dir = TempDir::new().unwrap();
    let db_path = temp_dir.path().join("test.db");
    let url = db_path.to_str().unwrap();

    let storage = LibsqlStore::new_kv(url, None).unwrap();
    storage.init_kv().unwrap();

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

