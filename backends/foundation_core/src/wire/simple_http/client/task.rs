//! HTTP request task implementation using `TaskIterator` pattern.
//!
//! WHY: Provides a non-blocking, state-machine-based HTTP request executor that
//! integrates with the valtron executor system. Enables async-like request handling
//! without async/await.
//!
//! WHAT: Implements `HttpRequestTask` which processes HTTP requests through a series
//! of states (connecting, sending request, receiving response).
//! Uses `TaskIterator` trait to yield `TaskStatus` variants.
//!
//! HOW: State machine pattern where each `next()` call advances through states.
//! Phase 1 uses blocking connection for simplicity. Future phases will use
//! non-blocking connection spawning and TLS support.
//!
//! PHASE 1 SCOPE: HTTP-only (no HTTPS), blocking connection, basic GET requests.

use crate::io::ioutils::SharedByteBufferStream;
use crate::netcap::RawStream;
use crate::valtron::{NoSpawner, TaskIterator, TaskStatus};
use crate::wire::simple_http::client::{
    DnsResolver, HttpClientAction, HttpClientConnection, PreparedRequest,
};
use crate::wire::simple_http::{
    ClientRequestErrors, Http11, HttpResponseIntro, RenderHttp, SimpleHeader,
};
use std::io::Write;
use std::sync::Arc;
use std::time::Duration;

/// HTTP request stream processing states.
///
/// WHY: HTTP request processing involves multiple sequential steps that should
/// not block. Each state represents a distinct phase of the request lifecycle
/// towards getting the actual http stream for the request.
///
/// WHAT: Enum representing all possible states during HTTP request processing.
///
/// HOW: State transitions occur in `HttpRequestTask::next()`. Each state
/// determines the next action or state transition.
///
/// PHASE 1: Only Init, Connecting (which also sends), `ReceivingIntro`,
/// `WaitingForBodyRequest`, Done, Error
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum GetHttpStreamState {
    /// Initial state - preparing to connect
    Init,
    /// Establishing TCP connection and sending request
    Connecting,
    /// Done: no more work to do
    Done,
}

/// Values yielded by `GetHttpRequestStreamTask` as Ready status.
///
/// WHY: Task needs to yield different types of values at different stages:
/// first intro/headers, then stream ownership.
///
/// WHAT: Enum representing the two types of Ready values the task can yield.
///
/// HOW: `IntroAndHeaders` yields first, then task waits. When signaled,
/// `StreamOwnership` is yielded.
pub enum HttpStreamReady {
    Done(SharedByteBufferStream<RawStream>),
    Error(ClientRequestErrors),
}

pub struct GetHttpRequestStreamInner<R>
where
    R: DnsResolver + Send + 'static,
{
    /// DNS resolver for hostname resolution
    pub resolver: R,
    /// Host for pool return
    pub host: Option<String>,
    /// Port for pool return
    pub port: Option<u16>,
    /// Number of remaining redirects allowed (Phase 1: unused)
    #[allow(dead_code)]
    pub remaining_redirects: u8,
    /// Connection timeout
    pub timeout: Option<Duration>,
    /// Current state of the request
    pub state: GetHttpStreamState,
    /// The prepared request to send
    pub request: Option<PreparedRequest>,
    /// Connection pool for reuse (optional)
    pub pool: Option<Arc<super::pool::ConnectionPool>>,
}

impl<R> GetHttpRequestStreamInner<R>
where
    R: DnsResolver + Send + 'static,
{
    /// Creates a new HTTP request task with connection pool.
    ///
    /// # Arguments
    ///
    /// * `request` - The prepared HTTP request to execute
    /// * `resolver` - DNS resolver for hostname resolution
    /// * `max_redirects` - Maximum number of redirects to follow
    /// * `pool` - Optional connection pool for reuse
    ///
    /// # Returns
    ///
    /// A new `HttpRequestTask` in the `Init` state.
    pub fn new(
        request: PreparedRequest,
        resolver: R,
        max_redirects: u8,
        pool: Option<Arc<super::pool::ConnectionPool>>,
    ) -> Self {
        Self {
            resolver,
            pool,
            host: None,
            port: None,
            remaining_redirects: max_redirects,
            request: Some(request),
            state: GetHttpStreamState::Init,
            timeout: Some(Duration::from_secs(30)),
        }
    }
}

pub struct GetHttpRequestStreamTask<R>(GetHttpRequestStreamInner<R>)
where
    R: DnsResolver + Send + 'static;

impl<R> GetHttpRequestStreamTask<R>
where
    R: DnsResolver + Send + 'static,
{
    /// Creates a new HTTP request task with connection pool.
    ///
    /// # Arguments
    ///
    /// * `request` - The prepared HTTP request to execute
    /// * `resolver` - DNS resolver for hostname resolution
    /// * `max_redirects` - Maximum number of redirects to follow
    /// * `pool` - Optional connection pool for reuse
    ///
    /// # Returns
    ///
    /// A new `HttpRequestTask` in the `Init` state.
    pub fn new(
        request: PreparedRequest,
        resolver: R,
        max_redirects: u8,
        pool: Option<Arc<super::pool::ConnectionPool>>,
    ) -> Self {
        Self(GetHttpRequestStreamInner::new(
            request,
            resolver,
            max_redirects,
            pool,
        ))
    }
}

impl<R> TaskIterator for GetHttpRequestStreamTask<R>
where
    R: DnsResolver + Send + 'static,
{
    type Pending = GetHttpStreamState;
    type Spawner = HttpClientAction<R>;
    type Ready = HttpStreamReady;

    fn next(&mut self) -> Option<TaskStatus<Self::Ready, Self::Pending, Self::Spawner>> {
        match self.0.state {
            GetHttpStreamState::Init => {
                if self.0.request.is_none() {
                    // Transition to Done
                    self.0.state = GetHttpStreamState::Done;
                    return Some(TaskStatus::Ready(HttpStreamReady::Error(
                        ClientRequestErrors::InvalidState,
                    )));
                }

                // Transition to Connecting
                self.0.state = GetHttpStreamState::Connecting;

                Some(TaskStatus::Pending(GetHttpStreamState::Connecting))
            }
            GetHttpStreamState::Connecting => {
                // Phase 1: Blocking connection and send request immediately
                let request = if let Some(req) = self.0.request.take() { req } else {
                    tracing::error!("Request disappeared during connecting");
                    self.0.state = GetHttpStreamState::Done;
                    return None;
                };

                // Extract host and port for pool return
                self.0.host = Some(
                    request
                        .url
                        .host_str()
                        .unwrap_or_else(|| "unknown".to_string()),
                );

                self.0.port = Some(request.url.port().unwrap_or(80));

                // Try to get connection from pool first
                let stream = if let Some(pool) = &self.0.pool {
                    pool.checkout(self.0.host.as_ref().unwrap(), self.0.port.unwrap())
                } else {
                    match HttpClientConnection::connect(
                        &request.url,
                        &self.0.resolver,
                        self.0.timeout,
                    ) {
                        Ok(mut connection) => {
                            tracing::debug!("Connected to {}", self.0.host.as_ref().unwrap());

                            // Convert PreparedRequest to SimpleIncomingRequest for rendering
                            let simple_request = match request.into_simple_incoming_request() {
                                Ok(req) => req,
                                Err(e) => {
                                    tracing::error!("Failed to convert request: {}", e);
                                    self.0.state = GetHttpStreamState::Done;
                                    return Some(TaskStatus::Ready(HttpStreamReady::Error(
                                        ClientRequestErrors::InvalidState,
                                    )));
                                }
                            };

                            // Render HTTP request to string
                            let request_string =
                                match Http11::request(simple_request).http_render_string() {
                                    Ok(s) => s,
                                    Err(e) => {
                                        tracing::error!("Failed to render request: {:?}", e);
                                        self.0.state = GetHttpStreamState::Done;
                                        return Some(TaskStatus::Ready(HttpStreamReady::Error(
                                            ClientRequestErrors::InvalidState,
                                        )));
                                    }
                                };

                            // Write request to connection stream BEFORE transferring ownership
                            let raw_stream = connection.stream_mut();
                            if let Err(e) = raw_stream.write_all(request_string.as_bytes()) {
                                tracing::error!("Failed to write request: {}", e);
                                self.0.state = GetHttpStreamState::Done;
                                return Some(TaskStatus::Ready(HttpStreamReady::Error(
                                    ClientRequestErrors::InvalidState,
                                )));
                            }

                            if let Err(e) = raw_stream.flush() {
                                tracing::error!("Failed to write request: {}", e);
                                self.0.state = GetHttpStreamState::Done;
                                return Some(TaskStatus::Ready(HttpStreamReady::Error(
                                    ClientRequestErrors::InvalidState,
                                )));
                            }

                            tracing::debug!("Request sent: {} bytes", request_string.len());

                            // Transfer ownership of the stream
                            let raw_stream = connection.take_stream();

                            Some(SharedByteBufferStream::rwrite(raw_stream))
                        }
                        Err(e) => {
                            tracing::error!("Connection failed: {}", e);
                            self.0.state = GetHttpStreamState::Done;
                            return Some(TaskStatus::Ready(HttpStreamReady::Error(
                                ClientRequestErrors::InvalidState,
                            )));
                        }
                    }
                };

                // Store stream and transition to receiving intro
                self.0.state = GetHttpStreamState::Done;

                Some(TaskStatus::Ready(HttpStreamReady::Done(stream.unwrap())))
            }
            GetHttpStreamState::Done => None,
        }
    }
}

pub struct GetRequestInner {
    stream: Option<SharedByteBufferStream<RawStream>>,
    /// intro options  for a http response
    intro: Option<HttpResponseIntro>,
    /// headers retrieved from the stream.
    headers: Option<SimpleHeader>,
}

pub struct RequestIntro {
    stream: SharedByteBufferStream<RawStream>,
    /// intro options  for a http response
    intro: HttpResponseIntro,
    /// headers retrieved from the stream.
    headers: SimpleHeader,
}

impl From<GetRequestInner> for RequestIntro {
    fn from(value: GetRequestInner) -> Self {
        Self {
            stream: value.stream.expect("request stream is provided"),
            intro: value.intro.expect("request intro is provided"),
            headers: value.headers.expect("request headers is provided"),
        }
    }
}

pub enum GetRequestIntroState {
    Init(GetRequestInner),
    GetStream(GetRequestInner),
    GetIntro(GetRequestInner),
}

pub enum GetRequestIntroStates {
    Init,
    GetSStream,
    GetIntro,
}

pub struct GetRequestIntroTask(GetRequestIntroState);

impl GetRequestIntroTask {
    #[must_use] 
    pub fn new(stream: SharedByteBufferStream<RawStream>) -> Self {
        Self(GetRequestIntroState::Init(GetRequestInner {
            stream: Some(stream),
            headers: None,
            intro: None,
        }))
    }
}

impl TaskIterator for GetRequestIntroTask {
    type Pending = GetRequestIntroStates;
    type Ready = RequestIntro;
    type Spawner = NoSpawner;

    fn next(&mut self) -> Option<TaskStatus<Self::Ready, Self::Pending, Self::Spawner>> {
        todo!()
    }
}

pub enum HttpRequestTaskState<R>
where
    R: DnsResolver + Send + 'static,
{
    Init(GetHttpRequestStreamInner<R>),
    GetStream(SharedByteBufferStream<RawStream>),
    Done,
}

pub enum HttpRequestTaskPendingState {
    Init,
    Connecting,
}

pub struct HttpRequestTask<R>(HttpRequestTaskState<R>)
where
    R: DnsResolver + Send + 'static;

impl<R> HttpRequestTask<R>
where
    R: DnsResolver + Send + 'static,
{
    pub fn new(
        request: PreparedRequest,
        resolver: R,
        max_redirects: u8,
        pool: Option<Arc<super::pool::ConnectionPool>>,
    ) -> Self {
        Self(HttpRequestTaskState::Init(GetHttpRequestStreamInner::new(
            request,
            resolver,
            max_redirects,
            pool,
        )))
    }
}

impl<R> TaskIterator for HttpRequestTask<R>
where
    R: DnsResolver + Send + 'static,
{
    type Pending = GetRequestIntroStates;
    type Ready = RequestIntro;
    type Spawner = NoSpawner;

    fn next(&mut self) -> Option<TaskStatus<Self::Ready, Self::Pending, Self::Spawner>> {
        todo!()
    }
}
