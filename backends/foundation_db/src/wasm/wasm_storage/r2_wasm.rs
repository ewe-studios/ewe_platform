//! R2 storage via wasm-bindgen — calls Cloudflare R2 JS API directly.
//!
//! Wraps `R2Bucket` and implements `BlobStore` using R2's object storage.
//! There is no sync impl: a sync call would block the isolate waiting on a JS
//! Promise that could then never resolve.

use crate::core::errors::{StorageError, StorageResult};
use crate::core::storage_provider::BlobStore;
use crate::wasm::bindgen::{R2Bucket, R2Object};
use js_sys::{ArrayBuffer, Uint8Array};
use wasm_bindgen::JsCast;
use wasm_bindgen_futures::JsFuture;
use foundation_compact::SendWrapper;

// ===========================================================================
// R2WasmStorage
// ===========================================================================

/// R2 storage backend using wasm-bindgen JS bindings.
#[derive(Clone)]
pub struct R2WasmStorage {
    bucket: R2Bucket,
    prefix: String,
}

impl R2WasmStorage {
    /// Create a new R2 storage instance.
    #[must_use]
    pub fn new(bucket: R2Bucket, prefix: &str) -> Self {
        Self {
            bucket,
            prefix: prefix.to_string(),
        }
    }

    fn object_key(&self, key: &str) -> String {
        let safe_key = key.replace('/', ":");
        format!("{}/{}", self.prefix, safe_key)
    }

}

// ===========================================================================
// BlobStore
// ===========================================================================

impl R2WasmStorage {
    /// Put a blob into R2 storage.
    pub async fn put_blob_async(&self, key: &str, data: &[u8]) -> Result<(), StorageError> {
        let object_key = self.object_key(key);

        // Create Uint8Array from bytes
        let u8 = Uint8Array::new_with_length(data.len() as u32);
        u8.copy_from(data);

        let promise = self.bucket.put(&object_key, &u8.into());
        SendWrapper::new(JsFuture::from(promise))
            .await
            .map_err(|e| StorageError::Backend(format!("R2 put failed: {e:?}")))?;

        Ok(())
    }

    /// Get a blob from R2 storage.
    pub async fn get_blob_async(&self, key: &str) -> Result<Option<Vec<u8>>, StorageError> {
        let object_key = self.object_key(key);

        let promise = self.bucket.get(&object_key);
        let result = SendWrapper::new(JsFuture::from(promise))
            .await
            .map_err(|e| StorageError::Backend(format!("R2 get failed: {e:?}")))?;

        if result.is_null() || result.is_undefined() {
            return Ok(None);
        }

        // Result is R2Object with .arrayBuffer() method
        let obj = result.dyn_into::<R2Object>().map_err(|_| {
            StorageError::Backend("R2 get returned non-R2Object".to_string())
        })?;

        let buf_promise = obj.array_buffer();
        let buf = SendWrapper::new(JsFuture::from(buf_promise))
            .await
            .map_err(|e| StorageError::Backend(format!("R2 arrayBuffer failed: {e:?}")))?;

        let array_buf = buf.dyn_into::<ArrayBuffer>().map_err(|_| {
            StorageError::Backend("R2 arrayBuffer returned non-ArrayBuffer".to_string())
        })?;

        let u8 = Uint8Array::new(&array_buf);
        Ok(Some(u8.to_vec()))
    }

    /// Delete a blob from R2 storage.
    pub async fn delete_blob_async(&self, key: &str) -> Result<(), StorageError> {
        let object_key = self.object_key(key);

        let promise = self.bucket.delete(&object_key);
        let _result = SendWrapper::new(JsFuture::from(promise))
            .await
            .map_err(|e| StorageError::Backend(format!("R2 delete failed: {e:?}")))?;

        Ok(())
    }

    /// Check if a blob exists in R2 storage.
    pub async fn blob_exists_async(&self, key: &str) -> Result<bool, StorageError> {
        let object_key = self.object_key(key);

        let promise = self.bucket.head(&object_key);
        let result = SendWrapper::new(JsFuture::from(promise))
            .await
            .map_err(|e| StorageError::Backend(format!("R2 head failed: {e:?}")))?;

        Ok(!result.is_null() && !result.is_undefined())
    }
}

#[async_trait::async_trait]
impl BlobStore for R2WasmStorage {
    async fn put_blob(&self, key: &str, data: &[u8]) -> StorageResult<()> {
        foundation_compact::SendWrapper::new(async move { self.put_blob_async(key, data).await }).await
    }

    async fn get_blob(&self, key: &str) -> StorageResult<Option<Vec<u8>>> {
        foundation_compact::SendWrapper::new(async move { self.get_blob_async(key).await }).await
    }

    async fn delete_blob(&self, key: &str) -> StorageResult<()> {
        foundation_compact::SendWrapper::new(async move { self.delete_blob_async(key).await }).await
    }

    async fn blob_exists(&self, key: &str) -> StorageResult<bool> {
        foundation_compact::SendWrapper::new(async move { self.blob_exists_async(key).await }).await
    }
}
