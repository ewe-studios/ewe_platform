use std::collections::HashMap;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Arc, Mutex};

use super::error::{VfsError, VfsResult};
use super::ipc_messages::*;
use super::traits::{SeekableVfsFile, VfsDirectory, VfsFile, VfsFileSystem};

pub struct VfsDaemon<F: VfsFileSystem> {
    fs: Arc<F>,
    file_handles: Mutex<HashMap<u64, Box<dyn VfsFile + Send + Sync>>>,
    seekable_handles: Mutex<HashMap<u64, Box<dyn SeekableVfsFile + Send + Sync>>>,
    dir_handles: Mutex<HashMap<u64, DirHandle>>,
    next_handle: AtomicU64,
}

struct DirHandle {
    path: String,
    entries: Option<Vec<super::types::VfsDirEntry>>,
}

impl<F: VfsFileSystem + 'static> VfsDaemon<F>
where
    F::File: Send + Sync + 'static,
    F::SeekableFile: Send + Sync + 'static,
    F::Directory: Send + Sync + 'static,
{
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

    pub fn dispatch(&self, req: VfsRequest) -> VfsResponse {
        match req {
            VfsRequest::Open { path, mode, seekable } => {
                if seekable {
                    self.handle_open_seekable(&path, mode.into())
                } else {
                    self.handle_open(&path, mode.into())
                }
            }
            VfsRequest::Create { path, mode } => self.handle_create(&path, mode),
            VfsRequest::ReadAt { handle, offset, size } => {
                self.handle_read_at(handle, offset, size)
            }
            VfsRequest::WriteAt { handle, data, offset } => {
                self.handle_write_at(handle, &data, offset)
            }
            VfsRequest::Seek { handle, pos } => self.handle_seek(handle, pos.into()),
            VfsRequest::Truncate { handle, size } => self.handle_truncate(handle, size),
            VfsRequest::SyncData { handle } => self.handle_sync(handle),
            VfsRequest::CloseFile { handle } => self.handle_close_file(handle),
            VfsRequest::FileSize { handle } => self.handle_file_size(handle),
            VfsRequest::FileMetadata { handle } => self.handle_file_metadata(handle),

            VfsRequest::Stat { path } => self.handle_stat(&path),
            VfsRequest::Exists { path } => self.handle_exists(&path),
            VfsRequest::Mkdir { path } => self.handle_mkdir(&path),
            VfsRequest::Remove { path } => self.handle_remove(&path),
            VfsRequest::Rename { from, to } => self.handle_rename(&from, &to),
            VfsRequest::Chmod { path, mode } => self.handle_chmod(&path, mode),
            VfsRequest::Symlink { target, link } => self.handle_symlink(&target, &link),
            VfsRequest::ReadLink { path } => self.handle_readlink(&path),

            VfsRequest::Inode { path } => self.handle_inode(&path),
            VfsRequest::PathByInode { ino } => self.handle_path_by_inode(ino),
            VfsRequest::StatByInode { ino } => self.handle_stat_by_inode(ino),

            VfsRequest::OpenDir { path } => self.handle_open_dir(&path),
            VfsRequest::DirList { handle } => self.handle_dir_list(handle),
            VfsRequest::DirGetEntry { handle, name } => self.handle_dir_get_entry(handle, &name),
            VfsRequest::DirPath { handle } => self.handle_dir_path(handle),
            VfsRequest::DirMetadata { handle } => self.handle_dir_metadata(handle),
            VfsRequest::DirCreateFile { handle, name, mode } => {
                self.handle_dir_create_file(handle, &name, mode)
            }
            VfsRequest::DirCreateDir { handle, name } => {
                self.handle_dir_create_dir(handle, &name)
            }
            VfsRequest::DirRemoveEntry { handle, name } => {
                self.handle_dir_remove_entry(handle, &name)
            }
            VfsRequest::CloseDir { handle } => self.handle_close_dir(handle),

            VfsRequest::ReadFile { path } => self.handle_read_file(&path),
            VfsRequest::WriteFile { path, data } => self.handle_write_file(&path, &data),
            VfsRequest::Capabilities => self.handle_capabilities(),
        }
    }

    // ── File handle operations ──

    fn handle_open(&self, path: &str, mode: super::types::OpenMode) -> VfsResponse {
        match self.fs.open(path, mode) {
            Ok(file) => {
                let h = self.alloc_handle();
                self.file_handles.lock().unwrap().insert(h, Box::new(file));
                VfsResponse::FileHandle(h)
            }
            Err(e) => VfsResponse::Error(format!("{e}")),
        }
    }

    fn handle_open_seekable(&self, path: &str, mode: super::types::OpenMode) -> VfsResponse {
        match self.fs.open_seekable(path, mode) {
            Ok(file) => {
                let h = self.alloc_handle();
                self.seekable_handles.lock().unwrap().insert(h, Box::new(file));
                VfsResponse::FileHandle(h)
            }
            Err(e) => VfsResponse::Error(format!("{e}")),
        }
    }

    fn handle_create(&self, path: &str, mode: u32) -> VfsResponse {
        match self.fs.create(path, mode) {
            Ok(file) => {
                let h = self.alloc_handle();
                self.file_handles.lock().unwrap().insert(h, Box::new(file));
                VfsResponse::FileHandle(h)
            }
            Err(e) => VfsResponse::Error(format!("{e}")),
        }
    }

    fn handle_read_at(&self, handle: u64, offset: u64, size: u32) -> VfsResponse {
        if let Some(file) = self.seekable_handles.lock().unwrap().get(&handle) {
            let mut buf = vec![0u8; size as usize];
            return match file.read_at(&mut buf, offset) {
                Ok(n) => {
                    buf.truncate(n);
                    VfsResponse::Data(buf)
                }
                Err(e) => VfsResponse::Error(format!("{e}")),
            };
        }
        if let Some(file) = self.file_handles.lock().unwrap().get(&handle) {
            let mut buf = vec![0u8; size as usize];
            return match file.read_at(&mut buf, offset) {
                Ok(n) => {
                    buf.truncate(n);
                    VfsResponse::Data(buf)
                }
                Err(e) => VfsResponse::Error(format!("{e}")),
            };
        }
        VfsResponse::Error("invalid file handle".into())
    }

    fn handle_write_at(&self, handle: u64, data: &[u8], offset: u64) -> VfsResponse {
        if let Some(file) = self.seekable_handles.lock().unwrap().get(&handle) {
            return match file.write_at(data, offset) {
                Ok(n) => VfsResponse::BytesTransferred(n),
                Err(e) => VfsResponse::Error(format!("{e}")),
            };
        }
        if let Some(file) = self.file_handles.lock().unwrap().get(&handle) {
            return match file.write_at(data, offset) {
                Ok(n) => VfsResponse::BytesTransferred(n),
                Err(e) => VfsResponse::Error(format!("{e}")),
            };
        }
        VfsResponse::Error("invalid file handle".into())
    }

    fn handle_seek(&self, handle: u64, pos: std::io::SeekFrom) -> VfsResponse {
        if let Some(file) = self.seekable_handles.lock().unwrap().get_mut(&handle) {
            return match file.seek(pos) {
                Ok(p) => VfsResponse::Position(p),
                Err(e) => VfsResponse::Error(format!("{e}")),
            };
        }
        VfsResponse::Error("handle is not seekable".into())
    }

    fn handle_truncate(&self, handle: u64, size: u64) -> VfsResponse {
        if let Some(file) = self.seekable_handles.lock().unwrap().get(&handle) {
            return match file.truncate(size) {
                Ok(()) => VfsResponse::Ok,
                Err(e) => VfsResponse::Error(format!("{e}")),
            };
        }
        if let Some(file) = self.file_handles.lock().unwrap().get(&handle) {
            return match file.truncate(size) {
                Ok(()) => VfsResponse::Ok,
                Err(e) => VfsResponse::Error(format!("{e}")),
            };
        }
        VfsResponse::Error("invalid file handle".into())
    }

    fn handle_sync(&self, handle: u64) -> VfsResponse {
        if let Some(file) = self.seekable_handles.lock().unwrap().get(&handle) {
            return match file.sync_data() {
                Ok(()) => VfsResponse::Ok,
                Err(e) => VfsResponse::Error(format!("{e}")),
            };
        }
        if let Some(file) = self.file_handles.lock().unwrap().get(&handle) {
            return match file.sync_data() {
                Ok(()) => VfsResponse::Ok,
                Err(e) => VfsResponse::Error(format!("{e}")),
            };
        }
        VfsResponse::Error("invalid file handle".into())
    }

    fn handle_close_file(&self, handle: u64) -> VfsResponse {
        let removed_file = self.file_handles.lock().unwrap().remove(&handle).is_some();
        let removed_seekable = self.seekable_handles.lock().unwrap().remove(&handle).is_some();
        if removed_file || removed_seekable {
            VfsResponse::Ok
        } else {
            VfsResponse::Error("invalid file handle".into())
        }
    }

    fn handle_file_size(&self, handle: u64) -> VfsResponse {
        if let Some(file) = self.seekable_handles.lock().unwrap().get(&handle) {
            return match file.size() {
                Ok(s) => VfsResponse::Size(s),
                Err(e) => VfsResponse::Error(format!("{e}")),
            };
        }
        if let Some(file) = self.file_handles.lock().unwrap().get(&handle) {
            return match file.size() {
                Ok(s) => VfsResponse::Size(s),
                Err(e) => VfsResponse::Error(format!("{e}")),
            };
        }
        VfsResponse::Error("invalid file handle".into())
    }

    fn handle_file_metadata(&self, handle: u64) -> VfsResponse {
        if let Some(file) = self.seekable_handles.lock().unwrap().get(&handle) {
            return match file.metadata() {
                Ok(m) => VfsResponse::Metadata(m.into()),
                Err(e) => VfsResponse::Error(format!("{e}")),
            };
        }
        if let Some(file) = self.file_handles.lock().unwrap().get(&handle) {
            return match file.metadata() {
                Ok(m) => VfsResponse::Metadata(m.into()),
                Err(e) => VfsResponse::Error(format!("{e}")),
            };
        }
        VfsResponse::Error("invalid file handle".into())
    }

    // ── Path operations ──

    fn handle_stat(&self, path: &str) -> VfsResponse {
        match self.fs.stat(path) {
            Ok(m) => VfsResponse::Metadata(m.into()),
            Err(e) => VfsResponse::Error(format!("{e}")),
        }
    }

    fn handle_exists(&self, path: &str) -> VfsResponse {
        match self.fs.exists(path) {
            Ok(b) => VfsResponse::Exists(b),
            Err(e) => VfsResponse::Error(format!("{e}")),
        }
    }

    fn handle_mkdir(&self, path: &str) -> VfsResponse {
        match self.fs.mkdir(path) {
            Ok(()) => VfsResponse::Ok,
            Err(e) => VfsResponse::Error(format!("{e}")),
        }
    }

    fn handle_remove(&self, path: &str) -> VfsResponse {
        match self.fs.remove(path) {
            Ok(()) => VfsResponse::Ok,
            Err(e) => VfsResponse::Error(format!("{e}")),
        }
    }

    fn handle_rename(&self, from: &str, to: &str) -> VfsResponse {
        match self.fs.rename(from, to) {
            Ok(()) => VfsResponse::Ok,
            Err(e) => VfsResponse::Error(format!("{e}")),
        }
    }

    fn handle_chmod(&self, path: &str, mode: u32) -> VfsResponse {
        match self.fs.chmod(path, mode) {
            Ok(()) => VfsResponse::Ok,
            Err(e) => VfsResponse::Error(format!("{e}")),
        }
    }

    fn handle_symlink(&self, target: &str, link: &str) -> VfsResponse {
        match self.fs.symlink(target, link) {
            Ok(()) => VfsResponse::Ok,
            Err(e) => VfsResponse::Error(format!("{e}")),
        }
    }

    fn handle_readlink(&self, path: &str) -> VfsResponse {
        match self.fs.readlink(path) {
            Ok(s) => VfsResponse::Path(s),
            Err(e) => VfsResponse::Error(format!("{e}")),
        }
    }

    fn handle_inode(&self, path: &str) -> VfsResponse {
        match self.fs.inode(path) {
            Ok(ino) => VfsResponse::Inode(ino),
            Err(e) => VfsResponse::Error(format!("{e}")),
        }
    }

    fn handle_path_by_inode(&self, ino: u64) -> VfsResponse {
        match self.fs.path_by_inode(ino) {
            Ok(p) => VfsResponse::Path(p),
            Err(e) => VfsResponse::Error(format!("{e}")),
        }
    }

    fn handle_stat_by_inode(&self, ino: u64) -> VfsResponse {
        match self.fs.stat_by_inode(ino) {
            Ok(m) => VfsResponse::Metadata(m.into()),
            Err(e) => VfsResponse::Error(format!("{e}")),
        }
    }

    // ── Directory handle operations ──

    fn handle_open_dir(&self, path: &str) -> VfsResponse {
        match self.fs.open_directory(path) {
            Ok(_dir) => {
                let h = self.alloc_handle();
                self.dir_handles.lock().unwrap().insert(
                    h,
                    DirHandle {
                        path: path.to_string(),
                        entries: None,
                    },
                );
                VfsResponse::DirHandle(h)
            }
            Err(e) => VfsResponse::Error(format!("{e}")),
        }
    }

    fn handle_dir_list(&self, handle: u64) -> VfsResponse {
        let path = {
            let dirs = self.dir_handles.lock().unwrap();
            match dirs.get(&handle) {
                Some(dh) => dh.path.clone(),
                None => return VfsResponse::Error("invalid dir handle".into()),
            }
        };
        match self.fs.open_directory(&path) {
            Ok(dir) => match dir.list() {
                Ok(entries) => {
                    VfsResponse::DirEntries(entries.into_iter().map(Into::into).collect())
                }
                Err(e) => VfsResponse::Error(format!("{e}")),
            },
            Err(e) => VfsResponse::Error(format!("{e}")),
        }
    }

    fn handle_dir_get_entry(&self, handle: u64, name: &str) -> VfsResponse {
        let path = {
            let dirs = self.dir_handles.lock().unwrap();
            match dirs.get(&handle) {
                Some(dh) => dh.path.clone(),
                None => return VfsResponse::Error("invalid dir handle".into()),
            }
        };
        match self.fs.open_directory(&path) {
            Ok(dir) => match dir.get_entry(name) {
                Ok(entry) => VfsResponse::DirEntry(entry.map(Into::into)),
                Err(e) => VfsResponse::Error(format!("{e}")),
            },
            Err(e) => VfsResponse::Error(format!("{e}")),
        }
    }

    fn handle_dir_path(&self, handle: u64) -> VfsResponse {
        let dirs = self.dir_handles.lock().unwrap();
        match dirs.get(&handle) {
            Some(dh) => VfsResponse::Path(dh.path.clone()),
            None => VfsResponse::Error("invalid dir handle".into()),
        }
    }

    fn handle_dir_metadata(&self, handle: u64) -> VfsResponse {
        let path = {
            let dirs = self.dir_handles.lock().unwrap();
            match dirs.get(&handle) {
                Some(dh) => dh.path.clone(),
                None => return VfsResponse::Error("invalid dir handle".into()),
            }
        };
        match self.fs.stat(&path) {
            Ok(m) => VfsResponse::Metadata(m.into()),
            Err(e) => VfsResponse::Error(format!("{e}")),
        }
    }

    fn handle_dir_create_file(&self, handle: u64, name: &str, mode: u32) -> VfsResponse {
        let path = {
            let dirs = self.dir_handles.lock().unwrap();
            match dirs.get(&handle) {
                Some(dh) => dh.path.clone(),
                None => return VfsResponse::Error("invalid dir handle".into()),
            }
        };
        let child_path = if path == "/" {
            format!("/{name}")
        } else {
            format!("{path}/{name}")
        };
        match self.fs.create(&child_path, mode) {
            Ok(file) => {
                let h = self.alloc_handle();
                self.file_handles.lock().unwrap().insert(h, Box::new(file));
                VfsResponse::FileHandle(h)
            }
            Err(e) => VfsResponse::Error(format!("{e}")),
        }
    }

    fn handle_dir_create_dir(&self, handle: u64, name: &str) -> VfsResponse {
        let path = {
            let dirs = self.dir_handles.lock().unwrap();
            match dirs.get(&handle) {
                Some(dh) => dh.path.clone(),
                None => return VfsResponse::Error("invalid dir handle".into()),
            }
        };
        let child_path = if path == "/" {
            format!("/{name}")
        } else {
            format!("{path}/{name}")
        };
        match self.fs.mkdir(&child_path) {
            Ok(()) => {
                let h = self.alloc_handle();
                self.dir_handles.lock().unwrap().insert(
                    h,
                    DirHandle {
                        path: child_path,
                        entries: None,
                    },
                );
                VfsResponse::DirHandle(h)
            }
            Err(e) => VfsResponse::Error(format!("{e}")),
        }
    }

    fn handle_dir_remove_entry(&self, handle: u64, name: &str) -> VfsResponse {
        let path = {
            let dirs = self.dir_handles.lock().unwrap();
            match dirs.get(&handle) {
                Some(dh) => dh.path.clone(),
                None => return VfsResponse::Error("invalid dir handle".into()),
            }
        };
        let child_path = if path == "/" {
            format!("/{name}")
        } else {
            format!("{path}/{name}")
        };
        match self.fs.remove(&child_path) {
            Ok(()) => VfsResponse::Ok,
            Err(e) => VfsResponse::Error(format!("{e}")),
        }
    }

    fn handle_close_dir(&self, handle: u64) -> VfsResponse {
        if self.dir_handles.lock().unwrap().remove(&handle).is_some() {
            VfsResponse::Ok
        } else {
            VfsResponse::Error("invalid dir handle".into())
        }
    }

    // ── Convenience ──

    fn handle_read_file(&self, path: &str) -> VfsResponse {
        match self.fs.read_file(path) {
            Ok(data) => VfsResponse::Data(data),
            Err(e) => VfsResponse::Error(format!("{e}")),
        }
    }

    fn handle_write_file(&self, path: &str, data: &[u8]) -> VfsResponse {
        match self.fs.write_file(path, data) {
            Ok(()) => VfsResponse::Ok,
            Err(e) => VfsResponse::Error(format!("{e}")),
        }
    }

    fn handle_capabilities(&self) -> VfsResponse {
        VfsResponse::Capabilities(self.fs.capabilities().into())
    }

    pub fn close_all_handles(&self) {
        self.file_handles.lock().unwrap().clear();
        self.seekable_handles.lock().unwrap().clear();
        self.dir_handles.lock().unwrap().clear();
    }
}

impl<F: VfsFileSystem> std::fmt::Debug for VfsDaemon<F> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("VfsDaemon")
            .field("file_handles", &self.file_handles.lock().unwrap().len())
            .field("seekable_handles", &self.seekable_handles.lock().unwrap().len())
            .field("dir_handles", &self.dir_handles.lock().unwrap().len())
            .finish()
    }
}
