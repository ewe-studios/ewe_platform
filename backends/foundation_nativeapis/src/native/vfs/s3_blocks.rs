//! The Amazon Simple Storage Service (S3) block tier for the SQL virtual file
//! system (VFS).
//!
//! [`S3BlockStore`] implements the block-store contract over one HTTP object
//! store. The SQL VFS (`foundation_core::durable::sql::SqlVfs`) writes immutable
//! blocks through this tier and reads byte ranges back.
//!
//! A block key is deterministic, so a retried `put` writes the same bytes to the
//! same key. A retry is therefore safe.
//!
//! The store is generic over the object transport, so a test drives it with a
//! stub transport and a live run uses [`NetioObjectTransport`].
//!
//! [`NetioObjectTransport`]: super::object_netio::NetioObjectTransport

use super::object_http::{HttpObjectStore, ObjectTransport};
use crate::shared::vfs::durable::{ObjectError, ObjectStore};
use foundation_core::durable::shared::sql::block::{BlockError, BlockFuture, BlockStore};

/// An object store that serves as the block tier of the SQL VFS.
pub struct S3BlockStore<T: ObjectTransport> {
    store: HttpObjectStore<T>,
}

impl<T: ObjectTransport> S3BlockStore<T> {
    /// Build the block store over an HTTP object store.
    #[must_use]
    pub fn new(store: HttpObjectStore<T>) -> Self {
        Self { store }
    }

    /// Return the object store under the block store.
    #[must_use]
    pub fn store(&self) -> &HttpObjectStore<T> {
        &self.store
    }
}

impl<T: ObjectTransport> core::fmt::Debug for S3BlockStore<T> {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        f.debug_struct("S3BlockStore")
            .field("store", &self.store)
            .finish()
    }
}

fn block_error(error: ObjectError) -> BlockError {
    match error {
        ObjectError::NotFound => BlockError::NotFound,
        other => BlockError::Io(Box::new(other)),
    }
}

impl<T: ObjectTransport> BlockStore for S3BlockStore<T> {
    fn put(&self, key: &str, bytes: Vec<u8>) -> BlockFuture<'_, ()> {
        let key = key.to_string();
        Box::pin(async move {
            self.store
                .put(&key, bytes)
                .await
                .map(|_version| ())
                .map_err(block_error)
        })
    }

    fn get_range(&self, key: &str, start: u64, len: usize) -> BlockFuture<'_, Vec<u8>> {
        let key = key.to_string();
        Box::pin(async move {
            self.store
                .get_range(&key, start, len)
                .await
                .map_err(block_error)
        })
    }
}
