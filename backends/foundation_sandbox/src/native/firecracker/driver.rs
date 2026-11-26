//! The Firecracker driver (feature 05).
//!
//! One `firecracker` process is one microVM. The driver starts the process,
//! configures the machine through the API client of feature 04, boots the slave
//! as PID 1 from the artifacts of feature 03, and returns a
//! [`Substrate`](crate::shared::driver::Substrate) with the vsock endpoint.
//!
//! The launch is one `#[task]` state machine. Every wait uses the injected
//! clock and timer. A failed launch leaves no process and no run directory
//! (invariant I5).
//!
//! The module is Linux only: Firecracker needs KVM and the Firecracker program.

use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex, PoisonError};
use std::time::Duration;

use foundation_compact::Instant;
use foundation_context::Ctx;
use foundation_core::valtron::{
    execute, from_future, send, task, Stream as ValtronStream, TaskStatus,
};
use futures::future::{select, Either};

use crate::native::artifacts::ArtifactStore;
use crate::native::firecracker::client::FirecrackerClient;
use crate::native::firecracker::generated::boot_source::BootSource;
use crate::native::firecracker::generated::default::InstanceInfo;
use crate::native::firecracker::generated::drives::Drive;
use crate::native::firecracker::generated::machine_config::MachineConfiguration;
use crate::native::firecracker::generated::vsock::Vsock;
use crate::shared::driver::{
    Capabilities, CreateProgress, HostCheck, HostReport, LaunchStream, SandboxDriver,
    SlaveEndpoint, StoredRecord, Substrate, SubstrateExit, SubstrateHandle,
};
use crate::shared::error::{SandboxError, SandboxResult};
use crate::shared::host::{ChildProcess, HostFs, ProcessSpec, SandboxHost};
use crate::shared::ids::{check_socket_path, DriverId, SandboxId};
use crate::shared::limits::SandboxLimits;
use crate::shared::spec::{
    FsType, GuestArch, KernelSource, NetworkMode, RootMode, RootSetup, RootSource, SandboxSpec,
};
use crate::shared::BoxFuture;

/// The lowest Firecracker version that this driver accepts.
///
/// The build script reads `ci/sandbox.toml` and sets this value. A packaged
/// build with no `ci/sandbox.toml` accepts every version: the API fields that
/// this driver sends are old and stable.
pub const MIN_FIRECRACKER_VERSION: &str =
    match option_env!("FOUNDATION_SANDBOX_MIN_FIRECRACKER_VERSION") {
        Some(version) => version,
        None => "0.0.0",
    };

/// The vsock port that the slave listens on.
pub const SLAVE_VSOCK_PORT: u32 = 1024;

/// The device name of the root image in the guest.
pub const ROOT_DEVICE: &str = "/dev/vda";

/// The largest guest context identifier that Firecracker accepts.
const MAX_GUEST_CID: u64 = (1_u64 << 31) - 1;

/// The most vCPUs that the driver gives one machine.
const MAX_VCPUS: u8 = 32;

/// One step of the API configuration.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ConfigStep {
    /// `PUT /machine-config`.
    MachineConfig,
    /// `PUT /boot-source`.
    BootSource,
    /// `PUT /drives/rootfs`.
    Drive,
    /// `PUT /vsock`.
    Vsock,
    /// `PUT /actions` with `InstanceStart`.
    Start,
}

/// Every resolved path and value of one virtual machine.
///
/// The `Prepare` state builds it; later states only read it.
#[derive(Clone, Debug)]
pub struct FcPlan {
    /// The sandbox identifier.
    pub id: SandboxId,
    /// The directory of this VM: sockets and logs.
    pub run_dir: PathBuf,
    /// The `firecracker` program.
    pub firecracker: PathBuf,
    /// The guest kernel.
    pub kernel: PathBuf,
    /// The initramfs with the slave as `/init`.
    pub initramfs: PathBuf,
    /// The root image file.
    pub rootfs: PathBuf,
    /// The file system of the root image.
    pub rootfs_type: FsType,
    /// How writes to the root are handled.
    pub root_mode: RootMode,
    /// The number of virtual CPUs.
    pub vcpus: u8,
    /// The memory in mebibytes.
    pub memory_mib: u32,
    /// The guest context identifier.
    pub guest_cid: u32,
    /// The kernel boot arguments.
    pub boot_args: String,
    /// Every size and time limit of the sandbox.
    pub limits: SandboxLimits,
}

impl FcPlan {
    /// The API socket path.
    #[must_use]
    pub fn api_socket(&self) -> PathBuf {
        self.run_dir.join("fc.sock")
    }

    /// The vsock socket path.
    #[must_use]
    pub fn vsock_socket(&self) -> PathBuf {
        self.run_dir.join("v.sock")
    }

    /// The serial console path.
    #[must_use]
    pub fn console_log(&self) -> PathBuf {
        self.run_dir.join("console.log")
    }

    /// The VMM log path.
    #[must_use]
    pub fn vmm_log(&self) -> PathBuf {
        self.run_dir.join("vmm.log")
    }

    /// The pid file path.
    #[must_use]
    pub fn pid_file(&self) -> PathBuf {
        self.run_dir.join("pid")
    }
}

/// The boot arguments of one guest.
///
/// `console=ttyS0` puts the kernel and slave messages on the serial port, thus
/// into `console.log`. `reboot=k` and `panic=1` stop the VM when the guest
/// reboots or panics. `ewe.listen=vsock:1024` is the slave listen address.
#[must_use]
pub fn boot_args(arch: GuestArch) -> String {
    let mut args = String::from("console=ttyS0 reboot=k panic=1 quiet ewe.listen=vsock:1024");
    if arch == GuestArch::X86_64 {
        args.push_str(" pci=off i8042.noaux i8042.nomux i8042.dumbkbd");
    }
    args
}

/// The API of one Firecracker process, so a test can replace it.
pub trait FirecrackerApi: Send + Sync + 'static {
    /// `GET /`: general information about the instance.
    fn describe_instance(&self) -> BoxFuture<'_, SandboxResult<InstanceInfo>>;

    /// `PUT /machine-config`.
    fn put_machine_config<'a>(
        &'a self,
        body: &'a MachineConfiguration,
    ) -> BoxFuture<'a, SandboxResult<()>>;

    /// `PUT /boot-source`.
    fn put_boot_source<'a>(&'a self, body: &'a BootSource) -> BoxFuture<'a, SandboxResult<()>>;

    /// `PUT /drives/{drive_id}`.
    fn put_drive<'a>(&'a self, body: &'a Drive) -> BoxFuture<'a, SandboxResult<()>>;

    /// `PUT /vsock`.
    fn put_vsock<'a>(&'a self, body: &'a Vsock) -> BoxFuture<'a, SandboxResult<()>>;

    /// `PUT /actions` with `InstanceStart`.
    fn instance_start(&self) -> BoxFuture<'_, SandboxResult<()>>;
}

/// The type of a factory that makes the API client of one process.
#[allow(
    clippy::type_complexity,
    reason = "the factory type is the design of feature 05, section 6.5"
)]
pub type FirecrackerApiFactory = Arc<
    dyn Fn(&Ctx, &Path, &SandboxLimits) -> SandboxResult<Arc<dyn FirecrackerApi>> + Send + Sync,
>;

impl FirecrackerApi for FirecrackerClient {
    fn describe_instance(&self) -> BoxFuture<'_, SandboxResult<InstanceInfo>> {
        Box::pin(async move { FirecrackerClient::describe_instance(self).await })
    }

    fn put_machine_config<'a>(
        &'a self,
        body: &'a MachineConfiguration,
    ) -> BoxFuture<'a, SandboxResult<()>> {
        Box::pin(async move { FirecrackerClient::put_machine_config(self, body).await })
    }

    fn put_boot_source<'a>(&'a self, body: &'a BootSource) -> BoxFuture<'a, SandboxResult<()>> {
        Box::pin(async move { FirecrackerClient::put_boot_source(self, body).await })
    }

    fn put_drive<'a>(&'a self, body: &'a Drive) -> BoxFuture<'a, SandboxResult<()>> {
        Box::pin(async move { FirecrackerClient::put_drive(self, body).await })
    }

    fn put_vsock<'a>(&'a self, body: &'a Vsock) -> BoxFuture<'a, SandboxResult<()>> {
        Box::pin(async move { FirecrackerClient::put_vsock(self, body).await })
    }

    fn instance_start(&self) -> BoxFuture<'_, SandboxResult<()>> {
        Box::pin(async move { FirecrackerClient::instance_start(self).await })
    }
}

/// The Firecracker driver.
pub struct FirecrackerDriver {
    api_factory: FirecrackerApiFactory,
    min_version: String,
    boot_args: Option<String>,
}

impl Default for FirecrackerDriver {
    fn default() -> Self {
        Self::new()
    }
}

impl std::fmt::Debug for FirecrackerDriver {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("FirecrackerDriver")
            .field("min_version", &self.min_version)
            .finish_non_exhaustive()
    }
}

impl FirecrackerDriver {
    /// A driver with the real API client.
    #[must_use]
    pub fn new() -> Self {
        Self {
            api_factory: Arc::new(|ctx, socket, limits| {
                let client = FirecrackerClient::new(ctx, socket, limits)?;
                Ok(Arc::new(client) as Arc<dyn FirecrackerApi>)
            }),
            min_version: MIN_FIRECRACKER_VERSION.to_string(),
            boot_args: None,
        }
    }

    /// A driver with a factory for the API client. Tests use it.
    #[must_use]
    pub fn with_api_factory(factory: FirecrackerApiFactory) -> Self {
        Self {
            api_factory: factory,
            min_version: MIN_FIRECRACKER_VERSION.to_string(),
            boot_args: None,
        }
    }

    /// A driver that boots the guest with `args` instead of the default line.
    ///
    /// A guest that cannot start test uses it, for example with
    /// `ewe.listen=bad`.
    #[must_use]
    pub fn with_boot_args(mut self, args: impl Into<String>) -> Self {
        self.boot_args = Some(args.into());
        self
    }

    /// The lowest accepted Firecracker version.
    #[must_use]
    pub fn min_version(&self) -> &str {
        &self.min_version
    }
}

/// The capabilities of this driver (feature section 6.6).
#[must_use]
pub fn capabilities() -> Capabilities {
    Capabilities {
        rootfs_directory: false,
        rootfs_ext4: true,
        rootfs_squashfs: true,
        rootfs_container_image: false,
        root_modes: vec![RootMode::Overlay, RootMode::ReadOnly],
        host_mounts: false,
        network_none: true,
        network_host: false,
        max_vcpus: MAX_VCPUS,
    }
}

impl SandboxDriver for FirecrackerDriver {
    fn id(&self) -> DriverId {
        DriverId::Firecracker
    }

    fn probe(&self, ctx: Ctx) -> BoxFuture<'static, HostReport> {
        let min_version = self.min_version.clone();
        Box::pin(async move {
            let capabilities = capabilities();
            let Ok(host) = SandboxHost::from_ctx(&ctx) else {
                return HostReport {
                    driver: DriverId::Firecracker,
                    usable: false,
                    checks: vec![HostCheck {
                        name: "host".to_string(),
                        ok: false,
                        message: "the sandbox host is not configured".to_string(),
                    }],
                    capabilities,
                };
            };
            let mut checks = Vec::new();

            let kvm = host.fs.is_openable_read_write(Path::new("/dev/kvm"));
            checks.push(HostCheck {
                name: "kvm".to_string(),
                ok: kvm,
                message: if kvm {
                    "/dev/kvm can be opened for reading and writing".to_string()
                } else {
                    "cannot open /dev/kvm for reading and writing: join the kvm group, or run \
                     `setfacl -m u:$USER:rw /dev/kvm`"
                        .to_string()
                },
            });

            let program = host.config.firecracker_path.clone();
            let present = program.as_ref().is_some_and(|path| host.fs.exists(path));
            let version = if present {
                probe_version(&ctx, &host).await
            } else {
                None
            };
            let firecracker_ok = present && version.is_some();
            checks.push(HostCheck {
                name: "firecracker".to_string(),
                ok: firecracker_ok,
                message: match (&program, &version) {
                    (_, Some(version)) => format!("Firecracker {version}"),
                    (Some(path), None) if present => {
                        format!("{} did not print a version", path.display())
                    }
                    (Some(path), None) => format!("{} does not exist", path.display()),
                    (None, None) => "no firecracker program on PATH".to_string(),
                },
            });

            let version_ok = version
                .as_deref()
                .is_some_and(|found| version_at_least(found, &min_version));
            checks.push(HostCheck {
                name: "firecracker_version".to_string(),
                ok: version_ok,
                message: match &version {
                    Some(found) => format!("{found} is at least {min_version}"),
                    None => format!("no version to compare with {min_version}"),
                },
            });

            let slave = host
                .config
                .slave_path
                .as_ref()
                .is_some_and(|path| host.fs.exists(path));
            checks.push(HostCheck {
                name: "slave".to_string(),
                ok: slave,
                message: match &host.config.slave_path {
                    Some(path) if slave => format!("the slave is at {}", path.display()),
                    Some(path) => format!("no slave at {}", path.display()),
                    None => "no slave program configured".to_string(),
                },
            });

            checks.push(HostCheck {
                name: "vcpu_rule".to_string(),
                ok: true,
                message: "1 vCPU or an even number".to_string(),
            });

            HostReport {
                driver: DriverId::Firecracker,
                usable: kvm && firecracker_ok && version_ok && slave,
                checks,
                capabilities,
            }
        })
    }

    fn launch(
        &self,
        ctx: Ctx,
        id: SandboxId,
        spec: SandboxSpec,
        deadline: Instant,
    ) -> LaunchStream {
        let state = FcState {
            launch: FcLaunch::Prepare {
                ctx,
                id,
                spec,
                deadline,
                boot_args: self.boot_args.clone(),
            },
            guard: LaunchGuard::new(),
            api_factory: Arc::clone(&self.api_factory),
        };
        match execute(fc_launch::new(state), None) {
            Ok(iterator) => Box::new(iterator),
            Err(error) => {
                let error: SandboxResult<Substrate> = Err(SandboxError::Io {
                    op: "launch firecracker".to_string(),
                    message: error.to_string(),
                }
                .into());
                Box::new(std::iter::once(ValtronStream::Next(error)))
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

/// One step of the launch.
pub enum FcLaunch {
    /// Resolve the artifacts and make the run directory.
    Prepare {
        /// The injected context.
        ctx: Ctx,
        /// The sandbox identifier.
        id: SandboxId,
        /// The caller's spec.
        spec: SandboxSpec,
        /// The one deadline of the create.
        deadline: Instant,
        /// Boot arguments that replace the default line, when a test sets them.
        boot_args: Option<String>,
    },
    /// Start the `firecracker` process.
    Spawn {
        /// The injected context.
        ctx: Ctx,
        /// The resolved plan.
        plan: FcPlan,
        /// The one deadline of the create.
        deadline: Instant,
    },
    /// Wait for the API socket to answer.
    AwaitApi {
        /// The injected context.
        ctx: Ctx,
        /// The resolved plan.
        plan: FcPlan,
        /// The VMM process.
        vmm: Arc<dyn ChildProcess>,
        /// The API client.
        api: Arc<dyn FirecrackerApi>,
        /// The one deadline of the create.
        deadline: Instant,
        /// How many API attempts already failed.
        attempt: u32,
    },
    /// Send the next API call.
    Configure {
        /// The injected context.
        ctx: Ctx,
        /// The resolved plan.
        plan: FcPlan,
        /// The VMM process.
        vmm: Arc<dyn ChildProcess>,
        /// The API client.
        api: Arc<dyn FirecrackerApi>,
        /// The next call.
        step: ConfigStep,
    },
    /// Give the ready substrate on the next step, after the boot progress.
    EmitReady {
        /// The ready substrate.
        substrate: Substrate,
    },
    /// The launch is done.
    Done,
}

/// The state of the launch task: the step, the clean-up guard and the factory.
pub struct FcState {
    /// The current step.
    pub launch: FcLaunch,
    /// The guard that kills the process when the task ends before the result.
    pub guard: Arc<LaunchGuard>,
    /// The factory that makes the API client of this process.
    pub api_factory: FirecrackerApiFactory,
}

type Status = TaskStatus<SandboxResult<Substrate>, CreateProgress>;

/// One transition of the launch state machine.
#[task]
pub async fn fc_launch(
    ctx: Ctx,
    state: FcState,
) -> Option<(
    FcState,
    TaskStatus<SandboxResult<Substrate>, CreateProgress>,
)> {
    // The executor gives its own context. Every clock and timer read must use
    // the context that the driver injected, which the state holds.
    drop(ctx);
    let FcState {
        launch,
        guard,
        api_factory,
    } = state;
    let next = match launch {
        FcLaunch::Prepare {
            ctx,
            id,
            spec,
            deadline,
            boot_args,
        } => prepare(ctx, guard, api_factory, id, spec, deadline, boot_args).await,
        FcLaunch::Spawn {
            ctx,
            plan,
            deadline,
        } => spawn(ctx, guard, api_factory, plan, deadline).await,
        FcLaunch::AwaitApi {
            ctx,
            plan,
            vmm,
            api,
            deadline,
            attempt,
        } => await_api(ctx, guard, api_factory, plan, vmm, api, deadline, attempt).await,
        FcLaunch::Configure {
            ctx,
            plan,
            vmm,
            api,
            step,
        } => configure(ctx, guard, api_factory, plan, vmm, api, step).await,
        FcLaunch::EmitReady { substrate } => (
            FcState {
                launch: FcLaunch::Done,
                guard,
                api_factory,
            },
            TaskStatus::Ready(Ok(substrate)),
        ),
        FcLaunch::Done => return None,
    };
    Some(next)
}

#[allow(
    clippy::too_many_arguments,
    reason = "one transition carries the state fields that the design names"
)]
async fn prepare(
    ctx: Ctx,
    guard: Arc<LaunchGuard>,
    api_factory: FirecrackerApiFactory,
    id: SandboxId,
    spec: SandboxSpec,
    deadline: Instant,
    boot_args: Option<String>,
) -> (FcState, Status) {
    let timer = match ctx.require_env() {
        Ok(env) => env.timer(),
        Err(_) => {
            return fail(guard, api_factory, SandboxError::HostNotConfigured, &ctx).await;
        }
    };
    let work = prepare_plan(&ctx, &id, &spec, boot_args.as_deref());
    let outcome = match select(Box::pin(work), timer.sleep_until(ctx.clone(), deadline)).await {
        Either::Left((result, _)) => Some(result),
        Either::Right(_) => None,
    };
    match outcome {
        Some(Ok(plan)) => {
            if let Ok(host) = SandboxHost::from_ctx(&ctx) {
                guard.arm_path(host.fs.clone(), plan.run_dir.clone());
            }
            (
                FcState {
                    launch: FcLaunch::Spawn {
                        ctx,
                        plan,
                        deadline,
                    },
                    guard,
                    api_factory,
                },
                TaskStatus::Pending(CreateProgress::Preparing),
            )
        }
        Some(Err(error)) => fail(guard, api_factory, error.current_context().clone(), &ctx).await,
        None => {
            fail(
                guard,
                api_factory,
                SandboxError::BootTimeout {
                    stage: "artifacts".to_string(),
                    log_tail: Vec::new(),
                },
                &ctx,
            )
            .await
        }
    }
}

#[allow(
    clippy::unused_async,
    reason = "the #[task] transition is async; the spawn itself is a short sync call"
)]
async fn spawn(
    ctx: Ctx,
    guard: Arc<LaunchGuard>,
    api_factory: FirecrackerApiFactory,
    plan: FcPlan,
    deadline: Instant,
) -> (FcState, Status) {
    let host = match SandboxHost::from_ctx(&ctx) {
        Ok(host) => host,
        Err(error) => {
            return fail(guard, api_factory, error.current_context().clone(), &ctx).await;
        }
    };
    let spec = ProcessSpec {
        program: plan.firecracker.clone(),
        args: vec![
            "--api-sock".to_string(),
            plan.api_socket().display().to_string(),
            "--id".to_string(),
            plan.id.to_string(),
            "--log-path".to_string(),
            plan.vmm_log().display().to_string(),
            "--level".to_string(),
            "Warning".to_string(),
        ],
        env: Vec::new(),
        cwd: plan.run_dir.clone(),
        stdout: plan.console_log(),
        stderr: plan.console_log(),
        kill_with_parent: true,
        extra_fds: Vec::new(),
    };
    let vmm = match host.processes.spawn(spec) {
        Ok(vmm) => vmm,
        Err(error) => {
            return fail(guard, api_factory, error.current_context().clone(), &ctx).await;
        }
    };
    let vmm: Arc<dyn ChildProcess> = Arc::from(vmm);
    guard.arm_vmm(Arc::clone(&vmm));
    if let Err(error) = host
        .fs
        .write(&plan.pid_file(), vmm.pid().to_string().as_bytes())
    {
        return fail(guard, api_factory, error.current_context().clone(), &ctx).await;
    }
    let api = match (api_factory)(&ctx, &plan.api_socket(), &plan.limits) {
        Ok(api) => api,
        Err(error) => {
            return fail(guard, api_factory, error.current_context().clone(), &ctx).await;
        }
    };
    (
        FcState {
            launch: FcLaunch::AwaitApi {
                ctx,
                plan,
                vmm,
                api,
                deadline,
                attempt: 0,
            },
            guard,
            api_factory,
        },
        TaskStatus::Pending(CreateProgress::SubstrateStarted),
    )
}

/// One candidate of the API wait.
enum ApiWait {
    /// The API answered.
    Describe(SandboxResult<InstanceInfo>),
    /// The VMM process ended.
    Exit(Option<crate::shared::host::ProcessExit>),
    /// The launch deadline passed.
    Deadline,
}

#[allow(
    clippy::too_many_arguments,
    clippy::too_many_lines,
    reason = "one transition carries the state fields and the three racing waits that the design names"
)]
async fn await_api(
    ctx: Ctx,
    guard: Arc<LaunchGuard>,
    api_factory: FirecrackerApiFactory,
    plan: FcPlan,
    vmm: Arc<dyn ChildProcess>,
    api: Arc<dyn FirecrackerApi>,
    deadline: Instant,
    attempt: u32,
) -> (FcState, Status) {
    if let Ok(Some(exit)) = vmm.try_wait() {
        let log_tail = log_tail(&ctx, &plan);
        let error = SandboxError::SubstrateExited {
            code: exit.code,
            log_tail,
        };
        return fail(guard, api_factory, error, &ctx).await;
    }
    if ctx.clock().now() >= deadline {
        let error = SandboxError::BootTimeout {
            stage: "api".to_string(),
            log_tail: log_tail(&ctx, &plan),
        };
        return fail(guard, api_factory, error, &ctx).await;
    }
    let timer = match ctx.require_env() {
        Ok(env) => env.timer(),
        Err(_) => {
            return fail(guard, api_factory, SandboxError::HostNotConfigured, &ctx).await;
        }
    };
    let describe: std::pin::Pin<Box<dyn std::future::Future<Output = ApiWait> + Send>> = {
        let api = Arc::clone(&api);
        Box::pin(async move { ApiWait::Describe(api.describe_instance().await) })
    };
    let exit: std::pin::Pin<Box<dyn std::future::Future<Output = ApiWait> + Send>> = {
        let vmm = Arc::clone(&vmm);
        Box::pin(async move { ApiWait::Exit(vmm.wait().await.ok()) })
    };
    let deadline_wait: std::pin::Pin<Box<dyn std::future::Future<Output = ApiWait> + Send>> = {
        let ctx = ctx.clone();
        Box::pin(async move {
            timer.sleep_until(ctx, deadline).await;
            ApiWait::Deadline
        })
    };
    let (candidate, _index, _rest) =
        futures::future::select_all(vec![describe, exit, deadline_wait]).await;
    match candidate {
        ApiWait::Describe(Ok(_)) => (
            FcState {
                launch: FcLaunch::Configure {
                    ctx,
                    plan,
                    vmm,
                    api,
                    step: ConfigStep::MachineConfig,
                },
                guard,
                api_factory,
            },
            TaskStatus::Ignore,
        ),
        ApiWait::Describe(Err(_)) => {
            let backoff = retry_backoff(
                attempt,
                Duration::from_millis(5),
                Duration::from_millis(100),
            );
            let timer = match ctx.require_env() {
                Ok(env) => env.timer(),
                Err(_) => {
                    return fail(guard, api_factory, SandboxError::HostNotConfigured, &ctx).await;
                }
            };
            let wait_until = ctx.clock().now() + backoff;
            timer.sleep_until(ctx.clone(), wait_until).await;
            (
                FcState {
                    launch: FcLaunch::AwaitApi {
                        ctx,
                        plan,
                        vmm,
                        api,
                        deadline,
                        attempt: attempt.saturating_add(1),
                    },
                    guard,
                    api_factory,
                },
                TaskStatus::Ignore,
            )
        }
        ApiWait::Exit(exit) => {
            let exit = exit.unwrap_or(crate::shared::host::ProcessExit {
                code: None,
                signal: None,
            });
            let error = SandboxError::SubstrateExited {
                code: exit.code,
                log_tail: log_tail(&ctx, &plan),
            };
            fail(guard, api_factory, error, &ctx).await
        }
        ApiWait::Deadline => {
            let error = SandboxError::BootTimeout {
                stage: "api".to_string(),
                log_tail: log_tail(&ctx, &plan),
            };
            fail(guard, api_factory, error, &ctx).await
        }
    }
}

async fn configure(
    ctx: Ctx,
    guard: Arc<LaunchGuard>,
    api_factory: FirecrackerApiFactory,
    plan: FcPlan,
    vmm: Arc<dyn ChildProcess>,
    api: Arc<dyn FirecrackerApi>,
    step: ConfigStep,
) -> (FcState, Status) {
    if let Ok(Some(exit)) = vmm.try_wait() {
        let log_tail = log_tail(&ctx, &plan);
        let error = SandboxError::SubstrateExited {
            code: exit.code,
            log_tail,
        };
        return fail(guard, api_factory, error, &ctx).await;
    }
    if step == ConfigStep::Start {
        match api.instance_start().await {
            Ok(()) => match make_substrate(&ctx, &plan, Arc::clone(&vmm)) {
                Ok(substrate) => {
                    guard.disarm();
                    (
                        FcState {
                            launch: FcLaunch::EmitReady { substrate },
                            guard,
                            api_factory,
                        },
                        TaskStatus::Pending(CreateProgress::Booting),
                    )
                }
                Err(error) => fail(guard, api_factory, error.current_context().clone(), &ctx).await,
            },
            Err(error) => {
                let error = configure_error(error.current_context().clone(), &plan, &ctx);
                fail(guard, api_factory, error, &ctx).await
            }
        }
    } else {
        let result = match step {
            ConfigStep::MachineConfig => api.put_machine_config(&machine_config(&plan)).await,
            ConfigStep::BootSource => api.put_boot_source(&boot_source(&plan)).await,
            ConfigStep::Drive => api.put_drive(&drive(&plan)).await,
            ConfigStep::Vsock => api.put_vsock(&vsock(&plan)).await,
            ConfigStep::Start => unreachable!("handled above"),
        };
        match result {
            Ok(()) => {
                let next_step = match step {
                    ConfigStep::MachineConfig => ConfigStep::BootSource,
                    ConfigStep::BootSource => ConfigStep::Drive,
                    ConfigStep::Drive => ConfigStep::Vsock,
                    ConfigStep::Vsock => ConfigStep::Start,
                    ConfigStep::Start => unreachable!("handled above"),
                };
                let status = if step == ConfigStep::Vsock {
                    TaskStatus::Pending(CreateProgress::SubstrateConfigured)
                } else {
                    TaskStatus::Ignore
                };
                (
                    FcState {
                        launch: FcLaunch::Configure {
                            ctx,
                            plan,
                            vmm,
                            api,
                            step: next_step,
                        },
                        guard,
                        api_factory,
                    },
                    status,
                )
            }
            Err(error) => {
                let error = configure_error(error.current_context().clone(), &plan, &ctx);
                fail(guard, api_factory, error, &ctx).await
            }
        }
    }
}

/// Maps an API time-out to the boot-stage error of the fault table.
fn configure_error(error: SandboxError, plan: &FcPlan, ctx: &Ctx) -> SandboxError {
    match error {
        SandboxError::ApiTimeout { .. } => SandboxError::BootTimeout {
            stage: "api".to_string(),
            log_tail: log_tail(ctx, plan),
        },
        other => other,
    }
}

/// Fails the launch: kills the process, waits for its exit, removes the run
/// directory, and emits the one error result.
async fn fail(
    guard: Arc<LaunchGuard>,
    api_factory: FirecrackerApiFactory,
    error: SandboxError,
    ctx: &Ctx,
) -> (FcState, Status) {
    let _ = ctx;
    if let Some(parts) = guard.take() {
        if let Some(vmm) = parts.vmm {
            let _ = vmm.signal(libc::SIGKILL);
            let _ = vmm.wait().await;
        }
        let _ = parts.fs.remove_dir_all(&parts.run_dir);
    }
    (
        FcState {
            launch: FcLaunch::Done,
            guard,
            api_factory,
        },
        TaskStatus::Ready(Err(error.into())),
    )
}

/// Resolves every file of one VM and makes its run directory.
async fn prepare_plan(
    ctx: &Ctx,
    id: &SandboxId,
    spec: &SandboxSpec,
    boot_args_override: Option<&str>,
) -> SandboxResult<FcPlan> {
    check_spec(spec)?;
    let host = SandboxHost::from_ctx(ctx)?;
    let (rootfs, rootfs_type) = match &spec.rootfs {
        crate::shared::spec::RootFs::Ext4Image(path) => (path.clone(), FsType::Ext4),
        crate::shared::spec::RootFs::SquashfsImage(path) => (path.clone(), FsType::Squashfs),
        _ => return Err(SandboxError::UnsupportedRootFs.into()),
    };
    if !host.fs.exists(&rootfs) {
        return Err(SandboxError::BinaryNotFound {
            name: rootfs.display().to_string(),
        }
        .into());
    }
    let firecracker = resolve_program(
        &host,
        host.config.firecracker_path.as_deref(),
        "firecracker",
    )?;
    let slave = resolve_program(&host, host.config.slave_path.as_deref(), "agent-slave")?;
    let store = ArtifactStore::new(ctx.clone(), host.fs.clone(), host.config.cache_dir.clone());
    let kernel = match &spec.kernel {
        Some(KernelSource::Path(path)) => {
            if !host.fs.exists(path) {
                return Err(SandboxError::BinaryNotFound {
                    name: path.display().to_string(),
                }
                .into());
            }
            path.clone()
        }
        _ => store.firecracker_kernel(GuestArch::host()).await?,
    };
    let initramfs = store.slave_initramfs(&slave).await?;
    let run_dir = host.config.state_dir.join(id.as_str());
    host.fs.create_dir_all(&run_dir, 0o700)?;
    let prepared = (|| -> SandboxResult<()> {
        check_socket_path(&run_dir.join("v.sock"))?;
        check_socket_path(&run_dir.join("fc.sock"))?;
        host.fs.write(&run_dir.join("vmm.log"), b"")?;
        Ok(())
    })();
    if let Err(error) = prepared {
        let _ = host.fs.remove_dir_all(&run_dir);
        return Err(error);
    }
    let guest_cid = {
        use foundation_compact::rng::Rng as _;
        let rng = ctx.rng();
        let mut rng = rng.lock().expect("the Ctx RNG lock is not poisoned");
        let value = 3 + rng.next_u64() % (MAX_GUEST_CID - 2);
        u32::try_from(value).unwrap_or(3)
    };
    Ok(FcPlan {
        id: id.clone(),
        run_dir,
        firecracker,
        kernel,
        initramfs,
        rootfs,
        rootfs_type,
        root_mode: spec.root_mode,
        vcpus: spec.vcpus,
        memory_mib: spec.memory_mib,
        guest_cid,
        boot_args: boot_args_override
            .map_or_else(|| boot_args(GuestArch::host()), ToString::to_string),
        limits: spec.limits.clone(),
    })
}

/// Rejects a spec that Firecracker cannot run, before any process starts.
fn check_spec(spec: &SandboxSpec) -> SandboxResult<()> {
    if spec.network == NetworkMode::Host {
        return Err(SandboxError::UnsupportedNetwork.into());
    }
    if !spec.mounts.is_empty() {
        return Err(SandboxError::UnsupportedMount.into());
    }
    if spec.vcpus > MAX_VCPUS || (spec.vcpus > 1 && spec.vcpus % 2 == 1) {
        return Err(SandboxError::InvalidSpec {
            field: "vcpus".to_string(),
            reason: "Firecracker accepts 1 vCPU or an even number, at most 32".to_string(),
        }
        .into());
    }
    Ok(())
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

/// Builds the substrate of a started VM.
fn make_substrate(
    ctx: &Ctx,
    plan: &FcPlan,
    vmm: Arc<dyn ChildProcess>,
) -> SandboxResult<Substrate> {
    let host = SandboxHost::from_ctx(ctx)?;
    let handle = FcHandle::new(ctx.clone(), vmm, host.fs.clone(), plan.run_dir.clone());
    Ok(Substrate {
        endpoint: SlaveEndpoint::FirecrackerVsock {
            uds: plan.vsock_socket(),
            port: SLAVE_VSOCK_PORT,
        },
        root: RootSetup {
            source: RootSource::BlockDevice {
                device: ROOT_DEVICE.to_string(),
                fstype: plan.rootfs_type,
            },
            mode: plan.root_mode,
        },
        mounts: Vec::new(),
        handle: Box::new(handle),
        calls: None,
        slave: None,
    })
}

/// The machine configuration body.
#[must_use]
pub fn machine_config(plan: &FcPlan) -> MachineConfiguration {
    MachineConfiguration {
        vcpu_count: i64::from(plan.vcpus),
        mem_size_mib: i64::from(plan.memory_mib),
        smt: Some(false),
        ..MachineConfiguration::default()
    }
}

/// The boot source body.
#[must_use]
pub fn boot_source(plan: &FcPlan) -> BootSource {
    BootSource {
        kernel_image_path: plan.kernel.display().to_string(),
        initrd_path: Some(plan.initramfs.display().to_string()),
        boot_args: Some(plan.boot_args.clone()),
    }
}

/// The root drive body.
#[must_use]
pub fn drive(plan: &FcPlan) -> Drive {
    Drive {
        drive_id: "rootfs".to_string(),
        path_on_host: Some(plan.rootfs.display().to_string()),
        is_root_device: false,
        is_read_only: Some(true),
        ..Drive::default()
    }
}

/// The vsock body.
#[must_use]
pub fn vsock(plan: &FcPlan) -> Vsock {
    Vsock {
        guest_cid: i64::from(plan.guest_cid),
        uds_path: plan.vsock_socket().display().to_string(),
        ..Vsock::default()
    }
}

/// The back-off of one API attempt: `min(base * 2^attempt, cap)`.
fn retry_backoff(attempt: u32, base: Duration, cap: Duration) -> Duration {
    let factor = 1_u32.checked_shl(attempt.min(16)).unwrap_or(u32::MAX);
    base.saturating_mul(factor).min(cap)
}

/// The log tail of one plan.
fn log_tail(ctx: &Ctx, plan: &FcPlan) -> Vec<u8> {
    match SandboxHost::from_ctx(ctx) {
        Ok(host) => read_log_tail(&host.fs, &plan.run_dir, plan.limits.max_error_message),
        Err(_) => Vec::new(),
    }
}

/// Runs `firecracker --version` and gives the version.
async fn probe_version(ctx: &Ctx, host: &SandboxHost) -> Option<String> {
    let program = host.config.firecracker_path.clone()?;
    let suffix = {
        use foundation_compact::rng::Rng as _;
        let rng = ctx.rng();
        let mut rng = rng.lock().ok()?;
        format!("{:016x}", rng.next_u64())
    };
    let dir = host.config.state_dir.join(format!("probe-{suffix}"));
    host.fs.create_dir_all(&dir, 0o700).ok()?;
    let stdout = dir.join("version.out");
    let stderr = dir.join("version.err");
    let spec = ProcessSpec {
        program,
        args: vec!["--version".to_string()],
        env: Vec::new(),
        cwd: dir.clone(),
        stdout: stdout.clone(),
        stderr: stderr.clone(),
        kill_with_parent: true,
        extra_fds: Vec::new(),
    };
    let child = host.processes.spawn(spec).ok()?;
    let timer = ctx.require_env().ok()?.timer();
    let deadline = ctx.clock().now() + SandboxLimits::default().rpc_timeout;
    let timed_out = match select(child.wait(), timer.sleep_until(ctx.clone(), deadline)).await {
        Either::Left((_exit, _)) => false,
        Either::Right(_) => true,
    };
    if timed_out {
        let _ = child.signal(libc::SIGKILL);
        let _ = child.wait().await;
        let _ = host.fs.remove_dir_all(&dir);
        return None;
    }
    let mut output = host.fs.read(&stdout, 4096).unwrap_or_default();
    output.extend_from_slice(&host.fs.read(&stderr, 4096).unwrap_or_default());
    let _ = host.fs.remove_dir_all(&dir);
    parse_version(&output)
}

/// Compares two dotted version strings.
#[must_use]
pub fn version_at_least(actual: &str, minimum: &str) -> bool {
    let parse = |version: &str| -> Option<(u64, u64, u64)> {
        let text = version.trim().trim_start_matches('v');
        let mut parts = text.split('.');
        let major = parts.next()?.parse().ok()?;
        let minor = parts.next()?.parse().ok()?;
        let patch = parts
            .next()
            .and_then(|part| part.split(['-', '+']).next())
            .unwrap_or("0")
            .parse()
            .ok()?;
        Some((major, minor, patch))
    };
    match (parse(actual), parse(minimum)) {
        (Some(actual), Some(minimum)) => actual >= minimum,
        _ => false,
    }
}

/// The version string that a `firecracker --version` line carries.
#[must_use]
pub fn parse_version(output: &[u8]) -> Option<String> {
    let text = String::from_utf8_lossy(output);
    text.split_whitespace()
        .map(|token| token.trim_start_matches('v'))
        .find(|token| {
            let mut parts = token.split('.');
            parts.next().is_some_and(|part| part.parse::<u64>().is_ok())
                && parts.next().is_some_and(|part| part.parse::<u64>().is_ok())
                && parts.next().is_some_and(|part| part.parse::<u64>().is_ok())
        })
        .map(ToString::to_string)
}

/// The guard that kills the VMM and removes the run directory when a launch
/// ends before its result.
pub struct LaunchGuard {
    inner: Mutex<Option<GuardParts>>,
}

struct GuardParts {
    fs: Arc<dyn HostFs>,
    vmm: Option<Arc<dyn ChildProcess>>,
    run_dir: PathBuf,
}

impl LaunchGuard {
    /// An unarmed guard.
    #[must_use]
    pub fn new() -> Arc<Self> {
        Arc::new(Self {
            inner: Mutex::new(None),
        })
    }

    /// Records the run directory, before any process exists.
    fn arm_path(&self, fs: Arc<dyn HostFs>, run_dir: PathBuf) {
        *self.inner.lock().unwrap_or_else(PoisonError::into_inner) = Some(GuardParts {
            fs,
            vmm: None,
            run_dir,
        });
    }

    /// Records the started process.
    fn arm_vmm(&self, vmm: Arc<dyn ChildProcess>) {
        let mut inner = self.inner.lock().unwrap_or_else(PoisonError::into_inner);
        if let Some(parts) = inner.as_mut() {
            parts.vmm = Some(vmm);
        }
    }

    fn disarm(&self) {
        let _ = self
            .inner
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .take();
    }

    fn take(&self) -> Option<GuardParts> {
        self.inner
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .take()
    }
}

impl Drop for LaunchGuard {
    fn drop(&mut self) {
        let parts = self
            .inner
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .take();
        let Some(parts) = parts else {
            return;
        };
        let GuardParts { fs, vmm, run_dir } = parts;
        if let Err(error) = send(from_future(async move {
            if let Some(vmm) = vmm {
                let _ = vmm.signal(libc::SIGKILL);
                let _ = vmm.wait().await;
            }
            let _ = fs.remove_dir_all(&run_dir);
        })) {
            tracing::warn!(error = %error, "could not start the launch clean-up task");
        }
    }
}

/// The handle of one Firecracker process.
pub struct FcHandle {
    ctx: Ctx,
    vmm: Arc<dyn ChildProcess>,
    fs: Arc<dyn HostFs>,
    run_dir: PathBuf,
    forced: AtomicBool,
}

impl FcHandle {
    /// The handle of one started VMM.
    #[must_use]
    pub fn new(
        ctx: Ctx,
        vmm: Arc<dyn ChildProcess>,
        fs: Arc<dyn HostFs>,
        run_dir: PathBuf,
    ) -> Self {
        Self {
            ctx,
            vmm,
            fs,
            run_dir,
            forced: AtomicBool::new(false),
        }
    }

    fn exit(&self, exit: crate::shared::host::ProcessExit) -> SubstrateExit {
        SubstrateExit {
            code: exit.code,
            signal: exit.signal,
            forced: self.forced.load(Ordering::SeqCst),
        }
    }

    async fn wait_with_timeout(
        &self,
        timeout: Duration,
    ) -> Option<crate::shared::host::ProcessExit> {
        let timer = self.ctx.require_env().ok()?.timer();
        let deadline = self.ctx.clock().now() + timeout;
        match select(
            self.vmm.wait(),
            timer.sleep_until(self.ctx.clone(), deadline),
        )
        .await
        {
            Either::Left((exit, _)) => exit.ok(),
            Either::Right(_) => None,
        }
    }
}

impl SubstrateHandle for FcHandle {
    fn exited(&self) -> BoxFuture<'_, SubstrateExit> {
        Box::pin(async move {
            match self.vmm.wait().await {
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
            if let Some(exit) = self.vmm.try_wait()? {
                return Ok(self.exit(exit));
            }
            if let Some(exit) = self.wait_with_timeout(grace).await {
                return Ok(self.exit(exit));
            }
            self.forced.store(true, Ordering::SeqCst);
            self.vmm.signal(libc::SIGTERM)?;
            if let Some(exit) = self.wait_with_timeout(Duration::from_secs(1)).await {
                return Ok(self.exit(exit));
            }
            self.vmm.signal(libc::SIGKILL)?;
            let exit = self.vmm.wait().await?;
            Ok(self.exit(exit))
        })
    }

    fn kill(&self) -> BoxFuture<'_, SandboxResult<SubstrateExit>> {
        Box::pin(async move {
            if let Some(exit) = self.vmm.try_wait()? {
                return Ok(self.exit(exit));
            }
            self.forced.store(true, Ordering::SeqCst);
            self.vmm.signal(libc::SIGKILL)?;
            let exit = self.vmm.wait().await?;
            Ok(self.exit(exit))
        })
    }

    fn log_tail(&self, max: usize) -> BoxFuture<'_, Vec<u8>> {
        let bytes = read_log_tail(&self.fs, &self.run_dir, max);
        Box::pin(async move { bytes })
    }

    fn cleanup(&self) -> BoxFuture<'_, SandboxResult<()>> {
        Box::pin(async move {
            if self.vmm.try_wait()?.is_none() {
                let _ = self.kill().await;
            }
            if self.fs.exists(&self.run_dir) {
                self.fs.remove_dir_all(&self.run_dir)?;
            }
            Ok(())
        })
    }

    fn exit_error(&self, exit: &SubstrateExit, log_tail: Vec<u8>) -> SandboxError {
        SandboxError::SubstrateExited {
            code: exit.code,
            log_tail,
        }
    }
}

/// The last `max / 2` bytes of `console.log` and `vmm.log`, joined with a
/// separator line.
#[must_use]
pub fn read_log_tail(fs: &Arc<dyn HostFs>, run_dir: &Path, max: usize) -> Vec<u8> {
    let half = max / 2;
    let mut bytes = fs
        .tail(&run_dir.join("console.log"), half)
        .unwrap_or_default();
    let vmm = fs.tail(&run_dir.join("vmm.log"), half).unwrap_or_default();
    if !vmm.is_empty() {
        bytes.extend_from_slice(b"\n--- vmm.log ---\n");
        bytes.extend_from_slice(&vmm);
    }
    bytes
}

/// Removes each directory of `state_dir` whose `pid` file names a process that
/// does not exist, or that has no `pid` file and is older than `boot_timeout`.
///
/// A control plane that crashes with a record store in memory cannot call
/// `attach`, so its run directories stay. `agent-sandbox` calls this before it
/// serves.
///
/// # Errors
/// The error of the host file system.
pub fn sweep_stale_run_dirs(ctx: &Ctx, limits: &SandboxLimits) -> SandboxResult<()> {
    let host = SandboxHost::from_ctx(ctx)?;
    let now = ctx.clock().wall_now();
    let directories = host.fs.read_dir(&host.config.state_dir)?;
    for directory in directories {
        let pid_file = directory.join("pid");
        let pid = host
            .fs
            .read(&pid_file, 32)
            .ok()
            .and_then(|bytes| String::from_utf8(bytes).ok())
            .and_then(|text| text.trim().parse::<u32>().ok());
        let stale = match pid {
            Some(pid) => !host.processes.is_running(pid),
            None => host.fs.modified(&directory).is_some_and(|modified| {
                now.duration_since(modified)
                    .is_ok_and(|age| age > limits.boot_timeout)
            }),
        };
        if stale {
            host.fs.remove_dir_all(&directory)?;
        }
    }
    Ok(())
}
