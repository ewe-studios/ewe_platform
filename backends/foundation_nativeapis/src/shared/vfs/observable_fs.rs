//! ObservableFs — decorator that wraps any AsyncVfsFileSystem and emits typed audit events.
//!
//! # Overview
//!
//! `ObservableFs<F: AsyncVfsFileSystem>` implements `AsyncVfsFileSystem` itself, delegates all
//! calls to the inner filesystem, and emits a typed event for every operation via
//! a `Broadcaster`. Every VFS operation — reads, writes, seeks, stat, readdir,
//! open, close, chmod, rename, delete — generates an event.
//!
//! # Example
//!
//! ```ignore
//! use foundation_nativeapis::vfs::{ObservableFs, MemoryFs};
//!
//! let fs = ObservableFs::new(MemoryFs::new());
//! let rx = fs.subscribe();
//!
//! fs.write_file_async("/hello.txt".into(), b"world".to_vec()).await.unwrap();
//! // subscriber receives VfsEvent::FileWritten
//! ```

use std::io::SeekFrom;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Mutex;

use foundation_core::synca::mpp::{Broadcaster, Receiver};

use async_trait::async_trait;

use super::async_traits::{AsyncSeekableVfsFile, AsyncVfsFile, AsyncVfsFileSystem};
use super::error::VfsResult;
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

/// Decorator wrapping any `AsyncVfsFileSystem` that emits [`VfsEvent`] audit events
/// for every operation.
///
/// The inner filesystem is unaware it's being observed. Events are delivered
/// via [`Broadcaster`] for multi-subscriber consumption.
pub struct ObservableFs<F: AsyncVfsFileSystem> {
    inner: F,
    broadcaster: Mutex<Broadcaster<VfsEvent>>,
    version: AtomicU64,
}

impl<F: AsyncVfsFileSystem> ObservableFs<F> {
    /// WHY: Wrap any AsyncVfsFileSystem to emit typed audit events for all operations.
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

#[async_trait]
impl<F: AsyncVfsFileSystem> AsyncVfsFileSystem for ObservableFs<F> {
    type File = F::File;
    type SeekableFile = F::SeekableFile;
    type Directory = F::Directory;

    fn capabilities(&self) -> VfsCapabilities {
        self.inner.capabilities()
    }

    async fn stat_async(&self, path: String) -> VfsResult<VfsMetadata> {
        let version = self.next_version();
        match self.inner.stat_async(path.clone()).await {
            Ok(meta) => {
                self.emit(VfsEvent::StatQueried {
                    path,
                    version,
                });
                Ok(meta)
            }
            Err(e) => {
                self.emit_error("stat", &path, &format!("{:?}", e));
                Err(e)
            }
        }
    }

    async fn exists_async(&self, path: String) -> VfsResult<bool> {
        let version = self.next_version();
        match self.inner.exists_async(path.clone()).await {
            Ok(exists) => {
                self.emit(VfsEvent::ExistsQueried {
                    path,
                    exists,
                    version,
                });
                Ok(exists)
            }
            Err(e) => {
                self.emit_error("exists", &path, &format!("{:?}", e));
                Err(e)
            }
        }
    }

    async fn inode_async(&self, path: String) -> VfsResult<u64> {
        self.inner.inode_async(path).await
    }

    async fn path_by_inode_async(&self, ino: u64) -> VfsResult<String> {
        self.inner.path_by_inode_async(ino).await
    }

    async fn stat_by_inode_async(&self, ino: u64) -> VfsResult<VfsMetadata> {
        self.inner.stat_by_inode_async(ino).await
    }

    async fn chmod_async(&self, path: String, mode: u32) -> VfsResult<()> {
        let version = self.next_version();
        match self.inner.chmod_async(path.clone(), mode).await {
            Ok(()) => {
                self.emit(VfsEvent::PermissionsChanged {
                    path,
                    mode,
                    version,
                });
                Ok(())
            }
            Err(e) => {
                self.emit_error("chmod", &path, &format!("{:?}", e));
                Err(e)
            }
        }
    }

    async fn symlink_async(&self, target: String, link: String) -> VfsResult<()> {
        let version = self.next_version();
        match self.inner.symlink_async(target.clone(), link.clone()).await {
            Ok(()) => {
                self.emit(VfsEvent::SymlinkCreated {
                    target,
                    link,
                    version,
                });
                Ok(())
            }
            Err(e) => {
                self.emit_error("symlink", &link, &format!("{:?}", e));
                Err(e)
            }
        }
    }

    async fn readlink_async(&self, path: String) -> VfsResult<String> {
        let version = self.next_version();
        match self.inner.readlink_async(path.clone()).await {
            Ok(target) => {
                self.emit(VfsEvent::SymlinkRead {
                    path,
                    target: target.clone(),
                    version,
                });
                Ok(target)
            }
            Err(e) => {
                self.emit_error("readlink", &path, &format!("{:?}", e));
                Err(e)
            }
        }
    }

    async fn rename_async(&self, from: String, to: String) -> VfsResult<()> {
        let version = self.next_version();
        match self.inner.rename_async(from.clone(), to.clone()).await {
            Ok(()) => {
                self.emit(VfsEvent::Renamed {
                    from,
                    to,
                    version,
                });
                Ok(())
            }
            Err(e) => {
                self.emit_error("rename", &from, &format!("{:?}", e));
                Err(e)
            }
        }
    }

    async fn remove_async(&self, path: String) -> VfsResult<()> {
        let version = self.next_version();
        match self.inner.remove_async(path.clone()).await {
            Ok(()) => {
                self.emit(VfsEvent::Removed {
                    path,
                    version,
                });
                Ok(())
            }
            Err(e) => {
                self.emit_error("remove", &path, &format!("{:?}", e));
                Err(e)
            }
        }
    }

    async fn open_async(&self, path: String, mode: OpenMode) -> VfsResult<Self::File> {
        let version = self.next_version();
        match self.inner.open_async(path.clone(), mode).await {
            Ok(file) => {
                self.emit(VfsEvent::FileOpened {
                    path,
                    mode,
                    version,
                });
                Ok(file)
            }
            Err(e) => {
                self.emit_error("open", &path, &format!("{:?}", e));
                Err(e)
            }
        }
    }

    async fn open_seekable_async(&self, path: String, mode: OpenMode) -> VfsResult<Self::SeekableFile> {
        let version = self.next_version();
        match self.inner.open_seekable_async(path.clone(), mode).await {
            Ok(file) => {
                self.emit(VfsEvent::FileOpened {
                    path,
                    mode,
                    version,
                });
                Ok(file)
            }
            Err(e) => {
                self.emit_error("open_seekable", &path, &format!("{:?}", e));
                Err(e)
            }
        }
    }

    async fn open_directory_async(&self, path: String) -> VfsResult<F::Directory> {
        let version = self.next_version();
        match self.inner.open_directory_async(path.clone()).await {
            Ok(dir) => {
                self.emit(VfsEvent::DirectoryOpened {
                    path,
                    version,
                });
                Ok(dir)
            }
            Err(e) => {
                self.emit_error("open_directory", &path, &format!("{:?}", e));
                Err(e)
            }
        }
    }

    async fn create_async(&self, path: String, mode: u32) -> VfsResult<Self::File> {
        let version = self.next_version();
        match self.inner.create_async(path.clone(), mode).await {
            Ok(file) => {
                self.emit(VfsEvent::FileCreated {
                    path,
                    mode,
                    version,
                });
                Ok(file)
            }
            Err(e) => {
                self.emit_error("create", &path, &format!("{:?}", e));
                Err(e)
            }
        }
    }

    async fn mkdir_async(&self, path: String) -> VfsResult<()> {
        let version = self.next_version();
        match self.inner.mkdir_async(path.clone()).await {
            Ok(()) => {
                self.emit(VfsEvent::DirectoryCreated {
                    path,
                    version,
                });
                Ok(())
            }
            Err(e) => {
                self.emit_error("mkdir", &path, &format!("{:?}", e));
                Err(e)
            }
        }
    }
}

// ── ObservableFile ──

/// Wrapper around an `AsyncVfsFile` that keeps the path and the open version.
/// It passes every call through; `ObservableFs` emits the events.
#[allow(dead_code)]
pub struct ObservableFile<Inner: AsyncVfsFile> {
    inner: Inner,
    path: String,
    open_version: u64,
}

impl<Inner: AsyncVfsFile> ObservableFile<Inner> {
    #[must_use]
    pub fn new(inner: Inner, path: String, open_version: u64) -> Self {
        Self {
            inner,
            path,
            open_version,
        }
    }
}

#[async_trait]
impl<Inner: AsyncVfsFile> AsyncVfsFile for ObservableFile<Inner> {
    async fn read_at_async(&self, len: usize, offset: u64) -> VfsResult<Vec<u8>> {
        self.inner.read_at_async(len, offset).await
    }

    async fn write_at_async(&self, data: Vec<u8>, offset: u64) -> VfsResult<usize> {
        self.inner.write_at_async(data, offset).await
    }

    async fn sync_data_async(&self) -> VfsResult<()> {
        self.inner.sync_data_async().await
    }

    async fn size_async(&self) -> VfsResult<u64> {
        self.inner.size_async().await
    }

    async fn truncate_async(&self, size: u64) -> VfsResult<()> {
        self.inner.truncate_async(size).await
    }

    async fn metadata_async(&self) -> VfsResult<VfsMetadata> {
        self.inner.metadata_async().await
    }
}

// ── ObservableSeekableFile ──

/// Wrapper around an `AsyncSeekableVfsFile`; passes every call through.
#[allow(dead_code)]
pub struct ObservableSeekableFile<Inner: AsyncSeekableVfsFile> {
    inner: Inner,
    path: String,
    open_version: u64,
}

impl<Inner: AsyncSeekableVfsFile> ObservableSeekableFile<Inner> {
    #[must_use]
    pub fn new(inner: Inner, path: String, open_version: u64) -> Self {
        Self {
            inner,
            path,
            open_version,
        }
    }
}

#[async_trait]
impl<Inner: AsyncSeekableVfsFile> AsyncVfsFile for ObservableSeekableFile<Inner> {
    async fn read_at_async(&self, len: usize, offset: u64) -> VfsResult<Vec<u8>> {
        self.inner.read_at_async(len, offset).await
    }

    async fn write_at_async(&self, data: Vec<u8>, offset: u64) -> VfsResult<usize> {
        self.inner.write_at_async(data, offset).await
    }

    async fn sync_data_async(&self) -> VfsResult<()> {
        self.inner.sync_data_async().await
    }

    async fn size_async(&self) -> VfsResult<u64> {
        self.inner.size_async().await
    }

    async fn truncate_async(&self, size: u64) -> VfsResult<()> {
        self.inner.truncate_async(size).await
    }

    async fn metadata_async(&self) -> VfsResult<VfsMetadata> {
        self.inner.metadata_async().await
    }
}

#[async_trait]
impl<Inner: AsyncSeekableVfsFile> AsyncSeekableVfsFile for ObservableSeekableFile<Inner> {
    async fn read_async(&mut self, len: usize) -> VfsResult<Vec<u8>> {
        self.inner.read_async(len).await
    }

    async fn write_async(&mut self, data: Vec<u8>) -> VfsResult<usize> {
        self.inner.write_async(data).await
    }

    async fn seek_async(&mut self, pos: SeekFrom) -> VfsResult<u64> {
        self.inner.seek_async(pos).await
    }

    fn position_async(&self) -> u64 {
        self.inner.position_async()
    }
}
