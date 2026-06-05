use std::io::SeekFrom;

use super::error::VfsResult;
use super::types::{OpenMode, VfsCapabilities, VfsDirEntry, VfsMetadata};

pub trait VfsFile: Send + Sync {
    fn read_at(&self, buf: &mut [u8], offset: u64) -> VfsResult<usize>;
    fn write_at(&self, buf: &[u8], offset: u64) -> VfsResult<usize>;
    fn sync_data(&self) -> VfsResult<()>;
    fn size(&self) -> VfsResult<u64>;
    fn truncate(&self, size: u64) -> VfsResult<()>;
    fn metadata(&self) -> VfsResult<VfsMetadata>;
}

pub trait SeekableVfsFile: VfsFile {
    fn read(&mut self, buf: &mut [u8]) -> VfsResult<usize>;
    fn write(&mut self, buf: &[u8]) -> VfsResult<usize>;
    fn seek(&mut self, pos: SeekFrom) -> VfsResult<u64>;
    fn position(&self) -> u64;
}

pub trait VfsDirectory: Send + Sync {
    type File: VfsFile;
    type SeekableFile: SeekableVfsFile;

    fn path(&self) -> &str;
    fn metadata(&self) -> VfsResult<VfsMetadata>;

    fn list(&self) -> VfsResult<Vec<VfsDirEntry>>;
    fn get_entry(&self, name: &str) -> VfsResult<Option<VfsDirEntry>>;

    fn create_file(&self, name: &str, mode: u32) -> VfsResult<Self::File>;
    fn create_dir(&self, name: &str) -> VfsResult<Box<dyn VfsDirectory<File = Self::File, SeekableFile = Self::SeekableFile>>>;
    fn remove_entry(&self, name: &str) -> VfsResult<()>;
    fn rename_entry(&self, old_name: &str, new_name: &str) -> VfsResult<()>;

    fn open(&self, path: &str, mode: OpenMode) -> VfsResult<Self::File>;
    fn open_seekable(&self, path: &str, mode: OpenMode) -> VfsResult<Self::SeekableFile>;
    fn open_directory(&self, path: &str) -> VfsResult<Box<dyn VfsDirectory<File = Self::File, SeekableFile = Self::SeekableFile>>>;
    fn stat(&self, path: &str) -> VfsResult<VfsMetadata>;
    fn exists(&self, path: &str) -> VfsResult<bool>;

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

    fn mkdir_all(&self, path: &str) -> VfsResult<()> {
        let parts: Vec<&str> = path.split('/').filter(|p| !p.is_empty()).collect();
        let mut current: Box<dyn VfsDirectory<File = Self::File, SeekableFile = Self::SeekableFile>> = self.open_directory(".")?;
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

    fn copy(&self, from: &str, to: &str) -> VfsResult<()> {
        let src = self.open(from, OpenMode::Read)?;
        let size = src.size()?;
        let mut buf = vec![0u8; size as usize];
        src.read_at(&mut buf, 0)?;

        let meta = src.metadata()?;
        let dst = self.create_file(to, meta.permissions)?;
        dst.write_at(&buf, 0)?;
        Ok(())
    }
}

pub trait VfsFileSystem: Send + Sync {
    type File: VfsFile;
    type SeekableFile: SeekableVfsFile;
    type Directory: VfsDirectory<File = Self::File, SeekableFile = Self::SeekableFile>;

    fn capabilities(&self) -> VfsCapabilities;

    fn stat(&self, path: &str) -> VfsResult<VfsMetadata>;
    fn exists(&self, path: &str) -> VfsResult<bool>;
    fn chmod(&self, path: &str, mode: u32) -> VfsResult<()>;
    fn symlink(&self, target: &str, link: &str) -> VfsResult<()>;
    fn readlink(&self, path: &str) -> VfsResult<String>;
    fn rename(&self, from: &str, to: &str) -> VfsResult<()>;
    fn remove(&self, path: &str) -> VfsResult<()>;

    fn open(&self, path: &str, mode: OpenMode) -> VfsResult<Self::File>;
    fn open_seekable(&self, path: &str, mode: OpenMode) -> VfsResult<Self::SeekableFile>;
    fn open_directory(&self, path: &str) -> VfsResult<Self::Directory>;
    fn create(&self, path: &str, mode: u32) -> VfsResult<Self::File>;
    fn mkdir(&self, path: &str) -> VfsResult<()>;

    fn read_file(&self, path: &str) -> VfsResult<Vec<u8>> {
        let file = self.open(path, OpenMode::Read)?;
        let size = file.size()?;
        let mut buf = vec![0u8; size as usize];
        file.read_at(&mut buf, 0)?;
        Ok(buf)
    }

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

    fn copy(&self, from: &str, to: &str) -> VfsResult<()> {
        let data = self.read_file(from)?;
        let meta = self.stat(from)?;
        let file = self.create(to, meta.permissions)?;
        file.write_at(&data, 0)?;
        Ok(())
    }

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
                            super::error::VfsError::NotADirectory {
                                path: current,
                            },
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
    fn add_whiteout(&self, path: &str, version: u64) -> VfsResult<()>;
    fn is_whiteout(&self, path: &str) -> VfsResult<Option<u64>>;
    fn remove_whiteout(&self, path: &str) -> VfsResult<()>;
    fn list_whiteouts(&self, dir: &str) -> VfsResult<Vec<(String, u64)>>;
    fn flush(&self) -> VfsResult<()>;
    fn reset(&self) -> VfsResult<()>;
}
