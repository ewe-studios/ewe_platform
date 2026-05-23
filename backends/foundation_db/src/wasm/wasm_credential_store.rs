//! Async credential store for wasm32 targets.
//!
//! WHY: The sync `CredentialStore` trait routes D1 operations through
//! `schedule_future` → `drive_non_send_iterator` → valtron executor. On
//! miniflare, this deadlocks because D1's JS Promise requires the event
//! loop to resolve, but `drive_non_send_iterator` blocks it.
//!
//! WHAT: `WasmCredentialStore` calls `D1WasmStorage`'s native async methods
//! (`get_async`, `set_async`, etc.) which use `JsFuture::from(promise).await`
//! directly — proper `.await` points where the wasm runtime yields correctly.
//!
//! HOW: Wraps `Arc<D1WasmStorage>` and delegates to its `*_async` methods.
//! Mirrors the `CredentialStore` trait surface but with async signatures.

use std::sync::Arc;

use serde::{de::DeserializeOwned, Serialize};

use crate::wasm::D1WasmStorage;
use crate::core::errors::StorageResult;

/// Async credential store for wasm32.
///
/// Provides the same operations as `CredentialStore` but via async methods
/// that properly yield to the JS event loop during D1 I/O.
pub struct WasmCredentialStore {
    storage: Arc<D1WasmStorage>,
}

impl WasmCredentialStore {
    /// Create a new async credential store.
    #[must_use]
    pub fn new(storage: Arc<D1WasmStorage>) -> Self {
        Self { storage }
    }

    /// Get a credential by key.
    ///
    /// # Errors
    ///
    /// Returns [`StorageError`] if the storage operation or
    /// deserialization fails.
    pub async fn get<V: DeserializeOwned + Send + 'static>(
        &self,
        key: &str,
    ) -> StorageResult<Option<V>> {
        self.storage.get_async(key).await
    }

    /// Set a credential.
    ///
    /// # Errors
    ///
    /// Returns [`StorageError`] if the storage operation or
    /// serialization fails.
    pub async fn set<V: Serialize + Send + 'static>(
        &self,
        key: &str,
        value: V,
    ) -> StorageResult<()> {
        self.storage.set_async(key, value).await
    }

    /// Delete a credential.
    ///
    /// # Errors
    ///
    /// Returns [`StorageError`] if the storage operation fails.
    pub async fn delete(&self, key: &str) -> StorageResult<()> {
        self.storage.delete_async(key).await
    }

    /// Check whether a credential exists.
    ///
    /// # Errors
    ///
    /// Returns [`StorageError`] if the storage operation fails.
    pub async fn exists(&self, key: &str) -> StorageResult<bool> {
        self.storage.exists_async(key).await
    }

    /// List credential keys, optionally filtered by prefix.
    ///
    /// # Errors
    ///
    /// Returns [`StorageError`] if the storage operation fails.
    pub async fn list_keys(&self, prefix: Option<&str>) -> StorageResult<Vec<String>> {
        self.storage.list_keys_async(prefix).await
    }
}

// ===========================================================================
// Tests
// ===========================================================================

#[cfg(test)]
mod tests {
    use super::*;
    use crate::core::schema::{MIGRATIONS, MigrationRunner};
    use foundation_core::valtron::initialize_pool;

    /// Build an in-memory credential store for testing.
    fn memory_store() -> WasmCredentialStore {
        let storage = Arc::new(D1WasmStorage::memory("test"));
        WasmCredentialStore::new(storage)
    }

    /// Build a D1-backed credential store. Requires valtron init.
    /// Note: D1 tests require a real CF D1 binding or miniflare.
    /// In-memory tests are preferred for CI.

    #[test]
    fn memory_store_set_and_get() {
        // In-memory storage doesn't need valtron init — it's a pure Rust HashMap.
        let store = memory_store();

        // Use a simple string value
        pollster::block_on(store.set("test_key", "test_value")).expect("set failed");

        let value: Option<String> = pollster::block_on(store.get("test_key"))
            .expect("get failed");
        assert_eq!(value, Some("test_value".to_string()));
    }

    #[test]
    fn memory_store_get_missing() {
        let store = memory_store();
        let value: Option<String> = pollster::block_on(store.get("nonexistent"))
            .expect("get failed");
        assert_eq!(value, None);
    }

    #[test]
    fn memory_store_delete() {
        let store = memory_store();
        pollster::block_on(store.set("delete_me", "gone")).expect("set failed");

        let value: Option<String> = pollster::block_on(store.get("delete_me"))
            .expect("get failed");
        assert_eq!(value, Some("gone".to_string()));

        pollster::block_on(store.delete("delete_me")).expect("delete failed");

        let value: Option<String> = pollster::block_on(store.get("delete_me"))
            .expect("get failed");
        assert_eq!(value, None);
    }

    #[test]
    fn memory_store_exists() {
        let store = memory_store();
        assert!(!pollster::block_on(store.exists("nope")).expect("exists failed"));

        pollster::block_on(store.set("exists", "yes")).expect("set failed");
        assert!(pollster::block_on(store.exists("exists")).expect("exists failed"));
    }

    #[test]
    fn memory_store_overwrite() {
        let store = memory_store();
        pollster::block_on(store.set("key", "v1")).expect("set failed");
        pollster::block_on(store.set("key", "v2")).expect("set failed");

        let value: Option<String> = pollster::block_on(store.get("key"))
            .expect("get failed");
        assert_eq!(value, Some("v2".to_string()));
    }

    #[test]
    fn memory_store_list_keys() {
        let store = memory_store();
        pollster::block_on(store.set("alpha:one", "1")).expect("set failed");
        pollster::block_on(store.set("alpha:two", "2")).expect("set failed");
        pollster::block_on(store.set("beta:one", "3")).expect("set failed");

        let keys = pollster::block_on(store.list_keys(None)).expect("list failed");
        assert_eq!(keys.len(), 3);

        let keys = pollster::block_on(store.list_keys(Some("alpha:")))
            .expect("list failed");
        assert_eq!(keys.len(), 2);
        assert!(keys.contains(&"alpha:one".to_string()));
        assert!(keys.contains(&"alpha:two".to_string()));
    }

    #[test]
    fn memory_store_complex_value() {
        #[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
        struct TestData {
            name: String,
            count: u32,
            tags: Vec<String>,
        }

        let store = memory_store();
        let data = TestData {
            name: "test".to_string(),
            count: 42,
            tags: vec!["a".to_string(), "b".to_string()],
        };

        pollster::block_on(store.set("complex", &data)).expect("set failed");
        let retrieved: Option<TestData> = pollster::block_on(store.get("complex"))
            .expect("get failed");
        assert_eq!(retrieved, Some(data));
    }

    #[test]
    fn memory_store_with_valtron_init() {
        // Verify that valtron init doesn't break in-memory operations.
        initialize_pool(123);

        let store = memory_store();
        pollster::block_on(store.set("after_init", "works")).expect("set failed");
        let value: Option<String> = pollster::block_on(store.get("after_init"))
            .expect("get failed");
        assert_eq!(value, Some("works".to_string()));
    }

    // D1 integration test — requires a real D1 binding.
    // This test is gated behind a feature flag to avoid CI failures.
    //
    // To run locally with miniflare:
    // ```
    // cargo test --features wasm-d1-test --target wasm32-unknown-unknown
    // ```
    #[cfg(feature = "wasm-d1-test")]
    mod d1_integration {
        use super::*;
        use wasm_bindgen_test::*;

        #[wasm_bindgen_test]
        async fn d1_store_basic() {
            // This would need a real D1Database from the test environment.
            // Placeholder for future implementation.
        }
    }
}
