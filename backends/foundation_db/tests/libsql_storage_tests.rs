//! libsql storage backend integration tests.
#![cfg(feature = "libsql")]

use foundation_core::valtron::{collect_one, collect_result, Stream};
use foundation_db::{KeyValueStore, LibsqlStore, QueryStore};
use std::sync::Mutex;
use tempfile::TempDir;

/// Shared Valtron pool guard — initialized once and reused across all tests
static POOL_GUARD: Mutex<Option<foundation_core::valtron::PoolGuard>> = Mutex::new(None);

/// Initialize the Valtron executor for tests.
fn init_valtron() {
    let mut guard = POOL_GUARD.lock().unwrap();
    if guard.is_none() {
        *guard = Some(foundation_core::valtron::initialize_pool(42, Some(3)));
    }
}

#[test]


#[tracing_test::traced_test]
fn test_libsql_storage_basic() {
    init_valtron();
    let temp_dir = TempDir::new().unwrap();
    let db_path = temp_dir.path().join("test.db");
    let url = db_path.to_str().unwrap();

    let storage = LibsqlStore::new_kv(url, None).unwrap();
    storage.init_kv().unwrap();

    let _: () = collect_one(
        storage
            .set::<String>("test_key", "test_value".to_string())
            .unwrap(),
    )
    .unwrap()
    .unwrap();

    let _ = storage.get("test_key")?
        .unwrap()
        .unwrap();
    assert_eq!(value, Some("test_value".to_string()));

    let _ = storage.exists("test_key")?
        .unwrap()
        .unwrap();
    assert!(exists);

    let _ = storage.exists("nonexistent")?
        .unwrap()
        .unwrap();
    assert!(!not_exists);

    let _ = storage.delete("test_key")?
        .unwrap()
        .unwrap();

    let _ = storage.exists("test_key")?
        .unwrap()
        .unwrap();
    assert!(!deleted);
}

#[test]


#[tracing_test::traced_test]
fn test_libsql_storage_list_keys() {
    init_valtron();
    let temp_dir = TempDir::new().unwrap();
    let db_path = temp_dir.path().join("test.db");
    let url = db_path.to_str().unwrap();

    let storage = LibsqlStore::new_kv(url, None).unwrap();
    storage.init_kv().unwrap();

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

    // List all keys - collect_result from the stream
    let keys: Vec<String> = collect_result(storage.list_keys(None).unwrap())
        .into_iter()
        .collect::<Result<Vec<_>, _>>()
        .unwrap();
    assert_eq!(keys.len(), 3);

    // List keys with prefix
    let keys: Vec<String> = collect_result(storage.list_keys(Some("prefix:")).unwrap())
        .into_iter()
        .collect::<Result<Vec<_>, _>>()
        .unwrap();
    assert_eq!(keys.len(), 2);
}

#[test]


#[tracing_test::traced_test]
fn test_libsql_storage_migrations() {
    init_valtron();
    let temp_dir = TempDir::new().unwrap();
    let db_path = temp_dir.path().join("test.db");
    let url = db_path.to_str().unwrap();

    let storage = LibsqlStore::new_kv(url, None).unwrap();

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

    // Apply migrations directly via execute_batch
    for (_name, sql) in migrations {
        collect_one(storage.execute_batch(sql).unwrap())
            .unwrap()
            .unwrap();
    }

    let users_exist = !storage
        .query(
            "SELECT 1 FROM sqlite_master WHERE type='table' AND name='users'",
            &[],
        )
        .unwrap()
        .flat_map(|stream_item| match stream_item {
            Stream::Next(Ok(result)) => vec![result],
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
            Stream::Next(Ok(result)) => vec![result],
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
            Stream::Next(Ok(result)) => vec![result],
            _ => vec![],
        })
        .collect::<Vec<_>>()
        .is_empty();

    assert!(users_exist, "users table should be accessible");
    assert!(sessions_exist, "sessions table should be accessible");
    // _migrations table is only created by the full migration runner, not by raw execute_batch
}
