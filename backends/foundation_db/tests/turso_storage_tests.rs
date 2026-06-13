//! Turso storage backend integration tests.

use foundation_db::{DataValue, EncryptionKey, KeyValueStore, QueryStore, TursoStorage};
use std::sync::Mutex;
use tempfile::TempDir;

/// Shared Valtron pool guard — initialized once and reused across all tests
/// to avoid parallel tests interfering with each other's thread pool.
static POOL_GUARD: Mutex<Option<foundation_core::valtron::PoolGuard>> = Mutex::new(None);

/// Initialize the Valtron executor for tests.
fn init_valtron() {
    let mut guard = POOL_GUARD.lock().unwrap();
    if guard.is_none() {
        *guard = Some(foundation_core::valtron::initialize_pool(42, None));
    }
}

#[test]
fn test_turso_storage_basic() {
    init_valtron();
    let temp_dir = TempDir::new().unwrap();
    let db_path = temp_dir.path().join("test.db");
    let url = db_path.to_str().unwrap();

    let storage = TursoStorage::new(url).unwrap();
    storage.init_schema().unwrap();

    storage
        .set::<String>("test_key", "test_value".to_string())
        .unwrap();

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
fn test_turso_storage_encryption() {
    init_valtron();
    let temp_dir = TempDir::new().unwrap();
    let db_path = temp_dir.path().join("test.db");
    let url = db_path.to_str().unwrap();

    let key = EncryptionKey::generate();
    let storage = TursoStorage::with_encryption(url, Some(key.clone())).unwrap();
    storage.init_schema().unwrap();

    let secret_value = "sensitive_data_12345".to_string();
    storage.set("secret_key", secret_value.clone()).unwrap();

    let value: Option<String> = storage.get("secret_key").unwrap();
    assert_eq!(value, Some(secret_value.clone()));

    let raw_result = storage
        .query(
            "SELECT value FROM kv_store WHERE key = ?",
            &[DataValue::Text("secret_key".to_string())],
        )
        .unwrap()
        .flat_map(|stream_item| match stream_item {
            foundation_core::valtron::Stream::Next(Ok(result)) => vec![result],
            _ => vec![],
        })
        .collect::<Vec<_>>();

    assert!(!raw_result.is_empty(), "Should have a result");
    let stored_value = raw_result[0].get::<String>(0).unwrap();
    assert_ne!(
        stored_value, secret_value,
        "Value should be encrypted in database"
    );
    assert!(
        stored_value.len() > secret_value.len(),
        "Encrypted value should be longer due to nonce + tag + base64"
    );
}

#[test]
fn test_turso_storage_encryption_wrong_key() {
    init_valtron();
    let temp_dir = TempDir::new().unwrap();
    let db_path = temp_dir.path().join("test.db");
    let url = db_path.to_str().unwrap();

    let key1 = EncryptionKey::generate();
    let storage1 = TursoStorage::with_encryption(url, Some(key1.clone())).unwrap();
    storage1.init_schema().unwrap();

    let secret_value = "sensitive_data".to_string();
    storage1.set("secret_key", secret_value.clone()).unwrap();

    let key2 = EncryptionKey::generate();
    let storage2 = TursoStorage::with_encryption(url, Some(key2.clone())).unwrap();

    let result = storage2.get::<String>("secret_key");
    assert!(
        result.is_err(),
        "Should fail with wrong key, got: {result:?}"
    );
}

#[test]
fn test_turso_storage_list_keys() {
    init_valtron();
    let temp_dir = TempDir::new().unwrap();
    let db_path = temp_dir.path().join("test.db");
    let url = db_path.to_str().unwrap();

    let storage = TursoStorage::new(url).unwrap();
    storage.init_schema().unwrap();

    storage
        .set::<String>("prefix:key1", "value1".to_string())
        .unwrap();
    storage
        .set::<String>("prefix:key2", "value2".to_string())
        .unwrap();
    storage
        .set::<String>("other:key3", "value3".to_string())
        .unwrap();

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

#[test]
fn test_turso_storage_migrations() {
    init_valtron();
    let temp_dir = TempDir::new().unwrap();
    let db_path = temp_dir.path().join("test.db");
    let url = db_path.to_str().unwrap();

    let storage = TursoStorage::new(url).unwrap();

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
