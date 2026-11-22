//! Virtual file system (VFS) traits and shared types.
//!
//! The crate `foundation_nativeapis` re-exports these items in its `vfs` module and
//! adds the implementations (`MemoryFs`, `NativeFs`, the delta stores). The
//! split exists so that a crate such as `foundation_db` can name the trait
//! without a package cycle: `foundation_nativeapis` has an optional dependency
//! on `foundation_db` for its Cloudflare delta stores.
//!
//! Written in ASD-STE100 Simplified Technical English (STE).

#[cfg(feature = "arrow")]
pub mod arrow;

pub mod async_traits;
pub mod empty;
pub mod error;
pub mod types;

pub use async_traits::{
    AsyncDeltaStore, AsyncSeekableVfsFile, AsyncVfsDirectory, AsyncVfsFile, AsyncVfsFileSystem,
};
pub use empty::{EmptyDirectory, EmptyFile, EmptyFs, EmptySeekableFile};
pub use error::{VfsError, VfsResult};
pub use types::{
    Checksum, OpenMode, SeekFrom, VfsCapabilities, VfsDirEntry, VfsEntryState, VfsFileType,
    VfsMetadata,
};
