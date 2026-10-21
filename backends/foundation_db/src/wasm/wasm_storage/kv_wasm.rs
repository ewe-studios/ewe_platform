//! KV storage via wasm-bindgen — calls Cloudflare KV JS API directly.
//!
//! Wraps `KVNamespace` and implements the `Async*` key-value, rate-limiter and
//! blob traits using Cloudflare Workers KV (plus an `AsyncQueryStore` that
//! reports SQL as unsupported). There are no sync impls: a sync call would
//! block the isolate waiting on a JS Promise that could then never resolve.

use crate::core::errors::{StorageError, StorageResult};
use crate::core::storage_provider::{
    AsyncBlobStore, AsyncKeyValueStore, AsyncListStream, AsyncQueryStore, AsyncQueryStream,
    AsyncRateLimiterStore, DataValue,
};
use crate::wasm::bindgen::KVNamespace;
use js_sys::Object;
use wasm_bindgen::JsCast;
use wasm_bindgen_futures::JsFuture;
use foundation_compact::SendWrapper;

// ===========================================================================
// KVWasmStorage
// ===========================================================================

/// KV storage backend using wasm-bindgen JS bindings.
#[derive(Clone)]
pub struct KVWasmStorage {
    kv: KVNamespace,
    prefix: String,
}

impl KVWasmStorage {
    /// Create a new KV storage instance.
    #[must_use]
    pub fn new(kv: KVNamespace, prefix: &str) -> Self {
        Self {
            kv,
            prefix: prefix.to_string(),
        }
    }

    fn prefixed_key(&self, key: &str) -> String {
        format!("{}:{}", self.prefix, key)
    }

}

// ===========================================================================
// KeyValueStore
// ===========================================================================

impl KVWasmStorage {
    /// Get a value by key.
    pub async fn get_async<V: serde::de::DeserializeOwned + Send + 'static>(
        &self,
        key: &str,
    ) -> Result<Option<V>, StorageError> {
        let prefixed = self.prefixed_key(key);
        let promise = self.kv.get(&prefixed);
        let result = SendWrapper::new(JsFuture::from(promise))
            .await
            .map_err(|e| StorageError::Backend(format!("KV get failed: {e:?}")))?;

        if result.is_null() || result.is_undefined() {
            return Ok(None);
        }

        let value = result.as_string().ok_or_else(|| {
            StorageError::Serialization("KV value is not a string".to_string())
        })?;

        let deserialized: V = serde_json::from_str(&value)
            .map_err(|e| StorageError::Serialization(e.to_string()))?;

        Ok(Some(deserialized))
    }

    /// Set a key-value pair.
    pub async fn set_async<V: serde::Serialize>(
        &self,
        key: &str,
        value: V,
    ) -> Result<(), StorageError> {
        let prefixed = self.prefixed_key(key);
        let json = serde_json::to_string(&value)
            .map_err(|e| StorageError::Serialization(e.to_string()))?;

        let value_js = wasm_bindgen::JsValue::from_str(&json);
        let promise = self.kv.put(&prefixed, &value_js);
        SendWrapper::new(JsFuture::from(promise))
            .await
            .map_err(|e| StorageError::Backend(format!("KV put failed: {e:?}")))?;

        Ok(())
    }

    /// Delete a key.
    pub async fn delete_async(&self, key: &str) -> Result<(), StorageError> {
        let prefixed = self.prefixed_key(key);
        let promise = self.kv.delete(&prefixed);
        SendWrapper::new(JsFuture::from(promise))
            .await
            .map_err(|e| StorageError::Backend(format!("KV delete failed: {e:?}")))?;

        Ok(())
    }

    /// Check if a key exists.
    pub async fn exists_async(&self, key: &str) -> Result<bool, StorageError> {
        let prefixed = self.prefixed_key(key);
        let promise = self.kv.get(&prefixed);
        let result = SendWrapper::new(JsFuture::from(promise))
            .await
            .map_err(|e| StorageError::Backend(format!("KV get failed: {e:?}")))?;

        Ok(!result.is_null() && !result.is_undefined())
    }

    /// List all keys with optional prefix filter.
    pub async fn list_keys_async(&self, prefix: Option<&str>) -> Result<Vec<String>, StorageError> {
        // KV list returns { keys: [{name: "..."}], cursor: "..." }
        let opts = Object::new();

        if let Some(p) = prefix {
            js_sys::Reflect::set(&opts, &"prefix".into(), &wasm_bindgen::JsValue::from_str(p))
                .map_err(|e| StorageError::Backend(format!("KV list opts failed: {e:?}")))?;
        }

        let promise = self.kv.list(&opts.into());
        let result = SendWrapper::new(JsFuture::from(promise))
            .await
            .map_err(|e| StorageError::Backend(format!("KV list failed: {e:?}")))?;

        let obj = result.dyn_into::<js_sys::Object>().map_err(|_| {
            StorageError::Backend("KV list returned non-object".to_string())
        })?;

        let keys_arr = js_sys::Reflect::get(&obj, &"keys".into())
            .ok()
            .and_then(|v| v.dyn_into::<js_sys::Array>().ok())
            .unwrap_or_default();

        let mut keys = Vec::new();
        for i in 0..keys_arr.length() {
            let entry = keys_arr.get(i);
            if let Ok(entry_obj) = entry.dyn_into::<js_sys::Object>() {
                if let Some(name) = js_sys::Reflect::get(&entry_obj, &"name".into())
                    .ok()
                    .and_then(|v| v.as_string())
                {
                    // Strip prefix to return clean key
                    let clean_key = name.strip_prefix(&format!("{}:", self.prefix))
                        .unwrap_or(&name)
                        .to_string();
                    keys.push(clean_key);
                }
            }
        }

        Ok(keys)
    }
}

#[async_trait::async_trait]
impl AsyncKeyValueStore for KVWasmStorage {
    async fn get_async<V: serde::de::DeserializeOwned + Send + 'static>(&self, key: &str) -> StorageResult<Option<V>> {
        foundation_compact::SendWrapper::new(async move { self.get_async(key).await }).await
    }

    async fn set_async<V: serde::Serialize + Send + 'static>(&self, key: &str, value: V) -> StorageResult<()> {
        foundation_compact::SendWrapper::new(async move { self.set_async(key, value).await }).await
    }

    async fn delete_async(&self, key: &str) -> StorageResult<()> {
        foundation_compact::SendWrapper::new(async move { self.delete_async(key).await }).await
    }

    async fn exists_async(&self, key: &str) -> StorageResult<bool> {
        foundation_compact::SendWrapper::new(async move { self.exists_async(key).await }).await
    }

    async fn list_keys_async(&self, prefix: Option<&str>) -> StorageResult<AsyncListStream> {
        let keys = foundation_compact::SendWrapper::new(async move {
            self.list_keys_async(prefix).await
        }).await?;
        Ok(AsyncListStream::new(futures_lite::stream::iter(
            keys.into_iter().map(Ok),
        )))
    }
}

// ===========================================================================
// QueryStore (not supported for KV)
// ===========================================================================

/// Workers KV has no SQL engine: every query method fails with
/// `StorageError::Generic`.
#[async_trait::async_trait]
impl AsyncQueryStore for KVWasmStorage {
    async fn query_async(
        &self,
        _sql: &str,
        _params: &[DataValue],
    ) -> StorageResult<AsyncQueryStream> {
        Err(StorageError::Generic(
            "QueryStore not supported for KVWasmStorage".to_string(),
        ))
    }

    async fn execute_async(&self, _sql: &str, _params: &[DataValue]) -> StorageResult<u64> {
        Err(StorageError::Generic(
            "QueryStore not supported for KVWasmStorage".to_string(),
        ))
    }

    async fn execute_batch_async(&self, _sql: &str) -> StorageResult<()> {
        Err(StorageError::Generic(
            "QueryStore not supported for KVWasmStorage".to_string(),
        ))
    }
}

// ===========================================================================
// RateLimiterStore
// ===========================================================================

impl KVWasmStorage {
    /// Check if a rate limit key is allowed.
    pub async fn check_rate_limit_async(
        &self,
        key: &str,
        max_count: u32,
        window_seconds: u64,
    ) -> Result<bool, StorageError> {
        let rate_key = format!("_rate_limit:{key}");

        let promise = self.kv.get(&self.prefixed_key(&rate_key));
        let result = SendWrapper::new(JsFuture::from(promise))
            .await
            .map_err(|e| StorageError::Backend(format!("KV get failed: {e:?}")))?;

        let now = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_secs();

        let allowed = if result.is_null() || result.is_undefined() {
            true
        } else {
            let json = result.as_string().ok_or_else(|| {
                StorageError::Serialization("KV rate limit value is not a string".to_string())
            })?;

            #[derive(serde::Deserialize)]
            struct Entry {
                count: u32,
                window_start: u64,
            }

            let entry: Entry = serde_json::from_str(&json)
                .map_err(|e| StorageError::Serialization(e.to_string()))?;

            if entry.window_start < now.saturating_sub(window_seconds) {
                true
            } else {
                entry.count < max_count
            }
        };

        Ok(allowed)
    }

    /// Record a rate-limited action.
    pub async fn record_rate_limit_async(&self, key: &str) -> Result<u32, StorageError> {
        let rate_key = format!("_rate_limit:{key}");
        let now = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_secs();

        // Get current entry
        let promise = self.kv.get(&self.prefixed_key(&rate_key));
        let result = SendWrapper::new(JsFuture::from(promise))
            .await
            .map_err(|e| StorageError::Backend(format!("KV get failed: {e:?}")))?;

        let new_count = if result.is_null() || result.is_undefined() {
            1
        } else {
            let json = result.as_string().ok_or_else(|| {
                StorageError::Serialization("KV rate limit value is not a string".to_string())
            })?;

            #[derive(serde::Deserialize, serde::Serialize)]
            struct Entry {
                count: u32,
                window_start: u64,
            }

            let mut entry: Entry = serde_json::from_str(&json)
                .map_err(|e| StorageError::Serialization(e.to_string()))?;
            entry.count += 1;
            entry.window_start = now;

            // Save updated
            let new_json = serde_json::to_string(&entry)
                .map_err(|e| StorageError::Serialization(e.to_string()))?;
            let value_js = wasm_bindgen::JsValue::from_str(&new_json);
            let put_promise = self.kv.put(&self.prefixed_key(&rate_key), &value_js);
            SendWrapper::new(JsFuture::from(put_promise))
                .await
                .map_err(|e| StorageError::Backend(format!("KV put failed: {e:?}")))?;

            entry.count
        };

        if new_count == 1 {
            // First entry — need to save it
            let entry = serde_json::json!({ "count": 1, "window_start": now }).to_string();
            let value_js = wasm_bindgen::JsValue::from_str(&entry);
            let put_promise = self.kv.put(&self.prefixed_key(&rate_key), &value_js);
            SendWrapper::new(JsFuture::from(put_promise))
                .await
                .map_err(|e| StorageError::Backend(format!("KV put failed: {e:?}")))?;
        }

        Ok(new_count)
    }

    /// Reset a rate limit key.
    pub async fn reset_rate_limit_async(&self, key: &str) -> Result<(), StorageError> {
        let rate_key = format!("_rate_limit:{key}");
        let promise = self.kv.delete(&self.prefixed_key(&rate_key));
        SendWrapper::new(JsFuture::from(promise))
            .await
            .map_err(|e| StorageError::Backend(format!("KV delete failed: {e:?}")))?;

        Ok(())
    }
}

#[async_trait::async_trait]
impl AsyncRateLimiterStore for KVWasmStorage {
    async fn check_rate_limit_async(
        &self,
        key: &str,
        max_count: u32,
        window_seconds: u64,
    ) -> StorageResult<bool> {
        foundation_compact::SendWrapper::new(async move {
            self.check_rate_limit_async(key, max_count, window_seconds).await
        }).await
    }

    async fn record_rate_limit_async(&self, key: &str) -> StorageResult<u32> {
        foundation_compact::SendWrapper::new(async move { self.record_rate_limit_async(key).await }).await
    }

    async fn reset_rate_limit_async(&self, key: &str) -> StorageResult<()> {
        foundation_compact::SendWrapper::new(async move { self.reset_rate_limit_async(key).await }).await
    }
}

// ===========================================================================
// BlobStore (KV stores strings, so blobs are base64-encoded)
// ===========================================================================

impl KVWasmStorage {
    /// Put a blob into KV storage (base64-encoded).
    pub async fn put_blob_async(&self, key: &str, data: &[u8]) -> Result<(), StorageError> {
        let encoded = base64::Engine::encode(&base64::engine::general_purpose::STANDARD, data);
        let json = serde_json::json!({ "type": "blob", "data": encoded }).to_string();
        let prefixed = self.prefixed_key(key);
        let value_js = wasm_bindgen::JsValue::from_str(&json);
        let promise = self.kv.put(&prefixed, &value_js);
        SendWrapper::new(JsFuture::from(promise))
            .await
            .map_err(|e| StorageError::Backend(format!("KV put failed: {e:?}")))?;
        Ok(())
    }

    /// Get a blob from KV storage (base64-decoded).
    pub async fn get_blob_async(&self, key: &str) -> Result<Option<Vec<u8>>, StorageError> {
        let prefixed = self.prefixed_key(key);
        let promise = self.kv.get(&prefixed);
        let result = SendWrapper::new(JsFuture::from(promise))
            .await
            .map_err(|e| StorageError::Backend(format!("KV get failed: {e:?}")))?;

        if result.is_null() || result.is_undefined() {
            return Ok(None);
        }

        let json = result.as_string().ok_or_else(|| {
            StorageError::Serialization("KV blob value is not a string".to_string())
        })?;

        let wrapper: serde_json::Value = serde_json::from_str(&json)
            .map_err(|e| StorageError::Serialization(e.to_string()))?;

        let is_blob = wrapper.get("type").and_then(|v| v.as_str()) == Some("blob");
        if !is_blob {
            return Ok(None);
        }

        let encoded = wrapper
            .get("data")
            .and_then(|v| v.as_str())
            .ok_or_else(|| StorageError::Serialization("missing data field in blob wrapper".to_string()))?;

        let decoded = base64::Engine::decode(&base64::engine::general_purpose::STANDARD, encoded)
            .map_err(|e| StorageError::Backend(format!("Base64 decode failed: {e}")))?;

        Ok(Some(decoded))
    }

    /// Delete a blob from KV storage.
    pub async fn delete_blob_async(&self, key: &str) -> Result<(), StorageError> {
        let prefixed = self.prefixed_key(key);
        let promise = self.kv.delete(&prefixed);
        SendWrapper::new(JsFuture::from(promise))
            .await
            .map_err(|e| StorageError::Backend(format!("KV delete failed: {e:?}")))?;
        Ok(())
    }

    /// Check if a blob exists in KV storage.
    pub async fn blob_exists_async(&self, key: &str) -> Result<bool, StorageError> {
        let prefixed = self.prefixed_key(key);
        let promise = self.kv.get(&prefixed);
        let result = SendWrapper::new(JsFuture::from(promise))
            .await
            .map_err(|e| StorageError::Backend(format!("KV get failed: {e:?}")))?;
        Ok(!result.is_null() && !result.is_undefined())
    }
}

#[async_trait::async_trait]
impl AsyncBlobStore for KVWasmStorage {
    async fn put_blob_async(&self, key: &str, data: &[u8]) -> StorageResult<()> {
        foundation_compact::SendWrapper::new(async move { self.put_blob_async(key, data).await }).await
    }

    async fn get_blob_async(&self, key: &str) -> StorageResult<Option<Vec<u8>>> {
        foundation_compact::SendWrapper::new(async move { self.get_blob_async(key).await }).await
    }

    async fn delete_blob_async(&self, key: &str) -> StorageResult<()> {
        foundation_compact::SendWrapper::new(async move { self.delete_blob_async(key).await }).await
    }

    async fn blob_exists_async(&self, key: &str) -> StorageResult<bool> {
        foundation_compact::SendWrapper::new(async move { self.blob_exists_async(key).await }).await
    }
}
