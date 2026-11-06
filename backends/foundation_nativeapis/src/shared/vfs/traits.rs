use std::io::SeekFrom;

use super::error::VfsResult;
use super::types::{OpenMode, VfsCapabilities, VfsDirEntry, VfsMetadata};

pub trait VfsFile: Send + Sync {
    ///
    /// # Errors
    /// Returns an error if the operation fails.
    fn read_at(&self, buf: &mut [u8], offset: u64) -> VfsResult<usize>;
    ///
    /// # Errors
    /// Returns an error if the operation fails.
    fn write_at(&self, buf: &[u8], offset: u64) -> VfsResult<usize>;
    ///
    /// # Errors
    /// Returns an error if the operation fails.
    fn sync_data(&self) -> VfsResult<()>;
    ///
    /// # Errors
    /// Returns an error if the operation fails.
    fn size(&self) -> VfsResult<u64>;
    ///
    /// # Errors
    /// Returns an error if the operation fails.
    fn truncate(&self, size: u64) -> VfsResult<()>;
    ///
    /// # Errors
    /// Returns an error if the operation fails.
    fn metadata(&self) -> VfsResult<VfsMetadata>;
}

pub trait SeekableVfsFile: VfsFile {
    ///
    /// # Errors
    /// Returns an error if the operation fails.
    fn read(&mut self, buf: &mut [u8]) -> VfsResult<usize>;
    ///
    /// # Errors
    /// Returns an error if the operation fails.
    fn write(&mut self, buf: &[u8]) -> VfsResult<usize>;
    ///
    /// # Errors
    /// Returns an error if the operation fails.
    fn seek(&mut self, pos: SeekFrom) -> VfsResult<u64>;
    fn position(&self) -> u64;
}

pub trait VfsDirectory: Send + Sync {
    type File: VfsFile;
    type SeekableFile: SeekableVfsFile;

    fn path(&self) -> &str;
    ///
    /// # Errors
    /// Returns an error if the operation fails.
    fn metadata(&self) -> VfsResult<VfsMetadata>;

    ///
    /// # Errors
    /// Returns an error if the operation fails.
    fn list(&self) -> VfsResult<Vec<VfsDirEntry>>;
    ///
    /// # Errors
    /// Returns an error if the operation fails.
    fn get_entry(&self, name: &str) -> VfsResult<Option<VfsDirEntry>>;

    ///
    /// # Errors
    /// Returns an error if the operation fails.
    fn create_file(&self, name: &str, mode: u32) -> VfsResult<Self::File>;
    ///
    /// # Errors
    /// Returns an error if the operation fails.
    fn create_dir(
        &self,
        name: &str,
    ) -> VfsResult<Box<dyn VfsDirectory<File = Self::File, SeekableFile = Self::SeekableFile>>>;
    ///
    /// # Errors
    /// Returns an error if the operation fails.
    fn remove_entry(&self, name: &str) -> VfsResult<()>;
    ///
    /// # Errors
    /// Returns an error if the operation fails.
    fn rename_entry(&self, old_name: &str, new_name: &str) -> VfsResult<()>;

    ///
    /// # Errors
    /// Returns an error if the operation fails.
    fn open(&self, path: &str, mode: OpenMode) -> VfsResult<Self::File>;
    ///
    /// # Errors
    /// Returns an error if the operation fails.
    fn open_seekable(&self, path: &str, mode: OpenMode) -> VfsResult<Self::SeekableFile>;
    ///
    /// # Errors
    /// Returns an error if the operation fails.
    fn open_directory(
        &self,
        path: &str,
    ) -> VfsResult<Box<dyn VfsDirectory<File = Self::File, SeekableFile = Self::SeekableFile>>>;
    ///
    /// # Errors
    /// Returns an error if the operation fails.
    fn stat(&self, path: &str) -> VfsResult<VfsMetadata>;
    ///
    /// # Errors
    /// Returns an error if the operation fails.
    fn exists(&self, path: &str) -> VfsResult<bool>;

    ///
    /// # Errors
    /// Returns an error if the operation fails.
    fn remove_all(&self, name: &str) -> VfsResult<()> {
        let child_path = if self.path() == "/" {
            format!("/{name}")
        } else {
            format!("{}/{name}", self.path())
        };
        let meta = self.stat(name)?;
        if meta.file_type == super::types::VfsFileType::Directory {
            let dir = self.open_directory(name)?;
            let entries = dir.list()?;
            for entry in entries {
                dir.remove_all(&entry.name)?;
            }
        }
        self.remove_entry(name)?;
        let _ = child_path;
        Ok(())
    }

    ///
    /// # Errors
    /// Returns an error if the operation fails.
    fn mkdir_all(&self, path: &str) -> VfsResult<()> {
        let parts: Vec<&str> = path.split('/').filter(|p| !p.is_empty()).collect();
        let mut current: Box<
            dyn VfsDirectory<File = Self::File, SeekableFile = Self::SeekableFile>,
        > = self.open_directory(".")?;
        for part in parts {
            match current.get_entry(part)? {
                Some(entry) if entry.file_type == super::types::VfsFileType::Directory => {
                    current = current.open_directory(part)?;
                }
                Some(_) => {
                    return Err(foundation_errstacks::ErrorTrace::new(
                        super::error::VfsError::NotADirectory {
                            path: part.to_string(),
                        },
                    ));
                }
                None => {
                    current = current.create_dir(part)?;
                }
            }
        }
        Ok(())
    }

    /// Chunk size for `copy`.
    ///
    /// The default is 32 KiB, matching Go `net/http` `io.Copy`. Override this
    /// method to read the size from a store's config.
    fn copy_chunk_size(&self) -> usize {
        32 * 1024
    }

    ///
    /// # Errors
    /// Returns an error if the operation fails.
    fn copy(&self, from: &str, to: &str) -> VfsResult<()> {
        let src = self.open(from, OpenMode::Read)?;
        let size = src.size()?;
        let meta = src.metadata()?;
        let dst = self.create_file(to, meta.permissions)?;
        let chunk_size = self.copy_chunk_size().max(1);
        let mut offset = 0u64;
        while offset < size {
            let remaining = size - offset;
            let to_read = usize::try_from(remaining.min(chunk_size as u64)).unwrap_or(usize::MAX);
            let mut buf = vec![0u8; to_read];
            let n = src.read_at(&mut buf, offset)?;
            if n == 0 {
                break;
            }
            dst.write_at(&buf[..n], offset)?;
            offset += n as u64;
        }
        Ok(())
    }
}

pub trait VfsFileSystem: Send + Sync {
    type File: VfsFile;
    type SeekableFile: SeekableVfsFile;
    type Directory: VfsDirectory<File = Self::File, SeekableFile = Self::SeekableFile>;

    fn capabilities(&self) -> VfsCapabilities;

    ///
    /// # Errors
    /// Returns an error if the operation fails.
    fn stat(&self, path: &str) -> VfsResult<VfsMetadata>;
    ///
    /// # Errors
    /// Returns an error if the operation fails.
    fn exists(&self, path: &str) -> VfsResult<bool>;
    ///
    /// # Errors
    /// Returns an error if the operation fails.
    fn chmod(&self, path: &str, mode: u32) -> VfsResult<()>;
    ///
    /// # Errors
    /// Returns an error if the operation fails.
    fn symlink(&self, target: &str, link: &str) -> VfsResult<()>;
    ///
    /// # Errors
    /// Returns an error if the operation fails.
    fn readlink(&self, path: &str) -> VfsResult<String>;
    ///
    /// # Errors
    /// Returns an error if the operation fails.
    fn rename(&self, from: &str, to: &str) -> VfsResult<()>;
    ///
    /// # Errors
    /// Returns an error if the operation fails.
    fn remove(&self, path: &str) -> VfsResult<()>;

    ///
    /// # Errors
    /// Returns an error if the operation fails.
    fn open(&self, path: &str, mode: OpenMode) -> VfsResult<Self::File>;
    ///
    /// # Errors
    /// Returns an error if the operation fails.
    fn open_seekable(&self, path: &str, mode: OpenMode) -> VfsResult<Self::SeekableFile>;
    ///
    /// # Errors
    /// Returns an error if the operation fails.
    fn open_directory(&self, path: &str) -> VfsResult<Self::Directory>;
    ///
    /// # Errors
    /// Returns an error if the operation fails.
    fn create(&self, path: &str, mode: u32) -> VfsResult<Self::File>;
    ///
    /// # Errors
    /// Returns an error if the operation fails.
    fn mkdir(&self, path: &str) -> VfsResult<()>;

    ///
    /// # Errors
    /// Returns an error if the operation fails.
    fn inode(&self, path: &str) -> VfsResult<u64>;
    ///
    /// # Errors
    /// Returns an error if the operation fails.
    fn path_by_inode(&self, ino: u64) -> VfsResult<String>;
    ///
    /// # Errors
    /// Returns an error if the operation fails.
    fn stat_by_inode(&self, ino: u64) -> VfsResult<VfsMetadata>;

    /// Reads the whole file into memory.
    ///
    /// The whole file fits in one buffer. Use this method only for a small
    /// file whose contents the next step needs. `copy` does not use this
    /// method; it streams the file in chunks.
    ///
    /// # Errors
    /// Returns an error if the operation fails.
    fn read_file(&self, path: &str) -> VfsResult<Vec<u8>> {
        let file = self.open(path, OpenMode::Read)?;
        let size = file.size()?;
        let mut buf = vec![0u8; usize::try_from(size).unwrap_or(usize::MAX)];
        file.read_at(&mut buf, 0)?;
        Ok(buf)
    }

    ///
    /// # Errors
    /// Returns an error if the operation fails.
    fn write_file(&self, path: &str, data: &[u8]) -> VfsResult<()> {
        let file = match self.open(path, OpenMode::Write) {
            Ok(f) => {
                f.truncate(0)?;
                f
            }
            Err(_) => self.create(path, 0o644)?,
        };
        file.write_at(data, 0)?;
        Ok(())
    }

    /// Chunk size for `copy`.
    ///
    /// The default is 32 KiB, matching Go `net/http` `io.Copy`. Override this
    /// method to read the size from a store's config.
    fn copy_chunk_size(&self) -> usize {
        32 * 1024
    }

    ///
    /// # Errors
    /// Returns an error if the operation fails.
    fn copy(&self, from: &str, to: &str) -> VfsResult<()> {
        let src = self.open(from, OpenMode::Read)?;
        let size = src.size()?;
        let meta = self.stat(from)?;
        let dst = self.create(to, meta.permissions)?;
        let chunk_size = self.copy_chunk_size().max(1);
        let mut offset = 0u64;
        while offset < size {
            let remaining = size - offset;
            let to_read = usize::try_from(remaining.min(chunk_size as u64)).unwrap_or(usize::MAX);
            let mut buf = vec![0u8; to_read];
            let n = src.read_at(&mut buf, offset)?;
            if n == 0 {
                break;
            }
            dst.write_at(&buf[..n], offset)?;
            offset += n as u64;
        }
        Ok(())
    }

    ///
    /// # Errors
    /// Returns an error if the operation fails.
    fn remove_all(&self, path: &str) -> VfsResult<()> {
        let meta = self.stat(path)?;
        if meta.file_type == super::types::VfsFileType::Directory {
            let dir = self.open_directory(path)?;
            let entries = dir.list()?;
            for entry in entries {
                let child = if path == "/" {
                    format!("/{}", entry.name)
                } else {
                    format!("{}/{}", path, entry.name)
                };
                self.remove_all(&child)?;
            }
        }
        self.remove(path)
    }

    ///
    /// # Errors
    /// Returns an error if the operation fails.
    fn mkdir_all(&self, path: &str) -> VfsResult<()> {
        let parts: Vec<&str> = path.split('/').filter(|p| !p.is_empty()).collect();
        let mut current = String::new();
        for part in parts {
            current = format!("{current}/{part}");
            match self.exists(&current) {
                Ok(true) => {
                    let meta = self.stat(&current)?;
                    if meta.file_type != super::types::VfsFileType::Directory {
                        return Err(foundation_errstacks::ErrorTrace::new(
                            super::error::VfsError::NotADirectory { path: current },
                        ));
                    }
                }
                _ => {
                    self.mkdir(&current)?;
                }
            }
        }
        Ok(())
    }
}

pub trait DeltaStore: VfsFileSystem {
    ///
    /// # Errors
    /// Returns an error if the operation fails.
    fn add_whiteout(&self, path: &str, version: u64) -> VfsResult<()>;
    ///
    /// # Errors
    /// Returns an error if the operation fails.
    fn is_whiteout(&self, path: &str) -> VfsResult<Option<u64>>;
    ///
    /// # Errors
    /// Returns an error if the operation fails.
    fn remove_whiteout(&self, path: &str) -> VfsResult<()>;
    ///
    /// # Errors
    /// Returns an error if the operation fails.
    fn list_whiteouts(&self, dir: &str) -> VfsResult<Vec<(String, u64)>>;
    ///
    /// # Errors
    /// Returns an error if the operation fails.
    fn flush(&self) -> VfsResult<()>;
    ///
    /// # Errors
    /// Returns an error if the operation fails.
    fn reset(&self) -> VfsResult<()>;
}
