//! Async trait definitions for the virtual file system.

use std::io::SeekFrom;

use async_trait::async_trait;

use super::error::{VfsError, VfsResult};
use super::types::{OpenMode, VfsCapabilities, VfsDirEntry, VfsFileType, VfsMetadata};

/// Async file handle for reading, writing, and querying file metadata.
///
/// All fallible methods return `VfsResult<T>`, wrapping errors as `ErrorTrace<VfsError>`.
#[async_trait]
pub trait AsyncVfsFile: Send + Sync {
    async fn read_at_async(&self, len: usize, offset: u64) -> VfsResult<Vec<u8>>;
    async fn write_at_async(&self, data: Vec<u8>, offset: u64) -> VfsResult<usize>;
    async fn sync_data_async(&self) -> VfsResult<()>;
    async fn size_async(&self) -> VfsResult<u64>;
    async fn truncate_async(&self, size: u64) -> VfsResult<()>;
    async fn metadata_async(&self) -> VfsResult<VfsMetadata>;
}

/// Seekable async file handle that tracks a cursor position.
///
/// All fallible methods return `VfsResult<T>`, wrapping errors as `ErrorTrace<VfsError>`.
#[async_trait]
pub trait AsyncSeekableVfsFile: AsyncVfsFile {
    async fn read_async(&mut self, len: usize) -> VfsResult<Vec<u8>>;
    async fn write_async(&mut self, data: Vec<u8>) -> VfsResult<usize>;
    async fn seek_async(&mut self, pos: SeekFrom) -> VfsResult<u64>;
    fn position_async(&self) -> u64;
}

/// Async directory handle for listing, creating, and managing entries.
///
/// All fallible methods return `VfsResult<T>`, wrapping errors as `ErrorTrace<VfsError>`.
#[async_trait]
pub trait AsyncVfsDirectory: Send + Sync {
    type File: AsyncVfsFile + 'static;
    type SeekableFile: AsyncSeekableVfsFile + 'static;

    fn path(&self) -> String;
    async fn metadata_async(&self) -> VfsResult<VfsMetadata>;

    async fn list_async(&self) -> VfsResult<Vec<VfsDirEntry>>;
    async fn get_entry_async(&self, name: String) -> VfsResult<Option<VfsDirEntry>>;

    async fn create_file_async(&self, name: String, mode: u32) -> VfsResult<Self::File>;
    async fn create_dir_async(
        &self,
        name: String,
    ) -> VfsResult<
        Box<dyn AsyncVfsDirectory<File = Self::File, SeekableFile = Self::SeekableFile>>,
    >;
    async fn remove_entry_async(&self, name: String) -> VfsResult<()>;
    async fn rename_entry_async(&self, old_name: String, new_name: String) -> VfsResult<()>;

    async fn open_async(&self, path: String, mode: OpenMode) -> VfsResult<Self::File>;
    async fn open_seekable_async(&self, path: String, mode: OpenMode) -> VfsResult<Self::SeekableFile>;
    async fn open_directory_async(
        &self,
        path: String,
    ) -> VfsResult<
        Box<dyn AsyncVfsDirectory<File = Self::File, SeekableFile = Self::SeekableFile>>,
    >;
    async fn stat_async(&self, path: String) -> VfsResult<VfsMetadata>;
    async fn exists_async(&self, path: String) -> VfsResult<bool>;

    async fn remove_all_async(&self, name: String) -> VfsResult<()> {
        let child_path = if self.path() == "/" {
            format!("/{name}")
        } else {
            format!("{}/{name}", self.path())
        };
        let meta = self.stat_async(name.clone()).await?;
        if meta.file_type == VfsFileType::Directory {
            let dir = self.open_directory_async(name.clone()).await?;
            let entries = dir.list_async().await?;
            for entry in entries {
                dir.remove_all_async(entry.name).await?;
            }
        }
        self.remove_entry_async(name).await?;
        let _ = child_path;
        Ok(())
    }

    async fn mkdir_all_async(&self, path: String) -> VfsResult<()> {
        let parts: Vec<String> = path
            .split('/')
            .filter(|p| !p.is_empty())
            .map(String::from)
            .collect();
        let mut current: Box<
            dyn AsyncVfsDirectory<File = Self::File, SeekableFile = Self::SeekableFile>,
        > = self.open_directory_async(".".to_string()).await?;
        for part in parts {
            match current.get_entry_async(part.clone()).await? {
                Some(entry) if entry.file_type == VfsFileType::Directory => {
                    current = current.open_directory_async(part).await?;
                }
                Some(_) => {
                    return Err(foundation_errstacks::ErrorTrace::new(
                        VfsError::NotADirectory { path: part },
                    ));
                }
                None => {
                    current = current.create_dir_async(part).await?;
                }
            }
        }
        Ok(())
    }

    async fn copy_async(&self, from: String, to: String) -> VfsResult<()> {
        let src = self.open_async(from, OpenMode::Read).await?;
        let size = src.size_async().await?;
        let data = src.read_at_async(usize::try_from(size).unwrap_or(usize::MAX), 0).await?;
        let meta = src.metadata_async().await?;
        let dst = self.create_file_async(to, meta.permissions).await?;
        dst.write_at_async(data, 0).await?;
        Ok(())
    }
}

/// Async filesystem providing path-based file and directory operations.
///
/// All fallible methods return `VfsResult<T>`, wrapping errors as `ErrorTrace<VfsError>`.
#[async_trait]
pub trait AsyncVfsFileSystem: Send + Sync {
    type File: AsyncVfsFile + 'static;
    type SeekableFile: AsyncSeekableVfsFile + 'static;
    type Directory: AsyncVfsDirectory<File = Self::File, SeekableFile = Self::SeekableFile>
        + 'static;

    fn capabilities(&self) -> VfsCapabilities;

    async fn stat_async(&self, path: String) -> VfsResult<VfsMetadata>;
    async fn exists_async(&self, path: String) -> VfsResult<bool>;
    async fn chmod_async(&self, path: String, mode: u32) -> VfsResult<()>;
    async fn symlink_async(&self, target: String, link: String) -> VfsResult<()>;
    async fn readlink_async(&self, path: String) -> VfsResult<String>;
    async fn rename_async(&self, from: String, to: String) -> VfsResult<()>;
    async fn remove_async(&self, path: String) -> VfsResult<()>;

    async fn open_async(&self, path: String, mode: OpenMode) -> VfsResult<Self::File>;
    async fn open_seekable_async(&self, path: String, mode: OpenMode) -> VfsResult<Self::SeekableFile>;
    async fn open_directory_async(&self, path: String) -> VfsResult<Self::Directory>;
    async fn create_async(&self, path: String, mode: u32) -> VfsResult<Self::File>;
    async fn mkdir_async(&self, path: String) -> VfsResult<()>;

    async fn read_file_async(&self, path: String) -> VfsResult<Vec<u8>> {
        let file = self.open_async(path, OpenMode::Read).await?;
        let size = file.size_async().await?;
        file.read_at_async(usize::try_from(size).unwrap_or(usize::MAX), 0).await
    }

    async fn write_file_async(&self, path: String, data: Vec<u8>) -> VfsResult<()> {
        let file = match self.open_async(path.clone(), OpenMode::Write).await {
            Ok(f) => {
                f.truncate_async(0).await?;
                f
            }
            Err(_) => self.create_async(path, 0o644).await?,
        };
        file.write_at_async(data, 0).await?;
        Ok(())
    }

    async fn copy_async(&self, from: String, to: String) -> VfsResult<()> {
        let data = self.read_file_async(from.clone()).await?;
        let meta = self.stat_async(from).await?;
        let file = self.create_async(to, meta.permissions).await?;
        file.write_at_async(data, 0).await?;
        Ok(())
    }

    async fn remove_all_async(&self, path: String) -> VfsResult<()> {
        let meta = self.stat_async(path.clone()).await?;
        if meta.file_type == VfsFileType::Directory {
            let dir = self.open_directory_async(path.clone()).await?;
            let entries = dir.list_async().await?;
            for entry in entries {
                let child = if path == "/" {
                    format!("/{}", entry.name)
                } else {
                    format!("{}/{}", path, entry.name)
                };
                self.remove_all_async(child).await?;
            }
        }
        self.remove_async(path).await
    }

    async fn mkdir_all_async(&self, path: String) -> VfsResult<()> {
        let parts: Vec<String> = path
            .split('/')
            .filter(|p| !p.is_empty())
            .map(String::from)
            .collect();
        let mut current = String::new();
        for part in parts {
            current = format!("{current}/{part}");
            match self.exists_async(current.clone()).await {
                Ok(true) => {
                    let meta = self.stat_async(current.clone()).await?;
                    if meta.file_type != VfsFileType::Directory {
                        return Err(foundation_errstacks::ErrorTrace::new(
                            VfsError::NotADirectory { path: current },
                        ));
                    }
                }
                _ => {
                    self.mkdir_async(current.clone()).await?;
                }
            }
        }
        Ok(())
    }
}

/// Delta-aware filesystem extension supporting whiteout markers for overlay semantics.
///
/// All fallible methods return `VfsResult<T>`, wrapping errors as `ErrorTrace<VfsError>`.
#[async_trait]
pub trait AsyncDeltaStore: AsyncVfsFileSystem {
    async fn add_whiteout_async(&self, path: String, version: u64) -> VfsResult<()>;
    async fn is_whiteout_async(&self, path: String) -> VfsResult<Option<u64>>;
    async fn remove_whiteout_async(&self, path: String) -> VfsResult<()>;
    async fn list_whiteouts_async(&self, dir: String) -> VfsResult<Vec<(String, u64)>>;
    async fn flush_async(&self) -> VfsResult<()>;
    async fn reset_async(&self) -> VfsResult<()>;
}
