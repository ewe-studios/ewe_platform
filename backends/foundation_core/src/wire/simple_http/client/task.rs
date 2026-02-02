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
    Http11, HttpResponseReader, IncomingResponseParts, RenderHttp, SimpleHttpBody,
};
use std::io::Write;
use std::time::Duration;

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
/// PHASE 1: Only Init, Connecting (which also sends), ReceivingIntro, Done, Error
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum HttpRequestState {
    /// Initial state - preparing to connect
    Init,
    /// Establishing TCP connection and sending request
    Connecting,
    /// Receiving response status line
    ReceivingIntro,
    /// Request completed successfully
    Done,
    /// Request failed with error
    Error,
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
    remaining_redirects: u8,
    /// Connection timeout
    timeout: Option<Duration>,
    /// HTTP response reader (owns the RawStream after connection established)
    response_reader: Option<HttpResponseReader<SimpleHttpBody, RawStream>>,
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
            response_reader: None,
        }
    }
}

impl<R> TaskIterator for HttpRequestTask<R>
where
    R: DnsResolver + Send + 'static,
{
    type Pending = HttpRequestState;
    type Ready = ResponseIntro;
    type Spawner = HttpClientAction<R>;

    fn next(&mut self) -> Option<TaskStatus<Self::Ready, Self::Pending, Self::Spawner>> {
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

                // Establish connection and get the RawStream
                match HttpClientConnection::connect(&request.url, &self.resolver, self.timeout) {
                    Ok(mut connection) => {
                        let host = request
                            .url
                            .host_str()
                            .unwrap_or_else(|| "unknown".to_string());
                        tracing::debug!("Connected to {}", host);

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
                        let stream = connection.stream_mut();
                        if let Err(e) = stream.write_all(request_string.as_bytes()) {
                            tracing::error!("Failed to write request: {}", e);
                            self.state = HttpRequestState::Error;
                            return None;
                        }

                        tracing::debug!("Request sent: {} bytes", request_string.len());

                        // Now transfer ownership of the stream to HttpResponseReader
                        let stream = connection.take_stream();
                        let shared_stream = SharedByteBufferStream::rwrite(stream);
                        let reader = HttpResponseReader::<SimpleHttpBody, RawStream>::new(
                            shared_stream,
                            SimpleHttpBody,
                        );

                        self.response_reader = Some(reader);
                        self.state = HttpRequestState::ReceivingIntro;
                        Some(TaskStatus::Pending(HttpRequestState::Connecting))
                    }
                    Err(e) => {
                        tracing::error!("Connection failed: {}", e);
                        self.state = HttpRequestState::Error;
                        None
                    }
                }
            }
            HttpRequestState::ReceivingIntro => {
                // Use HttpResponseReader to parse the response
                let reader = match self.response_reader.as_mut() {
                    Some(r) => r,
                    None => {
                        tracing::error!("No response reader in ReceivingIntro state");
                        self.state = HttpRequestState::Error;
                        return None;
                    }
                };

                // Poll the reader for IncomingResponseParts
                match reader.next() {
                    Some(Ok(IncomingResponseParts::Intro(status, proto, reason))) => {
                        tracing::debug!("Received response: {:?} {:?} {:?}", status, proto, reason);
                        let intro = ResponseIntro {
                            status,
                            proto,
                            reason,
                        };
                        self.state = HttpRequestState::Done;
                        Some(TaskStatus::Ready(intro))
                    }
                    Some(Ok(_other)) => {
                        // Not Intro yet, keep in ReceivingIntro state
                        Some(TaskStatus::Pending(HttpRequestState::ReceivingIntro))
                    }
                    Some(Err(e)) => {
                        tracing::error!("Failed to read response: {:?}", e);
                        self.state = HttpRequestState::Error;
                        None
                    }
                    None => {
                        tracing::error!("Connection closed before receiving response");
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
        assert!(task.response_reader.is_none());
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
                Ready = ResponseIntro,
                Spawner = HttpClientAction<MockDnsResolver>,
            >,
        {
        }

        _assert_types(&task);
    }
}
