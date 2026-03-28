//! JSON file storage backend implementation.
//!
//! Provides simple JSON-on-disk persistence for key-value data.
//! Uses atomic writes (temp file + rename) for crash safety and
//! Zeroizing for secure memory handling of sensitive data.

use serde::{de::DeserializeOwned, Serialize};
use std::collections::HashMap;
use std::fs::{self, File};
use std::io::{BufReader, Write};
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};
use zeroize::Zeroizing;

use crate::errors::{StorageError, StorageResult};
use crate::storage_provider::{KeyValueStore, RateLimiterStore, StorageItemStream};
use foundation_core::valtron::Stream;

/// JSON file storage backend.
///
/// Stores data as JSON on disk with atomic writes for crash safety.
/// Uses a Mutex for thread-safe access and Zeroizing for sensitive data.
pub struct JsonFileStorage {
    /// Path to the JSON file
    file_path: PathBuf,
    /// In-memory cache of data (protected by Mutex)
    data: Arc<Mutex<HashMap<String, Zeroizing<Vec<u8>>>>>,
}

impl JsonFileStorage {
    /// Create a new JSON file storage instance.
    ///
    /// # Arguments
    ///
    /// * `file_path` - Path to the JSON file for persistence
    ///
    /// # Errors
    ///
    /// Returns a `StorageError` if the file cannot be read or parsed.
    pub fn new<P: AsRef<Path>>(file_path: P) -> StorageResult<Self> {
        let file_path = file_path.as_ref().to_path_buf();

        // Load existing data if file exists
        let data = if file_path.exists() {
            Self::load_from_file(&file_path)?
        } else {
            HashMap::new()
        };

        Ok(Self {
            file_path,
            data: Arc::new(Mutex::new(data)),
        })
    }

    /// Load data from JSON file.
    ///
    /// # Errors
    ///
    /// Returns a `StorageError` if file I/O or parsing fails.
    fn load_from_file(file_path: &Path) -> StorageResult<HashMap<String, Zeroizing<Vec<u8>>>> {
        let file = File::open(file_path).map_err(|e| {
            StorageError::Io(e)
        })?;

        let reader = BufReader::new(file);
        let raw_data: HashMap<String, Vec<u8>> = serde_json::from_reader(reader)
            .map_err(StorageError::Json)?;

        // Convert to Zeroizing wrappers
        Ok(raw_data
            .into_iter()
            .map(|(k, v)| (k, Zeroizing::new(v)))
            .collect())
    }

    /// Flush data to disk atomically.
    ///
    /// Writes to a temp file first, then renames for atomicity.
    ///
    /// # Errors
    ///
    /// Returns a `StorageError` if file I/O or serialization fails.
    fn flush_to_disk(&self) -> StorageResult<()> {
        let data = self.data.lock().map_err(|e| {
            StorageError::Backend(format!("Mutex poisoned: {e}"))
        })?;

        // Serialize to Vec<u8> first (without Zeroizing for disk storage)
        let raw_data: HashMap<String, Vec<u8>> = data
            .iter()
            .map(|(k, v)| (k.clone(), (**v).clone()))
            .collect();

        let json_bytes = serde_json::to_vec(&raw_data)
            .map_err(StorageError::Json)?;

        // Write to temp file first
        let temp_path = self.file_path.with_extension("json.tmp");

        let mut temp_file = File::create(&temp_path).map_err(|e| {
            StorageError::Io(e)
        })?;

        temp_file.write_all(&json_bytes).map_err(|e| {
            StorageError::Io(e)
        })?;

        // Ensure data is flushed to disk
        temp_file.sync_all().map_err(|e| {
            StorageError::Io(e)
        })?;

        // Atomic rename
        fs::rename(&temp_path, &self.file_path).map_err(|e| {
            StorageError::Io(e)
        })?;

        Ok(())
    }
}

impl KeyValueStore for JsonFileStorage {
    fn get<V: DeserializeOwned + Send>(&self, key: &str) -> StorageResult<StorageItemStream<'_, Option<V>>> {
        let data = self.data.lock().map_err(|e| {
            StorageError::Backend(format!("Mutex poisoned: {e}"))
        })?;

        let result = match data.get(key) {
            Some(bytes) => {
                let value: V = serde_json::from_slice(bytes)
                    .map_err(|e| StorageError::Serialization(e.to_string()))?;
                Some(value)
            }
            None => None,
        };
        Ok(Box::new(std::iter::once(Stream::Next(result))))
    }

    fn set<V: Serialize>(&self, key: &str, value: V) -> StorageResult<StorageItemStream<'_, ()>> {
        let bytes = serde_json::to_vec(&value)
            .map_err(|e| StorageError::Serialization(e.to_string()))?;

        let mut data = self.data.lock().map_err(|e| {
            StorageError::Backend(format!("Mutex poisoned: {e}"))
        })?;

        data.insert(key.to_string(), Zeroizing::new(bytes));
        drop(data); // Release lock before flushing

        self.flush_to_disk()?;

        Ok(Box::new(std::iter::once(Stream::Next(()))))
    }

    fn delete(&self, key: &str) -> StorageResult<StorageItemStream<'_, ()>> {
        let mut data = self.data.lock().map_err(|e| {
            StorageError::Backend(format!("Mutex poisoned: {e}"))
        })?;

        data.remove(key);
        drop(data); // Release lock before flushing

        self.flush_to_disk()?;

        Ok(Box::new(std::iter::once(Stream::Next(()))))
    }

    fn exists(&self, key: &str) -> StorageResult<StorageItemStream<'_, bool>> {
        let data = self.data.lock().map_err(|e| {
            StorageError::Backend(format!("Mutex poisoned: {e}"))
        })?;
        Ok(Box::new(std::iter::once(Stream::Next(data.contains_key(key)))))
    }

    fn list_keys(&self, prefix: Option<&str>) -> StorageResult<StorageItemStream<'_, String>> {
        let data = self.data.lock().map_err(|e| {
            StorageError::Backend(format!("Mutex poisoned: {e}"))
        })?;

        let keys: Vec<String> = data
            .keys()
            .filter(|k| prefix.is_none_or(|p| k.starts_with(p)))
            .cloned()
            .collect();

        Ok(Box::new(keys.into_iter().map(Stream::Next)))
    }
}

/// `RateLimiterStore` not supported for `JsonFileStorage`.
impl RateLimiterStore for JsonFileStorage {
    fn check_rate_limit(
        &self,
        _key: &str,
        _max_count: u32,
        _window_seconds: u64,
    ) -> StorageResult<StorageItemStream<'_, bool>> {
        Err(StorageError::Generic(
            "RateLimiterStore not supported for JsonFileStorage".to_string(),
        ))
    }

    fn record_rate_limit(&self, _key: &str) -> StorageResult<StorageItemStream<'_, u32>> {
        Err(StorageError::Generic(
            "RateLimiterStore not supported for JsonFileStorage".to_string(),
        ))
    }

    fn reset_rate_limit(&self, _key: &str) -> StorageResult<StorageItemStream<'_, ()>> {
        Err(StorageError::Generic(
            "RateLimiterStore not supported for JsonFileStorage".to_string(),
        ))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use foundation_core::valtron::{collect_one, collect_result};
    use tempfile::TempDir;

    #[test]
    fn test_json_file_storage_basic() {
        let temp_dir = TempDir::new().unwrap();
        let file_path = temp_dir.path().join("test.json");

        let storage = JsonFileStorage::new(&file_path).unwrap();

        // Test set and get
        collect_one(storage.set("test_key", "test_value").unwrap()).unwrap();

        let value: Option<String> = collect_one(storage.get("test_key").unwrap()).unwrap();
        assert_eq!(value, Some("test_value".to_string()));

        // Test exists
        assert!(collect_one(storage.exists("test_key").unwrap()).unwrap());
        assert!(!collect_one(storage.exists("nonexistent").unwrap()).unwrap());

        // Test delete
        collect_one(storage.delete("test_key").unwrap()).unwrap();
        assert!(!collect_one(storage.exists("test_key").unwrap()).unwrap());
    }

    #[test]
    fn test_json_file_storage_list_keys() {
        let temp_dir = TempDir::new().unwrap();
        let file_path = temp_dir.path().join("test.json");

        let storage = JsonFileStorage::new(&file_path).unwrap();

        collect_one(storage.set("prefix:key1", "value1").unwrap()).unwrap();
        collect_one(storage.set("prefix:key2", "value2").unwrap()).unwrap();
        collect_one(storage.set("other:key3", "value3").unwrap()).unwrap();

        // List all keys
        let keys = collect_result(storage.list_keys(None).unwrap());
        assert_eq!(keys.len(), 3);

        // List keys with prefix
        let keys = collect_result(storage.list_keys(Some("prefix:")).unwrap());
        assert_eq!(keys.len(), 2);
        assert!(keys.contains(&"prefix:key1".to_string()));
        assert!(keys.contains(&"prefix:key2".to_string()));
    }

    #[test]
    fn test_json_file_storage_persistence() {
        let temp_dir = TempDir::new().unwrap();
        let file_path = temp_dir.path().join("test.json");

        // Create storage and add data
        let storage = JsonFileStorage::new(&file_path).unwrap();
        collect_one(storage.set("key1", "value1").unwrap()).unwrap();
        collect_one(storage.set("key2", "value2").unwrap()).unwrap();

        // Verify file was created
        assert!(file_path.exists());

        // Create new storage instance from same file
        let storage2 = JsonFileStorage::new(&file_path).unwrap();

        // Data should persist
        let value1: Option<String> = collect_one(storage2.get("key1").unwrap()).unwrap();
        assert_eq!(value1, Some("value1".to_string()));

        let value2: Option<String> = collect_one(storage2.get("key2").unwrap()).unwrap();
        assert_eq!(value2, Some("value2".to_string()));
    }

    #[test]
    fn test_json_file_storage_complex_value() {
        let temp_dir = TempDir::new().unwrap();
        let file_path = temp_dir.path().join("test.json");

        let storage = JsonFileStorage::new(&file_path).unwrap();

        #[derive(Serialize, serde::Deserialize, Debug, PartialEq)]
        struct TestData {
            name: String,
            count: u32,
        }

        let data = TestData {
            name: "test".to_string(),
            count: 42,
        };

        collect_one(storage.set("complex", &data).unwrap()).unwrap();

        let retrieved: Option<TestData> = collect_one(storage.get("complex").unwrap()).unwrap();
        assert_eq!(retrieved, Some(data));
    }

    #[test]
    fn test_json_file_storage_atomic_writes() {
        let temp_dir = TempDir::new().unwrap();
        let file_path = temp_dir.path().join("test.json");

        let storage = JsonFileStorage::new(&file_path).unwrap();

        // Add initial data
        collect_one(storage.set("key1", "value1").unwrap()).unwrap();

        // Verify no temp file left behind
        let temp_path = file_path.with_extension("json.tmp");
        assert!(!temp_path.exists());

        // Verify main file exists
        assert!(file_path.exists());
    }
}
