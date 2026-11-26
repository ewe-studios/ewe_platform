//! The real side effects of the native control plane.
//!
//! This module holds the only code of the control side that starts a host
//! process, dials a socket or touches the real file system (invariant I12 of
//! feature 05). The library reads the [`SandboxHost`] bundle from the
//! [`Ctx`](foundation_context::Ctx); only a boundary builds it with
//! [`SandboxHost::native`].
//!
//! - [`NativeFs`] is the real [`HostFs`].
//! - [`NativeProcessSpawner`] starts one host process on a dedicated thread, so
//!   `PR_SET_PDEATHSIG` names a thread that lives as long as the process.
//! - [`NativeConnector`] turns a slave endpoint into a Connect RPC transport.
//! - [`HostConfig::discover`] reads the host paths and the driver order from the
//!   environment.
//!
//! The module is native only. A Worker has no host file system.

use std::fs::{File, OpenOptions};
use std::io::{Read, Seek, SeekFrom, Write};
#[cfg(unix)]
use std::os::unix::process::CommandExt as _;
use std::path::{Path, PathBuf};
use std::sync::Arc;
#[cfg(unix)]
use std::sync::{mpsc, Mutex, PoisonError};
#[cfg(unix)]
use std::task::Poll;

#[cfg(any(
    feature = "artifacts",
    feature = "firecracker",
    feature = "slave",
    feature = "testing"
))]
use foundation_connectrpc::shared::transport::Transport;
#[cfg(any(
    feature = "artifacts",
    feature = "firecracker",
    feature = "slave",
    feature = "testing"
))]
use foundation_connectrpc::H1Transport;
use foundation_context::Ctx;
#[cfg(unix)]
use foundation_core::valtron::{EventReadiness, ReadinessWaker, ResumeSignal};
#[cfg(any(
    feature = "artifacts",
    feature = "firecracker",
    feature = "slave",
    feature = "testing"
))]
use foundation_netio::HttpClientBuilder;

use crate::shared::driver::SlaveEndpoint;
use crate::shared::error::{SandboxError, SandboxResult};
use crate::shared::host::{
    ChildProcess, HostConfig, HostFs, ProcessExit, ProcessSpawner, ProcessSpec, SandboxHost,
    SlaveConnector,
};
use crate::shared::ids::DriverId;
use crate::shared::limits::SandboxLimits;
use crate::shared::spec::GuestArch;
use crate::shared::BoxFuture;

/// The host file system behind [`HostFs`].
///
/// The value holds no state, so a clone is free.
#[derive(Clone, Copy, Debug, Default)]
pub struct NativeFs;

impl NativeFs {
    /// The real host file system.
    #[must_use]
    pub fn new() -> Self {
        Self
    }
}

/// A host input or output error with its operation and path.
fn io_error(op: &str, path: &Path, error: &std::io::Error) -> SandboxError {
    SandboxError::Io {
        op: op.to_string(),
        message: format!("{}: {error}", path.display()),
    }
}

impl HostFs for NativeFs {
    fn create_dir_all(&self, path: &Path, mode: u32) -> SandboxResult<()> {
        let mut builder = std::fs::DirBuilder::new();
        builder.recursive(true);
        #[cfg(unix)]
        {
            use std::os::unix::fs::DirBuilderExt;
            builder.mode(mode & 0o7777);
        }
        #[cfg(not(unix))]
        let _ = mode;
        builder
            .create(path)
            .map_err(|error| io_error("create_dir_all", path, &error).into())
    }

    fn remove_dir_all(&self, path: &Path) -> SandboxResult<()> {
        match std::fs::remove_dir_all(path) {
            Ok(()) => Ok(()),
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(()),
            Err(error) => Err(io_error("remove_dir_all", path, &error).into()),
        }
    }

    fn remove_file(&self, path: &Path) -> SandboxResult<()> {
        match std::fs::remove_file(path) {
            Ok(()) => Ok(()),
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(()),
            Err(error) => Err(io_error("remove_file", path, &error).into()),
        }
    }

    fn write(&self, path: &Path, data: &[u8]) -> SandboxResult<()> {
        std::fs::write(path, data).map_err(|error| io_error("write", path, &error).into())
    }

    fn read(&self, path: &Path, max: usize) -> SandboxResult<Vec<u8>> {
        let file = File::open(path).map_err(|error| io_error("read", path, &error))?;
        let mut bytes = Vec::new();
        file.take(max as u64)
            .read_to_end(&mut bytes)
            .map_err(|error| io_error("read", path, &error))?;
        Ok(bytes)
    }

    fn open_read(&self, path: &Path) -> SandboxResult<Box<dyn Read + Send + Sync>> {
        let file = File::open(path).map_err(|error| io_error("open_read", path, &error))?;
        Ok(Box::new(file))
    }

    fn file_size(&self, path: &Path) -> SandboxResult<u64> {
        std::fs::metadata(path)
            .map(|metadata| metadata.len())
            .map_err(|error| io_error("file_size", path, &error).into())
    }

    fn tail(&self, path: &Path, max: usize) -> SandboxResult<Vec<u8>> {
        let mut file = File::open(path).map_err(|error| io_error("tail", path, &error))?;
        let length = file
            .seek(SeekFrom::End(0))
            .map_err(|error| io_error("tail", path, &error))?;
        let start = length.saturating_sub(max as u64);
        file.seek(SeekFrom::Start(start))
            .map_err(|error| io_error("tail", path, &error))?;
        let mut bytes = Vec::new();
        file.take(max as u64)
            .read_to_end(&mut bytes)
            .map_err(|error| io_error("tail", path, &error))?;
        Ok(bytes)
    }

    fn exists(&self, path: &Path) -> bool {
        path.exists()
    }

    fn rename(&self, from: &Path, to: &Path) -> SandboxResult<()> {
        std::fs::rename(from, to).map_err(|error| {
            SandboxError::Io {
                op: "rename".to_string(),
                message: format!("{} to {}: {error}", from.display(), to.display()),
            }
            .into()
        })
    }

    fn create_file(&self, path: &Path) -> SandboxResult<Box<dyn Write + Send>> {
        let file = OpenOptions::new()
            .write(true)
            .create(true)
            .truncate(true)
            .open(path)
            .map_err(|error| io_error("create_file", path, &error))?;
        Ok(Box::new(file))
    }

    fn modified(&self, path: &Path) -> Option<foundation_compact::SystemTime> {
        std::fs::metadata(path)
            .and_then(|metadata| metadata.modified())
            .ok()
    }

    fn read_dir(&self, path: &Path) -> SandboxResult<Vec<PathBuf>> {
        let entries =
            std::fs::read_dir(path).map_err(|error| io_error("read_dir", path, &error))?;
        let mut paths = Vec::new();
        for entry in entries {
            let entry = entry.map_err(|error| io_error("read_dir", path, &error))?;
            paths.push(entry.path());
        }
        Ok(paths)
    }

    fn is_openable_read_write(&self, path: &Path) -> bool {
        OpenOptions::new().read(true).write(true).open(path).is_ok()
    }

    fn read_stream(&self, path: &Path) -> SandboxResult<Box<dyn Read + Send>> {
        let file = File::open(path).map_err(|error| io_error("read_stream", path, &error))?;
        Ok(Box::new(file))
    }

    fn clone_file(&self, from: &Path, to: &Path) -> SandboxResult<()> {
        if let Some(parent) = to.parent() {
            std::fs::create_dir_all(parent)
                .map_err(|error| io_error("clone_file", parent, &error))?;
        }
        if reflink(from, to) {
            return Ok(());
        }
        let mut source = File::open(from).map_err(|error| io_error("clone_file", from, &error))?;
        let mut target = OpenOptions::new()
            .write(true)
            .create(true)
            .truncate(true)
            .open(to)
            .map_err(|error| io_error("clone_file", to, &error))?;
        std::io::copy(&mut source, &mut target)
            .map_err(|error| io_error("clone_file", to, &error))?;
        Ok(())
    }

    fn lock_exclusive(&self, path: &Path) -> SandboxResult<crate::shared::host::FileLock> {
        #[cfg(unix)]
        {
            use std::os::fd::{AsRawFd as _, OwnedFd};

            let file = OpenOptions::new()
                .read(true)
                .write(true)
                .open(path)
                .map_err(|error| io_error("lock_exclusive", path, &error))?;
            let fd = OwnedFd::from(file);
            // SAFETY: `fd` is an open descriptor of this process. `flock` reads
            // only that descriptor and changes the lock state of its file.
            let result = unsafe { libc::flock(fd.as_raw_fd(), libc::LOCK_EX | libc::LOCK_NB) };
            if result != 0 {
                return Err(
                    io_error("lock_exclusive", path, &std::io::Error::last_os_error()).into(),
                );
            }
            Ok(crate::shared::host::FileLock::new(fd))
        }
        #[cfg(not(unix))]
        {
            Err(io_error(
                "lock_exclusive",
                path,
                &std::io::Error::from(std::io::ErrorKind::Unsupported),
            )
            .into())
        }
    }
}

/// Tries a copy-on-write clone of one file. It returns `false` when the file
/// system or the operating system has no clone call, so the caller copies.
fn reflink(from: &Path, to: &Path) -> bool {
    #[cfg(target_os = "linux")]
    {
        use std::os::fd::AsRawFd as _;

        // The FICLONE request number of `linux/fs.h`.
        const FICLONE: libc::c_ulong = 0x4004_9409;
        let Ok(source) = File::open(from) else {
            return false;
        };
        let Ok(target) = OpenOptions::new()
            .write(true)
            .create(true)
            .truncate(true)
            .open(to)
        else {
            return false;
        };
        // SAFETY: both descriptors are open, and FICLONE takes the source
        // descriptor as its argument.
        unsafe { libc::ioctl(target.as_raw_fd(), FICLONE, source.as_raw_fd()) == 0 }
    }
    #[cfg(target_os = "macos")]
    {
        use std::ffi::CString;

        let (Ok(source), Ok(target)) = (
            CString::new(from.as_os_str().as_encoded_bytes()),
            CString::new(to.as_os_str().as_encoded_bytes()),
        ) else {
            return false;
        };
        // SAFETY: both strings are valid and live for the call; `clonefile`
        // reads them only during the call.
        unsafe { libc::clonefile(source.as_ptr(), target.as_ptr(), 0) == 0 }
    }
    #[cfg(not(any(target_os = "linux", target_os = "macos")))]
    {
        let _ = (from, to);
        false
    }
}

/// The real side effects of the native control plane.
///
/// Only a boundary builds this bundle. A library call reads it with
/// [`SandboxHost::from_ctx`].
impl SandboxHost {
    /// The real processes, file system, connector and configuration.
    ///
    /// The process spawner starts one dedicated thread that outlives every
    /// child. The child of that thread dies with the host process, because
    /// `PR_SET_PDEATHSIG` follows the thread that started it.
    #[cfg(unix)]
    #[must_use]
    pub fn native(config: HostConfig) -> Self {
        Self {
            processes: Arc::new(NativeProcessSpawner::new()),
            fs: Arc::new(NativeFs::new()),
            connector: Arc::new(NativeConnector),
            config: Arc::new(config),
        }
    }
}

/// Starts host processes through a dedicated thread.
///
/// WHY: `prctl(PR_SET_PDEATHSIG, SIGKILL)` sends the signal when the **thread**
/// that made the child ends. A child made on an executor thread would die when
/// that thread is retired. Every child therefore comes from one thread that
/// starts at construction and lives as long as the spawner.
///
/// WHAT: [`ProcessSpawner::spawn`] sends the spec to that thread and waits for
/// its answer on a channel. The spawn is a short call, so a blocking wait here
/// is a boundary operation, not executor work.
///
/// HOW: `Command::env_clear()`, then the spec environment; standard input from
/// `/dev/null`; standard output and standard error to the two files, in append
/// mode.
#[cfg(unix)]
pub struct NativeProcessSpawner {
    requests: mpsc::Sender<SpawnRequest>,
}

#[cfg(unix)]
struct SpawnRequest {
    spec: ProcessSpec,
    reply: mpsc::Sender<SandboxResult<NativeChild>>,
}

#[cfg(unix)]
impl NativeProcessSpawner {
    /// A spawner with its one dedicated thread.
    ///
    /// # Panics
    /// When the operating system cannot start the thread.
    #[must_use]
    pub fn new() -> Self {
        let (requests, incoming) = mpsc::channel::<SpawnRequest>();
        std::thread::Builder::new()
            .name("ewe-sandbox-spawn".to_string())
            .spawn(move || {
                while let Ok(request) = incoming.recv() {
                    let result = spawn_process(request.spec);
                    let _ = request.reply.send(result);
                }
            })
            .expect("the sandbox spawner thread starts");
        Self { requests }
    }
}

#[cfg(unix)]
impl Default for NativeProcessSpawner {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(unix)]
impl std::fmt::Debug for NativeProcessSpawner {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter.write_str("NativeProcessSpawner")
    }
}

#[cfg(unix)]
impl ProcessSpawner for NativeProcessSpawner {
    fn spawn(&self, spec: ProcessSpec) -> SandboxResult<Box<dyn ChildProcess>> {
        let (reply, answer) = mpsc::channel();
        self.requests
            .send(SpawnRequest { spec, reply })
            .map_err(|_| SandboxError::Io {
                op: "spawn process".to_string(),
                message: "the spawner thread ended".to_string(),
            })?;
        let child = answer.recv().map_err(|_| SandboxError::Io {
            op: "spawn process".to_string(),
            message: "the spawner thread gave no answer".to_string(),
        })??;
        Ok(Box::new(child))
    }

    fn is_running(&self, pid: u32) -> bool {
        let Ok(pid) = libc::pid_t::try_from(pid) else {
            // A process identifier that does not fit: do not report it dead.
            return true;
        };
        // SAFETY: signal 0 checks for the process and sends nothing.
        let result = unsafe { libc::kill(pid, 0) };
        result == 0 || std::io::Error::last_os_error().raw_os_error() == Some(libc::EPERM)
    }
}

/// Starts one process from `spec` on the calling thread.
#[cfg(unix)]
fn spawn_process(mut spec: ProcessSpec) -> SandboxResult<NativeChild> {
    let stdout = open_append(&spec.stdout)?;
    let stderr = open_append(&spec.stderr)?;
    let extra_fds = std::mem::take(&mut spec.extra_fds);
    let mut command = std::process::Command::new(&spec.program);
    command
        .args(&spec.args)
        .env_clear()
        .envs(spec.env.iter().map(|(key, value)| (key, value)))
        .current_dir(&spec.cwd)
        .stdin(std::process::Stdio::null())
        .stdout(std::process::Stdio::from(stdout))
        .stderr(std::process::Stdio::from(stderr));

    #[cfg(unix)]
    if !extra_fds.is_empty() || (cfg!(target_os = "linux") && spec.kill_with_parent) {
        let kill_with_parent = spec.kill_with_parent;
        // SAFETY: `pre_exec` runs in the child after `fork`, so the closure
        // must call only async-signal-safe system calls. It reads memory that
        // the parent owns and the child inherits.
        unsafe {
            command.pre_exec(move || {
                for (number, fd) in &extra_fds {
                    use std::os::fd::AsRawFd as _;
                    if libc::dup2(fd.as_raw_fd(), *number) < 0 {
                        return Err(std::io::Error::last_os_error());
                    }
                }
                #[cfg(target_os = "linux")]
                if kill_with_parent {
                    if libc::prctl(libc::PR_SET_PDEATHSIG, libc::SIGKILL, 0, 0, 0) != 0 {
                        return Err(std::io::Error::last_os_error());
                    }
                    // The host process already died: no parent remains to
                    // receive the death signal.
                    if libc::getppid() == 1 {
                        libc::_exit(1);
                    }
                }
                #[cfg(not(target_os = "linux"))]
                let _ = kill_with_parent;
                Ok(())
            });
        }
    }

    let child = command.spawn().map_err(|error| SandboxError::Io {
        op: "spawn process".to_string(),
        message: format!("{}: {error}", spec.program.display()),
    })?;
    NativeChild::start(child)
}

/// Opens a log file in append mode, and creates it when it is absent.
#[cfg(unix)]
fn open_append(path: &Path) -> SandboxResult<File> {
    OpenOptions::new()
        .create(true)
        .append(true)
        .open(path)
        .map_err(|error| io_error("open process log", path, &error).into())
}

/// One started process with a helper thread that reaps it.
#[cfg(unix)]
pub struct NativeChild {
    pid: u32,
    state: Arc<ChildState>,
}

#[cfg(unix)]
struct ChildState {
    exit: Mutex<Option<ProcessExit>>,
    signal: ResumeSignal,
}

#[cfg(unix)]
impl NativeChild {
    /// Starts the helper thread that waits for `child`.
    fn start(child: std::process::Child) -> SandboxResult<Self> {
        let pid = child.id();
        let state = Arc::new(ChildState {
            exit: Mutex::new(None),
            signal: ResumeSignal::new(false),
        });
        let waiter = Arc::clone(&state);
        let (send_child, receive_child) = mpsc::channel::<std::process::Child>();
        let started = std::thread::Builder::new()
            .name(format!("ewe-sandbox-wait-{pid}"))
            .spawn(move || {
                let Ok(mut child) = receive_child.recv() else {
                    return;
                };
                let status = child.wait();
                let exit = match status {
                    Ok(status) => {
                        use std::os::unix::process::ExitStatusExt as _;
                        ProcessExit {
                            code: status.code(),
                            signal: status.signal(),
                        }
                    }
                    Err(_) => ProcessExit {
                        code: None,
                        signal: None,
                    },
                };
                *waiter.exit.lock().unwrap_or_else(PoisonError::into_inner) = Some(exit);
                waiter.signal.set(true);
            });
        if started.is_err() {
            let mut child = child;
            let _ = child.kill();
            let _ = child.wait();
            return Err(SandboxError::Io {
                op: "spawn process".to_string(),
                message: "the wait thread did not start".to_string(),
            }
            .into());
        }
        let _ = send_child.send(child);
        Ok(Self { pid, state })
    }
}

#[cfg(unix)]
impl ChildProcess for NativeChild {
    fn pid(&self) -> u32 {
        self.pid
    }

    fn signal(&self, signal: i32) -> SandboxResult<()> {
        let recorded = self
            .state
            .exit
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .is_some();
        if recorded {
            return Ok(());
        }
        let pid = libc::pid_t::try_from(self.pid).map_err(|_| SandboxError::Io {
            op: "signal process".to_string(),
            message: "the process identifier does not fit a signal call".to_string(),
        })?;
        // SAFETY: `pid` is the child that this value started, and the exit is
        // checked above under the lock that the wait thread writes.
        let result = unsafe { libc::kill(pid, signal) };
        if result != 0 {
            return Err(SandboxError::Io {
                op: "signal process".to_string(),
                message: std::io::Error::last_os_error().to_string(),
            }
            .into());
        }
        Ok(())
    }

    fn wait(&self) -> BoxFuture<'_, SandboxResult<ProcessExit>> {
        let state = Arc::clone(&self.state);
        Box::pin(futures::future::poll_fn(move |context| {
            let exit = *state.exit.lock().unwrap_or_else(PoisonError::into_inner);
            if let Some(exit) = exit {
                return Poll::Ready(Ok(exit));
            }
            state
                .signal
                .register(&ReadinessWaker::from_waker(context.waker().clone()));
            let exit = *state.exit.lock().unwrap_or_else(PoisonError::into_inner);
            match exit {
                Some(exit) => Poll::Ready(Ok(exit)),
                None => Poll::Pending,
            }
        }))
    }

    fn try_wait(&self) -> SandboxResult<Option<ProcessExit>> {
        Ok(*self
            .state
            .exit
            .lock()
            .unwrap_or_else(PoisonError::into_inner))
    }
}

/// Turns a slave endpoint into a Connect RPC transport.
///
/// The client read timeout is [`SandboxLimits::rpc_timeout`]. A streaming call
/// ends when the slave or the substrate ends it.
#[derive(Clone, Copy, Debug, Default)]
pub struct NativeConnector;

#[cfg(any(
    feature = "artifacts",
    feature = "firecracker",
    feature = "slave",
    feature = "testing"
))]
impl SlaveConnector for NativeConnector {
    fn connect(
        &self,
        _ctx: &Ctx,
        endpoint: &SlaveEndpoint,
        limits: &SandboxLimits,
    ) -> SandboxResult<Arc<dyn Transport>> {
        let timeout = limits.rpc_timeout;
        let client = match endpoint {
            SlaveEndpoint::Unix { path } => {
                #[cfg(unix)]
                {
                    HttpClientBuilder::new()
                        .unix_socket(path)
                        .unix_read_timeout(timeout)
                        .read_timeout(timeout)
                        // The slave is local: the 100-continue handshake only adds a wait.
                        .expect_continue(false)
                        .build()
                }
                #[cfg(not(unix))]
                {
                    let _ = path;
                    return Err(SandboxError::InvalidSpec {
                        field: "endpoint".to_string(),
                        reason: "a Unix domain socket needs a Unix host".to_string(),
                    }
                    .into());
                }
            }
            SlaveEndpoint::Tcp { host, port } => {
                let address = resolve_address(host, *port)?;
                HttpClientBuilder::new()
                    .resolve_to(address)
                    .read_timeout(timeout)
                    .expect_continue(false)
                    .build()
            }
            SlaveEndpoint::FirecrackerVsock { uds, port } => {
                #[cfg(unix)]
                {
                    let preamble = format!("CONNECT {port}\n").into_bytes();
                    HttpClientBuilder::new()
                        .unix_socket(uds)
                        .unix_read_timeout(timeout)
                        .unix_socket_preamble(preamble, b"OK ".to_vec(), 32)
                        .read_timeout(timeout)
                        .expect_continue(false)
                        .build()
                }
                #[cfg(not(unix))]
                {
                    let _ = (uds, port);
                    return Err(SandboxError::InvalidSpec {
                        field: "endpoint".to_string(),
                        reason: "a Firecracker vsock needs a Unix host".to_string(),
                    }
                    .into());
                }
            }
            SlaveEndpoint::CloudflarePort { .. } => {
                return Err(SandboxError::InvalidSpec {
                    field: "endpoint".to_string(),
                    reason: "a Cloudflare container port is not reachable from a native host"
                        .to_string(),
                }
                .into());
            }
            SlaveEndpoint::CloudflareDo { .. } => {
                return Err(SandboxError::PlatformMismatch {
                    expected: crate::shared::platform::PlatformKind::Cloudflare,
                    actual: crate::shared::platform::PlatformKind::Native,
                }
                .into());
            }
            SlaveEndpoint::Parent { .. } => {
                // A nested child has no native transport: its calls go to the
                // parent's inner control plane (feature 11).
                return Err(SandboxError::InvalidSpec {
                    field: "endpoint".to_string(),
                    reason: "a nested child is reached through its parent".to_string(),
                }
                .into());
            }
        };
        Ok(Arc::new(H1Transport::new(client)))
    }
}

/// A connector for a build with no network feature.
///
/// The host file system and the process spawner are useful without the network
/// stack. A build with no artifact, Firecracker, slave or testing feature has no
/// client to dial with, so the connector gives a typed error.
#[cfg(not(any(
    feature = "artifacts",
    feature = "firecracker",
    feature = "slave",
    feature = "testing"
)))]
impl SlaveConnector for NativeConnector {
    fn connect(
        &self,
        _ctx: &Ctx,
        _endpoint: &SlaveEndpoint,
        _limits: &SandboxLimits,
    ) -> SandboxResult<Arc<dyn foundation_connectrpc::shared::transport::Transport>> {
        Err(SandboxError::Io {
            op: "connect".to_string(),
            message: "this build has no network feature".to_string(),
        }
        .into())
    }
}

/// Resolves `host:port` to one socket address.
#[cfg(any(
    feature = "artifacts",
    feature = "firecracker",
    feature = "slave",
    feature = "testing"
))]
fn resolve_address(host: &str, port: u16) -> SandboxResult<std::net::SocketAddr> {
    use std::net::ToSocketAddrs as _;
    let mut addresses = (host, port)
        .to_socket_addrs()
        .map_err(|error| SandboxError::Io {
            op: "resolve slave address".to_string(),
            message: error.to_string(),
        })?;
    addresses.next().ok_or_else(|| {
        SandboxError::Io {
            op: "resolve slave address".to_string(),
            message: format!("{host}:{port} resolved to no address"),
        }
        .into()
    })
}

impl HostConfig {
    /// Reads the host paths and the driver order.
    ///
    /// `env` maps a name to its value. A boundary passes the real environment
    /// reader; a test passes a map. `exe_dir` is the directory of the host
    /// program, for the programs that ship beside it.
    ///
    /// Sources, in order:
    ///
    /// | Field | Sources |
    /// |---|---|
    /// | `state_dir` | `EWE_SANDBOX_STATE_DIR`; `$XDG_RUNTIME_DIR/ewe-sbx`; `/tmp/ewe-sbx-<uid>` |
    /// | `cache_dir` | `EWE_SANDBOX_CACHE_DIR`; `$XDG_CACHE_HOME/ewe-sandbox`; `$HOME/.cache/ewe-sandbox` |
    /// | `slave_path` | `EWE_SANDBOX_SLAVE`; `<exe_dir>/agent-slave-<guest_arch>` |
    /// | `firecracker_path` | `EWE_FIRECRACKER`; `firecracker` on `PATH` |
    /// | `krun_runner_path` | `EWE_SANDBOX_KRUN`; `<exe_dir>/sandbox-krun` |
    /// | `libkrun_path` | `EWE_LIBKRUN`; none |
    /// | `driver_order` | `EWE_SANDBOX_DRIVERS` (comma list); `firecracker, libkrun, docker` |
    #[must_use]
    pub fn discover(env: impl Fn(&str) -> Option<String>, exe_dir: &Path) -> Self {
        let state_dir = env("EWE_SANDBOX_STATE_DIR")
            .map(PathBuf::from)
            .or_else(|| env("XDG_RUNTIME_DIR").map(|dir| PathBuf::from(dir).join("ewe-sbx")))
            .unwrap_or_else(|| PathBuf::from(format!("/tmp/ewe-sbx-{}", user_id(&env))));
        let cache_dir = env("EWE_SANDBOX_CACHE_DIR")
            .map(PathBuf::from)
            .or_else(|| env("XDG_CACHE_HOME").map(|dir| PathBuf::from(dir).join("ewe-sandbox")))
            .or_else(|| env("HOME").map(|dir| PathBuf::from(dir).join(".cache/ewe-sandbox")))
            .unwrap_or_else(|| PathBuf::from(".cache/ewe-sandbox"));
        let arch = GuestArch::host();
        let slave_path = env("EWE_SANDBOX_SLAVE").map_or_else(
            || exe_dir.join(format!("agent-slave-{}", arch.as_str())),
            PathBuf::from,
        );
        let firecracker_path = env("EWE_FIRECRACKER")
            .map(PathBuf::from)
            .or_else(|| find_on_path(&env, "firecracker"));
        let krun_runner_path = Some(
            env("EWE_SANDBOX_KRUN").map_or_else(|| exe_dir.join("sandbox-krun"), PathBuf::from),
        );
        let libkrun_path = env("EWE_LIBKRUN").map(PathBuf::from);
        let mkfs_ext4_path = env("EWE_MKFS_EXT4")
            .map(PathBuf::from)
            .or_else(|| find_on_path(&env, "mkfs.ext4"));
        let driver_order = env("EWE_SANDBOX_DRIVERS")
            .map(|list| parse_driver_order(&list))
            .filter(|order| !order.is_empty())
            .unwrap_or_else(|| vec![DriverId::Firecracker, DriverId::Libkrun, DriverId::Docker]);
        Self {
            state_dir,
            cache_dir,
            slave_path: Some(slave_path),
            firecracker_path,
            krun_runner_path,
            libkrun_path,
            mkfs_ext4_path,
            driver_order,
            artifact_limits: crate::shared::limits::ArtifactLimits::default(),
            image_limits: crate::shared::limits::ImageLimits::default(),
        }
    }
}

/// The user identifier for the state-directory fallback.
fn user_id(env: &impl Fn(&str) -> Option<String>) -> u32 {
    if let Some(uid) = env("UID").and_then(|value| value.parse().ok()) {
        return uid;
    }
    #[cfg(unix)]
    {
        // SAFETY: `getuid` reads the process credential and has no side effect.
        unsafe { libc::getuid() }
    }
    #[cfg(not(unix))]
    {
        0
    }
}

/// Finds a program on `PATH`.
fn find_on_path(env: &impl Fn(&str) -> Option<String>, program: &str) -> Option<PathBuf> {
    let path = env("PATH")?;
    std::env::split_paths(&path)
        .map(|dir| dir.join(program))
        .find(|candidate| candidate.is_file())
}

/// Parses a comma list of driver identifiers.
fn parse_driver_order(list: &str) -> Vec<DriverId> {
    use std::str::FromStr as _;
    list.split(',')
        .filter_map(|entry| DriverId::from_str(entry.trim()).ok())
        .collect()
}
