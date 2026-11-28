//! The Cloudflare R2 block tier for the SQL virtual file system (VFS).
//!
//! [`R2BlockStore`] implements the block-store contract over the Cloudflare
//! Workers R2 binding. A Worker has no local file system, so the SQL VFS keeps
//! its hot rows in Durable Object SQLite and puts its immutable blocks in R2.
//!
//! A block key is deterministic, so a retried `put` writes the same bytes to the
//! same key. A retry is safe.
//!
//! The Wasm bindings are single-threaded. Each operation wraps its JavaScript
//! promise in [`SendWrapper`], the same pattern that the R2 storage backend in
//! `foundation_db` uses.

use foundation_compact::SendWrapper;
use foundation_core::durable::sql::{BlockError, BlockFuture, BlockStore};
use foundation_db::wasm::bindgen::{R2Bucket, R2Object};
use js_sys::{ArrayBuffer, Uint8Array};
use std::fmt;
use wasm_bindgen::JsCast;
use wasm_bindgen_futures::JsFuture;

/// A JavaScript failure, as a typed block error.
#[derive(Debug)]
struct R2Error(String);

impl fmt::Display for R2Error {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.0)
    }
}

impl core::error::Error for R2Error {}

fn backend(message: impl fmt::Display) -> BlockError {
    BlockError::Io(Box::new(R2Error(message.to_string())))
}

/// An object store for blocks over one Cloudflare R2 bucket.
pub struct R2BlockStore {
    bucket: R2Bucket,
    prefix: String,
}

impl R2BlockStore {
    /// Build the block store over a bucket.
    ///
    /// `prefix` is the object-key root. A host sets it to a value that names the
    /// environment, for example `durable/production`.
    #[must_use]
    pub fn new(bucket: R2Bucket, prefix: impl Into<String>) -> Self {
        Self {
            bucket,
            prefix: normalize(prefix.into()),
        }
    }

    /// Return the object-key root.
    #[must_use]
    pub fn prefix(&self) -> &str {
        &self.prefix
    }

    fn object_key(&self, key: &str) -> String {
        if self.prefix.is_empty() {
            key.to_string()
        } else {
            format!("{}/{}", self.prefix, key)
        }
    }
}

fn normalize(prefix: String) -> String {
    prefix.trim_matches('/').to_string()
}

fn to_uint8(bytes: &[u8]) -> Uint8Array {
    let data = Uint8Array::new_with_length(u32::try_from(bytes.len()).unwrap_or(u32::MAX));
    data.copy_from(bytes);
    data
}

impl BlockStore for R2BlockStore {
    fn put(&self, key: &str, bytes: Vec<u8>) -> BlockFuture<'_, ()> {
        let object_key = self.object_key(key);
        Box::pin(SendWrapper::new(async move {
            let body = to_uint8(&bytes);
            let promise = self.bucket.put(&object_key, &body.into());
            JsFuture::from(promise)
                .await
                .map_err(|error| backend(format!("R2 put failed: {error:?}")))?;
            Ok(())
        }))
    }

    fn get_range(&self, key: &str, start: u64, len: usize) -> BlockFuture<'_, Vec<u8>> {
        let object_key = self.object_key(key);
        Box::pin(SendWrapper::new(async move {
            if len == 0 {
                return Ok(Vec::new());
            }
            let options = js_sys::JSON::parse(&format!(
                "{{\"range\":{{\"offset\":{start},\"length\":{len}}}}}"
            ))
            .map_err(|error| backend(format!("R2 range options failed: {error:?}")))?;
            let promise = self.bucket.get_with_options(&object_key, &options);
            let result = JsFuture::from(promise)
                .await
                .map_err(|error| backend(format!("R2 get failed: {error:?}")))?;
            if result.is_null() || result.is_undefined() {
                return Err(BlockError::NotFound);
            }
            let object: R2Object = result.unchecked_into();
            let buffer = JsFuture::from(object.array_buffer())
                .await
                .map_err(|error| backend(format!("R2 arrayBuffer failed: {error:?}")))?;
            let buffer = buffer.dyn_into::<ArrayBuffer>().map_err(|_| {
                backend("R2 arrayBuffer returned a value that is not an ArrayBuffer")
            })?;
            let bytes = Uint8Array::new(&buffer).to_vec();
            if bytes.len() != len {
                return Err(BlockError::Invalid("short block range"));
            }
            Ok(bytes)
        }))
    }
}

impl fmt::Debug for R2BlockStore {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("R2BlockStore")
            .field("prefix", &self.prefix)
            .finish_non_exhaustive()
    }
}
