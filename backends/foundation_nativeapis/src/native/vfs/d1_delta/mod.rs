#![cfg(feature = "vfs-d1")]

//! Cloudflare D1 edge `SQLite` delta store.
//!
//! [`D1Delta`] is a [`BlobDelta`] over `foundation_db`'s async `D1Store`: it
//! implements the async VFS traits (`AsyncVfsFileSystem`, `AsyncDeltaStore`)
//! and awaits the store. Sync callers wrap it in
//! [`SyncFs`](crate::shared::vfs::sync_bridge::SyncFs). Storage layout: see
//! [`blob_delta`](super::blob_delta).

pub mod chunking;
pub mod schema;
pub mod types;

pub use types::{D1ChunkRef, D1FileMeta, D1FsConfig};

use foundation_db::D1Store;
use foundation_errstacks::ErrorTrace;

use super::blob_delta::{BlobDelta, BlobDirectory, BlobFile};
use crate::shared::vfs::error::{VfsError, VfsResult};

/// Cloudflare D1 delta store: a [`BlobDelta`] over `D1Store`.
pub type D1Delta = BlobDelta<D1Store>;
/// A file in a [`D1Delta`].
pub type AsyncD1File = BlobFile<D1Store>;
/// A directory in a [`D1Delta`].
pub type AsyncD1Directory = BlobDirectory<D1Store>;

fn env(name: &str) -> VfsResult<String> {
    std::env::var(name).map_err(|e| {
        ErrorTrace::new(VfsError::Backend {
            message: format!("{name} not set: {e}"),
        })
    })
}

impl BlobDelta<D1Store> {
    /// A D1 delta from `CF_ACCOUNT_ID`, `CF_API_TOKEN` and `CF_D1_DATABASE_ID`.
    ///
    /// # Errors
    ///
    /// Returns [`VfsError::Backend`] if a variable is missing.
    pub fn from_env() -> VfsResult<Self> {
        let account_id = env("CF_ACCOUNT_ID")?;
        let api_token = env("CF_API_TOKEN")?;
        let database_id = env("CF_D1_DATABASE_ID")?;
        Ok(Self::from_store(D1Store::new_kv(&api_token, &account_id, &database_id, "vfs")))
    }

    /// A D1 delta over the given database (Cloudflare's API endpoint).
    ///
    /// # Errors
    ///
    /// Infallible today; `Result` for parity with the other constructors.
    pub fn new(api_token: &str, account_id: &str, database_id: &str) -> VfsResult<Self> {
        Self::new_with_base_url(api_token, account_id, database_id, None)
    }

    /// A D1 delta against `base_url` (e.g. a local emulator) or Cloudflare.
    ///
    /// # Errors
    ///
    /// Infallible today; `Result` for parity with the other constructors.
    pub fn new_with_base_url(
        api_token: &str,
        account_id: &str,
        database_id: &str,
        base_url: Option<&str>,
    ) -> VfsResult<Self> {
        let store = match base_url {
            Some(url) => D1Store::new_kv_with_base_url(api_token, account_id, database_id, "vfs", url),
            None => D1Store::new_kv(api_token, account_id, database_id, "vfs"),
        };
        Ok(Self::from_store(store))
    }
}
