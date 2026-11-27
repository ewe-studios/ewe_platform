//! The typed client of `SlaveService` on one transport.
//!
//! Every unary call has a deadline from the injected clock. Idempotent calls
//! are retried one time on a transport error; the others are not.

use std::sync::Arc;
use std::time::Duration;

use foundation_connectrpc::shared::transport::Transport;
use foundation_connectrpc::{ClientOptions, ConnectError, Request, RpcCtx, ServerStream};
use foundation_context::Ctx;
use foundation_errstacks::ErrorTrace;

use crate::shared::control::{send_box, SendStream};
use crate::shared::error::{SandboxError, SandboxResult};
use crate::shared::handles::ExecFileCalls;
use crate::shared::limits::SandboxLimits;
use crate::shared::rpc::messages::{
    Empty, ExecEventMsg, ExecInputRequest, ExecInputResponse, ExecRequest, ExecSignalRequest,
    FileChunk, FileReadRequest, FileWriteAbortRequest, FileWriteBeginRequest,
    FileWriteBeginResponse, FileWriteChunkRequest, FileWriteChunkResponse, FileWriteCommitRequest,
    FileWriteCommitResponse, InfoRequest, PrepareRequest, PrepareResponse, ShutdownRequest,
    SlaveInfo, UnpackAbortRequest, UnpackBeginRequest, UnpackBeginResponse, UnpackChunkRequest,
    UnpackChunkResponse, UnpackCommitRequest, UnpackCommitResponse,
};
use crate::shared::rpc::SlaveServiceClient;
use crate::shared::{BoxFuture, ExecEventStream, FileChunkStream};

/// The address that the client uses in the request line. The transport is
/// already connected, so any authority works.
const SLAVE_AUTHORITY: &str = "http://slave.local";

/// The typed client of one slave.
pub struct SlaveClient {
    ctx: Ctx,
    client: SlaveServiceClient,
    limits: SandboxLimits,
}

impl std::fmt::Debug for SlaveClient {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("SlaveClient")
            .field("limits", &self.limits)
            .finish_non_exhaustive()
    }
}

impl SlaveClient {
    /// A client over a connected transport.
    ///
    /// # Errors
    /// [`SandboxError::Io`] when the generated client cannot be built.
    pub fn new(
        ctx: Ctx,
        transport: Arc<dyn Transport>,
        limits: SandboxLimits,
    ) -> SandboxResult<Self> {
        let client = SlaveServiceClient::new(
            transport,
            SLAVE_AUTHORITY,
            ClientOptions::new().with_codec("json"),
        )
        .map_err(|error| SandboxError::Io {
            op: "slave_client".to_string(),
            message: error.to_string(),
        })?;
        Ok(Self {
            ctx,
            client,
            limits,
        })
    }

    /// The limits of this client.
    #[must_use]
    pub fn limits(&self) -> &SandboxLimits {
        &self.limits
    }

    fn rpc_ctx(&self, timeout: Duration) -> RpcCtx {
        RpcCtx::client(self.ctx.clone()).with_deadline(timeout)
    }

    /// The context of a streaming call. A stream has no unary deadline: the
    /// caller ends it, or the slave does.
    fn stream_ctx(&self) -> RpcCtx {
        RpcCtx::client(self.ctx.clone())
    }

    /// Calls `Info` with the given time limit.
    ///
    /// # Errors
    /// The mapped error of the call.
    pub fn info(&self, timeout: Duration) -> BoxFuture<'_, SandboxResult<SlaveInfo>> {
        let client = &self.client;
        let ctx = self.ctx.clone();
        send_box(async move {
            let rpc_ctx = || RpcCtx::client(ctx.clone()).with_deadline(timeout);
            let answer = client
                .info(rpc_ctx(), Request::new(InfoRequest {}))
                .await
                .map_err(|error| connect_to_sandbox(&error));
            match answer {
                Ok(response) => Ok(response.msg),
                Err(error) => {
                    if is_transport_error(&error) {
                        client
                            .info(rpc_ctx(), Request::new(InfoRequest {}))
                            .await
                            .map(|response| response.msg)
                            .map_err(|error| connect_to_sandbox(&error))
                    } else {
                        Err(error)
                    }
                }
            }
        })
    }

    /// Calls `Prepare` with the given time limit.
    ///
    /// # Errors
    /// The mapped error of the call.
    pub fn prepare(
        &self,
        request: PrepareRequest,
        timeout: Duration,
    ) -> BoxFuture<'_, SandboxResult<PrepareResponse>> {
        let client = &self.client;
        let ctx = self.ctx.clone();
        send_box(async move {
            let rpc_ctx = || RpcCtx::client(ctx.clone()).with_deadline(timeout);
            let first = client
                .prepare(rpc_ctx(), Request::new(request.clone()))
                .await
                .map_err(|error| connect_to_sandbox(&error));
            match first {
                Ok(response) => Ok(response.msg),
                Err(error) => {
                    if is_transport_error(&error) {
                        client
                            .prepare(rpc_ctx(), Request::new(request))
                            .await
                            .map(|response| response.msg)
                            .map_err(|error| connect_to_sandbox(&error))
                    } else {
                        Err(error)
                    }
                }
            }
        })
    }

    /// Calls `Shutdown`. A failed call is not an error for a stop: the slave
    /// may already be gone.
    ///
    /// # Errors
    /// The mapped error of the call.
    pub fn shutdown(&self, grace_ms: u32) -> BoxFuture<'_, SandboxResult<()>> {
        let client = &self.client;
        let ctx = self.ctx.clone();
        let timeout = self.limits.rpc_timeout;
        send_box(async move {
            let rpc_ctx = || RpcCtx::client(ctx.clone()).with_deadline(timeout);
            let first = client
                .shutdown(rpc_ctx(), Request::new(ShutdownRequest { grace_ms }))
                .await
                .map_err(|error| connect_to_sandbox(&error));
            match first {
                Ok(_) => Ok(()),
                Err(error) => {
                    if is_transport_error(&error) {
                        client
                            .shutdown(rpc_ctx(), Request::new(ShutdownRequest { grace_ms }))
                            .await
                            .map(|_response| ())
                            .map_err(|error| connect_to_sandbox(&error))
                    } else {
                        Err(error)
                    }
                }
            }
        })
    }
}

/// Whether a mapped error came from the transport (and not from the slave).
fn is_transport_error(error: &ErrorTrace<SandboxError>) -> bool {
    matches!(
        error.current_context(),
        SandboxError::Rpc { code, .. } if code == "unavailable"
    )
}

/// Maps a Connect error to the sandbox error of this crate.
fn connect_to_sandbox(error: &ErrorTrace<ConnectError>) -> ErrorTrace<SandboxError> {
    SandboxError::from_connect(error.current_context()).into()
}

/// One response stream as an event stream of the control plane. A stream
/// error ends the stream after it reports the error one time.
pub(crate) fn event_stream(stream: ServerStream<ExecEventMsg>) -> ExecEventStream {
    let inner = Box::pin(futures::stream::unfold(Some(stream), |state| async move {
        let mut stream = state?;
        match stream.receive().await {
            Ok(Some(message)) => Some((Ok(message), Some(stream))),
            Ok(None) => None,
            Err(error) => Some((Err(connect_to_sandbox(&error)), None)),
        }
    }));
    Box::pin(SendStream::new(inner))
}

/// One response stream as a file chunk stream. A stream error ends the stream
/// after it reports the error one time.
pub(crate) fn chunk_stream(stream: ServerStream<FileChunk>) -> FileChunkStream {
    let inner = Box::pin(futures::stream::unfold(Some(stream), |state| async move {
        let mut stream = state?;
        match stream.receive().await {
            Ok(Some(chunk)) => Some((Ok(chunk), Some(stream))),
            Ok(None) => None,
            Err(error) => Some((Err(connect_to_sandbox(&error)), None)),
        }
    }));
    Box::pin(SendStream::new(inner))
}

/// Calls an idempotent unary call with the one allowed retry on a transport
/// error.
macro_rules! unary_retry {
    ($self:ident, $call:ident, $request:expr, $timeout:expr) => {{
        let first = $self
            .client
            .$call($self.rpc_ctx($timeout), Request::new($request.clone()))
            .await
            .map_err(|error| connect_to_sandbox(&error));
        match first {
            Ok(response) => Ok(response.msg),
            Err(error) => {
                if is_transport_error(&error) {
                    $self
                        .client
                        .$call($self.rpc_ctx($timeout), Request::new($request))
                        .await
                        .map(|response| response.msg)
                        .map_err(|error| connect_to_sandbox(&error))
                } else {
                    Err(error)
                }
            }
        }
    }};
}

impl ExecFileCalls for SlaveClient {
    fn exec(&self, request: ExecRequest) -> BoxFuture<'_, SandboxResult<ExecEventStream>> {
        send_box(async move {
            let stream = self
                .client
                .exec(self.stream_ctx(), Request::new(request))
                .await
                .map_err(|error| connect_to_sandbox(&error))?;
            Ok(event_stream(stream))
        })
    }

    fn exec_input(
        &self,
        request: ExecInputRequest,
    ) -> BoxFuture<'_, SandboxResult<ExecInputResponse>> {
        let timeout = self.limits.rpc_timeout;
        send_box(async move { unary_retry!(self, exec_input, request, timeout) })
    }

    fn exec_signal(&self, request: ExecSignalRequest) -> BoxFuture<'_, SandboxResult<()>> {
        let timeout = self.limits.rpc_timeout;
        send_box(async move {
            let answer: SandboxResult<Empty> = unary_retry!(self, exec_signal, request, timeout);
            answer.map(|_empty| ())
        })
    }

    fn file_write_begin(
        &self,
        request: FileWriteBeginRequest,
    ) -> BoxFuture<'_, SandboxResult<FileWriteBeginResponse>> {
        let timeout = self.limits.rpc_timeout;
        send_box(async move {
            self.client
                .file_write_begin(self.rpc_ctx(timeout), Request::new(request))
                .await
                .map(|response| response.msg)
                .map_err(|error| connect_to_sandbox(&error))
        })
    }

    fn file_write_chunk(
        &self,
        request: FileWriteChunkRequest,
    ) -> BoxFuture<'_, SandboxResult<FileWriteChunkResponse>> {
        let timeout = self.limits.rpc_timeout;
        send_box(async move { unary_retry!(self, file_write_chunk, request, timeout) })
    }

    fn file_write_commit(
        &self,
        request: FileWriteCommitRequest,
    ) -> BoxFuture<'_, SandboxResult<FileWriteCommitResponse>> {
        let timeout = self.limits.rpc_timeout;
        send_box(async move {
            self.client
                .file_write_commit(self.rpc_ctx(timeout), Request::new(request))
                .await
                .map(|response| response.msg)
                .map_err(|error| connect_to_sandbox(&error))
        })
    }

    fn file_write_abort(&self, request: FileWriteAbortRequest) -> BoxFuture<'_, SandboxResult<()>> {
        let timeout = self.limits.rpc_timeout;
        send_box(async move {
            let answer: SandboxResult<Empty> =
                unary_retry!(self, file_write_abort, request, timeout);
            answer.map(|_empty| ())
        })
    }

    fn file_read(&self, request: FileReadRequest) -> BoxFuture<'_, SandboxResult<FileChunkStream>> {
        send_box(async move {
            let stream = self
                .client
                .file_read(self.stream_ctx(), Request::new(request))
                .await
                .map_err(|error| connect_to_sandbox(&error))?;
            Ok(chunk_stream(stream))
        })
    }

    fn unpack_begin(
        &self,
        request: UnpackBeginRequest,
    ) -> BoxFuture<'_, SandboxResult<UnpackBeginResponse>> {
        let timeout = self.limits.rpc_timeout;
        send_box(async move {
            self.client
                .unpack_begin(self.rpc_ctx(timeout), Request::new(request))
                .await
                .map(|response| response.msg)
                .map_err(|error| connect_to_sandbox(&error))
        })
    }

    fn unpack_chunk(
        &self,
        request: UnpackChunkRequest,
    ) -> BoxFuture<'_, SandboxResult<UnpackChunkResponse>> {
        let timeout = self.limits.rpc_timeout;
        send_box(async move { unary_retry!(self, unpack_chunk, request, timeout) })
    }

    fn unpack_commit(
        &self,
        request: UnpackCommitRequest,
    ) -> BoxFuture<'_, SandboxResult<UnpackCommitResponse>> {
        let timeout = self.limits.rpc_timeout;
        send_box(async move {
            self.client
                .unpack_commit(self.rpc_ctx(timeout), Request::new(request))
                .await
                .map(|response| response.msg)
                .map_err(|error| connect_to_sandbox(&error))
        })
    }

    fn unpack_abort(&self, request: UnpackAbortRequest) -> BoxFuture<'_, SandboxResult<()>> {
        let timeout = self.limits.rpc_timeout;
        send_box(async move {
            let answer: SandboxResult<Empty> = unary_retry!(self, unpack_abort, request, timeout);
            answer.map(|_empty| ())
        })
    }
}
