//! The libkrun driver (feature 09).
//!
//! One `sandbox-krun` process is one microVM. The driver writes the runner
//! configuration, starts the runner, and returns a
//! [`Substrate`](crate::shared::driver::Substrate) with the Unix domain socket
//! that libkrun makes for the slave.
//!
//! The driver never calls the library (invariant I1). The runner process calls
//! `krun_start_enter`, which does not return on success, so the virtual machine
//! cannot share a process with the control plane.
//!
//! A failed launch leaves no runner process and no run directory (invariant I7).

use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
#[cfg(target_os = "macos")]
use std::sync::{Mutex, PoisonError};
use std::time::Duration;

use foundation_compact::Instant;
use foundation_context::Ctx;
use foundation_core::valtron::{execute, task, TaskStatus};
use futures::future::{select, Either};

use crate::shared::driver::{
    Capabilities, CreateProgress, HostCheck, HostReport, LaunchStream, SandboxDriver,
    SlaveEndpoint, StoredRecord, Substrate, SubstrateExit, SubstrateHandle,
};
use crate::shared::error::{SandboxError, SandboxResult};
use crate::shared::host::{ChildProcess, HostFs, ProcessExit, ProcessSpec, SandboxHost};
use crate::shared::ids::{check_socket_path, DriverId, SandboxId};
use crate::shared::krun_config::{KrunMount, KrunProbe, KrunRunnerConfig, KRUN_LOG_LEVEL_WARN};
use crate::shared::limits::SandboxLimits;
use crate::shared::spec::{
    GuestArch, GuestMount, NetworkMode, RootFs, RootMode, RootSetup, RootSource, SandboxSpec,
};
use crate::shared::BoxFuture;

/// The vsock port that the slave listens on.
pub const SLAVE_VSOCK_PORT: u32 = 1024;

/// The virtio-fs tag of the caller's root directory.
pub const USER_ROOT_TAG: &str = "ewe-root";

/// The prefix of the tag of one extra host mount. The index follows the prefix.
pub const MOUNT_TAG_PREFIX: &str = "ewe-m";

/// The four directories of the boot root. The libkrun init needs `dev`, `proc`
/// and `sys`; `run` is the private writable directory of the guest.
pub const BOOT_ROOT_DIRS: [&str; 4] = ["dev", "proc", "sys", "run"];

/// The lifeline write end on macOS, and nothing on Linux.
#[cfg(target_os = "macos")]
pub type Lifeline = Option<std::os::fd::OwnedFd>;
/// The lifeline write end on macOS, and nothing on Linux.
#[cfg(not(target_os = "macos"))]
pub type Lifeline = ();

/// The libkrun driver.
pub struct LibkrunDriver {
    log_level: u32,
}

impl Default for LibkrunDriver {
    fn default() -> Self {
        Self::new()
    }
}

impl std::fmt::Debug for LibkrunDriver {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("LibkrunDriver")
            .field("log_level", &self.log_level)
            .finish_non_exhaustive()
    }
}

impl LibkrunDriver {
    /// A driver with the default log level.
    #[must_use]
    pub fn new() -> Self {
        Self {
            log_level: KRUN_LOG_LEVEL_WARN,
        }
    }

    /// A driver with another libkrun log level.
    ///
    /// The values are the `KRUN_LOG_LEVEL_*` constants in
    /// [`crate::shared::krun_config`]. The default is
    /// [`KRUN_LOG_LEVEL_WARN`](crate::shared::krun_config::KRUN_LOG_LEVEL_WARN):
    /// it explains a failed start and keeps the log small.
    #[must_use]
    pub fn with_log_level(mut self, log_level: u32) -> Self {
        self.log_level = log_level;
        self
    }

    /// The libkrun log level of this driver.
    #[must_use]
    pub fn log_level(&self) -> u32 {
        self.log_level
    }
}

/// The capabilities of this driver (feature 09, section 4.4).
///
/// `max_vcpus` is the value from the probe. A root image format that has no
/// virtio-fs reader is off: the driver shares directories only.
#[must_use]
pub fn capabilities(max_vcpus: u8) -> Capabilities {
    Capabilities {
        rootfs_directory: true,
        rootfs_ext4: false,
        rootfs_squashfs: false,
        rootfs_container_image: false,
        root_modes: vec![RootMode::Overlay, RootMode::ReadOnly],
        host_mounts: true,
        network_none: true,
        network_host: true,
        max_vcpus,
    }
}

/// Every resolved path and value of one virtual machine.
///
/// The `Prepare` state builds it; the `Spawn` state starts the runner and then
/// builds the substrate.
pub struct KrunPlan {
    /// The sandbox identifier.
    pub id: SandboxId,
    /// The directory of this sandbox: sockets, logs and generated files.
    pub run_dir: PathBuf,
    /// The private boot root, with the four empty directories.
    pub boot_root: PathBuf,
    /// The `sandbox-krun` program.
    pub runner: PathBuf,
    /// The full runner description, as written to `krun.json`.
    pub config: KrunRunnerConfig,
    /// How writes to the root are handled.
    pub root_mode: RootMode,
    /// The mount descriptions that the control plane sends in `Prepare`.
    pub mounts: Vec<GuestMount>,
    /// The environment of the runner process.
    pub env: Vec<(String, String)>,
    /// Every size and time limit of the sandbox.
    pub limits: SandboxLimits,
    /// The started runner, after the first `Spawn` step.
    pub started: Option<KrunStarted>,
}

/// One started runner process.
pub struct KrunStarted {
    /// The runner process.
    pub child: Arc<dyn ChildProcess>,
    /// The write end of the macOS lifeline. The runner dies when it closes.
    pub lifeline: Lifeline,
}

impl KrunPlan {
    /// The runner configuration path.
    #[must_use]
    pub fn config_path(&self) -> PathBuf {
        self.run_dir.join("krun.json")
    }

    /// The Unix domain socket that libkrun makes for the slave port.
    #[must_use]
    pub fn slave_socket(&self) -> PathBuf {
        self.run_dir.join("slave.sock")
    }

    /// The standard output and standard error path of the runner.
    #[must_use]
    pub fn console_log(&self) -> PathBuf {
        self.run_dir.join("console.log")
    }
}

/// One step of the launch.
pub enum KrunLaunch {
    /// Check the root, make the run directory and write `krun.json`.
    Prepare {
        /// The injected context.
        ctx: Ctx,
        /// The sandbox identifier.
        id: SandboxId,
        /// The caller's spec.
        spec: SandboxSpec,
        /// The libkrun log level.
        log_level: u32,
    },
    /// Start the runner, then build the substrate.
    Spawn {
        /// The injected context.
        ctx: Ctx,
        /// The resolved plan.
        plan: KrunPlan,
    },
    /// The launch is done.
    Done,
}

type Status = TaskStatus<SandboxResult<Substrate>, CreateProgress>;

/// One transition of the launch state machine.
///
/// The first `Spawn` starts the runner and emits `SubstrateStarted`. The second
/// `Spawn` builds the substrate and emits the one result. Two steps are
/// necessary: one transition emits one status, and the control plane ignores a
/// `Spread`.
#[task]
pub async fn krun_launch(
    ctx: Ctx,
    state: KrunLaunch,
) -> Option<(
    KrunLaunch,
    TaskStatus<SandboxResult<Substrate>, CreateProgress>,
)> {
    drop(ctx);
    let next = match state {
        KrunLaunch::Prepare {
            ctx,
            id,
            spec,
            log_level,
        } => prepare(ctx, id, spec, log_level).await,
        KrunLaunch::Spawn { ctx, plan } => spawn(ctx, plan).await,
        KrunLaunch::Done => return None,
    };
    Some(next)
}

#[allow(
    clippy::unused_async,
    reason = "the #[task] transition is async; the prepare step is a short sync call"
)]
async fn prepare(
    ctx: Ctx,
    id: SandboxId,
    spec: SandboxSpec,
    log_level: u32,
) -> (KrunLaunch, Status) {
    match prepare_plan(&ctx, &id, &spec, log_level) {
        Ok(plan) => (
            KrunLaunch::Spawn { ctx, plan },
            TaskStatus::Pending(CreateProgress::Preparing),
        ),
        Err(error) => (KrunLaunch::Done, TaskStatus::Ready(Err(error))),
    }
}

#[allow(
    clippy::unused_async,
    reason = "the #[task] transition is async; the spawn step is a short sync boundary call"
)]
async fn spawn(ctx: Ctx, mut plan: KrunPlan) -> (KrunLaunch, Status) {
    if plan.started.is_some() {
        return ready(&ctx, plan);
    }
    let Ok(host) = SandboxHost::from_ctx(&ctx) else {
        // The host bundle is gone, so no file system is left to clean up with.
        tracing::warn!(
            run_dir = %plan.run_dir.display(),
            "cannot remove the run directory: the sandbox host is not configured"
        );
        return (
            KrunLaunch::Done,
            TaskStatus::Ready(Err(SandboxError::HostNotConfigured.into())),
        );
    };
    match spawn_runner(&host, &mut plan) {
        Ok(()) => (
            KrunLaunch::Spawn { ctx, plan },
            TaskStatus::Pending(CreateProgress::SubstrateStarted),
        ),
        Err(error) => {
            remove_dir_logged(&host.fs, &plan.run_dir);
            (KrunLaunch::Done, TaskStatus::Ready(Err(error)))
        }
    }
}

/// Builds the substrate after the runner started.
fn ready(ctx: &Ctx, plan: KrunPlan) -> (KrunLaunch, Status) {
    let Ok(host) = SandboxHost::from_ctx(ctx) else {
        tracing::warn!(
            run_dir = %plan.run_dir.display(),
            "cannot remove the run directory: the sandbox host is not configured"
        );
        return (
            KrunLaunch::Done,
            TaskStatus::Ready(Err(SandboxError::HostNotConfigured.into())),
        );
    };
    let KrunPlan {
        run_dir,
        root_mode,
        mounts,
        started,
        ..
    } = plan;
    let Some(started) = started else {
        remove_dir_logged(&host.fs, &run_dir);
        let error: SandboxResult<Substrate> = Err(SandboxError::Io {
            op: "build the substrate".to_string(),
            message: "the runner process is missing".to_string(),
        }
        .into());
        return (KrunLaunch::Done, TaskStatus::Ready(error));
    };
    #[cfg(target_os = "macos")]
    let handle = KrunHandle::new(
        ctx.clone(),
        started.child,
        host.fs.clone(),
        run_dir.clone(),
        started.lifeline,
    );
    #[cfg(not(target_os = "macos"))]
    let handle = KrunHandle::new(ctx.clone(), started.child, host.fs.clone(), run_dir.clone());
    let substrate = Substrate {
        endpoint: SlaveEndpoint::Unix {
            path: run_dir.join("slave.sock"),
        },
        root: RootSetup {
            source: RootSource::VirtioFs {
                tag: USER_ROOT_TAG.to_string(),
            },
            mode: root_mode,
        },
        mounts,
        handle: Box::new(handle),
    };
    (KrunLaunch::Done, TaskStatus::Ready(Ok(substrate)))
}

impl SandboxDriver for LibkrunDriver {
    fn id(&self) -> DriverId {
        DriverId::Libkrun
    }

    fn probe(&self, ctx: Ctx) -> BoxFuture<'static, HostReport> {
        Box::pin(async move {
            // One deadline for every host program: `probe` waits no longer than
            // `rpc_timeout` (invariant I9).
            let deadline = ctx.clock().now() + SandboxLimits::default().rpc_timeout;
            let Ok(host) = SandboxHost::from_ctx(&ctx) else {
                return HostReport {
                    driver: DriverId::Libkrun,
                    usable: false,
                    checks: vec![HostCheck {
                        name: "host".to_string(),
                        ok: false,
                        message: "the sandbox host is not configured".to_string(),
                    }],
                    capabilities: capabilities(0),
                };
            };
            let mut checks = Vec::new();

            let hypervisor = check_hypervisor(&ctx, &host, deadline).await;
            checks.push(hypervisor);

            let runner_path = host.config.krun_runner_path.clone();
            let runner_present = runner_path
                .as_ref()
                .is_some_and(|path| host.fs.exists(path));
            checks.push(HostCheck {
                name: "runner".to_string(),
                ok: runner_present,
                message: match &runner_path {
                    Some(path) if runner_present => format!("the runner is at {}", path.display()),
                    Some(path) => format!("no runner at {}", path.display()),
                    None => "no runner program configured".to_string(),
                },
            });

            checks.push(check_runner_signed(&ctx, &host, deadline).await);

            let probe = if runner_present {
                probe_library(&ctx, &host, deadline).await
            } else {
                Err("no runner program to probe".to_string())
            };
            let mut max_vcpus = 0_u8;
            checks.push(HostCheck {
                name: "libkrun".to_string(),
                ok: probe.is_ok(),
                message: match &probe {
                    Ok(found) => {
                        max_vcpus = found.max_vcpus;
                        format!("{} with at most {} vCPUs", found.library, found.max_vcpus)
                    }
                    Err(reason) => reason.clone(),
                },
            });

            let slave_present = host
                .config
                .slave_path
                .as_ref()
                .is_some_and(|path| host.fs.exists(path));
            let arch = GuestArch::host();
            checks.push(HostCheck {
                name: "slave".to_string(),
                ok: slave_present,
                message: match &host.config.slave_path {
                    Some(path) if slave_present => {
                        format!("the {} slave is at {}", arch.as_str(), path.display())
                    }
                    Some(path) => format!("no {} slave at {}", arch.as_str(), path.display()),
                    None => format!("no {} slave configured", arch.as_str()),
                },
            });

            let usable = checks.iter().all(|check| check.ok);
            HostReport {
                driver: DriverId::Libkrun,
                usable,
                checks,
                capabilities: capabilities(max_vcpus),
            }
        })
    }

    fn launch(
        &self,
        ctx: Ctx,
        id: SandboxId,
        spec: SandboxSpec,
        _deadline: Instant,
    ) -> LaunchStream {
        let state = KrunLaunch::Prepare {
            ctx,
            id,
            spec,
            log_level: self.log_level,
        };
        match execute(krun_launch::new(state), None) {
            Ok(iterator) => Box::new(iterator),
            Err(error) => {
                let error: SandboxResult<Substrate> = Err(SandboxError::Io {
                    op: "launch libkrun".to_string(),
                    message: error.to_string(),
                }
                .into());
                Box::new(std::iter::once(foundation_core::valtron::Stream::Next(
                    error,
                )))
            }
        }
    }

    fn attach(
        &self,
        ctx: Ctx,
        record: &StoredRecord,
    ) -> BoxFuture<'_, SandboxResult<Option<Substrate>>> {
        let id = record.id.clone();
        Box::pin(async move {
            let host = SandboxHost::from_ctx(&ctx)?;
            let run_dir = host.config.state_dir.join(id.as_str());
            if host.fs.exists(&run_dir) {
                host.fs.remove_dir_all(&run_dir)?;
            }
            Ok(None)
        })
    }
}

/// Resolves every file of one virtual machine and makes its run directory.
fn prepare_plan(
    ctx: &Ctx,
    id: &SandboxId,
    spec: &SandboxSpec,
    log_level: u32,
) -> SandboxResult<KrunPlan> {
    let host = SandboxHost::from_ctx(ctx)?;
    let user_root = match &spec.rootfs {
        RootFs::Directory(path) => path.clone(),
        _ => return Err(SandboxError::UnsupportedRootFs.into()),
    };
    if !host.fs.exists(&user_root) {
        return Err(SandboxError::InvalidSpec {
            field: "rootfs".to_string(),
            reason: format!("the root directory {} does not exist", user_root.display()),
        }
        .into());
    }
    let runner = resolve_program(
        &host,
        host.config.krun_runner_path.as_deref(),
        "sandbox-krun",
    )?;
    let slave = resolve_program(&host, host.config.slave_path.as_deref(), "agent-slave")?;
    let run_dir = host.config.state_dir.join(id.as_str());
    let boot_root = run_dir.join("root");

    let prepared = (|| -> SandboxResult<(KrunRunnerConfig, Vec<GuestMount>)> {
        host.fs.create_dir_all(&run_dir, 0o700)?;
        host.fs.create_dir_all(&boot_root, 0o700)?;
        for directory in BOOT_ROOT_DIRS {
            host.fs.create_dir_all(&boot_root.join(directory), 0o755)?;
        }
        check_socket_path(&run_dir.join("slave.sock"))?;

        let mut krun_mounts = Vec::with_capacity(spec.mounts.len());
        let mut guest_mounts = Vec::with_capacity(spec.mounts.len());
        for (index, mount) in spec.mounts.iter().enumerate() {
            let tag = format!("{MOUNT_TAG_PREFIX}{index}");
            krun_mounts.push(KrunMount {
                tag: tag.clone(),
                host_path: mount.host_path.clone(),
                read_only: mount.read_only,
            });
            guest_mounts.push(GuestMount {
                tag,
                guest_path: mount.guest_path.clone(),
                read_only: mount.read_only,
            });
        }

        let config = KrunRunnerConfig {
            libkrun_path: host.config.libkrun_path.clone(),
            vcpus: spec.vcpus,
            memory_mib: spec.memory_mib,
            boot_root: boot_root.clone(),
            user_root: Some(KrunMount {
                tag: USER_ROOT_TAG.to_string(),
                host_path: user_root.clone(),
                read_only: true,
            }),
            slave_path: slave.clone(),
            slave_socket: run_dir.join("slave.sock"),
            slave_port: SLAVE_VSOCK_PORT,
            tsi_inet: spec.network == NetworkMode::Host,
            mounts: krun_mounts,
            log_level,
            lifeline_fd: lifeline_fd(),
        };
        let json = serde_json::to_vec_pretty(&config).map_err(|error| SandboxError::Io {
            op: "encode krun.json".to_string(),
            message: error.to_string(),
        })?;
        host.fs.write(&run_dir.join("krun.json"), &json)?;
        Ok((config, guest_mounts))
    })();

    match prepared {
        Ok((config, mounts)) => {
            let env = runner_env(&config);
            Ok(KrunPlan {
                id: id.clone(),
                run_dir,
                boot_root,
                runner,
                config,
                root_mode: spec.root_mode,
                mounts,
                env,
                limits: spec.limits.clone(),
                started: None,
            })
        }
        Err(error) => {
            remove_dir_logged(&host.fs, &run_dir);
            Err(error)
        }
    }
}

/// The lifeline descriptor of this system.
fn lifeline_fd() -> Option<i32> {
    if cfg!(target_os = "macos") {
        // The driver passes the read end of a pipe as descriptor 3.
        Some(3)
    } else {
        None
    }
}

/// The loader environment of the runner, from the library directory.
fn runner_env(config: &KrunRunnerConfig) -> Vec<(String, String)> {
    let Some(library) = &config.libkrun_path else {
        return Vec::new();
    };
    let Some(directory) = library.parent() else {
        return Vec::new();
    };
    // The runner gets an empty environment, so this is the one entry. It lets
    // the loader find `libkrun` and `libkrunfw` beside the given library.
    let key = if cfg!(target_os = "macos") {
        "DYLD_LIBRARY_PATH"
    } else {
        "LD_LIBRARY_PATH"
    };
    vec![(key.to_string(), directory.display().to_string())]
}

/// Resolves one configured program and checks that it exists.
fn resolve_program(
    host: &SandboxHost,
    configured: Option<&Path>,
    name: &str,
) -> SandboxResult<PathBuf> {
    let path = configured
        .map(Path::to_path_buf)
        .ok_or_else(|| SandboxError::BinaryNotFound {
            name: name.to_string(),
        })?;
    if !host.fs.exists(&path) {
        return Err(SandboxError::BinaryNotFound {
            name: path.display().to_string(),
        }
        .into());
    }
    Ok(path)
}

/// Starts the runner process of one plan.
fn spawn_runner(host: &SandboxHost, plan: &mut KrunPlan) -> SandboxResult<()> {
    let (extra_fds, lifeline) = make_lifeline()?;
    let spec = ProcessSpec {
        program: plan.runner.clone(),
        args: vec![plan.config_path().display().to_string()],
        env: plan.env.clone(),
        cwd: plan.run_dir.clone(),
        stdout: plan.console_log(),
        stderr: plan.console_log(),
        // Linux uses the parent-death signal. macOS uses the lifeline pipe.
        kill_with_parent: cfg!(target_os = "linux"),
        extra_fds,
    };
    let child = host.processes.spawn(spec)?;
    plan.started = Some(KrunStarted {
        child: Arc::from(child),
        lifeline,
    });
    Ok(())
}

/// Makes the macOS lifeline pipe and returns its two ends.
///
/// The read end goes to the runner as descriptor 3. The write end stays with
/// the handle. Both ends are close-on-exec, so no other child keeps the write
/// end open after a new program starts.
#[cfg(target_os = "macos")]
fn make_lifeline() -> SandboxResult<(Vec<(i32, std::os::fd::OwnedFd)>, Lifeline)> {
    use std::os::fd::{AsRawFd as _, FromRawFd as _, OwnedFd};

    let mut fds = [0_i32; 2];
    // SAFETY: `fds` is a two-element array that `pipe` fills on success.
    if unsafe { libc::pipe(fds.as_mut_ptr()) } != 0 {
        return Err(SandboxError::Io {
            op: "make the lifeline pipe".to_string(),
            message: std::io::Error::last_os_error().to_string(),
        }
        .into());
    }
    // SAFETY: each descriptor is open and owned once by these values.
    let read = unsafe { OwnedFd::from_raw_fd(fds[0]) };
    // SAFETY: as above for the second descriptor.
    let write = unsafe { OwnedFd::from_raw_fd(fds[1]) };
    for fd in [&read, &write] {
        // SAFETY: `fd` is an open descriptor of this process.
        if unsafe { libc::fcntl(fd.as_raw_fd(), libc::F_SETFD, libc::FD_CLOEXEC) } < 0 {
            return Err(SandboxError::Io {
                op: "mark the lifeline pipe close-on-exec".to_string(),
                message: std::io::Error::last_os_error().to_string(),
            }
            .into());
        }
    }
    Ok((vec![(3, read)], Some(write)))
}

/// A host without the macOS lifeline has no pipe.
#[cfg(not(target_os = "macos"))]
#[allow(
    clippy::unnecessary_wraps,
    reason = "the macOS twin fails when the pipe cannot be made; one signature serves both"
)]
fn make_lifeline() -> SandboxResult<(Vec<(i32, std::os::fd::OwnedFd)>, Lifeline)> {
    Ok((Vec::new(), ()))
}

/// Removes one private directory and reports a failure.
fn remove_dir_logged(fs: &Arc<dyn HostFs>, path: &Path) {
    if let Err(error) = fs.remove_dir_all(path) {
        tracing::warn!(
            path = %path.display(),
            error = %error,
            "cannot remove a private directory of the sandbox"
        );
    }
}

/// Maps a runner exit code to its error (feature 09, section 3.5).
#[must_use]
pub fn krun_exit_error(code: Option<i32>, log_tail: Vec<u8>) -> SandboxError {
    match code {
        Some(64) => SandboxError::InvalidSpec {
            field: "spec".to_string(),
            reason: format!(
                "the runner rejected the configuration: {}",
                String::from_utf8_lossy(&log_tail)
            ),
        },
        Some(71) => {
            let text = String::from_utf8_lossy(&log_tail);
            match text
                .lines()
                .find(|line| line.contains("libkrun has no symbol "))
                .and_then(|line| line.split("libkrun has no symbol ").nth(1))
                .map(str::trim)
                .filter(|symbol| !symbol.is_empty())
            {
                Some(symbol) => SandboxError::LibraryTooOld {
                    symbol: symbol.to_string(),
                },
                None => SandboxError::LibraryNotFound {
                    name: "libkrun".to_string(),
                },
            }
        }
        _ => SandboxError::SubstrateExited { code, log_tail },
    }
}

/// The last `max` bytes of `console.log`.
#[must_use]
pub fn read_log_tail(fs: &Arc<dyn HostFs>, run_dir: &Path, max: usize) -> Vec<u8> {
    fs.tail(&run_dir.join("console.log"), max)
        .unwrap_or_default()
}

/// The handle of one runner process.
pub struct KrunHandle {
    ctx: Ctx,
    child: Arc<dyn ChildProcess>,
    fs: Arc<dyn HostFs>,
    run_dir: PathBuf,
    forced: AtomicBool,
    #[cfg(target_os = "macos")]
    lifeline: Mutex<Option<std::os::fd::OwnedFd>>,
}

impl KrunHandle {
    /// The handle of one started runner, with its macOS lifeline.
    #[cfg(target_os = "macos")]
    #[must_use]
    pub fn new(
        ctx: Ctx,
        child: Arc<dyn ChildProcess>,
        fs: Arc<dyn HostFs>,
        run_dir: PathBuf,
        lifeline: Lifeline,
    ) -> Self {
        Self {
            ctx,
            child,
            fs,
            run_dir,
            forced: AtomicBool::new(false),
            lifeline: Mutex::new(lifeline),
        }
    }

    /// The handle of one started runner. Linux has no lifeline: the runner
    /// dies with its parent.
    #[cfg(not(target_os = "macos"))]
    #[must_use]
    pub fn new(
        ctx: Ctx,
        child: Arc<dyn ChildProcess>,
        fs: Arc<dyn HostFs>,
        run_dir: PathBuf,
    ) -> Self {
        Self {
            ctx,
            child,
            fs,
            run_dir,
            forced: AtomicBool::new(false),
        }
    }

    fn exit(&self, exit: ProcessExit) -> SubstrateExit {
        SubstrateExit {
            code: exit.code,
            signal: exit.signal,
            forced: self.forced.load(Ordering::SeqCst),
        }
    }

    async fn wait_with_timeout(&self, timeout: Duration) -> Option<ProcessExit> {
        let timer = self.ctx.require_env().ok()?.timer();
        let deadline = self.ctx.clock().now() + timeout;
        match select(
            self.child.wait(),
            timer.sleep_until(self.ctx.clone(), deadline),
        )
        .await
        {
            Either::Left((exit, _)) => exit.ok(),
            Either::Right(_) => None,
        }
    }

    /// Closes the lifeline, when this system has one.
    #[allow(
        clippy::unused_self,
        reason = "the macOS body reads the lifeline field; Linux has no lifeline"
    )]
    fn close_lifeline(&self) {
        #[cfg(target_os = "macos")]
        {
            let _ = self
                .lifeline
                .lock()
                .unwrap_or_else(PoisonError::into_inner)
                .take();
        }
    }
}

impl SubstrateHandle for KrunHandle {
    fn exited(&self) -> BoxFuture<'_, SubstrateExit> {
        Box::pin(async move {
            match self.child.wait().await {
                Ok(exit) => self.exit(exit),
                Err(_) => SubstrateExit {
                    code: None,
                    signal: None,
                    forced: self.forced.load(Ordering::SeqCst),
                },
            }
        })
    }

    fn terminate(&self, grace: Duration) -> BoxFuture<'_, SandboxResult<SubstrateExit>> {
        Box::pin(async move {
            if let Some(exit) = self.child.try_wait()? {
                return Ok(self.exit(exit));
            }
            if let Some(exit) = self.wait_with_timeout(grace).await {
                return Ok(self.exit(exit));
            }
            self.forced.store(true, Ordering::SeqCst);
            self.child.signal(libc::SIGTERM)?;
            // The one second after SIGTERM mirrors the Firecracker handle. The
            // caller's `shutdown_grace` is the wait before SIGTERM.
            if let Some(exit) = self.wait_with_timeout(Duration::from_secs(1)).await {
                return Ok(self.exit(exit));
            }
            self.child.signal(libc::SIGKILL)?;
            let exit = self.child.wait().await?;
            Ok(self.exit(exit))
        })
    }

    fn kill(&self) -> BoxFuture<'_, SandboxResult<SubstrateExit>> {
        Box::pin(async move {
            if let Some(exit) = self.child.try_wait()? {
                return Ok(self.exit(exit));
            }
            self.forced.store(true, Ordering::SeqCst);
            self.child.signal(libc::SIGKILL)?;
            let exit = self.child.wait().await?;
            Ok(self.exit(exit))
        })
    }

    fn log_tail(&self, max: usize) -> BoxFuture<'_, Vec<u8>> {
        Box::pin(async move { read_log_tail(&self.fs, &self.run_dir, max) })
    }

    fn cleanup(&self) -> BoxFuture<'_, SandboxResult<()>> {
        Box::pin(async move {
            if self.child.try_wait()?.is_none() {
                let _ = self.kill().await;
            }
            self.close_lifeline();
            if self.fs.exists(&self.run_dir) {
                self.fs.remove_dir_all(&self.run_dir)?;
            }
            Ok(())
        })
    }

    fn exit_error(&self, exit: &SubstrateExit, log_tail: Vec<u8>) -> SandboxError {
        krun_exit_error(exit.code, log_tail)
    }
}

/// The host check of the hypervisor (feature 09, section 5).
#[allow(
    clippy::unused_async,
    reason = "the macOS branch runs a host program; the Linux branch reads one file"
)]
async fn check_hypervisor(ctx: &Ctx, host: &SandboxHost, deadline: Instant) -> HostCheck {
    #[cfg(target_os = "linux")]
    {
        let _ = (ctx, deadline);
        let ok = host.fs.is_openable_read_write(Path::new("/dev/kvm"));
        HostCheck {
            name: "hypervisor".to_string(),
            ok,
            message: if ok {
                "/dev/kvm can be opened for reading and writing".to_string()
            } else {
                "cannot open /dev/kvm for reading and writing: join the kvm group, or run \
                 `setfacl -m u:$USER:rw /dev/kvm`"
                    .to_string()
            },
        }
    }
    #[cfg(target_os = "macos")]
    {
        let arch_ok = cfg!(target_arch = "aarch64");
        let supported = if arch_ok {
            let args = vec!["-n".to_string(), "kern.hv_support".to_string()];
            run_capture(ctx, host, PathBuf::from("/usr/sbin/sysctl"), args, deadline)
                .await
                .and_then(|(code, output)| {
                    if code != Some(0) {
                        return None;
                    }
                    String::from_utf8(output)
                        .ok()
                        .map(|text| text.trim() == "1")
                })
                .unwrap_or(false)
        } else {
            false
        };
        HostCheck {
            name: "hypervisor".to_string(),
            ok: arch_ok && supported,
            message: if !arch_ok {
                "libkrun needs Apple silicon on macOS".to_string()
            } else if supported {
                "the hypervisor is available".to_string()
            } else {
                "sysctl kern.hv_support is not 1".to_string()
            },
        }
    }
    #[cfg(not(any(target_os = "linux", target_os = "macos")))]
    {
        let _ = (ctx, host, deadline);
        HostCheck {
            name: "hypervisor".to_string(),
            ok: false,
            message: "libkrun runs on Linux and macOS only".to_string(),
        }
    }
}

/// The host check of the runner signature (feature 09, section 5).
#[allow(
    clippy::unused_async,
    reason = "the macOS branch runs codesign; the Linux branch has nothing to do"
)]
async fn check_runner_signed(ctx: &Ctx, host: &SandboxHost, deadline: Instant) -> HostCheck {
    #[cfg(target_os = "linux")]
    {
        let _ = (ctx, host, deadline);
        HostCheck {
            name: "runner_signed".to_string(),
            ok: true,
            message: "not applicable on Linux".to_string(),
        }
    }
    #[cfg(target_os = "macos")]
    {
        let Some(runner) = host.config.krun_runner_path.clone() else {
            return HostCheck {
                name: "runner_signed".to_string(),
                ok: false,
                message: "no runner program configured".to_string(),
            };
        };
        if !host.fs.exists(&runner) {
            return HostCheck {
                name: "runner_signed".to_string(),
                ok: false,
                message: format!("no runner at {}", runner.display()),
            };
        }
        let args = vec![
            "-d".to_string(),
            "--entitlements".to_string(),
            "-".to_string(),
            runner.display().to_string(),
        ];
        let signed = run_capture(
            ctx,
            host,
            PathBuf::from("/usr/bin/codesign"),
            args,
            deadline,
        )
        .await
        .is_some_and(|(_code, output)| {
            String::from_utf8_lossy(&output).contains("com.apple.security.hypervisor")
        });
        HostCheck {
            name: "runner_signed".to_string(),
            ok: signed,
            message: if signed {
                "the runner has the hypervisor entitlement".to_string()
            } else {
                format!(
                    "{} has no hypervisor entitlement: run `mise run sandbox:krun`",
                    runner.display()
                )
            },
        }
    }
    #[cfg(not(any(target_os = "linux", target_os = "macos")))]
    {
        let _ = (ctx, host, deadline);
        HostCheck {
            name: "runner_signed".to_string(),
            ok: false,
            message: "libkrun runs on Linux and macOS only".to_string(),
        }
    }
}

/// Runs `sandbox-krun --probe` and reads its one JSON line.
async fn probe_library(
    ctx: &Ctx,
    host: &SandboxHost,
    deadline: Instant,
) -> Result<KrunProbe, String> {
    let runner = host
        .config
        .krun_runner_path
        .clone()
        .ok_or_else(|| "no runner program configured".to_string())?;
    let mut args = vec!["--probe".to_string()];
    if let Some(library) = &host.config.libkrun_path {
        args.push("--libkrun".to_string());
        args.push(library.display().to_string());
    }
    let Some((code, output)) = run_capture(ctx, host, runner.clone(), args, deadline).await else {
        return Err(format!(
            "{} --probe did not finish before the probe deadline",
            runner.display()
        ));
    };
    if code != Some(0) {
        return Err(format!(
            "{} --probe exited with {code:?}: {}",
            runner.display(),
            String::from_utf8_lossy(&output)
        ));
    }
    String::from_utf8_lossy(&output)
        .lines()
        .find_map(|line| serde_json::from_str::<KrunProbe>(line.trim()).ok())
        .ok_or_else(|| format!("{} --probe printed no JSON line", runner.display()))
}

/// Runs one host program and reads a bounded part of its output.
///
/// `None` means that the program did not end before `deadline`, or that the host
/// could not start it.
async fn run_capture(
    ctx: &Ctx,
    host: &SandboxHost,
    program: PathBuf,
    args: Vec<String>,
    deadline: Instant,
) -> Option<(Option<i32>, Vec<u8>)> {
    let dir = probe_dir(ctx, host)?;
    let result = run_in(ctx, host, program, args, deadline, &dir).await;
    remove_dir_logged(&host.fs, &dir);
    result
}

/// Runs one host program in `dir`.
async fn run_in(
    ctx: &Ctx,
    host: &SandboxHost,
    program: PathBuf,
    args: Vec<String>,
    deadline: Instant,
    dir: &Path,
) -> Option<(Option<i32>, Vec<u8>)> {
    let remaining = deadline.saturating_duration_since(ctx.clock().now());
    if remaining.is_zero() {
        return None;
    }
    let stdout = dir.join("stdout");
    let stderr = dir.join("stderr");
    let spec = ProcessSpec {
        program,
        args,
        env: Vec::new(),
        cwd: dir.to_path_buf(),
        stdout: stdout.clone(),
        stderr: stderr.clone(),
        kill_with_parent: true,
        extra_fds: Vec::new(),
    };
    let child = host.processes.spawn(spec).ok()?;
    let timer = ctx.require_env().ok()?.timer();
    let (exit, timed_out) =
        match select(child.wait(), timer.sleep_until(ctx.clone(), deadline)).await {
            Either::Left((exit, _)) => (exit.ok(), false),
            Either::Right(_) => (None, true),
        };
    if timed_out {
        let _ = child.signal(libc::SIGKILL);
        let _ = child.wait().await;
        return None;
    }
    let mut output = host.fs.read(&stdout, 4096).unwrap_or_default();
    output.extend_from_slice(&host.fs.read(&stderr, 4096).unwrap_or_default());
    Some((exit.and_then(|exit| exit.code), output))
}

/// A new private directory for one probe program.
fn probe_dir(ctx: &Ctx, host: &SandboxHost) -> Option<PathBuf> {
    use foundation_compact::rng::Rng as _;
    let suffix = {
        let rng = ctx.rng();
        let mut rng = rng.lock().ok()?;
        format!("{:016x}", rng.next_u64())
    };
    let dir = host.config.state_dir.join(format!("probe-{suffix}"));
    host.fs.create_dir_all(&dir, 0o700).ok()?;
    Some(dir)
}
