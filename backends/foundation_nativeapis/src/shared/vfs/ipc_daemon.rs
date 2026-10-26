//! `VfsDaemon`: serves an `AsyncVfsFileSystem` to remote `VfsClient`s, one
//! `VfsRequest` at a time.
//!
//! The daemon keeps the open file and directory handles. A file handle is an
//! `Arc`, so a request clones it out of the table and awaits the file without
//! a lock held.

use std::collections::HashMap;
use std::io::SeekFrom;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Arc, Mutex, MutexGuard, PoisonError};

use super::async_traits::{AsyncVfsDirectory, AsyncVfsFile, AsyncVfsFileSystem};
use super::ipc_messages::{VfsRequest, VfsResponse};
use super::types::{OpenMode, VfsDirEntry};

/// A seekable handle: the file and the cursor that `Seek` moves.
struct SeekHandle<S> {
    file: Arc<S>,
    position: AtomicU64,
}

struct DirHandle {
    path: String,
}

/// A file handle of either kind, cloned out of the table for one request.
enum AnyFile<F, S> {
    Plain(Arc<F>),
    Seekable(Arc<S>),
}

pub struct VfsDaemon<F: AsyncVfsFileSystem> {
    fs: Arc<F>,
    file_handles: Mutex<HashMap<u64, Arc<F::File>>>,
    seekable_handles: Mutex<HashMap<u64, Arc<SeekHandle<F::SeekableFile>>>>,
    dir_handles: Mutex<HashMap<u64, DirHandle>>,
    next_handle: AtomicU64,
}

fn lock<T>(m: &Mutex<T>) -> MutexGuard<'_, T> {
    // A panic while a table lock is held leaves the map itself valid (every
    // update is a single insert or remove), so a poisoned lock is still used.
    m.lock().unwrap_or_else(PoisonError::into_inner)
}

fn err(e: impl std::fmt::Display) -> VfsResponse {
    VfsResponse::Error(format!("{e}"))
}

fn child_path(dir: &str, name: &str) -> String {
    if dir == "/" {
        format!("/{name}")
    } else {
        format!("{dir}/{name}")
    }
}

impl<F: AsyncVfsFileSystem + 'static> VfsDaemon<F> {
    pub fn new(fs: Arc<F>) -> Self {
        Self {
            fs,
            file_handles: Mutex::new(HashMap::new()),
            seekable_handles: Mutex::new(HashMap::new()),
            dir_handles: Mutex::new(HashMap::new()),
            next_handle: AtomicU64::new(1),
        }
    }

    fn alloc_handle(&self) -> u64 {
        self.next_handle.fetch_add(1, Ordering::Relaxed)
    }

    /// Serves one request.
    pub async fn dispatch(&self, req: VfsRequest) -> VfsResponse {
        match req {
            VfsRequest::Open {
                path,
                mode,
                seekable,
            } => {
                if seekable {
                    self.handle_open_seekable(path, mode.into()).await
                } else {
                    self.handle_open(path, mode.into()).await
                }
            }
            VfsRequest::Create { path, mode } => self.handle_create(path, mode).await,
            VfsRequest::ReadAt {
                handle,
                offset,
                size,
            } => self.handle_read_at(handle, offset, size).await,
            VfsRequest::WriteAt {
                handle,
                data,
                offset,
            } => self.handle_write_at(handle, data, offset).await,
            VfsRequest::Seek { handle, pos } => self.handle_seek(handle, pos.into()).await,
            VfsRequest::Truncate { handle, size } => self.handle_truncate(handle, size).await,
            VfsRequest::SyncData { handle } => self.handle_sync(handle).await,
            VfsRequest::FileSize { handle } => self.handle_file_size(handle).await,
            VfsRequest::FileMetadata { handle } => self.handle_file_metadata(handle).await,

            VfsRequest::Stat { path } => match self.fs.stat_async(path).await {
                Ok(m) => VfsResponse::Metadata(m.into()),
                Err(e) => err(e),
            },
            VfsRequest::Exists { path } => match self.fs.exists_async(path).await {
                Ok(b) => VfsResponse::Exists(b),
                Err(e) => err(e),
            },
            VfsRequest::Mkdir { path } => ok_or_err(self.fs.mkdir_async(path).await),
            VfsRequest::Remove { path } => ok_or_err(self.fs.remove_async(path).await),
            VfsRequest::Rename { from, to } => ok_or_err(self.fs.rename_async(from, to).await),
            VfsRequest::Chmod { path, mode } => ok_or_err(self.fs.chmod_async(path, mode).await),
            VfsRequest::Symlink { target, link } => {
                ok_or_err(self.fs.symlink_async(target, link).await)
            }
            VfsRequest::ReadLink { path } => match self.fs.readlink_async(path).await {
                Ok(s) => VfsResponse::Path(s),
                Err(e) => err(e),
            },

            VfsRequest::Inode { path } => match self.fs.inode_async(path).await {
                Ok(ino) => VfsResponse::Inode(ino),
                Err(e) => err(e),
            },
            VfsRequest::PathByInode { ino } => match self.fs.path_by_inode_async(ino).await {
                Ok(p) => VfsResponse::Path(p),
                Err(e) => err(e),
            },
            VfsRequest::StatByInode { ino } => match self.fs.stat_by_inode_async(ino).await {
                Ok(m) => VfsResponse::Metadata(m.into()),
                Err(e) => err(e),
            },

            VfsRequest::OpenDir { path } => self.handle_open_dir(path).await,
            VfsRequest::DirList { handle } => self.handle_dir_list(handle).await,
            VfsRequest::DirGetEntry { handle, name } => {
                self.handle_dir_get_entry(handle, name).await
            }
            VfsRequest::DirPath { handle } => match self.dir_path(handle) {
                Ok(path) => VfsResponse::Path(path),
                Err(resp) => resp,
            },
            VfsRequest::DirMetadata { handle } => match self.dir_path(handle) {
                Ok(path) => match self.fs.stat_async(path).await {
                    Ok(m) => VfsResponse::Metadata(m.into()),
                    Err(e) => err(e),
                },
                Err(resp) => resp,
            },
            VfsRequest::DirCreateFile { handle, name, mode } => match self.dir_path(handle) {
                Ok(path) => self.handle_create(child_path(&path, &name), mode).await,
                Err(resp) => resp,
            },
            VfsRequest::DirCreateDir { handle, name } => {
                self.handle_dir_create_dir(handle, name).await
            }
            VfsRequest::DirRemoveEntry { handle, name } => match self.dir_path(handle) {
                Ok(path) => ok_or_err(self.fs.remove_async(child_path(&path, &name)).await),
                Err(resp) => resp,
            },

            VfsRequest::ReadFile { path } => match self.fs.read_file_async(path).await {
                Ok(data) => VfsResponse::Data(data),
                Err(e) => err(e),
            },
            VfsRequest::WriteFile { path, data } => {
                ok_or_err(self.fs.write_file_async(path, data).await)
            }
            VfsRequest::Capabilities => VfsResponse::Capabilities(self.fs.capabilities().into()),

            close @ (VfsRequest::CloseFile { .. } | VfsRequest::CloseDir { .. }) => {
                self.release(close)
            }
        }
    }

    /// Serves a request that needs no I/O and no wait: `CloseFile` and
    /// `CloseDir`. A transport calls it from a `Drop` impl, where it cannot
    /// await. Any other request gets an error response.
    pub fn release(&self, req: VfsRequest) -> VfsResponse {
        match req {
            VfsRequest::CloseFile { handle } => {
                let removed_file = lock(&self.file_handles).remove(&handle).is_some();
                let removed_seekable = lock(&self.seekable_handles).remove(&handle).is_some();
                if removed_file || removed_seekable {
                    VfsResponse::Ok
                } else {
                    VfsResponse::Error("invalid file handle".into())
                }
            }
            VfsRequest::CloseDir { handle } => {
                if lock(&self.dir_handles).remove(&handle).is_some() {
                    VfsResponse::Ok
                } else {
                    VfsResponse::Error("invalid dir handle".into())
                }
            }
            _ => VfsResponse::Error("only close requests can be released".into()),
        }
    }

    fn file(&self, handle: u64) -> Option<AnyFile<F::File, F::SeekableFile>> {
        if let Some(h) = lock(&self.seekable_handles).get(&handle) {
            return Some(AnyFile::Seekable(h.file.clone()));
        }
        lock(&self.file_handles)
            .get(&handle)
            .map(|f| AnyFile::Plain(f.clone()))
    }

    fn dir_path(&self, handle: u64) -> Result<String, VfsResponse> {
        lock(&self.dir_handles)
            .get(&handle)
            .map(|dh| dh.path.clone())
            .ok_or_else(|| VfsResponse::Error("invalid dir handle".into()))
    }

    // ── File handle operations ──

    async fn handle_open(&self, path: String, mode: OpenMode) -> VfsResponse {
        match self.fs.open_async(path, mode).await {
            Ok(file) => {
                let h = self.alloc_handle();
                lock(&self.file_handles).insert(h, Arc::new(file));
                VfsResponse::FileHandle(h)
            }
            Err(e) => err(e),
        }
    }

    async fn handle_open_seekable(&self, path: String, mode: OpenMode) -> VfsResponse {
        match self.fs.open_seekable_async(path, mode).await {
            Ok(file) => {
                let h = self.alloc_handle();
                lock(&self.seekable_handles).insert(
                    h,
                    Arc::new(SeekHandle {
                        file: Arc::new(file),
                        position: AtomicU64::new(0),
                    }),
                );
                VfsResponse::FileHandle(h)
            }
            Err(e) => err(e),
        }
    }

    async fn handle_create(&self, path: String, mode: u32) -> VfsResponse {
        match self.fs.create_async(path, mode).await {
            Ok(file) => {
                let h = self.alloc_handle();
                lock(&self.file_handles).insert(h, Arc::new(file));
                VfsResponse::FileHandle(h)
            }
            Err(e) => err(e),
        }
    }

    async fn handle_read_at(&self, handle: u64, offset: u64, size: u32) -> VfsResponse {
        let len = size as usize;
        let result = match self.file(handle) {
            Some(AnyFile::Plain(f)) => f.read_at_async(len, offset).await,
            Some(AnyFile::Seekable(f)) => f.read_at_async(len, offset).await,
            None => return VfsResponse::Error("invalid file handle".into()),
        };
        match result {
            Ok(data) => VfsResponse::Data(data),
            Err(e) => err(e),
        }
    }

    async fn handle_write_at(&self, handle: u64, data: Vec<u8>, offset: u64) -> VfsResponse {
        let result = match self.file(handle) {
            Some(AnyFile::Plain(f)) => f.write_at_async(data, offset).await,
            Some(AnyFile::Seekable(f)) => f.write_at_async(data, offset).await,
            None => return VfsResponse::Error("invalid file handle".into()),
        };
        match result {
            Ok(n) => VfsResponse::BytesTransferred(n),
            Err(e) => err(e),
        }
    }

    async fn handle_seek(&self, handle: u64, pos: SeekFrom) -> VfsResponse {
        let Some(h) = lock(&self.seekable_handles).get(&handle).cloned() else {
            return VfsResponse::Error("handle is not seekable".into());
        };
        let base = match pos {
            SeekFrom::Start(n) => {
                h.position.store(n, Ordering::SeqCst);
                return VfsResponse::Position(n);
            }
            SeekFrom::Current(d) => (h.position.load(Ordering::SeqCst), d),
            SeekFrom::End(d) => match h.file.size_async().await {
                Ok(size) => (size, d),
                Err(e) => return err(e),
            },
        };
        match base.0.checked_add_signed(base.1) {
            Some(new_pos) => {
                h.position.store(new_pos, Ordering::SeqCst);
                VfsResponse::Position(new_pos)
            }
            None => VfsResponse::Error("seek to a negative or overflowing position".into()),
        }
    }

    async fn handle_truncate(&self, handle: u64, size: u64) -> VfsResponse {
        let result = match self.file(handle) {
            Some(AnyFile::Plain(f)) => f.truncate_async(size).await,
            Some(AnyFile::Seekable(f)) => f.truncate_async(size).await,
            None => return VfsResponse::Error("invalid file handle".into()),
        };
        ok_or_err(result)
    }

    async fn handle_sync(&self, handle: u64) -> VfsResponse {
        let result = match self.file(handle) {
            Some(AnyFile::Plain(f)) => f.sync_data_async().await,
            Some(AnyFile::Seekable(f)) => f.sync_data_async().await,
            None => return VfsResponse::Error("invalid file handle".into()),
        };
        ok_or_err(result)
    }

    async fn handle_file_size(&self, handle: u64) -> VfsResponse {
        let result = match self.file(handle) {
            Some(AnyFile::Plain(f)) => f.size_async().await,
            Some(AnyFile::Seekable(f)) => f.size_async().await,
            None => return VfsResponse::Error("invalid file handle".into()),
        };
        match result {
            Ok(s) => VfsResponse::Size(s),
            Err(e) => err(e),
        }
    }

    async fn handle_file_metadata(&self, handle: u64) -> VfsResponse {
        let result = match self.file(handle) {
            Some(AnyFile::Plain(f)) => f.metadata_async().await,
            Some(AnyFile::Seekable(f)) => f.metadata_async().await,
            None => return VfsResponse::Error("invalid file handle".into()),
        };
        match result {
            Ok(m) => VfsResponse::Metadata(m.into()),
            Err(e) => err(e),
        }
    }

    // ── Directory handle operations ──

    async fn handle_open_dir(&self, path: String) -> VfsResponse {
        match self.fs.open_directory_async(path.clone()).await {
            Ok(_dir) => {
                let h = self.alloc_handle();
                lock(&self.dir_handles).insert(h, DirHandle { path });
                VfsResponse::DirHandle(h)
            }
            Err(e) => err(e),
        }
    }

    async fn list_dir(&self, path: String) -> Result<Vec<VfsDirEntry>, VfsResponse> {
        let dir = self.fs.open_directory_async(path).await.map_err(err)?;
        dir.list_async().await.map_err(err)
    }

    async fn handle_dir_list(&self, handle: u64) -> VfsResponse {
        let path = match self.dir_path(handle) {
            Ok(p) => p,
            Err(resp) => return resp,
        };
        match self.list_dir(path).await {
            Ok(entries) => VfsResponse::DirEntries(entries.into_iter().map(Into::into).collect()),
            Err(resp) => resp,
        }
    }

    async fn handle_dir_get_entry(&self, handle: u64, name: String) -> VfsResponse {
        let path = match self.dir_path(handle) {
            Ok(p) => p,
            Err(resp) => return resp,
        };
        let dir = match self.fs.open_directory_async(path).await {
            Ok(d) => d,
            Err(e) => return err(e),
        };
        match dir.get_entry_async(name).await {
            Ok(entry) => VfsResponse::DirEntry(entry.map(Into::into)),
            Err(e) => err(e),
        }
    }

    async fn handle_dir_create_dir(&self, handle: u64, name: String) -> VfsResponse {
        let path = match self.dir_path(handle) {
            Ok(p) => child_path(&p, &name),
            Err(resp) => return resp,
        };
        match self.fs.mkdir_async(path.clone()).await {
            Ok(()) => {
                let h = self.alloc_handle();
                lock(&self.dir_handles).insert(h, DirHandle { path });
                VfsResponse::DirHandle(h)
            }
            Err(e) => err(e),
        }
    }

    pub fn close_all_handles(&self) {
        lock(&self.file_handles).clear();
        lock(&self.seekable_handles).clear();
        lock(&self.dir_handles).clear();
    }
}

fn ok_or_err<E: std::fmt::Display>(result: Result<(), E>) -> VfsResponse {
    match result {
        Ok(()) => VfsResponse::Ok,
        Err(e) => err(e),
    }
}

impl<F: AsyncVfsFileSystem> std::fmt::Debug for VfsDaemon<F> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("VfsDaemon")
            .field("file_handles", &lock(&self.file_handles).len())
            .field("seekable_handles", &lock(&self.seekable_handles).len())
            .field("dir_handles", &lock(&self.dir_handles).len())
            .finish()
    }
}
