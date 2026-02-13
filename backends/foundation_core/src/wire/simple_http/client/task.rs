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

use crate::extensions::result_ext::BoxedError;
use crate::io::ioutils::SharedByteBufferStream;
use crate::netcap::RawStream;
use crate::synca::mpp::RecvIterator;
use crate::valtron::{
    BoxedSendExecutionAction, InlineSendAction, IntoBoxedSendExecutionAction, NoSpawner,
    TaskIterator, TaskStatus,
};
use crate::wire::simple_http::client::{
    DnsResolver, HttpClientAction, HttpClientConnection, PreparedRequest,
};
use crate::wire::simple_http::{
    ClientRequestErrors, Http11, HttpReaderError, HttpResponseIntro, HttpResponseReader,
    RenderHttp, SimpleHeader, SimpleHeaders, SimpleHttpBody,
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

    /// Creates a new HTTP request task with connection pool
    /// from the provided [`GetHttpRequestStreamInner`].
    pub fn from_data(data: GetHttpRequestStreamInner<R>) -> Self {
        Self(data)
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
                let Some(request) = self.0.request.take() else {
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

pub enum RequestIntro {
    Success {
        stream: HttpResponseReader<SimpleHttpBody, RawStream>,
        /// intro options  for a http response
        intro: HttpResponseIntro,
        /// headers retrieved from the stream.
        headers: SimpleHeaders,
    },

    Failed(HttpReaderError),
}

pub enum GetRequestIntroState {
    Init(Option<SharedByteBufferStream<RawStream>>),
    WithIntro(
        Option<(
            HttpResponseReader<SimpleHttpBody, RawStream>,
            HttpResponseIntro,
        )>,
    ),
}

pub struct GetRequestIntroTask(Option<GetRequestIntroState>);

impl GetRequestIntroTask {
    #[must_use]
    pub fn new(stream: SharedByteBufferStream<RawStream>) -> Self {
        Self(Some(GetRequestIntroState::Init(Some(stream))))
    }
}

impl TaskIterator for GetRequestIntroTask {
    type Pending = ();
    type Ready = RequestIntro;
    type Spawner = NoSpawner;

    fn next(&mut self) -> Option<TaskStatus<Self::Ready, Self::Pending, Self::Spawner>> {
        match self.0.take()? {
            GetRequestIntroState::Init(inner) => match inner {
                Some(stream) => {
                    let mut reader = HttpResponseReader::<SimpleHttpBody, RawStream>::new(
                        stream,
                        SimpleHttpBody,
                    );

                    let intro = match reader.next()? {
                        Ok(inner) => inner,
                        Err(err) => return Some(TaskStatus::Ready(RequestIntro::Failed(err))),
                    };

                    let crate::wire::simple_http::IncomingResponseParts::Intro(status, proto, text) =
                        intro
                    else {
                        return Some(TaskStatus::Ready(RequestIntro::Failed(
                            HttpReaderError::ReadFailed,
                        )));
                    };

                    let _ = self.0.replace(GetRequestIntroState::WithIntro(Some((
                        reader,
                        (status, proto, text),
                    ))));

                    return Some(TaskStatus::Pending(()));
                }
                None => None,
            },
            GetRequestIntroState::WithIntro(inner) => match inner {
                Some((mut reader, intro)) => {
                    let header_response = match reader.next()? {
                        Ok(inner) => inner,
                        Err(err) => return Some(TaskStatus::Ready(RequestIntro::Failed(err))),
                    };

                    let crate::wire::simple_http::IncomingResponseParts::Headers(headers) =
                        header_response
                    else {
                        return Some(TaskStatus::Ready(RequestIntro::Failed(
                            HttpReaderError::ReadFailed,
                        )));
                    };

                    return Some(TaskStatus::Ready(RequestIntro::Success {
                        stream: reader,
                        intro,
                        headers,
                    }));
                }
                None => None,
            },
        }
    }
}

pub enum HttpRequestTaskState<R>
where
    R: DnsResolver + Send + 'static,
{
    /// Init starts out with the request based on the provided [`GetHttpRequestStreamInner`]
    /// which then moves to [`Self::Connecting`] and then [`Self::Pull`]
    /// to get the actual request response.
    Init(Option<Box<GetHttpRequestStreamInner<R>>>),

    /// [`Connecting`] contains the read iterator to read the  response from the connection.
    Connecting(RecvIterator<TaskStatus<HttpStreamReady, GetHttpStreamState, HttpClientAction<R>>>),

    /// [`Reading`] reads the introduction information from (Status + Headers) from the connection.
    Reading(RecvIterator<TaskStatus<RequestIntro, (), NoSpawner>>),
    // TODO: implement state for redirection where response indicates we should reconnect to another server
}

pub struct HttpRequestTask<R>(Option<HttpRequestTaskState<R>>)
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
        Self(Some(HttpRequestTaskState::Init(Some(Box::new(
            GetHttpRequestStreamInner::new(request, resolver, max_redirects, pool),
        )))))
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, PartialOrd)]
pub enum HttpRequestPending {
    WaitingForStream,
    WaitingIntroAndHeaders,
}

impl<R> TaskIterator for HttpRequestTask<R>
where
    R: DnsResolver + Send + 'static,
{
    type Ready = RequestIntro;
    type Pending = HttpRequestPending;
    type Spawner = BoxedSendExecutionAction;

    fn next(&mut self) -> Option<TaskStatus<Self::Ready, Self::Pending, Self::Spawner>> {
        match self.0.take()? {
            HttpRequestTaskState::Init(mut inner) => match inner.take() {
                Some(req) => {
                    let (get_stream_action, get_stream_receiver) = InlineSendAction::boxed_mapper(
                        crate::valtron::InlineSendActionBehaviour::LiftWithParent,
                        Vec::new(),
                        GetHttpRequestStreamTask::from_data(*req),
                        std::time::Duration::from_millis(100),
                    );

                    self.0 = Some(HttpRequestTaskState::Connecting(get_stream_receiver));

                    Some(TaskStatus::Spawn(
                        get_stream_action.into_box_send_execution_action(),
                    ))
                }
                None => None,
            },
            HttpRequestTaskState::Connecting(mut recv_iter) => {
                let next_value = recv_iter.next();

                self.0 = Some(HttpRequestTaskState::Connecting(recv_iter));

                if next_value.is_none() {
                    self.0.take();
                    return Some(TaskStatus::Ready(RequestIntro::Failed(
                        HttpReaderError::ReadFailed,
                    )));
                }

                match next_value.unwrap() {
                    TaskStatus::Init => Some(TaskStatus::Init),
                    TaskStatus::Delayed(dur) => Some(TaskStatus::Delayed(dur)),
                    TaskStatus::Pending(_) => {
                        Some(TaskStatus::Pending(HttpRequestPending::WaitingForStream))
                    }
                    TaskStatus::Spawn(action) => {
                        Some(TaskStatus::Spawn(action.into_box_send_execution_action()))
                    }
                    TaskStatus::Ready(item) => match item {
                        HttpStreamReady::Done(stream) => {
                            let (get_intro_stream_action, get_intro_receiver) =
                                InlineSendAction::boxed_mapper(
                                    crate::valtron::InlineSendActionBehaviour::LiftWithParent,
                                    Vec::new(),
                                    GetRequestIntroTask::new(stream),
                                    std::time::Duration::from_millis(100),
                                );

                            self.0 = Some(HttpRequestTaskState::Reading(get_intro_receiver));

                            Some(TaskStatus::Spawn(
                                get_intro_stream_action.into_box_send_execution_action(),
                            ))
                        }
                        HttpStreamReady::Error(_) => {
                            self.0.take();

                            Some(TaskStatus::Ready(RequestIntro::Failed(
                                HttpReaderError::ReadFailed,
                            )))
                        }
                    },
                }
            }
            HttpRequestTaskState::Reading(mut intro_recv) => {
                let next_value = intro_recv.next();

                self.0 = Some(HttpRequestTaskState::Reading(intro_recv));

                if next_value.is_none() {
                    self.0.take();
                    return Some(TaskStatus::Ready(RequestIntro::Failed(
                        HttpReaderError::ReadFailed,
                    )));
                }

                match next_value.unwrap() {
                    TaskStatus::Init => Some(TaskStatus::Init),
                    TaskStatus::Delayed(dur) => Some(TaskStatus::Delayed(dur)),
                    TaskStatus::Pending(_) => {
                        Some(TaskStatus::Pending(HttpRequestPending::WaitingForStream))
                    }
                    TaskStatus::Spawn(action) => {
                        Some(TaskStatus::Spawn(action.into_box_send_execution_action()))
                    }
                    TaskStatus::Ready(item) => {
                        self.0.take();
                        Some(TaskStatus::Ready(item))
                    }
                }
            }
        }
    }
}
