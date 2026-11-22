// The traits and shared types live in `foundation_vfs` so that a crate such as
// `foundation_db` can name the async trait without a package cycle:
// `foundation_nativeapis` has optional Cloudflare delta stores that depend on
// `foundation_db`. These re-exports keep the old paths
// (`foundation_nativeapis::vfs::AsyncVfsFileSystem`, `...::error::VfsError`)
// unchanged.
pub use foundation_vfs::{async_traits, error, types};

pub mod dynfs;
pub mod exec_async;
pub mod memory_delta;
pub mod memory_fs;
pub mod observable_fs;
pub mod overlay_fs;
pub mod path_utils;
pub mod sync_bridge;
pub mod traits;

#[cfg(feature = "vfs-search")]
pub mod search;

#[cfg(all(
    feature = "vfs-ipc",
    any(target_os = "linux", target_os = "macos", target_os = "windows")
))]
pub mod ipc_client;

#[cfg(all(
    feature = "vfs-ipc",
    any(target_os = "linux", target_os = "macos", target_os = "windows")
))]
pub mod ipc_daemon;

#[cfg(feature = "vfs-ipc")]
pub mod ipc_messages;

pub use foundation_vfs::async_traits::{
    AsyncDeltaStore, AsyncSeekableVfsFile, AsyncVfsDirectory, AsyncVfsFile, AsyncVfsFileSystem,
};
pub use foundation_vfs::error::{VfsError, VfsResult};
pub use memory_delta::MemoryDelta;
pub use memory_fs::MemoryFs;
pub use observable_fs::{ObservableFile, ObservableFs, ObservableSeekableFile, VfsEvent};
pub use overlay_fs::OverlayFileSystem;
foundation_core::blocking_boundary! {
    pub use exec_async::exec_async;
    pub use sync_bridge::{LocalSeekableFile, SyncDirectory, SyncFile, SyncFs};
}
pub use foundation_vfs::types::{
    Checksum, OpenMode, SeekFrom, VfsCapabilities, VfsDirEntry, VfsEntryState, VfsFileType,
    VfsMetadata,
};
pub use traits::{DeltaStore, SeekableVfsFile, VfsDirectory, VfsFile, VfsFileSystem};

#[cfg(feature = "vfs-search")]
pub use search::{
    vfs_searcher, CascadingVfsSearcher, InCodeVfsSearcher, VfsSearchKind, VfsSearchMatch,
    VfsSearcher,
};

#[cfg(feature = "vfs-fjall")]
pub mod fjall_fs;

#[cfg(feature = "vfs-fjall")]
pub use fjall_fs::{InodeDelta, InodeFs};

#[cfg(feature = "vfs-fjall")]
pub mod fjall_document_store;

#[cfg(feature = "vfs-fjall")]
pub use fjall_document_store::{DurabilityWriteConfig, FjallDocumentStore};
