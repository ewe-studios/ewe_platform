//! R2 storage via wasm-bindgen — calls Cloudflare R2 JS API directly.
//!
//! Wraps `R2Bucket` and implements `BlobStore` using R2's object storage.
//! Async `*_async` methods resolve JS Promises; trait methods call them via
//! `futures_lite::block_on`.

use js_sys::{ArrayBuffer, Uint8Array};
use wasm_bindgen::JsCast;
use wasm_bindgen_futures::JsFuture;

use crate::core::errors::{StorageError, StorageResult};
use crate::core::storage_provider::{BlobStore, StorageItemStream};
use crate::wasm::bindgen::{R2Bucket, R2Object};
use foundation_core::valtron::Stream;

// ===========================================================================
// R2WasmStorage
// ===========================================================================

/// R2 storage backend using wasm-bindgen JS bindings.
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

    fn stream_once<T: Send + 'static>(val: T) -> StorageItemStream<'static, T> {
        Box::new(std::iter::once(Stream::Next(Ok(val))))
    }
}

// ===========================================================================
// BlobStore
// ===========================================================================

impl BlobStore for R2WasmStorage {
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

impl R2WasmStorage {
    async fn put_blob_async(&self, key: &str, data: &[u8]) -> Result<(), StorageError> {
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

    async fn get_blob_async(&self, key: &str) -> Result<Option<Vec<u8>>, StorageError> {
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

    async fn delete_blob_async(&self, key: &str) -> Result<(), StorageError> {
        let object_key = self.object_key(key);

        let promise = self.bucket.delete(&object_key);
        let _result = JsFuture::from(promise)
            .await
            .map_err(|e| StorageError::Backend(format!("R2 delete failed: {e:?}")))?;

        Ok(())
    }

    async fn blob_exists_async(&self, key: &str) -> Result<bool, StorageError> {
        let object_key = self.object_key(key);

        let promise = self.bucket.head(&object_key);
        let result = JsFuture::from(promise)
            .await
            .map_err(|e| StorageError::Backend(format!("R2 head failed: {e:?}")))?;

        Ok(!result.is_null() && !result.is_undefined())
    }
}
