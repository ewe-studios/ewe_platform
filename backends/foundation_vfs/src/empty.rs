//! A file system with no files.
//!
//! [`EmptyFs`] implements [`AsyncVfsFileSystem`] and answers "not found" to
//! every read. A caller uses it when the file system is a parameter but no
//! file is read, for example a migration runner whose groups are all in code.
//!
//! Written in ASD-STE100 Simplified Technical English (STE).

use std::io::SeekFrom;

use foundation_errstacks::ErrorTrace;

use crate::async_traits::{
    AsyncSeekableVfsFile, AsyncVfsDirectory, AsyncVfsFile, AsyncVfsFileSystem,
};
use crate::error::{VfsError, VfsResult};
use crate::types::{OpenMode, VfsCapabilities, VfsDirEntry, VfsMetadata};

/// A file system that holds no file.
#[derive(Debug, Clone, Copy, Default)]
pub struct EmptyFs;

impl EmptyFs {
    /// Make an empty file system.
    #[must_use]
    pub fn new() -> Self {
        Self
    }
}

fn not_found(path: &str) -> ErrorTrace<VfsError> {
    ErrorTrace::new(VfsError::NotFound {
        path: path.to_string(),
    })
}

fn unsupported(operation: &str) -> ErrorTrace<VfsError> {
    ErrorTrace::new(VfsError::Unsupported {
        operation: operation.to_string(),
    })
}

/// A file handle from [`EmptyFs`]. Every read fails.
#[derive(Debug, Clone, Copy, Default)]
pub struct EmptyFile;

#[async_trait::async_trait]
impl AsyncVfsFile for EmptyFile {
    async fn read_at_async(&self, _len: usize, _offset: u64) -> VfsResult<Vec<u8>> {
        Err(not_found(""))
    }

    async fn write_at_async(&self, _data: Vec<u8>, _offset: u64) -> VfsResult<usize> {
        Err(unsupported("write_at"))
    }

    async fn sync_data_async(&self) -> VfsResult<()> {
        Ok(())
    }

    async fn size_async(&self) -> VfsResult<u64> {
        Ok(0)
    }

    async fn truncate_async(&self, _size: u64) -> VfsResult<()> {
        Err(unsupported("truncate"))
    }

    async fn metadata_async(&self) -> VfsResult<VfsMetadata> {
        Err(not_found(""))
    }
}

/// A seekable file handle from [`EmptyFs`]. Every read fails.
#[derive(Debug, Clone, Copy, Default)]
pub struct EmptySeekableFile;

#[async_trait::async_trait]
impl AsyncVfsFile for EmptySeekableFile {
    async fn read_at_async(&self, _len: usize, _offset: u64) -> VfsResult<Vec<u8>> {
        Err(not_found(""))
    }

    async fn write_at_async(&self, _data: Vec<u8>, _offset: u64) -> VfsResult<usize> {
        Err(unsupported("write_at"))
    }

    async fn sync_data_async(&self) -> VfsResult<()> {
        Ok(())
    }

    async fn size_async(&self) -> VfsResult<u64> {
        Ok(0)
    }

    async fn truncate_async(&self, _size: u64) -> VfsResult<()> {
        Err(unsupported("truncate"))
    }

    async fn metadata_async(&self) -> VfsResult<VfsMetadata> {
        Err(not_found(""))
    }
}

#[async_trait::async_trait]
impl AsyncSeekableVfsFile for EmptySeekableFile {
    async fn read_async(&mut self, _len: usize) -> VfsResult<Vec<u8>> {
        Err(not_found(""))
    }

    async fn write_async(&mut self, _data: Vec<u8>) -> VfsResult<usize> {
        Err(unsupported("write"))
    }

    async fn seek_async(&mut self, _pos: SeekFrom) -> VfsResult<u64> {
        Err(unsupported("seek"))
    }

    fn position_async(&self) -> u64 {
        0
    }
}

/// A directory handle from [`EmptyFs`]. It is always empty.
#[derive(Debug, Clone, Copy, Default)]
pub struct EmptyDirectory;

#[async_trait::async_trait]
impl AsyncVfsDirectory for EmptyDirectory {
    type File = EmptyFile;
    type SeekableFile = EmptySeekableFile;

    fn path(&self) -> String {
        String::new()
    }

    async fn metadata_async(&self) -> VfsResult<VfsMetadata> {
        Err(not_found(""))
    }

    async fn list_async(&self) -> VfsResult<Vec<VfsDirEntry>> {
        Ok(Vec::new())
    }

    async fn get_entry_async(&self, _name: String) -> VfsResult<Option<VfsDirEntry>> {
        Ok(None)
    }

    async fn create_file_async(&self, _name: String, _mode: u32) -> VfsResult<Self::File> {
        Err(unsupported("create_file"))
    }

    async fn create_dir_async(
        &self,
        _name: String,
    ) -> VfsResult<Box<dyn AsyncVfsDirectory<File = Self::File, SeekableFile = Self::SeekableFile>>>
    {
        Err(unsupported("create_dir"))
    }

    async fn remove_entry_async(&self, _name: String) -> VfsResult<()> {
        Err(unsupported("remove_entry"))
    }

    async fn rename_entry_async(&self, _old_name: String, _new_name: String) -> VfsResult<()> {
        Err(unsupported("rename_entry"))
    }

    async fn open_async(&self, path: String, _mode: OpenMode) -> VfsResult<Self::File> {
        Err(not_found(&path))
    }

    async fn open_seekable_async(
        &self,
        path: String,
        _mode: OpenMode,
    ) -> VfsResult<Self::SeekableFile> {
        Err(not_found(&path))
    }

    async fn open_directory_async(
        &self,
        path: String,
    ) -> VfsResult<Box<dyn AsyncVfsDirectory<File = Self::File, SeekableFile = Self::SeekableFile>>>
    {
        Err(not_found(&path))
    }

    async fn stat_async(&self, path: String) -> VfsResult<VfsMetadata> {
        Err(not_found(&path))
    }

    async fn exists_async(&self, _path: String) -> VfsResult<bool> {
        Ok(false)
    }
}

#[async_trait::async_trait]
impl AsyncVfsFileSystem for EmptyFs {
    type File = EmptyFile;
    type SeekableFile = EmptySeekableFile;
    type Directory = EmptyDirectory;

    fn capabilities(&self) -> VfsCapabilities {
        VfsCapabilities::default()
    }

    async fn stat_async(&self, path: String) -> VfsResult<VfsMetadata> {
        Err(not_found(&path))
    }

    async fn exists_async(&self, _path: String) -> VfsResult<bool> {
        Ok(false)
    }

    async fn chmod_async(&self, _path: String, _mode: u32) -> VfsResult<()> {
        Err(unsupported("chmod"))
    }

    async fn symlink_async(&self, _target: String, _link: String) -> VfsResult<()> {
        Err(unsupported("symlink"))
    }

    async fn readlink_async(&self, path: String) -> VfsResult<String> {
        Err(not_found(&path))
    }

    async fn rename_async(&self, _from: String, _to: String) -> VfsResult<()> {
        Err(unsupported("rename"))
    }

    async fn remove_async(&self, path: String) -> VfsResult<()> {
        Err(not_found(&path))
    }

    async fn open_async(&self, path: String, _mode: OpenMode) -> VfsResult<Self::File> {
        Err(not_found(&path))
    }

    async fn open_seekable_async(
        &self,
        path: String,
        _mode: OpenMode,
    ) -> VfsResult<Self::SeekableFile> {
        Err(not_found(&path))
    }

    async fn open_directory_async(&self, path: String) -> VfsResult<Self::Directory> {
        Err(not_found(&path))
    }

    async fn create_async(&self, _path: String, _mode: u32) -> VfsResult<Self::File> {
        Err(unsupported("create"))
    }

    async fn mkdir_async(&self, _path: String) -> VfsResult<()> {
        Err(unsupported("mkdir"))
    }
}
