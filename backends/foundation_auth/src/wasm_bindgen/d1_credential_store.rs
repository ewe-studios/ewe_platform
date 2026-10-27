//! Credential store backed by `D1WasmStorage`.
//!
//! WHY: `SessionManager`'s async methods need an `CredentialStore`; this
//! is a shareable (`Arc`) handle to one over Cloudflare D1.
//!
//! WHAT: Wraps `Arc<D1WasmStorage>` and implements only `CredentialStore`.
//! There is no sync `CredentialStore` impl: a sync call would have to block the
//! Worker's event loop while D1's Promise can only resolve on that same loop.

use std::sync::Arc;

use foundation_db::D1WasmStorage;
use serde::{Deserialize, Serialize};

use crate::shared::credential_store::{CredentialStore, CredentialStoreError};

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
impl CredentialStore for D1CredentialStore {
    async fn get<V: for<'de> Deserialize<'de> + Send + 'static>(
        &self,
        key: &str,
    ) -> Result<Option<V>, CredentialStoreError> {
        CredentialStore::get(self.storage.as_ref(), key).await
    }

    async fn set<V: Serialize + Send + 'static>(
        &self,
        key: &str,
        value: V,
    ) -> Result<(), CredentialStoreError> {
        CredentialStore::set(self.storage.as_ref(), key, value).await
    }

    async fn delete(&self, key: &str) -> Result<(), CredentialStoreError> {
        CredentialStore::delete(self.storage.as_ref(), key).await
    }

    async fn exists(&self, key: &str) -> Result<bool, CredentialStoreError> {
        CredentialStore::exists(self.storage.as_ref(), key).await
    }

    async fn list_keys(&self, prefix: Option<&str>) -> Result<Vec<String>, CredentialStoreError> {
        CredentialStore::list_keys(self.storage.as_ref(), prefix).await
    }
}

#[async_trait::async_trait(?Send)]
impl CredentialStore for Arc<D1CredentialStore> {
    async fn get<V: for<'de> Deserialize<'de> + Send + 'static>(
        &self,
        key: &str,
    ) -> Result<Option<V>, CredentialStoreError> {
        CredentialStore::get(self.as_ref(), key).await
    }

    async fn set<V: Serialize + Send + 'static>(
        &self,
        key: &str,
        value: V,
    ) -> Result<(), CredentialStoreError> {
        CredentialStore::set(self.as_ref(), key, value).await
    }

    async fn delete(&self, key: &str) -> Result<(), CredentialStoreError> {
        CredentialStore::delete(self.as_ref(), key).await
    }

    async fn exists(&self, key: &str) -> Result<bool, CredentialStoreError> {
        CredentialStore::exists(self.as_ref(), key).await
    }

    async fn list_keys(&self, prefix: Option<&str>) -> Result<Vec<String>, CredentialStoreError> {
        CredentialStore::list_keys(self.as_ref(), prefix).await
    }
}
