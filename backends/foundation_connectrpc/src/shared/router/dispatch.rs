//! The frozen prefix route + request dispatch flow (Decision 08 §Request Dispatch
//! Flow / §Integration with foundation_http).
//!
//! WHY: One [`ConnectRpcHandler`] is registered as a **single** foundation_http
//! prefix route; it sub-routes internally by path. Dispatch is a fixed sequence
//! of decisions — 404 → 405(+Allow) → protocol match → codec membership(+415) →
//! capability check(+505) — then it runs the erased handler and frames the
//! response per protocol. Errors at any point render through the protocol-aware
//! [`ErrorWriter`].
//!
//! WHAT: [`ConnectRpcHandler`] and the async `dispatch` entry (returns a
//! `SimpleOutgoingResponse`; never `Err` — failures become error responses).
//!
//! HOW: unary runs sequentially (decode → handle → encode — it does **not** use
//! the streaming seam, Decision 08); streaming builds the protocol exchange and
//! drives its reader/writer tasks, the handler, a body feeder, and a response
//! pump concurrently (`join`/`select`) — cooperatively inside this one
//! dispatch future (no `spawn`, so no executor-pool dependency). The request
//! body is read as a [`RequestBody`] stream, chunk by chunk, and response
//! frames go out through a [`ResponseSink`] as they are produced, so neither is
//! held whole in memory.
//! A server passes a sink that writes to its connection
//! ([`ConnectRpcHandler::dispatch_to`]); [`ConnectRpcHandler::dispatch`]
//! collects into a [`CollectingSink`] for tests and in-process callers.
//!
//! Cancellation: every call's [`Ctx`] carries a [`CancelSignal`] that dispatch
//! fires when the call ends early — the peer reset the stream or went away
//! (the connection owner reports that through [`CallHooks`]), the request
//! body failed, the response could not be written, or the call's deadline
//! (`connect-timeout-ms` / `grpc-timeout`) passed. The handler's pending pipe
//! operations wake on it, and so does anything awaiting `ctx.cancelled()`.
//! A deadline is enforced with the [`DeadlineTimer`] the connection owner
//! supplies and answered with `deadline_exceeded`.

use std::io;
use std::pin::Pin;
use std::sync::Arc;
use std::task::{Context, Poll};
use std::time::Instant;

use bytes::Bytes;
use foundation_core::io::readers::Data;
use foundation_core::valtron::Pipe;
#[cfg(not(target_family = "wasm"))]
use foundation_core::valtron::Stream;
use foundation_core::valtron::{PipeReceiver, PipeSender};
use foundation_errstacks::ErrorTrace;
use foundation_http::shared::context::ContextBag;
use futures::future::{select, Either};
use futures::StreamExt;
// HTTP/2 dispatch is native-only (drives foundation_netio::http2). The h2 section
// below is cfg-gated to match; on wasm the router serves H1 protocols only.
#[cfg(not(target_family = "wasm"))]
use foundation_netio::http2::types::{H2Frame, H2IncomingFrame, SimpleIncomingRequestHeader};
#[cfg(not(target_family = "wasm"))]
use foundation_netio::shared::client::body_reader::{
    SendSafeBodyBytesItem, SendSafeBodyBytesIterator,
};
use foundation_netio::shared::http::{
    ChunkedData, HttpReaderError, Proto, SendSafeBody, SimpleHeader, SimpleHeaders,
    SimpleIncomingRequest, SimpleMethod, SimpleOutgoingResponse, Status,
};

use crate::shared::context::{CancelSignal, Ctx, IdempotencyLevel, Peer, RequestContext, Spec};
use crate::shared::error::{ConnectError, ConnectResult};
use crate::shared::error_writer::ErrorWriter;
use crate::shared::protocol::{ExchangeConfig, ProtocolHandler, UnaryOutcome};
use crate::shared::transport::{
    check_compatible, requirements, BoxFuture, ProtocolKind, TransportCapabilities,
};

use super::{HandlerEntry, HandlerKind, Router};

/// The frozen foundation_http prefix handler produced by
/// [`Router::into_handler`](super::Router::into_handler) (Q10 — no post-build
/// mutation). Register it as one prefix route; it dispatches every ConnectRPC
/// request by path.
pub struct ConnectRpcHandler {
    router: Arc<Router>,
}

impl ConnectRpcHandler {
    pub(crate) fn new(router: Router) -> Self {
        Self {
            router: Arc::new(router),
        }
    }

    /// The underlying router (read-only).
    #[must_use]
    pub fn router(&self) -> &Router {
        &self.router
    }

    /// Dispatch one request and collect the whole response in memory.
    ///
    /// Every failure is rendered as a protocol-appropriate error response. A
    /// streaming RPC is collected until it ends, so this is for tests and
    /// in-process callers; a server writes to its connection as frames are
    /// produced with [`Self::dispatch_to`].
    ///
    /// The request body is read from `request.body`. It must not need a
    /// connection to wait on: a body that reports "not ready yet" fails the
    /// read (see [`in_memory_body`]).
    ///
    /// No timer is attached, so a call deadline is not enforced here; use
    /// [`Self::dispatch_to`] with [`CallHooks::with_timer`] for that.
    #[must_use]
    pub fn dispatch(
        &self,
        bag: Arc<ContextBag>,
        mut request: SimpleIncomingRequest,
    ) -> BoxFuture<'static, SimpleOutgoingResponse> {
        let router = self.router.clone();
        Box::pin(async move {
            let proto = request.proto.clone();
            let body = in_memory_body(request.body.take());
            let mut sink = CollectingSink::default();
            match dispatch_request(router, bag, request, body, &mut sink, CallHooks::default())
                .await
            {
                Ok(()) => sink.into_response().unwrap_or_else(|| {
                    tracing::error!("dispatch finished without producing a response");
                    internal_error_response(proto)
                }),
                Err(e) => {
                    // A `CollectingSink` never fails; this guards the contract.
                    tracing::error!(error = %e, "collecting sink failed");
                    internal_error_response(proto)
                }
            }
        })
    }

    /// Dispatch one request, reading its body from `body` and writing the
    /// response to `sink` as it is produced: a whole response for unary calls
    /// and errors, or a head followed by each frame for a stream.
    ///
    /// `request.body` is ignored; the connection owner hands the body over as
    /// `body` so that it can wait for the connection without blocking. Every
    /// RPC failure is rendered as a response; `Err` is only a failure of
    /// `sink` itself (e.g. the peer went away), and it cancels the call.
    ///
    /// `hooks` carries the cancel signal the connection owner fires when the
    /// peer goes away, and the timer that enforces the call's deadline.
    pub fn dispatch_to<'a>(
        &self,
        bag: Arc<ContextBag>,
        request: SimpleIncomingRequest,
        body: RequestBody<'a>,
        sink: &'a mut dyn ResponseSink,
        hooks: CallHooks<'a>,
    ) -> BoxFuture<'a, io::Result<()>> {
        let router = self.router.clone();
        Box::pin(dispatch_request(router, bag, request, body, sink, hooks))
    }
}

/// Waits until an instant: how dispatch enforces a call's deadline.
///
/// WHY: dispatch runs inside whatever drives the connection — a thread
/// sleeping in `poll(2)` for HTTP/1.1, the valtron pool for HTTP/2 and
/// HTTP/3 — and only that driver can wake it at a given time without a thread
/// sleeping or spinning. Each connection owner supplies the timer that fits.
pub trait DeadlineTimer: Send + Sync {
    /// Resolve once `deadline` has passed.
    fn sleep_until(&self, deadline: Instant) -> BoxFuture<'_, ()>;
}

/// What the connection owner gives dispatch besides the request: the call's
/// cancel signal, which the owner fires when the peer goes away, and the timer
/// that enforces the call's deadline.
///
/// The default has a fresh signal nobody else fires and no timer, so a
/// deadline is not enforced.
#[derive(Default)]
pub struct CallHooks<'a> {
    cancel: CancelSignal,
    timer: Option<&'a dyn DeadlineTimer>,
}

impl<'a> CallHooks<'a> {
    /// Hooks around `cancel`: the call's [`Ctx`] carries this signal, so
    /// firing it cancels the call.
    #[must_use]
    pub fn new(cancel: CancelSignal) -> Self {
        Self {
            cancel,
            timer: None,
        }
    }

    /// Enforce the call's deadline with `timer`.
    #[must_use]
    pub fn with_timer(mut self, timer: &'a dyn DeadlineTimer) -> Self {
        self.timer = Some(timer);
        self
    }
}

/// The [`DeadlineTimer`] for connection owners that run dispatch on valtron
/// (HTTP/2, HTTP/3).
#[cfg(not(target_family = "wasm"))]
pub(crate) use crate::native::deadline_timer::ThreadTimer;

/// Resolves when the call's deadline passes; never when there is no deadline
/// or no timer to enforce it with.
pub(crate) async fn deadline_expiry(timer: Option<&dyn DeadlineTimer>, deadline: Option<Instant>) {
    match (timer, deadline) {
        (Some(timer), Some(deadline)) => timer.sleep_until(deadline).await,
        _ => std::future::pending().await,
    }
}

pub(crate) fn deadline_exceeded() -> ErrorTrace<ConnectError> {
    ConnectError::deadline_exceeded("the call's deadline passed").into()
}

/// The error response a call gets when its deadline passes before it answers.
/// Built up front because the request is moved into the call.
fn deadline_response(request: &SimpleIncomingRequest) -> SimpleOutgoingResponse {
    let mut response = blank_response(request);
    write_error(&mut response, request, &deadline_exceeded());
    response
}

/// An incoming request body, chunk by chunk, as the connection delivers it.
///
/// WHY: the HTTP/1.1 connection owner reads the body lazily from the same
/// socket it writes the response to. Reading it as an async stream lets the
/// owner wait for the socket instead of blocking or spinning, while the rest of
/// the RPC keeps running.
pub type RequestBody<'a> = Pin<Box<dyn futures::Stream<Item = io::Result<Bytes>> + Send + 'a>>;

/// A [`RequestBody`] over a body that is already in memory (or whose reads
/// never need to wait). A read that would block fails with
/// [`io::ErrorKind::WouldBlock`], because there is no connection to wait on.
pub fn in_memory_body(body: Option<SendSafeBody>) -> RequestBody<'static> {
    Box::pin(InMemoryBody(BodyReader::new(body)))
}

struct InMemoryBody(BodyReader);

impl futures::Stream for InMemoryBody {
    type Item = io::Result<Bytes>;

    fn poll_next(mut self: Pin<&mut Self>, _cx: &mut Context<'_>) -> Poll<Option<Self::Item>> {
        Poll::Ready(match self.0.step() {
            BodyStep::Chunk(bytes) => Some(Ok(bytes)),
            BodyStep::End => None,
            BodyStep::Failed(e) => Some(Err(e)),
            BodyStep::WouldBlock => Some(Err(io::Error::new(
                io::ErrorKind::WouldBlock,
                "request body is not ready and there is no connection to wait on",
            ))),
        })
    }
}

/// One non-blocking step of reading a request body.
pub(crate) enum BodyStep {
    /// The next piece of the body.
    Chunk(Bytes),
    /// No bytes are available yet; the connection must become readable first.
    WouldBlock,
    /// The body is complete.
    End,
    /// The body could not be read.
    Failed(io::Error),
}

/// Reads a `SendSafeBody` one non-blocking step at a time.
///
/// WHY: a reader that waits on the socket needs one answer per step — a
/// chunk, the end, a failure, or "would block" — while the body iterators
/// interleave bookkeeping items (the last chunk, trailers) with the data.
pub(crate) struct BodyReader {
    body: SendSafeBody,
}

impl BodyReader {
    pub(crate) fn new(body: Option<SendSafeBody>) -> Self {
        Self {
            body: body.unwrap_or(SendSafeBody::None),
        }
    }

    /// Advance the body by one step without blocking.
    pub(crate) fn step(&mut self) -> BodyStep {
        loop {
            match &mut self.body {
                SendSafeBody::None => return BodyStep::End,
                SendSafeBody::Bytes(bytes) => {
                    let bytes = std::mem::take(bytes);
                    self.body = SendSafeBody::None;
                    if !bytes.is_empty() {
                        return BodyStep::Chunk(Bytes::from(bytes));
                    }
                }
                SendSafeBody::Text(text) => {
                    let text = std::mem::take(text);
                    self.body = SendSafeBody::None;
                    if !text.is_empty() {
                        return BodyStep::Chunk(Bytes::from(text.into_bytes()));
                    }
                }
                SendSafeBody::Stream(iter) => match iter.as_mut().and_then(Iterator::next) {
                    None => self.body = SendSafeBody::None,
                    Some(Ok(Data::Bytes(bytes))) => {
                        if !bytes.is_empty() {
                            return BodyStep::Chunk(Bytes::from(bytes));
                        }
                    }
                    // The batch reader saw `WouldBlock`.
                    Some(Ok(Data::Retry)) => return BodyStep::WouldBlock,
                    Some(Err(e)) => return classify_body_error(&e),
                },
                SendSafeBody::ChunkedStream(iter) => {
                    match iter.as_mut().and_then(Iterator::next) {
                        None => self.body = SendSafeBody::None,
                        Some(Ok(ChunkedData::Data(bytes, _))) => {
                            if !bytes.is_empty() {
                                return BodyStep::Chunk(Bytes::from(bytes));
                            }
                        }
                        // The last chunk and the trailers carry no body bytes;
                        // the iterator ends right after them.
                        Some(Ok(ChunkedData::DataEnded | ChunkedData::Trailers(_))) => {}
                        Some(Err(e)) => return classify_body_error(&e),
                    }
                }
                // Line-feed and event-stream bodies come with
                // `text/event-stream`, which no RPC protocol accepts: dispatch
                // answers 415 before it reads such a body.
                SendSafeBody::LineFeedStream(_) | SendSafeBody::SseStream(_) => {
                    self.body = SendSafeBody::None;
                    return BodyStep::Failed(io::Error::new(
                        io::ErrorKind::InvalidData,
                        "line-feed and event-stream request bodies are not RPC bodies",
                    ));
                }
            }
        }
    }
}

/// A body read error is either "not ready yet" on a non-blocking socket or a
/// real failure.
fn classify_body_error(e: &foundation_core::extensions::result_ext::BoxedError) -> BodyStep {
    let would_block = e
        .downcast_ref::<HttpReaderError>()
        .is_some_and(HttpReaderError::is_would_block)
        || e
            .downcast_ref::<io::Error>()
            .is_some_and(|e| e.kind() == io::ErrorKind::WouldBlock);
    if would_block {
        BodyStep::WouldBlock
    } else {
        // The error is not `Send`, so only its message can travel.
        BodyStep::Failed(io::Error::other(e.to_string()))
    }
}

/// Where a dispatched response goes.
///
/// WHY: a streaming RPC's frames must reach the peer as they are produced, not
/// after the handler finishes. The router cannot own the connection, so the
/// connection owner hands it a sink. Writes are async so that a sink can wait
/// for a slow peer (backpressure) without blocking the rest of the RPC.
///
/// WHAT: either one [`Self::whole`] call, or [`Self::open`], any number of
/// [`Self::chunk`] calls, then [`Self::finish`].
pub trait ResponseSink: Send {
    /// Write a complete response: unary results, errors, and streams that
    /// produced no frames.
    ///
    /// # Errors
    /// Returns the underlying write error.
    fn whole(&mut self, response: SimpleOutgoingResponse) -> BoxFuture<'_, io::Result<()>>;

    /// Start a streamed response. `head` carries the status and headers; its
    /// body is empty and the frames follow through [`Self::chunk`].
    ///
    /// # Errors
    /// Returns the underlying write error.
    fn open(&mut self, head: SimpleOutgoingResponse) -> BoxFuture<'_, io::Result<()>>;

    /// Write one frame of an opened stream and deliver it now.
    ///
    /// # Errors
    /// Returns the underlying write error.
    fn chunk(&mut self, bytes: Bytes) -> BoxFuture<'_, io::Result<()>>;

    /// End an opened stream.
    ///
    /// # Errors
    /// Returns the underlying write error.
    fn finish(&mut self) -> BoxFuture<'_, io::Result<()>>;
}

/// A [`ResponseSink`] that assembles the response in memory, streamed frames
/// included. Used by [`ConnectRpcHandler::dispatch`].
#[derive(Default)]
pub struct CollectingSink {
    response: Option<SimpleOutgoingResponse>,
    body: Vec<u8>,
}

impl CollectingSink {
    /// The collected response, or `None` if nothing was written.
    #[must_use]
    pub fn into_response(self) -> Option<SimpleOutgoingResponse> {
        let mut response = self.response?;
        if response.body.is_none() {
            response.body = Some(SendSafeBody::Bytes(self.body));
        }
        Some(response)
    }
}

impl ResponseSink for CollectingSink {
    fn whole(&mut self, response: SimpleOutgoingResponse) -> BoxFuture<'_, io::Result<()>> {
        self.response = Some(response);
        Box::pin(std::future::ready(Ok(())))
    }

    fn open(&mut self, mut head: SimpleOutgoingResponse) -> BoxFuture<'_, io::Result<()>> {
        head.body = None;
        self.response = Some(head);
        Box::pin(std::future::ready(Ok(())))
    }

    fn chunk(&mut self, bytes: Bytes) -> BoxFuture<'_, io::Result<()>> {
        self.body.extend_from_slice(&bytes);
        Box::pin(std::future::ready(Ok(())))
    }

    fn finish(&mut self) -> BoxFuture<'_, io::Result<()>> {
        Box::pin(std::future::ready(Ok(())))
    }
}

fn internal_error_response(proto: Proto) -> SimpleOutgoingResponse {
    SimpleOutgoingResponse {
        proto,
        status: Status::InternalServerError,
        headers: SimpleHeaders::new(),
        body: None,
        trailers: SimpleHeaders::new(),
    }
}

async fn dispatch_request(
    router: Arc<Router>,
    bag: Arc<ContextBag>,
    request: SimpleIncomingRequest,
    body: RequestBody<'_>,
    sink: &mut dyn ResponseSink,
    hooks: CallHooks<'_>,
) -> io::Result<()> {
    let path = extract_path(&request.request_url.url);

    // 1. Path lookup → 404.
    let Some(entry) = router.lookup(&path) else {
        return sink.whole(status_only(&request, Status::NotFound)).await;
    };

    // 2. Method check → 405 (+Allow).
    let allowed = allowed_methods(entry);
    if !allowed.contains(&request.method) {
        return sink.whole(method_not_allowed(&request, &allowed)).await;
    }

    // 3. Protocol match by Content-Type / GET query → 415 (+Accept-Post).
    let Some(protocol) = entry
        .protocol_handlers
        .iter()
        .find(|p| p.can_handle(&request))
    else {
        return sink.whole(unsupported_media_type(&request, entry)).await;
    };
    let protocol = protocol.as_ref();

    // 4. Codec name + membership → 415 (+Accept-Post).
    let Some(codec_name) = protocol.codec_name(&request) else {
        return sink.whole(unsupported_media_type(&request, entry)).await;
    };
    if !entry.codec_meta.has_codec(&codec_name) {
        return sink.whole(unsupported_media_type(&request, entry)).await;
    }

    // 5. Capability check → 505 (bidi over HTTP/1.1, gRPC over HTTP/1.1, …).
    let caps = capabilities_for(&request.proto);
    let reqs = requirements(protocol.kind(), entry.spec.stream_type);
    if check_compatible(&reqs, &caps).is_err() {
        return sink.whole(http_version_not_supported(&request)).await;
    }

    // 5b. P7: enforce the Connect protocol-version marker when the procedure
    // requires it (opt-in per procedure).
    if entry.options.require_connect_protocol_header && protocol.kind() == ProtocolKind::Connect {
        if let Err(e) = require_connect_version(&request) {
            let mut response = blank_response(&request);
            write_error(&mut response, &request, &e);
            return sink.whole(response).await;
        }
    }

    // 6. Execute. Compression: per-procedure override else the router's global.
    let compression = entry
        .options
        .compression
        .as_ref()
        .unwrap_or_else(|| router.global_compression());
    let ctx = build_ctx(bag, &request, entry.spec.clone(), protocol, hooks.cancel);
    let spec = entry.spec.clone();
    let pipe_depth = entry.options.pipe_depth;
    let timer = hooks.timer;

    // Move `request` (a `Send` value) into the run functions — the decision phase
    // above is fully synchronous, so no `&request` is held across an await (which
    // would poison `Send`, as `SimpleIncomingRequest` is not `Sync`).
    match &entry.handler {
        HandlerKind::Unary(handler) => {
            let cancel = ctx.cancel_signal().clone();
            let response = run_unary_until_deadline(
                handler.clone(),
                protocol,
                request,
                body,
                codec_name,
                compression,
                ctx,
                entry.options.limits.read_max_bytes,
                timer,
            )
            .await;
            let written = sink.whole(response).await;
            if written.is_err() {
                // The peer is gone; anything the handler left running stops.
                cancel.cancel();
            }
            written
        }
        HandlerKind::ServerStream(handler)
        | HandlerKind::ClientStream(handler)
        | HandlerKind::BidiStream(handler) => {
            run_streaming(
                handler.clone(),
                protocol,
                request,
                body,
                spec,
                codec_name,
                compression,
                ctx,
                pipe_depth,
                entry.options.limits.read_max_bytes,
                sink,
                timer,
            )
            .await
        }
    }
}

/// [`run_unary_body`] bounded by the call's deadline: when it passes first the
/// call is cancelled, dropped, and answered with `deadline_exceeded`.
#[allow(clippy::too_many_arguments)]
pub(crate) async fn run_unary_until_deadline(
    handler: Arc<dyn super::erased::ErasedUnaryHandler>,
    protocol: &dyn ProtocolHandler,
    request: SimpleIncomingRequest,
    body: RequestBody<'_>,
    codec_name: String,
    compression: &crate::shared::compression::CompressionRegistry,
    ctx: Ctx,
    read_max_bytes: usize,
    timer: Option<&dyn DeadlineTimer>,
) -> SimpleOutgoingResponse {
    let deadline = ctx.request.deadline;
    let (Some(timer), Some(deadline)) = (timer, deadline) else {
        return run_unary_body(
            handler,
            protocol,
            request,
            body,
            codec_name,
            compression,
            ctx,
            read_max_bytes,
        )
        .await;
    };
    let cancel = ctx.cancel_signal().clone();
    let timed_out = deadline_response(&request);
    let call = std::pin::pin!(run_unary_body(
        handler,
        protocol,
        request,
        body,
        codec_name,
        compression,
        ctx,
        read_max_bytes,
    ));
    match select(call, timer.sleep_until(deadline)).await {
        Either::Left((response, _)) => response,
        Either::Right(((), _call)) => {
            cancel.cancel();
            timed_out
        }
    }
}

/// Unary over a request whose body is already in `request.body` (h2, h3),
/// bounded by the call's deadline when `timer` is given. See
/// [`run_unary_until_deadline`].
#[allow(clippy::too_many_arguments)]
pub(crate) async fn run_unary(
    handler: Arc<dyn super::erased::ErasedUnaryHandler>,
    protocol: &dyn ProtocolHandler,
    mut request: SimpleIncomingRequest,
    codec_name: String,
    compression: &crate::shared::compression::CompressionRegistry,
    ctx: Ctx,
    read_max_bytes: usize,
    timer: Option<&dyn DeadlineTimer>,
) -> SimpleOutgoingResponse {
    let body = in_memory_body(request.body.take());
    run_unary_until_deadline(
        handler,
        protocol,
        request,
        body,
        codec_name,
        compression,
        ctx,
        read_max_bytes,
        timer,
    )
    .await
}

/// Unary: read the request body (capped at `read_max_bytes`) → decode →
/// invoke the erased handler → frame the response per protocol. Unary bypasses
/// the streaming seam (Decision 08).
#[allow(clippy::too_many_arguments)]
async fn run_unary_body(
    handler: Arc<dyn super::erased::ErasedUnaryHandler>,
    protocol: &dyn ProtocolHandler,
    request: SimpleIncomingRequest,
    mut body: RequestBody<'_>,
    codec_name: String,
    compression: &crate::shared::compression::CompressionRegistry,
    ctx: Ctx,
    read_max_bytes: usize,
) -> SimpleOutgoingResponse {
    let mut response = blank_response(&request);

    let body = match read_unary_body(&mut body, read_max_bytes).await {
        Ok(b) => b,
        Err(UnaryBodyError::TooLarge(len)) => {
            tracing::warn!(
                len,
                read_max_bytes,
                "request body exceeded read_max_bytes; returning 413"
            );
            response.status = Status::PayloadTooLarge;
            return response;
        }
        Err(UnaryBodyError::Read(e)) => {
            tracing::warn!(error = %e, "request body could not be read");
            let error = ConnectError::new(
                crate::shared::error::Code::InvalidArgument,
                format!("request body could not be read: {e}"),
            );
            write_error(&mut response, &request, &error.into());
            return response;
        }
    };
    let decoded = protocol.decode_unary_request(&request, body, compression, read_max_bytes);
    let outcome: ConnectResult<UnaryOutcome> = match decoded {
        Ok(msg) => match handler.handle(ctx, &codec_name, msg).await {
            Ok((frame, headers, trailers)) => Ok(UnaryOutcome {
                frame,
                headers,
                trailers,
            }),
            Err(e) => Err(e),
        },
        Err(e) => Err(e),
    };

    match outcome {
        Ok(outcome) => {
            if let Err(e) = protocol.encode_unary_response(
                &mut response,
                &request,
                &codec_name,
                outcome,
                compression,
            ) {
                write_error(&mut response, &request, &e);
            }
        }
        Err(e) => write_error(&mut response, &request, &e),
    }
    response
}

/// Streaming: build the protocol exchange over byte pipes, then drive its
/// reader/writer tasks, the handler, a body feeder and a response pump
/// concurrently.
///
/// The request body is fed into the protocol pipe chunk by chunk, with
/// backpressure, as the reader task consumes it. Framed response bytes go to
/// `sink` as the writer task produces them. The head is withheld until the
/// first frame (as on h2), so a handler that fails before emitting anything is
/// still reported with a real status through [`ResponseSink::whole`]. After the
/// head has gone out, errors travel in-band, framed by the writer task.
///
/// The response decides when the RPC is over:
/// - a sink failure (the peer went away) ends it at once, cancelling the call
///   and dropping the handler, so an endless stream is released;
/// - the call's deadline passing does the same, and the call is answered with
///   `deadline_exceeded` (in band if the stream already started);
/// - once the response side (writer task + handler) is done, a request side
///   still waiting on the peer is abandoned, as on h2. The connection owner
///   sees that the body was not read to its end and must not reuse the
///   connection.
#[allow(clippy::too_many_arguments)]
async fn run_streaming(
    handler: Arc<dyn super::erased::ErasedStreamHandler>,
    protocol: &dyn ProtocolHandler,
    request: SimpleIncomingRequest,
    mut body: RequestBody<'_>,
    spec: Spec,
    codec_name: String,
    compression: &crate::shared::compression::CompressionRegistry,
    ctx: Ctx,
    pipe_depth: usize,
    read_max_bytes: usize,
    sink: &mut dyn ResponseSink,
    timer: Option<&dyn DeadlineTimer>,
) -> io::Result<()> {
    let (req_tx, req_rx) = Pipe::<Bytes>::with_depth(pipe_depth);
    let (resp_tx, resp_rx) = Pipe::<Bytes>::with_depth(pipe_depth);

    let cancel = ctx.cancel_signal().clone();
    let deadline = ctx.request.deadline;
    let config = ExchangeConfig {
        compression,
        cancel: cancel.clone(),
        read_max_bytes,
    };
    let exchange = match protocol.new_conn(&request, spec, req_rx, resp_tx, config) {
        Ok(exchange) => exchange,
        Err(e) => {
            let mut response = blank_response(&request);
            write_error(&mut response, &request, &e);
            return sink.whole(response).await;
        }
    };
    let crate::shared::protocol::HandlerExchange {
        conn,
        reader_task,
        writer_task,
    } = exchange;

    // Feed the request body into the protocol pipe as the reader consumes it.
    // A body that fails part-way aborts the call: ending the pipe quietly would
    // look to the handler like a stream that finished normally.
    let feeder = async move {
        while let Some(chunk) = body.next().await {
            // The reader task is gone: nobody wants the rest.
            if req_tx.send(chunk?).await.is_err() {
                break;
            }
        }
        req_tx.close();
        Ok::<(), io::Error>(())
    };

    let content_type = protocol.streaming_response_content_type(&request, &codec_name);
    // The writer task compressed per this same negotiation — the peer only
    // accepts flagged envelopes when the encoding is advertised.
    let response_encoding = protocol.streaming_response_encoding(&request, compression);
    let mut head = blank_response(&request);
    head.status = Status::OK;
    head.headers
        .insert(SimpleHeader::CONTENT_TYPE, vec![content_type.clone()]);
    if let Some((name, enc)) = response_encoding.clone() {
        head.headers.insert(SimpleHeader::from(name), vec![enc]);
    }

    // Forward framed response bytes to the sink as they are produced. Returns
    // whether the head went out. The pump owns `resp_rx`, so when it stops the
    // writer task's sends fail instead of waiting on a full pipe forever.
    let pump = async {
        let resp_rx = resp_rx;
        let mut head = Some(head);
        while let Some(chunk) = resp_rx.receive().await {
            if let Some(head) = head.take() {
                sink.open(head).await?;
            }
            sink.chunk(chunk).await?;
        }
        Ok::<_, io::Error>(head.is_none())
    };
    let handler_fut = handler.handle(ctx, &codec_name, conn);

    let request_side = futures::future::try_join(feeder, async {
        Ok::<_, io::Error>(reader_task.await)
    });
    let response_side = futures::future::join(writer_task, handler_fut);
    let rest = std::pin::pin!(exchange_until_aborted(
        request_side,
        response_side,
        deadline_expiry(timer, deadline),
        std::future::pending::<()>(),
        &cancel,
    ));
    // Scoped so the pump's borrow of `sink` ends before the closing write.
    let (opened, end) = {
        let pump = std::pin::pin!(pump);
        match select(pump, rest).await {
            // The peer is gone: cancel the call and drop everything now.
            Either::Left((Err(e), rest)) => {
                cancel_and_settle(&cancel, rest);
                return Err(e);
            }
            Either::Left((Ok(opened), rest)) => (opened, rest.await),
            // The writer has finished or been dropped, which ends the pump.
            Either::Right((end, pump)) => match pump.await {
                Ok(opened) => (opened, end),
                Err(e) => {
                    cancel.cancel();
                    return Err(e);
                }
            },
        }
    };

    let failure = match end {
        StreamEnd::Done {
            reader,
            writer,
            handler,
        } => {
            if opened {
                // The stream framed itself, unless the call failed without
                // closing it: then end it in band with the error.
                if let Some(e) = unclosed_stream_error(writer, handler) {
                    if let Some(frame) = stream_error_frame(protocol, &request, &e) {
                        sink.chunk(frame).await?;
                    }
                }
                return sink.finish().await;
            }
            stream_failure(reader, writer, handler)
        }
        StreamEnd::Aborted(abort) => {
            cancel.cancel();
            match abort {
                Abort::Deadline => Some(deadline_exceeded()),
                Abort::BodyFailed(e) => Some(body_read_error(&e)),
                // Not reachable here (nothing reports the peer gone), but a
                // gone peer has nothing left to answer.
                Abort::PeerGone => return Ok(()),
            }
        }
    };

    match failure {
        Some(e) if opened => {
            if let Some(frame) = stream_error_frame(protocol, &request, &e) {
                sink.chunk(frame).await?;
            }
            sink.finish().await
        }
        Some(e) => {
            let mut response = blank_response(&request);
            write_error(&mut response, &request, &e);
            sink.whole(response).await
        }
        None if opened => sink.finish().await,
        None => {
            // No frames and no error — an empty successful stream.
            let mut response = blank_response(&request);
            response.status = Status::OK;
            response
                .headers
                .insert(SimpleHeader::CONTENT_TYPE, vec![content_type]);
            if let Some((name, enc)) = response_encoding {
                response.headers.insert(SimpleHeader::from(name), vec![enc]);
            }
            response.body = Some(SendSafeBody::Bytes(Vec::new()));
            sink.whole(response).await
        }
    }
}

/// How a streaming exchange ended.
pub(crate) enum StreamEnd {
    /// The response side finished; the outcome of each part. The reader's is
    /// `Ok` when the request side was abandoned unfinished.
    Done {
        reader: ConnectResult<()>,
        writer: ConnectResult<()>,
        handler: ConnectResult<()>,
    },
    /// The call was cut short; its parts have been dropped.
    Aborted(Abort),
}

/// Why a streaming exchange was cut short.
pub(crate) enum Abort {
    /// The call's deadline passed.
    Deadline,
    /// The request body could not be read.
    BodyFailed(io::Error),
    /// The peer reset the stream or went away; nothing can be sent.
    PeerGone,
}

/// Run a streaming exchange — the request side (body feeder + protocol reader)
/// and the response side (protocol writer + handler) — until the response side
/// finishes, or the call is cut short by the request side failing, the
/// deadline (`expiry`), or the peer going away (`gone`).
///
/// The response side alone decides a normal end: a peer may hold its request
/// stream open until it sees the response end (a gRPC bidi peer like
/// buildkitd's fsutil does), so once the response is complete the request
/// side is abandoned (RFC 9113 lets a server end a stream whose request is
/// still open).
///
/// When the call is cut short, `cancel` fires *before* the handler is dropped,
/// and the response side is polled once more, so a handler awaiting
/// `ctx.cancelled()` runs its cancellation path.
pub(crate) async fn exchange_until_aborted<Req, Resp, Expiry, Gone>(
    request_side: Req,
    response_side: Resp,
    expiry: Expiry,
    gone: Gone,
    cancel: &CancelSignal,
) -> StreamEnd
where
    Req: std::future::Future<Output = Result<((), ConnectResult<()>), io::Error>>,
    Resp: std::future::Future<Output = (ConnectResult<()>, ConnectResult<()>)>,
    Expiry: std::future::Future<Output = ()>,
    Gone: std::future::Future<Output = ()>,
{
    let request_side = std::pin::pin!(request_side);
    let response_side = std::pin::pin!(response_side);
    let exchange = std::pin::pin!(async move {
        match select(request_side, response_side).await {
            Either::Left((Ok(((), reader)), response_side)) => {
                let (writer, handler) = response_side.await;
                StreamEnd::Done {
                    reader,
                    writer,
                    handler,
                }
            }
            Either::Left((Err(e), response_side)) => {
                cancel_and_settle(cancel, response_side);
                StreamEnd::Aborted(Abort::BodyFailed(e))
            }
            Either::Right(((writer, handler), _request_side)) => StreamEnd::Done {
                reader: Ok(()),
                writer,
                handler,
            },
        }
    });
    let expiry = std::pin::pin!(expiry);
    let gone = std::pin::pin!(gone);
    let cut_short = select(expiry, gone);
    match select(exchange, cut_short).await {
        Either::Left((end, _)) => end,
        Either::Right((Either::Left(_), exchange)) => {
            cancel_and_settle(cancel, exchange);
            StreamEnd::Aborted(Abort::Deadline)
        }
        Either::Right((Either::Right(_), exchange)) => {
            cancel_and_settle(cancel, exchange);
            StreamEnd::Aborted(Abort::PeerGone)
        }
    }
}

/// Fire the call's `cancel`, then poll `rest` — the part of the call still
/// holding the handler — once, so a handler awaiting `ctx.cancelled()` runs
/// its cancellation path before it is dropped. One poll, not a wait: a handler
/// that ignores cancellation must not hold the stream open.
pub(crate) fn cancel_and_settle<F: std::future::Future>(
    cancel: &CancelSignal,
    rest: std::pin::Pin<&mut F>,
) {
    cancel.cancel();
    let _ = futures::FutureExt::now_or_never(rest);
}

/// The error to end an already-started stream with when the call failed
/// without closing it (a closing handler returns `Ok`). A writer failure is the
/// cause of the handler's (its sends fail once the writer is gone), so it wins.
pub(crate) fn unclosed_stream_error(
    writer: ConnectResult<()>,
    handler: ConnectResult<()>,
) -> Option<ErrorTrace<ConnectError>> {
    match (writer, handler) {
        (Err(writer), Err(_)) => Some(writer),
        (Ok(()), Err(handler)) => Some(handler),
        (_, Ok(())) => None,
    }
}

/// The error a stream that produced nothing failed with, if any.
pub(crate) fn stream_failure(
    reader: ConnectResult<()>,
    writer: ConnectResult<()>,
    handler: ConnectResult<()>,
) -> Option<ErrorTrace<ConnectError>> {
    handler.err().or(writer.err()).or(reader.err())
}

/// The RPC error for a request body that could not be read.
pub(crate) fn body_read_error(error: &io::Error) -> ErrorTrace<ConnectError> {
    tracing::warn!(error = %error, "request body stream failed; cancelling the RPC");
    let code = if error.kind() == io::ErrorKind::TimedOut {
        crate::shared::error::Code::DeadlineExceeded
    } else {
        crate::shared::error::Code::Canceled
    };
    ConnectError::new(code, format!("request body could not be read: {error}")).into()
}

/// The bytes that end an already-started stream with `error`, in band, if the
/// protocol frames its outcome in the body. Built before any write so no
/// borrow of the request is held across an await.
pub(crate) fn stream_error_frame(
    protocol: &dyn ProtocolHandler,
    request: &SimpleIncomingRequest,
    error: &ErrorTrace<ConnectError>,
) -> Option<Bytes> {
    match protocol.encode_stream_error(request, error) {
        Ok(frame) => frame,
        Err(e) => {
            // The stream still ends; the peer sees it end without a status.
            tracing::error!(error = ?e, "could not encode the stream's terminal error");
            None
        }
    }
}

// ── helpers ───────────────────────────────────────────────────────────────────

/// The routing key: the path with any query string stripped (R1 leading slash).
pub(crate) fn extract_path(url: &str) -> String {
    url.split('?').next().unwrap_or(url).to_string()
}

/// The union of HTTP methods any protocol on this entry accepts (for `Allow`).
///
/// Connect GET is only valid for idempotent/NoSideEffects procedures. When the
/// procedure is not NoSideEffects, GET is excluded from the allowed list so that
/// a GET to a POST-only endpoint correctly returns 405 (Decision 05 §Unary GET).
pub(crate) fn allowed_methods(entry: &HandlerEntry) -> Vec<SimpleMethod> {
    let mut methods: Vec<SimpleMethod> = Vec::new();
    let is_nosideeffects = entry.spec.idempotency == IdempotencyLevel::NoSideEffects;
    for protocol in &entry.protocol_handlers {
        for method in protocol.allowed_methods() {
            // Filter out GET from Connect protocol when the procedure is not
            // NoSideEffects — the protocol handler always advertises GET, but
            // procedure-level idempotency restricts it (Decision 05 §Unary GET).
            if !is_nosideeffects && method == &SimpleMethod::GET {
                continue;
            }
            if !methods.contains(method) {
                methods.push(method.clone());
            }
        }
    }
    methods
}

/// Server-side transport capabilities inferred from the request's HTTP version.
pub(crate) fn capabilities_for(proto: &Proto) -> TransportCapabilities {
    let h2 = *proto >= Proto::HTTP20;
    TransportCapabilities {
        // HTTP/1.1 chunked and HTTP/2+ can both stream the request body.
        request_streaming: true,
        full_duplex: h2,
        h2_trailers: h2,
        http_versions: match proto {
            Proto::HTTP10 => &[Proto::HTTP10],
            Proto::HTTP11 => &[Proto::HTTP11],
            Proto::HTTP20 => &[Proto::HTTP20],
            Proto::HTTP30 => &[Proto::HTTP30],
            Proto::Custom(_) => &[Proto::HTTP11],
        },
        multiplexed: h2,
    }
}

/// Build the per-call [`Ctx`] the dispatcher hands the handler, around the
/// call's `cancel` signal.
pub(crate) fn build_ctx(
    bag: Arc<ContextBag>,
    request: &SimpleIncomingRequest,
    spec: Spec,
    protocol: &dyn ProtocolHandler,
    cancel: CancelSignal,
) -> Ctx {
    let deadline = protocol
        .parse_timeout(&request.headers)
        .map(|d| Instant::now() + d);
    let peer = Peer {
        addr: request
            .connection
            .peer_addr
            .as_ref()
            .map(|a| format!("{a:?}"))
            .unwrap_or_default(),
        protocol: protocol_name(protocol.kind()).to_string(),
    };
    let request_context = RequestContext::for_dispatch(
        spec,
        peer,
        request.headers.clone(),
        deadline,
        foundation_netio::shared::http::Extensions::new(),
        request.connection.clone(),
        cancel,
    );
    Ctx {
        bag,
        request: request_context,
    }
}

/// P7: a required Connect procedure must carry the version marker —
/// `Connect-Protocol-Version: 1` on POST, or `connect=v1` in the GET query.
pub(crate) fn require_connect_version(request: &SimpleIncomingRequest) -> ConnectResult<()> {
    use crate::shared::protocol::connect::constants;
    let present = if request.method == SimpleMethod::GET {
        request
            .request_url
            .queries
            .as_ref()
            .and_then(|q| q.get(constants::QUERY_CONNECT_VERSION))
            .is_some_and(|v| v == constants::QUERY_CONNECT_VERSION_VALUE)
    } else {
        request
            .headers
            .get(&SimpleHeader::from(
                constants::HEADER_PROTOCOL_VERSION.to_string(),
            ))
            .and_then(|v| v.first())
            .is_some_and(|v| v == constants::PROTOCOL_VERSION)
    };
    if present {
        Ok(())
    } else {
        Err(ConnectError::invalid_argument(format!(
            "missing required {} header",
            constants::HEADER_PROTOCOL_VERSION
        ))
        .into())
    }
}

fn protocol_name(kind: ProtocolKind) -> &'static str {
    match kind {
        ProtocolKind::Connect => "connect",
        ProtocolKind::Grpc => "grpc",
        ProtocolKind::GrpcWeb => "grpc-web",
    }
}

/// Why a unary request body could not be read.
enum UnaryBodyError {
    /// The body passed `read_max_bytes`; carries the size read so far.
    TooLarge(usize),
    /// The body stream failed.
    Read(io::Error),
}

/// Read a unary request body, stopping as soon as it passes `max_bytes`
/// (`0` means unlimited). The body is consumed chunk by chunk, so an oversized
/// body is rejected without being held whole.
async fn read_unary_body(
    body: &mut RequestBody<'_>,
    max_bytes: usize,
) -> Result<Bytes, UnaryBodyError> {
    let mut buf = bytes::BytesMut::new();
    while let Some(chunk) = body.next().await {
        let chunk = chunk.map_err(UnaryBodyError::Read)?;
        buf.extend_from_slice(&chunk);
        if max_bytes > 0 && buf.len() > max_bytes {
            return Err(UnaryBodyError::TooLarge(buf.len()));
        }
    }
    Ok(buf.freeze())
}

/// A blank response carrying the request's HTTP version and no headers/body.
pub(crate) fn blank_response(request: &SimpleIncomingRequest) -> SimpleOutgoingResponse {
    SimpleOutgoingResponse {
        proto: request.proto.clone(),
        status: Status::OK,
        headers: SimpleHeaders::new(),
        body: None,
        trailers: SimpleHeaders::new(),
    }
}

pub(crate) fn status_only(
    request: &SimpleIncomingRequest,
    status: Status,
) -> SimpleOutgoingResponse {
    let mut response = blank_response(request);
    response.status = status;
    response
}

pub(crate) fn method_not_allowed(
    request: &SimpleIncomingRequest,
    allowed: &[SimpleMethod],
) -> SimpleOutgoingResponse {
    let mut response = status_only(request, Status::MethodNotAllowed);
    let allow = allowed
        .iter()
        .map(SimpleMethod::to_string)
        .collect::<Vec<_>>()
        .join(", ");
    response
        .headers
        .insert(SimpleHeader::from("allow".to_string()), vec![allow]);
    response
}

pub(crate) fn unsupported_media_type(
    request: &SimpleIncomingRequest,
    entry: &HandlerEntry,
) -> SimpleOutgoingResponse {
    let mut response = status_only(request, Status::UnsupportedMediaType);
    // Accept-Post advertises the codecs (as content-types) this procedure accepts.
    let names = entry.codec_meta.codec_names();
    let accept_post = names
        .iter()
        .map(|codec| format!("application/{codec}"))
        .collect::<Vec<_>>()
        .join(", ");
    if !accept_post.is_empty() {
        response.headers.insert(
            SimpleHeader::from("accept-post".to_string()),
            vec![accept_post],
        );
    }
    response
}

pub(crate) fn http_version_not_supported(
    request: &SimpleIncomingRequest,
) -> SimpleOutgoingResponse {
    status_only(request, Status::HttpVersionNotSupported)
}

/// Render an RPC error into `response` via the protocol-aware [`ErrorWriter`].
pub(crate) fn write_error(
    response: &mut SimpleOutgoingResponse,
    request: &SimpleIncomingRequest,
    error: &foundation_errstacks::ErrorTrace<ConnectError>,
) {
    if let Err(e) = ErrorWriter::new().write(response, request, error) {
        // The response keeps whatever status was set before the failure.
        tracing::error!(error = ?e, "could not render the error response");
    }
}

// ── HTTP/2 dispatch ───────────────────────────────────────────────────────────

/// Rebuild the request shell from an h2 HEADERS frame. The body is supplied
/// separately (a pipe), so it stays `None` here.
#[cfg(not(target_family = "wasm"))]
pub(crate) fn request_from_header(header: &SimpleIncomingRequestHeader) -> SimpleIncomingRequest {
    SimpleIncomingRequest {
        proto: Proto::HTTP20,
        request_uri: header.uri.clone(),
        request_url: header.url.clone(),
        body: None,
        headers: header.headers.clone(),
        method: header.method.clone(),
        extensions: None,
        connection: header.connection.clone(),
    }
}

/// Headers that are connection-specific and forbidden on h2 (RFC 7540 §8.1.2.2).
#[cfg(not(target_family = "wasm"))]
const H2_FORBIDDEN_HEADERS: [&str; 5] = [
    "connection",
    "keep-alive",
    "proxy-connection",
    "transfer-encoding",
    "upgrade",
];

/// Flatten `SimpleHeaders` into h2 wire pairs.
///
/// h2 field names must be lowercase (RFC 7540 §8.1.2) — a capitalised name is a
/// protocol error at a strict peer, so names are lowered rather than trusted.
#[cfg(not(target_family = "wasm"))]
pub(crate) fn h2_headers(headers: &SimpleHeaders) -> Vec<(Bytes, Bytes)> {
    let mut out = Vec::new();
    for (key, values) in headers.iter() {
        let name = key.to_string().to_lowercase();
        if H2_FORBIDDEN_HEADERS.contains(&name.as_str()) {
            continue;
        }
        for value in values {
            out.push((Bytes::from(name.clone()), Bytes::from(value.clone())));
        }
    }
    out
}

/// The pipe is gone because the connection handler dropped the stream.
#[cfg(not(target_family = "wasm"))]
fn stream_gone<T>(_: T) -> io::Error {
    io::Error::new(io::ErrorKind::BrokenPipe, "h2 response stream closed")
}

/// Serialize a complete (non-streaming) response: HEADERS, then one DATA frame
/// per body chunk, then optional trailing HEADERS.
#[cfg(not(target_family = "wasm"))]
async fn send_whole_response(
    tx: &PipeSender<H2Frame>,
    mut response: SimpleOutgoingResponse,
) -> io::Result<()> {
    let status = response.status.into_usize() as u16;
    let headers = h2_headers(&response.headers);
    let has_trailers = !response.trailers.is_empty();
    let body = response.body.take();

    fn next_chunk(it: &mut Option<SendSafeBodyBytesIterator>) -> io::Result<Option<Bytes>> {
        let Some(ref mut iter) = it else {
            return Ok(None);
        };
        loop {
            match iter.next() {
                Some(Stream::Next(SendSafeBodyBytesItem::Chunk(b))) => return Ok(Some(b)),
                Some(Stream::Next(SendSafeBodyBytesItem::StreamError(e))) => {
                    return Err(io::Error::other(e.to_string()));
                }
                // The body's transport has nothing yet: this writer cannot wait
                // for it, so report it rather than spin.
                Some(Stream::Wait) => {
                    return Err(io::Error::new(
                        io::ErrorKind::WouldBlock,
                        "response body not ready",
                    ));
                }
                None => return Ok(None),
                _ => {} // Ignore, Init, etc — skip.
            }
        }
    }

    let mut iter = body.map(SendSafeBodyBytesIterator::new);
    let mut chunk = next_chunk(&mut iter)?;
    let end_stream = chunk.is_none() && !has_trailers;

    tx.send(H2Frame::Headers {
        status,
        headers,
        end_stream,
    })
    .await
    .map_err(stream_gone)?;

    while let Some(cur) = chunk.take() {
        let next = next_chunk(&mut iter)?;
        let data_end = next.is_none() && !has_trailers;
        tx.send(H2Frame::Data {
            payload: cur,
            end_stream: data_end,
        })
        .await
        .map_err(stream_gone)?;
        chunk = next;
    }

    if has_trailers {
        // Real trailing HEADERS (no pseudo-headers) — the old shape here was a
        // `Headers { status: 0, .. }` frame, which put an invalid `:status: 0`
        // pseudo-header in the trailers (RFC 9113 §8.1 forbids them).
        tx.send(H2Frame::Trailers {
            headers: h2_headers(&response.trailers),
        })
        .await
        .map_err(stream_gone)?;
    }

    Ok(())
}

/// What reading an h2 unary request body produced.
#[cfg(not(target_family = "wasm"))]
enum H2Body {
    Collected(Vec<u8>),
    /// The peer reset the stream, or the connection dropped it.
    Reset,
    /// The body passed `read_max_bytes`; carries the size read so far.
    TooLarge(usize),
}

/// The next request DATA chunk of an h2 stream: `Ok(None)` once the peer set
/// `END_STREAM`, and an error when the peer reset the stream or the
/// connection dropped it — never a clean end.
///
/// The connection closes the stream's response pipe (`tx`) before its body
/// pipe when it drops a stream, so a body pipe that closes under a closed
/// response pipe is a cancelled stream, not `END_STREAM`.
#[cfg(not(target_family = "wasm"))]
async fn next_h2_chunk(
    body: &PipeReceiver<H2IncomingFrame>,
    tx: &PipeSender<H2Frame>,
) -> io::Result<Option<Bytes>> {
    match body.receive().await {
        Some(H2IncomingFrame::Data(chunk)) => Ok(Some(chunk)),
        Some(H2IncomingFrame::Reset(code)) => Err(io::Error::new(
            io::ErrorKind::ConnectionReset,
            format!("the peer reset the h2 stream ({code:?})"),
        )),
        None if tx.is_closed() => Err(io::Error::new(
            io::ErrorKind::ConnectionReset,
            "the h2 stream was dropped before its request ended",
        )),
        None => Ok(None),
    }
}

/// Drain the inbound h2 body pipe into a buffer (unary: exactly one message),
/// stopping as soon as it passes `max_bytes` (`0` means unlimited) so an
/// oversized body is never held whole.
///
/// A peer RST_STREAM (or a dropped stream) ends collection early; the caller
/// abandons the stream.
#[cfg(not(target_family = "wasm"))]
async fn collect_h2_body(
    body: &PipeReceiver<H2IncomingFrame>,
    tx: &PipeSender<H2Frame>,
    max_bytes: usize,
) -> H2Body {
    let mut out = Vec::new();
    loop {
        match next_h2_chunk(body, tx).await {
            Ok(Some(chunk)) => {
                out.extend_from_slice(&chunk);
                if max_bytes > 0 && out.len() > max_bytes {
                    return H2Body::TooLarge(out.len());
                }
            }
            Ok(None) => return H2Body::Collected(out),
            Err(e) => {
                tracing::debug!(error = %e, "h2 unary request abandoned");
                return H2Body::Reset;
            }
        }
    }
}

#[cfg(not(target_family = "wasm"))]
impl ConnectRpcHandler {
    /// HTTP/2 dispatch: bridge one h2 stream through the router.
    ///
    /// Unlike [`dispatch`](Self::dispatch) this never materialises a
    /// `SimpleOutgoingResponse` for streaming procedures — response frames are
    /// pushed into `tx` as the handler produces them, and request DATA frames
    /// are forwarded into the protocol's byte pipe as they arrive.
    ///
    /// The call is cancelled — its `Ctx` fires and the handler is dropped — when
    /// the peer resets the stream, the connection drops it (`tx` closes), or
    /// the call's deadline passes (answered with `deadline_exceeded`).
    pub fn dispatch_h2(
        &self,
        bag: Arc<ContextBag>,
        header: SimpleIncomingRequestHeader,
        body: PipeReceiver<H2IncomingFrame>,
        tx: PipeSender<H2Frame>,
    ) -> BoxFuture<'static, io::Result<()>> {
        let router = self.router.clone();
        Box::pin(dispatch_h2_stream(router, bag, header, body, tx))
    }
}

#[cfg(not(target_family = "wasm"))]
async fn dispatch_h2_stream(
    router: Arc<Router>,
    bag: Arc<ContextBag>,
    header: SimpleIncomingRequestHeader,
    body: PipeReceiver<H2IncomingFrame>,
    tx: PipeSender<H2Frame>,
) -> io::Result<()> {
    let request = request_from_header(&header);
    let path = extract_path(&request.request_url.url);

    // Decision phase — identical to `dispatch_request`, but every rejection is
    // serialized as h2 frames rather than returned.
    let Some(entry) = router.lookup(&path) else {
        return send_whole_response(&tx, status_only(&request, Status::NotFound)).await;
    };

    let allowed = allowed_methods(entry);
    if !allowed.contains(&request.method) {
        return send_whole_response(&tx, method_not_allowed(&request, &allowed)).await;
    }

    let Some(protocol) = entry
        .protocol_handlers
        .iter()
        .find(|p| p.can_handle(&request))
    else {
        return send_whole_response(&tx, unsupported_media_type(&request, entry)).await;
    };
    let protocol = protocol.as_ref();

    let Some(codec_name) = protocol.codec_name(&request) else {
        return send_whole_response(&tx, unsupported_media_type(&request, entry)).await;
    };
    if !entry.codec_meta.has_codec(&codec_name) {
        return send_whole_response(&tx, unsupported_media_type(&request, entry)).await;
    }

    let caps = capabilities_for(&request.proto);
    let reqs = requirements(protocol.kind(), entry.spec.stream_type);
    if check_compatible(&reqs, &caps).is_err() {
        return send_whole_response(&tx, http_version_not_supported(&request)).await;
    }

    if entry.options.require_connect_protocol_header && protocol.kind() == ProtocolKind::Connect {
        if let Err(e) = require_connect_version(&request) {
            let mut response = blank_response(&request);
            write_error(&mut response, &request, &e);
            return send_whole_response(&tx, response).await;
        }
    }

    let compression = entry
        .options
        .compression
        .as_ref()
        .unwrap_or_else(|| router.global_compression());
    let ctx = build_ctx(
        bag,
        &request,
        entry.spec.clone(),
        protocol,
        CancelSignal::new(),
    );
    let spec = entry.spec.clone();
    let pipe_depth = entry.options.pipe_depth;

    match &entry.handler {
        HandlerKind::Unary(handler) => {
            // Unary carries exactly one message: collect it, then reuse the
            // sequential unary path.
            let read_max_bytes = entry.options.limits.read_max_bytes;
            let cancel = ctx.cancel_signal().clone();
            let deadline = ctx.request.deadline;
            let timed_out = deadline.map(|_| deadline_response(&request));
            let too_large = status_only(&request, Status::PayloadTooLarge);
            let handler = handler.clone();
            let (body, tx_ref) = (&body, &tx);
            let call = std::pin::pin!(async move {
                let collected = match collect_h2_body(body, tx_ref, read_max_bytes).await {
                    H2Body::Collected(collected) => collected,
                    // The peer reset the stream — nothing to answer.
                    H2Body::Reset => return None,
                    H2Body::TooLarge(len) => {
                        tracing::warn!(
                            len,
                            read_max_bytes,
                            "request body exceeded read_max_bytes; returning 413"
                        );
                        return Some(too_large);
                    }
                };
                let mut request = request;
                request.body = Some(SendSafeBody::Bytes(collected));
                Some(
                    run_unary(
                        handler,
                        protocol,
                        request,
                        codec_name,
                        compression,
                        ctx,
                        read_max_bytes,
                        // The deadline is enforced below, over the body read too.
                        None,
                    )
                    .await,
                )
            });
            let expiry = std::pin::pin!(deadline_expiry(Some(&ThreadTimer), deadline));
            let gone = std::pin::pin!(tx.closed());
            let cut_short = select(expiry, gone);
            match select(call, cut_short).await {
                Either::Left((Some(response), _)) => send_whole_response(&tx, response).await,
                Either::Left((None, _)) | Either::Right((Either::Right(_), _)) => {
                    // Reset by the peer, or dropped by the connection.
                    cancel.cancel();
                    Ok(())
                }
                Either::Right((Either::Left(_), _call)) => {
                    cancel.cancel();
                    match timed_out {
                        Some(response) => send_whole_response(&tx, response).await,
                        None => Ok(()),
                    }
                }
            }
        }
        HandlerKind::ServerStream(handler)
        | HandlerKind::ClientStream(handler)
        | HandlerKind::BidiStream(handler) => {
            run_streaming_h2(
                handler.clone(),
                protocol,
                request,
                spec,
                codec_name,
                compression,
                ctx,
                pipe_depth,
                entry.options.limits.read_max_bytes,
                body,
                &tx,
            )
            .await
        }
    }
}

/// Streaming over h2: request DATA frames feed the protocol's byte pipe while
/// the protocol's output pipe drains into response DATA frames — concurrently,
/// so a bidi handler can read and write at the same time.
///
/// A peer reset, the connection dropping the stream, or a failed write cancels
/// the call and drops the handler; nothing more is sent. The call's deadline
/// passing does the same, but the call is still answered with
/// `deadline_exceeded`.
#[allow(clippy::too_many_arguments)]
#[cfg(not(target_family = "wasm"))]
async fn run_streaming_h2(
    handler: Arc<dyn super::erased::ErasedStreamHandler>,
    protocol: &dyn ProtocolHandler,
    request: SimpleIncomingRequest,
    spec: Spec,
    codec_name: String,
    compression: &crate::shared::compression::CompressionRegistry,
    ctx: Ctx,
    pipe_depth: usize,
    read_max_bytes: usize,
    body: PipeReceiver<H2IncomingFrame>,
    tx: &PipeSender<H2Frame>,
) -> io::Result<()> {
    let (req_tx, req_rx) = Pipe::<Bytes>::with_depth(pipe_depth);
    let (resp_tx, resp_rx) = Pipe::<Bytes>::with_depth(pipe_depth);

    let cancel = ctx.cancel_signal().clone();
    let deadline = ctx.request.deadline;
    let config = ExchangeConfig {
        compression,
        cancel: cancel.clone(),
        read_max_bytes,
    };
    let exchange = match protocol.new_conn(&request, spec, req_rx, resp_tx, config) {
        Ok(exchange) => exchange,
        Err(e) => {
            let mut response = blank_response(&request);
            write_error(&mut response, &request, &e);
            return send_whole_response(tx, response).await;
        }
    };
    let crate::shared::protocol::HandlerExchange {
        conn,
        reader_task,
        writer_task,
    } = exchange;

    // Forward inbound DATA frames as they arrive — no buffering of the body. A
    // reset is not an end of the body: it aborts the call.
    let feeder = async {
        while let Some(chunk) = next_h2_chunk(&body, tx).await? {
            if req_tx.send(chunk).await.is_err() {
                break;
            }
        }
        req_tx.close();
        Ok::<(), io::Error>(())
    };

    // Forward framed response bytes out as DATA frames. HEADERS are withheld
    // until the first chunk so that a handler which fails before emitting
    // anything can still be reported with a real status. The pump owns
    // `resp_rx`, so when it stops the writer task's sends fail instead of
    // waiting on a full pipe forever.
    let content_type = protocol.streaming_response_content_type(&request, &codec_name);
    // The writer task compressed per this same negotiation — a flagged envelope
    // is only valid when the encoding header goes out with the response HEADERS.
    let response_encoding = protocol.streaming_response_encoding(&request, compression);
    let pump_content_type = content_type.clone();
    let pump = async {
        let resp_rx = resp_rx;
        let mut opened = false;
        while let Some(chunk) = resp_rx.receive().await {
            if !opened {
                opened = true;
                let mut headers = vec![(
                    Bytes::from_static(b"content-type"),
                    Bytes::from(pump_content_type.clone()),
                )];
                if let Some((name, enc)) = &response_encoding {
                    headers.push((Bytes::from(name.clone()), Bytes::from(enc.clone())));
                }
                tx.send(H2Frame::Headers {
                    status: 200,
                    headers,
                    end_stream: false,
                })
                .await
                .map_err(stream_gone)?;
            }
            tx.send(H2Frame::Data {
                payload: chunk,
                end_stream: false,
            })
            .await
            .map_err(stream_gone)?;
        }
        Ok::<bool, io::Error>(opened)
    };

    let handler_fut = handler.handle(ctx, &codec_name, conn);

    let request_side = futures::future::try_join(feeder, async {
        Ok::<_, io::Error>(reader_task.await)
    });
    let response_side = futures::future::join(writer_task, handler_fut);
    let rest = std::pin::pin!(exchange_until_aborted(
        request_side,
        response_side,
        deadline_expiry(Some(&ThreadTimer), deadline),
        tx.closed(),
        &cancel,
    ));
    let (opened, end) = {
        let pump = std::pin::pin!(pump);
        match select(pump, rest).await {
            // The stream is gone: cancel the call and drop everything now.
            Either::Left((Err(e), rest)) => {
                cancel_and_settle(&cancel, rest);
                return Err(e);
            }
            Either::Left((Ok(opened), rest)) => (opened, rest.await),
            // The writer has finished or been dropped, which ends the pump.
            Either::Right((end, pump)) => match pump.await {
                Ok(opened) => (opened, end),
                Err(e) => {
                    cancel.cancel();
                    return Err(e);
                }
            },
        }
    };

    let is_grpc = protocol.kind() == ProtocolKind::Grpc;
    let failure = match end {
        StreamEnd::Aborted(Abort::PeerGone | Abort::BodyFailed(_)) => {
            // Reset by the peer or dropped by the connection: nothing more can
            // be sent on this stream.
            cancel.cancel();
            return Ok(());
        }
        StreamEnd::Aborted(Abort::Deadline) => {
            cancel.cancel();
            Some(deadline_exceeded())
        }
        // An opened Connect / gRPC-Web stream framed its own outcome unless the
        // call failed without closing it.
        StreamEnd::Done {
            writer, handler, ..
        } if opened && !is_grpc => unclosed_stream_error(writer, handler),
        StreamEnd::Done {
            reader,
            writer,
            handler,
        } => stream_failure(reader, writer, handler),
    };

    if opened {
        // The response HEADERS are out. Close the stream per the protocol:
        // gRPC's outcome rides trailing HEADERS (`grpc-status`), and grpc-go
        // hangs/errors on a stream that ends without them. Connect and
        // gRPC-Web carry their terminal frame in-band (EndStreamResponse /
        // 0x80 trailer frame — written by the protocol writer task, or here
        // when the call failed without closing its stream), so a bare
        // END_STREAM DATA closes those.
        let closing = if is_grpc {
            H2Frame::Trailers {
                headers: grpc_status_trailers(failure.as_ref()),
            }
        } else {
            if let Some(frame) = failure
                .as_ref()
                .and_then(|e| stream_error_frame(protocol, &request, e))
            {
                tx.send(H2Frame::Data {
                    payload: frame,
                    end_stream: false,
                })
                .await
                .map_err(stream_gone)?;
            }
            H2Frame::Data {
                payload: Bytes::new(),
                end_stream: true,
            }
        };
        tx.send(closing).await.map_err(stream_gone)?;
        return Ok(());
    }

    // Nothing was emitted: report the failure, or an empty successful stream.
    let mut response = blank_response(&request);
    if let Some(e) = failure {
        write_error(&mut response, &request, &e);
    } else {
        response.status = Status::OK;
        response
            .headers
            .insert(SimpleHeader::CONTENT_TYPE, vec![content_type]);
        if is_grpc {
            // A gRPC stream with zero messages is still a gRPC response — it
            // MUST end in `grpc-status` trailers or the client reports
            // `Unknown` with an empty message (bit BuildKit's FileSend sink,
            // whose response stream is legitimately empty).
            response
                .trailers
                .insert(SimpleHeader::from("grpc-status".to_string()), vec!["0".to_string()]);
        }
    }
    send_whole_response(tx, response).await
}

/// gRPC trailing headers for a streaming outcome: `grpc-status: 0` on success,
/// the error's code and percent-encoded message otherwise (gRPC spec §Responses).
pub(crate) fn grpc_status_trailers(
    error: Option<&foundation_errstacks::ErrorTrace<crate::ConnectError>>,
) -> Vec<(Bytes, Bytes)> {
    match error {
        None => vec![(Bytes::from_static(b"grpc-status"), Bytes::from_static(b"0"))],
        Some(e) => {
            let ctx = e.current_context();
            let code = ctx.code() as u32;
            let message = ctx.message().to_string();
            let mut encoded = String::new();
            for b in message.bytes() {
                // Percent-encode per gRPC spec: space and non-printable/percent.
                if b == b'%' || !(0x20..=0x7e).contains(&b) {
                    encoded.push_str(&format!("%{b:02X}"));
                } else {
                    encoded.push(b as char);
                }
            }
            vec![
                (Bytes::from_static(b"grpc-status"), Bytes::from(code.to_string())),
                (Bytes::from_static(b"grpc-message"), Bytes::from(encoded)),
            ]
        }
    }
}
