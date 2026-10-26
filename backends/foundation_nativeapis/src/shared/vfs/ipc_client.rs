//! `VfsClient`: an `AsyncVfsFileSystem` whose operations run in a
//! `VfsDaemon`, reached through a `VfsTransport`.

use std::io::SeekFrom;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Arc;

use async_trait::async_trait;
use foundation_errstacks::ErrorTrace;

use super::async_traits::{
    AsyncSeekableVfsFile, AsyncVfsDirectory, AsyncVfsFile, AsyncVfsFileSystem,
};
use super::error::{VfsError, VfsResult};
use super::ipc_messages::{VfsRequest, VfsResponse};
use super::types::{OpenMode, VfsCapabilities, VfsDirEntry, VfsMetadata};

/// Carries VFS requests to a daemon.
#[async_trait]
pub trait VfsTransport: Send + Sync {
    /// Sends `req` and waits for its response.
    async fn request(&self, req: VfsRequest) -> VfsResponse;

    /// Sends a request whose response the caller does not need (`CloseFile`,
    /// `CloseDir`), without a wait. The remote handles call this from `Drop`,
    /// where they cannot await.
    fn release(&self, req: VfsRequest);
}

fn unexpected() -> ErrorTrace<VfsError> {
    ErrorTrace::new(VfsError::Backend {
        message: "unexpected response".into(),
    })
}

/// Sends `req` and turns a `VfsResponse::Error` into an `Err`.
async fn call(transport: &dyn VfsTransport, req: VfsRequest) -> VfsResult<VfsResponse> {
    match transport.request(req).await {
        VfsResponse::Error(message) => Err(ErrorTrace::new(VfsError::Backend { message })),
        resp => Ok(resp),
    }
}

async fn call_ok(transport: &dyn VfsTransport, req: VfsRequest) -> VfsResult<()> {
    match call(transport, req).await? {
        VfsResponse::Ok => Ok(()),
        _ => Err(unexpected()),
    }
}

async fn call_metadata(transport: &dyn VfsTransport, req: VfsRequest) -> VfsResult<VfsMetadata> {
    match call(transport, req).await? {
        VfsResponse::Metadata(m) => Ok(m.into()),
        _ => Err(unexpected()),
    }
}

async fn call_handle(transport: &dyn VfsTransport, req: VfsRequest) -> VfsResult<u64> {
    match call(transport, req).await? {
        VfsResponse::FileHandle(h) | VfsResponse::DirHandle(h) => Ok(h),
        _ => Err(unexpected()),
    }
}

// ── VfsClient ──

pub struct VfsClient {
    transport: Arc<dyn VfsTransport>,
    capabilities: VfsCapabilities,
}

impl std::fmt::Debug for VfsClient {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("VfsClient").finish_non_exhaustive()
    }
}

impl VfsClient {
    /// Connects a client. It asks the daemon for its capabilities once; when
    /// the daemon does not answer them, the client uses the defaults.
    pub async fn new(transport: Arc<dyn VfsTransport>) -> Self {
        let capabilities = match transport.request(VfsRequest::Capabilities).await {
            VfsResponse::Capabilities(c) => c.into(),
            _ => VfsCapabilities::default(),
        };
        Self {
            transport,
            capabilities,
        }
    }

    fn file(&self, handle: u64) -> RemoteFile {
        RemoteFile {
            handle,
            transport: Arc::clone(&self.transport),
        }
    }
}

#[async_trait]
impl AsyncVfsFileSystem for VfsClient {
    type File = RemoteFile;
    type SeekableFile = RemoteSeekableFile;
    type Directory = RemoteDirectory;

    fn capabilities(&self) -> VfsCapabilities {
        self.capabilities
    }

    async fn stat_async(&self, path: String) -> VfsResult<VfsMetadata> {
        call_metadata(&*self.transport, VfsRequest::Stat { path }).await
    }

    async fn exists_async(&self, path: String) -> VfsResult<bool> {
        match call(&*self.transport, VfsRequest::Exists { path }).await? {
            VfsResponse::Exists(b) => Ok(b),
            _ => Err(unexpected()),
        }
    }

    async fn chmod_async(&self, path: String, mode: u32) -> VfsResult<()> {
        call_ok(&*self.transport, VfsRequest::Chmod { path, mode }).await
    }

    async fn symlink_async(&self, target: String, link: String) -> VfsResult<()> {
        call_ok(&*self.transport, VfsRequest::Symlink { target, link }).await
    }

    async fn readlink_async(&self, path: String) -> VfsResult<String> {
        match call(&*self.transport, VfsRequest::ReadLink { path }).await? {
            VfsResponse::Path(s) => Ok(s),
            _ => Err(unexpected()),
        }
    }

    async fn rename_async(&self, from: String, to: String) -> VfsResult<()> {
        call_ok(&*self.transport, VfsRequest::Rename { from, to }).await
    }

    async fn remove_async(&self, path: String) -> VfsResult<()> {
        call_ok(&*self.transport, VfsRequest::Remove { path }).await
    }

    async fn open_async(&self, path: String, mode: OpenMode) -> VfsResult<RemoteFile> {
        let req = VfsRequest::Open {
            path,
            mode: mode.into(),
            seekable: false,
        };
        Ok(self.file(call_handle(&*self.transport, req).await?))
    }

    async fn open_seekable_async(
        &self,
        path: String,
        mode: OpenMode,
    ) -> VfsResult<RemoteSeekableFile> {
        let req = VfsRequest::Open {
            path,
            mode: mode.into(),
            seekable: true,
        };
        let handle = call_handle(&*self.transport, req).await?;
        Ok(RemoteSeekableFile::new(handle, Arc::clone(&self.transport)))
    }

    async fn open_directory_async(&self, path: String) -> VfsResult<RemoteDirectory> {
        let req = VfsRequest::OpenDir { path: path.clone() };
        let handle = call_handle(&*self.transport, req).await?;
        Ok(RemoteDirectory {
            handle,
            path,
            transport: Arc::clone(&self.transport),
        })
    }

    async fn create_async(&self, path: String, mode: u32) -> VfsResult<RemoteFile> {
        let req = VfsRequest::Create { path, mode };
        Ok(self.file(call_handle(&*self.transport, req).await?))
    }

    async fn mkdir_async(&self, path: String) -> VfsResult<()> {
        call_ok(&*self.transport, VfsRequest::Mkdir { path }).await
    }

    async fn inode_async(&self, path: String) -> VfsResult<u64> {
        match call(&*self.transport, VfsRequest::Inode { path }).await? {
            VfsResponse::Inode(ino) => Ok(ino),
            _ => Err(unexpected()),
        }
    }

    async fn path_by_inode_async(&self, ino: u64) -> VfsResult<String> {
        match call(&*self.transport, VfsRequest::PathByInode { ino }).await? {
            VfsResponse::Path(p) => Ok(p),
            _ => Err(unexpected()),
        }
    }

    async fn stat_by_inode_async(&self, ino: u64) -> VfsResult<VfsMetadata> {
        call_metadata(&*self.transport, VfsRequest::StatByInode { ino }).await
    }

    async fn read_file_async(&self, path: String) -> VfsResult<Vec<u8>> {
        match call(&*self.transport, VfsRequest::ReadFile { path }).await? {
            VfsResponse::Data(d) => Ok(d),
            _ => Err(unexpected()),
        }
    }

    async fn write_file_async(&self, path: String, data: Vec<u8>) -> VfsResult<()> {
        call_ok(&*self.transport, VfsRequest::WriteFile { path, data }).await
    }
}

// ── File handle operations shared by both remote file kinds ──

async fn remote_read_at(
    transport: &dyn VfsTransport,
    handle: u64,
    len: usize,
    offset: u64,
) -> VfsResult<Vec<u8>> {
    let size = u32::try_from(len).map_err(|_| {
        ErrorTrace::new(VfsError::Backend {
            message: format!("read of {len} bytes is larger than one IPC request allows"),
        })
    })?;
    match call(
        transport,
        VfsRequest::ReadAt {
            handle,
            offset,
            size,
        },
    )
    .await?
    {
        VfsResponse::Data(data) => Ok(data),
        _ => Err(unexpected()),
    }
}

async fn remote_write_at(
    transport: &dyn VfsTransport,
    handle: u64,
    data: Vec<u8>,
    offset: u64,
) -> VfsResult<usize> {
    match call(
        transport,
        VfsRequest::WriteAt {
            handle,
            data,
            offset,
        },
    )
    .await?
    {
        VfsResponse::BytesTransferred(n) => Ok(n),
        _ => Err(unexpected()),
    }
}

async fn remote_size(transport: &dyn VfsTransport, handle: u64) -> VfsResult<u64> {
    match call(transport, VfsRequest::FileSize { handle }).await? {
        VfsResponse::Size(s) => Ok(s),
        _ => Err(unexpected()),
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
            .finish_non_exhaustive()
    }
}

#[async_trait]
impl AsyncVfsFile for RemoteFile {
    async fn read_at_async(&self, len: usize, offset: u64) -> VfsResult<Vec<u8>> {
        remote_read_at(&*self.transport, self.handle, len, offset).await
    }

    async fn write_at_async(&self, data: Vec<u8>, offset: u64) -> VfsResult<usize> {
        remote_write_at(&*self.transport, self.handle, data, offset).await
    }

    async fn sync_data_async(&self) -> VfsResult<()> {
        call_ok(
            &*self.transport,
            VfsRequest::SyncData {
                handle: self.handle,
            },
        )
        .await
    }

    async fn size_async(&self) -> VfsResult<u64> {
        remote_size(&*self.transport, self.handle).await
    }

    async fn truncate_async(&self, size: u64) -> VfsResult<()> {
        let req = VfsRequest::Truncate {
            handle: self.handle,
            size,
        };
        call_ok(&*self.transport, req).await
    }

    async fn metadata_async(&self) -> VfsResult<VfsMetadata> {
        call_metadata(
            &*self.transport,
            VfsRequest::FileMetadata {
                handle: self.handle,
            },
        )
        .await
    }
}

impl Drop for RemoteFile {
    fn drop(&mut self) {
        self.transport.release(VfsRequest::CloseFile {
            handle: self.handle,
        });
    }
}

// ── RemoteSeekableFile ──

pub struct RemoteSeekableFile {
    file: RemoteFile,
    position: AtomicU64,
}

impl RemoteSeekableFile {
    fn new(handle: u64, transport: Arc<dyn VfsTransport>) -> Self {
        Self {
            file: RemoteFile { handle, transport },
            position: AtomicU64::new(0),
        }
    }
}

impl std::fmt::Debug for RemoteSeekableFile {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("RemoteSeekableFile")
            .field("handle", &self.file.handle)
            .field("position", &self.position.load(Ordering::Relaxed))
            .finish()
    }
}

#[async_trait]
impl AsyncVfsFile for RemoteSeekableFile {
    async fn read_at_async(&self, len: usize, offset: u64) -> VfsResult<Vec<u8>> {
        self.file.read_at_async(len, offset).await
    }

    async fn write_at_async(&self, data: Vec<u8>, offset: u64) -> VfsResult<usize> {
        self.file.write_at_async(data, offset).await
    }

    async fn sync_data_async(&self) -> VfsResult<()> {
        self.file.sync_data_async().await
    }

    async fn size_async(&self) -> VfsResult<u64> {
        self.file.size_async().await
    }

    async fn truncate_async(&self, size: u64) -> VfsResult<()> {
        self.file.truncate_async(size).await
    }

    async fn metadata_async(&self) -> VfsResult<VfsMetadata> {
        self.file.metadata_async().await
    }
}

#[async_trait]
impl AsyncSeekableVfsFile for RemoteSeekableFile {
    async fn read_async(&mut self, len: usize) -> VfsResult<Vec<u8>> {
        let pos = self.position.load(Ordering::Relaxed);
        let data = self.file.read_at_async(len, pos).await?;
        self.position
            .fetch_add(data.len() as u64, Ordering::Relaxed);
        Ok(data)
    }

    async fn write_async(&mut self, data: Vec<u8>) -> VfsResult<usize> {
        let pos = self.position.load(Ordering::Relaxed);
        let n = self.file.write_at_async(data, pos).await?;
        self.position.fetch_add(n as u64, Ordering::Relaxed);
        Ok(n)
    }

    async fn seek_async(&mut self, pos: SeekFrom) -> VfsResult<u64> {
        let req = VfsRequest::Seek {
            handle: self.file.handle,
            pos: pos.into(),
        };
        match call(&*self.file.transport, req).await? {
            VfsResponse::Position(p) => {
                self.position.store(p, Ordering::Relaxed);
                Ok(p)
            }
            _ => Err(unexpected()),
        }
    }

    fn position_async(&self) -> u64 {
        self.position.load(Ordering::Relaxed)
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
            .finish_non_exhaustive()
    }
}

type RemoteDirBox =
    Box<dyn AsyncVfsDirectory<File = RemoteFile, SeekableFile = RemoteSeekableFile>>;

impl RemoteDirectory {
    /// The full path of `relative` in this directory. `"."` is the directory
    /// itself, and an absolute path is used as is.
    fn resolve_path(&self, relative: &str) -> String {
        if relative == "." || relative.is_empty() {
            self.path.clone()
        } else if relative.starts_with('/') {
            relative.to_string()
        } else if self.path == "/" {
            format!("/{relative}")
        } else {
            format!("{}/{relative}", self.path)
        }
    }

    fn file(&self, handle: u64) -> RemoteFile {
        RemoteFile {
            handle,
            transport: Arc::clone(&self.transport),
        }
    }

    fn dir(&self, handle: u64, path: String) -> RemoteDirBox {
        Box::new(RemoteDirectory {
            handle,
            path,
            transport: Arc::clone(&self.transport),
        })
    }
}

#[async_trait]
impl AsyncVfsDirectory for RemoteDirectory {
    type File = RemoteFile;
    type SeekableFile = RemoteSeekableFile;

    fn path(&self) -> String {
        self.path.clone()
    }

    async fn metadata_async(&self) -> VfsResult<VfsMetadata> {
        call_metadata(
            &*self.transport,
            VfsRequest::DirMetadata {
                handle: self.handle,
            },
        )
        .await
    }

    async fn list_async(&self) -> VfsResult<Vec<VfsDirEntry>> {
        match call(
            &*self.transport,
            VfsRequest::DirList {
                handle: self.handle,
            },
        )
        .await?
        {
            VfsResponse::DirEntries(entries) => Ok(entries.into_iter().map(Into::into).collect()),
            _ => Err(unexpected()),
        }
    }

    async fn get_entry_async(&self, name: String) -> VfsResult<Option<VfsDirEntry>> {
        let req = VfsRequest::DirGetEntry {
            handle: self.handle,
            name,
        };
        match call(&*self.transport, req).await? {
            VfsResponse::DirEntry(e) => Ok(e.map(Into::into)),
            _ => Err(unexpected()),
        }
    }

    async fn create_file_async(&self, name: String, mode: u32) -> VfsResult<RemoteFile> {
        let req = VfsRequest::DirCreateFile {
            handle: self.handle,
            name,
            mode,
        };
        Ok(self.file(call_handle(&*self.transport, req).await?))
    }

    async fn create_dir_async(&self, name: String) -> VfsResult<RemoteDirBox> {
        let path = self.resolve_path(&name);
        let req = VfsRequest::DirCreateDir {
            handle: self.handle,
            name,
        };
        Ok(self.dir(call_handle(&*self.transport, req).await?, path))
    }

    async fn remove_entry_async(&self, name: String) -> VfsResult<()> {
        let req = VfsRequest::DirRemoveEntry {
            handle: self.handle,
            name,
        };
        call_ok(&*self.transport, req).await
    }

    async fn rename_entry_async(&self, old_name: String, new_name: String) -> VfsResult<()> {
        let req = VfsRequest::Rename {
            from: self.resolve_path(&old_name),
            to: self.resolve_path(&new_name),
        };
        call_ok(&*self.transport, req).await
    }

    async fn open_async(&self, path: String, mode: OpenMode) -> VfsResult<RemoteFile> {
        let req = VfsRequest::Open {
            path: self.resolve_path(&path),
            mode: mode.into(),
            seekable: false,
        };
        Ok(self.file(call_handle(&*self.transport, req).await?))
    }

    async fn open_seekable_async(
        &self,
        path: String,
        mode: OpenMode,
    ) -> VfsResult<RemoteSeekableFile> {
        let req = VfsRequest::Open {
            path: self.resolve_path(&path),
            mode: mode.into(),
            seekable: true,
        };
        let handle = call_handle(&*self.transport, req).await?;
        Ok(RemoteSeekableFile::new(handle, Arc::clone(&self.transport)))
    }

    async fn open_directory_async(&self, path: String) -> VfsResult<RemoteDirBox> {
        let full = self.resolve_path(&path);
        let req = VfsRequest::OpenDir { path: full.clone() };
        Ok(self.dir(call_handle(&*self.transport, req).await?, full))
    }

    async fn stat_async(&self, path: String) -> VfsResult<VfsMetadata> {
        let req = VfsRequest::Stat {
            path: self.resolve_path(&path),
        };
        call_metadata(&*self.transport, req).await
    }

    async fn exists_async(&self, path: String) -> VfsResult<bool> {
        let req = VfsRequest::Exists {
            path: self.resolve_path(&path),
        };
        match call(&*self.transport, req).await? {
            VfsResponse::Exists(b) => Ok(b),
            _ => Err(unexpected()),
        }
    }
}

impl Drop for RemoteDirectory {
    fn drop(&mut self) {
        self.transport.release(VfsRequest::CloseDir {
            handle: self.handle,
        });
    }
}

// ── DirectTransport (in-process) ──

/// A transport that calls a `VfsDaemon` in the same process.
pub struct DirectTransport<F: AsyncVfsFileSystem> {
    daemon: super::ipc_daemon::VfsDaemon<F>,
}

impl<F: AsyncVfsFileSystem + 'static> DirectTransport<F> {
    pub fn new(daemon: super::ipc_daemon::VfsDaemon<F>) -> Self {
        Self { daemon }
    }
}

#[async_trait]
impl<F: AsyncVfsFileSystem + 'static> VfsTransport for DirectTransport<F> {
    async fn request(&self, req: VfsRequest) -> VfsResponse {
        self.daemon.dispatch(req).await
    }

    fn release(&self, req: VfsRequest) {
        // A close of an unknown handle has no caller to report to.
        let _ = self.daemon.release(req);
    }
}
