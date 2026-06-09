//! Cloudflare R2 S3-compatible DeltaStore.
//!
//! R2 is Cloudflare's S3-compatible object storage, accessed via HTTP API.
//! Uses foundation_db's `R2Store` which implements sync `BlobStore` and `StateStore`.
//!
//! R2Store makes blocking HTTP calls directly — no valtron bridge needed.
//!
//! ## Configuration
//!
//! | Env var | Required | Description |
//! |---------|----------|-------------|
//! | `CF_ACCOUNT_ID` | Yes | Cloudflare account ID |
//! | `CF_API_TOKEN` | Yes | Cloudflare API token with R2 access |
//! | `CF_R2_BUCKET` | Yes | R2 bucket name |

pub mod types;

pub use types::{KeyLayout, KeyLayoutAdapter, PathKeyLayout, R2FsConfig, R2ObjectMeta};

/// R2Delta — Cloudflare R2 S3-compatible DeltaStore.
///
/// Wraps foundation_db's `R2Store` which implements sync `BlobStore`, `StateStore`.
/// No valtron bridge needed — R2Store makes blocking HTTP calls directly.
#[cfg(feature = "vfs-r2")]
pub mod r2_delta_impl {
    use foundation_db::native::R2Store;

    use crate::shared::vfs::error::{VfsError, VfsResult};

    /// R2Delta — sync DeltaStore backed by Cloudflare R2 HTTP API.
    pub struct R2Delta {
        store: R2Store,
        config: super::types::R2FsConfig,
    }

    impl R2Delta {
        /// Create from environment variables.
        /// Required: `CF_ACCOUNT_ID`, `CF_API_TOKEN`, `CF_R2_BUCKET`
        pub fn from_env() -> VfsResult<Self> {
            let account_id = std::env::var("CF_ACCOUNT_ID").map_err(|e| VfsError::Backend {
                message: format!("CF_ACCOUNT_ID not set: {e}"),
            })?;
            let api_token = std::env::var("CF_API_TOKEN").map_err(|e| VfsError::Backend {
                message: format!("CF_API_TOKEN not set: {e}"),
            })?;
            let bucket = std::env::var("CF_R2_BUCKET").map_err(|e| VfsError::Backend {
                message: format!("CF_R2_BUCKET not set: {e}"),
            })?;

            let store = R2Store::new_blob(&api_token, &account_id, &bucket, "vfs");

            Ok(Self {
                store,
                config: super::types::R2FsConfig::default(),
            })
        }

        /// Create with explicit credentials.
        pub fn new(account_id: &str, bucket: &str, api_token: &str) -> VfsResult<Self> {
            let store = R2Store::new_blob(api_token, account_id, bucket, "vfs");

            Ok(Self {
                store,
                config: super::types::R2FsConfig::default(),
            })
        }
    }
}

#[cfg(feature = "vfs-r2")]
pub use r2_delta_impl::R2Delta;

