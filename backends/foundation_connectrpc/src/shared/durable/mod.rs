//! The ConnectRPC adapter for Durable Streams.
//!
//! WHY: clients reach the protocol core through RPC. This adapter owns the
//! method names, the status codes and the response stream. The core stays
//! transport-neutral.
//!
//! WHAT: [`ConnectRpcAdapter`] implements the generated
//! [`DurableStreamService`](proto::v1::durable_stream_service::DurableStreamService)
//! trait over a [`DurableService`]. [`durable_stream_router`] registers it on a
//! [`Router`].
//!
//! HOW: each unary method converts one generated request to a core request,
//! calls the service and converts the result. `Read` returns one bounded page.
//! `Subscribe` returns a [`Stream`] that starts with one page and then waits on
//! the stream notifier for each append. The wait fires a waker, so the stream
//! never polls. The injected call context gives the cancellation signal.
//!
//! # Status table
//!
//! One [`StoreError`] maps to one ConnectRPC code. The HTTP intent comes from
//! the adapter status table, where a code exists.
//!
//! | `StoreError` | HTTP intent | ConnectRPC code |
//! |---|---|---|
//! | `NotFound` | `404` | `NotFound` |
//! | `Gone` | `410` | `NotFound` |
//! | `Trimmed` | `410` | `OutOfRange` |
//! | `Fenced` | `403` | `PermissionDenied` |
//! | `InvalidEpochSeq` | `400` | `InvalidArgument` |
//! | `SequenceGap` | `409` | `Aborted` |
//! | `StreamSeqConflict` | `409` | `Aborted` |
//! | `ContentTypeMismatch` | `409` | `FailedPrecondition` |
//! | `ConfigConflict` | `409` | `AlreadyExists` |
//! | `Closed` | `409` | `FailedPrecondition` |
//! | `BadRequest` | `400` | `InvalidArgument` |
//! | `RateLimited` | `429` | `ResourceExhausted` |
//! | `Unsupported` | `501` | `Unimplemented` |
//! | `Storage` | `500` | `Internal` |

pub mod proto;

use core::future::Future;
use core::pin::Pin;
use core::task::{Context as TaskContext, Poll};
use core::time::Duration;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};
use std::task::Waker;

use foundation_connectrpc::{Code, ConnectError, ConnectResult, Request, Response, Router, RpcCtx};
use foundation_core::durable::{
    wire, AppendKind, AppendRequest, BadRequestKind, Chunk, CloseRequest, ContentType, CreateSpec,
    DurableService, DurableStream, Expiry, ForkSpec, Producer, ProducerId, ReadFrom, ReadLimit,
    ReadPage, ReadRequest, ResumeCursor, StoreError, StreamHead, StreamName, StreamSeq,
};
use foundation_errstacks::ErrorTrace;
use futures::Stream;

use proto::v1 as pb;
use proto::v1::durable_stream_service::{register_durable_stream_service, DurableStreamService};

/// The largest integer that survives a JSON number without loss.
///
/// The HTTP adapter applies the same rule to its producer fields. A larger
/// value would change on a JSON round trip.
const MAX_SAFE_INTEGER: u64 = (1_u64 << 53) - 1;

/// A boxed future the live stream polls.
type BoxFuture<T> = Pin<Box<dyn Future<Output = T> + Send>>;

/// The `DurableStreamService` implementation over a protocol service.
pub struct ConnectRpcAdapter {
    service: Arc<DurableService<DurableStream>>,
}

impl ConnectRpcAdapter {
    /// Build the adapter on a protocol service.
    #[must_use]
    pub fn new(service: Arc<DurableService<DurableStream>>) -> Self {
        Self { service }
    }

    /// Return the protocol service.
    #[must_use]
    pub fn service(&self) -> &Arc<DurableService<DurableStream>> {
        &self.service
    }
}

/// A `Router` with every `DurableStreamService` procedure registered.
#[must_use]
pub fn durable_stream_router(service: Arc<ConnectRpcAdapter>) -> Router {
    let mut router = Router::new();
    register_durable_stream_service(&mut router, service);
    router
}

/// Map one core refusal to its `ConnectRPC` code.
///
/// The table in the module documentation is the contract. Change both
/// together.
#[must_use]
pub fn code_for(error: &StoreError) -> Code {
    match error {
        StoreError::NotFound | StoreError::Gone => Code::NotFound,
        StoreError::Trimmed => Code::OutOfRange,
        StoreError::Fenced { .. } => Code::PermissionDenied,
        StoreError::InvalidEpochSeq { .. } | StoreError::BadRequest(_) => Code::InvalidArgument,
        StoreError::SequenceGap { .. } | StoreError::StreamSeqConflict { .. } => Code::Aborted,
        StoreError::ContentTypeMismatch { .. } | StoreError::Closed { .. } => {
            Code::FailedPrecondition
        }
        StoreError::ConfigConflict => Code::AlreadyExists,
        StoreError::RateLimited { .. } => Code::ResourceExhausted,
        StoreError::Unsupported => Code::Unimplemented,
        StoreError::Storage(_) => Code::Internal,
    }
}

/// Turn a core refusal into a wire error.
fn status_error(error: StoreError) -> ErrorTrace<ConnectError> {
    let code = code_for(&error);
    tracing::debug!(code = code.as_str(), error = %error, "durable stream rpc error");
    ConnectError::with_source(code, error).into()
}

/// The wire error of a canceled stream.
fn canceled_error() -> ErrorTrace<ConnectError> {
    ConnectError::canceled("durable stream subscription canceled").into()
}

impl DurableStreamService for ConnectRpcAdapter {
    fn create(
        &self,
        _ctx: RpcCtx,
        request: Request<pb::CreateRequest>,
    ) -> impl Future<Output = ConnectResult<Response<pb::CreateResponse>>> + Send {
        let service = Arc::clone(&self.service);
        async move {
            let req = request.msg;
            let name = StreamName::parse(&req.name).map_err(status_error)?;
            let content_type = parse_content_type(req.content_type.as_deref())?;
            let expires = parse_expiry(req.ttl_ms, req.expires_at_unix_ms)?;
            let fork = req.fork.as_option().map(parse_fork).transpose()?;
            let spec = CreateSpec {
                content_type,
                expires,
                closed: req.closed,
                initial: req
                    .initial
                    .filter(|bytes| !bytes.is_empty())
                    .map(Chunk::new),
                fork,
            };
            let outcome = service.create(name, spec).await.map_err(status_error)?;
            Ok(Response::new(pb::CreateResponse {
                created: outcome.created,
                head: Some(head_message(&outcome.head)).into(),
                ..Default::default()
            }))
        }
    }

    fn append(
        &self,
        _ctx: RpcCtx,
        request: Request<pb::AppendRequest>,
    ) -> impl Future<Output = ConnectResult<Response<pb::AppendResponse>>> + Send {
        let service = Arc::clone(&self.service);
        async move {
            let req = request.msg;
            let name = StreamName::parse(&req.name).map_err(status_error)?;
            let producer = parse_producer(req.producer.as_option())?;
            let chunk = Chunk::new(req.chunk);
            let outcome = if chunk.is_empty() {
                // An empty chunk is a close-only request. Any other empty
                // append is malformed, as on the HTTP surface.
                if !req.close {
                    return Err(status_error(StoreError::BadRequest(BadRequestKind::Body)));
                }
                service
                    .close(
                        &name,
                        CloseRequest {
                            final_chunk: None,
                            producer,
                        },
                    )
                    .await
                    .map_err(status_error)?
            } else {
                let Some(content_type) = parse_content_type(req.content_type.as_deref())? else {
                    return Err(status_error(StoreError::BadRequest(BadRequestKind::Body)));
                };
                let stream_seq = req
                    .stream_seq
                    .map(|value| StreamSeq::from_bytes(value.into_bytes()));
                service
                    .append(
                        &name,
                        AppendRequest {
                            chunk,
                            content_type: Some(content_type),
                            stream_seq,
                            producer,
                            close: req.close,
                        },
                    )
                    .await
                    .map_err(status_error)?
            };
            Ok(Response::new(append_response(&outcome)))
        }
    }

    fn read(
        &self,
        _ctx: RpcCtx,
        request: Request<pb::ReadRequest>,
    ) -> impl Future<Output = ConnectResult<Response<pb::ReadResponse>>> + Send {
        let service = Arc::clone(&self.service);
        async move {
            let req = request.msg;
            let name = StreamName::parse(&req.name).map_err(status_error)?;
            let from = parse_read_from(req.offset.as_deref())?;
            let limit = parse_limit(req.max_bytes, req.max_chunks);
            let page = service
                .read(&name, ReadRequest { from, limit })
                .await
                .map_err(status_error)?;
            Ok(Response::new(page_message(page)))
        }
    }

    fn subscribe(
        &self,
        ctx: RpcCtx,
        request: Request<pb::SubscribeRequest>,
    ) -> impl Future<
        Output = ConnectResult<Pin<Box<dyn Stream<Item = ConnectResult<pb::ReadResponse>> + Send>>>,
    > + Send {
        let service = Arc::clone(&self.service);
        async move {
            let req = request.msg;
            let name = StreamName::parse(&req.name).map_err(status_error)?;
            // Fail the call at open when the stream is missing or gone.
            service.head(&name).await.map_err(status_error)?;
            let from = parse_read_from(req.offset.as_deref())?;
            let limit = parse_limit(req.max_bytes, req.max_chunks);
            let stream = SubscribeStream {
                live: service.notifier(&name).is_some(),
                service,
                name,
                from,
                limit,
                ctx,
                read: None,
                wake: None,
                cancel: None,
                first: true,
                finished: false,
            };
            Ok(Box::pin(stream)
                as Pin<
                    Box<dyn Stream<Item = ConnectResult<pb::ReadResponse>> + Send>,
                >)
        }
    }

    fn head(
        &self,
        _ctx: RpcCtx,
        request: Request<pb::HeadRequest>,
    ) -> impl Future<Output = ConnectResult<Response<pb::HeadResponse>>> + Send {
        let service = Arc::clone(&self.service);
        async move {
            let req = request.msg;
            let name = StreamName::parse(&req.name).map_err(status_error)?;
            let head = service.head(&name).await.map_err(status_error)?;
            Ok(Response::new(pb::HeadResponse {
                head: Some(head_message(&head)).into(),
                ..Default::default()
            }))
        }
    }

    fn close(
        &self,
        _ctx: RpcCtx,
        request: Request<pb::CloseRequest>,
    ) -> impl Future<Output = ConnectResult<Response<pb::AppendResponse>>> + Send {
        let service = Arc::clone(&self.service);
        async move {
            let req = request.msg;
            let name = StreamName::parse(&req.name).map_err(status_error)?;
            let producer = parse_producer(req.producer.as_option())?;
            let final_chunk = req
                .final_chunk
                .filter(|bytes| !bytes.is_empty())
                .map(Chunk::new);
            let outcome = service
                .close(
                    &name,
                    CloseRequest {
                        final_chunk,
                        producer,
                    },
                )
                .await
                .map_err(status_error)?;
            Ok(Response::new(append_response(&outcome)))
        }
    }

    fn delete(
        &self,
        _ctx: RpcCtx,
        request: Request<pb::DeleteRequest>,
    ) -> impl Future<Output = ConnectResult<Response<pb::DeleteResponse>>> + Send {
        let service = Arc::clone(&self.service);
        async move {
            let req = request.msg;
            let name = StreamName::parse(&req.name).map_err(status_error)?;
            let outcome = service.delete(&name).await.map_err(status_error)?;
            Ok(Response::new(pb::DeleteResponse {
                soft_deleted: outcome.soft_deleted,
                ..Default::default()
            }))
        }
    }

    fn fork(
        &self,
        _ctx: RpcCtx,
        request: Request<pb::ForkRequest>,
    ) -> impl Future<Output = ConnectResult<Response<pb::CreateResponse>>> + Send {
        let service = Arc::clone(&self.service);
        async move {
            let req = request.msg;
            let name = StreamName::parse(&req.name).map_err(status_error)?;
            let source = StreamName::parse(&req.source).map_err(status_error)?;
            let at = req.at.as_deref().map(parse_offset).transpose()?;
            let spec = ForkSpec {
                source,
                at,
                sub_offset: req.sub_offset,
            };
            let outcome = service.fork(name, spec).await.map_err(status_error)?;
            Ok(Response::new(pb::CreateResponse {
                created: outcome.created,
                head: Some(head_message(&outcome.head)).into(),
                ..Default::default()
            }))
        }
    }
}

/// The response of an append or a close.
fn append_response(outcome: &foundation_core::durable::AppendOutcome) -> pb::AppendResponse {
    pb::AppendResponse {
        cursor: Some(cursor_message(&outcome.cursor)).into(),
        duplicate: matches!(outcome.kind, AppendKind::Duplicate),
        producer_seq: outcome.producer_seq,
        closed: outcome.closed,
        ..Default::default()
    }
}

/// One read page on the wire.
fn page_message(page: ReadPage) -> pb::ReadResponse {
    pb::ReadResponse {
        chunks: page
            .chunks
            .into_iter()
            .map(|chunk| pb::Chunk {
                data: chunk.as_bytes().to_vec(),
                ..Default::default()
            })
            .collect(),
        cursor: Some(cursor_message(&page.cursor)).into(),
        ..Default::default()
    }
}

/// The resume state on the wire.
fn cursor_message(cursor: &ResumeCursor) -> pb::Cursor {
    pb::Cursor {
        next: wire::offset_text(&cursor.next),
        up_to_date: cursor.up_to_date,
        closed: cursor.closed,
        ..Default::default()
    }
}

/// The stream head on the wire.
fn head_message(head: &StreamHead) -> pb::StreamHead {
    let (ttl_ms, expires_at_unix_ms) = match head.expires {
        Some(Expiry::Ttl(ttl)) => (
            Some(u64::try_from(ttl.as_millis()).unwrap_or(u64::MAX)),
            None,
        ),
        Some(Expiry::Absolute(at)) => (None, Some(at)),
        None => (None, None),
    };
    pb::StreamHead {
        content_type: head.content_type.as_str().to_string(),
        tail: wire::offset_text(&head.tail),
        closed: head.closed,
        ttl_ms,
        expires_at_unix_ms,
        ..Default::default()
    }
}

/// Parse an optional content type. An empty value is absent.
fn parse_content_type(raw: Option<&str>) -> Result<Option<ContentType>, ErrorTrace<ConnectError>> {
    raw.filter(|value| !value.is_empty())
        .map(ContentType::parse)
        .transpose()
        .map_err(status_error)
}

/// Parse the two expiry fields into one core expiry.
fn parse_expiry(
    ttl_ms: Option<u64>,
    expires_at_unix_ms: Option<i64>,
) -> Result<Option<Expiry>, ErrorTrace<ConnectError>> {
    Expiry::from_headers(ttl_ms.map(Duration::from_millis), expires_at_unix_ms)
        .map_err(status_error)
}

/// Parse a producer position.
fn parse_producer(
    raw: Option<&pb::Producer>,
) -> Result<Option<Producer>, ErrorTrace<ConnectError>> {
    raw.map(|producer| {
        if producer.epoch > MAX_SAFE_INTEGER || producer.seq > MAX_SAFE_INTEGER {
            return Err(StoreError::BadRequest(BadRequestKind::Producer));
        }
        Ok(Producer {
            id: ProducerId::parse(&producer.id)?,
            epoch: producer.epoch,
            seq: producer.seq,
        })
    })
    .transpose()
    .map_err(status_error)
}

/// Parse a fork source and its divergence point.
fn parse_fork(fork: &pb::Fork) -> Result<ForkSpec, ErrorTrace<ConnectError>> {
    let source = StreamName::parse(&fork.source).map_err(status_error)?;
    let at = fork.at.as_deref().map(parse_offset).transpose()?;
    Ok(ForkSpec {
        source,
        at,
        sub_offset: fork.sub_offset,
    })
}

/// Parse a read offset. Absent starts at the beginning.
fn parse_read_from(raw: Option<&str>) -> Result<ReadFrom, ErrorTrace<ConnectError>> {
    match raw {
        None | Some("-1") => Ok(ReadFrom::Start),
        Some("now") => Ok(ReadFrom::Tail),
        Some(value) => Ok(ReadFrom::Offset(parse_offset(value)?)),
    }
}

/// Parse one offset in wire text.
fn parse_offset(
    value: &str,
) -> Result<foundation_core::durable::StreamOffset, ErrorTrace<ConnectError>> {
    wire::parse_offset(value)
        .ok_or_else(|| status_error(StoreError::BadRequest(BadRequestKind::Offset)))
}

/// Build a page limit from the two request fields.
fn parse_limit(max_bytes: Option<u32>, max_chunks: Option<u32>) -> ReadLimit {
    ReadLimit {
        max_bytes: max_bytes.map(|value| usize::try_from(value).unwrap_or(usize::MAX)),
        max_chunks: max_chunks.map(|value| usize::try_from(value).unwrap_or(usize::MAX)),
    }
}

/// A one-shot wake when a stream appends or closes.
struct AppendWake {
    fired: Arc<AtomicBool>,
    waker: Arc<Mutex<Option<Waker>>>,
}

impl Future for AppendWake {
    type Output = ();

    fn poll(self: Pin<&mut Self>, cx: &mut TaskContext<'_>) -> Poll<()> {
        if self.fired.load(Ordering::SeqCst) {
            return Poll::Ready(());
        }
        *self
            .waker
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner) = Some(cx.waker().clone());
        if self.fired.load(Ordering::SeqCst) {
            Poll::Ready(())
        } else {
            Poll::Pending
        }
    }
}

/// Register a wake on the stream notifier before the next read.
///
/// A store without a notifier fires at once. The stream then ends after its
/// first page, because the store gives no wake.
fn append_wake(service: &DurableService<DurableStream>, name: &StreamName) -> AppendWake {
    let fired = Arc::new(AtomicBool::new(false));
    let waker: Arc<Mutex<Option<Waker>>> = Arc::new(Mutex::new(None));
    match service.notifier(name) {
        Some(notifier) => {
            let fired = Arc::clone(&fired);
            let slot = Arc::clone(&waker);
            notifier.register(Box::new(move || {
                fired.store(true, Ordering::SeqCst);
                if let Some(waker) = slot
                    .lock()
                    .unwrap_or_else(std::sync::PoisonError::into_inner)
                    .take()
                {
                    waker.wake();
                }
            }));
        }
        None => fired.store(true, Ordering::SeqCst),
    }
    AppendWake { fired, waker }
}

/// A live read: one page at open, then one page per append or close.
struct SubscribeStream {
    service: Arc<DurableService<DurableStream>>,
    name: StreamName,
    from: ReadFrom,
    limit: ReadLimit,
    ctx: RpcCtx,
    read: Option<BoxFuture<Result<ReadPage, StoreError>>>,
    wake: Option<AppendWake>,
    cancel: Option<BoxFuture<()>>,
    live: bool,
    first: bool,
    finished: bool,
}

impl SubscribeStream {
    /// Start one bounded read at the current cursor.
    fn start_read(&self) -> BoxFuture<Result<ReadPage, StoreError>> {
        let service = Arc::clone(&self.service);
        let name = self.name.clone();
        let from = self.from.clone();
        let limit = self.limit;
        Box::pin(async move { service.read(&name, ReadRequest { from, limit }).await })
    }
}

impl Stream for SubscribeStream {
    type Item = ConnectResult<pb::ReadResponse>;

    fn poll_next(self: Pin<&mut Self>, cx: &mut TaskContext<'_>) -> Poll<Option<Self::Item>> {
        let this = self.get_mut();
        loop {
            if this.finished {
                return Poll::Ready(None);
            }
            if this.ctx.is_canceled() {
                this.finished = true;
                return Poll::Ready(Some(Err(canceled_error())));
            }
            if this.cancel.is_none() {
                let signal = this.ctx.cancel_signal().clone();
                this.cancel = Some(Box::pin(async move { signal.cancelled().await }));
            }
            if let Some(cancel) = this.cancel.as_mut() {
                if cancel.as_mut().poll(cx).is_ready() {
                    this.finished = true;
                    return Poll::Ready(Some(Err(canceled_error())));
                }
            }

            // Register the wake before the read, so an append between the read
            // and the wait is not missed.
            if this.wake.is_none() {
                this.wake = Some(append_wake(&this.service, &this.name));
            }
            if this.read.is_none() {
                this.read = Some(this.start_read());
            }
            let Some(read) = this.read.as_mut() else {
                this.finished = true;
                return Poll::Ready(Some(Err(status_error(StoreError::Unsupported))));
            };

            match read.as_mut().poll(cx) {
                Poll::Pending => return Poll::Pending,
                Poll::Ready(Err(error)) => {
                    this.finished = true;
                    return Poll::Ready(Some(Err(status_error(error))));
                }
                Poll::Ready(Ok(page)) => {
                    this.read = None;
                    this.from = ReadFrom::Offset(page.cursor.next.clone());
                    let closed = page.cursor.closed;
                    if closed {
                        this.finished = true;
                    }
                    // The first page opens the stream, even when it is empty,
                    // because the response head travels with the first message.
                    if !page.chunks.is_empty() || closed || this.first {
                        this.first = false;
                        this.wake = None;
                        return Poll::Ready(Some(Ok(page_message(page))));
                    }
                    if !this.live {
                        // No notifier means no wake. The stream ends here.
                        this.finished = true;
                        return Poll::Ready(Some(Ok(page_message(page))));
                    }
                    let Some(wake) = this.wake.as_mut() else {
                        this.finished = true;
                        return Poll::Ready(Some(Err(status_error(StoreError::Unsupported))));
                    };
                    match Future::poll(Pin::new(wake), cx) {
                        Poll::Ready(()) => {
                            this.wake = None;
                        }
                        Poll::Pending => return Poll::Pending,
                    }
                }
            }
        }
    }
}
