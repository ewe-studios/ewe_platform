#![cfg(feature = "vfs-d1")]

//! Cloudflare D1 edge SQLite DeltaStore (scaffolded).
//!
//! Types and schema are defined. Full `VfsFileSystem` + `DeltaStore` impls
//! pending: `D1Store` uses valtron streams requiring `collect_one` consumption,
//! and `VfsMetadata` needs `Serialize`/`Deserialize` derives.

pub mod chunking;
pub mod schema;
pub mod types;

pub use types::{D1ChunkRef, D1FileMeta, D1FsConfig};
