//! The file calls: the uploads and the reads.
//!
//! A path is absolute, or it joins the work directory of the sandbox and cannot
//! leave it (issue #1154). Every open goes through `openat2` with
//! `RESOLVE_IN_ROOT` on a directory descriptor of the root, so a symbolic link,
//! also an absolute one, resolves inside the root.

use std::collections::HashMap;
use std::ffi::CString;
use std::io;
use std::os::fd::{AsRawFd, OwnedFd};
use std::path::{Component, Path};
use std::pin::Pin;
use std::sync::{Arc, Mutex};
use std::task::{Context, Poll};

use foundation_compact::Instant;
use foundation_connectrpc::ConnectError;
use foundation_context::Ctx;
use foundation_errstacks::ErrorTrace;
use futures::Stream;

use crate::shared::error::{SandboxError, SandboxResult, SlaveErrorCode};
use crate::shared::ids::UploadId;
use crate::shared::rpc::messages::FileChunk;
use crate::shared::sequence::{Sequence, SequenceCheck};

use super::slave_error;
use super::sys;

/// The number of bytes that one `FileRead` chunk holds at most.
const READ_CHUNK_BYTES: usize = 64 * 1024;

/// One upload that has not committed or aborted.
struct Upload {
    /// The directory of the target, and of the temporary file.
    directory: OwnedFd,
    /// The name of the temporary file in `directory`.
    temporary: CString,
    /// The name of the target file in `directory`.
    target: CString,
    /// The temporary file, opened for writing.
    file: OwnedFd,
    /// The offsets that the caller applied.
    sequence: Sequence,
    /// The time of the last call of this upload.
    last_use: Instant,
}

/// The uploads and the root of one prepared slave.
pub struct Files {
    root: Arc<OwnedFd>,
    workdir: String,
    uploads: Mutex<HashMap<UploadId, Upload>>,
    ctx: Ctx,
    chunk_bytes: usize,
    max_uploads: usize,
    idle_timeout: std::time::Duration,
    max_error_message: usize,
}

impl Files {
    /// Opens the root for the file calls.
    ///
    /// # Errors
    /// The operating system error of the root open.
    pub fn new(
        root: &Path,
        workdir: &str,
        ctx: Ctx,
        chunk_bytes: u32,
        max_uploads: u32,
        idle_timeout_ms: u32,
        max_error_message: u32,
    ) -> io::Result<Arc<Self>> {
        let path = CString::new(root.as_os_str().as_encoded_bytes())
            .map_err(|_| io::Error::from_raw_os_error(libc::EINVAL))?;
        let directory = sys::open_path(&path, libc::O_PATH | libc::O_DIRECTORY, 0)?;
        Ok(Arc::new(Self {
            root: Arc::new(directory),
            workdir: workdir.to_string(),
            uploads: Mutex::new(HashMap::new()),
            ctx,
            chunk_bytes: (chunk_bytes as usize).clamp(1, READ_CHUNK_BYTES),
            max_uploads: max_uploads as usize,
            idle_timeout: std::time::Duration::from_millis(u64::from(idle_timeout_ms)),
            max_error_message: max_error_message as usize,
        }))
    }

    fn lock(&self) -> std::sync::MutexGuard<'_, HashMap<UploadId, Upload>> {
        self.uploads
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
    }

    /// Starts one upload.
    ///
    /// # Errors
    /// A `BadRequest` for a path that leaves the root, `NotFound` when the
    /// parent directory is missing, `PermissionDenied`, `TooManySessions` when
    /// the upload count is at its limit, and `Io`.
    pub fn begin(&self, path: &str, mode: u32) -> SandboxResult<UploadId> {
        let resolved = crate::shared::spec::resolve_guest_path(&self.workdir, path)
            .ok_or_else(|| bad_path(path))?;
        let (parent, name) = split_guest_path(&resolved)?;
        if self.lock().len() >= self.max_uploads {
            return Err(SandboxError::TooManySessions.into());
        }
        let upload_id = UploadId::generate(&self.ctx);
        let directory = self.open_parent(&parent)?;
        let temporary_name = format!("{name}.ewe-up-{upload_id}");
        let target = CString::new(name).map_err(|_| bad_path(path))?;
        let temporary = CString::new(temporary_name.as_str()).map_err(|_| bad_path(path))?;
        let file = sys::openat(
            directory.as_raw_fd(),
            &temporary,
            libc::O_CREAT | libc::O_EXCL | libc::O_WRONLY,
            mode,
        )
        .map_err(|error| file_error("create the temporary file", &error))?;
        let upload = Upload {
            directory,
            temporary,
            target,
            file,
            sequence: Sequence::new(),
            last_use: self.ctx.clock().now(),
        };
        self.lock().insert(upload_id.clone(), upload);
        Ok(upload_id)
    }

    /// Writes one chunk of one upload.
    ///
    /// # Errors
    /// `NotFound` for an unknown upload, `SequenceGap` for a gap or an overlap,
    /// and `Io`, which also removes the upload.
    pub fn chunk(&self, upload_id: &str, offset: u64, data: &[u8]) -> SandboxResult<u64> {
        let upload_id = Self::parse_upload_id(upload_id)?;
        let mut uploads = self.lock();
        let Some(upload) = uploads.get_mut(&upload_id) else {
            return Err(upload_not_found(&upload_id));
        };
        match upload.sequence.accept_span(offset, data.len() as u64) {
            SequenceCheck::Gap => return Err(SandboxError::SequenceGap.into()),
            SequenceCheck::Duplicate => {
                upload.last_use = self.ctx.clock().now();
                return Ok(upload.sequence.next());
            }
            SequenceCheck::Apply => {}
        }
        if let Err(error) = sys::write_all_at(upload.file.as_raw_fd(), data, offset) {
            if let Some(upload) = uploads.remove(&upload_id) {
                remove_temporary(&upload);
            }
            return Err(file_error("write the upload chunk", &error));
        }
        upload.last_use = self.ctx.clock().now();
        Ok(upload.sequence.next())
    }

    /// Commits one upload to its target.
    ///
    /// # Errors
    /// `NotFound` for an unknown upload, `BadRequest` when the size is not the
    /// next offset (the upload is removed), and `Io`.
    pub fn commit(&self, upload_id: &str, size: u64) -> SandboxResult<u64> {
        let upload_id = Self::parse_upload_id(upload_id)?;
        let mut uploads = self.lock();
        let Some(upload) = uploads.get_mut(&upload_id) else {
            return Err(upload_not_found(&upload_id));
        };
        let next = upload.sequence.next();
        if next != size {
            if let Some(upload) = uploads.remove(&upload_id) {
                remove_temporary(&upload);
            }
            return Err(slave_error(
                SlaveErrorCode::BadRequest,
                None,
                format!("commit size {size} is not the upload size {next}"),
            ));
        }
        if let Err(error) = sys::fsync(upload.file.as_raw_fd()) {
            if let Some(upload) = uploads.remove(&upload_id) {
                remove_temporary(&upload);
            }
            return Err(file_error("flush the upload", &error));
        }
        if let Err(error) = sys::renameat(
            upload.directory.as_raw_fd(),
            &upload.temporary,
            &upload.target,
        ) {
            if let Some(upload) = uploads.remove(&upload_id) {
                remove_temporary(&upload);
            }
            return Err(file_error("rename the upload", &error));
        }
        if let Some(upload) = uploads.remove(&upload_id) {
            drop(upload.file);
        }
        Ok(size)
    }

    /// Aborts one upload. An unknown identifier is accepted.
    ///
    /// # Errors
    /// An `Io` error when the temporary file cannot be removed.
    pub fn abort(&self, upload_id: &str) -> SandboxResult<()> {
        let Ok(upload_id) = upload_id.parse::<UploadId>() else {
            return Ok(());
        };
        let Some(upload) = self.lock().remove(&upload_id) else {
            return Ok(());
        };
        remove_temporary(&upload);
        Ok(())
    }

    /// Removes every upload that stayed idle for the idle time.
    ///
    /// # Errors
    /// An `Io` error when a temporary file cannot be removed.
    pub fn expire_idle(&self) -> SandboxResult<()> {
        let now = self.ctx.clock().now();
        let expired: Vec<UploadId> = {
            let uploads = self.lock();
            uploads
                .iter()
                .filter(|(_, upload)| {
                    now.saturating_duration_since(upload.last_use) >= self.idle_timeout
                })
                .map(|(id, _)| id.clone())
                .collect()
        };
        for id in expired {
            let Some(upload) = self.lock().remove(&id) else {
                continue;
            };
            remove_temporary(&upload);
        }
        Ok(())
    }

    /// Removes every upload, and every temporary file.
    pub fn remove_all(&self) {
        let uploads = std::mem::take(&mut *self.lock());
        for (_, upload) in uploads {
            remove_temporary(&upload);
        }
    }

    /// Opens one file for reading and returns its chunks.
    ///
    /// The name is `read_file`, not `read`: the source scan of
    /// `tests/slave/lifecycle.rs` rejects a method call with the `read` name in
    /// the slave, because a descriptor read must go through `sys.rs` and the
    /// reactor. The scan reads the raw text, so comments must avoid the pattern
    /// too.
    ///
    /// # Errors
    /// A `BadRequest` for a path that leaves the root or names a directory,
    /// `NotFound`, `PermissionDenied` and `Io`.
    pub fn read_file(&self, path: &str) -> SandboxResult<FileReadStream> {
        let resolved = crate::shared::spec::resolve_guest_path(&self.workdir, path)
            .ok_or_else(|| bad_path(path))?;
        let (_, _) = split_guest_path(&resolved)?;
        let path_c = CString::new(resolved.as_str()).map_err(|_| bad_path(path))?;
        let file = sys::open_in_root(self.root.as_raw_fd(), &path_c, libc::O_RDONLY, 0)
            .map_err(|error| read_error(path, &error))?;
        if sys::is_directory(file.as_raw_fd())
            .map_err(|error| file_error("inspect a file", &error))?
        {
            return Err(slave_error(
                SlaveErrorCode::BadRequest,
                Some(libc::EISDIR),
                format!("the path is a directory: {path}"),
            ));
        }
        Ok(FileReadStream {
            file,
            offset: 0,
            done: false,
            chunk_bytes: self.chunk_bytes,
            max_error_message: self.max_error_message,
        })
    }

    /// Opens the parent directory of a guest path.
    fn open_parent(&self, parent: &str) -> SandboxResult<OwnedFd> {
        let parent_c = CString::new(parent).map_err(|_| bad_path(parent))?;
        sys::open_in_root(
            self.root.as_raw_fd(),
            &parent_c,
            libc::O_PATH | libc::O_DIRECTORY,
            0,
        )
        .map_err(|error| match error.raw_os_error() {
            Some(libc::ENOENT) => slave_error(
                SlaveErrorCode::NotFound,
                Some(libc::ENOENT),
                format!("the parent directory does not exist: {parent}"),
            ),
            Some(libc::EACCES) => slave_error(
                SlaveErrorCode::PermissionDenied,
                Some(libc::EACCES),
                format!("the parent directory is not readable: {parent}"),
            ),
            _ => file_error("open the parent directory", &error),
        })
    }

    fn parse_upload_id(value: &str) -> SandboxResult<UploadId> {
        value.parse::<UploadId>().map_err(|_| {
            slave_error(
                SlaveErrorCode::NotFound,
                None,
                format!("unknown upload: {value}"),
            )
        })
    }
}

/// The chunks of one file, in order. The last chunk has `eof`.
pub struct FileReadStream {
    file: OwnedFd,
    offset: u64,
    done: bool,
    chunk_bytes: usize,
    max_error_message: usize,
}

impl Stream for FileReadStream {
    type Item = Result<FileChunk, ErrorTrace<ConnectError>>;

    fn poll_next(self: Pin<&mut Self>, _cx: &mut Context<'_>) -> Poll<Option<Self::Item>> {
        let this = self.get_mut();
        if this.done {
            return Poll::Ready(None);
        }
        // A regular file has no readiness: one call reads one chunk and
        // returns, so no task holds a thread.
        let mut buffer = vec![0_u8; this.chunk_bytes];
        match sys::pread(this.file.as_raw_fd(), &mut buffer, this.offset) {
            Ok(0) => {
                this.done = true;
                Poll::Ready(Some(Ok(FileChunk {
                    offset: this.offset,
                    data: Vec::new(),
                    eof: true,
                })))
            }
            Ok(bytes) => {
                buffer.truncate(bytes);
                let chunk = FileChunk {
                    offset: this.offset,
                    data: buffer,
                    eof: false,
                };
                this.offset += bytes as u64;
                Poll::Ready(Some(Ok(chunk)))
            }
            Err(error) => {
                this.done = true;
                let failure = file_error("read a file", &error);
                Poll::Ready(Some(Err(ErrorTrace::new(
                    failure.current_context().to_connect(this.max_error_message),
                ))))
            }
        }
    }
}

/// Splits a guest path into its parent and its last part.
fn split_guest_path(path: &str) -> SandboxResult<(String, String)> {
    let parsed = Path::new(path);
    if !parsed.is_absolute() {
        return Err(bad_path(path));
    }
    if path.is_empty() || path.as_bytes().contains(&0) {
        return Err(bad_path(path));
    }
    if parsed
        .components()
        .any(|part| matches!(part, Component::ParentDir))
    {
        return Err(bad_path(path));
    }
    let trimmed = path.trim_end_matches('/');
    let Some(index) = trimmed.rfind('/') else {
        return Err(bad_path(path));
    };
    let name = &trimmed[index + 1..];
    if name.is_empty() || name == "." || name == ".." {
        return Err(bad_path(path));
    }
    let parent = if index == 0 { "/" } else { &trimmed[..index] };
    Ok((parent.to_string(), name.to_string()))
}

/// The error of a path that is not absolute or has a `..` part.
fn bad_path(path: &str) -> ErrorTrace<SandboxError> {
    slave_error(
        SlaveErrorCode::BadRequest,
        None,
        format!("the path must be absolute and stay inside the root: {path}"),
    )
}

/// The error of an unknown upload.
fn upload_not_found(id: &UploadId) -> ErrorTrace<SandboxError> {
    slave_error(
        SlaveErrorCode::NotFound,
        None,
        format!("unknown upload: {id}"),
    )
}

/// The error of one file call.
fn file_error(operation: &str, error: &io::Error) -> ErrorTrace<SandboxError> {
    slave_error(
        SlaveErrorCode::Io,
        error.raw_os_error(),
        format!("{operation}: {error}"),
    )
}

/// The error of one file open.
fn read_error(path: &str, error: &io::Error) -> ErrorTrace<SandboxError> {
    let code = match error.raw_os_error() {
        Some(libc::ENOENT) => SlaveErrorCode::NotFound,
        Some(libc::EACCES) => SlaveErrorCode::PermissionDenied,
        _ => SlaveErrorCode::Io,
    };
    slave_error(code, error.raw_os_error(), format!("open {path}: {error}"))
}

/// Removes the temporary file of one upload.
fn remove_temporary(upload: &Upload) {
    let _ = sys::unlinkat(upload.directory.as_raw_fd(), &upload.temporary);
}
