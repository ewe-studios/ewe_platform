pub mod async_traits;
pub mod dynfs;
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

#[cfg(feature = "vfs-search")]
pub mod search;

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

#[cfg(feature = "vfs-search")]
pub use search::{
    CascadingVfsSearcher, InCodeVfsSearcher, VfsSearchKind, VfsSearchMatch, VfsSearcher,
    vfs_searcher,
};

#[cfg(all(feature = "vfs-search", not(target_family = "wasm")))]
pub use search::{CliSearcher, native_vfs_searcher};


#[cfg(feature = "vfs-fjall")]
pub mod fjall_fs;

#[cfg(feature = "vfs-fjall")]
pub use fjall_fs::{FjallFs, FjallDelta, FjallVfsConfig};
