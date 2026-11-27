//! The Cloudflare drivers: one for the container and one for the model slave.

use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::time::Duration;

use foundation_compact::SendWrapper;
use foundation_context::Ctx;
use foundation_core::valtron::{task, Stream, TaskStatus};

use crate::shared::cloudflare::container::{CloudflareContainerControl, DoContainerOps};
use crate::shared::cloudflare::{CloudflareConfig, CloudflareHost, ContainerStart};
use crate::shared::container::{
    ContainerControl, ContainerHandle, ContainerId, ContainerResources,
};
use crate::shared::control::record::created_unix_ms;
use crate::shared::control::wait_until;
use crate::shared::driver::{
    Capabilities, CreateProgress, HostCheck, HostReport, LaunchStream, SandboxDriver,
    SlaveEndpoint, StoredRecord, Substrate, SubstrateExit, SubstrateHandle,
};
use crate::shared::error::{SandboxError, SandboxResult};
use crate::shared::host::SlaveConnector;
use crate::shared::ids::{DriverId, SandboxId};
use crate::shared::limits::SandboxLimits;
use crate::shared::spec::{NetworkMode, RootFs, RootMode, RootSetup, RootSource, SandboxSpec};
use crate::shared::BoxFuture;

/// An iterator that is `Send` on every target.
///
/// On a native target the wrapped iterator is `Send` already. On single-threaded
/// wasm [`SendWrapper`] supplies the bound; the value never crosses a thread
/// because the target has one.
struct SendIterator<I>(SendWrapper<I>);

impl<I: Iterator> Iterator for SendIterator<I> {
    type Item = I::Item;

    fn next(&mut self) -> Option<Self::Item> {
        self.0.next()
    }
}

/// The Cloudflare container driver.
pub struct CloudflareContainerDriver {
    host: CloudflareHost,
    connector: Arc<dyn SlaveConnector>,
    control: Arc<CloudflareContainerControl>,
}

impl CloudflareContainerDriver {
    /// A driver over one DO bundle and its slave connector.
    #[must_use]
    pub fn new(host: CloudflareHost, connector: Arc<dyn SlaveConnector>) -> Self {
        let control = Arc::new(CloudflareContainerControl::new(
            host.clone(),
            Arc::clone(&connector),
        ));
        Self {
            host,
            connector,
            control,
        }
    }

    /// The configuration of this driver.
    #[must_use]
    pub fn config(&self) -> &CloudflareConfig {
        &self.host.config
    }

    /// What this driver can do.
    #[must_use]
    pub fn capabilities() -> Capabilities {
        Capabilities {
            rootfs_directory: false,
            rootfs_ext4: false,
            rootfs_squashfs: false,
            rootfs_container_image: true,
            root_modes: vec![RootMode::Overlay],
            host_mounts: false,
            network_none: true,
            network_host: true,
            max_vcpus: 1,
        }
    }

    /// The substrate of the running container.
    fn substrate(&self, ctx: Ctx, created_unix_ms: u64) -> Substrate {
        let endpoint = SlaveEndpoint::CloudflarePort {
            port: self.host.config.slave_port,
        };
        Substrate {
            endpoint: endpoint.clone(),
            root: RootSetup {
                source: RootSource::Plain,
                mode: RootMode::Overlay,
            },
            mounts: Vec::new(),
            handle: Box::new(CloudflareSubstrateHandle {
                ctx,
                host: self.host.clone(),
                connector: Arc::clone(&self.connector),
                endpoint,
                created_unix_ms,
                stopped: Arc::new(AtomicBool::new(false)),
            }),
            calls: None,
            slave: None,
        }
    }
}

/// One step of the container launch.
enum LaunchState {
    /// Checks the spec and the container before the start.
    Check {
        spec: SandboxSpec,
        created_unix_ms: u64,
    },
    /// Starts the container.
    Start {
        spec: SandboxSpec,
        created_unix_ms: u64,
    },
    /// Gives the ready substrate.
    Ready { created_unix_ms: u64 },
    /// The task is done.
    Done,
}

/// One run of the container launch.
pub struct LaunchRun {
    ctx: Ctx,
    host: CloudflareHost,
    connector: Arc<dyn SlaveConnector>,
    state: LaunchState,
}

/// The error for a spec that the driver does not accept.
fn reject_spec(spec: &SandboxSpec, config: &CloudflareConfig) -> Option<SandboxError> {
    match &spec.rootfs {
        RootFs::ContainerImage(name) if name == &config.image_name => {}
        _ => return Some(SandboxError::UnsupportedRootFs),
    }
    if spec.root_mode != RootMode::Overlay {
        return Some(SandboxError::InvalidSpec {
            field: "root_mode".to_string(),
            reason: "the Cloudflare driver supports only the overlay root mode".to_string(),
        });
    }
    if !spec.mounts.is_empty() {
        return Some(SandboxError::UnsupportedMount);
    }
    None
}

/// The state machine of one container launch.
#[task]
#[allow(
    clippy::unused_async,
    reason = "the task macro needs an async body; this launch has no await"
)]
async fn launch_container(
    ctx: Ctx,
    mut run: LaunchRun,
) -> Option<(
    LaunchRun,
    TaskStatus<SandboxResult<Substrate>, CreateProgress>,
)> {
    let _ = &ctx;
    let state = std::mem::replace(&mut run.state, LaunchState::Done);
    match state {
        LaunchState::Check {
            spec,
            created_unix_ms,
        } => {
            if let Some(error) = reject_spec(&spec, &run.host.config) {
                return Some((run, TaskStatus::Ready(Err(error.into()))));
            }
            if run.host.container.running() {
                return Some((
                    run,
                    TaskStatus::Ready(Err(SandboxError::TooManySessions.into())),
                ));
            }
            run.state = LaunchState::Start {
                spec,
                created_unix_ms,
            };
            Some((run, TaskStatus::Pending(CreateProgress::Preparing)))
        }
        LaunchState::Start {
            spec,
            created_unix_ms,
        } => {
            let start = ContainerStart {
                entrypoint: run.host.config.slave_entrypoint(),
                enable_internet: spec.network == NetworkMode::Host,
                env: Vec::new(),
            };
            if let Err(error) = run.host.container.start(start) {
                return Some((
                    run,
                    TaskStatus::Ready(Err(SandboxError::SubstrateApi {
                        status: 0,
                        message: error.to_string(),
                    }
                    .into())),
                ));
            }
            run.state = LaunchState::Ready { created_unix_ms };
            Some((run, TaskStatus::Pending(CreateProgress::SubstrateStarted)))
        }
        LaunchState::Ready { created_unix_ms } => {
            let substrate = substrate_of(
                run.ctx.clone(),
                run.host.clone(),
                Arc::clone(&run.connector),
                created_unix_ms,
            );
            Some((run, TaskStatus::Ready(Ok(substrate))))
        }
        LaunchState::Done => None,
    }
}

/// The substrate of one started container.
fn substrate_of(
    ctx: Ctx,
    host: CloudflareHost,
    connector: Arc<dyn SlaveConnector>,
    created_unix_ms: u64,
) -> Substrate {
    let endpoint = SlaveEndpoint::CloudflarePort {
        port: host.config.slave_port,
    };
    Substrate {
        endpoint: endpoint.clone(),
        root: RootSetup {
            source: RootSource::Plain,
            mode: RootMode::Overlay,
        },
        mounts: Vec::new(),
        handle: Box::new(CloudflareSubstrateHandle {
            ctx,
            host,
            connector,
            endpoint,
            created_unix_ms,
            stopped: Arc::new(AtomicBool::new(false)),
        }),
        calls: None,
        slave: None,
    }
}

impl SandboxDriver for CloudflareContainerDriver {
    fn id(&self) -> DriverId {
        DriverId::CloudflareContainer
    }

    fn probe(&self, _ctx: Ctx) -> BoxFuture<'static, HostReport> {
        Box::pin(async move {
            HostReport {
                driver: DriverId::CloudflareContainer,
                usable: true,
                checks: vec![HostCheck {
                    name: "container_binding".to_string(),
                    ok: true,
                    message: "the Durable Object has a container".to_string(),
                }],
                capabilities: Self::capabilities(),
            }
        })
    }

    fn launch(
        &self,
        ctx: Ctx,
        _id: SandboxId,
        spec: SandboxSpec,
        _deadline: foundation_compact::Instant,
    ) -> LaunchStream {
        let run = LaunchRun {
            ctx: ctx.clone(),
            host: self.host.clone(),
            connector: Arc::clone(&self.connector),
            state: LaunchState::Check {
                spec,
                created_unix_ms: created_unix_ms(&ctx),
            },
        };
        match foundation_core::valtron::execute(launch_container::new(run), None) {
            Ok(iterator) => Box::new(SendIterator(SendWrapper::new(iterator))),
            Err(error) => Box::new(std::iter::once(Stream::Next(Err(SandboxError::Io {
                op: "launch".to_string(),
                message: error.to_string(),
            }
            .into())))),
        }
    }

    fn attach(
        &self,
        ctx: Ctx,
        record: &StoredRecord,
    ) -> BoxFuture<'_, SandboxResult<Option<Substrate>>> {
        let running = self.host.container.running();
        let created = record.created_unix_ms;
        let substrate = running.then(|| self.substrate(ctx, created));
        Box::pin(async move { Ok(substrate) })
    }

    fn containers(&self) -> Option<Arc<dyn ContainerControl>> {
        let control: Arc<dyn ContainerControl> = self.control.clone();
        Some(control)
    }
}

/// The handle of one Cloudflare container substrate.
struct CloudflareSubstrateHandle {
    ctx: Ctx,
    host: CloudflareHost,
    connector: Arc<dyn SlaveConnector>,
    endpoint: SlaveEndpoint,
    created_unix_ms: u64,
    /// Whether this handle already destroyed the container.
    ///
    /// A stop can destroy the container and then call `cleanup`. The flag keeps
    /// the second call from asking the host about a container that is gone.
    stopped: Arc<AtomicBool>,
}

impl CloudflareSubstrateHandle {
    fn container_id(&self) -> Option<ContainerId> {
        ContainerId::new(self.host.config.name.clone()).ok()
    }
}

impl SubstrateHandle for CloudflareSubstrateHandle {
    fn exited(&self) -> BoxFuture<'_, SubstrateExit> {
        let container = Arc::clone(&self.host.container);
        Box::pin(async move {
            if let Err(error) = container.wait_for_exit().await {
                tracing::warn!(error = %error, "the container exit wait failed");
            }
            SubstrateExit {
                code: None,
                signal: None,
                forced: false,
            }
        })
    }

    fn terminate(&self, grace: Duration) -> BoxFuture<'_, SandboxResult<SubstrateExit>> {
        let ctx = self.ctx.clone();
        let container = Arc::clone(&self.host.container);
        let stopped = Arc::clone(&self.stopped);
        Box::pin(async move {
            let deadline = ctx.clock().now() + grace;
            let exit = container.wait_for_exit();
            let timer = wait_until(deadline, &ctx);
            let forced = match futures::future::select(Box::pin(exit), Box::pin(timer)).await {
                futures::future::Either::Left((_exit, _timer)) => false,
                futures::future::Either::Right((_timer, _exit)) => true,
            };
            let _ = container.destroy().await;
            stopped.store(true, Ordering::SeqCst);
            Ok(SubstrateExit {
                code: None,
                signal: None,
                forced,
            })
        })
    }

    fn kill(&self) -> BoxFuture<'_, SandboxResult<SubstrateExit>> {
        let container = Arc::clone(&self.host.container);
        let stopped = Arc::clone(&self.stopped);
        Box::pin(async move {
            let _ = container.destroy().await;
            stopped.store(true, Ordering::SeqCst);
            Ok(SubstrateExit {
                code: None,
                signal: None,
                forced: true,
            })
        })
    }

    fn log_tail(&self, _max: usize) -> BoxFuture<'_, Vec<u8>> {
        // The platform gives no container log through this API. The error
        // message of a failed start says "see the Worker logs".
        Box::pin(async { Vec::new() })
    }

    fn cleanup(&self) -> BoxFuture<'_, SandboxResult<()>> {
        let container = Arc::clone(&self.host.container);
        let stopped = Arc::clone(&self.stopped);
        Box::pin(async move {
            // Do not ask `running`: the container may already be gone, and the
            // Cloudflare host throws `Critical error` instead of answering false
            // (issue #1125). A container that a stop already destroyed needs no
            // second destroy; a failed destroy of a gone container is logged and
            // does not fail the stop.
            if !stopped.swap(true, Ordering::SeqCst) {
                if let Err(error) = container.destroy().await {
                    tracing::warn!(error = %error, "the container cleanup did not finish");
                }
            }
            Ok(())
        })
    }

    fn container(&self) -> Option<ContainerHandle> {
        let id = self.container_id()?;
        let ops = DoContainerOps::new(
            self.ctx.clone(),
            self.host.clone(),
            Some(self.endpoint.clone()),
            Arc::clone(&self.connector),
            id.clone(),
            self.created_unix_ms,
            ContainerResources::default(),
            SandboxLimits::default(),
        );
        Some(ContainerHandle::new(id, Arc::new(ops)))
    }
}

/// The model slave driver of decision D16.
///
/// It starts no container. Each command is one model turn in a second Durable
/// Object.
pub struct CloudflareDoDriver {
    binding: String,
    model: Option<crate::shared::cloudflare::ModelConfig>,
}

impl CloudflareDoDriver {
    /// A driver that names the service binding of the model slave.
    #[must_use]
    pub fn new(config: &CloudflareConfig) -> Self {
        Self {
            binding: config.slave_binding.clone(),
            model: config.model.clone(),
        }
    }

    /// What this driver can do.
    #[must_use]
    pub fn capabilities() -> Capabilities {
        Capabilities {
            rootfs_directory: false,
            rootfs_ext4: false,
            rootfs_squashfs: false,
            rootfs_container_image: true,
            root_modes: vec![RootMode::Overlay],
            host_mounts: false,
            network_none: true,
            network_host: true,
            max_vcpus: 1,
        }
    }

    fn substrate(&self, id: SandboxId) -> Substrate {
        Substrate {
            endpoint: SlaveEndpoint::CloudflareDo {
                binding: self.binding.clone(),
                name: id,
            },
            root: RootSetup {
                source: RootSource::Plain,
                mode: RootMode::Overlay,
            },
            mounts: Vec::new(),
            handle: Box::new(ModelSubstrateHandle),
            calls: None,
            slave: None,
        }
    }
}

impl SandboxDriver for CloudflareDoDriver {
    fn id(&self) -> DriverId {
        DriverId::CloudflareDo
    }

    fn probe(&self, _ctx: Ctx) -> BoxFuture<'static, HostReport> {
        Box::pin(async move {
            HostReport {
                driver: DriverId::CloudflareDo,
                usable: true,
                checks: vec![HostCheck {
                    name: "service_binding".to_string(),
                    ok: true,
                    message: "the model slave has a service binding".to_string(),
                }],
                capabilities: Self::capabilities(),
            }
        })
    }

    fn launch(
        &self,
        _ctx: Ctx,
        id: SandboxId,
        _spec: SandboxSpec,
        _deadline: foundation_compact::Instant,
    ) -> LaunchStream {
        let backend_ready = self
            .model
            .as_ref()
            .and_then(|config| config.backend.as_ref())
            .is_some();
        if !backend_ready {
            return Box::new(std::iter::once(Stream::Next(Err(
                SandboxError::UnsupportedCapability {
                    capability: "model backend".to_string(),
                }
                .into(),
            ))));
        }
        let substrate = self.substrate(id);
        Box::new(
            std::iter::once(Stream::Pending(CreateProgress::Preparing))
                .chain(std::iter::once(Stream::Pending(
                    CreateProgress::SubstrateStarted,
                )))
                .chain(std::iter::once(Stream::Next(Ok(substrate)))),
        )
    }

    fn attach(
        &self,
        _ctx: Ctx,
        record: &StoredRecord,
    ) -> BoxFuture<'_, SandboxResult<Option<Substrate>>> {
        let substrate = match &record.endpoint {
            Some(SlaveEndpoint::CloudflareDo { .. }) => Some(self.substrate(record.id.clone())),
            _ => None,
        };
        Box::pin(async move { Ok(substrate) })
    }
}

/// The handle of a model substrate: it has no container.
struct ModelSubstrateHandle;

impl SubstrateHandle for ModelSubstrateHandle {
    fn exited(&self) -> BoxFuture<'_, SubstrateExit> {
        Box::pin(futures::future::pending())
    }

    fn terminate(&self, _grace: Duration) -> BoxFuture<'_, SandboxResult<SubstrateExit>> {
        Box::pin(async {
            Ok(SubstrateExit {
                code: None,
                signal: None,
                forced: false,
            })
        })
    }

    fn kill(&self) -> BoxFuture<'_, SandboxResult<SubstrateExit>> {
        Box::pin(async {
            Ok(SubstrateExit {
                code: None,
                signal: None,
                forced: true,
            })
        })
    }

    fn log_tail(&self, _max: usize) -> BoxFuture<'_, Vec<u8>> {
        Box::pin(async { Vec::new() })
    }

    fn cleanup(&self) -> BoxFuture<'_, SandboxResult<()>> {
        Box::pin(async { Ok(()) })
    }
}
