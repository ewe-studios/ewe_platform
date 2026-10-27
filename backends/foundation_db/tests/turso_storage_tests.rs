//! Turso storage backend integration tests.

use foundation_core::valtron::valtron_test;
use foundation_db::{KeyValueStore, QueryStore, DataValue, EncryptionKey, SqlRow, TursoStorage};
use tempfile::TempDir;

async fn open(url: &str, key: Option<EncryptionKey>) -> TursoStorage {
    TursoStorage::with_encryption(url, key).await.unwrap()
}

async fn init_schema(storage: &TursoStorage) {
    storage.init_schema().await.unwrap();
}

async fn query_rows(storage: &TursoStorage, sql: &str, params: &[DataValue]) -> Vec<SqlRow> {
    storage.query(sql, params).await.unwrap().collect_all().await.unwrap()
}

#[valtron_test]
async fn test_turso_storage_basic() {
    let temp_dir = TempDir::new().unwrap();
    let db_path = temp_dir.path().join("test.db");
    let url = db_path.to_str().unwrap();

    let storage = open(url, None).await;
    init_schema(&storage).await;

    storage
        .set("test_key", "test_value".to_string())
        .await
        .unwrap();

    let value: Option<String> = storage.get("test_key").await.unwrap();
    assert_eq!(value, Some("test_value".to_string()));

    let exists: bool = storage.exists("test_key").await.unwrap();
    assert!(exists);

    let not_exists: bool = storage.exists("nonexistent").await.unwrap();
    assert!(!not_exists);

    storage.delete("test_key").await.unwrap();

    let deleted: bool = storage.exists("test_key").await.unwrap();
    assert!(!deleted);
}

#[valtron_test]
async fn test_turso_storage_encryption() {
    let temp_dir = TempDir::new().unwrap();
    let db_path = temp_dir.path().join("test.db");
    let url = db_path.to_str().unwrap();

    let key = EncryptionKey::generate();
    let storage = open(url, Some(key.clone())).await;
    init_schema(&storage).await;

    let secret_value = "sensitive_data_12345".to_string();
    storage.set("secret_key", secret_value.clone()).await.unwrap();

    let value: Option<String> = storage.get("secret_key").await.unwrap();
    assert_eq!(value, Some(secret_value.clone()));

    let raw_result = query_rows(&storage, "SELECT value FROM kv_store WHERE key = ?", &[DataValue::Text("secret_key".to_string())]).await;

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

#[valtron_test]
async fn test_turso_storage_encryption_wrong_key() {
    let temp_dir = TempDir::new().unwrap();
    let db_path = temp_dir.path().join("test.db");
    let url = db_path.to_str().unwrap();

    let key1 = EncryptionKey::generate();
    let storage1 = open(url, Some(key1.clone())).await;
    init_schema(&storage1).await;

    let secret_value = "sensitive_data".to_string();
    storage1.set("secret_key", secret_value.clone()).await.unwrap();

    let key2 = EncryptionKey::generate();
    let storage2 = open(url, Some(key2.clone())).await;

    let result = storage2.get::<String>("secret_key").await;
    assert!(
        result.is_err(),
        "Should fail with wrong key, got: {result:?}"
    );
}

#[valtron_test]
async fn test_turso_storage_list_keys() {
    let temp_dir = TempDir::new().unwrap();
    let db_path = temp_dir.path().join("test.db");
    let url = db_path.to_str().unwrap();

    let storage = open(url, None).await;
    init_schema(&storage).await;

    storage
        .set("prefix:key1", "value1".to_string())
        .await
        .unwrap();
    storage
        .set("prefix:key2", "value2".to_string())
        .await
        .unwrap();
    storage
        .set("other:key3", "value3".to_string())
        .await
        .unwrap();

    let keys: Vec<String> = storage
        .list_keys(None)
        .await
        .unwrap()
        .collect_all()
        .await
        .unwrap();
    assert_eq!(keys.len(), 3);

    let keys: Vec<String> = storage
        .list_keys(Some("prefix:"))
        .await
        .unwrap()
        .collect_all()
        .await
        .unwrap();
    assert_eq!(keys.len(), 2);
}

#[valtron_test]
async fn test_turso_storage_migrations() {
    let temp_dir = TempDir::new().unwrap();
    let db_path = temp_dir.path().join("test.db");
    let url = db_path.to_str().unwrap();

    let storage = open(url, None).await;

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

    storage.migrate(migrations).await.unwrap();

    let users_exist = !query_rows(&storage, "SELECT 1 FROM sqlite_master WHERE type='table' AND name='users'", &[]).await
        .is_empty();

    let sessions_exist = !query_rows(&storage, "SELECT 1 FROM sqlite_master WHERE type='table' AND name='sessions'", &[]).await
        .is_empty();

    let migrations_exist = !query_rows(&storage, "SELECT 1 FROM sqlite_master WHERE type='table' AND name='_migrations'", &[]).await
        .is_empty();

    assert!(users_exist, "users table should be accessible");
    assert!(sessions_exist, "sessions table should be accessible");
    assert!(migrations_exist, "_migrations table should be accessible");
}

/// Migration 024/025: upstream_providers + user_provider_links tables are
/// created by init_schema and support real INSERT/SELECT round-trips.
#[valtron_test]
async fn test_social_login_tables_functional() {
    let temp_dir = TempDir::new().unwrap();
    let db_path = temp_dir.path().join("test.db");
    let url = db_path.to_str().unwrap();

    let storage = open(url, None).await;
    init_schema(&storage).await;

    // Insert a provider row (mirrors ProviderService::create's columns).
    storage
        .execute(
            "INSERT INTO upstream_providers \
             (id, name, provider_type, client_id, encryption_key_id, discovery_url, scopes, is_active, mapping_config, created_at, updated_at) \
             VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)",
            &[
                DataValue::Text("google".into()),
                DataValue::Text("Google".into()),
                DataValue::Text("oidc".into()),
                DataValue::Text("client-abc".into()),
                DataValue::Text("default".into()),
                DataValue::Text("https://accounts.google.com/.well-known/openid-configuration".into()),
                DataValue::Text("[\"openid\",\"email\"]".into()),
                DataValue::Integer(1),
                DataValue::Text("{}".into()),
                DataValue::Integer(1000),
                DataValue::Integer(1000),
            ],
        )
        .await
        .unwrap();

    // Read it back.
    let rows = query_rows(&storage, "SELECT name, client_id, is_active FROM upstream_providers WHERE id = ?", &[DataValue::Text("google".into())]).await;
    assert_eq!(rows.len(), 1, "provider row should be readable");
    assert_eq!(rows[0].get::<String>(0).unwrap(), "Google");
    assert_eq!(rows[0].get::<String>(1).unwrap(), "client-abc");
    assert_eq!(rows[0].get::<i64>(2).unwrap(), 1);

    // A user + a link referencing the provider.
    storage
        .execute(
            "INSERT INTO users (id, email, created_at, updated_at) VALUES (?, ?, ?, ?)",
            &[
                DataValue::Text("user-1".into()),
                DataValue::Text("alice@example.com".into()),
                DataValue::Integer(1000),
                DataValue::Integer(1000),
            ],
        )
        .await
        .unwrap();
    storage
        .execute(
            "INSERT INTO user_provider_links \
             (id, user_id, provider_id, upstream_subject, upstream_email, created_at) \
             VALUES (?, ?, ?, ?, ?, ?)",
            &[
                DataValue::Text("link-1".into()),
                DataValue::Text("user-1".into()),
                DataValue::Text("google".into()),
                DataValue::Text("google-sub-123".into()),
                DataValue::Text("alice@example.com".into()),
                DataValue::Integer(1000),
            ],
        )
        .await
        .unwrap();

    // The unique (provider_id, upstream_subject) index must reject a duplicate.
    let dup = storage.execute(
        "INSERT INTO user_provider_links \
         (id, user_id, provider_id, upstream_subject, created_at) \
         VALUES (?, ?, ?, ?, ?)",
        &[
            DataValue::Text("link-2".into()),
            DataValue::Text("user-1".into()),
            DataValue::Text("google".into()),
            DataValue::Text("google-sub-123".into()),
            DataValue::Integer(1001),
        ],
    ).await;
    assert!(
        dup.is_err(),
        "duplicate (provider_id, upstream_subject) must be rejected by unique index"
    );

    // Lookup by (provider_id, upstream_subject) returns the linked user.
    let link_rows = query_rows(&storage, "SELECT user_id FROM user_provider_links WHERE provider_id = ? AND upstream_subject = ?", &[
                DataValue::Text("google".into()),
                DataValue::Text("google-sub-123".into()),
            ]).await;
    assert_eq!(link_rows.len(), 1);
    assert_eq!(link_rows[0].get::<String>(0).unwrap(), "user-1");
}
