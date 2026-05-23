//! Credential store backed by `D1WasmStorage`.
//!
//! WHY: `SessionManager<S: CredentialStore>` requires the sync `CredentialStore`
//! trait. `D1WasmStorage` implements sync `KeyValueStore` methods that route
//! through valtron iterators (`schedule_future` → `drive_non_send_iterator`).
//! With `JSThreadYielder` active, the valtron executor yields to the JS event
//! loop during waits, so these sync calls work correctly instead of deadlocking.
//!
//! WHAT: Wraps `Arc<D1WasmStorage>` and iterates valtron streams into plain
//! `Result` values, implementing `CredentialStore`.

use std::sync::Arc;

use foundation_core::valtron::Stream;
use foundation_db::{D1WasmStorage, KeyValueStore, StorageError};
use serde::{Deserialize, Serialize};

use crate::shared::credential_store::{AsyncCredentialStore, CredentialStore, CredentialStoreError};

/// Credential store backed by D1 via valtron-sync iterators.
pub struct D1CredentialStore {
    storage: Arc<D1WasmStorage>,
}

impl D1CredentialStore {
    pub fn new(storage: Arc<D1WasmStorage>) -> Self {
        Self { storage }
    }
}

fn drain_next<V>(stream: Result<impl Iterator<Item = Stream<Result<V, StorageError>, ()>>, StorageError>) -> Result<Option<V>, CredentialStoreError> {
    let stream = stream.map_err(CredentialStoreError::Storage)?;
    for item in stream {
        if let Stream::Next(result) = item {
            return result.map(Some).map_err(CredentialStoreError::Storage);
        }
    }
    Ok(None)
}

fn drain_unit(stream: Result<impl Iterator<Item = Stream<Result<(), StorageError>, ()>>, StorageError>) -> Result<(), CredentialStoreError> {
    let stream = stream.map_err(CredentialStoreError::Storage)?;
    for item in stream {
        if let Stream::Next(result) = item {
            return result.map_err(CredentialStoreError::Storage);
        }
    }
    Err(CredentialStoreError::Generic("Stream ended without result".to_string()))
}

fn drain_bool(stream: Result<impl Iterator<Item = Stream<Result<bool, StorageError>, ()>>, StorageError>) -> Result<bool, CredentialStoreError> {
    let stream = stream.map_err(CredentialStoreError::Storage)?;
    for item in stream {
        if let Stream::Next(result) = item {
            return result.map_err(CredentialStoreError::Storage);
        }
    }
    Err(CredentialStoreError::Generic("Stream ended without result".to_string()))
}

fn drain_collect<V>(stream: Result<impl Iterator<Item = Stream<Result<V, StorageError>, ()>>, StorageError>) -> Result<Vec<V>, CredentialStoreError> {
    let stream = stream.map_err(CredentialStoreError::Storage)?;
    let mut results = Vec::new();
    for item in stream {
        if let Stream::Next(Ok(v)) = item {
            results.push(v);
        } else if let Stream::Next(Err(e)) = item {
            return Err(CredentialStoreError::Storage(e));
        }
    }
    Ok(results)
}

impl CredentialStore for D1CredentialStore {
    fn get<V: for<'de> Deserialize<'de> + Send + 'static>(
        &self,
        key: &str,
    ) -> Result<Option<V>, CredentialStoreError> {
        drain_next(self.storage.get(key)).map(Option::flatten)
    }

    fn set<V: Serialize + Send + 'static>(
        &self,
        key: &str,
        value: V,
    ) -> Result<(), CredentialStoreError> {
        let stream = self.storage.set(key, value);
        drain_unit(stream)
    }

    fn delete(&self, key: &str) -> Result<(), CredentialStoreError> {
        let stream = self.storage.delete(key);
        drain_unit(stream)
    }

    fn exists(&self, key: &str) -> Result<bool, CredentialStoreError> {
        let stream = self.storage.exists(key);
        drain_bool(stream)
    }

    fn list_keys(&self, prefix: Option<&str>) -> Result<Vec<String>, CredentialStoreError> {
        let stream = self.storage.list_keys(prefix);
        drain_collect(stream)
    }
}

#[async_trait::async_trait(?Send)]
impl AsyncCredentialStore for D1CredentialStore {
    async fn get_async<V: for<'de> Deserialize<'de> + Send + 'static>(
        &self,
        key: &str,
    ) -> Result<Option<V>, CredentialStoreError> {
        self.get(key)
    }

    async fn set_async<V: Serialize + Send + 'static>(
        &self,
        key: &str,
        value: V,
    ) -> Result<(), CredentialStoreError> {
        self.set(key, value)
    }

    async fn delete_async(&self, key: &str) -> Result<(), CredentialStoreError> {
        self.delete(key)
    }

    async fn exists_async(&self, key: &str) -> Result<bool, CredentialStoreError> {
        self.exists(key)
    }

    async fn list_keys_async(&self, prefix: Option<&str>) -> Result<Vec<String>, CredentialStoreError> {
        self.list_keys(prefix)
    }
}
