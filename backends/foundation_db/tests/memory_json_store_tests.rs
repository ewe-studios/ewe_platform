//! MemoryJsonStore tests — the JSON key-value store over the async store
//! traits. Its futures are ready at once, so the tests drive them with
//! `futures_lite`'s `block_on`.

use foundation_db::traits::{
    BlobStore, KeyValueStore, QueryStore, RateLimiterStore,
};
use foundation_db::{MemoryJsonStore, StorageBackend, StorageError, StorageProvider};
use futures_lite::future::block_on;

// ============================================================================
// KeyValueStore tests
// ============================================================================

#[test]
fn test_memory_json_set_and_get() {
    block_on(async {
        let store = MemoryJsonStore::new();

        store.set::<()>("key", ()).await.unwrap();
        let result: Option<()> = store.get("key").await.unwrap();
        assert!(result.is_some());

        store.set("count", 42i64).await.unwrap();
        let count: Option<i64> = store.get("count").await.unwrap();
        assert_eq!(count, Some(42));

        store.set("name", "test").await.unwrap();
        let name: Option<String> = store.get("name").await.unwrap();
        assert_eq!(name, Some("test".to_string()));
    });
}

#[test]
fn test_memory_json_get_missing_key() {
    block_on(async {
        let store = MemoryJsonStore::new();
        let result: Option<String> = store.get("nonexistent").await.unwrap();
        assert_eq!(result, None);
    });
}

#[test]
fn test_memory_json_delete() {
    block_on(async {
        let store = MemoryJsonStore::new();
        store.set("key", "value").await.unwrap();
        store.delete("key").await.unwrap();
        let result: Option<String> = store.get("key").await.unwrap();
        assert_eq!(result, None);
    });
}

#[test]
fn test_memory_json_exists() {
    block_on(async {
        let store = MemoryJsonStore::new();
        let exists: bool = store.exists("key").await.unwrap();
        assert!(!exists);

        store.set("key", "value").await.unwrap();
        let exists: bool = store.exists("key").await.unwrap();
        assert!(exists);
    });
}

#[test]
fn test_memory_json_list_keys() {
    block_on(async {
        let store = MemoryJsonStore::new();
        store.set("user:1", "alice").await.unwrap();
        store.set("user:2", "bob").await.unwrap();
        store.set("admin:1", "charlie").await.unwrap();

        let all_keys = store.list_keys(None).await.unwrap().collect_all().await.unwrap();
        assert_eq!(all_keys.len(), 3);

        let user_keys = store
            .list_keys(Some("user:"))
            .await
            .unwrap()
            .collect_all()
            .await
            .unwrap();
        assert_eq!(user_keys.len(), 2);
    });
}

#[test]
fn test_memory_json_struct_value() {
    block_on(async {
        #[derive(serde::Serialize, serde::Deserialize, Clone, PartialEq, Debug)]
        struct User {
            name: String,
            age: u32,
        }

        let store = MemoryJsonStore::new();
        let user = User {
            name: "Alice".to_string(),
            age: 30,
        };
        store.set("user:1", user.clone()).await.unwrap();
        let retrieved: Option<User> = store.get("user:1").await.unwrap();
        assert_eq!(retrieved, Some(user));
    });
}

// ============================================================================
// RateLimiterStore tests
// ============================================================================

#[test]
fn test_memory_json_rate_limit_basic() {
    block_on(async {
        let store = MemoryJsonStore::new();

        let allowed: bool = store.check_rate_limit("api", 3, 60).await.unwrap();
        assert!(allowed);

        store.record_rate_limit("api", 60).await.unwrap();
        store.record_rate_limit("api", 60).await.unwrap();
        store.record_rate_limit("api", 60).await.unwrap();

        let allowed: bool = store.check_rate_limit("api", 3, 60).await.unwrap();
        assert!(!allowed);
    });
}

#[test]
fn test_memory_json_rate_limit_reset() {
    block_on(async {
        let store = MemoryJsonStore::new();
        store.record_rate_limit("api", 60).await.unwrap();
        store.reset_rate_limit("api").await.unwrap();
        let allowed: bool = store.check_rate_limit("api", 1, 60).await.unwrap();
        assert!(allowed);
    });
}

// ============================================================================
// BlobStore tests
// ============================================================================

#[test]
fn test_memory_json_blob_roundtrip() {
    block_on(async {
        let store = MemoryJsonStore::new();
        let data = b"hello world";
        store.put_blob("blob1", data).await.unwrap();

        let retrieved: Option<Vec<u8>> = store.get_blob("blob1").await.unwrap();
        assert_eq!(retrieved, Some(data.to_vec()));
    });
}

#[test]
fn test_memory_json_blob_exists() {
    block_on(async {
        let store = MemoryJsonStore::new();
        let exists: bool = store.blob_exists("blob1").await.unwrap();
        assert!(!exists);

        store.put_blob("blob1", b"data").await.unwrap();
        let exists: bool = store.blob_exists("blob1").await.unwrap();
        assert!(exists);
    });
}

#[test]
fn test_memory_json_delete_blob() {
    block_on(async {
        let store = MemoryJsonStore::new();
        store.put_blob("blob1", b"data").await.unwrap();
        store.delete_blob("blob1").await.unwrap();
        let exists: bool = store.blob_exists("blob1").await.unwrap();
        assert!(!exists);
    });
}

#[test]
fn test_memory_json_blob_binary_data() {
    block_on(async {
        let store = MemoryJsonStore::new();
        // Binary data with null bytes
        let data = vec![0x00, 0xFF, 0x42, 0x00, 0x01];
        store.put_blob("binary", &data).await.unwrap();
        let retrieved: Option<Vec<u8>> = store.get_blob("binary").await.unwrap();
        assert_eq!(retrieved, Some(data));
    });
}

/// A value that is not a blob is an error, not a missing blob.
#[test]
fn test_memory_json_get_blob_rejects_a_non_blob_value() {
    block_on(async {
        let store = MemoryJsonStore::new();
        store.set("not-a-blob", "plain text").await.unwrap();
        assert!(matches!(
            store.get_blob("not-a-blob").await,
            Err(StorageError::Deserialization(_))
        ));
    });
}

// ============================================================================
// QueryStore tests (not supported)
// ============================================================================

#[test]
fn test_memory_json_query_not_supported() {
    block_on(async {
        let store = MemoryJsonStore::new();
        let result = store.query("SELECT 1", &[]).await;
        assert!(result.is_err());
    });
}

/// Every query method reports the missing SQL engine.
#[test]
fn test_memory_json_async_query_not_supported() {
    let store = MemoryJsonStore::new();
    block_on(async {
        assert!(matches!(store.query("SELECT 1", &[]).await, Err(StorageError::Generic(_))));
        assert!(matches!(store.execute("DELETE FROM t", &[]).await, Err(StorageError::Generic(_))));
        assert!(matches!(store.execute_batch("SELECT 1").await, Err(StorageError::Generic(_))));
    });
}

// ============================================================================
// StorageProvider integration
// ============================================================================

#[test]
fn test_storage_provider_memory_json() {
    block_on(async {
        let provider = StorageProvider::memory_json();
        provider.set::<()>("key", ()).await.unwrap();
        let result: Option<()> = provider.get("key").await.unwrap();
        assert!(result.is_some());
    });
}

#[test]
fn test_storage_provider_memory_json_via_new() {
    block_on(async {
        let provider = StorageProvider::new(StorageBackend::MemoryJson).await.unwrap();
        provider.set("count", 99i64).await.unwrap();
        let count: Option<i64> = provider.get("count").await.unwrap();
        assert_eq!(count, Some(99));
    });
}
