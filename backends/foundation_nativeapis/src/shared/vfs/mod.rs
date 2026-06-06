pub mod async_traits;
pub mod error;
pub mod exec_async;
pub mod memory_delta;
pub mod memory_fs;
pub mod overlay_fs;
pub mod path_utils;
pub mod sync_bridge;
pub mod traits;
pub mod types;

#[cfg(feature = "vfs-sqlite")]
pub mod libsql_delta;

#[cfg(feature = "vfs-turso")]
pub mod turso_delta;

pub use error::{VfsError, VfsResult};
pub use memory_delta::MemoryDelta;
pub use memory_fs::MemoryFs;
pub use overlay_fs::OverlayFileSystem;
pub use async_traits::{
    AsyncDeltaStore, AsyncSeekableVfsFile, AsyncVfsDirectory, AsyncVfsFile, AsyncVfsFileSystem,
};
pub use exec_async::exec_async;
pub use sync_bridge::{LocalSeekableFile, SyncDirectory, SyncFile, SyncFs};
pub use traits::{DeltaStore, SeekableVfsFile, VfsDirectory, VfsFile, VfsFileSystem};
pub use types::{
    Checksum, OpenMode, SeekFrom, VfsCapabilities, VfsDirEntry, VfsEntryState, VfsFileType,
    VfsMetadata,
};

#[cfg(feature = "vfs-sqlite")]
pub use libsql_delta::{LibsqlDelta, SyncLibsqlDelta};

#[cfg(feature = "vfs-turso")]
pub use turso_delta::TursoDelta;
