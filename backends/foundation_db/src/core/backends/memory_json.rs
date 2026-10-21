//! In-memory JSON key-value store — works on both native and wasm.
//!
//! Unlike [`MemoryStorage`](super::memory::MemoryStorage), this stores
//! values as JSON strings rather than zeroizing byte buffers, making it
//! suitable for non-sensitive data and easier to inspect/debug.

use std::collections::HashMap;
use std::sync::{Arc, Mutex, MutexGuard};

use serde::{de::DeserializeOwned, Serialize};

use super::unix_now_secs;
use crate::core::errors::{StorageError, StorageResult};
use crate::core::storage_provider::{
    AsyncBlobStore, AsyncKeyValueStore, AsyncListStream, AsyncQueryStore, AsyncQueryStream,
    AsyncRateLimiterStore, DataValue,
};

/// In-memory JSON key-value store.
///
/// Values are serialized as JSON strings in a `HashMap<String, String>`.
pub struct MemoryJsonStore {
    data: Arc<Mutex<HashMap<String, String>>>,
}

impl MemoryJsonStore {
    /// Create a new in-memory JSON store.
    #[must_use]
    pub fn new() -> Self {
        Self {
            data: Arc::new(Mutex::new(HashMap::new())),
        }
    }

    fn lock(&self) -> Result<MutexGuard<'_, HashMap<String, String>>, StorageError> {
        self.data
            .lock()
            .map_err(|e| StorageError::Backend(format!("Mutex poisoned: {e}")))
    }
}

impl Default for MemoryJsonStore {
    fn default() -> Self {
        Self::new()
    }
}

// ===========================================================================
// Async trait implementations — in-memory ops resolve immediately.
// ===========================================================================

#[async_trait::async_trait]
impl AsyncKeyValueStore for MemoryJsonStore {
    async fn get_async<V: DeserializeOwned + Send + 'static>(&self, key: &str) -> StorageResult<Option<V>> {
        let data = self.lock()?;
        match data.get(key) {
            Some(json) => {
                let value: V = serde_json::from_str(json)
                    .map_err(|e| StorageError::Serialization(e.to_string()))?;
                Ok(Some(value))
            }
            None => Ok(None),
        }
    }

    async fn set_async<V: Serialize + Send + 'static>(&self, key: &str, value: V) -> StorageResult<()> {
        let json = serde_json::to_string(&value)
            .map_err(|e| StorageError::Serialization(e.to_string()))?;
        let mut data = self.lock()?;
        data.insert(key.to_string(), json);
        Ok(())
    }

    async fn delete_async(&self, key: &str) -> StorageResult<()> {
        let mut data = self.lock()?;
        data.remove(key);
        Ok(())
    }

    async fn exists_async(&self, key: &str) -> StorageResult<bool> {
        let data = self.lock()?;
        Ok(data.contains_key(key))
    }

    async fn list_keys_async(&self, prefix: Option<&str>) -> StorageResult<AsyncListStream> {
        let data = self.lock()?;
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
impl AsyncBlobStore for MemoryJsonStore {
    async fn put_blob_async(&self, key: &str, data: &[u8]) -> StorageResult<()> {
        let encoded = base64::Engine::encode(&base64::engine::general_purpose::STANDARD, data);
        let json = serde_json::json!({ "type": "blob", "data": encoded }).to_string();
        let mut store = self.lock()?;
        store.insert(key.to_string(), json);
        Ok(())
    }

    async fn get_blob_async(&self, key: &str) -> StorageResult<Option<Vec<u8>>> {
        let data = self.lock()?;
        let Some(json) = data.get(key) else {
            return Ok(None);
        };
        // A value that is not a blob written by `put_blob_async` is an error,
        // not a missing blob.
        let wrapper: serde_json::Value = serde_json::from_str(json)
            .map_err(|e| StorageError::Deserialization(format!("blob {key}: {e}")))?;
        let encoded = wrapper
            .get("data")
            .and_then(serde_json::Value::as_str)
            .ok_or_else(|| {
                StorageError::Deserialization(format!("blob {key}: no base64 `data` field"))
            })?;
        base64::Engine::decode(&base64::engine::general_purpose::STANDARD, encoded)
            .map(Some)
            .map_err(|e| StorageError::Deserialization(format!("blob {key}: {e}")))
    }

    async fn delete_blob_async(&self, key: &str) -> StorageResult<()> {
        let mut data = self.lock()?;
        data.remove(key);
        Ok(())
    }

    async fn blob_exists_async(&self, key: &str) -> StorageResult<bool> {
        let data = self.lock()?;
        Ok(data.contains_key(key))
    }
}

#[async_trait::async_trait]
impl AsyncRateLimiterStore for MemoryJsonStore {
    async fn check_rate_limit_async(
        &self,
        key: &str,
        max_count: u32,
        window_seconds: u64,
    ) -> StorageResult<bool> {
        let now = unix_now_secs()?;
        let rate_key = format!("_rate_limit:{key}");
        let data = self.lock()?;
        let allowed = match data.get(&rate_key) {
            Some(json) => {
                #[derive(serde::Deserialize)]
                struct Entry { count: u32, window_start: u64 }
                let entry: Entry = serde_json::from_str(json)
                    .map_err(|e| StorageError::Serialization(e.to_string()))?;
                if entry.window_start < now.saturating_sub(window_seconds) {
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
        let now = unix_now_secs()?;
        let rate_key = format!("_rate_limit:{key}");
        let mut data = self.lock()?;
        let new_count = if let Some(json) = data.get(&rate_key) {
            #[derive(serde::Serialize, serde::Deserialize)]
            struct Entry { count: u32, window_start: u64 }
            let mut entry: Entry = serde_json::from_str(json)
                .map_err(|e| StorageError::Serialization(e.to_string()))?;
            entry.count += 1;
            entry.window_start = now;
            let new_json = serde_json::to_string(&entry)
                .map_err(|e| StorageError::Serialization(e.to_string()))?;
            data.insert(rate_key, new_json);
            entry.count
        } else {
            let entry = serde_json::json!({ "count": 1, "window_start": now }).to_string();
            data.insert(rate_key, entry);
            1
        };
        Ok(new_count)
    }

    async fn reset_rate_limit_async(&self, key: &str) -> StorageResult<()> {
        let rate_key = format!("_rate_limit:{key}");
        let mut data = self.lock()?;
        data.remove(&rate_key);
        Ok(())
    }
}

/// `MemoryJsonStore` has no SQL engine, so every query method fails with
/// `StorageError::Generic`.
#[async_trait::async_trait]
impl AsyncQueryStore for MemoryJsonStore {
    async fn query_async(
        &self,
        _sql: &str,
        _params: &[DataValue],
    ) -> StorageResult<AsyncQueryStream> {
        Err(StorageError::Generic(
            "QueryStore not supported for MemoryJsonStore".to_string(),
        ))
    }

    async fn execute_async(&self, _sql: &str, _params: &[DataValue]) -> StorageResult<u64> {
        Err(StorageError::Generic(
            "QueryStore not supported for MemoryJsonStore".to_string(),
        ))
    }

    async fn execute_batch_async(&self, _sql: &str) -> StorageResult<()> {
        Err(StorageError::Generic(
            "QueryStore not supported for MemoryJsonStore".to_string(),
        ))
    }
}
