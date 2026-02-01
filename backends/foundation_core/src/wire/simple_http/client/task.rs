//! HTTP request task implementation using `TaskIterator` pattern.
//!
//! WHY: Provides a non-blocking, state-machine-based HTTP request executor that
//! integrates with the valtron executor system. Enables async-like request handling
//! without async/await.
//!
//! WHAT: Implements `HttpRequestTask` which processes HTTP requests through a series
//! of states (connecting, TLS handshake, sending request, receiving response).
//! Uses `TaskIterator` trait to yield `TaskStatus` variants.
//!
//! HOW: State machine pattern where each `next()` call advances through states.
//! Spawns child tasks (redirects, TLS upgrades) via `TaskStatus::Spawn`.
//! Uses `HttpClientAction` as the Spawner type for child task spawning.

use crate::synca::mpp::Receiver;
use crate::valtron::{TaskIterator, TaskStatus};
use crate::wire::simple_http::client::{
    DnsResolver, HttpClientAction, PreparedRequest, ResponseIntro,
};

/// HTTP request processing states.
///
/// WHY: HTTP request processing involves multiple sequential steps that should
/// not block. Each state represents a distinct phase of the request lifecycle.
///
/// WHAT: Enum representing all possible states during HTTP request processing.
///
/// HOW: State transitions occur in `HttpRequestTask::next()`. Each state
/// determines the next action or state transition.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum HttpRequestState {
    /// Initial state - preparing to connect
    Init,
    /// Establishing TCP connection
    Connecting,
    /// Performing TLS handshake (HTTPS only)
    TlsHandshake,
    /// Sending HTTP request
    SendingRequest,
    /// Receiving response status line
    ReceivingIntro,
    /// Receiving response headers
    ReceivingHeaders,
    /// Receiving response body
    ReceivingBody,
    /// Waiting for redirect decision
    AwaitingRedirect,
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
/// Spawns child tasks (redirects, TLS) via `TaskStatus::Spawn` with actions.
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
    /// Number of remaining redirects allowed
    remaining_redirects: u8,
    /// Receiver for redirect responses (if spawned)
    redirect_receiver: Option<Receiver<Result<ResponseIntro, String>>>,
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
    /// * `max_redirects` - Maximum number of redirects to follow (typically 5-10)
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
            redirect_receiver: None,
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
                // TODO: Implement initialization logic
                // Transition to Connecting state
                self.state = HttpRequestState::Connecting;
                Some(TaskStatus::Pending(HttpRequestState::Init))
            }
            HttpRequestState::Connecting => {
                // TODO: Implement DNS resolution and TCP connection
                // For now, return Pending
                Some(TaskStatus::Pending(HttpRequestState::Connecting))
            }
            HttpRequestState::TlsHandshake => {
                // TODO: Implement TLS handshake spawning
                Some(TaskStatus::Pending(HttpRequestState::TlsHandshake))
            }
            HttpRequestState::SendingRequest => {
                // TODO: Implement request sending
                Some(TaskStatus::Pending(HttpRequestState::SendingRequest))
            }
            HttpRequestState::ReceivingIntro => {
                // TODO: Implement response intro parsing
                Some(TaskStatus::Pending(HttpRequestState::ReceivingIntro))
            }
            HttpRequestState::ReceivingHeaders => {
                // TODO: Implement header parsing
                Some(TaskStatus::Pending(HttpRequestState::ReceivingHeaders))
            }
            HttpRequestState::ReceivingBody => {
                // TODO: Implement body handling
                Some(TaskStatus::Pending(HttpRequestState::ReceivingBody))
            }
            HttpRequestState::AwaitingRedirect => {
                // TODO: Implement redirect logic
                Some(TaskStatus::Pending(HttpRequestState::AwaitingRedirect))
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

    /// WHY: Verify HttpRequestState enum has all expected states
    /// WHAT: Tests that all state variants exist and are distinct
    #[test]
    fn test_http_request_state_variants() {
        let states = [
            HttpRequestState::Init,
            HttpRequestState::Connecting,
            HttpRequestState::TlsHandshake,
            HttpRequestState::SendingRequest,
            HttpRequestState::ReceivingIntro,
            HttpRequestState::ReceivingHeaders,
            HttpRequestState::ReceivingBody,
            HttpRequestState::AwaitingRedirect,
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
        assert!(task.redirect_receiver.is_none());
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
    /// WHAT: Tests that Connecting state returns Pending
    #[test]
    fn test_http_request_task_next_connecting() {
        let request = ClientRequestBuilder::get("http://example.com")
            .unwrap()
            .build();
        let resolver = MockDnsResolver::new();

        let mut task = HttpRequestTask::new(request, resolver, 5);

        // Advance to Connecting state
        let _ = task.next(); // Init -> Connecting

        // Connecting should return Pending
        let status = task.next();
        assert!(matches!(
            status,
            Some(TaskStatus::Pending(HttpRequestState::Connecting))
        ));
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
