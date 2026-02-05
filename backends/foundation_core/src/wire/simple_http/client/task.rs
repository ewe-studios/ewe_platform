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
use crate::valtron::{TaskIterator, TaskStatus};
use crate::wire::simple_http::client::{
    DnsResolver, HttpClientAction, HttpClientConnection, PreparedRequest, ResponseIntro,
};
use crate::wire::simple_http::{
    Http11, HttpResponseReader, IncomingResponseParts, RenderHttp, SimpleHeaders, SimpleHttpBody,
};
use std::io::Write;
use std::sync::atomic::{AtomicU8, Ordering};
use std::sync::Arc;
use std::time::Duration;

// State constants for atomic coordination
const STATE_NOT_STARTED: u8 = 0;
const STATE_INTRO_READY: u8 = 1;
const STATE_BODY_REQUESTED: u8 = 2;
#[allow(dead_code)]
const STATE_COMPLETED: u8 = 3;

/// Shared control for coordinating between task and ClientRequest.
///
/// WHY: ClientRequest needs to signal when body reading is desired, and task
/// needs to wait for that signal. Atomic coordination enables lock-free
/// communication.
///
/// WHAT: Arc-wrapped atomic state that both task and ClientRequest can access.
///
/// HOW: Clone-able handle with atomic operations for state changes.
#[derive(Clone, Debug)]
pub struct RequestControl {
    state: Arc<AtomicU8>,
}

impl RequestControl {
    /// Creates a new request control in NOT_STARTED state.
    pub fn new() -> Self {
        Self {
            state: Arc::new(AtomicU8::new(STATE_NOT_STARTED)),
        }
    }

    /// Signals that intro/headers are ready.
    pub fn set_intro_ready(&self) {
        self.state.store(STATE_INTRO_READY, Ordering::Release);
    }

    /// Signals that body reading is requested.
    pub fn set_body_requested(&self) {
        self.state.store(STATE_BODY_REQUESTED, Ordering::Release);
    }

    /// Gets current state.
    pub fn get_state(&self) -> u8 {
        self.state.load(Ordering::Acquire)
    }
}

impl Default for RequestControl {
    fn default() -> Self {
        Self::new()
    }
}

/// HTTP request processing states.
///
/// WHY: HTTP request processing involves multiple sequential steps that should
/// not block. Each state represents a distinct phase of the request lifecycle.
///
/// WHAT: Enum representing all possible states during HTTP request processing.
///
/// HOW: State transitions occur in `HttpRequestTask::next()`. Each state
/// determines the next action or state transition.
///
/// PHASE 1: Only Init, Connecting (which also sends), ReceivingIntro,
/// WaitingForBodyRequest, Done, Error
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum HttpRequestState {
    /// Initial state - preparing to connect
    Init,
    /// Establishing TCP connection and sending request
    Connecting,
    /// Receiving response status line and headers
    ReceivingIntro,
    /// Paused state - waiting for user to request body
    WaitingForBodyRequest,
    /// Request completed successfully
    Done,
    /// Request failed with error
    Error,
}

/// Values yielded by HttpRequestTask as Ready status.
///
/// WHY: Task needs to yield different types of values at different stages:
/// first intro/headers, then stream ownership.
///
/// WHAT: Enum representing the two types of Ready values the task can yield.
///
/// HOW: IntroAndHeaders yields first, then task waits. When signaled,
/// StreamOwnership is yielded.
pub enum HttpTaskReady {
    /// Response introduction and headers received.
    IntroAndHeaders {
        intro: ResponseIntro,
        headers: SimpleHeaders,
    },
    /// Stream ownership transferred for body reading.
    StreamOwnership(SharedByteBufferStream<RawStream>),
}

/// HTTP request task implementing `TaskIterator`.
///
/// WHY: Provides non-blocking HTTP request execution using iterator pattern.
/// Integrates with valtron executor for concurrent request handling.
///
/// WHAT: Stateful task that processes HTTP requests through multiple phases.
/// Yields `TaskStatus` variants to indicate progress, completion, or spawn needs.
///
/// HOW: Maintains internal state and advances through states on each `next()` call.
/// Phase 1 uses blocking connection for simplicity. HttpResponseReader owns the
/// RawStream, avoiding lifetime issues.
///
/// # Type Parameters
///
/// * `R` - DNS resolver type implementing `DnsResolver` trait
pub struct HttpRequestTask<R>
where
    R: DnsResolver + Send + 'static,
{
    /// Current state of the request
    state: HttpRequestState,
    /// DNS resolver for hostname resolution
    resolver: R,
    /// The prepared request to send
    request: Option<PreparedRequest>,
    /// Number of remaining redirects allowed (Phase 1: unused)
    #[allow(dead_code)]
    remaining_redirects: u8,
    /// Connection timeout
    timeout: Option<Duration>,
    /// Connection pool for reuse (optional)
    pool: Option<Arc<super::pool::ConnectionPool>>,
    /// Shared coordination with ClientRequest
    control: RequestControl,
    /// Stream holder for yielding ownership later
    stream_holder: Option<SharedByteBufferStream<RawStream>>,
    /// Host for pool return
    host: Option<String>,
    /// Port for pool return
    port: Option<u16>,
}

impl<R> HttpRequestTask<R>
where
    R: DnsResolver + Send + 'static,
{
    /// Creates a new HTTP request task.
    ///
    /// # Arguments
    ///
    /// * `request` - The prepared HTTP request to execute
    /// * `resolver` - DNS resolver for hostname resolution
    /// * `max_redirects` - Maximum number of redirects to follow (Phase 1: unused)
    ///
    /// # Returns
    ///
    /// A new `HttpRequestTask` in the `Init` state.
    pub fn new(request: PreparedRequest, resolver: R, max_redirects: u8) -> Self {
        Self {
            state: HttpRequestState::Init,
            resolver,
            request: Some(request),
            remaining_redirects: max_redirects,
            timeout: Some(Duration::from_secs(30)),
            pool: None,
            control: RequestControl::new(),
            stream_holder: None,
            host: None,
            port: None,
        }
    }

    /// Creates a new HTTP request task with connection pool and control.
    ///
    /// # Arguments
    ///
    /// * `request` - The prepared HTTP request to execute
    /// * `resolver` - DNS resolver for hostname resolution
    /// * `max_redirects` - Maximum number of redirects to follow
    /// * `pool` - Optional connection pool for reuse
    /// * `control` - Shared coordination with ClientRequest
    ///
    /// # Returns
    ///
    /// A new `HttpRequestTask` in the `Init` state.
    pub fn with_control(
        request: PreparedRequest,
        resolver: R,
        max_redirects: u8,
        pool: Option<Arc<super::pool::ConnectionPool>>,
        control: RequestControl,
    ) -> Self {
        Self {
            state: HttpRequestState::Init,
            resolver,
            request: Some(request),
            remaining_redirects: max_redirects,
            timeout: Some(Duration::from_secs(30)),
            pool,
            control,
            stream_holder: None,
            host: None,
            port: None,
        }
    }
}

impl<R> TaskIterator for HttpRequestTask<R>
where
    R: DnsResolver + Send + 'static,
{
    type Pending = HttpRequestState;
    type Ready = HttpTaskReady;
    type Spawner = HttpClientAction<R>;

    fn next(&mut self) -> Option<TaskStatus<Self::Ready, Self::Pending, Self::Spawner>> {
        println!(
            "Iterator through HttpRequestTask with state: {:?}",
            &self.state
        );
        match self.state {
            HttpRequestState::Init => {
                // Validate request exists and check for HTTPS (not supported in Phase 1)
                if let Some(request) = &self.request {
                    if request.url.scheme().is_https() {
                        tracing::error!("HTTPS not supported in Phase 1");
                        self.state = HttpRequestState::Error;
                        return None;
                    }
                    // Transition to Connecting
                    self.state = HttpRequestState::Connecting;
                    Some(TaskStatus::Pending(HttpRequestState::Init))
                } else {
                    tracing::error!("No request to process");
                    self.state = HttpRequestState::Error;
                    None
                }
            }
            HttpRequestState::Connecting => {
                // Phase 1: Blocking connection and send request immediately
                let request = match self.request.take() {
                    Some(req) => req,
                    None => {
                        tracing::error!("Request disappeared during connecting");
                        self.state = HttpRequestState::Error;
                        return None;
                    }
                };

                // Extract host and port for pool return
                self.host = Some(
                    request
                        .url
                        .host_str()
                        .unwrap_or_else(|| "unknown".to_string()),
                );
                self.port = Some(request.url.port().unwrap_or(80));

                // Try to get connection from pool first
                let mut stream = if let Some(pool) = &self.pool {
                    pool.checkout(self.host.as_ref().unwrap(), self.port.unwrap())
                } else {
                    None
                };

                // If no pooled connection, establish new connection
                if stream.is_none() {
                    match HttpClientConnection::connect(&request.url, &self.resolver, self.timeout)
                    {
                        Ok(mut connection) => {
                            tracing::debug!("Connected to {}", self.host.as_ref().unwrap());

                            // Convert PreparedRequest to SimpleIncomingRequest for rendering
                            let simple_request = match request.into_simple_incoming_request() {
                                Ok(req) => req,
                                Err(e) => {
                                    tracing::error!("Failed to convert request: {}", e);
                                    self.state = HttpRequestState::Error;
                                    return None;
                                }
                            };

                            // Render HTTP request to string
                            let request_string =
                                match Http11::request(simple_request).http_render_string() {
                                    Ok(s) => s,
                                    Err(e) => {
                                        tracing::error!("Failed to render request: {:?}", e);
                                        self.state = HttpRequestState::Error;
                                        return None;
                                    }
                                };

                            // Write request to connection stream BEFORE transferring ownership
                            let raw_stream = connection.stream_mut();
                            if let Err(e) = raw_stream.write_all(request_string.as_bytes()) {
                                tracing::error!("Failed to write request: {}", e);
                                self.state = HttpRequestState::Error;
                                return None;
                            }

                            if let Err(e) = raw_stream.flush() {
                                tracing::error!("Failed to write request: {}", e);
                                self.state = HttpRequestState::Error;
                                return None;
                            }

                            tracing::debug!("Request sent: {} bytes", request_string.len());

                            // Transfer ownership of the stream
                            let raw_stream = connection.take_stream();
                            stream = Some(SharedByteBufferStream::rwrite(raw_stream));
                        }
                        Err(e) => {
                            tracing::error!("Connection failed: {}", e);
                            self.state = HttpRequestState::Error;
                            return None;
                        }
                    }
                }

                // Store stream and transition to receiving intro
                self.stream_holder = stream;
                self.state = HttpRequestState::ReceivingIntro;
                Some(TaskStatus::Pending(HttpRequestState::Connecting))
            }
            HttpRequestState::ReceivingIntro => {
                // Use HttpResponseReader to parse intro and headers
                let shared_stream = match self.stream_holder.as_ref() {
                    Some(s) => s.clone(),
                    None => {
                        tracing::error!("No stream in ReceivingIntro state");
                        self.state = HttpRequestState::Error;
                        return None;
                    }
                };

                // Create temporary reader (not stored)
                let mut reader = HttpResponseReader::<SimpleHttpBody, RawStream>::new(
                    shared_stream,
                    SimpleHttpBody,
                );

                tracing::debug!("Read intro from stream reader");

                // Read intro
                let intro = match reader.next() {
                    Some(Ok(IncomingResponseParts::Intro(status, proto, reason))) => {
                        tracing::debug!("Received response: {:?} {:?} {:?}", status, proto, reason);
                        ResponseIntro {
                            status,
                            proto,
                            reason,
                        }
                    }
                    Some(Ok(other)) => {
                        tracing::debug!("Read other response: {:?}", &other);
                        // Not Intro yet, keep waiting
                        return Some(TaskStatus::Pending(HttpRequestState::ReceivingIntro));
                    }
                    Some(Err(e)) => {
                        tracing::error!("Failed to read intro: {:?}", e);
                        self.state = HttpRequestState::Error;
                        return None;
                    }
                    None => {
                        tracing::error!("Connection closed before receiving intro");
                        self.state = HttpRequestState::Error;
                        return None;
                    }
                };

                tracing::debug!("Read headers from stream reader");

                // Read headers
                let headers = match reader.next() {
                    Some(Ok(IncomingResponseParts::Headers(headers))) => {
                        tracing::debug!("Received headers: {} entries", headers.len());
                        headers
                    }
                    Some(Ok(_other)) => {
                        // Expected headers, got something else
                        tracing::warn!("Expected headers, got different part");
                        SimpleHeaders::default()
                    }
                    Some(Err(e)) => {
                        tracing::error!("Failed to read headers: {:?}", e);
                        self.state = HttpRequestState::Error;
                        return None;
                    }
                    None => {
                        // No headers - use empty
                        SimpleHeaders::default()
                    }
                };

                // Signal that intro/headers are ready
                self.control.set_intro_ready();
                self.state = HttpRequestState::WaitingForBodyRequest;

                // Yield intro and headers
                Some(TaskStatus::Ready(HttpTaskReady::IntroAndHeaders {
                    intro,
                    headers,
                }))
            }
            HttpRequestState::WaitingForBodyRequest => {
                // Check atomic flag to see if body is requested
                match self.control.get_state() {
                    STATE_INTRO_READY => {
                        // Still waiting - delay to yield control
                        Some(TaskStatus::Delayed(Duration::from_nanos(500)))
                    }
                    STATE_BODY_REQUESTED => {
                        // User wants body - yield stream ownership
                        let stream = match self.stream_holder.take() {
                            Some(s) => s,
                            None => {
                                tracing::error!("No stream to yield in WaitingForBodyRequest");
                                self.state = HttpRequestState::Error;
                                return None;
                            }
                        };

                        self.state = HttpRequestState::Done;
                        Some(TaskStatus::Ready(HttpTaskReady::StreamOwnership(stream)))
                    }
                    _ => {
                        // Invalid state
                        tracing::error!("Invalid control state in WaitingForBodyRequest");
                        self.state = HttpRequestState::Error;
                        None
                    }
                }
            }
            HttpRequestState::Done => {
                // Request completed - no more values
                None
            }
            HttpRequestState::Error => {
                // Error occurred - terminate iteration
                None
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::wire::simple_http::client::dns::MockDnsResolver;
    use crate::wire::simple_http::client::ClientRequestBuilder;

    // ========================================================================
    // HttpRequestState Tests
    // ========================================================================

    /// WHY: Verify HttpRequestState enum has all expected states (Phase 1)
    /// WHAT: Tests that all Phase 1 state variants exist and are distinct
    #[test]
    fn test_http_request_state_variants() {
        let states = [
            HttpRequestState::Init,
            HttpRequestState::Connecting,
            HttpRequestState::ReceivingIntro,
            HttpRequestState::WaitingForBodyRequest,
            HttpRequestState::Done,
            HttpRequestState::Error,
        ];

        // Verify each state is unique
        for (i, state1) in states.iter().enumerate() {
            for (j, state2) in states.iter().enumerate() {
                if i == j {
                    assert_eq!(state1, state2);
                } else {
                    assert_ne!(state1, state2);
                }
            }
        }
    }

    /// WHY: Verify HttpRequestState implements Debug for diagnostics
    /// WHAT: Tests that Debug trait produces non-empty output
    #[test]
    fn test_http_request_state_debug() {
        let state = HttpRequestState::Init;
        let debug_str = format!("{:?}", state);
        assert!(!debug_str.is_empty());
        assert!(debug_str.contains("Init"));
    }

    // ========================================================================
    // HttpRequestTask Tests
    // ========================================================================

    /// WHY: Verify HttpRequestTask can be constructed
    /// WHAT: Tests that new() creates task with expected initial state
    #[test]
    fn test_http_request_task_new() {
        let request = ClientRequestBuilder::get("http://example.com")
            .unwrap()
            .build();
        let resolver = MockDnsResolver::new();

        let task = HttpRequestTask::new(request, resolver, 5);

        assert_eq!(task.state, HttpRequestState::Init);
        assert!(task.request.is_some());
        assert_eq!(task.remaining_redirects, 5);
        assert!(task.stream_holder.is_none());
        assert!(task.timeout.is_some());
    }

    /// WHY: Verify HttpRequestTask implements TaskIterator
    /// WHAT: Tests that HttpRequestTask can be used as a TaskIterator
    #[test]
    fn test_http_request_task_is_task_iterator() {
        let request = ClientRequestBuilder::get("http://example.com")
            .unwrap()
            .build();
        let resolver = MockDnsResolver::new();

        let task = HttpRequestTask::new(request, resolver, 5);

        // Type check - ensure it implements TaskIterator
        fn _assert_is_task_iterator<T: TaskIterator>(_: &T) {}
        _assert_is_task_iterator(&task);
    }

    /// WHY: Verify HttpRequestTask::next() transitions from Init state
    /// WHAT: Tests that first call to next() returns Pending(Init) and transitions
    #[test]
    fn test_http_request_task_next_init() {
        let request = ClientRequestBuilder::get("http://example.com")
            .unwrap()
            .build();
        let resolver = MockDnsResolver::new();

        let mut task = HttpRequestTask::new(request, resolver, 5);

        // First next() should return Pending(Init)
        let status = task.next();
        assert!(matches!(
            status,
            Some(TaskStatus::Pending(HttpRequestState::Init))
        ));

        // State should have transitioned to Connecting
        assert_eq!(task.state, HttpRequestState::Connecting);
    }

    /// WHY: Verify HttpRequestTask::next() handles Connecting state
    /// WHAT: Tests that Connecting state attempts connection (Phase 1: blocking)
    /// NOTE: Phase 1 uses blocking connection, so this will fail without a real server
    #[test]
    fn test_http_request_task_next_connecting() {
        let request = ClientRequestBuilder::get("http://example.com")
            .unwrap()
            .build();
        let resolver = MockDnsResolver::new();

        let mut task = HttpRequestTask::new(request, resolver, 5);

        // Advance to Connecting state
        let _ = task.next(); // Init -> Connecting

        // Phase 1: Connecting attempts real connection (blocking)
        // MockDnsResolver returns empty addresses, so connection will fail
        // This results in None (Error state)
        let status = task.next();
        assert!(status.is_none());
        assert_eq!(task.state, HttpRequestState::Error);
    }

    /// WHY: Verify HttpRequestTask::next() handles Done state
    /// WHAT: Tests that Done state returns None (iteration complete)
    #[test]
    fn test_http_request_task_next_done() {
        let request = ClientRequestBuilder::get("http://example.com")
            .unwrap()
            .build();
        let resolver = MockDnsResolver::new();

        let mut task = HttpRequestTask::new(request, resolver, 5);

        // Manually set state to Done
        task.state = HttpRequestState::Done;

        // Done state should return None
        let status = task.next();
        assert!(status.is_none());
    }

    /// WHY: Verify HttpRequestTask::next() handles Error state
    /// WHAT: Tests that Error state returns None (iteration terminates)
    #[test]
    fn test_http_request_task_next_error() {
        let request = ClientRequestBuilder::get("http://example.com")
            .unwrap()
            .build();
        let resolver = MockDnsResolver::new();

        let mut task = HttpRequestTask::new(request, resolver, 5);

        // Manually set state to Error
        task.state = HttpRequestState::Error;

        // Error state should return None
        let status = task.next();
        assert!(status.is_none());
    }

    /// WHY: Verify HttpRequestTask associated types are correct
    /// WHAT: Tests that TaskIterator associated types match expectations
    #[test]
    fn test_http_request_task_associated_types() {
        let request = ClientRequestBuilder::get("http://example.com")
            .unwrap()
            .build();
        let resolver = MockDnsResolver::new();

        let task = HttpRequestTask::new(request, resolver, 5);

        // Type assertions (compile-time checks)
        fn _assert_types<T>(_: &T)
        where
            T: TaskIterator<
                Pending = HttpRequestState,
                Ready = HttpTaskReady,
                Spawner = HttpClientAction<MockDnsResolver>,
            >,
        {
        }

        _assert_types(&task);
    }
}
