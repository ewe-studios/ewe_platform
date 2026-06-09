pub mod async_traits;
pub mod error;
pub mod exec_async;
pub mod memory_delta;
pub mod memory_fs;
pub mod observable_fs;
pub mod overlay_fs;
pub mod path_utils;
pub mod sync_bridge;
pub mod traits;
pub mod types;

#[cfg(feature = "vfs-sqlite")]
pub mod libsql_delta;

#[cfg(feature = "vfs-turso")]
pub mod turso_delta;

#[cfg(feature = "vfs-d1")]
pub mod d1_delta;

#[cfg(feature = "vfs-r2")]
pub mod r2_delta;

#[cfg(feature = "vfs-arrow")]
pub mod arrow;

#[cfg(all(feature = "vfs-ipc", any(target_os = "linux", target_os = "macos", target_os = "windows")))]
pub mod ipc_client;

#[cfg(all(feature = "vfs-ipc", any(target_os = "linux", target_os = "macos", target_os = "windows")))]
pub mod ipc_daemon;

#[cfg(feature = "vfs-ipc")]
pub mod ipc_messages;

pub use error::{VfsError, VfsResult};
pub use memory_delta::MemoryDelta;
pub use memory_fs::MemoryFs;
pub use observable_fs::{ObservableFs, ObservableFile, ObservableSeekableFile, VfsEvent};
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
