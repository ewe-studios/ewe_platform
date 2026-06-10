#![cfg(feature = "vfs-r2")]

//! Cloudflare R2 S3-compatible DeltaStore (scaffolded).
//!
//! Types and key layouts are defined. Full `VfsFileSystem` + `DeltaStore` impls
//! pending: `R2Store` uses valtron streams requiring `collect_one` consumption,
//! and `VfsMetadata` needs `Serialize`/`Deserialize` derives.

pub mod types;

pub use types::{KeyLayout, KeyLayoutAdapter, PathKeyLayout, R2FsConfig, R2ObjectMeta};
