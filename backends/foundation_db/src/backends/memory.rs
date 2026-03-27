//! In-memory storage backend with secure memory handling.

use std::collections::HashMap;

use async_trait::async_trait;
use serde::{de::DeserializeOwned, Serialize};
use zeroize::Zeroizing;

use crate::errors::{StorageError, StorageResult};
use crate::storage_provider::{KeyValueStore, QueryStore};

/// In-memory storage with zeroizing support for sensitive data.
pub struct MemoryStorage {
    data: tokio::sync::Mutex<HashMap<String, Zeroizing<Vec<u8>>>>,
}

impl MemoryStorage {
    /// Create a new in-memory storage instance.
    pub fn new() -> Self {
        Self {
            data: tokio::sync::Mutex::new(HashMap::new()),
        }
    }
}

impl Default for MemoryStorage {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait]
impl KeyValueStore for MemoryStorage {
    async fn get<V: DeserializeOwned>(&self, key: &str) -> StorageResult<Option<V>> {
        let data = self.data.lock().await;
        match data.get(key) {
            Some(bytes) => {
                let value: V = serde_json::from_slice(bytes)
                    .map_err(|e| StorageError::Serialization(e.to_string()))?;
                Ok(Some(value))
            }
            None => Ok(None),
        }
    }

    async fn set<V: Serialize + Send>(&self, key: &str, value: V) -> StorageResult<()> {
        let bytes = serde_json::to_vec(&value)
            .map_err(|e| StorageError::Serialization(e.to_string()))?;
        // Use Zeroizing to ensure secure memory handling
        let mut data = self.data.lock().await;
        data.insert(key.to_string(), Zeroizing::new(bytes));
        Ok(())
    }

    async fn delete(&self, key: &str) -> StorageResult<()> {
        let mut data = self.data.lock().await;
        data.remove(key);
        Ok(())
    }

    async fn exists(&self, key: &str) -> StorageResult<bool> {
        let data = self.data.lock().await;
        Ok(data.contains_key(key))
    }

    async fn list_keys(&self, prefix: Option<&str>) -> StorageResult<Vec<String>> {
        let data = self.data.lock().await;
        let keys: Vec<String> = data
            .keys()
            .filter(|k| {
                prefix
                    .map(|p| k.starts_with(p))
                    .unwrap_or(true)
            })
            .cloned()
            .collect();
        Ok(keys)
    }
}

/// In-memory implementation of QueryStore for testing.
/// Note: This is a simplified implementation for testing purposes.
#[async_trait]
impl QueryStore for MemoryStorage {
    async fn query(
        &self,
        _sql: &str,
        _params: Vec<libsql::Value>,
    ) -> StorageResult<Vec<libsql::Row>> {
        Err(StorageError::Generic(
            "QueryStore not supported for MemoryStorage".to_string(),
        ))
    }

    async fn execute(
        &self,
        _sql: &str,
        _params: Vec<libsql::Value>,
    ) -> StorageResult<u64> {
        Err(StorageError::Generic(
            "QueryStore not supported for MemoryStorage".to_string(),
        ))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_memory_storage_basic() {
        let storage = MemoryStorage::new();

        // Test set and get
        storage
            .set("test_key", "test_value")
            .await
            .unwrap();

        let value: String = storage.get("test_key").await.unwrap().unwrap();
        assert_eq!(value, "test_value");

        // Test exists
        assert!(storage.exists("test_key").await.unwrap());
        assert!(!storage.exists("nonexistent").await.unwrap());

        // Test delete
        storage.delete("test_key").await.unwrap();
        assert!(!storage.exists("test_key").await.unwrap());
    }

    #[tokio::test]
    async fn test_memory_storage_list_keys() {
        let storage = MemoryStorage::new();

        storage.set("prefix:key1", "value1").await.unwrap();
        storage.set("prefix:key2", "value2").await.unwrap();
        storage.set("other:key3", "value3").await.unwrap();

        // List all keys
        let keys = storage.list_keys(None).await.unwrap();
        assert_eq!(keys.len(), 3);

        // List keys with prefix
        let keys = storage.list_keys(Some("prefix:")).await.unwrap();
        assert_eq!(keys.len(), 2);
        assert!(keys.contains(&"prefix:key1".to_string()));
        assert!(keys.contains(&"prefix:key2".to_string()));
    }

    #[tokio::test]
    async fn test_memory_storage_complex_value() {
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

        storage.set("complex", &data).await.unwrap();

        let retrieved: TestData = storage.get("complex").await.unwrap().unwrap();
        assert_eq!(data, retrieved);
    }
}
