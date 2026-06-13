//! R2 storage via wasm-bindgen — calls Cloudflare R2 JS API directly.
//!
//! Wraps `R2Bucket` and implements `BlobStore` using R2's object storage.
//! Async `*_async` methods are the source of truth (JS Promises);
//! sync trait methods delegate via `schedule_future`.

use crate::core::errors::{StorageError, StorageResult};
use crate::core::storage_provider::{AsyncBlobStore, BlobStore, StorageItemStream};
use crate::wasm::bindgen::{R2Bucket, R2Object};
use foundation_core::valtron::{execute, from_future, Stream, StreamIteratorExt};
use js_sys::{ArrayBuffer, Uint8Array};
use wasm_bindgen::JsCast;
use wasm_bindgen_futures::JsFuture;

/// Schedule a future, returning a boxed stream.
fn schedule_future<T: 'static, E: Into<StorageError> + 'static, F>(
    future: F,
) -> StorageResult<StorageItemStream<'static, T>>
where
    F: std::future::Future<Output = Result<T, E>> + 'static,
{
    let task = from_future(future);
    let stream = execute(task, None)
        .map_err(|e| StorageError::Backend(format!("Valtron scheduling failed: {e}")))?;
    Ok(Box::new(
        stream
            .map_done(|r: Result<T, E>| r.map_err(Into::into))
            .map_pending(|_| ()),
    ))
}

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

    #[allow(dead_code)]
    fn stream_once<T: Send + 'static>(val: T) -> StorageItemStream<'static, T> {
        val
    }
}

// ===========================================================================
// BlobStore
// ===========================================================================

impl BlobStore for R2WasmStorage {
    fn put_blob(&self, key: &str, data: &[u8]) -> StorageResult<()> {
        let this = self.clone();
        let key = key.to_string();
        let data = data.to_vec();
        schedule_future(async move {
            this.put_blob_async(&key, &data).await
        })
    }

    fn get_blob(&self, key: &str) -> StorageResult<Option<Vec<u8>>> {
        let this = self.clone();
        let key = key.to_string();
        schedule_future(async move {
            this.get_blob_async(&key).await
        })
    }

    fn delete_blob(&self, key: &str) -> StorageResult<()> {
        let this = self.clone();
        let key = key.to_string();
        schedule_future(async move {
            this.delete_blob_async(&key).await
        })
    }

    fn blob_exists(&self, key: &str) -> StorageResult<bool> {
        let this = self.clone();
        let key = key.to_string();
        schedule_future(async move {
            this.blob_exists_async(&key).await
        })
    }
}

impl R2WasmStorage {
    /// Put a blob into R2 storage.
    pub async fn put_blob_async(&self, key: &str, data: &[u8]) -> Result<(), StorageError> {
        let object_key = self.object_key(key);

        // Create Uint8Array from bytes
        let u8 = Uint8Array::new_with_length(data.len() as u32);
        u8.copy_from(data);

        let promise = self.bucket.put(&object_key, &u8.into());
        JsFuture::from(promise)
            .await
            .map_err(|e| StorageError::Backend(format!("R2 put failed: {e:?}")))?;

        Ok(())
    }

    /// Get a blob from R2 storage.
    pub async fn get_blob_async(&self, key: &str) -> Result<Option<Vec<u8>>, StorageError> {
        let object_key = self.object_key(key);

        let promise = self.bucket.get(&object_key);
        let result = JsFuture::from(promise)
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
        let buf = JsFuture::from(buf_promise)
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
        let _result = JsFuture::from(promise)
            .await
            .map_err(|e| StorageError::Backend(format!("R2 delete failed: {e:?}")))?;

        Ok(())
    }

    /// Check if a blob exists in R2 storage.
    pub async fn blob_exists_async(&self, key: &str) -> Result<bool, StorageError> {
        let object_key = self.object_key(key);

        let promise = self.bucket.head(&object_key);
        let result = JsFuture::from(promise)
            .await
            .map_err(|e| StorageError::Backend(format!("R2 head failed: {e:?}")))?;

        Ok(!result.is_null() && !result.is_undefined())
    }
}

#[async_trait::async_trait(?Send)]
impl AsyncBlobStore for R2WasmStorage {
    async fn put_blob_async(&self, key: &str, data: &[u8]) -> StorageResult<()> {
        self.put_blob_async(key, data).await
    }

    async fn get_blob_async(&self, key: &str) -> StorageResult<Option<Vec<u8>>> {
        self.get_blob_async(key).await
    }

    async fn delete_blob_async(&self, key: &str) -> StorageResult<()> {
        self.delete_blob_async(key).await
    }

    async fn blob_exists_async(&self, key: &str) -> StorageResult<bool> {
        self.blob_exists_async(key).await
    }
}
