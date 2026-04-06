//! In-memory storage backend with secure memory handling.
//!
//! Uses `std::sync::Mutex` for thread-safe access. All operations
//! are synchronous and return `StorageResult<T>` directly.

use std::collections::HashMap;
use std::sync::{Arc, Mutex};

use serde::{de::DeserializeOwned, Serialize};
use zeroize::Zeroizing;

use crate::errors::{StorageError, StorageResult};
use crate::storage_provider::{
    DataValue, KeyValueStore, QueryStore, RateLimiterStore, SqlRow, StorageItemStream,
};
use foundation_core::valtron::Stream;

/// In-memory storage with zeroizing support for sensitive data.
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
    fn get<'a, V: DeserializeOwned + Send + 'static>(
        &'a self,
        key: &str,
    ) -> StorageResult<StorageItemStream<'a, Option<V>>> {
        let data = self
            .data
            .lock()
            .map_err(|e| StorageError::Backend(format!("Mutex poisoned: {e}")))?;

        let result = match data.get(key) {
            Some(bytes) => {
                let value: V = serde_json::from_slice(bytes)
                    .map_err(|e| StorageError::Serialization(e.to_string()))?;
                Some(value)
            }
            None => None,
        };
        Ok(Box::new(std::iter::once(Stream::Next(Ok(result)))))
    }

    fn set<V: Serialize>(&self, key: &str, value: V) -> StorageResult<StorageItemStream<'_, ()>> {
        let bytes =
            serde_json::to_vec(&value).map_err(|e| StorageError::Serialization(e.to_string()))?;

        let mut data = self
            .data
            .lock()
            .map_err(|e| StorageError::Backend(format!("Mutex poisoned: {e}")))?;

        data.insert(key.to_string(), Zeroizing::new(bytes));
        Ok(Box::new(std::iter::once(Stream::Next(Ok(())))))
    }

    fn delete(&self, key: &str) -> StorageResult<StorageItemStream<'_, ()>> {
        let mut data = self
            .data
            .lock()
            .map_err(|e| StorageError::Backend(format!("Mutex poisoned: {e}")))?;
        data.remove(key);
        Ok(Box::new(std::iter::once(Stream::Next(Ok(())))))
    }

    fn exists(&self, key: &str) -> StorageResult<StorageItemStream<'_, bool>> {
        let data = self
            .data
            .lock()
            .map_err(|e| StorageError::Backend(format!("Mutex poisoned: {e}")))?;
        Ok(Box::new(std::iter::once(Stream::Next(Ok(
            data.contains_key(key)
        )))))
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

    fn execute(
        &self,
        _sql: &str,
        _params: &[DataValue],
    ) -> StorageResult<StorageItemStream<'_, u64>> {
        Err(StorageError::Generic(
            "QueryStore not supported for MemoryStorage".to_string(),
        ))
    }

    fn execute_batch(&self, _sql: &str) -> StorageResult<StorageItemStream<'_, ()>> {
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
    ) -> StorageResult<StorageItemStream<'_, bool>> {
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
        Ok(Box::new(std::iter::once(Stream::Next(Ok(allowed)))))
    }

    fn record_rate_limit(&self, key: &str) -> StorageResult<StorageItemStream<'_, u32>> {
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

        Ok(Box::new(std::iter::once(Stream::Next(Ok(new_count)))))
    }

    fn reset_rate_limit(&self, key: &str) -> StorageResult<StorageItemStream<'_, ()>> {
        let rate_key = format!("_rate_limit:{key}");
        let mut data = self
            .data
            .lock()
            .map_err(|e| StorageError::Backend(format!("Mutex poisoned: {e}")))?;
        data.remove(&rate_key);
        Ok(Box::new(std::iter::once(Stream::Next(Ok(())))))
    }
}
