//! R2 storage via wasm-bindgen — calls Cloudflare R2 JS API directly.
//!
//! Wraps `R2Bucket` and implements `BlobStore` using R2's object storage.
//! There is no sync impl: a sync call would block the isolate waiting on a JS
//! Promise that could then never resolve.

use crate::core::errors::{StorageError, StorageResult};
use crate::core::storage_provider::{
    BlobListStream, BlobPutCondition, BlobRange, BlobStore, BlobStream,
};
use crate::wasm::bindgen::{R2Bucket, R2MultipartUpload, R2Object};
use foundation_compact::SendWrapper;
use futures_core::Stream;
use js_sys::{Array, ArrayBuffer, Uint8Array};
use std::sync::atomic::{AtomicU32, Ordering};
use std::sync::Arc;
use wasm_bindgen::prelude::*;
use wasm_bindgen::JsCast;
use wasm_bindgen_futures::JsFuture;

// ===========================================================================
// R2WasmStorage
// ===========================================================================

/// R2 storage backend using wasm-bindgen JS bindings.
#[derive(Clone)]
pub struct R2WasmStorage {
    bucket: R2Bucket,
    prefix: String,
    /// The smallest part of a streamed upload, in bytes.
    part_size: usize,
    /// Parts written by the last streamed upload (a test hook).
    parts: Arc<AtomicU32>,
}

/// Default part size: 8 mebibytes (MiB). It is above the 5 MiB minimum of the
/// R2 multipart upload, and it is the default part size of the Amazon Web
/// Services (AWS) command-line tool.
pub const DEFAULT_R2_PART_SIZE: usize = 8 * 1024 * 1024;

/// The smallest part that R2 accepts, except the last: 5 MiB.
pub const MIN_R2_PART_SIZE: usize = 5 * 1024 * 1024;

impl R2WasmStorage {
    /// Create a new R2 storage instance.
    #[must_use]
    pub fn new(bucket: R2Bucket, prefix: &str) -> Self {
        Self {
            bucket,
            prefix: prefix.to_string(),
            part_size: DEFAULT_R2_PART_SIZE,
            parts: Arc::new(AtomicU32::new(0)),
        }
    }

    /// Set the smallest part of a streamed upload, in bytes.
    ///
    /// A value below [`MIN_R2_PART_SIZE`] becomes that minimum. The last part
    /// may be smaller.
    #[must_use]
    pub fn with_part_size(mut self, part_size: usize) -> Self {
        self.part_size = part_size.max(MIN_R2_PART_SIZE);
        self
    }

    /// The parts written by the last streamed upload.
    #[must_use]
    pub fn last_part_count(&self) -> u32 {
        self.parts.load(Ordering::Acquire)
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
    async fn put_blob_async(&self, key: &str, data: &[u8]) -> Result<(), StorageError> {
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
    async fn get_blob_async(&self, key: &str) -> Result<Option<Vec<u8>>, StorageError> {
        let object_key = self.object_key(key);

        let promise = self.bucket.get(&object_key);
        let result = SendWrapper::new(JsFuture::from(promise))
            .await
            .map_err(|e| StorageError::Backend(format!("R2 get failed: {e:?}")))?;

        if result.is_null() || result.is_undefined() {
            return Ok(None);
        }

        // Result is R2Object with .arrayBuffer() method. Workers R2 objects are
        // host objects without a JS class, so `dyn_into` (an `instanceof`
        // check) cannot see them; the binding guarantees the shape.
        let obj: R2Object = result.unchecked_into();

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
    async fn delete_blob_async(&self, key: &str) -> Result<(), StorageError> {
        let object_key = self.object_key(key);

        let promise = self.bucket.delete(&object_key);
        let _result = SendWrapper::new(JsFuture::from(promise))
            .await
            .map_err(|e| StorageError::Backend(format!("R2 delete failed: {e:?}")))?;

        Ok(())
    }

    /// Check if a blob exists in R2 storage.
    async fn blob_exists_async(&self, key: &str) -> Result<bool, StorageError> {
        let object_key = self.object_key(key);

        let promise = self.bucket.head(&object_key);
        let result = SendWrapper::new(JsFuture::from(promise))
            .await
            .map_err(|e| StorageError::Backend(format!("R2 head failed: {e:?}")))?;

        Ok(!result.is_null() && !result.is_undefined())
    }

    /// The object's `httpEtag`, or `None` when it is absent.
    async fn blob_etag_async(&self, key: &str) -> Result<Option<String>, StorageError> {
        let object_key = self.object_key(key);
        let promise = self.bucket.head(&object_key);
        let result = SendWrapper::new(JsFuture::from(promise))
            .await
            .map_err(|e| StorageError::Backend(format!("R2 head failed: {e:?}")))?;
        if result.is_null() || result.is_undefined() {
            return Ok(None);
        }
        let object: R2Object = result.unchecked_into();
        Ok(Some(object.http_etag()))
    }

    /// One byte range of an object. `end = None` reads to the end.
    async fn get_blob_range_async(
        &self,
        key: &str,
        range: BlobRange,
    ) -> Result<Vec<u8>, StorageError> {
        let object_key = self.object_key(key);
        let mut range_options = serde_json::json!({ "offset": range.start });
        if let Some(end) = range.end {
            range_options["length"] = serde_json::json!(end.saturating_sub(range.start));
        }
        let options =
            js_sys::JSON::parse(&serde_json::json!({ "range": range_options }).to_string())
                .map_err(|e| StorageError::Backend(format!("R2 range options failed: {e:?}")))?;
        let promise = self.bucket.get_with_options(&object_key, &options);
        let result = SendWrapper::new(JsFuture::from(promise))
            .await
            .map_err(|e| StorageError::Backend(format!("R2 get failed: {e:?}")))?;
        if result.is_null() || result.is_undefined() {
            return Ok(Vec::new());
        }
        let object: R2Object = result.unchecked_into();
        let buffer = SendWrapper::new(JsFuture::from(object.array_buffer()))
            .await
            .map_err(|e| StorageError::Backend(format!("R2 arrayBuffer failed: {e:?}")))?;
        let buffer = buffer.dyn_into::<ArrayBuffer>().map_err(|_| {
            StorageError::Backend("R2 arrayBuffer returned non-ArrayBuffer".to_string())
        })?;
        Ok(Uint8Array::new(&buffer).to_vec())
    }

    /// Ordered keys under `prefix`, after `start_after`, through the listing
    /// cursor. `limit` caps the keys (R2 pages with `cursor`).
    async fn list_blobs_after_async(
        &self,
        prefix: &str,
        start_after: Option<&str>,
        limit: Option<usize>,
    ) -> Result<Vec<String>, StorageError> {
        let strip = self.object_key("");
        let mut keys = Vec::new();
        let mut cursor: Option<String> = None;
        loop {
            let mut options = serde_json::json!({ "prefix": self.object_key(prefix) });
            if let Some(cursor) = &cursor {
                options["cursor"] = serde_json::json!(cursor);
            }
            let options = js_sys::JSON::parse(&options.to_string())
                .map_err(|e| StorageError::Backend(format!("R2 list options failed: {e:?}")))?;
            let promise = self.bucket.list(&options);
            let result = SendWrapper::new(JsFuture::from(promise))
                .await
                .map_err(|e| StorageError::Backend(format!("R2 list failed: {e:?}")))?;
            let objects: crate::wasm::bindgen::R2Objects = result.unchecked_into();
            let page = objects.objects();
            for index in 0..page.length() {
                let value = page.get(index);
                if value.is_null() || value.is_undefined() {
                    continue;
                }
                let object: R2Object = value.unchecked_into();
                if let Some(relative) = object.key().strip_prefix(&strip) {
                    keys.push(relative.replace(':', "/"));
                }
            }
            if !objects.truncated() {
                break;
            }
            match objects.cursor() {
                Some(next) if !next.is_empty() => cursor = Some(next),
                _ => break,
            }
        }
        keys.retain(|key| start_after.is_none_or(|start| key.as_str() > start));
        keys.sort();
        keys.dedup();
        if let Some(limit) = limit {
            keys.truncate(limit);
        }
        Ok(keys)
    }
}

/// Upload one multipart part and return the `R2UploadedPart` object.
async fn upload_one_part(
    upload: &R2MultipartUpload,
    number: u16,
    bytes: Vec<u8>,
) -> StorageResult<JsValue> {
    let value: JsValue = Uint8Array::from(bytes.as_slice()).into();
    let promise = upload.upload_part(number, &value);
    SendWrapper::new(JsFuture::from(promise))
        .await
        .map_err(|error| {
            StorageError::Backend(format!("R2 multipart part {number} failed: {error:?}"))
        })
}

#[async_trait::async_trait]
impl BlobStore for R2WasmStorage {
    async fn put_blob(&self, key: &str, data: &[u8]) -> StorageResult<()> {
        foundation_compact::SendWrapper::new(async move { self.put_blob_async(key, data).await })
            .await
    }

    /// Upload a body in R2 multipart parts.
    ///
    /// The stream is read one chunk at a time into a buffer of `part_size`,
    /// and each full part is uploaded before the next is read. The isolate
    /// holds one part plus one chunk, never the body (issue #940).
    async fn put_blob_stream(
        &self,
        key: &str,
        _length: Option<u64>,
        mut chunks: BlobStream,
    ) -> StorageResult<()> {
        let object_key = self.object_key(key);
        let part_size = self.part_size;
        let parts_count = Arc::clone(&self.parts);
        SendWrapper::new(async move {
            let promise = self.bucket.create_multipart_upload(&object_key);
            let upload: R2MultipartUpload = SendWrapper::new(JsFuture::from(promise))
                .await
                .map_err(|error| {
                    StorageError::Backend(format!("R2 multipart start failed: {error:?}"))
                })?
                .unchecked_into();
            parts_count.store(0, Ordering::Release);

            let mut parts = Array::new();
            let mut buffer: Vec<u8> = Vec::with_capacity(part_size);
            let mut number: u16 = 1;
            loop {
                let step = std::future::poll_fn(|cx| chunks.as_mut().poll_next(cx)).await;
                match step {
                    Some(Ok(chunk)) => {
                        buffer.extend_from_slice(&chunk);
                        while buffer.len() >= part_size {
                            let part: Vec<u8> = buffer.drain(..part_size).collect();
                            let uploaded = upload_one_part(&upload, number, part).await?;
                            parts.push(&uploaded);
                            parts_count.fetch_add(1, Ordering::AcqRel);
                            number = number.checked_add(1).ok_or_else(|| {
                                StorageError::Backend("too many multipart parts".to_string())
                            })?;
                        }
                    }
                    Some(Err(error)) => {
                        let _ = SendWrapper::new(JsFuture::from(upload.abort())).await;
                        return Err(error);
                    }
                    None => break,
                }
            }
            if !buffer.is_empty() {
                let uploaded = upload_one_part(&upload, number, buffer).await?;
                parts.push(&uploaded);
                parts_count.fetch_add(1, Ordering::AcqRel);
            } else if parts.length() == 0 {
                // An empty body has no valid multipart; abort and write it whole.
                let _ = SendWrapper::new(JsFuture::from(upload.abort())).await;
                return self.put_blob_async(key, &[]).await;
            }
            let promise = upload.complete(&parts.into());
            SendWrapper::new(JsFuture::from(promise))
                .await
                .map_err(|error| {
                    StorageError::Backend(format!("R2 multipart complete failed: {error:?}"))
                })?;
            Ok(())
        })
        .await
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

    async fn blob_etag(&self, key: &str) -> StorageResult<Option<String>> {
        foundation_compact::SendWrapper::new(async move { self.blob_etag_async(key).await }).await
    }

    async fn get_blob_range(&self, key: &str, range: BlobRange) -> StorageResult<BlobStream> {
        if range.end.is_some_and(|end| end <= range.start) {
            return Ok(Box::pin(futures_lite::stream::empty()));
        }
        let data = foundation_compact::SendWrapper::new(async move {
            self.get_blob_range_async(key, range).await
        })
        .await?;
        Ok(Box::pin(futures_lite::stream::iter(
            (!data.is_empty()).then_some(data).into_iter().map(Ok),
        )))
    }

    async fn put_blob_conditionally(
        &self,
        key: &str,
        data: &[u8],
        condition: BlobPutCondition,
    ) -> StorageResult<()> {
        // The trait's non-atomic check-then-write default, deliberately.
        // workerd's local R2 binding enforces `onlyIf` but resolves the promise
        // when the precondition fails, instead of rejecting, so a conditional
        // PUT there cannot report `PreconditionFailed`. A failed write is
        // invisible, and a read-after-write check is not atomic. The default
        // compares first, so it reports the failure. Re-check `onlyIf` on a
        // newer wrangler before overriding this.
        match &condition {
            BlobPutCondition::IfNoneMatch => {
                if self.blob_exists(key).await? {
                    return Err(StorageError::PreconditionFailed(key.to_string()));
                }
            }
            BlobPutCondition::IfMatch(expected) => {
                let current = self.blob_etag(key).await?;
                if current.as_deref() != Some(expected.as_str()) {
                    return Err(StorageError::PreconditionFailed(key.to_string()));
                }
            }
        }
        self.put_blob(key, data).await
    }

    async fn list_blobs_after(
        &self,
        prefix: &str,
        start_after: Option<&str>,
        limit: Option<usize>,
    ) -> StorageResult<BlobListStream> {
        let prefix = prefix.to_string();
        let start_after = start_after.map(str::to_string);
        let keys = foundation_compact::SendWrapper::new(async move {
            self.list_blobs_after_async(&prefix, start_after.as_deref(), limit)
                .await
        })
        .await?;
        Ok(Box::pin(futures_lite::stream::iter(
            keys.into_iter().map(Ok),
        )))
    }
}
