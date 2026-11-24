//! Browser key-value storage. Each store owns one application namespace.
use crate::core::errors::{StorageError, StorageResult};
use crate::core::storage_provider::{AsyncListStream, KeyValueStore};
use foundation_compact::SendWrapper;
use futures_util::{TryStreamExt, stream};
use serde::{Serialize, de::DeserializeOwned};
use std::num::NonZeroU32;
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
}

impl BrowserStore {
    /// Open a namespace. Key streams fetch 100 names per page by default.
    pub fn new(backend: BrowserBackend, namespace: impl Into<String>) -> Self {
        Self {
            backend,
            namespace: namespace.into(),
            page_size: const { NonZeroU32::new(100).unwrap() },
        }
    }

    /// Set the maximum number of names kept in a key stream page.
    #[must_use]
    pub fn with_page_size(mut self, size: NonZeroU32) -> Self {
        self.page_size = size;
        self
    }

    async fn request(
        &self,
        op: &str,
        key: &str,
        value: Option<String>,
        after: Option<String>,
    ) -> StorageResult<JsValue> {
        SendWrapper::new(operation(
            matches!(self.backend, BrowserBackend::IndexedDb),
            &self.namespace,
            op,
            key,
            value,
            after,
            self.page_size.get(),
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
