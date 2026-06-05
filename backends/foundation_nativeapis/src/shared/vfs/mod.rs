pub mod error;
pub mod memory_delta;
pub mod memory_fs;
pub mod overlay_fs;
pub mod path_utils;
pub mod traits;
pub mod types;

pub use error::{VfsError, VfsResult};
pub use memory_delta::MemoryDelta;
pub use memory_fs::MemoryFs;
pub use overlay_fs::OverlayFileSystem;
pub use traits::{DeltaStore, SeekableVfsFile, VfsDirectory, VfsFile, VfsFileSystem};
pub use types::{
    Checksum, OpenMode, SeekFrom, VfsCapabilities, VfsDirEntry, VfsEntryState, VfsFileType,
    VfsMetadata,
};
