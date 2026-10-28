//! In-memory storage backend with secure memory handling.
//!
//! Uses `std::sync::Mutex` for thread-safe access. It implements only the
//! `Async*` store traits, like every other store; its futures resolve at once
//! (there is no I/O).

use std::collections::HashMap;
use std::sync::{Arc, Mutex};

use serde::{de::DeserializeOwned, Serialize};
use zeroize::Zeroizing;

use super::unix_now_secs;
use crate::core::errors::{StorageError, StorageResult};
use crate::core::rate_limit::RateLimitEntry;
use crate::core::storage_provider::{
    BlobStore, KeyValueStore, AsyncListStream, QueryStore, AsyncQueryStream,
    RateLimiterStore, DataValue,
};

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


// ===========================================================================
// Async trait implementations — in-memory ops resolve immediately.
// ===========================================================================

#[async_trait::async_trait]
impl KeyValueStore for MemoryStorage {
    async fn get<V: DeserializeOwned + Send + 'static>(&self, key: &str) -> StorageResult<Option<V>> {
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

    async fn set<V: Serialize + Send + 'static>(&self, key: &str, value: V) -> StorageResult<()> {
        let bytes =
            serde_json::to_vec(&value).map_err(|e| StorageError::Serialization(e.to_string()))?;
        let mut data = self.data.lock()
            .map_err(|e| StorageError::Backend(format!("Mutex poisoned: {e}")))?;
        data.insert(key.to_string(), Zeroizing::new(bytes));
        Ok(())
    }

    async fn delete(&self, key: &str) -> StorageResult<()> {
        let mut data = self.data.lock()
            .map_err(|e| StorageError::Backend(format!("Mutex poisoned: {e}")))?;
        data.remove(key);
        Ok(())
    }

    async fn exists(&self, key: &str) -> StorageResult<bool> {
        let data = self.data.lock()
            .map_err(|e| StorageError::Backend(format!("Mutex poisoned: {e}")))?;
        Ok(data.contains_key(key))
    }

    async fn list_keys(&self, prefix: Option<&str>) -> StorageResult<AsyncListStream> {
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
impl BlobStore for MemoryStorage {
    async fn put_blob(&self, key: &str, data: &[u8]) -> StorageResult<()> {
        let mut storage = self.data.lock()
            .map_err(|e| StorageError::Backend(format!("Mutex poisoned: {e}")))?;
        storage.insert(key.to_string(), Zeroizing::new(data.to_vec()));
        Ok(())
    }

    async fn get_blob(&self, key: &str) -> StorageResult<Option<Vec<u8>>> {
        let data = self.data.lock()
            .map_err(|e| StorageError::Backend(format!("Mutex poisoned: {e}")))?;
        Ok(data.get(key).cloned().map(|z| z.to_vec()))
    }

    async fn delete_blob(&self, key: &str) -> StorageResult<()> {
        let mut data = self.data.lock()
            .map_err(|e| StorageError::Backend(format!("Mutex poisoned: {e}")))?;
        data.remove(key);
        Ok(())
    }

    async fn blob_exists(&self, key: &str) -> StorageResult<bool> {
        let data = self.data.lock()
            .map_err(|e| StorageError::Backend(format!("Mutex poisoned: {e}")))?;
        Ok(data.contains_key(key))
    }
}

#[async_trait::async_trait]
impl RateLimiterStore for MemoryStorage {
    async fn check_rate_limit(
        &self,
        key: &str,
        max_count: u32,
        window_seconds: u64,
    ) -> StorageResult<bool> {
        let now = unix_now_secs()?;
        let rate_key = format!("_rate_limit:{key}");
        let data = self.data.lock()
            .map_err(|e| StorageError::Backend(format!("Mutex poisoned: {e}")))?;
        match data.get(&rate_key) {
            Some(bytes) => {
                let entry: RateLimitEntry = serde_json::from_slice(bytes)
                    .map_err(|e| StorageError::Serialization(e.to_string()))?;
                Ok(entry.allows(max_count, now, window_seconds))
            }
            None => Ok(true),
        }
    }

    async fn record_rate_limit(&self, key: &str, window_seconds: u64) -> StorageResult<u32> {
        let now = unix_now_secs()?;
        let rate_key = format!("_rate_limit:{key}");
        let mut data = self.data.lock()
            .map_err(|e| StorageError::Backend(format!("Mutex poisoned: {e}")))?;
        let previous = data
            .get(&rate_key)
            .map(|bytes| serde_json::from_slice::<RateLimitEntry>(bytes))
            .transpose()
            .map_err(|e| StorageError::Serialization(e.to_string()))?;
        let entry = RateLimitEntry::record(previous, now, window_seconds);
        data.insert(rate_key, Zeroizing::new(
            serde_json::to_vec(&entry).map_err(|e| StorageError::Serialization(e.to_string()))?,
        ));
        Ok(entry.count)
    }

    async fn reset_rate_limit(&self, key: &str) -> StorageResult<()> {
        let rate_key = format!("_rate_limit:{key}");
        let mut data = self.data.lock()
            .map_err(|e| StorageError::Backend(format!("Mutex poisoned: {e}")))?;
        data.remove(&rate_key);
        Ok(())
    }
}

/// `MemoryStorage` has no SQL engine, so every query method fails with
/// `StorageError::Generic`.
#[async_trait::async_trait]
impl QueryStore for MemoryStorage {
    async fn query(
        &self,
        _sql: &str,
        _params: &[DataValue],
    ) -> StorageResult<AsyncQueryStream> {
        Err(StorageError::Generic(
            "QueryStore not supported for MemoryStorage".to_string(),
        ))
    }

    async fn execute(&self, _sql: &str, _params: &[DataValue]) -> StorageResult<u64> {
        Err(StorageError::Generic(
            "QueryStore not supported for MemoryStorage".to_string(),
        ))
    }

    async fn execute_batch(&self, _sql: &str) -> StorageResult<()> {
        Err(StorageError::Generic(
            "QueryStore not supported for MemoryStorage".to_string(),
        ))
    }
}
