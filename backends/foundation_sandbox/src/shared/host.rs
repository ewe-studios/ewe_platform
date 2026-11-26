//! The side-effect bundles of the native roles.
//!
//! Only a boundary builds a bundle and puts it in the
//! [`Ctx`](foundation_context::Ctx). Library code reads it with
//! [`SandboxHost::from_ctx`] or [`SlaveHost::from_ctx`] and never falls back to
//! the real system.

use std::path::{Path, PathBuf};
use std::sync::Arc;

use foundation_connectrpc::shared::transport::Transport;
use foundation_context::Ctx;
#[cfg(unix)]
use serde::{Deserialize, Serialize};

use crate::shared::driver::SlaveEndpoint;
use crate::shared::error::{SandboxError, SandboxResult};
use crate::shared::ids::DriverId;
use crate::shared::limits::{ArtifactLimits, SandboxLimits};
#[cfg(unix)]
use crate::shared::BoxFuture;

/// The side effects of the native control plane.
#[derive(Clone)]
pub struct SandboxHost {
    /// Starts host processes.
    #[cfg(unix)]
    pub processes: Arc<dyn ProcessSpawner>,
    /// Reads and writes host files.
    pub fs: Arc<dyn HostFs>,
    /// Turns a slave endpoint into an RPC transport.
    pub connector: Arc<dyn SlaveConnector>,
    /// The host paths and driver order.
    pub config: Arc<HostConfig>,
}

impl SandboxHost {
    /// Reads the bundle from `ctx`.
    ///
    /// # Errors
    /// [`SandboxError::HostNotConfigured`] when the context holds no bundle.
    pub fn from_ctx(ctx: &Ctx) -> SandboxResult<Self> {
        ctx.get::<Self>()
            .map(|host| (*host).clone())
            .ok_or_else(|| SandboxError::HostNotConfigured.into())
    }
}

/// Turns a slave endpoint into an RPC transport.
pub trait SlaveConnector: Send + Sync + 'static {
    /// Connects to `endpoint` with the limits of the sandbox.
    ///
    /// # Errors
    /// [`crate::shared::error::SandboxError::Io`] or
    /// [`crate::shared::error::SandboxError::Rpc`] when the transport fails.
    fn connect(
        &self,
        ctx: &Ctx,
        endpoint: &SlaveEndpoint,
        limits: &SandboxLimits,
    ) -> SandboxResult<Arc<dyn Transport>>;
}

/// The host paths and the driver order of the native control plane.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct HostConfig {
    /// The directory for run files. Keep it short: a socket path has a limit.
    pub state_dir: PathBuf,
    /// The directory for downloaded and built files.
    pub cache_dir: PathBuf,
    /// The path of `agent-slave` for the guest architecture.
    pub slave_path: Option<PathBuf>,
    /// The path of the Firecracker program.
    pub firecracker_path: Option<PathBuf>,
    /// The path of the libkrun runner program.
    pub krun_runner_path: Option<PathBuf>,
    /// The path of the libkrun library.
    pub libkrun_path: Option<PathBuf>,
    /// The path of `mkfs.ext4` (feature 12). `None` searches the host.
    pub mkfs_ext4_path: Option<PathBuf>,
    /// The driver order. The default is Firecracker, libkrun and Docker.
    pub driver_order: Vec<DriverId>,
    /// The limits of the guest-artifact store (feature 03).
    pub artifact_limits: ArtifactLimits,
    /// The limits of the sandbox image store (feature 12).
    pub image_limits: crate::shared::limits::ImageLimits,
}

impl Default for HostConfig {
    fn default() -> Self {
        Self {
            state_dir: PathBuf::new(),
            cache_dir: PathBuf::new(),
            slave_path: None,
            firecracker_path: None,
            krun_runner_path: None,
            libkrun_path: None,
            mkfs_ext4_path: None,
            driver_order: vec![DriverId::Firecracker, DriverId::Libkrun, DriverId::Docker],
            artifact_limits: ArtifactLimits::default(),
            image_limits: crate::shared::limits::ImageLimits::default(),
        }
    }
}

/// Starts host processes.
#[cfg(unix)]
pub trait ProcessSpawner: Send + Sync + 'static {
    /// Starts one process.
    ///
    /// # Errors
    /// [`SandboxError::Io`] when the program cannot start.
    fn spawn(&self, spec: ProcessSpec) -> SandboxResult<Box<dyn ChildProcess>>;

    /// Whether a process with `pid` exists.
    ///
    /// A stale run directory of a control plane that crashed names a process
    /// that does not exist. A host that cannot tell answers `true`, so a sweep
    /// never removes a directory of a live process.
    fn is_running(&self, pid: u32) -> bool;
}

/// One host process to start.
#[cfg(unix)]
pub struct ProcessSpec {
    /// The program path.
    pub program: PathBuf,
    /// The arguments after the program.
    pub args: Vec<String>,
    /// The full environment. Nothing is inherited.
    pub env: Vec<(String, String)>,
    /// The working directory.
    pub cwd: PathBuf,
    /// The standard output file, opened for append.
    pub stdout: PathBuf,
    /// The standard error file, opened for append.
    pub stderr: PathBuf,
    /// Whether the child dies when this process dies.
    pub kill_with_parent: bool,
    /// File descriptors that the child inherits, as
    /// `(number in the child, descriptor)`.
    ///
    /// It is empty except for the libkrun lifeline on macOS (feature 09).
    pub extra_fds: Vec<(i32, std::os::fd::OwnedFd)>,
}

/// One started host process.
#[cfg(unix)]
pub trait ChildProcess: Send + Sync + 'static {
    /// The process identifier.
    fn pid(&self) -> u32;

    /// Sends one signal to the process.
    ///
    /// # Errors
    /// [`SandboxError::Io`] when the signal cannot be sent.
    fn signal(&self, signal: i32) -> SandboxResult<()>;

    /// Waits for the process. It can be awaited more than once.
    fn wait(&self) -> BoxFuture<'_, SandboxResult<ProcessExit>>;

    /// Returns the exit state without waiting, or `None` while the process
    /// runs.
    ///
    /// # Errors
    /// [`SandboxError::Io`] when the state cannot be read.
    fn try_wait(&self) -> SandboxResult<Option<ProcessExit>>;
}

/// How one host process ended.
#[cfg(unix)]
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct ProcessExit {
    /// The exit code, when the process exited with one.
    pub code: Option<i32>,
    /// The signal, when a signal ended the process.
    pub signal: Option<i32>,
}

/// An exclusive advisory lock on one file.
///
/// The lock lives until this value drops. The native host holds the locked file
/// descriptor; a test fake holds its own token. The wrapped resource is opaque,
/// so the trait stays free of platform types.
pub struct FileLock {
    #[allow(
        dead_code,
        reason = "the value holds the lock open and releases it when it drops"
    )]
    resource: Arc<dyn std::any::Any + Send + Sync>,
}

impl FileLock {
    /// A lock over the resource that releases it when the value drops.
    #[must_use]
    pub fn new<R>(resource: R) -> Self
    where
        R: std::any::Any + Send + Sync,
    {
        Self {
            resource: Arc::new(resource),
        }
    }
}

impl std::fmt::Debug for FileLock {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter.write_str("FileLock(..)")
    }
}

/// Reads and writes host files.
pub trait HostFs: Send + Sync + 'static {
    /// Creates a directory and its parents.
    ///
    /// # Errors
    /// [`SandboxError::Io`] when the directory cannot be created.
    fn create_dir_all(&self, path: &Path, mode: u32) -> SandboxResult<()>;

    /// Removes a directory and its contents.
    ///
    /// # Errors
    /// [`SandboxError::Io`] when the directory cannot be removed.
    fn remove_dir_all(&self, path: &Path) -> SandboxResult<()>;

    /// Removes one file. A missing file is not an error.
    ///
    /// # Errors
    /// [`SandboxError::Io`] when the file cannot be removed.
    fn remove_file(&self, path: &Path) -> SandboxResult<()>;

    /// Writes a file and replaces its contents.
    ///
    /// # Errors
    /// [`SandboxError::Io`] when the file cannot be written.
    fn write(&self, path: &Path, data: &[u8]) -> SandboxResult<()>;

    /// Reads at most `max` bytes of a file.
    ///
    /// # Errors
    /// [`SandboxError::Io`] when the file cannot be read.
    fn read(&self, path: &Path, max: usize) -> SandboxResult<Vec<u8>>;

    /// Opens a file for a streamed read.
    ///
    /// The caller reads it in bounded chunks. The driver uses it to send the
    /// slave into a container without holding the whole file (feature 10).
    ///
    /// # Errors
    /// [`SandboxError::Io`] when the file cannot be opened.
    fn open_read(&self, path: &Path) -> SandboxResult<Box<dyn std::io::Read + Send + Sync>>;

    /// The size of a file in bytes.
    ///
    /// # Errors
    /// [`SandboxError::Io`] when the size cannot be read.
    fn file_size(&self, path: &Path) -> SandboxResult<u64>;

    /// Reads at most `max` bytes from the end of a file.
    ///
    /// # Errors
    /// [`SandboxError::Io`] when the file cannot be read.
    fn tail(&self, path: &Path, max: usize) -> SandboxResult<Vec<u8>>;

    /// Whether a file or directory exists.
    fn exists(&self, path: &Path) -> bool;

    /// Renames a file or a directory. It replaces a file, and it fails when the
    /// target is a non-empty directory.
    ///
    /// # Errors
    /// [`SandboxError::Io`] when the rename fails.
    fn rename(&self, from: &Path, to: &Path) -> SandboxResult<()>;

    /// Creates a file for a streamed write. It replaces the file contents.
    ///
    /// The caller writes the body in bounded chunks. The returned writer does
    /// not hold the whole body.
    ///
    /// # Errors
    /// [`SandboxError::Io`] when the file cannot be created.
    fn create_file(&self, path: &Path) -> SandboxResult<Box<dyn std::io::Write + Send>>;

    /// Removes one file. A missing file is not an error.
    ///
    /// # Errors
    /// [`SandboxError::Io`] when the file cannot be removed.
    fn remove_file(&self, path: &Path) -> SandboxResult<()>;

    /// Opens one file for a streamed read.
    ///
    /// The caller reads the body through this handle, so it never holds the
    /// whole file.
    ///
    /// # Errors
    /// [`SandboxError::Io`] when the file cannot be opened.
    fn read_stream(&self, path: &Path) -> SandboxResult<Box<dyn std::io::Read + Send>>;

    /// Copies one file, keeping its holes when the file system supports it.
    ///
    /// It tries a reflink, then a sparse copy, then a plain copy.
    ///
    /// # Errors
    /// [`SandboxError::Io`] when no copy works.
    fn clone_file(&self, from: &Path, to: &Path) -> SandboxResult<()>;

    /// Takes an exclusive advisory lock on one file.
    ///
    /// A second lock of the same file by another process fails; a second lock
    /// by this process also fails.
    ///
    /// # Errors
    /// [`SandboxError::Io`] when the file cannot be opened or the lock is held.
    fn lock_exclusive(&self, path: &Path) -> SandboxResult<FileLock>;

    /// The modification time of a path, when the host can read it.
    ///
    /// The default gives `None`: a host that cannot tell never reports a stale
    /// directory. The sweep of stale run directories uses it.
    fn modified(&self, _path: &Path) -> Option<foundation_compact::SystemTime> {
        None
    }

    /// The direct children of a directory.
    ///
    /// The default gives an empty list: a host that cannot list has nothing to
    /// sweep. The sweep of stale run directories uses it.
    ///
    /// # Errors
    /// [`SandboxError::Io`] when the directory cannot be read.
    fn read_dir(&self, _path: &Path) -> SandboxResult<Vec<PathBuf>> {
        Ok(Vec::new())
    }

    /// Whether the path can be opened for reading and writing.
    ///
    /// The default reports [`Self::exists`]. A real host opens the file, so the
    /// `/dev/kvm` check sees a permission problem. A test fake answers with
    /// its own rule.
    fn is_openable_read_write(&self, path: &Path) -> bool {
        self.exists(path)
    }
}

/// The guest side effects of the slave.
///
/// The real one is `SlaveHost::native()`, which the slave entry point builds.
/// A test gives a fake only for the parts that need root rights: the mounts and
/// the power off. Process start, pipes and signals are not behind a trait: they
/// are the slave's own job, they need no root right, and the tests use the real
/// ones.
#[derive(Clone)]
pub struct SlaveHost {
    /// Mounts, directories and the files that a mount needs.
    pub mounts: Arc<dyn GuestMounts>,
    /// The sync and the power off of the machine.
    pub power: Arc<dyn GuestPower>,
}

impl SlaveHost {
    /// Reads the bundle from `ctx`.
    ///
    /// # Errors
    /// [`SandboxError::HostNotConfigured`] when the context holds no bundle.
    pub fn from_ctx(ctx: &Ctx) -> SandboxResult<Self> {
        ctx.get::<Self>()
            .map(|host| (*host).clone())
            .ok_or_else(|| SandboxError::HostNotConfigured.into())
    }
}

/// The mounts of the guest.
pub trait GuestMounts: Send + Sync + 'static {
    /// Mounts `source` on `target` with `fstype`, `flags` and `data`.
    ///
    /// # Errors
    /// The operating system error of the mount call.
    fn mount(
        &self,
        source: &str,
        target: &Path,
        fstype: &str,
        flags: u64,
        data: &str,
    ) -> std::io::Result<()>;

    /// Makes `path` and its parents, with `mode`.
    ///
    /// # Errors
    /// The operating system error of the directory calls.
    fn mkdir_p(&self, path: &Path, mode: u32) -> std::io::Result<()>;

    /// Writes `data` to `path`, and replaces its contents.
    ///
    /// # Errors
    /// The operating system error of the file calls.
    fn write_file(&self, path: &Path, data: &[u8]) -> std::io::Result<()>;
}

/// The power state of the guest.
pub trait GuestPower: Send + Sync + 'static {
    /// Flushes the file system buffers.
    fn sync(&self);

    /// Powers the machine off. A successful call never returns: the guest stops
    /// before the result can leave.
    ///
    /// # Errors
    /// The operating system error of the reboot call.
    fn power_off(&self) -> std::io::Result<std::convert::Infallible>;
}
