//! KV storage via wasm-bindgen — calls Cloudflare KV JS API directly.
//!
//! Wraps `KVNamespace` and implements `KeyValueStore` and `RateLimiterStore`
//! using Cloudflare Workers KV. Async `*_async` methods resolve JS Promises;
//! trait methods call them via `futures_lite::block_on`.

use js_sys::Object;
use wasm_bindgen::JsCast;
use wasm_bindgen_futures::JsFuture;

use crate::core::errors::{StorageError, StorageResult};
use crate::core::storage_provider::{
    BlobStore, DataValue, KeyValueStore, QueryStore, RateLimiterStore, SqlRow, StorageItemStream,
};
use crate::wasm::bindgen::KVNamespace;
use foundation_core::valtron::Stream;

// ===========================================================================
// KVWasmStorage
// ===========================================================================

/// KV storage backend using wasm-bindgen JS bindings.
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

    fn stream_once<T: Send + 'static>(val: T) -> StorageItemStream<'static, T> {
        Box::new(std::iter::once(Stream::Next(Ok(val))))
    }

    fn stream_many<T: Send + 'static>(vals: Vec<T>) -> StorageItemStream<'static, T> {
        Box::new(vals.into_iter().map(|v| Stream::Next(Ok(v))))
    }
}

// ===========================================================================
// KeyValueStore
// ===========================================================================

impl KeyValueStore for KVWasmStorage {
    fn get<'a, V: serde::de::DeserializeOwned + Send + 'static>(
        &'a self,
        key: &str,
    ) -> StorageResult<StorageItemStream<'a, Option<V>>> {
        let result = futures_lite::future::block_on(self.get_async(key))?;
        Ok(Self::stream_once(result))
    }

    fn set<V: serde::Serialize>(&self, key: &str, value: V) -> StorageResult<StorageItemStream<'_, ()>> {
        futures_lite::future::block_on(self.set_async(key, value))?;
        Ok(Self::stream_once(()))
    }

    fn delete(&self, key: &str) -> StorageResult<StorageItemStream<'_, ()>> {
        futures_lite::future::block_on(self.delete_async(key))?;
        Ok(Self::stream_once(()))
    }

    fn exists(&self, key: &str) -> StorageResult<StorageItemStream<'_, bool>> {
        let result = futures_lite::future::block_on(self.exists_async(key))?;
        Ok(Self::stream_once(result))
    }

    fn list_keys(&self, prefix: Option<&str>) -> StorageResult<StorageItemStream<'_, String>> {
        let result = futures_lite::future::block_on(self.list_keys_async(prefix))?;
        Ok(Self::stream_many(result))
    }
}

impl KVWasmStorage {
    async fn get_async<V: serde::de::DeserializeOwned + Send + 'static>(
        &self,
        key: &str,
    ) -> Result<Option<V>, StorageError> {
        let prefixed = self.prefixed_key(key);
        let promise = self.kv.get(&prefixed);
        let result = JsFuture::from(promise)
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

    async fn set_async<V: serde::Serialize>(
        &self,
        key: &str,
        value: V,
    ) -> Result<(), StorageError> {
        let prefixed = self.prefixed_key(key);
        let json = serde_json::to_string(&value)
            .map_err(|e| StorageError::Serialization(e.to_string()))?;

        let value_js = wasm_bindgen::JsValue::from_str(&json);
        let promise = self.kv.put(&prefixed, &value_js);
        JsFuture::from(promise)
            .await
            .map_err(|e| StorageError::Backend(format!("KV put failed: {e:?}")))?;

        Ok(())
    }

    async fn delete_async(&self, key: &str) -> Result<(), StorageError> {
        let prefixed = self.prefixed_key(key);
        let promise = self.kv.delete(&prefixed);
        JsFuture::from(promise)
            .await
            .map_err(|e| StorageError::Backend(format!("KV delete failed: {e:?}")))?;

        Ok(())
    }

    async fn exists_async(&self, key: &str) -> Result<bool, StorageError> {
        let prefixed = self.prefixed_key(key);
        let promise = self.kv.get(&prefixed);
        let result = JsFuture::from(promise)
            .await
            .map_err(|e| StorageError::Backend(format!("KV get failed: {e:?}")))?;

        Ok(!result.is_null() && !result.is_undefined())
    }

    async fn list_keys_async(&self, prefix: Option<&str>) -> Result<Vec<String>, StorageError> {
        // KV list returns { keys: [{name: "..."}], cursor: "..." }
        let opts = Object::new();

        if let Some(p) = prefix {
            js_sys::Reflect::set(&opts, &"prefix".into(), &wasm_bindgen::JsValue::from_str(p))
                .map_err(|e| StorageError::Backend(format!("KV list opts failed: {e:?}")))?;
        }

        let promise = self.kv.list(&opts.into());
        let result = JsFuture::from(promise)
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

// ===========================================================================
// QueryStore (not supported for KV)
// ===========================================================================

impl QueryStore for KVWasmStorage {
    fn query(
        &self,
        _sql: &str,
        _params: &[DataValue],
    ) -> StorageResult<StorageItemStream<'_, SqlRow>> {
        Err(StorageError::Generic(
            "QueryStore not supported for KVWasmStorage".to_string(),
        ))
    }

    fn execute(
        &self,
        _sql: &str,
        _params: &[DataValue],
    ) -> StorageResult<StorageItemStream<'_, u64>> {
        Err(StorageError::Generic(
            "QueryStore not supported for KVWasmStorage".to_string(),
        ))
    }

    fn execute_batch(&self, _sql: &str) -> StorageResult<StorageItemStream<'_, ()>> {
        Err(StorageError::Generic(
            "QueryStore not supported for KVWasmStorage".to_string(),
        ))
    }
}

// ===========================================================================
// RateLimiterStore
// ===========================================================================

impl RateLimiterStore for KVWasmStorage {
    fn check_rate_limit(
        &self,
        key: &str,
        max_count: u32,
        window_seconds: u64,
    ) -> StorageResult<StorageItemStream<'_, bool>> {
        let result = futures_lite::future::block_on(self.check_rate_limit_async(key, max_count, window_seconds))?;
        Ok(Self::stream_once(result))
    }

    fn record_rate_limit(&self, key: &str) -> StorageResult<StorageItemStream<'_, u32>> {
        let result = futures_lite::future::block_on(self.record_rate_limit_async(key))?;
        Ok(Self::stream_once(result))
    }

    fn reset_rate_limit(&self, key: &str) -> StorageResult<StorageItemStream<'_, ()>> {
        futures_lite::future::block_on(self.reset_rate_limit_async(key))?;
        Ok(Self::stream_once(()))
    }
}

impl KVWasmStorage {
    async fn check_rate_limit_async(
        &self,
        key: &str,
        max_count: u32,
        window_seconds: u64,
    ) -> Result<bool, StorageError> {
        let rate_key = format!("_rate_limit:{key}");

        let promise = self.kv.get(&self.prefixed_key(&rate_key));
        let result = JsFuture::from(promise)
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

    async fn record_rate_limit_async(&self, key: &str) -> Result<u32, StorageError> {
        let rate_key = format!("_rate_limit:{key}");
        let now = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_secs();

        // Get current entry
        let promise = self.kv.get(&self.prefixed_key(&rate_key));
        let result = JsFuture::from(promise)
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
            JsFuture::from(put_promise)
                .await
                .map_err(|e| StorageError::Backend(format!("KV put failed: {e:?}")))?;

            entry.count
        };

        if new_count == 1 {
            // First entry — need to save it
            let entry = serde_json::json!({ "count": 1, "window_start": now }).to_string();
            let value_js = wasm_bindgen::JsValue::from_str(&entry);
            let put_promise = self.kv.put(&self.prefixed_key(&rate_key), &value_js);
            JsFuture::from(put_promise)
                .await
                .map_err(|e| StorageError::Backend(format!("KV put failed: {e:?}")))?;
        }

        Ok(new_count)
    }

    async fn reset_rate_limit_async(&self, key: &str) -> Result<(), StorageError> {
        let rate_key = format!("_rate_limit:{key}");
        let promise = self.kv.delete(&self.prefixed_key(&rate_key));
        JsFuture::from(promise)
            .await
            .map_err(|e| StorageError::Backend(format!("KV delete failed: {e:?}")))?;

        Ok(())
    }
}

// ===========================================================================
// BlobStore (KV stores strings, so blobs are base64-encoded)
// ===========================================================================

impl BlobStore for KVWasmStorage {
    fn put_blob(&self, key: &str, data: &[u8]) -> StorageResult<StorageItemStream<'_, ()>> {
        futures_lite::future::block_on(self.put_blob_async(key, data))?;
        Ok(Self::stream_once(()))
    }

    fn get_blob(&self, key: &str) -> StorageResult<StorageItemStream<'_, Option<Vec<u8>>>> {
        let result = futures_lite::future::block_on(self.get_blob_async(key))?;
        Ok(Self::stream_once(result))
    }

    fn delete_blob(&self, key: &str) -> StorageResult<StorageItemStream<'_, ()>> {
        futures_lite::future::block_on(self.delete_blob_async(key))?;
        Ok(Self::stream_once(()))
    }

    fn blob_exists(&self, key: &str) -> StorageResult<StorageItemStream<'_, bool>> {
        let result = futures_lite::future::block_on(self.blob_exists_async(key))?;
        Ok(Self::stream_once(result))
    }
}

impl KVWasmStorage {
    async fn put_blob_async(&self, key: &str, data: &[u8]) -> Result<(), StorageError> {
        let encoded = base64::Engine::encode(&base64::engine::general_purpose::STANDARD, data);
        let json = serde_json::json!({ "type": "blob", "data": encoded }).to_string();
        let prefixed = self.prefixed_key(key);
        let value_js = wasm_bindgen::JsValue::from_str(&json);
        let promise = self.kv.put(&prefixed, &value_js);
        JsFuture::from(promise)
            .await
            .map_err(|e| StorageError::Backend(format!("KV put failed: {e:?}")))?;
        Ok(())
    }

    async fn get_blob_async(&self, key: &str) -> Result<Option<Vec<u8>>, StorageError> {
        let prefixed = self.prefixed_key(key);
        let promise = self.kv.get(&prefixed);
        let result = JsFuture::from(promise)
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

    async fn delete_blob_async(&self, key: &str) -> Result<(), StorageError> {
        let prefixed = self.prefixed_key(key);
        let promise = self.kv.delete(&prefixed);
        JsFuture::from(promise)
            .await
            .map_err(|e| StorageError::Backend(format!("KV delete failed: {e:?}")))?;
        Ok(())
    }

    async fn blob_exists_async(&self, key: &str) -> Result<bool, StorageError> {
        let prefixed = self.prefixed_key(key);
        let promise = self.kv.get(&prefixed);
        let result = JsFuture::from(promise)
            .await
            .map_err(|e| StorageError::Backend(format!("KV get failed: {e:?}")))?;
        Ok(!result.is_null() && !result.is_undefined())
    }
}
