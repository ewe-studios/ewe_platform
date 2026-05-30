//! MemoryJsonStore tests — JSON key-value store with full trait support.

use foundation_core::valtron::Stream;
use foundation_db::{
    BlobStore, KeyValueStore, MemoryJsonStore, QueryStore, RateLimiterStore, StorageBackend,
    StorageError, StorageItemStream, StorageProvider,
};

fn collect_one<T>(mut stream: StorageItemStream<'_, T>) -> Result<T, StorageError> {
    loop {
        match stream.next() {
            Some(Stream::Next(Ok(v))) => return Ok(v),
            Some(Stream::Next(Err(e))) => return Err(e),
            Some(
                Stream::Init
                | Stream::Ignore
                | Stream::Delayed(_)
                | Stream::Pending(_)
                | Stream::Wait,
            ) => continue,
            Some(Stream::Spread(items)) => {
                for item in items {
                    if let foundation_core::valtron::StreamSpread::Done(Ok(v)) = item {
                        return Ok(v);
                    }
                    if let foundation_core::valtron::StreamSpread::Done(Err(e)) = item {
                        return Err(e);
                    }
                }
                continue;
            }
            None => panic!("stream ended without value"),
        }
    }
}

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

    let _ = collect_one(store.set::<()>("key", ()).unwrap()).unwrap();
    let result: Option<()> = collect_one(store.get("key").unwrap()).unwrap();
    assert!(result.is_some());

    let _ = collect_one(store.set("count", 42i64).unwrap()).unwrap();
    let count: Option<i64> = collect_one(store.get("count").unwrap()).unwrap();
    assert_eq!(count, Some(42));

    let _ = collect_one(store.set("name", "test").unwrap()).unwrap();
    let name: Option<String> = collect_one(store.get("name").unwrap()).unwrap();
    assert_eq!(name, Some("test".to_string()));
}

#[test]
fn test_memory_json_get_missing_key() {
    let store = MemoryJsonStore::new();
    let result: Option<String> = collect_one(store.get("nonexistent").unwrap()).unwrap();
    assert_eq!(result, None);
}

#[test]
fn test_memory_json_delete() {
    let store = MemoryJsonStore::new();
    let _ = collect_one(store.set("key", "value").unwrap()).unwrap();
    collect_one(store.delete("key").unwrap()).unwrap();
    let result: Option<String> = collect_one(store.get("key").unwrap()).unwrap();
    assert_eq!(result, None);
}

#[test]
fn test_memory_json_exists() {
    let store = MemoryJsonStore::new();
    let exists: bool = collect_one(store.exists("key").unwrap()).unwrap();
    assert!(!exists);

    let _ = collect_one(store.set("key", "value").unwrap()).unwrap();
    let exists: bool = collect_one(store.exists("key").unwrap()).unwrap();
    assert!(exists);
}

#[test]
fn test_memory_json_list_keys() {
    let store = MemoryJsonStore::new();
    let _ = collect_one(store.set("user:1", "alice").unwrap()).unwrap();
    let _ = collect_one(store.set("user:2", "bob").unwrap()).unwrap();
    let _ = collect_one(store.set("admin:1", "charlie").unwrap()).unwrap();

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
    let _ = collect_one(store.set("user:1", user.clone()).unwrap()).unwrap();
    let retrieved: Option<User> = collect_one(store.get("user:1").unwrap()).unwrap();
    assert_eq!(retrieved, Some(user));
}

// ============================================================================
// RateLimiterStore tests
// ============================================================================

#[test]
fn test_memory_json_rate_limit_basic() {
    let store = MemoryJsonStore::new();

    let allowed: bool = collect_one(store.check_rate_limit("api", 3, 60).unwrap()).unwrap();
    assert!(allowed);

    collect_one(store.record_rate_limit("api").unwrap()).unwrap();
    collect_one(store.record_rate_limit("api").unwrap()).unwrap();
    collect_one(store.record_rate_limit("api").unwrap()).unwrap();

    let allowed: bool = collect_one(store.check_rate_limit("api", 3, 60).unwrap()).unwrap();
    assert!(!allowed);
}

#[test]
fn test_memory_json_rate_limit_reset() {
    let store = MemoryJsonStore::new();
    collect_one(store.record_rate_limit("api").unwrap()).unwrap();
    collect_one(store.reset_rate_limit("api").unwrap()).unwrap();
    let allowed: bool = collect_one(store.check_rate_limit("api", 1, 60).unwrap()).unwrap();
    assert!(allowed);
}

// ============================================================================
// BlobStore tests
// ============================================================================

#[test]
fn test_memory_json_blob_roundtrip() {
    let store = MemoryJsonStore::new();
    let data = b"hello world";
    collect_one(store.put_blob("blob1", data).unwrap()).unwrap();

    let retrieved: Option<Vec<u8>> = collect_one(store.get_blob("blob1").unwrap()).unwrap();
    assert_eq!(retrieved, Some(data.to_vec()));
}

#[test]
fn test_memory_json_blob_exists() {
    let store = MemoryJsonStore::new();
    let exists: bool = collect_one(store.blob_exists("blob1").unwrap()).unwrap();
    assert!(!exists);

    collect_one(store.put_blob("blob1", b"data").unwrap()).unwrap();
    let exists: bool = collect_one(store.blob_exists("blob1").unwrap()).unwrap();
    assert!(exists);
}

#[test]
fn test_memory_json_delete_blob() {
    let store = MemoryJsonStore::new();
    collect_one(store.put_blob("blob1", b"data").unwrap()).unwrap();
    collect_one(store.delete_blob("blob1").unwrap()).unwrap();
    let exists: bool = collect_one(store.blob_exists("blob1").unwrap()).unwrap();
    assert!(!exists);
}

#[test]
fn test_memory_json_blob_binary_data() {
    let store = MemoryJsonStore::new();
    // Binary data with null bytes
    let data = vec![0x00, 0xFF, 0x42, 0x00, 0x01];
    collect_one(store.put_blob("binary", &data).unwrap()).unwrap();
    let retrieved: Option<Vec<u8>> = collect_one(store.get_blob("binary").unwrap()).unwrap();
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
    let _ = collect_one(provider.set::<()>("key", ()).unwrap()).unwrap();
    let result: Option<()> = collect_one(provider.get("key").unwrap()).unwrap();
    assert!(result.is_some());
}

#[test]
fn test_storage_provider_memory_json_via_new() {
    let provider = StorageProvider::new(StorageBackend::MemoryJson).unwrap();
    let _ = collect_one(provider.set("count", 99i64).unwrap()).unwrap();
    let count: Option<i64> = collect_one(provider.get("count").unwrap()).unwrap();
    assert_eq!(count, Some(99));
}
