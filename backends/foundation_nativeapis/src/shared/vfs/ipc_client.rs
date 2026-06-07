use std::io::SeekFrom;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Arc;

use foundation_errstacks::ErrorTrace;

use super::error::{VfsError, VfsResult};
use super::ipc_messages::*;
use super::traits::{SeekableVfsFile, VfsDirectory, VfsFile, VfsFileSystem};
use super::types::{OpenMode, VfsCapabilities, VfsDirEntry, VfsMetadata};

pub trait VfsTransport: Send + Sync {
    fn request(&self, req: VfsRequest) -> VfsResponse;
}

fn into_result(resp: VfsResponse) -> VfsResult<VfsResponse> {
    if let VfsResponse::Error(msg) = resp {
        Err(ErrorTrace::new(VfsError::Backend { message: msg }))
    } else {
        Ok(resp)
    }
}

fn expect_ok(resp: VfsResponse) -> VfsResult<()> {
    match into_result(resp)? {
        VfsResponse::Ok => Ok(()),
        _ => Err(ErrorTrace::new(VfsError::Backend {
            message: "unexpected response".into(),
        })),
    }
}

// ── VfsClient ──

pub struct VfsClient {
    transport: Arc<dyn VfsTransport>,
}

impl std::fmt::Debug for VfsClient {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("VfsClient").finish()
    }
}

impl VfsClient {
    pub fn new(transport: Arc<dyn VfsTransport>) -> Self {
        Self { transport }
    }
}

impl VfsFileSystem for VfsClient {
    type File = RemoteFile;
    type SeekableFile = RemoteSeekableFile;
    type Directory = RemoteDirectory;

    fn capabilities(&self) -> VfsCapabilities {
        match self.transport.request(VfsRequest::Capabilities) {
            VfsResponse::Capabilities(c) => c.into(),
            _ => VfsCapabilities::default(),
        }
    }

    fn stat(&self, path: &str) -> VfsResult<VfsMetadata> {
        match into_result(self.transport.request(VfsRequest::Stat { path: path.into() }))? {
            VfsResponse::Metadata(m) => Ok(m.into()),
            _ => Err(ErrorTrace::new(VfsError::Backend {
                message: "unexpected response".into(),
            })),
        }
    }

    fn exists(&self, path: &str) -> VfsResult<bool> {
        match into_result(self.transport.request(VfsRequest::Exists { path: path.into() }))? {
            VfsResponse::Exists(b) => Ok(b),
            _ => Err(ErrorTrace::new(VfsError::Backend {
                message: "unexpected response".into(),
            })),
        }
    }

    fn chmod(&self, path: &str, mode: u32) -> VfsResult<()> {
        expect_ok(self.transport.request(VfsRequest::Chmod {
            path: path.into(),
            mode,
        }))
    }

    fn symlink(&self, target: &str, link: &str) -> VfsResult<()> {
        expect_ok(self.transport.request(VfsRequest::Symlink {
            target: target.into(),
            link: link.into(),
        }))
    }

    fn readlink(&self, path: &str) -> VfsResult<String> {
        match into_result(self.transport.request(VfsRequest::ReadLink { path: path.into() }))? {
            VfsResponse::Path(s) => Ok(s),
            _ => Err(ErrorTrace::new(VfsError::Backend {
                message: "unexpected response".into(),
            })),
        }
    }

    fn rename(&self, from: &str, to: &str) -> VfsResult<()> {
        expect_ok(self.transport.request(VfsRequest::Rename {
            from: from.into(),
            to: to.into(),
        }))
    }

    fn remove(&self, path: &str) -> VfsResult<()> {
        expect_ok(
            self.transport
                .request(VfsRequest::Remove { path: path.into() }),
        )
    }

    fn open(&self, path: &str, mode: OpenMode) -> VfsResult<RemoteFile> {
        let resp = into_result(self.transport.request(VfsRequest::Open {
            path: path.into(),
            mode: mode.into(),
            seekable: false,
        }))?;
        match resp {
            VfsResponse::FileHandle(h) => Ok(RemoteFile {
                handle: h,
                transport: Arc::clone(&self.transport),
            }),
            _ => Err(ErrorTrace::new(VfsError::Backend {
                message: "unexpected response".into(),
            })),
        }
    }

    fn open_seekable(&self, path: &str, mode: OpenMode) -> VfsResult<RemoteSeekableFile> {
        let resp = into_result(self.transport.request(VfsRequest::Open {
            path: path.into(),
            mode: mode.into(),
            seekable: true,
        }))?;
        match resp {
            VfsResponse::FileHandle(h) => Ok(RemoteSeekableFile {
                handle: h,
                position: AtomicU64::new(0),
                transport: Arc::clone(&self.transport),
            }),
            _ => Err(ErrorTrace::new(VfsError::Backend {
                message: "unexpected response".into(),
            })),
        }
    }

    fn open_directory(&self, path: &str) -> VfsResult<RemoteDirectory> {
        let resp = into_result(
            self.transport
                .request(VfsRequest::OpenDir { path: path.into() }),
        )?;
        match resp {
            VfsResponse::DirHandle(h) => Ok(RemoteDirectory {
                handle: h,
                path: path.to_string(),
                transport: Arc::clone(&self.transport),
            }),
            _ => Err(ErrorTrace::new(VfsError::Backend {
                message: "unexpected response".into(),
            })),
        }
    }

    fn create(&self, path: &str, mode: u32) -> VfsResult<RemoteFile> {
        let resp = into_result(self.transport.request(VfsRequest::Create {
            path: path.into(),
            mode,
        }))?;
        match resp {
            VfsResponse::FileHandle(h) => Ok(RemoteFile {
                handle: h,
                transport: Arc::clone(&self.transport),
            }),
            _ => Err(ErrorTrace::new(VfsError::Backend {
                message: "unexpected response".into(),
            })),
        }
    }

    fn mkdir(&self, path: &str) -> VfsResult<()> {
        expect_ok(
            self.transport
                .request(VfsRequest::Mkdir { path: path.into() }),
        )
    }

    fn inode(&self, path: &str) -> VfsResult<u64> {
        match into_result(self.transport.request(VfsRequest::Inode { path: path.into() }))? {
            VfsResponse::Inode(ino) => Ok(ino),
            _ => Err(ErrorTrace::new(VfsError::Backend {
                message: "unexpected response".into(),
            })),
        }
    }

    fn path_by_inode(&self, ino: u64) -> VfsResult<String> {
        match into_result(self.transport.request(VfsRequest::PathByInode { ino }))? {
            VfsResponse::Path(p) => Ok(p),
            _ => Err(ErrorTrace::new(VfsError::Backend {
                message: "unexpected response".into(),
            })),
        }
    }

    fn stat_by_inode(&self, ino: u64) -> VfsResult<VfsMetadata> {
        match into_result(self.transport.request(VfsRequest::StatByInode { ino }))? {
            VfsResponse::Metadata(m) => Ok(m.into()),
            _ => Err(ErrorTrace::new(VfsError::Backend {
                message: "unexpected response".into(),
            })),
        }
    }

    fn read_file(&self, path: &str) -> VfsResult<Vec<u8>> {
        match into_result(
            self.transport
                .request(VfsRequest::ReadFile { path: path.into() }),
        )? {
            VfsResponse::Data(d) => Ok(d),
            _ => Err(ErrorTrace::new(VfsError::Backend {
                message: "unexpected response".into(),
            })),
        }
    }

    fn write_file(&self, path: &str, data: &[u8]) -> VfsResult<()> {
        expect_ok(self.transport.request(VfsRequest::WriteFile {
            path: path.into(),
            data: data.to_vec(),
        }))
    }
}

// ── RemoteFile ──

pub struct RemoteFile {
    handle: u64,
    transport: Arc<dyn VfsTransport>,
}

impl std::fmt::Debug for RemoteFile {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("RemoteFile")
            .field("handle", &self.handle)
            .finish()
    }
}

impl VfsFile for RemoteFile {
    fn read_at(&self, buf: &mut [u8], offset: u64) -> VfsResult<usize> {
        let resp = into_result(self.transport.request(VfsRequest::ReadAt {
            handle: self.handle,
            offset,
            size: buf.len() as u32,
        }))?;
        match resp {
            VfsResponse::Data(data) => {
                let n = data.len().min(buf.len());
                buf[..n].copy_from_slice(&data[..n]);
                Ok(n)
            }
            _ => Err(ErrorTrace::new(VfsError::Backend {
                message: "unexpected response".into(),
            })),
        }
    }

    fn write_at(&self, buf: &[u8], offset: u64) -> VfsResult<usize> {
        let resp = into_result(self.transport.request(VfsRequest::WriteAt {
            handle: self.handle,
            data: buf.to_vec(),
            offset,
        }))?;
        match resp {
            VfsResponse::BytesTransferred(n) => Ok(n),
            _ => Err(ErrorTrace::new(VfsError::Backend {
                message: "unexpected response".into(),
            })),
        }
    }

    fn sync_data(&self) -> VfsResult<()> {
        expect_ok(
            self.transport
                .request(VfsRequest::SyncData { handle: self.handle }),
        )
    }

    fn size(&self) -> VfsResult<u64> {
        match into_result(
            self.transport
                .request(VfsRequest::FileSize { handle: self.handle }),
        )? {
            VfsResponse::Size(s) => Ok(s),
            _ => Err(ErrorTrace::new(VfsError::Backend {
                message: "unexpected response".into(),
            })),
        }
    }

    fn truncate(&self, size: u64) -> VfsResult<()> {
        expect_ok(self.transport.request(VfsRequest::Truncate {
            handle: self.handle,
            size,
        }))
    }

    fn metadata(&self) -> VfsResult<VfsMetadata> {
        match into_result(
            self.transport
                .request(VfsRequest::FileMetadata { handle: self.handle }),
        )? {
            VfsResponse::Metadata(m) => Ok(m.into()),
            _ => Err(ErrorTrace::new(VfsError::Backend {
                message: "unexpected response".into(),
            })),
        }
    }
}

impl Drop for RemoteFile {
    fn drop(&mut self) {
        let _ = self
            .transport
            .request(VfsRequest::CloseFile { handle: self.handle });
    }
}

// ── RemoteSeekableFile ──

pub struct RemoteSeekableFile {
    handle: u64,
    position: AtomicU64,
    transport: Arc<dyn VfsTransport>,
}

impl std::fmt::Debug for RemoteSeekableFile {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("RemoteSeekableFile")
            .field("handle", &self.handle)
            .field("position", &self.position.load(Ordering::Relaxed))
            .finish()
    }
}

impl VfsFile for RemoteSeekableFile {
    fn read_at(&self, buf: &mut [u8], offset: u64) -> VfsResult<usize> {
        let resp = into_result(self.transport.request(VfsRequest::ReadAt {
            handle: self.handle,
            offset,
            size: buf.len() as u32,
        }))?;
        match resp {
            VfsResponse::Data(data) => {
                let n = data.len().min(buf.len());
                buf[..n].copy_from_slice(&data[..n]);
                Ok(n)
            }
            _ => Err(ErrorTrace::new(VfsError::Backend {
                message: "unexpected response".into(),
            })),
        }
    }

    fn write_at(&self, buf: &[u8], offset: u64) -> VfsResult<usize> {
        let resp = into_result(self.transport.request(VfsRequest::WriteAt {
            handle: self.handle,
            data: buf.to_vec(),
            offset,
        }))?;
        match resp {
            VfsResponse::BytesTransferred(n) => Ok(n),
            _ => Err(ErrorTrace::new(VfsError::Backend {
                message: "unexpected response".into(),
            })),
        }
    }

    fn sync_data(&self) -> VfsResult<()> {
        expect_ok(
            self.transport
                .request(VfsRequest::SyncData { handle: self.handle }),
        )
    }

    fn size(&self) -> VfsResult<u64> {
        match into_result(
            self.transport
                .request(VfsRequest::FileSize { handle: self.handle }),
        )? {
            VfsResponse::Size(s) => Ok(s),
            _ => Err(ErrorTrace::new(VfsError::Backend {
                message: "unexpected response".into(),
            })),
        }
    }

    fn truncate(&self, size: u64) -> VfsResult<()> {
        expect_ok(self.transport.request(VfsRequest::Truncate {
            handle: self.handle,
            size,
        }))
    }

    fn metadata(&self) -> VfsResult<VfsMetadata> {
        match into_result(
            self.transport
                .request(VfsRequest::FileMetadata { handle: self.handle }),
        )? {
            VfsResponse::Metadata(m) => Ok(m.into()),
            _ => Err(ErrorTrace::new(VfsError::Backend {
                message: "unexpected response".into(),
            })),
        }
    }
}

impl SeekableVfsFile for RemoteSeekableFile {
    fn read(&mut self, buf: &mut [u8]) -> VfsResult<usize> {
        let pos = self.position.load(Ordering::Relaxed);
        let n = self.read_at(buf, pos)?;
        self.position.fetch_add(n as u64, Ordering::Relaxed);
        Ok(n)
    }

    fn write(&mut self, buf: &[u8]) -> VfsResult<usize> {
        let pos = self.position.load(Ordering::Relaxed);
        let n = self.write_at(buf, pos)?;
        self.position.fetch_add(n as u64, Ordering::Relaxed);
        Ok(n)
    }

    fn seek(&mut self, pos: SeekFrom) -> VfsResult<u64> {
        let resp = into_result(self.transport.request(VfsRequest::Seek {
            handle: self.handle,
            pos: pos.into(),
        }))?;
        match resp {
            VfsResponse::Position(p) => {
                self.position.store(p, Ordering::Relaxed);
                Ok(p)
            }
            _ => Err(ErrorTrace::new(VfsError::Backend {
                message: "unexpected response".into(),
            })),
        }
    }

    fn position(&self) -> u64 {
        self.position.load(Ordering::Relaxed)
    }
}

impl Drop for RemoteSeekableFile {
    fn drop(&mut self) {
        let _ = self
            .transport
            .request(VfsRequest::CloseFile { handle: self.handle });
    }
}

// ── RemoteDirectory ──

pub struct RemoteDirectory {
    handle: u64,
    path: String,
    transport: Arc<dyn VfsTransport>,
}

impl std::fmt::Debug for RemoteDirectory {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("RemoteDirectory")
            .field("handle", &self.handle)
            .field("path", &self.path)
            .finish()
    }
}

impl RemoteDirectory {
    fn child_path(&self, name: &str) -> String {
        if self.path == "/" {
            format!("/{name}")
        } else {
            format!("{}/{name}", self.path)
        }
    }

    fn resolve_path(&self, relative: &str) -> String {
        if relative == "." || relative.is_empty() {
            self.path.clone()
        } else if self.path == "/" {
            format!("/{relative}")
        } else {
            format!("{}/{relative}", self.path)
        }
    }
}

impl VfsDirectory for RemoteDirectory {
    type File = RemoteFile;
    type SeekableFile = RemoteSeekableFile;

    fn path(&self) -> &str {
        &self.path
    }

    fn metadata(&self) -> VfsResult<VfsMetadata> {
        match into_result(
            self.transport
                .request(VfsRequest::DirMetadata { handle: self.handle }),
        )? {
            VfsResponse::Metadata(m) => Ok(m.into()),
            _ => Err(ErrorTrace::new(VfsError::Backend {
                message: "unexpected response".into(),
            })),
        }
    }

    fn list(&self) -> VfsResult<Vec<VfsDirEntry>> {
        match into_result(
            self.transport
                .request(VfsRequest::DirList { handle: self.handle }),
        )? {
            VfsResponse::DirEntries(entries) => {
                Ok(entries.into_iter().map(Into::into).collect())
            }
            _ => Err(ErrorTrace::new(VfsError::Backend {
                message: "unexpected response".into(),
            })),
        }
    }

    fn get_entry(&self, name: &str) -> VfsResult<Option<VfsDirEntry>> {
        match into_result(self.transport.request(VfsRequest::DirGetEntry {
            handle: self.handle,
            name: name.into(),
        }))? {
            VfsResponse::DirEntry(e) => Ok(e.map(Into::into)),
            _ => Err(ErrorTrace::new(VfsError::Backend {
                message: "unexpected response".into(),
            })),
        }
    }

    fn create_file(&self, name: &str, mode: u32) -> VfsResult<RemoteFile> {
        let resp = into_result(self.transport.request(VfsRequest::DirCreateFile {
            handle: self.handle,
            name: name.into(),
            mode,
        }))?;
        match resp {
            VfsResponse::FileHandle(h) => Ok(RemoteFile {
                handle: h,
                transport: Arc::clone(&self.transport),
            }),
            _ => Err(ErrorTrace::new(VfsError::Backend {
                message: "unexpected response".into(),
            })),
        }
    }

    fn create_dir(
        &self,
        name: &str,
    ) -> VfsResult<Box<dyn VfsDirectory<File = RemoteFile, SeekableFile = RemoteSeekableFile>>>
    {
        let resp = into_result(self.transport.request(VfsRequest::DirCreateDir {
            handle: self.handle,
            name: name.into(),
        }))?;
        match resp {
            VfsResponse::DirHandle(h) => Ok(Box::new(RemoteDirectory {
                handle: h,
                path: self.child_path(name),
                transport: Arc::clone(&self.transport),
            })),
            _ => Err(ErrorTrace::new(VfsError::Backend {
                message: "unexpected response".into(),
            })),
        }
    }

    fn remove_entry(&self, name: &str) -> VfsResult<()> {
        expect_ok(self.transport.request(VfsRequest::DirRemoveEntry {
            handle: self.handle,
            name: name.into(),
        }))
    }

    fn rename_entry(&self, old_name: &str, new_name: &str) -> VfsResult<()> {
        let from = self.child_path(old_name);
        let to = self.child_path(new_name);
        expect_ok(self.transport.request(VfsRequest::Rename { from, to }))
    }

    fn open(&self, path: &str, mode: OpenMode) -> VfsResult<RemoteFile> {
        let full = self.resolve_path(path);
        let resp = into_result(self.transport.request(VfsRequest::Open {
            path: full,
            mode: mode.into(),
            seekable: false,
        }))?;
        match resp {
            VfsResponse::FileHandle(h) => Ok(RemoteFile {
                handle: h,
                transport: Arc::clone(&self.transport),
            }),
            _ => Err(ErrorTrace::new(VfsError::Backend {
                message: "unexpected response".into(),
            })),
        }
    }

    fn open_seekable(&self, path: &str, mode: OpenMode) -> VfsResult<RemoteSeekableFile> {
        let full = self.resolve_path(path);
        let resp = into_result(self.transport.request(VfsRequest::Open {
            path: full,
            mode: mode.into(),
            seekable: true,
        }))?;
        match resp {
            VfsResponse::FileHandle(h) => Ok(RemoteSeekableFile {
                handle: h,
                position: AtomicU64::new(0),
                transport: Arc::clone(&self.transport),
            }),
            _ => Err(ErrorTrace::new(VfsError::Backend {
                message: "unexpected response".into(),
            })),
        }
    }

    fn open_directory(
        &self,
        path: &str,
    ) -> VfsResult<Box<dyn VfsDirectory<File = RemoteFile, SeekableFile = RemoteSeekableFile>>>
    {
        let full = self.resolve_path(path);
        let resp = into_result(
            self.transport
                .request(VfsRequest::OpenDir { path: full.clone() }),
        )?;
        match resp {
            VfsResponse::DirHandle(h) => Ok(Box::new(RemoteDirectory {
                handle: h,
                path: full,
                transport: Arc::clone(&self.transport),
            })),
            _ => Err(ErrorTrace::new(VfsError::Backend {
                message: "unexpected response".into(),
            })),
        }
    }

    fn stat(&self, path: &str) -> VfsResult<VfsMetadata> {
        let full = self.resolve_path(path);
        match into_result(self.transport.request(VfsRequest::Stat { path: full }))? {
            VfsResponse::Metadata(m) => Ok(m.into()),
            _ => Err(ErrorTrace::new(VfsError::Backend {
                message: "unexpected response".into(),
            })),
        }
    }

    fn exists(&self, path: &str) -> VfsResult<bool> {
        let full = self.resolve_path(path);
        match into_result(self.transport.request(VfsRequest::Exists { path: full }))? {
            VfsResponse::Exists(b) => Ok(b),
            _ => Err(ErrorTrace::new(VfsError::Backend {
                message: "unexpected response".into(),
            })),
        }
    }
}

impl Drop for RemoteDirectory {
    fn drop(&mut self) {
        let _ = self
            .transport
            .request(VfsRequest::CloseDir { handle: self.handle });
    }
}

// ── DirectTransport (for in-process testing) ──

pub struct DirectTransport<F: VfsFileSystem> {
    daemon: super::ipc_daemon::VfsDaemon<F>,
}

impl<F: VfsFileSystem + 'static> DirectTransport<F>
where
    F::File: Send + Sync + 'static,
    F::SeekableFile: Send + Sync + 'static,
    F::Directory: Send + Sync + 'static,
{
    pub fn new(daemon: super::ipc_daemon::VfsDaemon<F>) -> Self {
        Self { daemon }
    }
}

impl<F: VfsFileSystem + 'static> VfsTransport for DirectTransport<F>
where
    F::File: Send + Sync + 'static,
    F::SeekableFile: Send + Sync + 'static,
    F::Directory: Send + Sync + 'static,
{
    fn request(&self, req: VfsRequest) -> VfsResponse {
        self.daemon.dispatch(req)
    }
}
