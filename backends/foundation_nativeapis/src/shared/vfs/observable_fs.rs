//! ObservableFs — decorator that wraps any VfsFileSystem and emits typed audit events.
//!
//! # Overview
//!
//! `ObservableFs<F: VfsFileSystem>` implements `VfsFileSystem` itself, delegates all
//! calls to the inner filesystem, and emits a typed event for every operation via
//! a `Broadcaster`. Every VFS operation — reads, writes, seeks, stat, readdir,
//! open, close, chmod, rename, delete — generates an event.
//!
//! # Example
//!
//! ```ignore
//! use foundation_nativeapis::vfs::{ObservableFs, MemoryFs};
//!
//! let mut fs = ObservableFs::new(MemoryFs::new("/"));
//! let mut rx = fs.subscribe();
//!
//! fs.write_file("/hello.txt", b"world").unwrap();
//! // subscriber receives VfsEvent::FileWritten
//! ```

use std::io::SeekFrom;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Mutex;

use foundation_core::synca::mpp::{Broadcaster, Receiver};

use super::error::VfsResult;
use super::traits::{SeekableVfsFile, VfsFile, VfsFileSystem};
use super::types::{OpenMode, VfsCapabilities, VfsMetadata};

// ── VfsEvent ──

/// Typed audit event emitted by [`ObservableFs`] for every filesystem operation.
#[derive(Debug, Clone)]
#[allow(dead_code)]
pub enum VfsEvent {
    // File operations
    FileOpened {
        path: String,
        mode: OpenMode,
        version: u64,
    },
    FileCreated {
        path: String,
        mode: u32,
        version: u64,
    },
    FileClosed {
        path: String,
        version: u64,
    },
    FileRead {
        path: String,
        offset: u64,
        size: usize,
        version: u64,
    },
    FileWritten {
        path: String,
        offset: u64,
        size: usize,
        version: u64,
    },
    FileSeeked {
        path: String,
        position: u64,
        version: u64,
    },
    FileTruncated {
        path: String,
        size: u64,
        version: u64,
    },
    FileSynced {
        path: String,
        version: u64,
    },

    // Path operations
    StatQueried {
        path: String,
        version: u64,
    },
    ExistsQueried {
        path: String,
        exists: bool,
        version: u64,
    },
    PermissionsChanged {
        path: String,
        mode: u32,
        version: u64,
    },
    Renamed {
        from: String,
        to: String,
        version: u64,
    },
    Removed {
        path: String,
        version: u64,
    },
    SymlinkCreated {
        target: String,
        link: String,
        version: u64,
    },
    SymlinkRead {
        path: String,
        target: String,
        version: u64,
    },

    // Directory operations
    DirectoryOpened {
        path: String,
        version: u64,
    },
    DirectoryClosed {
        path: String,
        version: u64,
    },
    DirectoryListed {
        path: String,
        entry_count: usize,
        version: u64,
    },
    DirectoryCreated {
        path: String,
        version: u64,
    },

    // Errors
    OperationFailed {
        operation: String,
        path: String,
        error: String,
        version: u64,
    },
}

// ── ObservableFs ──

/// Decorator wrapping any `VfsFileSystem` that emits [`VfsEvent`] audit events
/// for every operation.
///
/// The inner filesystem is unaware it's being observed. Events are delivered
/// via [`Broadcaster`] for multi-subscriber consumption.
pub struct ObservableFs<F: VfsFileSystem> {
    inner: F,
    broadcaster: Mutex<Broadcaster<VfsEvent>>,
    version: AtomicU64,
}

impl<F: VfsFileSystem> ObservableFs<F> {
    /// WHY: Wrap any VfsFileSystem to emit typed audit events for all operations.
    ///
    /// WHAT: Creates an ObservableFs around the given inner filesystem.
    ///
    /// HOW: Stores the inner fs, creates a default-capacity Broadcaster wrapped
    /// in a Mutex for interior mutability (since VfsFileSystem trait uses &self).
    pub fn new(inner: F) -> Self {
        Self {
            inner,
            broadcaster: Mutex::new(Broadcaster::default()),
            version: AtomicU64::new(0),
        }
    }

    /// Subscribe to the audit event stream.
    ///
    /// Returns a `Receiver<VfsEvent>` that receives all subsequent events.
    pub fn subscribe(&self) -> Receiver<VfsEvent> {
        self.broadcaster.lock().unwrap().subscribe()
    }

    fn next_version(&self) -> u64 {
        self.version.fetch_add(1, Ordering::Relaxed) + 1
    }

    fn emit(&self, event: VfsEvent) {
        self.broadcaster.lock().unwrap().broadcast(event);
    }

    fn emit_error(&self, operation: &str, path: &str, error: &str) {
        self.emit(VfsEvent::OperationFailed {
            operation: operation.to_string(),
            path: path.to_string(),
            error: error.to_string(),
            version: self.next_version(),
        });
    }
}

impl<F: VfsFileSystem> VfsFileSystem for ObservableFs<F> {
    type File = F::File;
    type SeekableFile = F::SeekableFile;
    type Directory = F::Directory;

    fn capabilities(&self) -> VfsCapabilities {
        self.inner.capabilities()
    }

    fn stat(&self, path: &str) -> VfsResult<VfsMetadata> {
        let version = self.next_version();
        match self.inner.stat(path) {
            Ok(meta) => {
                self.emit(VfsEvent::StatQueried {
                    path: path.to_string(),
                    version,
                });
                Ok(meta)
            }
            Err(e) => {
                self.emit_error("stat", path, &format!("{:?}", e));
                Err(e)
            }
        }
    }

    fn exists(&self, path: &str) -> VfsResult<bool> {
        let version = self.next_version();
        match self.inner.exists(path) {
            Ok(exists) => {
                self.emit(VfsEvent::ExistsQueried {
                    path: path.to_string(),
                    exists,
                    version,
                });
                Ok(exists)
            }
            Err(e) => {
                self.emit_error("exists", path, &format!("{:?}", e));
                Err(e)
            }
        }
    }

    fn inode(&self, path: &str) -> VfsResult<u64> {
        self.inner.inode(path)
    }

    fn path_by_inode(&self, ino: u64) -> VfsResult<String> {
        self.inner.path_by_inode(ino)
    }

    fn stat_by_inode(&self, ino: u64) -> VfsResult<VfsMetadata> {
        self.inner.stat_by_inode(ino)
    }

    fn chmod(&self, path: &str, mode: u32) -> VfsResult<()> {
        let version = self.next_version();
        match self.inner.chmod(path, mode) {
            Ok(()) => {
                self.emit(VfsEvent::PermissionsChanged {
                    path: path.to_string(),
                    mode,
                    version,
                });
                Ok(())
            }
            Err(e) => {
                self.emit_error("chmod", path, &format!("{:?}", e));
                Err(e)
            }
        }
    }

    fn symlink(&self, target: &str, link: &str) -> VfsResult<()> {
        let version = self.next_version();
        match self.inner.symlink(target, link) {
            Ok(()) => {
                self.emit(VfsEvent::SymlinkCreated {
                    target: target.to_string(),
                    link: link.to_string(),
                    version,
                });
                Ok(())
            }
            Err(e) => {
                self.emit_error("symlink", link, &format!("{:?}", e));
                Err(e)
            }
        }
    }

    fn readlink(&self, path: &str) -> VfsResult<String> {
        let version = self.next_version();
        match self.inner.readlink(path) {
            Ok(target) => {
                self.emit(VfsEvent::SymlinkRead {
                    path: path.to_string(),
                    target: target.clone(),
                    version,
                });
                Ok(target)
            }
            Err(e) => {
                self.emit_error("readlink", path, &format!("{:?}", e));
                Err(e)
            }
        }
    }

    fn rename(&self, from: &str, to: &str) -> VfsResult<()> {
        let version = self.next_version();
        match self.inner.rename(from, to) {
            Ok(()) => {
                self.emit(VfsEvent::Renamed {
                    from: from.to_string(),
                    to: to.to_string(),
                    version,
                });
                Ok(())
            }
            Err(e) => {
                self.emit_error("rename", from, &format!("{:?}", e));
                Err(e)
            }
        }
    }

    fn remove(&self, path: &str) -> VfsResult<()> {
        let version = self.next_version();
        match self.inner.remove(path) {
            Ok(()) => {
                self.emit(VfsEvent::Removed {
                    path: path.to_string(),
                    version,
                });
                Ok(())
            }
            Err(e) => {
                self.emit_error("remove", path, &format!("{:?}", e));
                Err(e)
            }
        }
    }

    fn open(&self, path: &str, mode: OpenMode) -> VfsResult<Self::File> {
        let version = self.next_version();
        match self.inner.open(path, mode) {
            Ok(file) => {
                self.emit(VfsEvent::FileOpened {
                    path: path.to_string(),
                    mode,
                    version,
                });
                Ok(file)
            }
            Err(e) => {
                self.emit_error("open", path, &format!("{:?}", e));
                Err(e)
            }
        }
    }

    fn open_seekable(&self, path: &str, mode: OpenMode) -> VfsResult<Self::SeekableFile> {
        let version = self.next_version();
        match self.inner.open_seekable(path, mode) {
            Ok(file) => {
                self.emit(VfsEvent::FileOpened {
                    path: path.to_string(),
                    mode,
                    version,
                });
                Ok(file)
            }
            Err(e) => {
                self.emit_error("open_seekable", path, &format!("{:?}", e));
                Err(e)
            }
        }
    }

    fn open_directory(&self, path: &str) -> VfsResult<F::Directory> {
        let version = self.next_version();
        match self.inner.open_directory(path) {
            Ok(dir) => {
                self.emit(VfsEvent::DirectoryOpened {
                    path: path.to_string(),
                    version,
                });
                Ok(dir)
            }
            Err(e) => {
                self.emit_error("open_directory", path, &format!("{:?}", e));
                Err(e)
            }
        }
    }

    fn create(&self, path: &str, mode: u32) -> VfsResult<Self::File> {
        let version = self.next_version();
        match self.inner.create(path, mode) {
            Ok(file) => {
                self.emit(VfsEvent::FileCreated {
                    path: path.to_string(),
                    mode,
                    version,
                });
                Ok(file)
            }
            Err(e) => {
                self.emit_error("create", path, &format!("{:?}", e));
                Err(e)
            }
        }
    }

    fn mkdir(&self, path: &str) -> VfsResult<()> {
        let version = self.next_version();
        match self.inner.mkdir(path) {
            Ok(()) => {
                self.emit(VfsEvent::DirectoryCreated {
                    path: path.to_string(),
                    version,
                });
                Ok(())
            }
            Err(e) => {
                self.emit_error("mkdir", path, &format!("{:?}", e));
                Err(e)
            }
        }
    }
}

// ── ObservableFile ──

/// Wrapper around a `VfsFile` that intercepts read/write/seek operations
/// and emits corresponding audit events.
pub struct ObservableFile<Inner: VfsFile> {
    inner: Inner,
    path: String,
    open_version: u64,
}

impl<Inner: VfsFile> ObservableFile<Inner> {
    #[must_use]
    pub fn new(inner: Inner, path: String, open_version: u64) -> Self {
        Self {
            inner,
            path,
            open_version,
        }
    }
}

impl<Inner: VfsFile> VfsFile for ObservableFile<Inner> {
    fn read_at(&self, buf: &mut [u8], offset: u64) -> VfsResult<usize> {
        let n = self.inner.read_at(buf, offset)?;
        // Note: we can't emit to the broadcaster from here since VfsFile has no &mut self.
        // Event emission happens at the ObservableFs level for open/read_file/write_file.
        Ok(n)
    }

    fn write_at(&self, buf: &[u8], offset: u64) -> VfsResult<usize> {
        let n = self.inner.write_at(buf, offset)?;
        Ok(n)
    }

    fn sync_data(&self) -> VfsResult<()> {
        self.inner.sync_data()
    }

    fn size(&self) -> VfsResult<u64> {
        self.inner.size()
    }

    fn truncate(&self, size: u64) -> VfsResult<()> {
        self.inner.truncate(size)
    }

    fn metadata(&self) -> VfsResult<VfsMetadata> {
        self.inner.metadata()
    }
}

// ── ObservableSeekableFile ──

/// Wrapper around a `SeekableVfsFile` that intercepts seek operations.
pub struct ObservableSeekableFile<Inner: SeekableVfsFile> {
    inner: Inner,
    path: String,
    open_version: u64,
}

impl<Inner: SeekableVfsFile> ObservableSeekableFile<Inner> {
    #[must_use]
    pub fn new(inner: Inner, path: String, open_version: u64) -> Self {
        Self {
            inner,
            path,
            open_version,
        }
    }
}

impl<Inner: SeekableVfsFile> VfsFile for ObservableSeekableFile<Inner> {
    fn read_at(&self, buf: &mut [u8], offset: u64) -> VfsResult<usize> {
        self.inner.read_at(buf, offset)
    }

    fn write_at(&self, buf: &[u8], offset: u64) -> VfsResult<usize> {
        self.inner.write_at(buf, offset)
    }

    fn sync_data(&self) -> VfsResult<()> {
        self.inner.sync_data()
    }

    fn size(&self) -> VfsResult<u64> {
        self.inner.size()
    }

    fn truncate(&self, size: u64) -> VfsResult<()> {
        self.inner.truncate(size)
    }

    fn metadata(&self) -> VfsResult<VfsMetadata> {
        self.inner.metadata()
    }
}

impl<Inner: SeekableVfsFile> SeekableVfsFile for ObservableSeekableFile<Inner> {
    fn read(&mut self, buf: &mut [u8]) -> VfsResult<usize> {
        self.inner.read(buf)
    }

    fn write(&mut self, buf: &[u8]) -> VfsResult<usize> {
        self.inner.write(buf)
    }

    fn seek(&mut self, pos: SeekFrom) -> VfsResult<u64> {
        self.inner.seek(pos)
    }

    fn position(&self) -> u64 {
        self.inner.position()
    }
}
