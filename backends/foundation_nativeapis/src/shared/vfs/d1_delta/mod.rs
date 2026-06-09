//! Cloudflare D1 edge SQLite DeltaStore.
//!
//! D1 is Cloudflare's serverless SQLite, accessed via HTTP API.
//! Uses foundation_db's `D1Store` which implements `KeyValueStore`, `QueryStore`,
//! `BlobStore`, and `StateStore` — both sync and async.
//!
//! D1Store makes blocking HTTP calls directly — no valtron bridge needed.
//!
//! ## Configuration
//!
//! | Env var | Required | Description |
//! |---------|----------|-------------|
//! | `CF_ACCOUNT_ID` | Yes | Cloudflare account ID |
//! | `CF_API_TOKEN` | Yes | Cloudflare API token with D1 access |
//! | `CF_D1_DATABASE_ID` | Yes | D1 database UUID |

pub mod chunking;
pub mod schema;
pub mod types;

pub use types::{D1ChunkRef, D1FileMeta, D1FsConfig};

/// D1Delta — Cloudflare D1 edge SQLite DeltaStore.
///
/// Wraps foundation_db's `D1Store` which implements sync `KeyValueStore`,
/// `QueryStore`, `BlobStore`, `StateStore`. No valtron bridge needed —
/// D1Store makes blocking HTTP calls directly.
#[cfg(feature = "vfs-d1")]
pub mod d1_delta_impl {
    use foundation_db::native::D1Store;

    use crate::shared::vfs::error::{VfsError, VfsResult};

    /// D1Delta — sync DeltaStore backed by Cloudflare D1 HTTP API.
    pub struct D1Delta {
        store: D1Store,
        config: super::types::D1FsConfig,
    }

    impl D1Delta {
        /// Create from environment variables.
        /// Required: `CF_ACCOUNT_ID`, `CF_API_TOKEN`, `CF_D1_DATABASE_ID`
        pub fn from_env() -> VfsResult<Self> {
            let account_id = std::env::var("CF_ACCOUNT_ID").map_err(|e| VfsError::Backend {
                message: format!("CF_ACCOUNT_ID not set: {e}"),
            })?;
            let api_token = std::env::var("CF_API_TOKEN").map_err(|e| VfsError::Backend {
                message: format!("CF_API_TOKEN not set: {e}"),
            })?;
            let database_id = std::env::var("CF_D1_DATABASE_ID").map_err(|e| VfsError::Backend {
                message: format!("CF_D1_DATABASE_ID not set: {e}"),
            })?;

            let store = D1Store::new_kv(&api_token, &account_id, &database_id, "vfs");

            Ok(Self {
                store,
                config: super::types::D1FsConfig::default(),
            })
        }

        /// Create with explicit credentials.
        pub fn new(account_id: &str, database_id: &str, api_token: &str) -> VfsResult<Self> {
            let store = D1Store::new_kv(api_token, account_id, database_id, "vfs");

            Ok(Self {
                store,
                config: super::types::D1FsConfig::default(),
            })
        }
    }
}

#[cfg(feature = "vfs-d1")]
pub use d1_delta_impl::D1Delta;

