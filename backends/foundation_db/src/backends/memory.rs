//! In-memory storage backend with secure memory handling.
//!
//! Uses `std::sync::Mutex` for thread-safe access. All operations
//! are synchronous and return `StorageResult<T>` directly.

use std::collections::HashMap;
use std::sync::{Arc, Mutex};

use serde::{de::DeserializeOwned, Serialize};
use zeroize::Zeroizing;

use crate::errors::{StorageError, StorageResult};
use crate::storage_provider::{DataValue, KeyValueStore, QueryStore, SqlRow, StorageItemStream};

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
    fn get<V: DeserializeOwned>(&self, key: &str) -> StorageResult<Option<V>> {
        let data = self.data.lock().map_err(|e| {
            StorageError::Backend(format!("Mutex poisoned: {e}"))
        })?;

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
        let bytes = serde_json::to_vec(&value)
            .map_err(|e| StorageError::Serialization(e.to_string()))?;

        let mut data = self.data.lock().map_err(|e| {
            StorageError::Backend(format!("Mutex poisoned: {e}"))
        })?;

        // Use Zeroizing to ensure secure memory handling
        data.insert(key.to_string(), Zeroizing::new(bytes));
        Ok(())
    }

    fn delete(&self, key: &str) -> StorageResult<()> {
        let mut data = self.data.lock().map_err(|e| {
            StorageError::Backend(format!("Mutex poisoned: {e}"))
        })?;
        data.remove(key);
        Ok(())
    }

    fn exists(&self, key: &str) -> StorageResult<bool> {
        let data = self.data.lock().map_err(|e| {
            StorageError::Backend(format!("Mutex poisoned: {e}"))
        })?;
        Ok(data.contains_key(key))
    }

    fn list_keys(&self, prefix: Option<&str>) -> StorageResult<StorageItemStream<'_, String>> {
        let data = self.data.lock().map_err(|e| {
            StorageError::Backend(format!("Mutex poisoned: {e}"))
        })?;

        let keys: Vec<String> = data
            .keys()
            .filter(|k| prefix.map_or(true, |p| k.starts_with(p)))
            .cloned()
            .collect();

        Ok(Box::new(keys.into_iter()))
    }
}

/// In-memory implementation of [`QueryStore`] for testing.
/// Note: This is a simplified implementation for testing purposes.
impl QueryStore for MemoryStorage {
    fn query(&self, _sql: &str, _params: &[DataValue]) -> StorageResult<StorageItemStream<'_, SqlRow>> {
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_memory_storage_basic() {
        let storage = MemoryStorage::new();

        // Test set and get
        storage.set("test_key", "test_value").unwrap();

        let value: String = storage.get("test_key").unwrap().unwrap();
        assert_eq!(value, "test_value");

        // Test exists
        assert!(storage.exists("test_key").unwrap());
        assert!(!storage.exists("nonexistent").unwrap());

        // Test delete
        storage.delete("test_key").unwrap();
        assert!(!storage.exists("test_key").unwrap());
    }

    #[test]
    fn test_memory_storage_list_keys() {
        let storage = MemoryStorage::new();

        storage.set("prefix:key1", "value1").unwrap();
        storage.set("prefix:key2", "value2").unwrap();
        storage.set("other:key3", "value3").unwrap();

        // List all keys
        let keys: Vec<String> = storage.list_keys(None).unwrap().collect();
        assert_eq!(keys.len(), 3);

        // List keys with prefix
        let keys: Vec<String> = storage.list_keys(Some("prefix:")).unwrap().collect();
        assert_eq!(keys.len(), 2);
        assert!(keys.contains(&"prefix:key1".to_string()));
        assert!(keys.contains(&"prefix:key2".to_string()));
    }

    #[test]
    fn test_memory_storage_complex_value() {
        let storage = MemoryStorage::new();

        #[derive(Serialize, serde::Deserialize, Debug, PartialEq)]
        struct TestData {
            name: String,
            count: u32,
        }

        let data = TestData {
            name: "test".to_string(),
            count: 42,
        };

        storage.set("complex", &data).unwrap();

        let retrieved: TestData = storage.get("complex").unwrap().unwrap();
        assert_eq!(data, retrieved);
    }
}
