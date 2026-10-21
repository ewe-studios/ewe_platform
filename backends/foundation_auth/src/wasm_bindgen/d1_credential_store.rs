//! Credential store backed by `D1WasmStorage`.
//!
//! WHY: `SessionManager`'s async methods need an `AsyncCredentialStore`; this
//! is a shareable (`Arc`) handle to one over Cloudflare D1.
//!
//! WHAT: Wraps `Arc<D1WasmStorage>` and implements only `AsyncCredentialStore`.
//! There is no sync `CredentialStore` impl: a sync call would have to block the
//! Worker's event loop while D1's Promise can only resolve on that same loop.

use std::sync::Arc;

use foundation_db::D1WasmStorage;
use serde::{Deserialize, Serialize};

use crate::shared::credential_store::{AsyncCredentialStore, CredentialStoreError};

/// Async credential store backed by D1.
pub struct D1CredentialStore {
    storage: Arc<D1WasmStorage>,
}

impl D1CredentialStore {
    pub fn new(storage: Arc<D1WasmStorage>) -> Self {
        Self { storage }
    }
}

#[async_trait::async_trait(?Send)]
impl AsyncCredentialStore for D1CredentialStore {
    async fn get_async<V: for<'de> Deserialize<'de> + Send + 'static>(
        &self,
        key: &str,
    ) -> Result<Option<V>, CredentialStoreError> {
        AsyncCredentialStore::get_async(self.storage.as_ref(), key).await
    }

    async fn set_async<V: Serialize + Send + 'static>(
        &self,
        key: &str,
        value: V,
    ) -> Result<(), CredentialStoreError> {
        AsyncCredentialStore::set_async(self.storage.as_ref(), key, value).await
    }

    async fn delete_async(&self, key: &str) -> Result<(), CredentialStoreError> {
        AsyncCredentialStore::delete_async(self.storage.as_ref(), key).await
    }

    async fn exists_async(&self, key: &str) -> Result<bool, CredentialStoreError> {
        AsyncCredentialStore::exists_async(self.storage.as_ref(), key).await
    }

    async fn list_keys_async(&self, prefix: Option<&str>) -> Result<Vec<String>, CredentialStoreError> {
        AsyncCredentialStore::list_keys_async(self.storage.as_ref(), prefix).await
    }
}

#[async_trait::async_trait(?Send)]
impl AsyncCredentialStore for Arc<D1CredentialStore> {
    async fn get_async<V: for<'de> Deserialize<'de> + Send + 'static>(
        &self,
        key: &str,
    ) -> Result<Option<V>, CredentialStoreError> {
        AsyncCredentialStore::get_async(self.as_ref(), key).await
    }

    async fn set_async<V: Serialize + Send + 'static>(
        &self,
        key: &str,
        value: V,
    ) -> Result<(), CredentialStoreError> {
        AsyncCredentialStore::set_async(self.as_ref(), key, value).await
    }

    async fn delete_async(&self, key: &str) -> Result<(), CredentialStoreError> {
        AsyncCredentialStore::delete_async(self.as_ref(), key).await
    }

    async fn exists_async(&self, key: &str) -> Result<bool, CredentialStoreError> {
        AsyncCredentialStore::exists_async(self.as_ref(), key).await
    }

    async fn list_keys_async(&self, prefix: Option<&str>) -> Result<Vec<String>, CredentialStoreError> {
        AsyncCredentialStore::list_keys_async(self.as_ref(), prefix).await
    }
}
