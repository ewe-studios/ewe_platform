//! Browser key-value storage. Each store owns one application namespace.
use crate::core::errors::{StorageError, StorageResult};
use crate::core::storage_provider::{AsyncListStream, KeyValueStore};
use foundation_compact::SendWrapper;
use futures_util::{stream, TryStreamExt};
use serde::{de::DeserializeOwned, Serialize};
use std::{collections::BTreeMap, num::NonZeroU32};
use wasm_bindgen::prelude::*;

#[wasm_bindgen(module = "/src/wasm/browser_storage.js")]
extern "C" {
    #[wasm_bindgen(catch, js_name = browserOperation)]
    async fn operation(
        indexed: bool,
        namespace: &str,
        op: &str,
        key: &str,
        value: Option<String>,
        after: Option<String>,
        limit: u32,
    ) -> Result<JsValue, JsValue>;
}

// The browser transports each value as a string to preserve large integers.
fn encode_records<K: AsRef<str>, V: Serialize>(
    records: &BTreeMap<K, V>,
) -> StorageResult<Vec<(&str, String)>> {
    records
        .iter()
        .map(|(key, value)| Ok((key.as_ref(), serde_json::to_string(value)?)))
        .collect()
}

/// Select the browser database or the origin's local settings store.
#[derive(Clone, Copy, Debug)]
pub enum BrowserBackend {
    IndexedDb,
    LocalStorage,
}

/// Store serialized records under independent keys. Clones share the namespace.
#[derive(Clone, Debug)]
pub struct BrowserStore {
    backend: BrowserBackend,
    namespace: String,
    page_size: NonZeroU32,
    legacy_limit: NonZeroU32,
}

impl BrowserStore {
    /// Open a namespace. Key streams fetch 100 names per page by default.
    /// Limit legacy records to one mebibyte, as for native legacy files.
    pub fn new(backend: BrowserBackend, namespace: impl Into<String>) -> Self {
        Self {
            backend,
            namespace: namespace.into(),
            page_size: const { NonZeroU32::new(100).unwrap() },
            legacy_limit: const { NonZeroU32::new(1024 * 1024).unwrap() },
        }
    }

    /// Set the maximum number of names kept in a key stream page.
    #[must_use]
    pub fn with_page_size(mut self, size: NonZeroU32) -> Self {
        self.page_size = size;
        self
    }

    /// Set the maximum encoded size of a legacy record before it enters WebAssembly.
    #[must_use]
    pub fn with_legacy_limit(mut self, bytes: NonZeroU32) -> Self {
        self.legacy_limit = bytes;
        self
    }

    /// Read an unscoped `localStorage` record written by an earlier application.
    ///
    /// # Errors
    /// Return an access, size, or decoding error.
    pub async fn legacy_record<V: DeserializeOwned>(&self, key: &str) -> StorageResult<Option<V>> {
        let value = self.request("legacy", key, None, None).await?;
        value
            .as_string()
            .map(|json| serde_json::from_str(&json).map_err(Into::into))
            .transpose()
    }

    /// Import records and their receipts in one `IndexedDB` transaction.
    ///
    /// # Errors
    /// Return a configuration, encoding, or transaction error.
    pub async fn import_once<V: Serialize>(
        &self,
        destination: &str,
        receipts: &str,
        records: &BTreeMap<String, V>,
    ) -> StorageResult<()> {
        if !matches!(self.backend, BrowserBackend::IndexedDb) {
            return Err(StorageError::Migration("Import requires IndexedDB".into()));
        }
        if destination.starts_with(receipts) || receipts.starts_with(destination) {
            return Err(StorageError::Migration(
                "Overlapping record and receipt prefixes".into(),
            ));
        }
        self.request(
            "import",
            destination,
            Some(serde_json::to_string(&encode_records(records)?)?),
            Some(receipts.into()),
        )
        .await?;
        Ok(())
    }

    /// Apply independent inserts, updates, and removals. `IndexedDB` commits them in one transaction.
    /// An update skips a record that a concurrent writer removes.
    ///
    /// # Errors
    /// Return an encoding or storage error.
    pub async fn change_records<V: Serialize>(
        &self,
        prefix: &str,
        inserts: &BTreeMap<&str, &V>,
        updates: &BTreeMap<&str, &V>,
        removals: &[&str],
    ) -> StorageResult<()> {
        self.request(
            "change",
            prefix,
            Some(serde_json::to_string(&(
                encode_records(inserts)?,
                encode_records(updates)?,
                removals,
            ))?),
            None,
        )
        .await?;
        Ok(())
    }

    async fn request(
        &self,
        op: &str,
        key: &str,
        value: Option<String>,
        after: Option<String>,
    ) -> StorageResult<JsValue> {
        let limit = if op == "legacy" {
            self.legacy_limit
        } else {
            self.page_size
        };
        SendWrapper::new(operation(
            matches!(self.backend, BrowserBackend::IndexedDb),
            &self.namespace,
            op,
            key,
            value,
            after,
            limit.get(),
        ))
        .await
        .map_err(|error| {
            let field = |name: &str| {
                js_sys::Reflect::get(&error, &name.into())
                    .ok()
                    .and_then(|v| v.as_string())
            };
            StorageError::Browser {
                name: field("name").unwrap_or_else(|| "Error".into()),
                message: field("message").unwrap_or_else(|| format!("{error:?}")),
            }
        })
    }
}

#[async_trait::async_trait]
impl KeyValueStore for BrowserStore {
    async fn get<V: DeserializeOwned + Send + 'static>(
        &self,
        key: &str,
    ) -> StorageResult<Option<V>> {
        let value = self.request("get", key, None, None).await?;
        if value.is_null() {
            return Ok::<_, StorageError>(None);
        }
        let json = value
            .as_string()
            .ok_or_else(|| StorageError::Deserialization("Expected a stored string".into()))?;
        Ok(Some(serde_json::from_str(&json)?))
    }

    async fn set<V: Serialize + Send + 'static>(&self, key: &str, value: V) -> StorageResult<()> {
        self.request("set", key, Some(serde_json::to_string(&value)?), None)
            .await?;
        Ok(())
    }

    async fn exists(&self, key: &str) -> StorageResult<bool> {
        self.request("exists", key, None, None)
            .await?
            .as_bool()
            .ok_or_else(|| StorageError::Backend("Expected a presence flag".into()))
    }

    async fn delete(&self, key: &str) -> StorageResult<()> {
        self.request("delete", key, None, None).await?;
        Ok(())
    }

    async fn list_keys(&self, prefix: Option<&str>) -> StorageResult<AsyncListStream> {
        let state = (self.clone(), prefix.unwrap_or_default().to_owned(), None);
        let pages = stream::try_unfold(state, |(store, prefix, after)| async move {
            let response = store.request("list", &prefix, None, after).await?;
            let keys = response
                .dyn_into::<js_sys::Array>()
                .map_err(|_| StorageError::Backend("Expected a page of keys".into()))?
                .iter()
                .map(|v| {
                    v.as_string().ok_or_else(|| {
                        StorageError::Deserialization("Expected a string key".into())
                    })
                })
                .collect::<StorageResult<Vec<_>>>()?;
            let Some(last) = keys.last().cloned() else {
                return Ok::<_, StorageError>(None);
            };
            Ok(Some((
                stream::iter(keys.into_iter().map(Ok)),
                (store, prefix, Some(last)),
            )))
        });
        Ok(AsyncListStream::new(pages.try_flatten()))
    }
}
