//! `SandboxService` on a control plane, and its client.

use std::pin::Pin;
use std::sync::Arc;
use std::time::Duration;

use foundation_connectrpc::shared::transport::Transport;
use foundation_connectrpc::{
    ClientOptions, ConnectError, ConnectResult, Request, Response, RpcCtx, ServerStream,
};
use foundation_context::Ctx;
use foundation_core::valtron::{task, Stream, StreamIteratorExt, TaskStatus};
use foundation_errstacks::ErrorTrace;
use futures::{Stream as FuturesStream, StreamExt};

use crate::shared::control::handle::{ScopedCalls, SendIterator};
use crate::shared::control::send_box;
use crate::shared::control::{ExitReport, RemoteSandboxOps, SandboxHandle, SandboxInfo};
use crate::shared::error::{SandboxError, SandboxResult};
use crate::shared::rpc::messages::{
    CreateEvent, CreateRequest, Empty, ExecEventMsg, ExecInputRequest, ExecInputResponse,
    ExecRequest, ExecSignalRequest, FileChunk, FileReadRequest, FileWriteAbortRequest,
    FileWriteBeginRequest, FileWriteBeginResponse, FileWriteChunkRequest, FileWriteChunkResponse,
    FileWriteCommitRequest, FileWriteCommitResponse, ListResponse, ProbeResponse, SandboxRef,
    Scoped, UnpackAbortRequest, UnpackBeginRequest, UnpackBeginResponse, UnpackChunkRequest,
    UnpackChunkResponse, UnpackCommitRequest, UnpackCommitResponse,
};
use crate::shared::rpc::{SandboxService, SandboxServiceClient};
use crate::shared::{BoxFuture, Secret};

/// The address that the remote client uses in the request line.
const CONTROL_AUTHORITY: &str = "http://control.local";

/// Checks the `Authorization` header against the served token.
///
/// `header` is the raw `Authorization` header. A missing header, a scheme that
/// is not Bearer, and a wrong token are all [`SandboxError::Unauthorized`].
///
/// # Errors
/// [`SandboxError::Unauthorized`] when the header does not carry the token.
pub fn authorize(header: Option<&str>, token: &Secret) -> SandboxResult<()> {
    match Secret::from_bearer(header) {
        Some(provided) if provided.ct_eq(token) => Ok(()),
        _ => Err(SandboxError::Unauthorized.into()),
    }
}

/// A control plane with no sandbox: every call answers `NotFound`.
///
/// A slave without `--control` serves the control paths with this provider, so
/// a caller gets the typed `NotFound { what: "method" }` of feature 11,
/// invariant I4, and not a transport error.
pub struct DisabledControl;

/// The error of a call to a slave that cannot control.
fn disabled() -> foundation_errstacks::ErrorTrace<SandboxError> {
    SandboxError::NotFound {
        what: "method".to_string(),
    }
    .into()
}

impl crate::shared::control::SandboxProvider for DisabledControl {
    fn probe(&self) -> BoxFuture<'_, SandboxResult<Vec<crate::shared::driver::HostReport>>> {
        send_box(async { Err(disabled()) })
    }

    fn create(
        &self,
        _spec: crate::shared::spec::SandboxSpec,
    ) -> BoxFuture<'_, SandboxResult<SandboxHandle>> {
        send_box(async { Err(disabled()) })
    }

    fn create_with_progress(
        &self,
        _spec: crate::shared::spec::SandboxSpec,
    ) -> crate::shared::handles::CreateStream {
        let item: foundation_core::valtron::Stream<
            SandboxResult<SandboxHandle>,
            crate::shared::driver::CreateProgress,
        > = foundation_core::valtron::Stream::Next(Err(disabled()));
        crate::shared::handles::CreateStream::new(Box::new(std::iter::once(item)))
    }

    fn get(
        &self,
        _id: crate::shared::ids::SandboxId,
    ) -> BoxFuture<'_, SandboxResult<SandboxHandle>> {
        send_box(async { Err(disabled()) })
    }

    fn list(&self) -> BoxFuture<'_, SandboxResult<Vec<SandboxInfo>>> {
        send_box(async { Err(disabled()) })
    }

    fn stop(&self, _id: crate::shared::ids::SandboxId) -> BoxFuture<'_, SandboxResult<ExitReport>> {
        send_box(async { Err(disabled()) })
    }

    fn kill(&self, _id: crate::shared::ids::SandboxId) -> BoxFuture<'_, SandboxResult<ExitReport>> {
        send_box(async { Err(disabled()) })
    }
}

/// `SandboxService` on any control plane.
pub struct ControlService {
    control: Arc<dyn crate::shared::control::SandboxProvider>,
    limits: crate::shared::limits::SandboxLimits,
}

impl ControlService {
    /// A service over one provider and its limits.
    #[must_use]
    pub fn new(
        control: Arc<dyn crate::shared::control::SandboxProvider>,
        limits: crate::shared::limits::SandboxLimits,
    ) -> Self {
        Self { control, limits }
    }

    fn connect_error(&self, error: &ErrorTrace<SandboxError>) -> ErrorTrace<ConnectError> {
        ErrorTrace::new(
            error
                .current_context()
                .to_connect(self.limits.max_error_message),
        )
    }

    async fn ready_sandbox(&self, sandbox_id: &str) -> ConnectResult<SandboxHandle> {
        let id = sandbox_id.parse().map_err(|_| {
            ErrorTrace::<ConnectError>::new(
                SandboxError::NotFound {
                    what: "sandbox".to_string(),
                }
                .to_connect(self.limits.max_error_message),
            )
        })?;
        let handle = self
            .control
            .get(id)
            .await
            .map_err(|error| self.connect_error(&error))?;
        if handle.info().state != crate::shared::control::SandboxState::Ready {
            return Err(ErrorTrace::new(
                SandboxError::SandboxStopped.to_connect(self.limits.max_error_message),
            ));
        }
        Ok(handle)
    }
}

#[allow(
    clippy::unused_async_trait_impl,
    reason = "the service trait declares async methods; the bodies only await the control plane"
)]
impl SandboxService for ControlService {
    async fn probe(
        &self,
        _ctx: RpcCtx,
        _request: Request<Empty>,
    ) -> ConnectResult<Response<ProbeResponse>> {
        let reports = self
            .control
            .probe()
            .await
            .map_err(|error| self.connect_error(&error))?;
        Ok(Response::new(ProbeResponse { reports }))
    }

    async fn create(
        &self,
        ctx: RpcCtx,
        request: Request<CreateRequest>,
    ) -> ConnectResult<Pin<Box<dyn FuturesStream<Item = ConnectResult<CreateEvent>> + Send>>> {
        let iterator = self.control.create_with_progress(request.msg.spec);
        let poll_interval = self.limits.create_poll_interval;
        let max_error_message = self.limits.max_error_message;
        let call_ctx = ctx.ctx.clone();
        let stream = futures::stream::unfold(
            (iterator, call_ctx),
            move |(mut iterator, call_ctx)| async move {
                loop {
                    match iterator.next() {
                        None => return None,
                        Some(Stream::Pending(progress)) => {
                            return Some((
                                Ok(CreateEvent::Progress(progress)),
                                (iterator, call_ctx),
                            ));
                        }
                        Some(Stream::Next(result)) => {
                            let event = match result {
                                Ok(handle) => Ok(CreateEvent::Created(handle.info())),
                                Err(error) => Err(ErrorTrace::new(
                                    error.current_context().to_connect(max_error_message),
                                )),
                            };
                            return Some((event, (iterator, call_ctx)));
                        }
                        Some(Stream::Wait) => {
                            // The create progress iterator is a driving stream:
                            // it runs its producer inside `next()`. `Wait` means
                            // "the queue is empty; check again", not "the
                            // producer is asleep". Pulling it again in this poll
                            // is a synchronous loop. A wasm host has one thread,
                            // so that loop starves its event loop: the create
                            // can never finish and the Durable Object stops
                            // answering (issue #1125). Park on the injected timer
                            // so the host runs, then pull again.
                            if crate::shared::control::wait_until(
                                call_ctx.clock().now() + poll_interval,
                                &call_ctx,
                            )
                            .await
                            .is_err()
                            {
                                tracing::warn!(
                                    "the create stream has no timer to yield on; it polls at once"
                                );
                            }
                        }
                        Some(
                            Stream::Ignore | Stream::Init | Stream::Delayed(_) | Stream::Spread(_),
                        ) => {}
                    }
                }
            },
        );
        Ok(Box::pin(stream))
    }

    async fn get(
        &self,
        _ctx: RpcCtx,
        request: Request<SandboxRef>,
    ) -> ConnectResult<Response<SandboxInfo>> {
        let handle = self.ready_sandbox(&request.msg.sandbox_id).await?;
        Ok(Response::new(handle.info()))
    }

    async fn list(
        &self,
        _ctx: RpcCtx,
        _request: Request<Empty>,
    ) -> ConnectResult<Response<ListResponse>> {
        let sandboxes = self
            .control
            .list()
            .await
            .map_err(|error| self.connect_error(&error))?;
        Ok(Response::new(ListResponse { sandboxes }))
    }

    async fn stop(
        &self,
        _ctx: RpcCtx,
        request: Request<SandboxRef>,
    ) -> ConnectResult<Response<ExitReport>> {
        let id = request.msg.sandbox_id.parse().map_err(|_| {
            ErrorTrace::<ConnectError>::new(
                SandboxError::NotFound {
                    what: "sandbox".to_string(),
                }
                .to_connect(self.limits.max_error_message),
            )
        })?;
        let report = self
            .control
            .stop(id)
            .await
            .map_err(|error| self.connect_error(&error))?;
        Ok(Response::new(report))
    }

    async fn kill(
        &self,
        _ctx: RpcCtx,
        request: Request<SandboxRef>,
    ) -> ConnectResult<Response<ExitReport>> {
        let id = request.msg.sandbox_id.parse().map_err(|_| {
            ErrorTrace::<ConnectError>::new(
                SandboxError::NotFound {
                    what: "sandbox".to_string(),
                }
                .to_connect(self.limits.max_error_message),
            )
        })?;
        let report = self
            .control
            .kill(id)
            .await
            .map_err(|error| self.connect_error(&error))?;
        Ok(Response::new(report))
    }

    async fn exec(
        &self,
        _ctx: RpcCtx,
        request: Request<Scoped<ExecRequest>>,
    ) -> ConnectResult<Pin<Box<dyn FuturesStream<Item = ConnectResult<ExecEventMsg>> + Send>>> {
        let sandbox = self.ready_sandbox(&request.msg.sandbox_id).await?;
        let stream = sandbox
            .calls()
            .exec(request.msg.inner)
            .await
            .map_err(|error| self.connect_error(&error))?;
        let limits = self.limits.clone();
        let mapped = stream.map(move |item| {
            item.map_err(|error| {
                ErrorTrace::new(error.current_context().to_connect(limits.max_error_message))
            })
        });
        Ok(Box::pin(mapped))
    }

    async fn exec_input(
        &self,
        _ctx: RpcCtx,
        request: Request<Scoped<ExecInputRequest>>,
    ) -> ConnectResult<Response<ExecInputResponse>> {
        let sandbox = self.ready_sandbox(&request.msg.sandbox_id).await?;
        let answer = sandbox
            .calls()
            .exec_input(request.msg.inner)
            .await
            .map_err(|error| self.connect_error(&error))?;
        Ok(Response::new(answer))
    }

    async fn exec_signal(
        &self,
        _ctx: RpcCtx,
        request: Request<Scoped<ExecSignalRequest>>,
    ) -> ConnectResult<Response<Empty>> {
        let sandbox = self.ready_sandbox(&request.msg.sandbox_id).await?;
        sandbox
            .calls()
            .exec_signal(request.msg.inner)
            .await
            .map_err(|error| self.connect_error(&error))?;
        Ok(Response::new(Empty {}))
    }

    async fn file_write_begin(
        &self,
        _ctx: RpcCtx,
        request: Request<Scoped<FileWriteBeginRequest>>,
    ) -> ConnectResult<Response<FileWriteBeginResponse>> {
        let sandbox = self.ready_sandbox(&request.msg.sandbox_id).await?;
        let answer = sandbox
            .calls()
            .file_write_begin(request.msg.inner)
            .await
            .map_err(|error| self.connect_error(&error))?;
        Ok(Response::new(answer))
    }

    async fn file_write_chunk(
        &self,
        _ctx: RpcCtx,
        request: Request<Scoped<FileWriteChunkRequest>>,
    ) -> ConnectResult<Response<FileWriteChunkResponse>> {
        let sandbox = self.ready_sandbox(&request.msg.sandbox_id).await?;
        let answer = sandbox
            .calls()
            .file_write_chunk(request.msg.inner)
            .await
            .map_err(|error| self.connect_error(&error))?;
        Ok(Response::new(answer))
    }

    async fn file_write_commit(
        &self,
        _ctx: RpcCtx,
        request: Request<Scoped<FileWriteCommitRequest>>,
    ) -> ConnectResult<Response<FileWriteCommitResponse>> {
        let sandbox = self.ready_sandbox(&request.msg.sandbox_id).await?;
        let answer = sandbox
            .calls()
            .file_write_commit(request.msg.inner)
            .await
            .map_err(|error| self.connect_error(&error))?;
        Ok(Response::new(answer))
    }

    async fn file_write_abort(
        &self,
        _ctx: RpcCtx,
        request: Request<Scoped<FileWriteAbortRequest>>,
    ) -> ConnectResult<Response<Empty>> {
        let sandbox = self.ready_sandbox(&request.msg.sandbox_id).await?;
        sandbox
            .calls()
            .file_write_abort(request.msg.inner)
            .await
            .map_err(|error| self.connect_error(&error))?;
        Ok(Response::new(Empty {}))
    }

    async fn file_read(
        &self,
        _ctx: RpcCtx,
        request: Request<Scoped<FileReadRequest>>,
    ) -> ConnectResult<Pin<Box<dyn FuturesStream<Item = ConnectResult<FileChunk>> + Send>>> {
        let sandbox = self.ready_sandbox(&request.msg.sandbox_id).await?;
        let stream = sandbox
            .calls()
            .file_read(request.msg.inner)
            .await
            .map_err(|error| self.connect_error(&error))?;
        let limits = self.limits.clone();
        let mapped = stream.map(move |item| {
            item.map_err(|error| {
                ErrorTrace::new(error.current_context().to_connect(limits.max_error_message))
            })
        });
        Ok(Box::pin(mapped))
    }

    async fn unpack_begin(
        &self,
        _ctx: RpcCtx,
        request: Request<Scoped<UnpackBeginRequest>>,
    ) -> ConnectResult<Response<UnpackBeginResponse>> {
        let sandbox = self.ready_sandbox(&request.msg.sandbox_id).await?;
        let answer = sandbox
            .calls()
            .unpack_begin(request.msg.inner)
            .await
            .map_err(|error| self.connect_error(&error))?;
        Ok(Response::new(answer))
    }

    async fn unpack_chunk(
        &self,
        _ctx: RpcCtx,
        request: Request<Scoped<UnpackChunkRequest>>,
    ) -> ConnectResult<Response<UnpackChunkResponse>> {
        let sandbox = self.ready_sandbox(&request.msg.sandbox_id).await?;
        let answer = sandbox
            .calls()
            .unpack_chunk(request.msg.inner)
            .await
            .map_err(|error| self.connect_error(&error))?;
        Ok(Response::new(answer))
    }

    async fn unpack_commit(
        &self,
        _ctx: RpcCtx,
        request: Request<Scoped<UnpackCommitRequest>>,
    ) -> ConnectResult<Response<UnpackCommitResponse>> {
        let sandbox = self.ready_sandbox(&request.msg.sandbox_id).await?;
        let answer = sandbox
            .calls()
            .unpack_commit(request.msg.inner)
            .await
            .map_err(|error| self.connect_error(&error))?;
        Ok(Response::new(answer))
    }

    async fn unpack_abort(
        &self,
        _ctx: RpcCtx,
        request: Request<Scoped<UnpackAbortRequest>>,
    ) -> ConnectResult<Response<Empty>> {
        let sandbox = self.ready_sandbox(&request.msg.sandbox_id).await?;
        sandbox
            .calls()
            .unpack_abort(request.msg.inner)
            .await
            .map_err(|error| self.connect_error(&error))?;
        Ok(Response::new(Empty {}))
    }
}

/// `SandboxProvider` on a transport to a `SandboxService`.
pub struct SandboxRpcClient {
    ctx: Ctx,
    transport: Arc<dyn Transport>,
    client: Arc<SandboxServiceClient>,
    limits: crate::shared::limits::SandboxLimits,
    authority: String,
}

impl SandboxRpcClient {
    /// A client over a transport to a control plane.
    ///
    /// # Panics
    /// When the JSON client cannot be built. The options are fixed and valid,
    /// so this is a programming error and not a run-time failure.
    #[must_use]
    pub fn new(
        ctx: Ctx,
        transport: Arc<dyn Transport>,
        limits: crate::shared::limits::SandboxLimits,
    ) -> Self {
        Self::with_authority(ctx, transport, limits, CONTROL_AUTHORITY)
    }

    /// A client over a transport, with the authority that the request line
    /// carries.
    ///
    /// A Unix domain socket and a resolved TCP address ignore the authority; a
    /// base `http://` or `https://` address needs it (`SandboxRpcClient::
    /// connect` on an [`ConnectAddr::Url`](crate::native::connect::ConnectAddr)).
    ///
    /// # Panics
    /// When the JSON client cannot be built. The options are fixed and valid,
    /// so this is a programming error and not a run-time failure.
    #[must_use]
    pub fn with_authority(
        ctx: Ctx,
        transport: Arc<dyn Transport>,
        limits: crate::shared::limits::SandboxLimits,
        authority: &str,
    ) -> Self {
        let client = SandboxServiceClient::new(
            Arc::clone(&transport),
            authority,
            ClientOptions::new().with_codec("json"),
        )
        .expect("the JSON SandboxService client builds");
        Self {
            ctx,
            transport,
            client: Arc::new(client),
            limits,
            authority: authority.to_string(),
        }
    }

    /// The same client, with the token on every request.
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
        let client = SandboxServiceClient::new(
            Arc::clone(&self.transport),
            &self.authority,
            ClientOptions::new()
                .with_codec("json")
                .with_header("AUTHORIZATION".to_string(), header),
        )
        .expect("the JSON SandboxService client builds");
        Self {
            ctx: self.ctx,
            transport: self.transport,
            client: Arc::new(client),
            limits: self.limits,
            authority: self.authority,
        }
    }

    fn rpc_ctx(&self, timeout: Duration) -> RpcCtx {
        RpcCtx::client(self.ctx.clone()).with_deadline(timeout)
    }

    fn remote_create(&self, spec: crate::shared::spec::SandboxSpec) -> CreateStreamState {
        CreateStreamState {
            ctx: self.ctx.clone(),
            client: Arc::clone(&self.client),
            limits: self.limits.clone(),
            spec,
            state: Some(RemoteCreateState::Open),
        }
    }
}

/// One step of the remote create stream.
enum RemoteCreateState {
    /// The task opens the `Create` stream.
    Open,
    /// The task reads the stream of the server.
    Stream(Box<ServerStream<CreateEvent>>),
    /// The task is done.
    Done,
}

/// One run of the remote create story.
pub struct CreateStreamState {
    ctx: Ctx,
    client: Arc<SandboxServiceClient>,
    limits: crate::shared::limits::SandboxLimits,
    spec: crate::shared::spec::SandboxSpec,
    state: Option<RemoteCreateState>,
}

#[task]
async fn remote_create(
    ctx: Ctx,
    state: CreateStreamState,
) -> Option<(
    CreateStreamState,
    TaskStatus<SandboxResult<SandboxHandle>, crate::shared::driver::CreateProgress>,
)> {
    let mut task = state;
    drop(ctx);
    let ctx = task.ctx.clone();
    let step = task.state.take()?;
    match step {
        RemoteCreateState::Open => {
            let request = CreateRequest {
                spec: task.spec.clone(),
            };
            // One create runs the driver launch, the readiness wait and
            // `Prepare`, so its deadline is the create deadline and not the
            // deadline of one unary call. The server uses the same field.
            let rpc_ctx = RpcCtx::client(ctx).with_deadline(task.limits.boot_timeout);
            match task.client.create(rpc_ctx, Request::new(request)).await {
                Ok(stream) => Some((
                    task.next(RemoteCreateState::Stream(Box::new(stream))),
                    TaskStatus::Ignore,
                )),
                Err(error) => Some((
                    task.next(RemoteCreateState::Done),
                    TaskStatus::Ready(Err(
                        SandboxError::from_connect(error.current_context()).into()
                    )),
                )),
            }
        }
        RemoteCreateState::Stream(mut stream) => match stream.receive().await {
            Ok(Some(CreateEvent::Progress(progress))) => Some((
                task.next(RemoteCreateState::Stream(stream)),
                TaskStatus::Pending(progress),
            )),
            Ok(Some(CreateEvent::Created(info))) => {
                let result = SandboxRpcClient::handle_with(
                    task.ctx.clone(),
                    Arc::clone(&task.client),
                    task.limits.clone(),
                    info,
                );
                Some((
                    task.next(RemoteCreateState::Done),
                    TaskStatus::Ready(result),
                ))
            }
            Ok(None) => Some((
                task.next(RemoteCreateState::Done),
                TaskStatus::Ready(Err(SandboxError::StreamLost.into())),
            )),
            Err(error) => Some((
                task.next(RemoteCreateState::Done),
                TaskStatus::Ready(Err(
                    SandboxError::from_connect(error.current_context()).into()
                )),
            )),
        },
        RemoteCreateState::Done => None,
    }
}

impl CreateStreamState {
    fn next(mut self, state: RemoteCreateState) -> Self {
        self.state = Some(state);
        self
    }
}

impl SandboxRpcClient {
    fn handle_with(
        ctx: Ctx,
        client: Arc<SandboxServiceClient>,
        limits: crate::shared::limits::SandboxLimits,
        info: SandboxInfo,
    ) -> SandboxResult<SandboxHandle> {
        let id = info.id.parse().map_err(|_| SandboxError::NotFound {
            what: "sandbox".to_string(),
        })?;
        let calls = Arc::new(ScopedCalls::with_client(
            ctx.clone(),
            client,
            info.id.clone(),
            limits.clone(),
        ));
        let ops: Arc<dyn crate::shared::control::SandboxOps> =
            Arc::new(RemoteSandboxOps::new(info, calls, ctx, limits));
        Ok(SandboxHandle::new(id, ops))
    }
}

impl crate::shared::control::SandboxProvider for SandboxRpcClient {
    fn probe(&self) -> BoxFuture<'_, SandboxResult<Vec<crate::shared::driver::HostReport>>> {
        let client = Arc::clone(&self.client);
        let rpc_ctx = self.rpc_ctx(self.limits.rpc_timeout);
        send_box(async move {
            client
                .probe(rpc_ctx, Request::new(Empty {}))
                .await
                .map(|response| response.msg.reports)
                .map_err(|error| SandboxError::from_connect(error.current_context()).into())
        })
    }

    fn create(
        &self,
        spec: crate::shared::spec::SandboxSpec,
    ) -> BoxFuture<'_, SandboxResult<SandboxHandle>> {
        let task = self.remote_create(spec);
        let stream = match foundation_core::valtron::execute(remote_create::new(task), None) {
            Ok(stream) => stream,
            Err(error) => {
                let message = error.to_string();
                return send_box(async move {
                    Err(SandboxError::Io {
                        op: "create".to_string(),
                        message,
                    }
                    .into())
                });
            }
        };
        send_box(async move {
            let (result, _rest) = stream
                .into_ready_future()
                .await
                .ok_or(SandboxError::SlaveHandshake)?;
            result
        })
    }

    fn create_with_progress(
        &self,
        spec: crate::shared::spec::SandboxSpec,
    ) -> crate::shared::handles::CreateStream {
        let task = self.remote_create(spec);
        match foundation_core::valtron::execute(remote_create::new(task), None) {
            Ok(iterator) => {
                crate::shared::handles::CreateStream::new(Box::new(SendIterator::new(iterator)))
            }
            Err(error) => {
                let item: Stream<
                    SandboxResult<SandboxHandle>,
                    crate::shared::driver::CreateProgress,
                > = Stream::Next(Err(SandboxError::Io {
                    op: "create".to_string(),
                    message: error.to_string(),
                }
                .into()));
                crate::shared::handles::CreateStream::new(Box::new(std::iter::once(item)))
            }
        }
    }

    fn get(
        &self,
        id: crate::shared::ids::SandboxId,
    ) -> BoxFuture<'_, SandboxResult<SandboxHandle>> {
        let client = Arc::clone(&self.client);
        let ctx = self.ctx.clone();
        let limits = self.limits.clone();
        let rpc_ctx = self.rpc_ctx(self.limits.rpc_timeout);
        let sandbox_id = id.to_string();
        send_box(async move {
            let info = client
                .get(rpc_ctx, Request::new(SandboxRef { sandbox_id }))
                .await
                .map(|response| response.msg)
                .map_err(|error| SandboxError::from_connect(error.current_context()))?;
            SandboxRpcClient::handle_with(ctx, client, limits, info)
        })
    }

    fn list(&self) -> BoxFuture<'_, SandboxResult<Vec<SandboxInfo>>> {
        let client = Arc::clone(&self.client);
        let rpc_ctx = self.rpc_ctx(self.limits.rpc_timeout);
        send_box(async move {
            client
                .list(rpc_ctx, Request::new(Empty {}))
                .await
                .map(|response| response.msg.sandboxes)
                .map_err(|error| SandboxError::from_connect(error.current_context()).into())
        })
    }

    fn stop(&self, id: crate::shared::ids::SandboxId) -> BoxFuture<'_, SandboxResult<ExitReport>> {
        let client = Arc::clone(&self.client);
        let rpc_ctx = self.rpc_ctx(self.limits.rpc_timeout);
        send_box(async move {
            client
                .stop(
                    rpc_ctx,
                    Request::new(SandboxRef {
                        sandbox_id: id.to_string(),
                    }),
                )
                .await
                .map(|response| response.msg)
                .map_err(|error| SandboxError::from_connect(error.current_context()).into())
        })
    }

    fn kill(&self, id: crate::shared::ids::SandboxId) -> BoxFuture<'_, SandboxResult<ExitReport>> {
        let client = Arc::clone(&self.client);
        let rpc_ctx = self.rpc_ctx(self.limits.rpc_timeout);
        send_box(async move {
            client
                .kill(
                    rpc_ctx,
                    Request::new(SandboxRef {
                        sandbox_id: id.to_string(),
                    }),
                )
                .await
                .map(|response| response.msg)
                .map_err(|error| SandboxError::from_connect(error.current_context()).into())
        })
    }
}
