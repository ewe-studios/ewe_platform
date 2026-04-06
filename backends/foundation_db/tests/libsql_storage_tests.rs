//! libsql storage backend integration tests.
#![cfg(feature = "libsql")]

use foundation_core::valtron::collect_one;
use foundation_db::{KeyValueStore, LibsqlStorage, QueryStore};
use tempfile::TempDir;

/// Initialize the Valtron executor for tests.
fn init_valtron() {
    foundation_core::valtron::single::initialize_pool(42);
}

#[test]
fn test_libsql_storage_basic() {
    init_valtron();
    let temp_dir = TempDir::new().unwrap();
    let db_path = temp_dir.path().join("test.db");
    let url = db_path.to_str().unwrap();

    let storage = LibsqlStorage::new(url).unwrap();
    storage.init_schema().unwrap();

    let _: () = collect_one(
        storage
            .set::<String>("test_key", "test_value".to_string())
            .unwrap(),
    )
    .unwrap()
    .unwrap();

    let value: Option<String> = collect_one(storage.get("test_key").unwrap())
        .unwrap()
        .unwrap();
    assert_eq!(value, Some("test_value".to_string()));

    let exists: bool = collect_one(storage.exists("test_key").unwrap())
        .unwrap()
        .unwrap();
    assert!(exists);

    let not_exists: bool = collect_one(storage.exists("nonexistent").unwrap())
        .unwrap()
        .unwrap();
    assert!(!not_exists);

    let _: () = collect_one(storage.delete("test_key").unwrap())
        .unwrap()
        .unwrap();

    let deleted: bool = collect_one(storage.exists("test_key").unwrap())
        .unwrap()
        .unwrap();
    assert!(!deleted);
}

#[test]
fn test_libsql_storage_list_keys() {
    init_valtron();
    let temp_dir = TempDir::new().unwrap();
    let db_path = temp_dir.path().join("test.db");
    let url = db_path.to_str().unwrap();

    let storage = LibsqlStorage::new(url).unwrap();
    storage.init_schema().unwrap();

    let _: () = collect_one(
        storage
            .set::<String>("prefix:key1", "value1".to_string())
            .unwrap(),
    )
    .unwrap()
    .unwrap();
    let _: () = collect_one(
        storage
            .set::<String>("prefix:key2", "value2".to_string())
            .unwrap(),
    )
    .unwrap()
    .unwrap();
    let _: () = collect_one(
        storage
            .set::<String>("other:key3", "value3".to_string())
            .unwrap(),
    )
    .unwrap()
    .unwrap();

    // List all keys - flat_map to extract Result from Stream, then collect
    let keys: Vec<String> = storage
        .list_keys(None)
        .unwrap()
        .flat_map(|stream_item| match stream_item {
            foundation_core::valtron::Stream::Next(Ok(result)) => vec![result],
            _ => vec![],
        })
        .collect();
    assert_eq!(keys.len(), 3);

    // List keys with prefix
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

#[test]
fn test_libsql_storage_migrations() {
    init_valtron();
    let temp_dir = TempDir::new().unwrap();
    let db_path = temp_dir.path().join("test.db");
    let url = db_path.to_str().unwrap();

    let storage = LibsqlStorage::new(url).unwrap();

    let migrations = &[
        (
            "001_create_users",
            "CREATE TABLE users (id TEXT PRIMARY KEY, email TEXT UNIQUE NOT NULL)",
        ),
        (
            "002_create_sessions",
            "CREATE TABLE sessions (id TEXT PRIMARY KEY, user_id TEXT NOT NULL)",
        ),
    ];

    storage.migrate(migrations).unwrap();

    let users_exist = !storage
        .query(
            "SELECT 1 FROM sqlite_master WHERE type='table' AND name='users'",
            &[],
        )
        .unwrap()
        .flat_map(|stream_item| match stream_item {
            foundation_core::valtron::Stream::Next(Ok(result)) => vec![result],
            _ => vec![],
        })
        .collect::<Vec<_>>()
        .is_empty();

    let sessions_exist = !storage
        .query(
            "SELECT 1 FROM sqlite_master WHERE type='table' AND name='sessions'",
            &[],
        )
        .unwrap()
        .flat_map(|stream_item| match stream_item {
            foundation_core::valtron::Stream::Next(Ok(result)) => vec![result],
            _ => vec![],
        })
        .collect::<Vec<_>>()
        .is_empty();

    let migrations_exist = !storage
        .query(
            "SELECT 1 FROM sqlite_master WHERE type='table' AND name='_migrations'",
            &[],
        )
        .unwrap()
        .flat_map(|stream_item| match stream_item {
            foundation_core::valtron::Stream::Next(Ok(result)) => vec![result],
            _ => vec![],
        })
        .collect::<Vec<_>>()
        .is_empty();

    assert!(users_exist, "users table should be accessible");
    assert!(sessions_exist, "sessions table should be accessible");
    assert!(migrations_exist, "_migrations table should be accessible");
}
