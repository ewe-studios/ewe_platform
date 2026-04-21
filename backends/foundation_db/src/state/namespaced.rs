//! `NamespacedStore` — `StateStore` wrapper with automatic key prefixing.
//!
//! WHY: Deployables need isolated state so one deployable's resources can't
//! collide with another's. The wrapper makes prefixing structural, not convention.
//!
//! WHAT: Thin wrapper around any `StateStore` that prefixes all keys with a namespace.
//!
//! HOW: All key-based operations prepend `self.prefix`. `list()` filters by prefix
//! and strips it from returned keys.

use foundation_core::valtron::ThreadedValue;
use std::sync::Arc;

use super::traits::{StateStore, StateStoreStream};
use super::types::ResourceState;
use crate::errors::StorageError;

/// `StateStore` wrapper that automatically prefixes all keys with a namespace.
///
/// WHY: Convention-based prefixing is easy to forget and impossible to enforce.
/// The wrapper makes it structural — once you have a `NamespacedStore`, every
/// operation is scoped. You can't accidentally write to another deployable's namespace.
///
/// WHAT: Implements `StateStore` by prepending `{prefix}/` to all keys.
///
/// HOW: `store(key, val)` → inner.store("{prefix}{key}", val), etc.
/// `list()` filters to prefix keys and strips the prefix from returned keys.
pub struct NamespacedStore<S: StateStore> {
    inner: Arc<S>,
    prefix: String,
}

impl<S: StateStore> NamespacedStore<S> {
    /// Create a new `NamespacedStore` wrapping the given `StateStore`.
    ///
    /// All operations on the returned store will be prefixed with `namespace`.
    #[must_use]
    pub fn new(inner: Arc<S>, namespace: &str) -> Self {
        Self {
            inner,
            prefix: format!("{namespace}/"),
        }
    }

    /// Get the namespace prefix used by this store.
    #[must_use]
    pub fn prefix(&self) -> &str {
        &self.prefix
    }

    /// Store a typed value under the given key.
    ///
    /// Serializes the value as JSON into the `ResourceState`'s `output` field.
    ///
    /// # Errors
    ///
    /// Returns an error if serialization fails or the inner store fails.
    pub fn store_typed<T: serde::Serialize>(
        &self,
        key: &str,
        value: &T,
    ) -> Result<(), StorageError> {
        let full_key = format!("{prefix}{key}", prefix = self.prefix);
        let json_value = serde_json::to_value(value).map_err(|e| {
            StorageError::Io(std::io::Error::other(format!(
                "Failed to serialize state: {e}"
            )))
        })?;

        let state = ResourceState {
            id: full_key.clone(),
            kind: String::new(),
            provider: String::new(),
            status: crate::state::types::StateStatus::Created,
            environment: None,
            config_hash: String::new(),
            output: json_value,
            config_snapshot: serde_json::json!(null),
            created_at: chrono::Utc::now(),
            updated_at: chrono::Utc::now(),
        };

        let stream = self.inner.set(&full_key, &state)?;
        // Drive the stream to completion (sync backends yield one value)
        for item in stream {
            if let ThreadedValue::Value(Err(e)) = item {
                return Err(e);
            }
        }
        Ok(())
    }

    /// Get a typed value from the store.
    ///
    /// Deserializes the `ResourceState`'s `output` field into the requested type.
    ///
    /// # Errors
    ///
    /// Returns an error if deserialization fails or the inner store fails.
    pub fn get_typed<T: for<'de> serde::Deserialize<'de>>(
        &self,
        key: &str,
    ) -> Result<Option<T>, StorageError> {
        let full_key = format!("{prefix}{key}", prefix = self.prefix);
        let stream = self.inner.get(&full_key)?;

        if let Some(item) = stream.into_iter().next() {
            match item {
                ThreadedValue::Value(Ok(Some(state))) => {
                    let value: T = serde_json::from_value(state.output).map_err(|e| {
                        StorageError::Io(std::io::Error::other(format!(
                            "Failed to deserialize state: {e}"
                        )))
                    })?;
                    return Ok(Some(value));
                }
                ThreadedValue::Value(Ok(None)) => return Ok(None),
                ThreadedValue::Value(Err(e)) => return Err(e),
            }
        }
        Ok(None)
    }

    /// Delete a key from the store.
    ///
    /// # Errors
    ///
    /// Returns an error if the inner store fails.
    pub fn remove(&self, key: &str) -> Result<(), StorageError> {
        let full_key = format!("{prefix}{key}", prefix = self.prefix);
        let stream = self.inner.delete(&full_key)?;
        for item in stream {
            if let ThreadedValue::Value(Err(e)) = item {
                return Err(e);
            }
        }
        Ok(())
    }
}

impl<S: StateStore> StateStore for NamespacedStore<S> {
    fn init(&self) -> Result<(), StorageError> {
        self.inner.init()
    }

    fn list(&self) -> Result<StateStoreStream<String>, StorageError> {
        let stream = self.inner.list_by_prefix(&self.prefix)?;
        let prefix = self.prefix.clone();
        Ok(Box::new(stream.map(move |item| match item {
            ThreadedValue::Value(Ok(id)) => {
                ThreadedValue::Value(Ok(id.strip_prefix(&prefix).unwrap().to_string()))
            }
            other @ ThreadedValue::Value(_) => other,
        })))
    }

    fn count(&self) -> Result<StateStoreStream<usize>, StorageError> {
        self.inner.count_by_prefix(&self.prefix)
    }

    fn get(
        &self,
        resource_id: &str,
    ) -> Result<StateStoreStream<Option<ResourceState>>, StorageError> {
        let full_key = format!("{prefix}{resource_id}", prefix = self.prefix);
        self.inner.get(&full_key)
    }

    fn get_batch(&self, ids: &[&str]) -> Result<StateStoreStream<ResourceState>, StorageError> {
        let full_ids: Vec<String> = ids
            .iter()
            .map(|id| format!("{prefix}{id}", prefix = self.prefix))
            .collect();
        let full_id_refs: Vec<&str> = full_ids.iter().map(String::as_str).collect();
        self.inner.get_batch(&full_id_refs)
    }

    fn all(&self) -> Result<StateStoreStream<ResourceState>, StorageError> {
        self.inner.all_by_prefix(&self.prefix)
    }

    fn set(
        &self,
        resource_id: &str,
        state: &ResourceState,
    ) -> Result<StateStoreStream<()>, StorageError> {
        let full_key = format!("{prefix}{resource_id}", prefix = self.prefix);
        self.inner.set(&full_key, state)
    }

    fn delete(&self, resource_id: &str) -> Result<StateStoreStream<()>, StorageError> {
        let full_key = format!("{prefix}{resource_id}", prefix = self.prefix);
        self.inner.delete(&full_key)
    }

    fn sync_remote(&self) -> Result<(), StorageError> {
        self.inner.sync_remote()
    }

    fn list_by_prefix(&self, prefix: &str) -> Result<StateStoreStream<String>, StorageError> {
        let combined = format!("{ns}{prefix}", ns = self.prefix, prefix = prefix);
        let stream = self.inner.list_by_prefix(&combined)?;
        let ns_prefix = self.prefix.clone();
        Ok(Box::new(stream.map(move |item| match item {
            ThreadedValue::Value(Ok(id)) => {
                let stripped = id.strip_prefix(&ns_prefix).unwrap_or(&id);
                ThreadedValue::Value(Ok(stripped.to_string()))
            }
            other @ ThreadedValue::Value(_) => other,
        })))
    }

    fn count_by_prefix(&self, prefix: &str) -> Result<StateStoreStream<usize>, StorageError> {
        let combined = format!("{ns}{prefix}", ns = self.prefix, prefix = prefix);
        self.inner.count_by_prefix(&combined)
    }

    fn all_by_prefix(&self, prefix: &str) -> Result<StateStoreStream<ResourceState>, StorageError> {
        // Combine namespace prefix with the given prefix filter.
        // The inner store uses list_by_prefix semantics on id.
        let combined = format!("{ns}{prefix}", ns = self.prefix, prefix = prefix);
        self.inner.all_by_prefix(&combined)
    }

    fn find_by_kind(&self, kind: &str) -> Result<StateStoreStream<ResourceState>, StorageError> {
        let prefix = self.prefix.clone();
        let kind = kind.to_string();
        let all = self.inner.all_by_prefix(&prefix)?;
        Ok(Box::new(all.filter_map(move |item| match item {
            ThreadedValue::Value(Ok(state)) if state.kind == kind => {
                Some(ThreadedValue::Value(Ok(state)))
            }
            ThreadedValue::Value(Ok(_)) => None,
            ThreadedValue::Value(Err(e)) => Some(ThreadedValue::Value(Err(e))),
        })))
    }

    fn find_by_status(
        &self,
        status: &str,
    ) -> Result<StateStoreStream<ResourceState>, StorageError> {
        let prefix = self.prefix.clone();
        let status = status.to_string();
        let all = self.inner.all_by_prefix(&prefix)?;
        Ok(Box::new(all.filter_map(move |item| match item {
            ThreadedValue::Value(Ok(state)) => {
                let status_str = state.status.to_string();
                if status_str == status {
                    Some(ThreadedValue::Value(Ok(state)))
                } else {
                    None
                }
            }
            ThreadedValue::Value(Err(e)) => Some(ThreadedValue::Value(Err(e))),
        })))
    }

    fn find_by_provider(
        &self,
        provider: &str,
    ) -> Result<StateStoreStream<ResourceState>, StorageError> {
        let prefix = self.prefix.clone();
        let provider = provider.to_string();
        let all = self.inner.all_by_prefix(&prefix)?;
        Ok(Box::new(all.filter_map(move |item| match item {
            ThreadedValue::Value(Ok(state)) if state.provider == provider => {
                Some(ThreadedValue::Value(Ok(state)))
            }
            ThreadedValue::Value(Ok(_)) => None,
            ThreadedValue::Value(Err(e)) => Some(ThreadedValue::Value(Err(e))),
        })))
    }

    fn delete_by_prefix(&self, prefix: &str) -> Result<StateStoreStream<usize>, StorageError> {
        let combined = format!("{ns}{prefix}", ns = self.prefix, prefix = prefix);
        self.inner.delete_by_prefix(&combined)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::state::helpers::drive_to_completion;
    use crate::state::FileStateStore;

    fn temp_store() -> (tempfile::TempDir, FileStateStore) {
        let temp_dir = tempfile::tempdir().expect("Failed to create temp dir");
        let store = FileStateStore::with_root(temp_dir.path().to_path_buf());
        store.init().expect("Failed to init store");
        (temp_dir, store)
    }

    #[test]
    fn test_prefix_isolation() {
        let (_temp, store) = temp_store();
        let store_arc = Arc::new(store);

        let ns_a = NamespacedStore::new(Arc::clone(&store_arc), "namespace-a");
        let ns_b = NamespacedStore::new(store_arc, "namespace-b");

        ns_a.store_typed("key1", &"value-a").unwrap();
        ns_b.store_typed("key1", &"value-b").unwrap();

        let a_val: Option<String> = ns_a.get_typed("key1").unwrap();
        let b_val: Option<String> = ns_b.get_typed("key1").unwrap();

        assert_eq!(a_val, Some("value-a".to_string()));
        assert_eq!(b_val, Some("value-b".to_string()));
    }

    #[test]
    fn test_list_filters_by_prefix() {
        let (_temp, store) = temp_store();
        let store_arc = Arc::new(store);
        let ns = NamespacedStore::new(Arc::clone(&store_arc), "test");

        ns.store_typed("resource-1", &1).unwrap();
        ns.store_typed("resource-2", &2).unwrap();
        drive_to_completion(
            store_arc
                .set(
                    "other-key",
                    &ResourceState {
                        id: "other-key".to_string(),
                        kind: String::new(),
                        provider: String::new(),
                        status: crate::state::types::StateStatus::Created,
                        environment: None,
                        config_hash: String::new(),
                        output: serde_json::json!(99),
                        config_snapshot: serde_json::json!(null),
                        created_at: chrono::Utc::now(),
                        updated_at: chrono::Utc::now(),
                    },
                )
                .unwrap(),
        )
        .unwrap();

        let namespaced_ids: Vec<_> = ns
            .list()
            .unwrap()
            .filter_map(|i| match i {
                ThreadedValue::Value(Ok(id)) => Some(id),
                _ => None,
            })
            .collect();

        assert!(namespaced_ids.contains(&"resource-1".to_string()));
        assert!(namespaced_ids.contains(&"resource-2".to_string()));
        assert!(!namespaced_ids.contains(&"other-key".to_string()));
    }

    #[test]
    fn test_delete_removes_key() {
        let (_temp, store) = temp_store();
        let ns = NamespacedStore::new(Arc::new(store), "del-test");

        ns.store_typed("to-delete", &42).unwrap();
        assert!(ns.get_typed::<i32>("to-delete").unwrap().is_some());

        ns.remove("to-delete").unwrap();
        assert!(ns.get_typed::<i32>("to-delete").unwrap().is_none());
    }

    #[test]
    fn test_struct_serialization() {
        #[derive(Debug, Clone, serde::Serialize, serde::Deserialize, PartialEq)]
        struct TestData {
            name: String,
            count: u32,
        }

        let (_temp, store) = temp_store();
        let ns = NamespacedStore::new(Arc::new(store), "struct-test");

        let data = TestData {
            name: "test".to_string(),
            count: 42,
        };
        ns.store_typed("data", &data).unwrap();

        let loaded: Option<TestData> = ns.get_typed("data").unwrap();
        assert_eq!(loaded, Some(data));
    }

    #[test]
    fn test_all_returns_only_namespaced() {
        let (_temp, store) = temp_store();
        let store_arc = Arc::new(store);
        let ns = NamespacedStore::new(Arc::clone(&store_arc), "all-test");
        ns.store_typed("one", &1).unwrap();
        ns.store_typed("two", &2).unwrap();

        drive_to_completion(
            store_arc
                .set(
                    "unrelated",
                    &ResourceState {
                        id: "unrelated".to_string(),
                        kind: String::new(),
                        provider: String::new(),
                        status: crate::state::types::StateStatus::Created,
                        environment: None,
                        config_hash: String::new(),
                        output: serde_json::json!(0),
                        config_snapshot: serde_json::json!(null),
                        created_at: chrono::Utc::now(),
                        updated_at: chrono::Utc::now(),
                    },
                )
                .unwrap(),
        )
        .unwrap();

        let all: Vec<_> = ns
            .all()
            .unwrap()
            .filter_map(|i| match i {
                ThreadedValue::Value(Ok(s)) => Some(s),
                _ => None,
            })
            .collect();

        assert_eq!(all.len(), 2);
        assert!(all.iter().any(|s| s.id.ends_with("one")));
        assert!(all.iter().any(|s| s.id.ends_with("two")));
        assert!(!all.iter().any(|s| s.id == "unrelated"));
    }

    #[test]
    fn test_count_only_namespaced() {
        let (_temp, store) = temp_store();
        let store_arc = Arc::new(store);
        let ns = NamespacedStore::new(Arc::clone(&store_arc), "count-test");

        ns.store_typed("a", &1).unwrap();
        ns.store_typed("b", &2).unwrap();

        drive_to_completion(
            store_arc
                .set(
                    "other",
                    &ResourceState {
                        id: "other".to_string(),
                        kind: String::new(),
                        provider: String::new(),
                        status: crate::state::types::StateStatus::Created,
                        environment: None,
                        config_hash: String::new(),
                        output: serde_json::json!(0),
                        config_snapshot: serde_json::json!(null),
                        created_at: chrono::Utc::now(),
                        updated_at: chrono::Utc::now(),
                    },
                )
                .unwrap(),
        )
        .unwrap();

        let count: Vec<_> = ns
            .count()
            .unwrap()
            .filter_map(|i| match i {
                ThreadedValue::Value(Ok(c)) => Some(c),
                _ => None,
            })
            .collect();

        assert_eq!(count, vec![2]);
    }

    #[test]
    fn test_list_by_prefix_delegates() {
        let (_temp, store) = temp_store();
        let store_arc = Arc::new(store);
        let ns = NamespacedStore::new(Arc::clone(&store_arc), "prefix-test");

        ns.store_typed("alpha-1", &1).unwrap();
        ns.store_typed("alpha-2", &2).unwrap();
        ns.store_typed("beta-1", &3).unwrap();

        let alpha_ids: Vec<_> = ns
            .list_by_prefix("alpha-")
            .unwrap()
            .filter_map(|i| match i {
                ThreadedValue::Value(Ok(id)) => Some(id),
                _ => None,
            })
            .collect();

        assert_eq!(alpha_ids.len(), 2);
        assert!(alpha_ids.contains(&"alpha-1".to_string()));
        assert!(alpha_ids.contains(&"alpha-2".to_string()));
    }

    #[test]
    fn test_count_by_prefix_delegates() {
        let (_temp, store) = temp_store();
        let store_arc = Arc::new(store);
        let ns = NamespacedStore::new(Arc::clone(&store_arc), "count-prefix");

        ns.store_typed("a", &1).unwrap();
        ns.store_typed("b", &2).unwrap();
        drive_to_completion(
            store_arc
                .set(
                    "other",
                    &ResourceState {
                        id: "other".to_string(),
                        kind: String::new(),
                        provider: String::new(),
                        status: crate::state::types::StateStatus::Created,
                        environment: None,
                        config_hash: String::new(),
                        output: serde_json::json!(0),
                        config_snapshot: serde_json::json!(null),
                        created_at: chrono::Utc::now(),
                        updated_at: chrono::Utc::now(),
                    },
                )
                .unwrap(),
        )
        .unwrap();

        let count: Vec<_> = ns
            .count_by_prefix("")
            .unwrap()
            .filter_map(|i| match i {
                ThreadedValue::Value(Ok(c)) => Some(c),
                _ => None,
            })
            .collect();

        assert_eq!(count, vec![2]);
    }

    #[test]
    fn test_delete_by_prefix_deletes_only_namespaced() {
        let (_temp, store) = temp_store();
        let store_arc = Arc::new(store);
        let ns = NamespacedStore::new(Arc::clone(&store_arc), "del-prefix");

        ns.store_typed("keep", &1).unwrap();
        ns.store_typed("remove-1", &2).unwrap();
        ns.store_typed("remove-2", &3).unwrap();

        // Insert non-namespaced key directly
        drive_to_completion(
            store_arc
                .set(
                    "unrelated",
                    &ResourceState {
                        id: "unrelated".to_string(),
                        kind: String::new(),
                        provider: String::new(),
                        status: crate::state::types::StateStatus::Created,
                        environment: None,
                        config_hash: String::new(),
                        output: serde_json::json!(0),
                        config_snapshot: serde_json::json!(null),
                        created_at: chrono::Utc::now(),
                        updated_at: chrono::Utc::now(),
                    },
                )
                .unwrap(),
        )
        .unwrap();

        ns.remove("remove-1").unwrap();
        ns.remove("remove-2").unwrap();

        assert!(ns.get_typed::<i32>("keep").unwrap().is_some());
        // Verify unrelated is still there
        let stream = store_arc.get("unrelated").unwrap();
        let found: Vec<_> = stream
            .filter_map(|i| match i {
                ThreadedValue::Value(Ok(Some(s))) => Some(s),
                _ => None,
            })
            .collect();
        assert_eq!(found.len(), 1);
    }

    #[test]
    fn test_all_by_prefix_delegates() {
        let (_temp, store) = temp_store();
        let store_arc = Arc::new(store);
        let ns = NamespacedStore::new(Arc::clone(&store_arc), "all-prefix");

        ns.store_typed("one", &10).unwrap();
        ns.store_typed("two", &20).unwrap();

        // Insert non-namespaced key directly
        drive_to_completion(
            store_arc
                .set(
                    "unrelated",
                    &ResourceState {
                        id: "unrelated".to_string(),
                        kind: String::new(),
                        provider: String::new(),
                        status: crate::state::types::StateStatus::Created,
                        environment: None,
                        config_hash: String::new(),
                        output: serde_json::json!(0),
                        config_snapshot: serde_json::json!(null),
                        created_at: chrono::Utc::now(),
                        updated_at: chrono::Utc::now(),
                    },
                )
                .unwrap(),
        )
        .unwrap();

        let all: Vec<_> = ns
            .all()
            .unwrap()
            .filter_map(|i| match i {
                ThreadedValue::Value(Ok(s)) => Some(s),
                _ => None,
            })
            .collect();

        assert_eq!(all.len(), 2);
        assert!(!all.iter().any(|s| s.id == "unrelated"));
    }
}
