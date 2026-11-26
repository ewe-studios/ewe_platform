//! The remote procedure call (RPC) transport of a Cloudflare container port
//! and of a Durable Object (DO) service binding.
//!
//! A container port and a DO stub are both a [`NetClient`]: one `fetch` call
//! takes a request and gives a response with a streamed body. This module turns
//! either one into the [`DynNetClient`] that `foundation_connectrpc`'s
//! `H1Transport` expects. It writes no transport of its own and it buffers no
//! response body.
//!
//! [`PortSource`] is the one small seam: the container and the service binding
//! both answer it with a [`PortResponse`].

use std::sync::Arc;

use bytes::Bytes;
use foundation_compact::SendWrapper;
use foundation_connectrpc::shared::transport::h1::H1Transport;
use foundation_connectrpc::shared::transport::{
    Transport, TransportCapabilities, TransportError, TransportStream,
};
use foundation_context::Ctx;
use foundation_core::valtron::{
    from_future, from_stream, FutureTask, Stream as ValtronStream, StreamTask, TaskIterator,
    TaskStatus,
};
use foundation_netio::shared::client::body_reader::{
    SendSafeBodyBytesItem, SendSafeBodyBytesIterator,
};
use foundation_netio::shared::client::http_client::{
    BoxedSseFutureStream, BoxedSseIterator, HttpClient,
};
use foundation_netio::shared::client::request::PreparedRequest;
use foundation_netio::shared::client::request_task::{
    HttpExchange, HttpExchangeClientTask, HttpExchangePending,
};
use foundation_netio::shared::http::{
    BodyReadiness, HttpClientError, RequestDescriptor, SendSafeBody, SimpleHeader, SimpleHeaders,
    SimpleResponse, Status,
};
use foundation_netio::websocket::shared::client::{
    MessageDelivery, WebSocketClient, WebSocketConnectConfig,
};
use foundation_netio::websocket::shared::connector::{WebSocketConnector, WsExchangeTask};
use foundation_netio::websocket::shared::error::WebSocketError;
use foundation_netio::DynNetClient;

use crate::shared::cloudflare::{
    CloudflareConfig, ContainerApi, DoSlaveConnector, PortError, PortRequest, PortResponse,
};
use crate::shared::error::{SandboxError, SandboxResult};
use crate::shared::host::SlaveConnector;
use crate::shared::ids::SandboxId;
use crate::shared::limits::SandboxLimits;
use crate::shared::BoxByteStream;
use crate::shared::BoxFuture;

/// One place that answers a port request: the container, or a service binding.
pub trait PortSource: Send + Sync + 'static {
    /// Sends one request and gives the response. The body streams.
    fn fetch(
        &self,
        ctx: &Ctx,
        request: PortRequest,
    ) -> BoxFuture<'_, Result<PortResponse, PortError>>;
}

/// The container as a port source.
struct ContainerSource {
    container: Arc<dyn ContainerApi>,
    port: u16,
}

impl PortSource for ContainerSource {
    fn fetch(
        &self,
        _ctx: &Ctx,
        request: PortRequest,
    ) -> BoxFuture<'_, Result<PortResponse, PortError>> {
        self.container.fetch(self.port, request)
    }
}

/// The remote procedure call (RPC) transport of one container port.
///
/// It is a thin wrapper over `H1Transport`. It adds one rule: the port cannot
/// stream a request body, so a client-streaming call is refused before it sends
/// a fetch.
pub struct ContainerPortTransport {
    inner: H1Transport,
}

impl ContainerPortTransport {
    /// A transport over `source`.
    #[must_use]
    pub fn new(source: Arc<dyn PortSource>) -> Self {
        let client: DynNetClient = Arc::new(ContainerPortClient { source });
        Self {
            inner: H1Transport::new(client),
        }
    }

    /// A transport over one container port.
    #[must_use]
    pub fn over_container(container: Arc<dyn ContainerApi>, port: u16) -> Self {
        Self::new(Arc::new(ContainerSource { container, port }))
    }

    /// Whether the port can take a streamed request body. It cannot: every call
    /// is one fetch with one small request message.
    #[must_use]
    pub fn accepts_request_stream(&self) -> bool {
        Transport::capabilities(self).request_streaming
    }
}

impl Transport for ContainerPortTransport {
    fn capabilities(&self) -> TransportCapabilities {
        let mut capabilities = self.inner.capabilities();
        // Every call is one fetch with one small request message. No call
        // streams its request.
        capabilities.request_streaming = false;
        capabilities
    }

    fn open(&self, request: RequestDescriptor) -> Result<TransportStream, TransportError> {
        self.inner.open(request)
    }

    fn uses_fixed_request_body(&self) -> bool {
        self.inner.uses_fixed_request_body()
    }

    fn open_with_body(
        &self,
        request: RequestDescriptor,
        body: Bytes,
    ) -> Result<TransportStream, TransportError> {
        self.inner.open_with_body(request, body)
    }
}

/// The `NetClient` of one port source.
///
/// Only `open_exchange` is used: `H1Transport` calls it for every exchange. The
/// other client methods are not part of this contract and answer a typed error.
struct ContainerPortClient {
    source: Arc<dyn PortSource>,
}

impl ContainerPortClient {
    fn unsupported(method: &str) -> HttpClientError {
        HttpClientError::Reason(format!(
            "the container port client does not support {method}; use open_exchange"
        ))
    }
}

#[async_trait::async_trait]
impl HttpClient for ContainerPortClient {
    async fn send_async(
        &self,
        _req: PreparedRequest,
    ) -> Result<SimpleResponse<SendSafeBody>, HttpClientError> {
        Err(Self::unsupported("send_async"))
    }

    async fn send_sse_async(
        &self,
        _req: PreparedRequest,
    ) -> Result<BoxedSseFutureStream, HttpClientError> {
        Err(Self::unsupported("send_sse_async"))
    }

    fn send(&self, _req: PreparedRequest) -> Result<SimpleResponse<SendSafeBody>, HttpClientError> {
        Err(Self::unsupported("send"))
    }

    fn send_sse(&self, _req: PreparedRequest) -> Result<BoxedSseIterator, HttpClientError> {
        Err(Self::unsupported("send_sse"))
    }

    fn open_exchange(&self, req: PreparedRequest) -> HttpExchangeClientTask {
        Box::new(PortExchangeTask::new(Arc::clone(&self.source), req))
    }
}

impl WebSocketConnector for ContainerPortClient {
    fn open_websocket(
        &self,
        _url: &str,
        _config: WebSocketConnectConfig,
    ) -> Result<WebSocketClient, WebSocketError> {
        Err(WebSocketError::ProtocolError(
            "a container port has no WebSocket".to_string(),
        ))
    }

    fn open_websocket_task(
        &self,
        _url: &str,
        _config: WebSocketConnectConfig,
    ) -> Result<(WsExchangeTask, MessageDelivery), WebSocketError> {
        Err(WebSocketError::ProtocolError(
            "a container port has no WebSocket".to_string(),
        ))
    }
}

/// The state of one port exchange.
enum ExchangeState {
    /// Reading the small request body before the fetch.
    Request {
        body: SendSafeBodyBytesIterator,
        buffer: Vec<u8>,
        readiness: Option<BodyReadiness>,
    },
    /// Waiting for the port to answer.
    AwaitFetch {
        task: FutureTask<BoxFuture<'static, Result<PortResponse, PortError>>>,
    },
    /// The response head is out; the body streams.
    Body { task: StreamTask<BoxByteStream> },
    /// The exchange failed. The error leaves once.
    Failed(Option<Arc<dyn std::error::Error + Send + Sync + 'static>>),
    /// The exchange is done.
    Done,
}

/// One exchange over a port source.
struct PortExchangeTask {
    source: Arc<dyn PortSource>,
    method: String,
    path: String,
    headers: Vec<(String, String)>,
    state: ExchangeState,
}

impl PortExchangeTask {
    fn new(source: Arc<dyn PortSource>, request: PreparedRequest) -> Self {
        let method = request.method.to_string();
        let path = match request.url.query() {
            Some(query) => format!("{}?{}", request.url.path(), query),
            None => request.url.path().to_string(),
        };
        let headers = flatten_headers(&request.headers);
        let readiness = request.body_readiness.clone();
        Self {
            source,
            method,
            path,
            headers,
            state: ExchangeState::Request {
                body: SendSafeBodyBytesIterator::new(request.body),
                buffer: Vec::new(),
                readiness,
            },
        }
    }

    fn start_fetch(
        &mut self,
        ctx: &Ctx,
        body: Vec<u8>,
    ) -> TaskStatus<HttpExchange, HttpExchangePending> {
        let request = PortRequest {
            method: std::mem::take(&mut self.method),
            path: std::mem::take(&mut self.path),
            headers: std::mem::take(&mut self.headers),
            body,
        };
        let source = Arc::clone(&self.source);
        let fetch_ctx = ctx.clone();
        let future: BoxFuture<'static, Result<PortResponse, PortError>> =
            Box::pin(SendWrapper::new(async move {
                source.fetch(&fetch_ctx, request).await
            }));
        self.state = ExchangeState::AwaitFetch {
            task: from_future(future),
        };
        TaskStatus::Pending(HttpExchangePending::Waiting)
    }
}

impl TaskIterator for PortExchangeTask {
    type Ready = HttpExchange;
    type Pending = HttpExchangePending;

    #[allow(
        clippy::too_many_lines,
        reason = "one step of a four-state exchange reads best in one place"
    )]
    fn next_status(&mut self, ctx: Ctx) -> Option<TaskStatus<Self::Ready, Self::Pending>> {
        loop {
            // Take the state out, so no borrow of `self.state` is held while the
            // arm changes it.
            let state = std::mem::replace(&mut self.state, ExchangeState::Done);
            match state {
                ExchangeState::Request {
                    mut body,
                    mut buffer,
                    readiness,
                } => match body.next() {
                    Some(ValtronStream::Next(SendSafeBodyBytesItem::Chunk(bytes))) => {
                        buffer.extend_from_slice(&bytes);
                        self.state = ExchangeState::Request {
                            body,
                            buffer,
                            readiness,
                        };
                    }
                    Some(ValtronStream::Next(SendSafeBodyBytesItem::StreamError(error))) => {
                        self.state = ExchangeState::Failed(Some(Arc::new(std::io::Error::other(
                            error.to_string(),
                        ))));
                        return Some(TaskStatus::Pending(HttpExchangePending::Waiting));
                    }
                    Some(ValtronStream::Wait | ValtronStream::Pending(())) => {
                        #[cfg(not(target_family = "wasm"))]
                        let waiting = match &readiness {
                            Some(readiness) => TaskStatus::Depends(readiness.clone()),
                            None => TaskStatus::Pending(HttpExchangePending::Waiting),
                        };
                        #[cfg(target_family = "wasm")]
                        let waiting = {
                            let _ = &readiness;
                            TaskStatus::Pending(HttpExchangePending::Waiting)
                        };
                        self.state = ExchangeState::Request {
                            body,
                            buffer,
                            readiness,
                        };
                        return Some(waiting);
                    }
                    Some(
                        ValtronStream::Ignore
                        | ValtronStream::Init
                        | ValtronStream::Delayed(_)
                        | ValtronStream::Spread(_),
                    ) => {
                        self.state = ExchangeState::Request {
                            body,
                            buffer,
                            readiness,
                        };
                    }
                    None => {
                        let status = self.start_fetch(&ctx, buffer);
                        return Some(status);
                    }
                },
                ExchangeState::AwaitFetch { mut task } => match task.next_status(ctx.clone()) {
                    Some(TaskStatus::Ready(Ok(response))) => {
                        self.state = ExchangeState::Body {
                            task: from_stream(response.body),
                        };
                        return Some(TaskStatus::Ready(HttpExchange::Head {
                            status: Status::from(response.status.to_string()),
                            headers: to_simple_headers(response.headers),
                        }));
                    }
                    Some(TaskStatus::Ready(Err(error))) => {
                        self.state = ExchangeState::Failed(Some(Arc::new(PortFailure(error))));
                        return Some(TaskStatus::Pending(HttpExchangePending::Waiting));
                    }
                    Some(TaskStatus::Depends(signal)) => {
                        self.state = ExchangeState::AwaitFetch { task };
                        return Some(TaskStatus::Depends(signal));
                    }
                    Some(TaskStatus::Pending(()) | TaskStatus::Delayed(_)) => {
                        self.state = ExchangeState::AwaitFetch { task };
                        return Some(TaskStatus::Pending(HttpExchangePending::Waiting));
                    }
                    Some(
                        TaskStatus::Ignore
                        | TaskStatus::Init
                        | TaskStatus::Wait
                        | TaskStatus::Spawn(_)
                        | TaskStatus::Spread(_),
                    ) => {
                        self.state = ExchangeState::AwaitFetch { task };
                    }
                    None => {
                        self.state = ExchangeState::Failed(Some(Arc::new(std::io::Error::other(
                            "the port fetch ended without a result",
                        ))));
                        return Some(TaskStatus::Pending(HttpExchangePending::Waiting));
                    }
                },
                ExchangeState::Body { mut task } => match task.next_status(ctx.clone()) {
                    Some(TaskStatus::Ready(Some(Ok(bytes)))) => {
                        self.state = ExchangeState::Body { task };
                        return Some(TaskStatus::Ready(HttpExchange::BodyChunk(Bytes::from(
                            bytes,
                        ))));
                    }
                    Some(TaskStatus::Ready(Some(Err(error)))) => {
                        self.state = ExchangeState::Failed(Some(Arc::new(PortFailure(
                            PortError::Other(error.to_string()),
                        ))));
                        return Some(TaskStatus::Pending(HttpExchangePending::Waiting));
                    }
                    Some(TaskStatus::Ready(None)) | None => {
                        self.state = ExchangeState::Done;
                        return None;
                    }
                    Some(TaskStatus::Depends(signal)) => {
                        self.state = ExchangeState::Body { task };
                        return Some(TaskStatus::Depends(signal));
                    }
                    Some(TaskStatus::Pending(()) | TaskStatus::Delayed(_)) => {
                        self.state = ExchangeState::Body { task };
                        return Some(TaskStatus::Pending(HttpExchangePending::Waiting));
                    }
                    Some(
                        TaskStatus::Ignore
                        | TaskStatus::Init
                        | TaskStatus::Wait
                        | TaskStatus::Spawn(_)
                        | TaskStatus::Spread(_),
                    ) => {
                        self.state = ExchangeState::Body { task };
                    }
                },
                ExchangeState::Failed(mut error) => {
                    if let Some(error) = error.take() {
                        return Some(TaskStatus::Ready(HttpExchange::Failed(error)));
                    }
                    self.state = ExchangeState::Done;
                    return None;
                }
                ExchangeState::Done => return None,
            }
        }
    }
}

/// `PortError` as a standard error, so the exchange carries it.
#[derive(Debug, derive_more::Display, derive_more::Error)]
#[display("{_0}")]
struct PortFailure(PortError);

/// The headers of one port response as HTTP headers.
fn to_simple_headers(headers: Vec<(String, String)>) -> SimpleHeaders {
    let mut map = SimpleHeaders::new();
    for (name, value) in headers {
        map.entry(SimpleHeader::from(name)).or_default().push(value);
    }
    map
}

/// The headers of one HTTP request as a flat list.
fn flatten_headers(headers: &SimpleHeaders) -> Vec<(String, String)> {
    let mut flat = Vec::new();
    for (name, values) in headers {
        for value in values {
            flat.push((name.to_string(), value.clone()));
        }
    }
    flat
}

/// The connector for a DO: it dispatches on the endpoint.
pub struct PortConnector {
    /// The container of this DO.
    pub container: Arc<dyn ContainerApi>,
    /// The configuration of this DO.
    pub config: Arc<CloudflareConfig>,
    /// The connector of the model slave, when the host has a service binding.
    pub do_slave: Option<Arc<DoSlaveConnector>>,
}

impl SlaveConnector for PortConnector {
    fn connect(
        &self,
        ctx: &Ctx,
        endpoint: &crate::shared::driver::SlaveEndpoint,
        limits: &SandboxLimits,
    ) -> SandboxResult<Arc<dyn Transport>> {
        match endpoint {
            crate::shared::driver::SlaveEndpoint::CloudflarePort { port } => Ok(Arc::new(
                ContainerPortTransport::over_container(Arc::clone(&self.container), *port),
            )),
            crate::shared::driver::SlaveEndpoint::CloudflareDo { .. } => {
                let Some(connector) = &self.do_slave else {
                    return Err(SandboxError::InvalidSpec {
                        field: "endpoint".to_string(),
                        reason: "this host has no service binding for a Durable Object slave"
                            .to_string(),
                    }
                    .into());
                };
                connector.connect(ctx, endpoint, limits)
            }
            _ => Err(SandboxError::InvalidSpec {
                field: "endpoint".to_string(),
                reason: format!("the Cloudflare connector does not reach {endpoint:?}"),
            }
            .into()),
        }
    }
}

/// A port source over a Durable Object service binding.
pub struct BindingSource {
    binding: String,
    name: SandboxId,
    service: Arc<dyn crate::shared::cloudflare::ServiceBinding>,
}

impl BindingSource {
    /// A source over the named service binding and DO.
    #[must_use]
    pub fn new(
        binding: String,
        name: SandboxId,
        service: Arc<dyn crate::shared::cloudflare::ServiceBinding>,
    ) -> Self {
        Self {
            binding,
            name,
            service,
        }
    }
}

impl PortSource for BindingSource {
    fn fetch(
        &self,
        _ctx: &Ctx,
        request: PortRequest,
    ) -> BoxFuture<'_, Result<PortResponse, PortError>> {
        self.service.fetch(&self.binding, &self.name, request)
    }
}
