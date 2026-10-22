//! `CredentialStore` impl for `D1WasmStorage`.
//!
//! WHY: `WasmSessionManager` uses `CredentialStore` for async session
//! operations. `D1WasmStorage` already has `*_async` methods — this bridges
//! them to the trait interface.

use std::sync::Arc;

use async_trait::async_trait;
use foundation_db::D1WasmStorage;

use crate::shared::credential_store::{CredentialStore, CredentialStoreError};

#[async_trait(?Send)]
impl CredentialStore for D1WasmStorage {
    async fn get_async<V: for<'de> serde::de::Deserialize<'de> + Send + 'static>(
        &self,
        key: &str,
    ) -> Result<Option<V>, CredentialStoreError> {
        D1WasmStorage::get_async::<V>(self, key)
            .await
            .map_err(CredentialStoreError::Storage)
    }

    async fn set_async<V: serde::Serialize + Send + 'static>(
        &self,
        key: &str,
        value: V,
    ) -> Result<(), CredentialStoreError> {
        D1WasmStorage::set_async(self, key, value)
            .await
            .map_err(CredentialStoreError::Storage)
    }

    async fn delete_async(&self, key: &str) -> Result<(), CredentialStoreError> {
        D1WasmStorage::delete_async(self, key)
            .await
            .map_err(CredentialStoreError::Storage)
    }

    async fn exists_async(&self, key: &str) -> Result<bool, CredentialStoreError> {
        D1WasmStorage::exists_async(self, key)
            .await
            .map_err(CredentialStoreError::Storage)
    }

    async fn list_keys_async(&self, prefix: Option<&str>) -> Result<Vec<String>, CredentialStoreError> {
        D1WasmStorage::list_keys_async(self, prefix)
            .await
            .map_err(CredentialStoreError::Storage)
    }
}

#[async_trait(?Send)]
impl CredentialStore for Arc<D1WasmStorage> {
    async fn get_async<V: for<'de> serde::de::Deserialize<'de> + Send + 'static>(
        &self,
        key: &str,
    ) -> Result<Option<V>, CredentialStoreError> {
        CredentialStore::get_async(self.as_ref(), key).await
    }

    async fn set_async<V: serde::Serialize + Send + 'static>(
        &self,
        key: &str,
        value: V,
    ) -> Result<(), CredentialStoreError> {
        CredentialStore::set_async(self.as_ref(), key, value).await
    }

    async fn delete_async(&self, key: &str) -> Result<(), CredentialStoreError> {
        CredentialStore::delete_async(self.as_ref(), key).await
    }

    async fn exists_async(&self, key: &str) -> Result<bool, CredentialStoreError> {
        CredentialStore::exists_async(self.as_ref(), key).await
    }

    async fn list_keys_async(&self, prefix: Option<&str>) -> Result<Vec<String>, CredentialStoreError> {
        CredentialStore::list_keys_async(self.as_ref(), prefix).await
    }
}
