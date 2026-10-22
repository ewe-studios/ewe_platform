#![cfg(feature = "vfs-r2")]

//! Cloudflare R2 (S3-compatible) delta store.
//!
//! [`R2Delta`] is a [`BlobDelta`] over `foundation_db`'s async `R2Store`: it
//! implements the async VFS traits (`AsyncVfsFileSystem`, `AsyncDeltaStore`)
//! and awaits the store. Sync callers wrap it in
//! [`SyncFs`](crate::shared::vfs::sync_bridge::SyncFs). Storage layout: see
//! [`blob_delta`](super::blob_delta).

pub mod types;

pub use types::{KeyLayout, KeyLayoutAdapter, PathKeyLayout, R2FsConfig, R2ObjectMeta};

use foundation_db::R2Store;
use foundation_errstacks::ErrorTrace;

use super::blob_delta::{BlobDelta, BlobDirectory, BlobFile};
use crate::shared::vfs::error::{VfsError, VfsResult};

/// Cloudflare R2 delta store: a [`BlobDelta`] over `R2Store`.
pub type R2Delta = BlobDelta<R2Store>;
/// A file in an [`R2Delta`].
pub type AsyncR2File = BlobFile<R2Store>;
/// A directory in an [`R2Delta`].
pub type AsyncR2Directory = BlobDirectory<R2Store>;

fn env(name: &str) -> VfsResult<String> {
    std::env::var(name).map_err(|e| {
        ErrorTrace::new(VfsError::Backend {
            message: format!("{name} not set: {e}"),
        })
    })
}

impl BlobDelta<R2Store> {
    /// An R2 delta from `CF_ACCOUNT_ID`, `CF_API_TOKEN` and `CF_R2_BUCKET`.
    ///
    /// # Errors
    ///
    /// Returns [`VfsError::Backend`] if a variable is missing.
    pub fn from_env() -> VfsResult<Self> {
        let account_id = env("CF_ACCOUNT_ID")?;
        let api_token = env("CF_API_TOKEN")?;
        let bucket = env("CF_R2_BUCKET")?;
        Ok(Self::from_store(R2Store::new_blob(&api_token, &account_id, &bucket, "vfs")))
    }

    /// An R2 delta over the given bucket (Cloudflare's API endpoint).
    ///
    /// # Errors
    ///
    /// Infallible today; `Result` for parity with the other constructors.
    pub fn new(api_token: &str, account_id: &str, bucket: &str) -> VfsResult<Self> {
        Self::new_with_base_url(api_token, account_id, bucket, None)
    }

    /// An R2 delta against `base_url` (e.g. a local emulator) or Cloudflare.
    ///
    /// # Errors
    ///
    /// Infallible today; `Result` for parity with the other constructors.
    pub fn new_with_base_url(
        api_token: &str,
        account_id: &str,
        bucket: &str,
        base_url: Option<&str>,
    ) -> VfsResult<Self> {
        let store = match base_url {
            Some(url) => R2Store::new_blob_with_base_url(api_token, account_id, bucket, "vfs", url),
            None => R2Store::new_blob(api_token, account_id, bucket, "vfs"),
        };
        Ok(Self::from_store(store))
    }
}
