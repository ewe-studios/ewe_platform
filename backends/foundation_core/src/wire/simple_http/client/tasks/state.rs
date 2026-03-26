use crate::netcap::RawStream;
use crate::wire::simple_http::client::{
    DnsResolver, HttpClientConnection, HttpConnectionPool, HttpRequestPending, PreparedRequest,
};
use crate::wire::simple_http::{
    HttpClientError, HttpResponseReader, IncomingResponseParts, SimpleHttpBody,
};
use std::sync::Arc;

// ============================================================================
// Progress Tracking
// ============================================================================

/// Progress state for HTTP fetch operations with source identification.
///
/// WHY: Tracks progress of individual API fetches during parallel execution.
/// Error tracking allows users to identify which sources failed after completion.
///
/// WHAT: Progress states with source identification for observability.
/// Error states preserve source and error information for post-execution reporting.
///
/// HOW: Used as the `Pending` type in `TaskIterator` combinators.
/// Errors are stored for post-execution inspection.
///
/// # Type Parameters
/// * `S` - Source type (default: &'static str)
/// * `E` - Error type (default: String)
///
/// # Examples
///
/// ```rust,ignore
/// let progress = FetchPending::Connecting { source: "api.example.com" };
/// println!("Progress: {}", progress); // "api.example.com: Connecting..."
///
/// let failed = FetchPending::Failed {
///     source: "api.example.com",
///     error: "Connection refused".to_string(),
/// };
/// println!("Error: {}", failed); // "api.example.com: FAILED - Connection refused"
/// ```
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum FetchPending<S = &'static str, E = String> {
    /// Establishing connection to source
    Connecting { source: S },
    /// Connection established, awaiting response headers
    AwaitingResponse { source: S },
    /// Fetch completed with error - source and error preserved for reporting
    Failed { source: S, error: E },
    /// Fetch completed successfully
    Completed { source: S },
}

impl<S, E> FetchPending<S, E> {
    /// Create a Connecting state
    pub fn connecting(source: S) -> Self {
        Self::Connecting { source }
    }

    /// Create an `AwaitingResponse` state
    pub fn awaiting_response(source: S) -> Self {
        Self::AwaitingResponse { source }
    }

    /// Create a Failed state
    pub fn failed(source: S, error: E) -> Self {
        Self::Failed { source, error }
    }

    /// Create a Completed state
    pub fn completed(source: S) -> Self {
        Self::Completed { source }
    }
}

impl<S: AsRef<str>, E: std::fmt::Display> FetchPending<S, E> {
    /// Convert from `HttpRequestPending` with source context
    pub fn from_http_request(p: HttpRequestPending, source: S) -> Self {
        match p {
            HttpRequestPending::WaitingForStream => Self::connecting(source),
            HttpRequestPending::WaitingIntroAndHeaders => Self::awaiting_response(source),
        }
    }
}

impl<S: AsRef<str>, E: std::fmt::Display> std::fmt::Display for FetchPending<S, E> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            FetchPending::Connecting { source } => {
                write!(f, "{}: Connecting...", source.as_ref())
            }
            FetchPending::AwaitingResponse { source } => {
                write!(f, "{}: Awaiting response...", source.as_ref())
            }
            FetchPending::Failed { source, error } => {
                write!(f, "{}: FAILED - {}", source.as_ref(), error)
            }
            FetchPending::Completed { source } => {
                write!(f, "{}: Completed", source.as_ref())
            }
        }
    }
}

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
pub enum HttpOperationState {
    /// Initial state - preparing to connect
    Init,
    /// Establishing TCP connection and sending request
    Connecting,
    /// Done: no more work to do
    Done,
}

pub struct SendRequest<R>
where
    R: DnsResolver + Send + 'static,
{
    /// Number of remaining redirects allowed (Phase 1: unused)
    #[allow(dead_code)]
    pub remaining_redirects: u8,
    /// Client configuration (for proxy support)
    pub config: crate::wire::simple_http::client::ClientConfig,
    /// The prepared request to send
    pub request: Option<PreparedRequest>,
    /// Connection pool for reuse (optional)
    pub pool: Arc<HttpConnectionPool<R>>,
}

impl<R> SendRequest<R>
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
    #[must_use]
    pub fn new(
        request: PreparedRequest,
        max_redirects: u8,
        pool: Arc<HttpConnectionPool<R>>,
        config: crate::wire::simple_http::client::ClientConfig,
    ) -> Self {
        Self {
            pool,
            config,
            request: Some(request),
            remaining_redirects: max_redirects,
        }
    }
}

/// Values yielded by `HttpStreamReady` as Ready status.
///
/// WHY: Task needs to yield different types of values at different stages:
/// first intro/headers, then stream ownership.
///
/// WHAT: Enum representing the two types of Ready values the task can yield.
///
/// HOW: `IntroAndHeaders` yields first, then task waits. When signaled,
/// `StreamOwnership` is yielded.
pub enum HttpStreamReady {
    /// Done returns an optional intro (if probe read an intro) and the owned stream
    Done(HttpClientConnection),
    Error(HttpClientError),
}

/// [`IncomingResponseMapper`] implements an iterator wrapper for the [`HttpResponseReader<SimpleHttpBody, RawStream>`]
/// returning the type that matches client response.
pub enum IncomingResponseMapper {
    Reader(HttpResponseReader<SimpleHttpBody, RawStream>),
    List(std::vec::IntoIter<Result<IncomingResponseParts, HttpClientError>>),
}

impl IncomingResponseMapper {
    #[must_use]
    pub fn from_reader(reader: HttpResponseReader<SimpleHttpBody, RawStream>) -> Self {
        Self::Reader(reader)
    }

    #[must_use]
    pub fn from_list(
        items: std::vec::IntoIter<Result<IncomingResponseParts, HttpClientError>>,
    ) -> Self {
        Self::List(items)
    }
}

impl Iterator for IncomingResponseMapper {
    type Item = Result<IncomingResponseParts, HttpClientError>;

    fn next(&mut self) -> Option<Self::Item> {
        match self {
            Self::List(inner) => match inner.next()? {
                Ok(inner) => Some(Ok(inner)),
                Err(err) => Some(Err(err)),
            },
            Self::Reader(inner) => match inner.next()? {
                Ok(inner) => Some(Ok(inner)),
                Err(err) => Some(Err(HttpClientError::ReaderError(err))),
            },
        }
    }
}

// ============================================================================
// Tests
// ============================================================================

#[cfg(test)]
mod tests {
    use super::FetchPending;
    use crate::wire::simple_http::client::HttpRequestPending;

    #[test]
    fn test_fetch_pending_display() {
        // Test Connecting state
        let connecting: FetchPending<&str> = FetchPending::connecting("api.example.com");
        assert_eq!(format!("{connecting}"), "api.example.com: Connecting...");

        // Test AwaitingResponse state
        let awaiting: FetchPending<&str> = FetchPending::awaiting_response("api.example.com");
        assert_eq!(
            format!("{awaiting}"),
            "api.example.com: Awaiting response..."
        );

        // Test Failed state
        let failed = FetchPending::failed("api.example.com", "Connection refused".to_string());
        assert_eq!(
            format!("{failed}"),
            "api.example.com: FAILED - Connection refused"
        );

        // Test Completed state
        let completed: FetchPending<&str> = FetchPending::completed("api.example.com");
        assert_eq!(format!("{completed}"), "api.example.com: Completed");
    }

    #[test]
    fn test_fetch_pending_constructors() {
        let connecting: FetchPending<&str> = FetchPending::connecting("test");
        assert!(matches!(
            connecting,
            FetchPending::Connecting { source: "test" }
        ));

        let awaiting: FetchPending<&str> = FetchPending::awaiting_response("test");
        assert!(matches!(
            awaiting,
            FetchPending::AwaitingResponse { source: "test" }
        ));

        let failed: FetchPending<&str, &str> = FetchPending::failed("test", "error");
        assert!(matches!(
            failed,
            FetchPending::Failed {
                source: "test",
                error: "error"
            }
        ));

        let completed: FetchPending<&str> = FetchPending::completed("test");
        assert!(matches!(
            completed,
            FetchPending::Completed { source: "test" }
        ));
    }

    #[test]
    fn test_fetch_pending_from_http_request() {
        let pending_stream: FetchPending<&str> =
            FetchPending::from_http_request(HttpRequestPending::WaitingForStream, "test");
        assert!(matches!(pending_stream, FetchPending::Connecting { .. }));

        let pending_intro: FetchPending<&str> =
            FetchPending::from_http_request(HttpRequestPending::WaitingIntroAndHeaders, "test");
        assert!(matches!(
            pending_intro,
            FetchPending::AwaitingResponse { .. }
        ));
    }

    #[test]
    fn test_fetch_pending_generic_source() {
        // Test with String source
        let string_source: FetchPending<String> = FetchPending::connecting("api".to_string());
        assert_eq!(format!("{string_source}"), "api: Connecting...");

        // Test with custom error type
        let custom_error: FetchPending<&str, &'static str> = FetchPending::failed("api", "err");
        assert_eq!(format!("{custom_error}"), "api: FAILED - err");
    }
}
