//! The Docker driver (feature 10).
//!
//! One sandbox is one Docker container. The driver starts it from any image,
//! puts `agent-slave` into it, and gives the control plane the endpoint of the
//! slave (section 6.3). A container is a weaker boundary than a micro virtual
//! machine (VM); feature 11 adds VMs inside the container.
//!
//! The launch is one `#[task]` state machine with the states `Prepare`, `Pull`,
//! `Create`, `Inject`, `Start`, `Sidecar` and `Done`. A failed launch removes
//! the container and the run directory (invariant I5).

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex, PoisonError};
use std::time::Duration;

use foundation_compact::Instant;
use foundation_context::Ctx;
use foundation_core::valtron::{
    execute, from_future, send, task, Stream as ValtronStream, TaskStatus,
};
use foundation_deployment_docker::client::body::ProgressStream;
use foundation_deployment_docker::client::ContainerCreateBody;
use foundation_deployment_docker::error::DockerError;
use foundation_deployment_docker::generated::create::{ContainerCreateResponse, CreateImageInfo};
use foundation_deployment_docker::generated::json::{
    ContainerConfig, ContainerInspectResponse, HostConfig,
};
use futures::future::{select, Either};
use serde_json::{json, Map, Value};

use crate::native::docker::api::{
    create_body, is_not_found, map_error, slave_tar, DockerApi, LABEL_SANDBOX_DRIVER,
    LABEL_SANDBOX_ID,
};
use crate::native::docker::control::{self, exit_of, read_log_tail, DockerContainerControl};
use crate::shared::container::{
    ContainerControl, ContainerHandle, ContainerId, ContainerInfo, ContainerResources,
    ContainerStatus,
};
use crate::shared::driver::{
    Capabilities, CreateProgress, HostCheck, HostReport, LaunchStream, SandboxDriver,
    SlaveEndpoint, StoredRecord, Substrate, SubstrateExit, SubstrateHandle,
};
use crate::shared::error::{SandboxError, SandboxResult};
use crate::shared::host::{HostFs, SandboxHost};
use crate::shared::ids::{DriverId, SandboxId};
use crate::shared::limits::SandboxLimits;
use crate::shared::spec::{
    GuestArch, NetworkMode, RootFs, RootMode, RootSetup, RootSource, SandboxSpec,
};
use crate::shared::BoxFuture;

/// The port that the slave listens on with the Transmission Control Protocol
/// (TCP) link.
pub const SLAVE_PORT: u16 = 1024;

/// The path of the slave in the container.
pub const SLAVE_FILE: &str = "/.ewe/agent-slave";

/// The path of the slave socket in the container, under the bind mount.
pub const SLAVE_SOCKET: &str = "/.ewe/run/slave.sock";

/// How the control plane reaches the slave (section 6.2).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum DockerLink {
    /// A Unix domain socket (UDS) under the run directory, bind-mounted into
    /// the container. It needs a Linux Docker host.
    Unix,
    /// A published TCP port. Docker Desktop on macOS runs the containers in a
    /// VM, and a UDS does not cross its file sharing.
    Tcp,
}

/// Where the slave process runs in the container (section 6.3).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SlaveMode {
    /// The slave is the container's main process.
    Main,
    /// The image's main process keeps its own entry point, and the slave runs
    /// beside it.
    Sidecar,
}

/// The configuration of the Docker driver (section 6.2).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct DockerDriverConfig {
    /// How the control plane reaches the slave.
    ///
    /// Default: [`DockerLink::Unix`] on Linux, [`DockerLink::Tcp`] on macOS.
    pub link: DockerLink,
    /// Where the slave runs in the container. Default [`SlaveMode::Main`].
    pub slave_mode: SlaveMode,
    /// The address that the control plane dials, with the TCP link. Default
    /// `127.0.0.1`.
    pub publish_address: String,
    /// The deadline of one image pull.
    ///
    /// Default 10 minutes: the default pull deadline of the containerd CRI is
    /// 5 minutes (`image-pull-progress-timeout`); a full pull gets twice that.
    pub pull_timeout: Duration,
    /// The most processes in the container.
    ///
    /// Default 4096: the default of Podman is 2048; a build tool chain needs
    /// more.
    pub pids_limit: i64,
}

impl Default for DockerDriverConfig {
    fn default() -> Self {
        Self {
            link: if cfg!(target_os = "linux") {
                DockerLink::Unix
            } else {
                DockerLink::Tcp
            },
            slave_mode: SlaveMode::Main,
            publish_address: "127.0.0.1".to_string(),
            pull_timeout: Duration::from_secs(600),
            pids_limit: 4096,
        }
    }
}

impl DockerDriverConfig {
    /// Replaces [`Self::link`].
    #[must_use]
    pub fn with_link(mut self, link: DockerLink) -> Self {
        self.link = link;
        self
    }

    /// Replaces [`Self::slave_mode`].
    #[must_use]
    pub fn with_slave_mode(mut self, mode: SlaveMode) -> Self {
        self.slave_mode = mode;
        self
    }

    /// Replaces [`Self::publish_address`].
    #[must_use]
    pub fn with_publish_address(mut self, address: impl Into<String>) -> Self {
        self.publish_address = address.into();
        self
    }

    /// Replaces [`Self::pull_timeout`].
    #[must_use]
    pub fn with_pull_timeout(mut self, timeout: Duration) -> Self {
        self.pull_timeout = timeout;
        self
    }

    /// Replaces [`Self::pids_limit`].
    #[must_use]
    pub fn with_pids_limit(mut self, limit: i64) -> Self {
        self.pids_limit = limit;
        self
    }
}

/// The container name of one sandbox.
#[must_use]
pub fn container_name(id: &SandboxId) -> String {
    format!("ewe-sbx-{id}")
}

/// The slave command of one link (section 6.3).
#[must_use]
pub fn slave_command(link: DockerLink) -> Vec<String> {
    slave_command_for(link, false)
}

/// The slave command of one link, with the control role when `control` holds.
///
/// A nested host adds `--control`, so its slave also serves the control plane
/// (feature 11, section 6.3).
#[must_use]
pub fn slave_command_for(link: DockerLink, control: bool) -> Vec<String> {
    let mut command = match link {
        DockerLink::Unix => vec![
            SLAVE_FILE.to_string(),
            "--listen".to_string(),
            format!("unix:{SLAVE_SOCKET}"),
            // The slave runs as root in the container and the control plane on
            // the host is a normal user, so mode 0600 would be out of reach.
            "--socket-mode".to_string(),
            "666".to_string(),
        ],
        DockerLink::Tcp => vec![
            SLAVE_FILE.to_string(),
            "--listen".to_string(),
            format!("tcp:0.0.0.0:{SLAVE_PORT}"),
        ],
    };
    if control {
        command.push("--control".to_string());
    }
    command
}

/// The Docker driver.
pub struct DockerDriver {
    config: DockerDriverConfig,
    api: Option<Arc<dyn DockerApi>>,
    connect_error: Option<String>,
}

impl Default for DockerDriver {
    fn default() -> Self {
        Self::new()
    }
}

impl std::fmt::Debug for DockerDriver {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("DockerDriver")
            .field("config", &self.config)
            .field("connected", &self.api.is_some())
            .finish_non_exhaustive()
    }
}

impl DockerDriver {
    /// A driver with the real Engine client of this host.
    #[must_use]
    pub fn new() -> Self {
        Self::with_config(DockerDriverConfig::default())
    }

    /// A driver with `config` and the real Engine client of this host.
    #[must_use]
    pub fn with_config(config: DockerDriverConfig) -> Self {
        match foundation_deployment_docker::DockerClient::connect_with_defaults() {
            Ok(client) => Self {
                config,
                api: Some(Arc::new(client) as Arc<dyn DockerApi>),
                connect_error: None,
            },
            Err(error) => Self {
                config,
                api: None,
                connect_error: Some(error.to_string()),
            },
        }
    }

    /// A driver over `api`. Tests use it.
    #[must_use]
    pub fn with_api(api: Arc<dyn DockerApi>) -> Self {
        Self::with_api_and_config(api, DockerDriverConfig::default())
    }

    /// A driver over `api` with `config`. Tests use it.
    #[must_use]
    pub fn with_api_and_config(api: Arc<dyn DockerApi>, config: DockerDriverConfig) -> Self {
        Self {
            config,
            api: Some(api),
            connect_error: None,
        }
    }

    /// The configuration of this driver.
    #[must_use]
    pub fn config(&self) -> &DockerDriverConfig {
        &self.config
    }

    /// The Engine connection, when this host has one.
    #[must_use]
    pub fn api(&self) -> Option<Arc<dyn DockerApi>> {
        self.api.clone()
    }

    /// The capabilities of this driver for one CPU count (section 6.5).
    #[must_use]
    pub fn capabilities(&self, max_vcpus: u8) -> Capabilities {
        capabilities_of(&self.config, max_vcpus)
    }

    /// The container control of this Engine.
    #[must_use]
    pub fn control(&self) -> Option<Arc<DockerContainerControl>> {
        self.api
            .as_ref()
            .map(|api| Arc::new(DockerContainerControl::new(Arc::clone(api))))
    }
}

impl SandboxDriver for DockerDriver {
    fn id(&self) -> DriverId {
        DriverId::Docker
    }

    fn probe(&self, ctx: Ctx) -> BoxFuture<'static, HostReport> {
        let config = self.config.clone();
        let api = self.api.clone();
        let connect_error = self.connect_error.clone();
        Box::pin(async move {
            let limits = SandboxLimits::default();
            let mut checks = Vec::new();
            let mut max_vcpus = 1_u8;
            let Some(api) = api else {
                checks.push(HostCheck {
                    name: "daemon".to_string(),
                    ok: false,
                    message: connect_error.unwrap_or_else(|| "no Docker connection".to_string()),
                });
                return HostReport {
                    driver: DriverId::Docker,
                    usable: false,
                    checks,
                    capabilities: capabilities_of(&config, max_vcpus),
                };
            };
            let info = call_with_timeout(&ctx, limits.rpc_timeout, api.system_info()).await;
            let (daemon_ok, daemon_message) = match &info {
                Ok(info) => {
                    if let Some(ncpu) = info
                        .ncpu
                        .and_then(|ncpu| u8::try_from(ncpu).ok())
                        .filter(|ncpu| *ncpu > 0)
                    {
                        max_vcpus = ncpu;
                    }
                    (
                        true,
                        format!(
                            "the daemon answers; {} CPUs, {}",
                            info.ncpu.unwrap_or(0),
                            info.architecture
                                .as_deref()
                                .unwrap_or("unknown architecture")
                        ),
                    )
                }
                Err(error) => (false, error.to_string()),
            };
            checks.push(HostCheck {
                name: "daemon".to_string(),
                ok: daemon_ok,
                message: daemon_message,
            });
            let host = SandboxHost::from_ctx(&ctx);
            let (slave_ok, slave_message) = match host.as_ref() {
                Ok(host) => {
                    let present = host
                        .config
                        .slave_path
                        .as_ref()
                        .is_some_and(|path| host.fs.exists(path));
                    (
                        present,
                        host.config.slave_path.as_ref().map_or_else(
                            || "no slave program configured".to_string(),
                            |path| {
                                if present {
                                    format!("the slave is at {}", path.display())
                                } else {
                                    format!("no slave at {}", path.display())
                                }
                            },
                        ),
                    )
                }
                Err(error) => (false, error.to_string()),
            };
            checks.push(HostCheck {
                name: "slave".to_string(),
                ok: slave_ok,
                message: slave_message,
            });
            // A nested host needs `/dev/kvm` on the daemon's host. The check is
            // a report only: it is not part of `usable` (feature 11, section
            // 6.3).
            let kvm_for_nested = host.is_ok_and(|host| host.fs.exists(Path::new("/dev/kvm")));
            checks.push(HostCheck {
                name: "kvm_for_nested".to_string(),
                ok: kvm_for_nested,
                message: if kvm_for_nested {
                    "/dev/kvm exists on the daemon's host".to_string()
                } else {
                    "no /dev/kvm on the daemon's host: a nested sandbox cannot start".to_string()
                },
            });
            HostReport {
                driver: DriverId::Docker,
                usable: daemon_ok && slave_ok,
                checks,
                capabilities: capabilities_of(&config, max_vcpus),
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
        let Some(api) = self.api.clone() else {
            let error: SandboxResult<Substrate> = Err(SandboxError::DriverUnavailable {
                driver: DriverId::Docker,
                reason: self
                    .connect_error
                    .clone()
                    .unwrap_or_else(|| "no Docker connection".to_string()),
            }
            .into());
            return Box::new(std::iter::once(ValtronStream::Next(error)));
        };
        let state = DockerState {
            launch: DockerLaunch::Prepare {
                ctx,
                id,
                spec,
                deadline,
            },
            guard: DockerLaunchGuard::new(),
            config: self.config.clone(),
            api,
        };
        match execute(docker_launch::new(state), None) {
            Ok(iterator) => Box::new(iterator),
            Err(error) => {
                let error: SandboxResult<Substrate> = Err(SandboxError::Io {
                    op: "launch docker".to_string(),
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
        let config = self.config.clone();
        let api = self.api.clone();
        Box::pin(async move {
            let Some(api) = api else {
                remove_run_dir(&ctx, &id)?;
                return Ok(None);
            };
            let name = container_name(&id);
            let inspected = match api.inspect_container(&name).await {
                Ok(inspected) => inspected,
                Err(error) if is_not_found(&error) => {
                    remove_run_dir(&ctx, &id)?;
                    return Ok(None);
                }
                Err(error) => return Err(map_error(error).into()),
            };
            if !container_runs(&inspected) || !has_sandbox_label(&inspected, &id) {
                let _ = api.remove_container(&name, true).await;
                remove_run_dir(&ctx, &id)?;
                return Ok(None);
            }
            let substrate = build_substrate(&ctx, &api, &config, &id, &inspected)?;
            Ok(Some(substrate))
        })
    }

    fn containers(&self) -> Option<Arc<dyn ContainerControl>> {
        self.control()
            .map(|control| control as Arc<dyn ContainerControl>)
    }
}

/// The capabilities of the driver for one CPU count.
fn capabilities_of(config: &DockerDriverConfig, max_vcpus: u8) -> Capabilities {
    Capabilities {
        rootfs_directory: false,
        rootfs_ext4: false,
        rootfs_squashfs: false,
        rootfs_container_image: true,
        root_modes: vec![RootMode::Overlay],
        host_mounts: true,
        network_none: config.link == DockerLink::Unix,
        network_host: true,
        max_vcpus,
    }
}

/// Every resolved value of one launch.
#[derive(Clone, Debug)]
pub struct DockerPlan {
    /// The sandbox identifier.
    pub id: SandboxId,
    /// The image name.
    pub image: String,
    /// The container name.
    pub container_name: String,
    /// The run directory, with the Unix domain socket link.
    pub run_dir: Option<PathBuf>,
    /// The slave program on this host.
    pub slave_path: PathBuf,
    /// The configuration of the launch.
    pub config: DockerDriverConfig,
    /// The caller's spec.
    pub spec: SandboxSpec,
}

/// One step of the launch.
pub enum DockerLaunch {
    /// Resolve the image and the slave, and make the run directory.
    Prepare {
        /// The injected context.
        ctx: Ctx,
        /// The sandbox identifier.
        id: SandboxId,
        /// The caller's spec.
        spec: SandboxSpec,
        /// The one deadline of the create.
        deadline: Instant,
    },
    /// Inspect the image, and pull it when it is missing. Read the pull
    /// progress one line at a time.
    Pull {
        /// The injected context.
        ctx: Ctx,
        /// The resolved plan.
        plan: Box<DockerPlan>,
        /// The pull progress stream, once a pull started.
        stream: Option<ProgressStream<CreateImageInfo>>,
        /// When the last progress value was emitted.
        last_emit: Option<Instant>,
        /// The end of the pull.
        pull_deadline: Instant,
    },
    /// Create the container.
    Create {
        /// The injected context.
        ctx: Ctx,
        /// The resolved plan.
        plan: Box<DockerPlan>,
    },
    /// Upload the slave into the container.
    Inject {
        /// The injected context.
        ctx: Ctx,
        /// The resolved plan.
        plan: Box<DockerPlan>,
        /// The created container.
        container: String,
    },
    /// Start the container.
    Start {
        /// The injected context.
        ctx: Ctx,
        /// The resolved plan.
        plan: Box<DockerPlan>,
        /// The created container.
        container: String,
    },
    /// Start the slave beside the image's own main process.
    Sidecar {
        /// The injected context.
        ctx: Ctx,
        /// The resolved plan.
        plan: Box<DockerPlan>,
        /// The created container.
        container: String,
        /// The ready substrate, once the container runs.
        substrate: Substrate,
    },
    /// Give the ready substrate on the next step.
    EmitReady {
        /// The ready substrate.
        substrate: Substrate,
    },
    /// The launch is done.
    Done,
}

/// The state of the launch task.
pub struct DockerState {
    /// The current step.
    pub launch: DockerLaunch,
    /// The guard that removes the container when the task ends before its
    /// result.
    pub guard: Arc<DockerLaunchGuard>,
    /// The configuration of the driver.
    pub config: DockerDriverConfig,
    /// The Engine connection.
    pub api: Arc<dyn DockerApi>,
}

impl DockerState {
    fn new(launch: DockerLaunch, guard: &Arc<DockerLaunchGuard>, state: &DockerState) -> Self {
        Self {
            launch,
            guard: Arc::clone(guard),
            config: state.config.clone(),
            api: Arc::clone(&state.api),
        }
    }
}

type Status = TaskStatus<SandboxResult<Substrate>, CreateProgress>;

/// One transition of the launch state machine.
#[task]
pub async fn docker_launch(
    ctx: Ctx,
    state: DockerState,
) -> Option<(
    DockerState,
    TaskStatus<SandboxResult<Substrate>, CreateProgress>,
)> {
    // The executor supplies its own context. Every clock and timer read must use
    // the driver's injected context, which the state holds.
    drop(ctx);
    let mut state = state;
    let launch = std::mem::replace(&mut state.launch, DockerLaunch::Done);
    let next = match launch {
        DockerLaunch::Prepare {
            ctx,
            id,
            spec,
            deadline,
        } => prepare(&state, &ctx, id, spec, deadline).await,
        DockerLaunch::Pull {
            ctx,
            plan,
            stream,
            last_emit,
            pull_deadline,
        } => pull(&state, &ctx, plan, stream, last_emit, pull_deadline).await,
        DockerLaunch::Create { ctx, plan } => create(&state, &ctx, plan).await,
        DockerLaunch::Inject {
            ctx,
            plan,
            container,
        } => inject(&state, &ctx, plan, container).await,
        DockerLaunch::Start {
            ctx,
            plan,
            container,
        } => start(&state, &ctx, plan, container).await,
        DockerLaunch::Sidecar {
            ctx,
            plan,
            container,
            substrate,
        } => sidecar(&state, &ctx, plan, container, substrate).await,
        DockerLaunch::EmitReady { substrate } => {
            // The substrate handle owns the container from now on. Without
            // this, the guard would remove a good container when the task ends.
            state.guard.disarm();
            (
                DockerState::new(DockerLaunch::Done, &state.guard, &state),
                TaskStatus::Ready(Ok(substrate)),
            )
        }
        DockerLaunch::Done => return None,
    };
    Some(next)
}

/// The `Prepare` step: resolve the image, the slave and the run directory.
async fn prepare(
    state: &DockerState,
    ctx: &Ctx,
    id: SandboxId,
    spec: SandboxSpec,
    deadline: Instant,
) -> (DockerState, Status) {
    if ctx.clock().now() >= deadline {
        return fail(
            state,
            SandboxError::BootTimeout {
                stage: "prepare".to_string(),
                log_tail: Vec::new(),
            },
        )
        .await;
    }
    if state.config.link == DockerLink::Tcp && spec.network == NetworkMode::None {
        return fail(state, SandboxError::UnsupportedNetwork).await;
    }
    let image = match &spec.rootfs {
        RootFs::ContainerImage(image) => image.clone(),
        _ => return fail(state, SandboxError::UnsupportedRootFs).await,
    };
    let host = match SandboxHost::from_ctx(ctx) {
        Ok(host) => host,
        Err(error) => return fail(state, error.current_context().clone()).await,
    };
    let info = match call_with_timeout(ctx, spec.limits.rpc_timeout, state.api.system_info()).await
    {
        Ok(info) => info,
        Err(error) => return fail(state, error.current_context().clone()).await,
    };
    let arch = daemon_arch(info.architecture.as_deref());
    let slave_path = match resolve_slave(&host, arch) {
        Ok(path) => path,
        Err(error) => return fail(state, error.current_context().clone()).await,
    };
    let run_dir = if state.config.link == DockerLink::Unix {
        let dir = host.config.state_dir.join(id.as_str());
        if let Err(error) = host.fs.create_dir_all(&dir, 0o700) {
            return fail(state, error.current_context().clone()).await;
        }
        state.guard.arm_run_dir(Arc::clone(&host.fs), dir.clone());
        Some(dir)
    } else {
        None
    };
    state.guard.arm_api(Arc::clone(&state.api));
    let plan = Box::new(DockerPlan {
        id: id.clone(),
        image,
        container_name: container_name(&id),
        run_dir,
        slave_path,
        config: state.config.clone(),
        spec,
    });
    (
        DockerState::new(
            DockerLaunch::Pull {
                ctx: ctx.clone(),
                plan,
                stream: None,
                last_emit: None,
                pull_deadline: ctx.clock().now() + state.config.pull_timeout,
            },
            &state.guard,
            state,
        ),
        TaskStatus::Pending(CreateProgress::Preparing),
    )
}

/// The `Pull` step: inspect the image, then read one pull progress line.
#[allow(
    clippy::too_many_lines,
    reason = "the step has the inspect, the pull start and one progress line of the design"
)]
async fn pull(
    state: &DockerState,
    ctx: &Ctx,
    plan: Box<DockerPlan>,
    stream: Option<ProgressStream<CreateImageInfo>>,
    last_emit: Option<Instant>,
    pull_deadline: Instant,
) -> (DockerState, Status) {
    let mut stream = if let Some(stream) = stream {
        stream
    } else {
        match call_raw(
            ctx,
            plan.spec.limits.rpc_timeout,
            state.api.image_inspect(&plan.image),
        )
        .await
        {
            Ok(_) => {
                // The image exists: no pull is needed.
                return (
                    DockerState::new(
                        DockerLaunch::Create {
                            ctx: ctx.clone(),
                            plan,
                        },
                        &state.guard,
                        state,
                    ),
                    TaskStatus::Ignore,
                );
            }
            Err(CallFailure::Docker(error)) if is_not_found(&error) => {}
            Err(CallFailure::Docker(error)) => {
                return fail(state, map_error(error)).await;
            }
            Err(CallFailure::Timeout) => {
                return fail(state, timeout_error(plan.spec.limits.rpc_timeout)).await;
            }
        }
        // The pull has its own deadline: the daemon keeps the request open
        // while it downloads the layers (section 6.2).
        match call_with_timeout(
            ctx,
            plan.config.pull_timeout,
            state.api.image_pull(&plan.image),
        )
        .await
        {
            Ok(stream) => stream,
            Err(error) => return fail(state, error.current_context().clone()).await,
        }
    };
    let now = ctx.clock().now();
    if now >= pull_deadline {
        return fail(
            state,
            SandboxError::BootTimeout {
                stage: "pull".to_string(),
                log_tail: Vec::new(),
            },
        )
        .await;
    }
    let timer = match ctx.require_env() {
        Ok(env) => env.timer(),
        Err(_) => return fail(state, SandboxError::HostNotConfigured).await,
    };
    let next = Box::pin(stream.next());
    let outcome = match select(next, timer.sleep_until(ctx.clone(), pull_deadline)).await {
        Either::Left((item, _)) => item,
        Either::Right(_) => {
            return fail(
                state,
                SandboxError::BootTimeout {
                    stage: "pull".to_string(),
                    log_tail: Vec::new(),
                },
            )
            .await;
        }
    };
    match outcome {
        Some(Ok(_item)) => {
            let emit = last_emit
                .is_none_or(|last| now.saturating_duration_since(last) >= Duration::from_secs(1));
            (
                DockerState::new(
                    DockerLaunch::Pull {
                        ctx: ctx.clone(),
                        plan,
                        stream: Some(stream),
                        last_emit: if emit { Some(now) } else { last_emit },
                        pull_deadline,
                    },
                    &state.guard,
                    state,
                ),
                if emit {
                    TaskStatus::Pending(CreateProgress::Preparing)
                } else {
                    TaskStatus::Ignore
                },
            )
        }
        Some(Err(error)) => fail(state, map_error(error)).await,
        None => (
            DockerState::new(
                DockerLaunch::Create {
                    ctx: ctx.clone(),
                    plan,
                },
                &state.guard,
                state,
            ),
            TaskStatus::Ignore,
        ),
    }
}

/// The `Create` step: make the container.
async fn create(state: &DockerState, ctx: &Ctx, plan: Box<DockerPlan>) -> (DockerState, Status) {
    let body = sandbox_body(&plan);
    let created: ContainerCreateResponse = match call_with_timeout(
        ctx,
        plan.spec.limits.rpc_timeout,
        state
            .api
            .create_container(&body, Some(&plan.container_name)),
    )
    .await
    {
        Ok(created) => created,
        Err(error) => return fail(state, error.current_context().clone()).await,
    };
    state.guard.arm_container(created.id.clone());
    (
        DockerState::new(
            DockerLaunch::Inject {
                ctx: ctx.clone(),
                plan,
                container: created.id,
            },
            &state.guard,
            state,
        ),
        TaskStatus::Ignore,
    )
}

/// The `Inject` step: upload the slave into the container.
async fn inject(
    state: &DockerState,
    ctx: &Ctx,
    plan: Box<DockerPlan>,
    container: String,
) -> (DockerState, Status) {
    let host = match SandboxHost::from_ctx(ctx) {
        Ok(host) => host,
        Err(error) => return fail(state, error.current_context().clone()).await,
    };
    let tar = match slave_tar(&host.fs, &plan.slave_path) {
        Ok(tar) => tar,
        Err(error) => return fail(state, error.current_context().clone()).await,
    };
    if let Err(error) = call_with_timeout(
        ctx,
        plan.spec.limits.rpc_timeout,
        state.api.upload_to_container(&container, "/", tar),
    )
    .await
    {
        return fail(state, error.current_context().clone()).await;
    }
    (
        DockerState::new(
            DockerLaunch::Start {
                ctx: ctx.clone(),
                plan,
                container,
            },
            &state.guard,
            state,
        ),
        TaskStatus::Ignore,
    )
}

/// The `Start` step: start the container and build the substrate.
async fn start(
    state: &DockerState,
    ctx: &Ctx,
    plan: Box<DockerPlan>,
    container: String,
) -> (DockerState, Status) {
    if let Err(error) = call_with_timeout(
        ctx,
        plan.spec.limits.rpc_timeout,
        state.api.start_container(&container),
    )
    .await
    {
        return fail(state, error.current_context().clone()).await;
    }
    let inspected = match call_with_timeout(
        ctx,
        plan.spec.limits.rpc_timeout,
        state.api.inspect_container(&container),
    )
    .await
    {
        Ok(inspected) => inspected,
        Err(error) => return fail(state, error.current_context().clone()).await,
    };
    let substrate = match build_substrate(ctx, &state.api, &plan.config, &plan.id, &inspected) {
        Ok(substrate) => substrate,
        Err(error) => return fail(state, error.current_context().clone()).await,
    };
    let next = if plan.config.slave_mode == SlaveMode::Sidecar {
        DockerLaunch::Sidecar {
            ctx: ctx.clone(),
            plan,
            container,
            substrate,
        }
    } else {
        DockerLaunch::EmitReady { substrate }
    };
    (
        DockerState::new(next, &state.guard, state),
        TaskStatus::Pending(CreateProgress::SubstrateStarted),
    )
}

/// The `Sidecar` step: start the slave beside the image's main process.
async fn sidecar(
    state: &DockerState,
    ctx: &Ctx,
    plan: Box<DockerPlan>,
    container: String,
    substrate: Substrate,
) -> (DockerState, Status) {
    let command = slave_command_for(plan.config.link, plan.spec.nested_host);
    let argv: Vec<&str> = command.iter().map(String::as_str).collect();
    let created = match call_with_timeout(
        ctx,
        plan.spec.limits.rpc_timeout,
        state.api.exec_create(&container, &argv),
    )
    .await
    {
        Ok(created) => created,
        Err(error) => return fail(state, error.current_context().clone()).await,
    };
    // Detached: the slave runs for the life of the container, so this call must
    // not wait for its exit.
    match call_with_timeout(
        ctx,
        plan.spec.limits.rpc_timeout,
        state.api.exec_start(&created.id, true),
    )
    .await
    {
        Ok(mut stream) => {
            // A detached exec has an empty body; read it to the end so the
            // connection closes at once.
            while stream.next().await.is_some() {}
        }
        Err(error) => return fail(state, error.current_context().clone()).await,
    }
    (
        DockerState::new(DockerLaunch::EmitReady { substrate }, &state.guard, state),
        TaskStatus::Ignore,
    )
}

/// Ends the launch with `error`, after it removes the container and the run
/// directory (invariant I5).
async fn fail(state: &DockerState, error: SandboxError) -> (DockerState, Status) {
    if let Some(parts) = state.guard.take() {
        if let (Some(api), Some(container)) = (parts.api, parts.container) {
            // A missing container is fine: the launch may fail before create.
            let _ = api.remove_container(&container, true).await;
        }
        if let (Some(fs), Some(run_dir)) = (parts.fs, parts.run_dir) {
            let _ = fs.remove_dir_all(&run_dir);
        }
    }
    state.guard.disarm();
    (
        DockerState::new(DockerLaunch::Done, &state.guard, state),
        TaskStatus::Ready(Err(error.into())),
    )
}

/// Builds the substrate of a running container.
fn build_substrate(
    ctx: &Ctx,
    api: &Arc<dyn DockerApi>,
    config: &DockerDriverConfig,
    id: &SandboxId,
    inspected: &ContainerInspectResponse,
) -> SandboxResult<Substrate> {
    let endpoint = endpoint_of(ctx, config, id, inspected)?;
    let run_dir = if config.link == DockerLink::Unix {
        Some(run_dir(ctx, id)?)
    } else {
        None
    };
    let container = inspected.id.clone().unwrap_or_else(|| container_name(id));
    let image = inspected
        .config
        .as_ref()
        .and_then(|config| config.image.clone())
        .unwrap_or_default();
    let handle = DockerHandle::new(Arc::clone(api), ctx.clone(), container, run_dir, image);
    Ok(Substrate {
        endpoint,
        root: RootSetup {
            source: RootSource::Plain,
            mode: RootMode::Overlay,
        },
        mounts: Vec::new(),
        handle: Box::new(handle),
        calls: None,
        slave: None,
    })
}

/// The slave endpoint of one inspected container.
fn endpoint_of(
    ctx: &Ctx,
    config: &DockerDriverConfig,
    id: &SandboxId,
    inspected: &ContainerInspectResponse,
) -> SandboxResult<SlaveEndpoint> {
    match config.link {
        DockerLink::Unix => Ok(SlaveEndpoint::Unix {
            path: run_dir(ctx, id)?.join("slave.sock"),
        }),
        DockerLink::Tcp => {
            let port = published_port(inspected, SLAVE_PORT).ok_or_else(|| SandboxError::Io {
                op: "docker inspect".to_string(),
                message: format!("the container publishes no {SLAVE_PORT}/tcp port"),
            })?;
            Ok(SlaveEndpoint::Tcp {
                host: config.publish_address.clone(),
                port,
            })
        }
    }
}

/// The run directory of one sandbox.
fn run_dir(ctx: &Ctx, id: &SandboxId) -> SandboxResult<PathBuf> {
    let host = SandboxHost::from_ctx(ctx)?;
    Ok(host.config.state_dir.join(id.as_str()))
}

/// Removes the run directory of one sandbox, when it exists.
fn remove_run_dir(ctx: &Ctx, id: &SandboxId) -> SandboxResult<()> {
    let Ok(host) = SandboxHost::from_ctx(ctx) else {
        return Ok(());
    };
    let dir = host.config.state_dir.join(id.as_str());
    if host.fs.exists(&dir) {
        host.fs.remove_dir_all(&dir)?;
    }
    Ok(())
}

/// Whether an inspected container runs.
fn container_runs(inspected: &ContainerInspectResponse) -> bool {
    inspected
        .state
        .as_ref()
        .and_then(|state| state.running)
        .unwrap_or(false)
}

/// Whether an inspected container has the label of this sandbox.
fn has_sandbox_label(inspected: &ContainerInspectResponse, id: &SandboxId) -> bool {
    inspected
        .config
        .as_ref()
        .and_then(|config| config.labels.as_ref())
        .and_then(|labels| labels.get(LABEL_SANDBOX_ID))
        .and_then(serde_json::Value::as_str)
        == Some(id.as_str())
}

/// The host port that a container publishes for one container port.
fn published_port(inspected: &ContainerInspectResponse, port: u16) -> Option<u16> {
    let settings = inspected.network_settings.as_ref()?;
    let ports = settings.ports.as_ref()?;
    let bindings = ports.get(format!("{port}/tcp"))?;
    let first = bindings.as_array()?.first()?;
    first
        .get("HostPort")
        .and_then(serde_json::Value::as_str)
        .and_then(|value| value.parse().ok())
}

/// The slave architecture of the daemon.
fn daemon_arch(architecture: Option<&str>) -> GuestArch {
    match architecture {
        Some("x86_64" | "amd64") => GuestArch::X86_64,
        Some("aarch64" | "arm64") => GuestArch::Aarch64,
        _ => GuestArch::host(),
    }
}

/// Resolves the slave program for `arch`.
fn resolve_slave(host: &SandboxHost, arch: GuestArch) -> SandboxResult<PathBuf> {
    let configured =
        host.config
            .slave_path
            .as_ref()
            .ok_or_else(|| SandboxError::BinaryNotFound {
                name: "agent-slave".to_string(),
            })?;
    if let Some(parent) = configured.parent() {
        let candidate = parent.join(format!("agent-slave-{}", arch.as_str()));
        if host.fs.exists(&candidate) {
            return Ok(candidate);
        }
    }
    if host.fs.exists(configured) {
        return Ok(configured.clone());
    }
    Err(SandboxError::BinaryNotFound {
        name: configured.display().to_string(),
    }
    .into())
}

/// The create body of one sandbox container (section 6.4).
#[must_use]
pub fn sandbox_body(plan: &DockerPlan) -> ContainerCreateBody {
    let mut labels = BTreeMap::new();
    labels.insert(LABEL_SANDBOX_ID, plan.id.to_string());
    labels.insert(LABEL_SANDBOX_DRIVER, "docker".to_string());
    let nested = plan.spec.nested_host;
    let (entrypoint, cmd) = match plan.config.slave_mode {
        SlaveMode::Main => (
            Some(slave_command_for(plan.config.link, nested)),
            Some(Vec::new()),
        ),
        // Sidecar: the image keeps its own entry point and command.
        SlaveMode::Sidecar => (None, None),
    };
    let exposed_ports = (plan.config.link == DockerLink::Tcp).then(|| {
        let mut ports = Map::new();
        ports.insert(format!("{SLAVE_PORT}/tcp"), json!({}));
        Value::Object(ports)
    });
    // A nested host gets the paths of its artifacts and its run directory
    // (feature 11, section 6.3). A normal container gets no environment
    // variable (invariant I2).
    let env = nested.then(|| {
        vec![
            "EWE_FIRECRACKER=/usr/local/bin/firecracker".to_string(),
            format!("EWE_SANDBOX_SLAVE={SLAVE_FILE}"),
            "EWE_SANDBOX_STATE_DIR=/run/ewe-sbx".to_string(),
            "EWE_SANDBOX_CACHE_DIR=/var/cache/ewe-sandbox".to_string(),
        ]
    });
    let config = ContainerConfig {
        image: Some(plan.image.clone()),
        entrypoint,
        cmd,
        env,
        labels: Some(json!(labels)),
        exposed_ports,
        ..ContainerConfig::default()
    };
    let mut binds = Vec::new();
    if let Some(run_dir) = &plan.run_dir {
        binds.push(format!("{}:/.ewe/run", run_dir.display()));
    }
    for mount in &plan.spec.mounts {
        let mut bind = format!("{}:{}", mount.host_path.display(), mount.guest_path);
        if mount.read_only {
            bind.push_str(":ro");
        }
        binds.push(bind);
    }
    let port_bindings = (plan.config.link == DockerLink::Tcp).then(|| {
        let mut ports = Map::new();
        ports.insert(
            format!("{SLAVE_PORT}/tcp"),
            json!([{
                "HostIp": plan.config.publish_address,
                "HostPort": "0",
            }]),
        );
        Value::Object(ports)
    });
    // A nested host gets `/dev/kvm`, and still no privilege (invariant I6).
    let devices = nested.then(|| {
        vec![
            foundation_deployment_docker::generated::json::DeviceMapping {
                cgroup_permissions: Some("rwm".to_string()),
                path_in_container: Some("/dev/kvm".to_string()),
                path_on_host: Some("/dev/kvm".to_string()),
            },
        ]
    });
    let host = HostConfig {
        auto_remove: Some(false),
        binds: (!binds.is_empty()).then_some(binds),
        devices,
        memory: Some(i64::from(plan.spec.memory_mib) * 1_048_576),
        nano_cpus: Some(i64::from(plan.spec.vcpus) * 1_000_000_000),
        network_mode: Some(
            match plan.spec.network {
                NetworkMode::Host => "bridge",
                NetworkMode::None => "none",
            }
            .to_string(),
        ),
        pids_limit: Some(plan.config.pids_limit),
        port_bindings,
        privileged: Some(false),
        security_opt: Some(vec!["no-new-privileges".to_string()]),
        ..HostConfig::default()
    };
    create_body(config, host)
}

/// Why one Engine call did not give a result.
enum CallFailure {
    /// The daemon answered with an error.
    Docker(DockerError),
    /// The call passed its time limit.
    Timeout,
}

/// Calls one Engine operation with a time limit, and keeps the Docker error.
async fn call_raw<T, F>(ctx: &Ctx, timeout: Duration, work: F) -> Result<T, CallFailure>
where
    F: std::future::Future<Output = Result<T, DockerError>> + Send,
{
    let timer = match ctx.require_env() {
        Ok(env) => env.timer(),
        Err(_) => {
            return Err(CallFailure::Docker(DockerError::Unavailable(
                "the sandbox host is not configured".to_string(),
            )))
        }
    };
    let deadline = ctx.clock().now() + timeout;
    match select(Box::pin(work), timer.sleep_until(ctx.clone(), deadline)).await {
        Either::Left((result, _)) => result.map_err(CallFailure::Docker),
        Either::Right(_) => Err(CallFailure::Timeout),
    }
}

/// Calls one Engine operation with a time limit, and maps its error.
async fn call_with_timeout<T, F>(ctx: &Ctx, timeout: Duration, work: F) -> SandboxResult<T>
where
    F: std::future::Future<Output = Result<T, DockerError>> + Send,
{
    match call_raw(ctx, timeout, work).await {
        Ok(value) => Ok(value),
        Err(CallFailure::Docker(error)) => Err(map_error(error).into()),
        Err(CallFailure::Timeout) => Err(timeout_error(timeout).into()),
    }
}

/// The error of a call that passed its time limit.
fn timeout_error(timeout: Duration) -> SandboxError {
    SandboxError::ApiTimeout {
        timeout_ms: u64::try_from(timeout.as_millis()).unwrap_or(u64::MAX),
    }
}

/// The guard that removes the container and the run directory when a launch
/// ends before its result.
pub struct DockerLaunchGuard {
    inner: Mutex<Option<GuardParts>>,
}

struct GuardParts {
    api: Option<Arc<dyn DockerApi>>,
    container: Option<String>,
    fs: Option<Arc<dyn HostFs>>,
    run_dir: Option<PathBuf>,
}

impl DockerLaunchGuard {
    /// An unarmed guard.
    #[must_use]
    pub fn new() -> Arc<Self> {
        Arc::new(Self {
            inner: Mutex::new(None),
        })
    }

    fn arm_api(&self, api: Arc<dyn DockerApi>) {
        self.with_parts(|parts| parts.api = Some(api));
    }

    fn arm_run_dir(&self, fs: Arc<dyn HostFs>, run_dir: PathBuf) {
        self.with_parts(|parts| {
            parts.fs = Some(fs);
            parts.run_dir = Some(run_dir);
        });
    }

    fn arm_container(&self, container: String) {
        self.with_parts(|parts| parts.container = Some(container));
    }

    fn with_parts(&self, action: impl FnOnce(&mut GuardParts)) {
        let mut inner = self.inner.lock().unwrap_or_else(PoisonError::into_inner);
        let parts = inner.get_or_insert_with(|| GuardParts {
            api: None,
            container: None,
            fs: None,
            run_dir: None,
        });
        action(parts);
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

impl Drop for DockerLaunchGuard {
    fn drop(&mut self) {
        let parts = self
            .inner
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .take();
        let Some(GuardParts {
            api,
            container,
            fs,
            run_dir,
        }) = parts
        else {
            return;
        };
        if api.is_none() && run_dir.is_none() {
            return;
        }
        if let Err(error) = send(from_future(async move {
            if let (Some(api), Some(container)) = (api, container) {
                let _ = api.remove_container(&container, true).await;
            }
            if let (Some(fs), Some(run_dir)) = (fs, run_dir) {
                let _ = fs.remove_dir_all(&run_dir);
            }
        })) {
            tracing::warn!(error = %error, "could not start the launch clean-up task");
        }
    }
}

/// The handle of one sandbox container (section 6.6).
pub struct DockerHandle {
    api: Arc<dyn DockerApi>,
    ctx: Ctx,
    container: String,
    run_dir: Option<PathBuf>,
    image: String,
    forced: AtomicBool,
}

impl DockerHandle {
    /// The handle of one started container.
    #[must_use]
    pub fn new(
        api: Arc<dyn DockerApi>,
        ctx: Ctx,
        container: String,
        run_dir: Option<PathBuf>,
        image: String,
    ) -> Self {
        Self {
            api,
            ctx,
            container,
            run_dir,
            image,
            forced: AtomicBool::new(false),
        }
    }

    /// The Docker container identifier.
    #[must_use]
    pub fn id(&self) -> &str {
        &self.container
    }
}

impl SubstrateHandle for DockerHandle {
    fn exited(&self) -> BoxFuture<'_, SubstrateExit> {
        let api = Arc::clone(&self.api);
        let container = self.container.clone();
        let forced = self.forced.load(Ordering::SeqCst);
        Box::pin(async move {
            match api.wait_container(&container).await {
                Ok(response) => SubstrateExit {
                    code: control_exit_code(response.status_code),
                    signal: None,
                    forced,
                },
                Err(_) => SubstrateExit {
                    code: None,
                    signal: None,
                    forced,
                },
            }
        })
    }

    fn terminate(&self, grace: Duration) -> BoxFuture<'_, SandboxResult<SubstrateExit>> {
        let api = Arc::clone(&self.api);
        let container = self.container.clone();
        Box::pin(async move {
            let seconds = u32::try_from(grace.as_secs()).unwrap_or(u32::MAX).max(1);
            api.stop_container(&container, Some(seconds))
                .await
                .map_err(map_error)?;
            let response = api.wait_container(&container).await.map_err(map_error)?;
            Ok(exit_of(&response))
        })
    }

    fn kill(&self) -> BoxFuture<'_, SandboxResult<SubstrateExit>> {
        let api = Arc::clone(&self.api);
        let container = self.container.clone();
        self.forced.store(true, Ordering::SeqCst);
        Box::pin(async move {
            match api.remove_container(&container, true).await {
                Ok(()) => {}
                Err(error) if is_not_found(&error) => {}
                Err(error) => return Err(map_error(error).into()),
            }
            Ok(SubstrateExit {
                code: None,
                signal: Some(9),
                forced: true,
            })
        })
    }

    fn log_tail(&self, max: usize) -> BoxFuture<'_, Vec<u8>> {
        let api = Arc::clone(&self.api);
        let container = self.container.clone();
        Box::pin(async move { read_log_tail(&*api, &container, max).await })
    }

    fn cleanup(&self) -> BoxFuture<'_, SandboxResult<()>> {
        let api = Arc::clone(&self.api);
        let container = self.container.clone();
        let ctx = self.ctx.clone();
        let run_dir = self.run_dir.clone();
        Box::pin(async move {
            match api.remove_container(&container, true).await {
                Ok(()) => {}
                Err(error) if is_not_found(&error) => {}
                Err(error) => return Err(map_error(error).into()),
            }
            if let Some(run_dir) = run_dir {
                let host = SandboxHost::from_ctx(&ctx)?;
                if host.fs.exists(&run_dir) {
                    host.fs.remove_dir_all(&run_dir)?;
                }
            }
            Ok(())
        })
    }

    fn container(&self) -> Option<ContainerHandle> {
        let id = ContainerId::new(self.container.clone()).ok()?;
        let info = ContainerInfo {
            id: id.clone(),
            driver: DriverId::Docker,
            image: self.image.clone(),
            status: ContainerStatus::Running,
            created_unix_ms: 0,
            resources: ContainerResources::default(),
        };
        let ops = Arc::new(control::DockerContainerOps::new(
            Arc::clone(&self.api),
            self.ctx.clone(),
            info,
        ));
        Some(ContainerHandle::new(id, ops))
    }
}

/// The exit code of one Docker wait answer, in the handle.
fn control_exit_code(status: i64) -> Option<i32> {
    i32::try_from(status).ok()
}
