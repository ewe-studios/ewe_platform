//! The control plane of one Durable Object (DO).
//!
//! [`DoControlPlane`] restores the sandbox once for each DO object, touches the
//! idle deadline on each request, routes the request through a
//! `foundation_connectrpc` router, and stops an idle sandbox when the one alarm
//! fires.

use std::io;
use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};
use std::sync::Arc;

use bytes::Bytes;
use foundation_connectrpc::{in_memory_body, CallHooks, ConnectRpcHandler, ResponseSink};
use foundation_context::Ctx;
use foundation_core::valtron::{Pipe, PipeReceiver, PipeSender};
use foundation_netio::shared::http::{SendSafeBody, SimpleHeaders, SimpleOutgoingResponse, Status};

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
/// The head of the response returns before its stream ends.
pub(crate) async fn dispatch_streaming(
    ctx: Ctx,
    handler: &ConnectRpcHandler,
    mut request: HttpRequestParts,
    queue_len: usize,
) -> HttpResponseParts {
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
        Some(ResponseEvent::Head(head)) => HttpResponseParts {
            status: status_u16(&head.status),
            headers: flatten_headers(head.headers),
            body: body_stream(state, receiver),
        },
        Some(ResponseEvent::Chunk(_) | ResponseEvent::End) | None => HttpResponseParts::empty(500),
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

/// The shared state of one DO control plane.
struct Inner {
    ctx: Ctx,
    host: CloudflareHost,
    manager: Arc<SandboxManager>,
    handler: ConnectRpcHandler,
    id: SandboxId,
    restored: AtomicBool,
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
        dispatch_streaming(
            self.inner.ctx.clone(),
            &self.inner.handler,
            request,
            self.inner.host.config.stream_queue_len,
        )
        .await
    }

    /// The one DO alarm.
    pub async fn on_alarm(&self) {
        self.restore().await;
        let now = created_unix_ms(&self.inner.ctx);
        let deadline = read_deadline(&*self.inner.host.storage).await;
        let sandboxes = self.inner.manager.list().await.unwrap_or_default();
        if sandboxes.is_empty() {
            if let Err(error) = self.inner.host.alarm.clear().await {
                tracing::warn!(error = %error, "the idle alarm was not cleared");
            }
            return;
        }
        match deadline {
            Some(deadline) if now >= deadline => {
                if let Err(error) = self.inner.manager.stop(self.inner.id.clone()).await {
                    tracing::warn!(error = %error, id = %self.inner.id, "the idle stop failed");
                }
            }
            Some(deadline) => {
                if let Err(error) = self.inner.host.alarm.set(deadline).await {
                    tracing::warn!(error = %error, "the idle alarm was not set again");
                }
            }
            None => {
                if let Err(error) = self.inner.host.alarm.clear().await {
                    tracing::warn!(error = %error, "the idle alarm was not cleared");
                }
            }
        }
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

/// Moves the idle deadline and sets the alarm.
async fn touch_inner(inner: &Inner) {
    let now = created_unix_ms(&inner.ctx);
    let timeout = millis(inner.host.config.idle_timeout);
    let next = now.saturating_add(timeout);
    let current = read_deadline(&*inner.host.storage).await;
    let deadline = current.map_or(next, |current| current.max(next));
    if let Err(error) = inner
        .host
        .storage
        .put(IDLE_DEADLINE_KEY, deadline.to_string().into_bytes())
        .await
    {
        tracing::warn!(error = %error, "the idle deadline was not stored");
    }
    if let Err(error) = inner.host.alarm.set(deadline).await {
        tracing::warn!(error = %error, "the idle alarm was not set");
    }
}

/// A duration in whole milliseconds, saturating at `u64`.
fn millis(duration: std::time::Duration) -> u64 {
    u64::try_from(duration.as_millis()).unwrap_or(u64::MAX)
}
