//! MemoryJsonStore tests — JSON key-value store with full trait support.

use foundation_core::valtron::Stream;
use foundation_db::{
    BlobStore, KeyValueStore, MemoryJsonStore, QueryStore, RateLimiterStore, StorageBackend,
    StorageError, StorageItemStream, StorageProvider,
};


fn collect_many<T>(stream: StorageItemStream<'_, T>) -> Vec<T> {
    stream
        .filter_map(|item| match item {
            Stream::Next(Ok(v)) => Some(v),
            _ => None,
        })
        .collect()
}

// ============================================================================
// KeyValueStore tests
// ============================================================================

#[test]
fn test_memory_json_set_and_get() {
    let store = MemoryJsonStore::new();

    let _ = store.set::<()>("key", ())?;
    let result: Option<()> = store.get("key").unwrap();
    assert!(result.is_some());

    let _ = store.set("count", 42i64).unwrap();
    let count: Option<i64> = store.get("count").unwrap();
    assert_eq!(count, Some(42));

    let _ = store.set("name", "test").unwrap();
    let name: Option<String> = store.get("name").unwrap();
    assert_eq!(name, Some("test".to_string()));
}

#[test]
fn test_memory_json_get_missing_key() {
    let store = MemoryJsonStore::new();
    let result: Option<String> = store.get("nonexistent").unwrap();
    assert_eq!(result, None);
}

#[test]
fn test_memory_json_delete() {
    let store = MemoryJsonStore::new();
    let _ = store.set("key", "value").unwrap();
    store.delete("key").unwrap();
    let result: Option<String> = store.get("key").unwrap();
    assert_eq!(result, None);
}

#[test]
fn test_memory_json_exists() {
    let store = MemoryJsonStore::new();
    let exists: bool = store.exists("key").unwrap();
    assert!(!exists);

    let _ = store.set("key", "value").unwrap();
    let exists: bool = store.exists("key").unwrap();
    assert!(exists);
}

#[test]
fn test_memory_json_list_keys() {
    let store = MemoryJsonStore::new();
    let _ = store.set("user:1", "alice").unwrap();
    let _ = store.set("user:2", "bob").unwrap();
    let _ = store.set("admin:1", "charlie").unwrap();

    let all_keys = collect_many(store.list_keys(None).unwrap());
    assert_eq!(all_keys.len(), 3);

    let user_keys = collect_many(store.list_keys(Some("user:")).unwrap());
    assert_eq!(user_keys.len(), 2);
}

#[test]
fn test_memory_json_struct_value() {
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
    let _ = store.set("user:1", user.clone()).unwrap();
    let retrieved: Option<User> = store.get("user:1").unwrap();
    assert_eq!(retrieved, Some(user));
}

// ============================================================================
// RateLimiterStore tests
// ============================================================================

#[test]
fn test_memory_json_rate_limit_basic() {
    let store = MemoryJsonStore::new();

    let allowed: bool = store.check_rate_limit("api", 3, 60).unwrap();
    assert!(allowed);

    store.record_rate_limit("api").unwrap();
    store.record_rate_limit("api").unwrap();
    store.record_rate_limit("api").unwrap();

    let allowed: bool = store.check_rate_limit("api", 3, 60).unwrap();
    assert!(!allowed);
}

#[test]
fn test_memory_json_rate_limit_reset() {
    let store = MemoryJsonStore::new();
    store.record_rate_limit("api").unwrap();
    store.reset_rate_limit("api").unwrap();
    let allowed: bool = store.check_rate_limit("api", 1, 60).unwrap();
    assert!(allowed);
}

// ============================================================================
// BlobStore tests
// ============================================================================

#[test]
fn test_memory_json_blob_roundtrip() {
    let store = MemoryJsonStore::new();
    let data = b"hello world";
    store.put_blob("blob1", data).unwrap();

    let retrieved: Option<Vec<u8>> = store.get_blob("blob1").unwrap();
    assert_eq!(retrieved, Some(data.to_vec()));
}

#[test]
fn test_memory_json_blob_exists() {
    let store = MemoryJsonStore::new();
    let exists: bool = store.blob_exists("blob1").unwrap();
    assert!(!exists);

    store.put_blob("blob1", b"data").unwrap();
    let exists: bool = store.blob_exists("blob1").unwrap();
    assert!(exists);
}

#[test]
fn test_memory_json_delete_blob() {
    let store = MemoryJsonStore::new();
    store.put_blob("blob1", b"data").unwrap();
    store.delete_blob("blob1").unwrap();
    let exists: bool = store.blob_exists("blob1").unwrap();
    assert!(!exists);
}

#[test]
fn test_memory_json_blob_binary_data() {
    let store = MemoryJsonStore::new();
    // Binary data with null bytes
    let data = vec![0x00, 0xFF, 0x42, 0x00, 0x01];
    store.put_blob("binary", &data).unwrap();
    let retrieved: Option<Vec<u8>> = store.get_blob("binary").unwrap();
    assert_eq!(retrieved, Some(data));
}

// ============================================================================
// QueryStore tests (not supported)
// ============================================================================

#[test]
fn test_memory_json_query_not_supported() {
    let store = MemoryJsonStore::new();
    let result = store.query("SELECT 1", &[]);
    assert!(result.is_err());
}

// ============================================================================
// StorageProvider integration
// ============================================================================

#[test]
fn test_storage_provider_memory_json() {
    let provider = StorageProvider::memory_json();
    let _ = provider.set::<()>("key", ())?;
    let result: Option<()> = provider.get("key").unwrap();
    assert!(result.is_some());
}

#[test]
fn test_storage_provider_memory_json_via_new() {
    let provider = StorageProvider::new(StorageBackend::MemoryJson).unwrap();
    let _ = provider.set("count", 99i64)?;
    let count: Option<i64> = provider.get("count").unwrap();
    assert_eq!(count, Some(99));
}
