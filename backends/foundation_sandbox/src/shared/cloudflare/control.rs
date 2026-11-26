//! The control plane of one Durable Object (DO).
//!
//! [`DoControlPlane`] restores the sandbox once for each DO object, touches the
//! idle deadline on each request, routes the request through a
//! `foundation_connectrpc` router, and stops an idle sandbox when the one alarm
//! fires.

use std::io;
use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};
use std::sync::Arc;
use std::time::Duration;

use bytes::Bytes;
use foundation_compact::Instant;
use foundation_connectrpc::{
    in_memory_body, CallHooks, ConnectError, ConnectRpcHandler, EnvelopeWriter, ResponseSink,
};
use foundation_context::Ctx;
use foundation_core::valtron::{Pipe, PipeReceiver, PipeSender};
use foundation_errstacks::ErrorTrace;
use foundation_netio::shared::http::{SendSafeBody, SimpleHeaders, SimpleOutgoingResponse, Status};
use futures::future::{select, Either};
use futures::StreamExt;

use crate::shared::cloudflare::container_service::ContainerControlService;
use crate::shared::cloudflare::driver::{CloudflareContainerDriver, CloudflareDoDriver};
use crate::shared::cloudflare::transport::PortConnector;
use crate::shared::cloudflare::{CloudflareConfig, CloudflareHost, DoSlaveConnector, DoStorage};
use crate::shared::cloudflare::{CloudflareContainerControl, DoRecordStore};
use crate::shared::container::ContainerControl;
use crate::shared::control::record::created_unix_ms;
use crate::shared::control::stop::spawn_detached;
use crate::shared::control::wait_until;
use crate::shared::control::{
    ControlService, DropPolicy, ManagerConfig, SandboxIdPolicy, SandboxManager, SandboxProvider,
};
use crate::shared::error::SandboxResult;
use crate::shared::ids::SandboxId;
use crate::shared::limits::SandboxLimits;
use crate::shared::rpc::{register_container_service, register_sandbox_service};
use crate::shared::{BoxByteStream, BoxFuture};

/// The storage key of the DO's sandbox identifier.
pub const SANDBOX_ID_KEY: &str = "sandbox_id";
/// The storage key of the idle deadline as Unix milliseconds.
pub const IDLE_DEADLINE_KEY: &str = "idle_deadline_ms";

/// One request to the Durable Object.
pub type HttpRequestParts = foundation_netio::shared::http::SimpleIncomingRequest;

/// One response from the Durable Object.
///
/// The body is a stream, so the response head leaves before the body ends. A
/// unary answer is one chunk. This type replaces the collected
/// `SimpleOutgoingResponse` that the router's `dispatch` gives.
pub struct HttpResponseParts {
    /// The HTTP status.
    pub status: u16,
    /// The response headers.
    pub headers: Vec<(String, String)>,
    /// The response body, one chunk at a time.
    pub body: BoxByteStream,
}

impl HttpResponseParts {
    /// A response with one body chunk.
    #[must_use]
    pub fn whole(status: u16, headers: Vec<(String, String)>, body: Vec<u8>) -> Self {
        Self {
            status,
            headers,
            body: Box::pin(futures::stream::once(async move { Ok(body) })),
        }
    }

    /// A response with no body.
    #[must_use]
    pub fn empty(status: u16) -> Self {
        Self::whole(status, Vec::new(), Vec::new())
    }
}

/// One event of a streamed response.
enum ResponseEvent {
    /// A complete response, unary or a stream that produced no frame.
    Whole(SimpleOutgoingResponse),
    /// The head of a streamed response.
    Head(SimpleOutgoingResponse),
    /// One response frame.
    Chunk(Bytes),
    /// The end of a streamed response.
    End,
}

/// The response sink that turns router writes into stream events.
struct StreamSink {
    sender: PipeSender<ResponseEvent>,
}

impl StreamSink {
    fn closed() -> io::Error {
        io::Error::new(io::ErrorKind::BrokenPipe, "the response sink is closed")
    }

    fn send(&self, event: ResponseEvent) -> BoxFuture<'_, io::Result<()>> {
        let sender = self.sender.clone();
        Box::pin(async move { sender.send(event).await.map_err(|_error| Self::closed()) })
    }
}

impl ResponseSink for StreamSink {
    fn whole(&mut self, response: SimpleOutgoingResponse) -> BoxFuture<'_, io::Result<()>> {
        self.send(ResponseEvent::Whole(response))
    }

    fn open(&mut self, mut head: SimpleOutgoingResponse) -> BoxFuture<'_, io::Result<()>> {
        head.body = None;
        self.send(ResponseEvent::Head(head))
    }

    fn chunk(&mut self, bytes: Bytes) -> BoxFuture<'_, io::Result<()>> {
        self.send(ResponseEvent::Chunk(bytes))
    }

    fn finish(&mut self) -> BoxFuture<'_, io::Result<()>> {
        self.send(ResponseEvent::End)
    }
}

/// Runs one request through the router and streams its response.
///
/// The head of the response returns before its stream ends. When
/// `stream_max_duration` is `Some`, a streaming response ends at that limit
/// with a Connect end-stream error.
pub(crate) async fn dispatch_streaming(
    ctx: Ctx,
    handler: &ConnectRpcHandler,
    mut request: HttpRequestParts,
    queue_len: usize,
    stream_max_duration: Option<Duration>,
) -> HttpResponseParts {
    let deadline = stream_max_duration.map(|limit| ctx.clock().now() + limit);
    let deadline_ctx = ctx.clone();
    let body = in_memory_body(request.body.take());
    let (sender, receiver) = Pipe::<ResponseEvent>::with_depth(queue_len);
    let sink: Box<dyn ResponseSink> = Box::new(StreamSink { sender });
    let dispatch = handler.dispatch_to(ctx, request, body, sink, CallHooks::default());
    let mut state = DispatchState::Running(dispatch);
    match next_event(&mut state, &receiver).await {
        Some(ResponseEvent::Whole(response)) => {
            let (status, headers, body) = split_whole(response);
            HttpResponseParts::whole(status, headers, body)
        }
        Some(ResponseEvent::Head(head)) => {
            let body = body_stream(state, receiver);
            let body = match deadline {
                Some(deadline) => with_stream_deadline(body, deadline, deadline_ctx),
                None => body,
            };
            HttpResponseParts {
                status: status_u16(&head.status),
                headers: flatten_headers(head.headers),
                body,
            }
        }
        Some(ResponseEvent::Chunk(_) | ResponseEvent::End) | None => HttpResponseParts::empty(500),
    }
}

/// Ends one response body at the stream deadline.
///
/// The body yields every frame until the deadline. At the deadline it yields
/// one Connect end-stream error frame with the code `deadline_exceeded` and
/// then ends. Dropping the body before the deadline ends the stream at once.
///
/// A host with no timer in its context gets no limit: the stream stays open
/// and the log holds one warning. Such a host cannot measure a deadline.
fn with_stream_deadline(body: BoxByteStream, deadline: Instant, ctx: Ctx) -> BoxByteStream {
    let stream = futures::stream::unfold(
        (body, ctx, Some(deadline), false),
        move |(mut body, ctx, deadline, ended)| async move {
            if ended {
                return None;
            }
            let Some(deadline) = deadline else {
                // No timer: pass every frame on, with no limit.
                let item = body.next().await?;
                return Some((item, (body, ctx, None, false)));
            };
            let seen = select(
                body.next(),
                Box::pin(wait_for_deadline(deadline, ctx.clone())),
            )
            .await;
            match seen {
                Either::Left((Some(item), timer)) => {
                    // Drop the timer future before the state moves on.
                    drop(timer);
                    Some((item, (body, ctx, Some(deadline), false)))
                }
                Either::Left((None, timer)) => {
                    drop(timer);
                    None
                }
                Either::Right((waited, body_next)) => {
                    // Drop the waiting body read before the body moves on.
                    drop(body_next);
                    if !waited {
                        // The helper logged the missing timer. Keep the stream
                        // open with no limit.
                        let item = body.next().await?;
                        return Some((item, (body, ctx, None, false)));
                    }
                    // Drop the live stream at once, so the slave sees the end
                    // and kills the program. Then yield the error frame.
                    let done: BoxByteStream = Box::pin(futures::stream::empty());
                    Some((Ok(deadline_end_frame()), (done, ctx, None, true)))
                }
            }
        },
    );
    Box::pin(crate::shared::cloudflare::SendStream::new(stream))
}

/// Waits until `deadline` with its own copy of the context.
///
/// The stream guard keeps its `Ctx` in the stream state, so the wait future
/// owns a copy and holds no borrow of that state. The result is `false` when
/// the context has no timer, so the caller does not claim a deadline.
async fn wait_for_deadline(deadline: Instant, ctx: Ctx) -> bool {
    match wait_until(deadline, &ctx).await {
        Ok(()) => true,
        Err(error) => {
            tracing::warn!(error = %error, "the stream deadline has no timer");
            false
        }
    }
}

/// The Connect end-stream frame of a stream that passed its deadline.
fn deadline_end_frame() -> Vec<u8> {
    let error: ErrorTrace<ConnectError> =
        ConnectError::deadline_exceeded("the stream passed its stream_max_duration limit").into();
    match EnvelopeWriter::new(None, 0, 0).write_end_stream(Some(&error), &SimpleHeaders::new()) {
        Ok(frame) => frame,
        Err(error) => {
            tracing::warn!(error = %error, "the deadline error frame was not written");
            Vec::new()
        }
    }
}

/// The dispatch future of one response, or its end.
enum DispatchState {
    /// The router is still writing.
    Running(BoxFuture<'static, io::Result<()>>),
    /// The router finished. The remaining buffered events stay readable.
    Done,
}

/// The next response event, driving the router while the pipe is empty.
async fn next_event(
    state: &mut DispatchState,
    receiver: &PipeReceiver<ResponseEvent>,
) -> Option<ResponseEvent> {
    loop {
        match state {
            DispatchState::Done => return receiver.receive().await,
            DispatchState::Running(_) => {
                let DispatchState::Running(dispatch) =
                    std::mem::replace(state, DispatchState::Done)
                else {
                    return None;
                };
                let waiting = receiver.clone();
                match futures::future::select(dispatch, Box::pin(waiting.receive())).await {
                    futures::future::Either::Left((_result, _waiting)) => {
                        *state = DispatchState::Done;
                    }
                    futures::future::Either::Right((event, dispatch)) => {
                        *state = DispatchState::Running(dispatch);
                        return event;
                    }
                }
            }
        }
    }
}

/// The body stream of a streamed response.
fn body_stream(state: DispatchState, receiver: PipeReceiver<ResponseEvent>) -> BoxByteStream {
    let inner = futures::stream::unfold((state, receiver), |(mut state, receiver)| async move {
        loop {
            match next_event(&mut state, &receiver).await {
                Some(ResponseEvent::Chunk(bytes)) => {
                    return Some((Ok(bytes.to_vec()), (state, receiver)));
                }
                Some(ResponseEvent::Whole(response)) => {
                    let (_, _, body) = split_whole(response);
                    if !body.is_empty() {
                        return Some((Ok(body), (state, receiver)));
                    }
                }
                Some(ResponseEvent::End) | None => return None,
                Some(ResponseEvent::Head(_)) => {}
            }
        }
    });
    Box::pin(crate::shared::cloudflare::SendStream::new(inner))
}

/// The same body stream, with one index removal when the stream fails.
///
/// The removal runs once for the first error, and the error still reaches the
/// caller. A failure of the removal is logged and does not change the stream.
pub fn remove_index_on_stream_error(
    body: BoxByteStream,
    index: Arc<dyn crate::shared::cloudflare::SandboxIndex>,
    id: SandboxId,
) -> BoxByteStream {
    let stream = futures::stream::unfold((body, false), move |(mut body, removed)| {
        let index = Arc::clone(&index);
        let id = id.clone();
        async move {
            let item = body.next().await?;
            let failed = item.is_err();
            if failed && !removed {
                if let Err(error) = index.remove(&id).await {
                    tracing::warn!(error = %error, %id, "the stream error left the index entry");
                }
            }
            Some((item, (body, removed || failed)))
        }
    });
    Box::pin(stream)
}

/// The status, headers and body bytes of a whole response.
fn split_whole(response: SimpleOutgoingResponse) -> (u16, Vec<(String, String)>, Vec<u8>) {
    let status = status_u16(&response.status);
    let headers = flatten_headers(response.headers);
    let body = match response.body {
        Some(SendSafeBody::Bytes(bytes)) => bytes,
        Some(SendSafeBody::Text(text)) => text.into_bytes(),
        _ => Vec::new(),
    };
    (status, headers, body)
}

/// One status as a number.
fn status_u16(status: &Status) -> u16 {
    u16::try_from(status.clone().into_usize()).unwrap_or(0)
}

/// The headers of one response as a flat list.
fn flatten_headers(headers: SimpleHeaders) -> Vec<(String, String)> {
    let mut flat = Vec::new();
    for (name, values) in headers {
        for value in values {
            flat.push((name.to_string(), value));
        }
    }
    flat
}

/// Why the one alarm stopped a sandbox.
///
/// The control plane keeps the last reason so a test can read it. A restart of
/// the object forgets it.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum StopReason {
    /// The idle deadline passed.
    Idle,
    /// The lifetime deadline passed. It wins when both deadlines passed.
    Expired,
}

/// The shared state of one DO control plane.
struct Inner {
    ctx: Ctx,
    host: CloudflareHost,
    manager: Arc<SandboxManager>,
    handler: ConnectRpcHandler,
    id: SandboxId,
    restored: AtomicBool,
    last_stop_reason: std::sync::Mutex<Option<StopReason>>,
}

/// The control plane of one Durable Object.
pub struct DoControlPlane {
    inner: Arc<Inner>,
}

impl DoControlPlane {
    /// Builds the control plane of one DO.
    ///
    /// `id` is `Some` on a request and `None` on an alarm. The identifier is
    /// stored under [`SANDBOX_ID_KEY`] the first time it is known.
    ///
    /// # Errors
    /// The error of the DO storage.
    pub async fn open(
        ctx: Ctx,
        host: CloudflareHost,
        id: Option<SandboxId>,
        limits: SandboxLimits,
    ) -> SandboxResult<Option<Self>> {
        let id = match id {
            Some(id) => {
                host.storage
                    .put(SANDBOX_ID_KEY, id.to_string().into_bytes())
                    .await?;
                Some(id)
            }
            None => stored_sandbox_id(&*host.storage).await?,
        };
        let Some(id) = id else {
            if let Err(error) = host.alarm.clear().await {
                tracing::warn!(error = %error, "the idle alarm was not cleared");
            }
            return Ok(None);
        };
        let mut limits = limits;
        limits.boot_timeout = host.config.boot_timeout;

        let connector = Arc::new(PortConnector {
            container: Arc::clone(&host.container),
            config: Arc::clone(&host.config),
            do_slave: host
                .service
                .as_ref()
                .map(|service| Arc::new(DoSlaveConnector::new(Arc::clone(service)))),
        });

        let drivers: Vec<Arc<dyn crate::shared::driver::SandboxDriver>> = vec![
            Arc::new(CloudflareContainerDriver::new(
                host.clone(),
                Arc::clone(&connector) as Arc<dyn crate::shared::host::SlaveConnector>,
            )),
            Arc::new(CloudflareDoDriver::new(&host.config)),
        ];
        let store = Arc::new(DoRecordStore::new(
            Arc::clone(&host.storage),
            Arc::clone(&host.index),
            Arc::clone(&host.alarm),
        ));
        let manager_config = ManagerConfig::new()
            .with_limits(limits.clone())
            .with_max_sandboxes(1)
            .with_id_policy(SandboxIdPolicy::Fixed(id.clone()))
            .with_on_drop(DropPolicy::Detach);
        let manager = Arc::new(SandboxManager::new(
            ctx.clone(),
            drivers,
            Arc::clone(&connector) as Arc<dyn crate::shared::host::SlaveConnector>,
            store,
            manager_config,
        ));

        let provider: Arc<dyn SandboxProvider> = manager.clone();
        let sandbox_service = Arc::new(ControlService::new(provider, limits.clone()));
        let control: Arc<dyn ContainerControl> = Arc::new(
            CloudflareContainerControl::new(
                host.clone(),
                Arc::clone(&connector) as Arc<dyn crate::shared::host::SlaveConnector>,
            )
            .with_limits(limits.clone()),
        );
        let slave_transport = crate::shared::host::SlaveConnector::connect(
            connector.as_ref(),
            &ctx,
            &crate::shared::driver::SlaveEndpoint::CloudflarePort {
                port: host.config.slave_port,
            },
            &limits,
        )?;
        let slave_client =
            crate::shared::control::SlaveClient::new(ctx.clone(), slave_transport, limits.clone())?;
        let calls: Arc<dyn crate::shared::handles::ExecFileCalls> = Arc::new(slave_client);
        let container_service = Arc::new(ContainerControlService::new(
            ctx.clone(),
            control,
            limits.clone(),
            calls,
        ));
        let mut router = foundation_connectrpc::Router::new();
        register_sandbox_service(&mut router, sandbox_service);
        register_container_service(&mut router, container_service);
        let handler = router.into_handler();

        Ok(Some(Self {
            inner: Arc::new(Inner {
                ctx,
                host,
                manager,
                handler,
                id,
                restored: AtomicBool::new(false),
                last_stop_reason: std::sync::Mutex::new(None),
            }),
        }))
    }

    /// The sandbox identifier of this DO.
    #[must_use]
    pub fn id(&self) -> &SandboxId {
        &self.inner.id
    }

    /// This DO's sandbox manager.
    #[must_use]
    pub fn sandboxes(&self) -> Arc<dyn SandboxProvider> {
        self.inner.manager.clone()
    }

    /// The configuration of this DO.
    #[must_use]
    pub fn config(&self) -> &CloudflareConfig {
        &self.inner.host.config
    }

    /// Loads the record and finds the substrate again. It runs one time for each
    /// object.
    async fn restore(&self) {
        if self.inner.restored.swap(true, Ordering::SeqCst) {
            return;
        }
        if let Err(error) = self.inner.manager.restore().await {
            tracing::warn!(error = %error, id = %self.inner.id, "restore failed");
        }
    }

    /// Moves the idle deadline to `now + idle_timeout` and sets the alarm.
    async fn touch(&self) {
        touch_inner(&self.inner).await;
    }

    /// Handles one HTTP request to this DO.
    ///
    /// The head of the response returns before its stream ends. A streaming
    /// call keeps the sender alive on the pool, so a long `Exec` does not hold
    /// the whole answer in memory.
    pub async fn handle(&self, request: HttpRequestParts) -> HttpResponseParts {
        self.restore().await;
        self.touch().await;
        let parts = dispatch_streaming(
            self.inner.ctx.clone(),
            &self.inner.handler,
            request,
            self.inner.host.config.stream_queue_len,
            Some(self.inner.host.config.stream_max_duration),
        )
        .await;
        // The create writes its record during the call, so its lifetime deadline
        // is known only after the response started. Arm the one alarm again.
        self.rearm().await;
        // A stream error after the head means that the sandbox is gone, so the
        // entry of this Durable Object leaves the index (invariant I6).
        let body = remove_index_on_stream_error(
            parts.body,
            Arc::clone(&self.inner.host.index),
            self.inner.id.clone(),
        );
        HttpResponseParts {
            status: parts.status,
            headers: parts.headers,
            body,
        }
    }

    /// The one DO alarm.
    ///
    /// The alarm stops the sandbox when either deadline passed. It removes the
    /// record and the idle alarm with the stop. Otherwise it moves the alarm to
    /// the earlier future deadline.
    pub async fn on_alarm(&self) {
        self.restore().await;
        let now = created_unix_ms(&self.inner.ctx);
        let sandboxes = self.inner.manager.list().await.unwrap_or_default();
        let Some(info) = sandboxes.first() else {
            if let Err(error) = self.inner.host.alarm.clear().await {
                tracing::warn!(error = %error, "the idle alarm was not cleared");
            }
            return;
        };
        let idle = read_deadline(&*self.inner.host.storage).await;
        let expires = info.expires_at_unix_ms;
        let reason = if expires.is_some_and(|expires| now >= expires) {
            Some(StopReason::Expired)
        } else if idle.is_some_and(|idle| now >= idle) {
            Some(StopReason::Idle)
        } else {
            None
        };
        if let Some(reason) = reason {
            self.record_stop_reason(reason);
            tracing::debug!(?reason, id = %self.inner.id, "the alarm stops the sandbox");
            if let Err(error) = self.inner.manager.stop(self.inner.id.clone()).await {
                tracing::warn!(error = %error, id = %self.inner.id, "the alarm stop failed");
            }
            return;
        }
        let next_idle = if let Some(idle) = idle {
            idle
        } else {
            let next = now.saturating_add(millis(self.inner.host.config.idle_timeout));
            if let Err(error) = self
                .inner
                .host
                .storage
                .put(IDLE_DEADLINE_KEY, next.to_string().into_bytes())
                .await
            {
                tracing::warn!(error = %error, "the idle deadline was not stored");
            }
            next
        };
        arm(&self.inner, Some(next_idle), expires).await;
    }

    /// The reason of the last stop that the alarm made.
    ///
    /// The value is `None` before the first alarm stop. A restart of the object
    /// forgets it.
    #[must_use]
    pub fn last_stop_reason(&self) -> Option<StopReason> {
        *self
            .inner
            .last_stop_reason
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
    }

    /// Keeps the reason of the stop that the alarm just made.
    fn record_stop_reason(&self, reason: StopReason) {
        *self
            .inner
            .last_stop_reason
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner) = Some(reason);
    }

    /// Sets the one alarm to the earlier of the two stored deadlines.
    async fn rearm(&self) {
        let idle = read_deadline(&*self.inner.host.storage).await;
        let expires = expires_at(&self.inner).await;
        arm(&self.inner, idle, expires).await;
    }

    /// A guard that keeps the sandbox alive while a response streams.
    ///
    /// While the guard lives, a timer task touches the sandbox every
    /// `idle_timeout / 2`.
    #[must_use]
    pub fn stream_guard(&self) -> StreamGuard {
        let inner = Arc::clone(&self.inner);
        let alive = Arc::new(AtomicBool::new(true));
        let touches = Arc::new(AtomicUsize::new(0));
        let task_alive = Arc::clone(&alive);
        let task_touches = Arc::clone(&touches);
        let period = self.inner.host.config.idle_timeout / 2;
        spawn_detached(async move {
            loop {
                if !task_alive.load(Ordering::SeqCst) {
                    return;
                }
                touch_inner(&inner).await;
                task_touches.fetch_add(1, Ordering::SeqCst);
                let deadline = inner.ctx.clock().now() + period;
                if wait_until(deadline, &inner.ctx).await.is_err() {
                    return;
                }
            }
        });
        StreamGuard { alive, touches }
    }
}

/// A guard that stops the stream touch task when it is dropped.
pub struct StreamGuard {
    alive: Arc<AtomicBool>,
    touches: Arc<AtomicUsize>,
}

impl StreamGuard {
    /// How many touches the guard's task already made.
    #[must_use]
    pub fn touches(&self) -> usize {
        self.touches.load(Ordering::SeqCst)
    }
}

impl Drop for StreamGuard {
    fn drop(&mut self) {
        self.alive.store(false, Ordering::SeqCst);
    }
}

/// The stored sandbox identifier.
async fn stored_sandbox_id(storage: &dyn DoStorage) -> SandboxResult<Option<SandboxId>> {
    let Some(bytes) = storage.get(SANDBOX_ID_KEY).await? else {
        return Ok(None);
    };
    let Ok(text) = String::from_utf8(bytes) else {
        return Ok(None);
    };
    Ok(text.parse().ok())
}

/// The stored idle deadline.
async fn read_deadline(storage: &dyn DoStorage) -> Option<u64> {
    let value = match storage.get(IDLE_DEADLINE_KEY).await {
        Ok(value) => value?,
        Err(error) => {
            tracing::warn!(error = %error, "the idle deadline was not read");
            return None;
        }
    };
    String::from_utf8(value).ok()?.parse().ok()
}

/// The lifetime deadline of the one sandbox, when the record is known.
async fn expires_at(inner: &Inner) -> Option<u64> {
    inner
        .manager
        .list()
        .await
        .ok()
        .and_then(|sandboxes| sandboxes.first().and_then(|info| info.expires_at_unix_ms))
}

/// Sets the one alarm to the earlier deadline, or clears it.
async fn arm(inner: &Inner, idle: Option<u64>, expires: Option<u64>) {
    match earlier_deadline(idle, expires) {
        Some(at) => {
            if let Err(error) = inner.host.alarm.set(at).await {
                tracing::warn!(error = %error, "the alarm was not set");
            }
        }
        None => {
            if let Err(error) = inner.host.alarm.clear().await {
                tracing::warn!(error = %error, "the alarm was not cleared");
            }
        }
    }
}

/// The earlier of the idle deadline and the lifetime deadline.
///
/// The DO has one alarm, so a sandbox with both deadlines uses this value
/// (invariant I1).
fn earlier_deadline(idle: Option<u64>, expires: Option<u64>) -> Option<u64> {
    match (idle, expires) {
        (Some(idle), Some(expires)) => Some(idle.min(expires)),
        (Some(deadline), None) | (None, Some(deadline)) => Some(deadline),
        (None, None) => None,
    }
}

/// Moves the idle deadline and sets the alarm to the earlier deadline.
///
/// A touch never moves the lifetime deadline (invariant I4).
async fn touch_inner(inner: &Inner) {
    let now = created_unix_ms(&inner.ctx);
    let timeout = millis(inner.host.config.idle_timeout);
    let next = now.saturating_add(timeout);
    let current = read_deadline(&*inner.host.storage).await;
    let idle = current.map_or(next, |current| current.max(next));
    if let Err(error) = inner
        .host
        .storage
        .put(IDLE_DEADLINE_KEY, idle.to_string().into_bytes())
        .await
    {
        tracing::warn!(error = %error, "the idle deadline was not stored");
    }
    arm(inner, Some(idle), expires_at(inner).await).await;
}

/// A duration in whole milliseconds, saturating at `u64`.
fn millis(duration: std::time::Duration) -> u64 {
    u64::try_from(duration.as_millis()).unwrap_or(u64::MAX)
}
