//! The image unpack calls of the slave (feature 12, section 6.6).
//!
//! `UnpackBegin` starts a child process of this same program with the mode
//! `--unpack-child`. The child `chroot`s to the target, reads the tar bytes
//! from its standard input and applies them. The parent writes the tar chunks
//! to the child's standard input, so the parent never holds the archive.
//!
//! The child writes the line `ready` after the `chroot`, then one JSON result
//! line. A `chroot` failure is an error at `UnpackBegin`; an entry failure is
//! an error at `UnpackCommit` with the entry path and the operating system
//! error number.

use std::collections::HashMap;
use std::io::{BufRead, BufReader, Read, Write};
use std::path::{Component, Path, PathBuf};
use std::process::{Child, ChildStdin, ChildStdout, Command, Stdio};
use std::sync::Mutex;

use foundation_compact::Instant;
use foundation_context::Ctx;
use serde::{Deserialize, Serialize};

use crate::shared::error::{SandboxError, SandboxResult, SlaveErrorCode};
use crate::shared::ids::UploadId;
use crate::shared::sequence::{Sequence, SequenceCheck};
use crate::shared::unpack::{UnpackOptions, UNPACK_LIMIT_MESSAGE};

use super::{slave_error, sys};

/// The first argument of the unpack child process.
pub const UNPACK_CHILD_FLAG: &str = "--unpack-child";

/// The line that the child writes after its `chroot` succeeds.
const READY_LINE: &str = "ready";

/// The options that the parent gives the child.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct ChildOptions {
    /// Whether OCI whiteouts apply.
    pub oci_whiteouts: bool,
    /// Exact relative paths that the reader drops.
    pub skip: Vec<String>,
    /// The most entries.
    pub max_entries: usize,
    /// The most data bytes.
    pub max_bytes: u64,
    /// The longest path in bytes.
    pub max_path_bytes: usize,
}

/// The result that the child writes as one JSON line.
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "snake_case", tag = "status")]
pub enum ChildOutcome {
    /// The unpack applied every entry.
    Ok {
        /// The applied entries.
        entries: u64,
        /// The data bytes.
        bytes: u64,
    },
    /// The `chroot` failed, so the target is not safe.
    PermissionDenied {
        /// The reason.
        message: String,
    },
    /// One entry failed.
    Entry {
        /// The entry path.
        path: String,
        /// The operating system error number.
        errno: Option<i32>,
        /// The reason.
        message: String,
    },
    /// The unpack passed a limit.
    Limit {
        /// The reason, which starts with [`UNPACK_LIMIT_MESSAGE`].
        message: String,
    },
    /// The tar bytes are malformed.
    BadRequest {
        /// The entry path, when the reader knows it.
        path: String,
        /// The reason.
        message: String,
    },
}

/// One running unpack.
struct Session {
    child: Child,
    stdin: Option<ChildStdin>,
    stdout: BufReader<ChildStdout>,
    sequence: Sequence,
    last_use: Instant,
}

/// The unpack sessions of one prepared slave.
pub struct UnpackSessions {
    sessions: Mutex<HashMap<UploadId, Session>>,
    ctx: Ctx,
    max_sessions: usize,
    idle_timeout: std::time::Duration,
}

impl UnpackSessions {
    /// An empty set of sessions.
    #[must_use]
    pub fn new(ctx: Ctx, max_sessions: usize, idle_timeout: std::time::Duration) -> Self {
        Self {
            sessions: Mutex::new(HashMap::new()),
            ctx,
            max_sessions,
            idle_timeout,
        }
    }

    fn lock(&self) -> std::sync::MutexGuard<'_, HashMap<UploadId, Session>> {
        self.sessions
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
    }

    /// Starts one unpack of `dest`, which is a directory of the guest.
    ///
    /// # Errors
    /// `BadRequest` for a bad target, `TooManySessions` at the session limit,
    /// `PermissionDenied` when the child cannot `chroot`, and `Io`.
    pub fn begin(
        &self,
        exec_root: &Path,
        dest: &str,
        limits: &ChildOptions,
    ) -> SandboxResult<UploadId> {
        let target = dest_path(exec_root, dest)?;
        if self.lock().len() >= self.max_sessions {
            return Err(SandboxError::TooManySessions.into());
        }
        let program = std::env::current_exe().map_err(|error| SandboxError::Io {
            op: "find the slave program".to_string(),
            message: error.to_string(),
        })?;
        let options_json = serde_json::to_string(limits).map_err(|error| SandboxError::Io {
            op: "encode the unpack options".to_string(),
            message: error.to_string(),
        })?;
        let mut child = Command::new(program)
            .arg(UNPACK_CHILD_FLAG)
            .arg(&target)
            .arg(&options_json)
            .env_clear()
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::null())
            .spawn()
            .map_err(|error| SandboxError::Io {
                op: "start the unpack child".to_string(),
                message: error.to_string(),
            })?;
        let stdin = child.stdin.take();
        let stdout = child.stdout.take().ok_or_else(|| SandboxError::Io {
            op: "read the unpack child".to_string(),
            message: "the child has no standard output".to_string(),
        })?;
        let mut stdout = BufReader::new(stdout);
        let mut line = String::new();
        let ready = stdout.read_line(&mut line).map(|_| line.trim().to_string());
        match ready {
            Ok(text) if text == READY_LINE => {}
            Ok(text) => {
                let _ = child.kill();
                let _ = child.wait();
                return Err(slave_error(
                    SlaveErrorCode::PermissionDenied,
                    Some(libc::EPERM),
                    format!("the unpack child cannot enter the target {dest}: {text}"),
                ));
            }
            Err(error) => {
                let _ = child.kill();
                let _ = child.wait();
                return Err(SandboxError::Io {
                    op: "read the unpack child".to_string(),
                    message: error.to_string(),
                }
                .into());
            }
        }
        let upload_id = UploadId::generate(&self.ctx);
        self.lock().insert(
            upload_id.clone(),
            Session {
                child,
                stdin,
                stdout,
                sequence: Sequence::new(),
                last_use: self.ctx.clock().now(),
            },
        );
        Ok(upload_id)
    }

    /// Writes one tar chunk.
    ///
    /// # Errors
    /// `NotFound` for an unknown session, `SequenceGap` for a gap or an
    /// overlap, and `Io`.
    pub fn chunk(&self, upload_id: &str, offset: u64, data: &[u8]) -> SandboxResult<u64> {
        let upload_id = parse_upload_id(upload_id)?;
        let mut sessions = self.lock();
        let Some(session) = sessions.get_mut(&upload_id) else {
            return Err(upload_not_found(&upload_id));
        };
        match session.sequence.accept_span(offset, data.len() as u64) {
            SequenceCheck::Gap => return Err(SandboxError::SequenceGap.into()),
            SequenceCheck::Duplicate => {
                session.last_use = self.ctx.clock().now();
                return Ok(session.sequence.next());
            }
            SequenceCheck::Apply => {}
        }
        let written = match session.stdin.as_mut() {
            Some(stdin) => stdin.write_all(data),
            None => Err(std::io::Error::from(std::io::ErrorKind::BrokenPipe)),
        };
        if let Err(error) = written {
            if let Some(mut session) = sessions.remove(&upload_id) {
                let _ = session.child.kill();
                let _ = session.child.wait();
            }
            return Err(SandboxError::Io {
                op: "write the unpack chunk".to_string(),
                message: error.to_string(),
            }
            .into());
        }
        session.last_use = self.ctx.clock().now();
        Ok(session.sequence.next())
    }

    /// Ends the tar stream and reads the child result.
    ///
    /// # Errors
    /// `NotFound`, `BadRequest` for a size that is not the next offset, and the
    /// child result as `Io` with the entry path and `errno`.
    pub fn commit(&self, upload_id: &str, size: u64) -> SandboxResult<(u64, u64)> {
        let upload_id = parse_upload_id(upload_id)?;
        let Some(mut session) = self.lock().remove(&upload_id) else {
            return Err(upload_not_found(&upload_id));
        };
        let next = session.sequence.next();
        if size != next {
            let _ = session.child.kill();
            let _ = session.child.wait();
            return Err(slave_error(
                SlaveErrorCode::BadRequest,
                None,
                format!("the unpack size {size} is not the stream size {next}"),
            ));
        }
        drop(session.stdin.take());
        let mut output = String::new();
        let read = session.stdout.read_to_string(&mut output);
        let _status = session.child.wait().map_err(|error| SandboxError::Io {
            op: "wait for the unpack child".to_string(),
            message: error.to_string(),
        })?;
        if let Err(error) = read {
            return Err(SandboxError::Io {
                op: "read the unpack result".to_string(),
                message: error.to_string(),
            }
            .into());
        }
        let outcome = serde_json::from_str::<ChildOutcome>(output.trim()).map_err(|error| {
            SandboxError::Io {
                op: "parse the unpack result".to_string(),
                message: format!("{}: {error}", output.trim()),
            }
        })?;
        match outcome {
            ChildOutcome::Ok { entries, bytes } => Ok((entries, bytes)),
            ChildOutcome::Limit { message } => Err(slave_error(
                SlaveErrorCode::Io,
                None,
                // The control side maps this text to `ArtifactTooLarge`.
                format!("{UNPACK_LIMIT_MESSAGE}: {message}"),
            )),
            ChildOutcome::PermissionDenied { message } => Err(slave_error(
                SlaveErrorCode::PermissionDenied,
                Some(libc::EPERM),
                message,
            )),
            ChildOutcome::Entry {
                path,
                errno,
                message,
            } => Err(slave_error(
                SlaveErrorCode::Io,
                errno,
                format!("the unpack of {path} failed: {message}"),
            )),
            ChildOutcome::BadRequest { path, message } => Err(slave_error(
                SlaveErrorCode::BadRequest,
                None,
                format!("the tar entry {path} is not supported: {message}"),
            )),
        }
    }

    /// Aborts one session. An unknown identifier is accepted.
    pub fn abort(&self, upload_id: &str) {
        let Ok(upload_id) = upload_id.parse::<UploadId>() else {
            return;
        };
        let Some(mut session) = self.lock().remove(&upload_id) else {
            return;
        };
        let _ = session.child.kill();
        let _ = session.child.wait();
    }

    /// Aborts every session that stayed idle for the idle time.
    pub fn expire_idle(&self) {
        let now = self.ctx.clock().now();
        let expired: Vec<UploadId> = {
            let sessions = self.lock();
            sessions
                .iter()
                .filter(|(_, session)| {
                    now.saturating_duration_since(session.last_use) >= self.idle_timeout
                })
                .map(|(id, _)| id.clone())
                .collect()
        };
        for id in expired {
            if let Some(mut session) = self.lock().remove(&id) {
                let _ = session.child.kill();
                let _ = session.child.wait();
            }
        }
    }

    /// Aborts every session.
    pub fn remove_all(&self) {
        let sessions = std::mem::take(&mut *self.lock());
        for (_, mut session) in sessions {
            let _ = session.child.kill();
            let _ = session.child.wait();
        }
    }
}

/// The target path of one unpack, checked to stay inside `exec_root`.
fn dest_path(exec_root: &Path, dest: &str) -> SandboxResult<PathBuf> {
    let parsed = Path::new(dest);
    if !parsed.is_absolute() || dest.as_bytes().contains(&0) {
        return Err(slave_error(
            SlaveErrorCode::BadRequest,
            None,
            format!("the unpack target must be an absolute path: {dest}"),
        ));
    }
    if parsed
        .components()
        .any(|part| matches!(part, Component::ParentDir))
    {
        return Err(slave_error(
            SlaveErrorCode::BadRequest,
            None,
            format!("the unpack target must not contain `..`: {dest}"),
        ));
    }
    let path = exec_root.join(dest.trim_start_matches('/'));
    if !path.is_dir() {
        return Err(slave_error(
            SlaveErrorCode::BadRequest,
            Some(libc::ENOTDIR),
            format!("the unpack target is not a directory: {dest}"),
        ));
    }
    Ok(path)
}

fn parse_upload_id(value: &str) -> SandboxResult<UploadId> {
    value.parse::<UploadId>().map_err(|_| {
        slave_error(
            SlaveErrorCode::NotFound,
            None,
            format!("unknown unpack: {value}"),
        )
    })
}

fn upload_not_found(id: &UploadId) -> foundation_errstacks::ErrorTrace<SandboxError> {
    slave_error(
        SlaveErrorCode::NotFound,
        None,
        format!("unknown unpack: {id}"),
    )
}

/// Runs the child mode of one unpack. It never returns to the caller.
///
/// It `chroot`s to `dest`, writes `ready` to standard output, reads the tar
/// bytes from standard input, applies them to `/`, and writes one JSON result
/// line. It returns the process exit code.
#[must_use]
pub fn run_unpack_child(dest: &str, options_json: &str) -> i32 {
    let options = match serde_json::from_str::<ChildOptions>(options_json) {
        Ok(options) => options,
        Err(error) => {
            write_outcome(&ChildOutcome::BadRequest {
                path: String::new(),
                message: format!("bad options: {error}"),
            });
            return 1;
        }
    };
    if let Err(error) = sys::chroot(dest) {
        write_outcome(&ChildOutcome::PermissionDenied {
            message: format!("cannot chroot to {dest}: {error}"),
        });
        return 1;
    }
    if let Err(error) = sys::chdir(Path::new("/")) {
        write_outcome(&ChildOutcome::PermissionDenied {
            message: format!("cannot change to the target root: {error}"),
        });
        return 1;
    }
    // The parent waits for this line before it sends a tar byte.
    let mut stdout = std::io::stdout();
    if writeln!(stdout, "{READY_LINE}").is_err() || stdout.flush().is_err() {
        return 1;
    }
    let limits = crate::native::unpack::UnpackLimits {
        max_entries: options.max_entries,
        max_bytes: options.max_bytes,
        max_path_bytes: options.max_path_bytes,
    };
    let options = UnpackOptions {
        oci_whiteouts: options.oci_whiteouts,
        skip: options.skip,
    };
    let stdin = std::io::stdin();
    let outcome =
        match crate::native::unpack::unpack(stdin.lock(), Path::new("/"), &options, &limits) {
            Ok(stats) => ChildOutcome::Ok {
                entries: stats.entries,
                bytes: stats.bytes,
            },
            Err(crate::native::unpack::UnpackError::BadRequest { path, message }) => {
                ChildOutcome::BadRequest { path, message }
            }
            Err(crate::native::unpack::UnpackError::Entry {
                path,
                errno,
                message,
            }) => ChildOutcome::Entry {
                path,
                errno,
                message,
            },
            Err(crate::native::unpack::UnpackError::Limit { message }) => {
                ChildOutcome::Limit { message }
            }
        };
    let failed = !matches!(outcome, ChildOutcome::Ok { .. });
    write_outcome(&outcome);
    i32::from(failed)
}

/// Writes one JSON outcome line to standard output.
fn write_outcome(outcome: &ChildOutcome) {
    if let Ok(line) = serde_json::to_string(outcome) {
        let mut stdout = std::io::stdout();
        let _ = writeln!(stdout, "{line}");
        let _ = stdout.flush();
    }
}
