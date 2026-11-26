//! The Cloudflare container control and its operations.
//!
//! [`CloudflareContainerControl`] is the public factory. [`DoContainerOps`] is
//! the server side inside the Durable Object. [`DoContainerClient`] is the
//! remote client over the internal `ContainerService`.

use std::sync::Arc;
use std::time::Duration;

use foundation_connectrpc::shared::transport::Transport;
use foundation_connectrpc::{ClientOptions, Request, RpcCtx};
use foundation_context::Ctx;
use foundation_errstacks::ErrorTrace;
use futures::StreamExt;

use crate::shared::cloudflare::{CloudflareHost, ContainerStart, PortError};
use crate::shared::command::{Command, ExecEvent, ExitStatus, StdinMode};
use crate::shared::container::{
    ContainerCapabilities, ContainerControl, ContainerHandle, ContainerId, ContainerInfo,
    ContainerOps, ContainerResources, ContainerSpec, ContainerStatus, PortRequest, PortResponse,
};
use crate::shared::control::record::created_unix_ms;
use crate::shared::control::send_box;
use crate::shared::control::slave_client::{event_stream, SlaveClient};
use crate::shared::control::wait_until;
use crate::shared::driver::{SlaveEndpoint, SubstrateExit};
use crate::shared::error::{SandboxError, SandboxResult};
use crate::shared::handles::{ExecEvents, ExecHandle, ExecHandleOps, ExecStdin};
use crate::shared::host::SlaveConnector;
use crate::shared::ids::{DriverId, ExecId};
use crate::shared::limits::SandboxLimits;
use crate::shared::rpc::messages::{
    ContainerDestroyRequest, ContainerExecRequest, ContainerFetchEvent, ContainerFetchRequest,
    ContainerLogTailRequest, ContainerSignalRequest, ContainerStopRequest, ContainerWaitRequest,
    ExecEventKind, ExecEventMsg, ExecRequest,
};
use crate::shared::rpc::ContainerServiceClient;
use crate::shared::{BoxByteStream, BoxFuture, Secret};

/// The address that the remote container client uses in the request line.
const CONTAINER_AUTHORITY: &str = "http://container.local";

/// The public container control of one Durable Object.
pub struct CloudflareContainerControl {
    host: CloudflareHost,
    connector: Arc<dyn SlaveConnector>,
    limits: SandboxLimits,
    started_unix_ms: std::sync::Mutex<Option<u64>>,
}

impl CloudflareContainerControl {
    /// A control over one DO bundle and its slave connector.
    #[must_use]
    pub fn new(host: CloudflareHost, connector: Arc<dyn SlaveConnector>) -> Self {
        Self {
            host,
            connector,
            limits: SandboxLimits::default(),
            started_unix_ms: std::sync::Mutex::new(None),
        }
    }

    /// The same control with other limits.
    #[must_use]
    pub fn with_limits(mut self, limits: SandboxLimits) -> Self {
        self.limits = limits;
        self
    }

    /// The configured container identifier.
    fn container_id(&self) -> Option<ContainerId> {
        ContainerId::new(self.host.config.name.clone()).ok()
    }

    fn created_unix_ms(&self, ctx: &Ctx) -> u64 {
        self.started_unix_ms
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .unwrap_or_else(|| created_unix_ms(ctx))
    }

    fn handle(
        &self,
        ctx: &Ctx,
        id: ContainerId,
        endpoint: Option<SlaveEndpoint>,
    ) -> ContainerHandle {
        let ops = DoContainerOps::new(
            ctx.clone(),
            self.host.clone(),
            endpoint,
            Arc::clone(&self.connector),
            id.clone(),
            self.created_unix_ms(ctx),
            ContainerResources::default(),
            self.limits.clone(),
        );
        ContainerHandle::new(id, Arc::new(ops))
    }
}

impl ContainerControl for CloudflareContainerControl {
    fn start(
        &self,
        ctx: Ctx,
        spec: ContainerSpec,
    ) -> BoxFuture<'_, SandboxResult<ContainerHandle>> {
        let result: SandboxResult<ContainerId> = (|| {
            if spec.image != self.host.config.image_name {
                return Err(SandboxError::UnsupportedRootFs.into());
            }
            let id = self.container_id().ok_or_else(|| {
                ErrorTrace::new(SandboxError::InvalidSpec {
                    field: "container_id".to_string(),
                    reason: "the DO name is not a valid container identifier".to_string(),
                })
            })?;
            Ok(id)
        })();
        let host = self.host.clone();
        let connector = Arc::clone(&self.connector);
        let limits = self.limits.clone();
        let started = self.created_unix_ms(&ctx);
        Box::pin(async move {
            let id = result?;
            let slave_entrypoint = host.config.slave_entrypoint();
            let entrypoint = if spec.entrypoint.is_empty() {
                slave_entrypoint.clone()
            } else {
                spec.entrypoint.clone()
            };
            let runs_slave = entrypoint == slave_entrypoint;
            let start = ContainerStart {
                entrypoint,
                enable_internet: spec.network == crate::shared::spec::NetworkMode::Host,
                env: spec.env.clone(),
            };
            host.container.start(start)?;
            let endpoint = runs_slave.then(|| SlaveEndpoint::CloudflarePort {
                port: host.config.slave_port,
            });
            let ops = DoContainerOps::new(
                ctx,
                host,
                endpoint,
                connector,
                id.clone(),
                started,
                spec.resources,
                limits,
            );
            Ok(ContainerHandle::new(id, Arc::new(ops)))
        })
    }

    fn get(
        &self,
        ctx: Ctx,
        id: &ContainerId,
    ) -> BoxFuture<'_, SandboxResult<Option<ContainerHandle>>> {
        let running = self.host.container.running();
        let configured = self.container_id();
        let handle = running
            .then(|| configured.filter(|configured| configured == id))
            .flatten()
            .map(|id| {
                self.handle(
                    &ctx,
                    id,
                    Some(SlaveEndpoint::CloudflarePort {
                        port: self.host.config.slave_port,
                    }),
                )
            });
        Box::pin(async move { Ok(handle) })
    }

    fn list(&self, ctx: Ctx) -> BoxFuture<'_, SandboxResult<Vec<ContainerInfo>>> {
        let running = self.host.container.running();
        let containers = if running {
            self.container_id().map_or_else(Vec::new, |id| {
                vec![self
                    .handle(
                        &ctx,
                        id,
                        Some(SlaveEndpoint::CloudflarePort {
                            port: self.host.config.slave_port,
                        }),
                    )
                    .info()]
            })
        } else {
            Vec::new()
        };
        Box::pin(async move { Ok(containers) })
    }
}

/// The operations of one container inside the Durable Object.
pub struct DoContainerOps {
    ctx: Ctx,
    host: CloudflareHost,
    endpoint: Option<SlaveEndpoint>,
    connector: Arc<dyn SlaveConnector>,
    id: ContainerId,
    created_unix_ms: u64,
    resources: ContainerResources,
    limits: SandboxLimits,
    destroyed: Arc<std::sync::atomic::AtomicBool>,
}

impl DoContainerOps {
    /// The operations of one container.
    #[must_use]
    #[allow(
        clippy::too_many_arguments,
        reason = "the operations need the host, the slave route and the contract values"
    )]
    pub fn new(
        ctx: Ctx,
        host: CloudflareHost,
        endpoint: Option<SlaveEndpoint>,
        connector: Arc<dyn SlaveConnector>,
        id: ContainerId,
        created_unix_ms: u64,
        resources: ContainerResources,
        limits: SandboxLimits,
    ) -> Self {
        Self {
            ctx,
            host,
            endpoint,
            connector,
            id,
            created_unix_ms,
            resources,
            limits,
            destroyed: Arc::new(std::sync::atomic::AtomicBool::new(false)),
        }
    }

    fn status(&self) -> ContainerStatus {
        if self.destroyed.load(std::sync::atomic::Ordering::SeqCst) {
            return ContainerStatus::Stopped;
        }
        if self.host.container.running() {
            ContainerStatus::Running
        } else {
            ContainerStatus::Stopped
        }
    }

    /// Refuses a call on a container that this handle already destroyed.
    fn ensure_live(&self) -> SandboxResult<()> {
        if self.destroyed.load(std::sync::atomic::Ordering::SeqCst) {
            return Err(SandboxError::NotFound {
                what: "container".to_string(),
            }
            .into());
        }
        Ok(())
    }
}

impl ContainerOps for DoContainerOps {
    fn info(&self) -> ContainerInfo {
        ContainerInfo {
            id: self.id.clone(),
            driver: DriverId::CloudflareContainer,
            image: self.host.config.image_name.clone(),
            status: self.status(),
            created_unix_ms: self.created_unix_ms,
            resources: self.resources.clone(),
        }
    }

    fn capabilities(&self) -> ContainerCapabilities {
        ContainerCapabilities {
            exec: self.endpoint.is_some(),
            port_fetch: true,
            signal: true,
            logs: false,
        }
    }

    fn fetch(&self, port: u16, request: PortRequest) -> BoxFuture<'_, SandboxResult<PortResponse>> {
        let live = self.ensure_live();
        let container = Arc::clone(&self.host.container);
        send_box(async move {
            live?;
            container
                .fetch(port, request)
                .await
                .map_err(|error| port_error(&error))
        })
    }

    fn exec(&self, command: Command) -> BoxFuture<'_, SandboxResult<ExecHandle>> {
        let live = self.ensure_live();
        let endpoint = self.endpoint.clone();
        let connector = Arc::clone(&self.connector);
        let ctx = self.ctx.clone();
        let limits = self.limits.clone();
        send_box(async move {
            live?;
            let Some(endpoint) = endpoint else {
                return Err(SandboxError::UnsupportedCapability {
                    capability: "exec".to_string(),
                }
                .into());
            };
            let transport = connector.connect(&ctx, &endpoint, &limits)?;
            let client = SlaveClient::new(ctx.clone(), transport, limits.clone())?;
            crate::shared::control::exec_session(ctx, limits, Arc::new(client), command).await
        })
    }

    fn signal(&self, signo: i32) -> BoxFuture<'_, SandboxResult<()>> {
        let live = self.ensure_live();
        let result = self.host.container.signal(signo);
        send_box(async move {
            live?;
            result
        })
    }

    fn wait(&self) -> BoxFuture<'_, SandboxResult<SubstrateExit>> {
        let live = self.ensure_live();
        let container = Arc::clone(&self.host.container);
        send_box(async move {
            live?;
            container.wait_for_exit().await?;
            Ok(SubstrateExit {
                code: None,
                signal: None,
                forced: false,
            })
        })
    }

    fn stop(&self, grace: Duration) -> BoxFuture<'_, SandboxResult<SubstrateExit>> {
        let live = self.ensure_live();
        let container = Arc::clone(&self.host.container);
        let ctx = self.ctx.clone();
        let destroyed = Arc::clone(&self.destroyed);
        send_box(async move {
            live?;
            let deadline = ctx.clock().now() + grace;
            let exit = container.wait_for_exit();
            let timer = wait_until(deadline, &ctx);
            let forced = match futures::future::select(exit, Box::pin(timer)).await {
                futures::future::Either::Left((_exit, _timer)) => false,
                futures::future::Either::Right((_timer, _exit)) => true,
            };
            let _ = container.destroy().await;
            destroyed.store(true, std::sync::atomic::Ordering::SeqCst);
            Ok(SubstrateExit {
                code: None,
                signal: None,
                forced,
            })
        })
    }

    fn destroy(&self) -> BoxFuture<'_, SandboxResult<()>> {
        let container = Arc::clone(&self.host.container);
        let destroyed = Arc::clone(&self.destroyed);
        send_box(async move {
            if container.running() {
                container.destroy().await?;
            }
            destroyed.store(true, std::sync::atomic::Ordering::SeqCst);
            Ok(())
        })
    }

    fn log_tail(&self, _max: usize) -> BoxFuture<'_, SandboxResult<Vec<u8>>> {
        let live = self.ensure_live();
        send_box(async move {
            live?;
            Ok(Vec::new())
        })
    }
}

/// A port error as a sandbox error.
fn port_error(error: &PortError) -> ErrorTrace<SandboxError> {
    SandboxError::Rpc {
        code: "unavailable".to_string(),
        message: error.to_string(),
    }
    .into()
}

/// The remote client of one container over the internal `ContainerService`.
pub struct DoContainerClient {
    ctx: Ctx,
    transport: Arc<dyn Transport>,
    client: Arc<ContainerServiceClient>,
    limits: SandboxLimits,
    info: ContainerInfo,
    wait_timeout: Option<Duration>,
}

impl DoContainerClient {
    /// A client over a transport to a Durable Object.
    ///
    /// # Panics
    /// When the JSON client cannot be built. The options are fixed and valid,
    /// so this is a programming error and not a run-time failure.
    #[must_use]
    pub fn new(ctx: Ctx, transport: Arc<dyn Transport>, limits: SandboxLimits) -> Self {
        let client = ContainerServiceClient::new(
            Arc::clone(&transport),
            CONTAINER_AUTHORITY,
            ClientOptions::new().with_codec("json"),
        )
        .expect("the JSON ContainerService client builds");
        let info = ContainerInfo {
            id: ContainerId::new("sandbox".to_string()).expect("sandbox is a valid container id"),
            driver: DriverId::CloudflareContainer,
            image: "sandbox".to_string(),
            status: ContainerStatus::Running,
            created_unix_ms: 0,
            resources: ContainerResources::default(),
        };
        Self {
            ctx,
            transport,
            client: Arc::new(client),
            limits,
            info,
            wait_timeout: None,
        }
    }

    /// The same client with the container information of the remote DO.
    #[must_use]
    pub fn with_info(mut self, info: ContainerInfo) -> Self {
        self.info = info;
        self
    }

    /// The same client with the deadline of a remote `Wait`.
    #[must_use]
    pub fn with_wait_timeout(mut self, value: Option<Duration>) -> Self {
        self.wait_timeout = value;
        self
    }

    /// The same client, with the bearer token on every request.
    ///
    /// # Panics
    /// When the JSON client cannot be built. The options are fixed and valid,
    /// so this is a programming error and not a run-time failure.
    #[must_use]
    #[allow(
        clippy::needless_pass_by_value,
        reason = "the API takes ownership of the secret"
    )]
    pub fn with_token(self, token: Secret) -> Self {
        let header = foundation_auth::server_credential::bearer_header(&token);
        let client = ContainerServiceClient::new(
            Arc::clone(&self.transport),
            CONTAINER_AUTHORITY,
            ClientOptions::new()
                .with_codec("json")
                .with_header("AUTHORIZATION".to_string(), header),
        )
        .expect("the JSON ContainerService client builds");
        Self {
            ctx: self.ctx,
            transport: self.transport,
            client: Arc::new(client),
            limits: self.limits,
            info: self.info,
            wait_timeout: self.wait_timeout,
        }
    }

    fn rpc_ctx(&self, timeout: Duration) -> RpcCtx {
        RpcCtx::client(self.ctx.clone()).with_deadline(timeout)
    }

    fn stream_ctx(&self) -> RpcCtx {
        RpcCtx::client(self.ctx.clone())
    }

    fn container_id(&self) -> String {
        self.info.id.to_string()
    }
}

impl ContainerOps for DoContainerClient {
    fn info(&self) -> ContainerInfo {
        self.info.clone()
    }

    fn capabilities(&self) -> ContainerCapabilities {
        ContainerCapabilities {
            exec: true,
            port_fetch: true,
            signal: true,
            logs: false,
        }
    }

    fn fetch(&self, port: u16, request: PortRequest) -> BoxFuture<'_, SandboxResult<PortResponse>> {
        let client = Arc::clone(&self.client);
        let rpc_ctx = self.stream_ctx();
        let container_id = self.container_id();
        send_box(async move {
            let mut stream = client
                .fetch(
                    rpc_ctx,
                    Request::new(ContainerFetchRequest {
                        container_id,
                        port,
                        request,
                    }),
                )
                .await
                .map_err(|error| {
                    ErrorTrace::new(SandboxError::from_connect(error.current_context()))
                })?;
            let head = stream.receive().await.map_err(|error| {
                ErrorTrace::new(SandboxError::from_connect(error.current_context()))
            })?;
            let (status, headers) = match head {
                Some(ContainerFetchEvent::Head { status, headers }) => (status, headers),
                Some(ContainerFetchEvent::Chunk { .. }) | None => {
                    return Err(ErrorTrace::new(SandboxError::StreamLost));
                }
            };
            let body = fetch_body(stream);
            Ok(PortResponse {
                status,
                headers,
                body,
            })
        })
    }

    fn exec(&self, command: Command) -> BoxFuture<'_, SandboxResult<ExecHandle>> {
        let client = Arc::clone(&self.client);
        let ctx = self.ctx.clone();
        let rpc_ctx = self.stream_ctx();
        let container_id = self.container_id();
        remote_exec(ctx, rpc_ctx, client, container_id, command)
    }

    fn signal(&self, signo: i32) -> BoxFuture<'_, SandboxResult<()>> {
        let client = Arc::clone(&self.client);
        let rpc_ctx = self.rpc_ctx(self.limits.rpc_timeout);
        let container_id = self.container_id();
        send_box(async move {
            client
                .signal(
                    rpc_ctx,
                    Request::new(ContainerSignalRequest {
                        container_id,
                        signo,
                    }),
                )
                .await
                .map(|_response| ())
                .map_err(|error| {
                    ErrorTrace::new(SandboxError::from_connect(error.current_context()))
                })
        })
    }

    fn wait(&self) -> BoxFuture<'_, SandboxResult<SubstrateExit>> {
        let client = Arc::clone(&self.client);
        let ctx = self.ctx.clone();
        let rpc_ctx = self.stream_ctx();
        let container_id = self.container_id();
        let wait_timeout = self.wait_timeout;
        send_box(async move {
            let wait = client.wait(rpc_ctx, Request::new(ContainerWaitRequest { container_id }));
            let Some(timeout) = wait_timeout else {
                return wait.await.map(|response| response.msg).map_err(|error| {
                    ErrorTrace::new(SandboxError::from_connect(error.current_context()))
                });
            };
            let deadline = ctx.clock().now() + timeout;
            let timer = wait_until(deadline, &ctx);
            match futures::future::select(Box::pin(wait), Box::pin(timer)).await {
                futures::future::Either::Left((result, _timer)) => {
                    result.map(|response| response.msg).map_err(|error| {
                        ErrorTrace::new(SandboxError::from_connect(error.current_context()))
                    })
                }
                futures::future::Either::Right((_timer, _wait)) => {
                    Err(ErrorTrace::new(SandboxError::Rpc {
                        code: "deadline_exceeded".to_string(),
                        message: "the container wait passed its deadline".to_string(),
                    }))
                }
            }
        })
    }

    fn stop(&self, grace: Duration) -> BoxFuture<'_, SandboxResult<SubstrateExit>> {
        let client = Arc::clone(&self.client);
        let rpc_ctx = self.rpc_ctx(self.limits.rpc_timeout);
        let container_id = self.container_id();
        let grace_ms = u64::try_from(grace.as_millis()).unwrap_or(u64::MAX);
        send_box(async move {
            client
                .stop(
                    rpc_ctx,
                    Request::new(ContainerStopRequest {
                        container_id,
                        grace_ms,
                    }),
                )
                .await
                .map(|response| response.msg)
                .map_err(|error| {
                    ErrorTrace::new(SandboxError::from_connect(error.current_context()))
                })
        })
    }

    fn destroy(&self) -> BoxFuture<'_, SandboxResult<()>> {
        let client = Arc::clone(&self.client);
        let rpc_ctx = self.rpc_ctx(self.limits.rpc_timeout);
        let container_id = self.container_id();
        send_box(async move {
            client
                .destroy(
                    rpc_ctx,
                    Request::new(ContainerDestroyRequest { container_id }),
                )
                .await
                .map(|_response| ())
                .map_err(|error| {
                    ErrorTrace::new(SandboxError::from_connect(error.current_context()))
                })
        })
    }

    fn log_tail(&self, max: usize) -> BoxFuture<'_, SandboxResult<Vec<u8>>> {
        let client = Arc::clone(&self.client);
        let rpc_ctx = self.rpc_ctx(self.limits.rpc_timeout);
        let container_id = self.container_id();
        send_box(async move {
            client
                .log_tail(
                    rpc_ctx,
                    Request::new(ContainerLogTailRequest { container_id, max }),
                )
                .await
                .map(|response| response.msg.data)
                .map_err(|error| {
                    ErrorTrace::new(SandboxError::from_connect(error.current_context()))
                })
        })
    }
}

/// The body stream of one remote container fetch.
fn fetch_body(stream: foundation_connectrpc::ServerStream<ContainerFetchEvent>) -> BoxByteStream {
    let inner = futures::stream::unfold(Some(stream), |state| async move {
        let mut stream = state?;
        loop {
            match stream.receive().await {
                Ok(Some(ContainerFetchEvent::Chunk { data })) => {
                    return Some((Ok(data), Some(stream)));
                }
                Ok(Some(ContainerFetchEvent::Head { .. })) => {}
                Ok(None) => return None,
                Err(error) => {
                    return Some((
                        Err(ErrorTrace::new(SandboxError::from_connect(
                            error.current_context(),
                        ))),
                        None,
                    ));
                }
            }
        }
    });
    Box::pin(crate::shared::cloudflare::SendStream::new(inner))
}

/// The remote exec call, split out so the future stays readable.
fn remote_exec(
    ctx: Ctx,
    rpc_ctx: RpcCtx,
    client: Arc<ContainerServiceClient>,
    container_id: String,
    command: Command,
) -> BoxFuture<'static, SandboxResult<ExecHandle>> {
    send_box(async move {
        let exec_id = ExecId::generate(&ctx);
        let stdin = !matches!(&command.stdin, StdinMode::Null);
        let request = ExecRequest {
            exec_id: exec_id.to_string(),
            argv: command.argv,
            env: command.env,
            cwd: command.cwd,
            stdin,
        };
        let stream = client
            .exec(
                rpc_ctx,
                Request::new(ContainerExecRequest {
                    container_id,
                    request,
                }),
            )
            .await
            .map_err(|error| {
                ErrorTrace::new(SandboxError::from_connect(error.current_context()))
            })?;
        let events = event_stream(stream);
        let ops = RemoteExecOps::new(events);
        Ok(ExecHandle::new(exec_id, Arc::new(ops)))
    })
}

/// The handle of one execution on a remote container.
struct RemoteExecOps {
    stream: std::sync::Mutex<Option<crate::shared::ExecEventStream>>,
    exit: Arc<std::sync::Mutex<Option<ExitStatus>>>,
}

impl RemoteExecOps {
    fn new(stream: crate::shared::ExecEventStream) -> Self {
        Self {
            stream: std::sync::Mutex::new(Some(stream)),
            exit: Arc::new(std::sync::Mutex::new(None)),
        }
    }
}

impl ExecHandleOps for RemoteExecOps {
    fn pid(&self) -> u32 {
        0
    }

    fn stdin(&self) -> Option<ExecStdin> {
        None
    }

    fn events(&self) -> ExecEvents {
        let stream = self
            .stream
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .take();
        let Some(stream) = stream else {
            return ExecEvents::new(Box::pin(futures::stream::empty()));
        };
        let exit = Arc::clone(&self.exit);
        let state = futures::stream::unfold((Some(stream), exit), |(stream, exit)| async move {
            let mut stream = stream?;
            match stream.next().await {
                Some(Ok(message)) => {
                    let event = match message.event {
                        ExecEventKind::Stdout { data } => ExecEvent::Stdout(data),
                        ExecEventKind::Stderr { data } => ExecEvent::Stderr(data),
                        ExecEventKind::Exit { code, signal } => {
                            let status = match (code, signal) {
                                (Some(code), _) => ExitStatus::code(code),
                                (None, Some(signal)) => ExitStatus::signal(signal),
                                (None, None) => ExitStatus::code(0),
                            };
                            *exit
                                .lock()
                                .unwrap_or_else(std::sync::PoisonError::into_inner) = Some(status);
                            ExecEvent::Exit(status)
                        }
                        ExecEventKind::Started { .. } => {
                            return Some((Err(SandboxError::StreamLost.into()), (None, exit)));
                        }
                    };
                    Some((Ok(event), (Some(stream), exit)))
                }
                Some(Err(error)) => Some((Err(error), (None, exit))),
                None => None,
            }
        });
        ExecEvents::new(Box::pin(state))
    }

    fn signal(&self, _signal: i32) -> BoxFuture<'_, SandboxResult<()>> {
        send_box(async {
            Err(SandboxError::UnsupportedCapability {
                capability: "exec signal".to_string(),
            }
            .into())
        })
    }

    fn wait(&self) -> BoxFuture<'_, SandboxResult<ExitStatus>> {
        let stream = self
            .stream
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .take();
        let exit = Arc::clone(&self.exit);
        send_box(async move {
            if let Some(stream) = stream {
                let mut stream = stream;
                while let Some(item) = stream.next().await {
                    if let Ok(ExecEventMsg {
                        event: ExecEventKind::Exit { code, signal },
                        ..
                    }) = item
                    {
                        let status = match (code, signal) {
                            (Some(code), _) => ExitStatus::code(code),
                            (None, Some(signal)) => ExitStatus::signal(signal),
                            (None, None) => ExitStatus::code(0),
                        };
                        *exit
                            .lock()
                            .unwrap_or_else(std::sync::PoisonError::into_inner) = Some(status);
                    }
                }
            }
            exit.lock()
                .unwrap_or_else(std::sync::PoisonError::into_inner)
                .ok_or_else(|| SandboxError::StreamLost.into())
        })
    }
}
