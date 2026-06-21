//! In-memory storage backend with secure memory handling.
//!
//! Uses `std::sync::Mutex` for thread-safe access. All operations
//! are synchronous and return `StorageResult<T>` directly.

use std::collections::HashMap;
use std::sync::{Arc, Mutex};

use serde::{de::DeserializeOwned, Serialize};
use zeroize::Zeroizing;

use crate::core::errors::{StorageError, StorageResult};
use crate::core::storage_provider::{
    AsyncBlobStore, AsyncKeyValueStore, AsyncListStream, AsyncRateLimiterStore,
    BlobStore, DataValue, KeyValueStore, QueryStore, RateLimiterStore, SqlRow, StorageItemStream,
};
use foundation_core::valtron::Stream;

/// In-memory storage with zeroizing support for sensitive data.
#[derive(Clone)]
pub struct MemoryStorage {
    data: Arc<Mutex<HashMap<String, Zeroizing<Vec<u8>>>>>,
}

impl MemoryStorage {
    /// Create a new in-memory storage instance.
    #[must_use]
    pub fn new() -> Self {
        Self {
            data: Arc::new(Mutex::new(HashMap::new())),
        }
    }
}

impl Default for MemoryStorage {
    fn default() -> Self {
        Self::new()
    }
}

impl KeyValueStore for MemoryStorage {
    fn get<V: DeserializeOwned + Send + 'static>(&self, key: &str) -> StorageResult<Option<V>> {
        let data = self
            .data
            .lock()
            .map_err(|e| StorageError::Backend(format!("Mutex poisoned: {e}")))?;

        match data.get(key) {
            Some(bytes) => {
                let value: V = serde_json::from_slice(bytes)
                    .map_err(|e| StorageError::Serialization(e.to_string()))?;
                Ok(Some(value))
            }
            None => Ok(None),
        }
    }

    fn set<V: Serialize>(&self, key: &str, value: V) -> StorageResult<()> {
        let bytes =
            serde_json::to_vec(&value).map_err(|e| StorageError::Serialization(e.to_string()))?;

        let mut data = self
            .data
            .lock()
            .map_err(|e| StorageError::Backend(format!("Mutex poisoned: {e}")))?;

        data.insert(key.to_string(), Zeroizing::new(bytes));
        Ok(())
    }

    fn delete(&self, key: &str) -> StorageResult<()> {
        let mut data = self
            .data
            .lock()
            .map_err(|e| StorageError::Backend(format!("Mutex poisoned: {e}")))?;
        data.remove(key);
        Ok(())
    }

    fn exists(&self, key: &str) -> StorageResult<bool> {
        let data = self
            .data
            .lock()
            .map_err(|e| StorageError::Backend(format!("Mutex poisoned: {e}")))?;
        Ok(data.contains_key(key))
    }

    fn list_keys(&self, prefix: Option<&str>) -> StorageResult<StorageItemStream<'_, String>> {
        let data = self
            .data
            .lock()
            .map_err(|e| StorageError::Backend(format!("Mutex poisoned: {e}")))?;

        let keys: Vec<String> = data
            .keys()
            .filter(|k| prefix.is_none_or(|p| k.starts_with(p)))
            .cloned()
            .collect();

        Ok(Box::new(keys.into_iter().map(|key| Stream::Next(Ok(key)))))
    }
}

/// In-memory implementation of [`QueryStore`] for testing.
/// Note: This is a simplified implementation for testing purposes.
impl QueryStore for MemoryStorage {
    fn query(
        &self,
        _sql: &str,
        _params: &[DataValue],
    ) -> StorageResult<StorageItemStream<'_, SqlRow>> {
        Err(StorageError::Generic(
            "QueryStore not supported for MemoryStorage".to_string(),
        ))
    }

    fn execute(&self, _sql: &str, _params: &[DataValue]) -> StorageResult<u64> {
        Err(StorageError::Generic(
            "QueryStore not supported for MemoryStorage".to_string(),
        ))
    }

    fn execute_batch(&self, _sql: &str) -> StorageResult<()> {
        Err(StorageError::Generic(
            "QueryStore not supported for MemoryStorage".to_string(),
        ))
    }
}

/// Rate limit entry for in-memory storage.
#[derive(Serialize, serde::Deserialize)]
struct RateLimitEntry {
    count: u32,
    window_start: u64,
}

/// In-memory implementation of [`RateLimiterStore`].
impl RateLimiterStore for MemoryStorage {
    fn check_rate_limit(
        &self,
        key: &str,
        max_count: u32,
        window_seconds: u64,
    ) -> StorageResult<bool> {
        let now = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_secs();

        let data = self
            .data
            .lock()
            .map_err(|e| StorageError::Backend(format!("Mutex poisoned: {e}")))?;

        let rate_key = format!("_rate_limit:{key}");
        let allowed = match data.get(&rate_key) {
            Some(bytes) => {
                let entry: RateLimitEntry = serde_json::from_slice(bytes)
                    .map_err(|e| StorageError::Serialization(e.to_string()))?;

                if entry.window_start < now - window_seconds {
                    true
                } else {
                    entry.count < max_count
                }
            }
            None => true,
        };
        Ok(allowed)
    }

    fn record_rate_limit(&self, key: &str) -> StorageResult<u32> {
        let now = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_secs();

        let rate_key = format!("_rate_limit:{key}");
        let mut data = self
            .data
            .lock()
            .map_err(|e| StorageError::Backend(format!("Mutex poisoned: {e}")))?;

        let new_count = if let Some(bytes) = data.get(&rate_key) {
            let mut entry: RateLimitEntry = serde_json::from_slice(bytes)
                .map_err(|e| StorageError::Serialization(e.to_string()))?;
            entry.count += 1;
            entry.window_start = now;
            data.insert(
                rate_key.clone(),
                Zeroizing::new(
                    serde_json::to_vec(&entry)
                        .map_err(|e| StorageError::Serialization(e.to_string()))?,
                ),
            );
            entry.count
        } else {
            let entry = RateLimitEntry {
                count: 1,
                window_start: now,
            };
            data.insert(
                rate_key.clone(),
                Zeroizing::new(
                    serde_json::to_vec(&entry)
                        .map_err(|e| StorageError::Serialization(e.to_string()))?,
                ),
            );
            1
        };

        Ok(new_count)
    }

    fn reset_rate_limit(&self, key: &str) -> StorageResult<()> {
        let rate_key = format!("_rate_limit:{key}");
        let mut data = self
            .data
            .lock()
            .map_err(|e| StorageError::Backend(format!("Mutex poisoned: {e}")))?;
        data.remove(&rate_key);
        Ok(())
    }
}

impl BlobStore for MemoryStorage {
    fn put_blob(&self, key: &str, data: &[u8]) -> StorageResult<()> {
        let mut storage = self
            .data
            .lock()
            .map_err(|e| StorageError::Backend(format!("Mutex poisoned: {e}")))?;
        storage.insert(key.to_string(), Zeroizing::new(data.to_vec()));
        Ok(())
    }

    fn get_blob(&self, key: &str) -> StorageResult<Option<Vec<u8>>> {
        let data = self
            .data
            .lock()
            .map_err(|e| StorageError::Backend(format!("Mutex poisoned: {e}")))?;
        Ok(data.get(key).cloned().map(|z| z.to_vec()))
    }

    fn delete_blob(&self, key: &str) -> StorageResult<()> {
        let mut data = self
            .data
            .lock()
            .map_err(|e| StorageError::Backend(format!("Mutex poisoned: {e}")))?;
        data.remove(key);
        Ok(())
    }

    fn blob_exists(&self, key: &str) -> StorageResult<bool> {
        let data = self
            .data
            .lock()
            .map_err(|e| StorageError::Backend(format!("Mutex poisoned: {e}")))?;
        Ok(data.contains_key(key))
    }
}

// ===========================================================================
// Async trait implementations — in-memory ops resolve immediately.
// ===========================================================================

#[async_trait::async_trait]
impl AsyncKeyValueStore for MemoryStorage {
    async fn get_async<V: DeserializeOwned + Send + 'static>(&self, key: &str) -> StorageResult<Option<V>> {
        let bytes = self.data.lock()
            .map_err(|e| StorageError::Backend(format!("Mutex poisoned: {e}")))?
            .get(key).cloned().map(|z| z.to_vec());
        match bytes {
            Some(bytes) => {
                let value: V = serde_json::from_slice(&bytes)
                    .map_err(|e| StorageError::Serialization(e.to_string()))?;
                Ok(Some(value))
            }
            None => Ok(None),
        }
    }

    async fn set_async<V: Serialize + Send + 'static>(&self, key: &str, value: V) -> StorageResult<()> {
        let bytes =
            serde_json::to_vec(&value).map_err(|e| StorageError::Serialization(e.to_string()))?;
        let mut data = self.data.lock()
            .map_err(|e| StorageError::Backend(format!("Mutex poisoned: {e}")))?;
        data.insert(key.to_string(), Zeroizing::new(bytes));
        Ok(())
    }

    async fn delete_async(&self, key: &str) -> StorageResult<()> {
        let mut data = self.data.lock()
            .map_err(|e| StorageError::Backend(format!("Mutex poisoned: {e}")))?;
        data.remove(key);
        Ok(())
    }

    async fn exists_async(&self, key: &str) -> StorageResult<bool> {
        let data = self.data.lock()
            .map_err(|e| StorageError::Backend(format!("Mutex poisoned: {e}")))?;
        Ok(data.contains_key(key))
    }

    async fn list_keys_async(&self, prefix: Option<&str>) -> StorageResult<AsyncListStream> {
        let data = self.data.lock()
            .map_err(|e| StorageError::Backend(format!("Mutex poisoned: {e}")))?;
        let keys: Vec<String> = data.keys()
            .filter(|k| prefix.is_none_or(|p| k.starts_with(p)))
            .cloned()
            .collect();
        Ok(AsyncListStream::new(futures_lite::stream::iter(
            keys.into_iter().map(Ok)
        )))
    }
}

#[async_trait::async_trait]
impl AsyncBlobStore for MemoryStorage {
    async fn put_blob_async(&self, key: &str, data: &[u8]) -> StorageResult<()> {
        let mut storage = self.data.lock()
            .map_err(|e| StorageError::Backend(format!("Mutex poisoned: {e}")))?;
        storage.insert(key.to_string(), Zeroizing::new(data.to_vec()));
        Ok(())
    }

    async fn get_blob_async(&self, key: &str) -> StorageResult<Option<Vec<u8>>> {
        let data = self.data.lock()
            .map_err(|e| StorageError::Backend(format!("Mutex poisoned: {e}")))?;
        Ok(data.get(key).cloned().map(|z| z.to_vec()))
    }

    async fn delete_blob_async(&self, key: &str) -> StorageResult<()> {
        let mut data = self.data.lock()
            .map_err(|e| StorageError::Backend(format!("Mutex poisoned: {e}")))?;
        data.remove(key);
        Ok(())
    }

    async fn blob_exists_async(&self, key: &str) -> StorageResult<bool> {
        let data = self.data.lock()
            .map_err(|e| StorageError::Backend(format!("Mutex poisoned: {e}")))?;
        Ok(data.contains_key(key))
    }
}

#[async_trait::async_trait]
impl AsyncRateLimiterStore for MemoryStorage {
    async fn check_rate_limit_async(
        &self,
        key: &str,
        max_count: u32,
        window_seconds: u64,
    ) -> StorageResult<bool> {
        let now = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_secs();
        let rate_key = format!("_rate_limit:{key}");
        let data = self.data.lock()
            .map_err(|e| StorageError::Backend(format!("Mutex poisoned: {e}")))?;
        let allowed = match data.get(&rate_key) {
            Some(bytes) => {
                let entry: RateLimitEntry = serde_json::from_slice(bytes)
                    .map_err(|e| StorageError::Serialization(e.to_string()))?;
                if entry.window_start < now - window_seconds {
                    true
                } else {
                    entry.count < max_count
                }
            }
            None => true,
        };
        Ok(allowed)
    }

    async fn record_rate_limit_async(&self, key: &str) -> StorageResult<u32> {
        let now = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_secs();
        let rate_key = format!("_rate_limit:{key}");
        let mut data = self.data.lock()
            .map_err(|e| StorageError::Backend(format!("Mutex poisoned: {e}")))?;
        let new_count = if let Some(bytes) = data.get(&rate_key) {
            let mut entry: RateLimitEntry = serde_json::from_slice(bytes)
                .map_err(|e| StorageError::Serialization(e.to_string()))?;
            entry.count += 1;
            entry.window_start = now;
            data.insert(rate_key.clone(), Zeroizing::new(
                serde_json::to_vec(&entry).map_err(|e| StorageError::Serialization(e.to_string()))?,
            ));
            entry.count
        } else {
            let entry = RateLimitEntry { count: 1, window_start: now };
            data.insert(rate_key.clone(), Zeroizing::new(
                serde_json::to_vec(&entry).map_err(|e| StorageError::Serialization(e.to_string()))?,
            ));
            1
        };
        Ok(new_count)
    }

    async fn reset_rate_limit_async(&self, key: &str) -> StorageResult<()> {
        let rate_key = format!("_rate_limit:{key}");
        let mut data = self.data.lock()
            .map_err(|e| StorageError::Backend(format!("Mutex poisoned: {e}")))?;
        data.remove(&rate_key);
        Ok(())
    }
}
