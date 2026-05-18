//! In-memory JSON key-value store — works on both native and wasm.
//!
//! Unlike [`MemoryStorage`](super::memory::MemoryStorage), this stores
//! values as JSON strings rather than zeroizing byte buffers, making it
//! suitable for non-sensitive data and easier to inspect/debug.

use std::collections::HashMap;
use std::sync::{Arc, Mutex};

use serde::{de::DeserializeOwned, Serialize};

use crate::core::errors::{StorageError, StorageResult};
use crate::core::storage_provider::{
    BlobStore, DataValue, KeyValueStore, QueryStore, RateLimiterStore, SqlRow, StorageItemStream,
};
use foundation_core::valtron::Stream;

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

    fn lock(&self) -> Result<std::sync::MutexGuard<'_, HashMap<String, String>>, StorageError> {
        self.data
            .lock()
            .map_err(|e| StorageError::Backend(format!("Mutex poisoned: {e}")))
    }

    fn stream_once<T: Send + 'static>(val: T) -> StorageItemStream<'static, T> {
        Box::new(std::iter::once(Stream::Next(Ok(val))))
    }

    fn stream_many<T: Send + 'static>(vals: Vec<T>) -> StorageItemStream<'static, T> {
        Box::new(vals.into_iter().map(|v| Stream::Next(Ok(v))))
    }
}

impl Default for MemoryJsonStore {
    fn default() -> Self {
        Self::new()
    }
}

impl KeyValueStore for MemoryJsonStore {
    fn get<'a, V: DeserializeOwned + Send + 'static>(
        &'a self,
        key: &str,
    ) -> StorageResult<StorageItemStream<'a, Option<V>>> {
        let data = self.lock()?;
        let result = match data.get(key) {
            Some(json) => {
                let value: V =
                    serde_json::from_str(json).map_err(|e| StorageError::Serialization(e.to_string()))?;
                Some(value)
            }
            None => None,
        };
        Ok(Self::stream_once(result))
    }

    fn set<V: Serialize>(&self, key: &str, value: V) -> StorageResult<StorageItemStream<'_, ()>> {
        let json = serde_json::to_string(&value)
            .map_err(|e| StorageError::Serialization(e.to_string()))?;
        let mut data = self.lock()?;
        data.insert(key.to_string(), json);
        Ok(Self::stream_once(()))
    }

    fn delete(&self, key: &str) -> StorageResult<StorageItemStream<'_, ()>> {
        let mut data = self.lock()?;
        data.remove(key);
        Ok(Self::stream_once(()))
    }

    fn exists(&self, key: &str) -> StorageResult<StorageItemStream<'_, bool>> {
        let data = self.lock()?;
        Ok(Self::stream_once(data.contains_key(key)))
    }

    fn list_keys(&self, prefix: Option<&str>) -> StorageResult<StorageItemStream<'_, String>> {
        let data = self.lock()?;
        let keys: Vec<String> = data
            .keys()
            .filter(|k| prefix.is_none_or(|p| k.starts_with(p)))
            .cloned()
            .collect();
        Ok(Self::stream_many(keys))
    }
}

impl QueryStore for MemoryJsonStore {
    fn query(
        &self,
        _sql: &str,
        _params: &[DataValue],
    ) -> StorageResult<StorageItemStream<'_, SqlRow>> {
        Err(StorageError::Generic(
            "QueryStore not supported for MemoryJsonStore".to_string(),
        ))
    }

    fn execute(
        &self,
        _sql: &str,
        _params: &[DataValue],
    ) -> StorageResult<StorageItemStream<'_, u64>> {
        Err(StorageError::Generic(
            "QueryStore not supported for MemoryJsonStore".to_string(),
        ))
    }

    fn execute_batch(&self, _sql: &str) -> StorageResult<StorageItemStream<'_, ()>> {
        Err(StorageError::Generic(
            "QueryStore not supported for MemoryJsonStore".to_string(),
        ))
    }
}

impl RateLimiterStore for MemoryJsonStore {
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
        let rate_key = format!("_rate_limit:{key}");
        let data = self.lock()?;
        let allowed = match data.get(&rate_key) {
            Some(json) => {
                #[derive(serde::Deserialize)]
                struct Entry {
                    count: u32,
                    window_start: u64,
                }
                let entry: Entry = serde_json::from_str(json)
                    .map_err(|e| StorageError::Serialization(e.to_string()))?;
                if entry.window_start < now - window_seconds {
                    true
                } else {
                    entry.count < max_count
                }
            }
            None => true,
        };
        Ok(Self::stream_once(allowed))
    }

    fn record_rate_limit(&self, key: &str) -> StorageResult<StorageItemStream<'_, u32>> {
        let now = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_secs();
        let rate_key = format!("_rate_limit:{key}");
        let mut data = self.lock()?;
        let new_count = if let Some(json) = data.get(&rate_key) {
            #[derive(serde::Serialize, serde::Deserialize)]
            struct Entry {
                count: u32,
                window_start: u64,
            }
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
        Ok(Self::stream_once(new_count))
    }

    fn reset_rate_limit(&self, key: &str) -> StorageResult<StorageItemStream<'_, ()>> {
        let rate_key = format!("_rate_limit:{key}");
        let mut data = self.lock()?;
        data.remove(&rate_key);
        Ok(Self::stream_once(()))
    }
}

impl BlobStore for MemoryJsonStore {
    fn put_blob(&self, key: &str, data: &[u8]) -> StorageResult<StorageItemStream<'_, ()>> {
        // Store binary data as base64 JSON string
        let encoded = base64::Engine::encode(&base64::engine::general_purpose::STANDARD, data);
        let json = serde_json::json!({ "type": "blob", "data": encoded }).to_string();
        let mut store = self.lock()?;
        store.insert(key.to_string(), json);
        Ok(Self::stream_once(()))
    }

    fn get_blob(&self, key: &str) -> StorageResult<StorageItemStream<'_, Option<Vec<u8>>>> {
        let data = self.lock()?;
        let result = data.get(key).and_then(|json| {
            let wrapper: serde_json::Value = serde_json::from_str(json).ok()?;
            let encoded = wrapper.get("data").and_then(|v| v.as_str())?;
            base64::Engine::decode(&base64::engine::general_purpose::STANDARD, encoded).ok()
        });
        Ok(Self::stream_once(result))
    }

    fn delete_blob(&self, key: &str) -> StorageResult<StorageItemStream<'_, ()>> {
        let mut data = self.lock()?;
        data.remove(key);
        Ok(Self::stream_once(()))
    }

    fn blob_exists(&self, key: &str) -> StorageResult<StorageItemStream<'_, bool>> {
        let data = self.lock()?;
        Ok(Self::stream_once(data.contains_key(key)))
    }
}
