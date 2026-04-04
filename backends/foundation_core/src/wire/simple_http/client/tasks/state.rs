use crate::netcap::RawStream;
use crate::wire::simple_http::client::{
    DnsResolver, HttpClientConnection, HttpConnectionPool, HttpRequestPending, PreparedRequest,
};
use crate::wire::simple_http::{
    HttpClientError, HttpResponseReader, IncomingResponseParts, SimpleHttpBody,
};
use std::sync::{Arc, Mutex};

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

// ============================================================================
// Fetch Result Aggregation
// ============================================================================

/// Outcome of a single fetch operation.
///
/// WHY: Provides unified success/error representation for result aggregation.
///
/// WHAT: Generic enum for fetch outcomes with typed success and error values.
///
/// HOW: Used with `FetchResult` to track per-source outcomes in parallel fetches.
///
/// # Type Parameters
/// * `T` - Success result type
/// * `E` - Error type
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum FetchOutcome<T, E> {
    /// Fetch succeeded with result
    Success(T),
    /// Fetch failed with error
    Error(E),
}

impl<T, E> FetchOutcome<T, E> {
    /// Create a Success variant
    pub fn success(value: T) -> Self {
        Self::Success(value)
    }

    /// Create an Error variant
    pub fn error(error: E) -> Self {
        Self::Error(error)
    }

    /// Get the success value if available
    pub fn ok(self) -> Option<T> {
        match self {
            Self::Success(v) => Some(v),
            Self::Error(_) => None,
        }
    }

    /// Get the error value if available
    pub fn err(self) -> Option<E> {
        match self {
            Self::Error(e) => Some(e),
            Self::Success(_) => None,
        }
    }

    /// Map the success value
    pub fn map<U, F>(self, f: F) -> FetchOutcome<U, E>
    where
        F: FnOnce(T) -> U,
    {
        match self {
            Self::Success(v) => FetchOutcome::Success(f(v)),
            Self::Error(e) => FetchOutcome::Error(e),
        }
    }
}

/// Result of a fetch operation with source identification.
///
/// WHY: Tracks which source succeeded or failed in parallel fetch operations.
/// Enables post-execution error reporting and result aggregation.
///
/// WHAT: Combines source identifier with `FetchOutcome` for complete tracking.
///
/// HOW: Use as the `Ready` type in TaskIterator combinators for observable fetches.
/// After execution, collect all results and report failures by source.
///
/// # Type Parameters
/// * `S` - Source type (default: &'static str)
/// * `T` - Success result type (default: Vec<ModelEntry>)
/// * `E` - Error type (default: String)
///
/// # Examples
///
/// ```rust,ignore
/// use foundation_core::wire::simple_http::client::tasks::{FetchResult, FetchOutcome};
///
/// let result = FetchResult {
///     source: "api.example.com",
///     outcome: FetchOutcome::Success(vec![/* models */]),
/// };
///
/// // Log with appropriate level based on outcome
/// result.log();
///
/// // Extract success value or error
/// if let Some(models) = result.ok() {
///     // Process successful result
/// }
/// if let Some(error) = result.err() {
///     // Handle error
/// }
/// ```
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FetchResult<S = &'static str, T = Vec<()>, E = String> {
    /// Source identifier
    pub source: S,
    /// Fetch outcome (success or error)
    pub outcome: FetchOutcome<T, E>,
}

impl<S, T, E> FetchResult<S, T, E> {
    /// Create a new FetchResult
    pub fn new(source: S, outcome: FetchOutcome<T, E>) -> Self {
        Self { source, outcome }
    }

    /// Create a success result
    pub fn success(source: S, value: T) -> Self {
        Self {
            source,
            outcome: FetchOutcome::Success(value),
        }
    }

    /// Create an error result
    pub fn error(source: S, error: E) -> Self {
        Self {
            source,
            outcome: FetchOutcome::Error(error),
        }
    }
}

impl<S: AsRef<str>, T, E: std::fmt::Display> FetchResult<S, T, E> {
    /// Log the result with appropriate level.
    ///
    /// WHY: Provides consistent logging for fetch outcomes.
    ///
    /// HOW: Logs success at info level, errors at error level.
    ///
    /// # Examples
    ///
    /// ```rust,ignore
    /// let result: FetchResult<&str, Vec<ModelEntry>, String> = /* ... */;
    /// result.log(); // Logs "api.example.com: Success" or "api.example.com: Error - ..."
    /// ```
    pub fn log(&self) {
        match &self.outcome {
            FetchOutcome::Success(_) => {
                tracing::info!("{}: Success", self.source.as_ref());
            }
            FetchOutcome::Error(err) => {
                tracing::error!("{}: Error - {}", self.source.as_ref(), err);
            }
        }
    }
}

impl<S, T, E> FetchResult<S, T, E> {
    /// Get the success value if available
    pub fn ok(self) -> Option<T> {
        self.outcome.ok()
    }

    /// Get the error value if available
    pub fn err(self) -> Option<E> {
        self.outcome.err()
    }

    /// Map the success value, preserving source and error
    pub fn map<U, F>(self, f: F) -> FetchResult<S, U, E>
    where
        F: FnOnce(T) -> U,
    {
        FetchResult {
            source: self.source,
            outcome: self.outcome.map(f),
        }
    }
}

impl<S: AsRef<str>, T, E: std::fmt::Display> std::fmt::Display for FetchResult<S, T, E> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match &self.outcome {
            FetchOutcome::Success(_) => write!(f, "{}: Success", self.source.as_ref()),
            FetchOutcome::Error(err) => write!(f, "{}: Error - {}", self.source.as_ref(), err),
        }
    }
}

/// Error collection utility for parallel fetch operations.
///
/// WHY: Parallel fetches need to collect errors from multiple sources
/// for post-execution reporting.
///
/// WHAT: Thread-safe error collector that aggregates (source, error) pairs.
///
/// HOW: Clone the collector before parallel execution, each task records
/// its errors. After execution, collect all errors for reporting.
///
/// # Examples
///
/// ```rust,ignore
/// use std::sync::{Arc, Mutex};
/// use foundation_core::wire::simple_http::client::tasks::ErrorCollector;
///
/// let errors = ErrorCollector::new();
/// let errors_clone = errors.clone();
///
/// // In fetch task
/// if fetch_failed {
///     errors_clone.record("api.example.com", "Connection refused");
/// }
///
/// // After execution
/// let collected = errors.collect();
/// for (source, error) in collected {
///     tracing::warn!("{}: {}", source, error);
/// }
/// ```
#[derive(Clone, Default)]
pub struct ErrorCollector<S = String, E = String> {
    inner: Arc<Mutex<Vec<(S, E)>>>,
}

impl<S, E> ErrorCollector<S, E> {
    /// Create a new error collector
    pub fn new() -> Self {
        Self {
            inner: Arc::new(Mutex::new(Vec::new())),
        }
    }
}

impl<S: Clone, E: Clone> ErrorCollector<S, E> {
    /// Record an error for a source
    pub fn record(&self, source: S, error: E) {
        if let Ok(mut guard) = self.inner.lock() {
            guard.push((source, error));
        }
    }

    /// Collect all recorded errors
    pub fn collect(&self) -> Vec<(S, E)> {
        self.inner.lock().map(|g| g.clone()).unwrap_or_default()
    }

    /// Check if any errors were recorded
    pub fn has_errors(&self) -> bool {
        self.inner.lock().map(|g| !g.is_empty()).unwrap_or(false)
    }

    /// Get the count of recorded errors
    pub fn count(&self) -> usize {
        self.inner.lock().map(|g| g.len()).unwrap_or(0)
    }
}

impl<S: Clone + std::fmt::Display, E: Clone + std::fmt::Display> ErrorCollector<S, E> {
    /// Log all collected errors at warn level
    pub fn log_errors(&self) {
        let errors = self.collect();
        if !errors.is_empty() {
            tracing::warn!("{} sources failed:", errors.len());
            for (source, error) in &errors {
                tracing::warn!("  - {}: {}", source, error);
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
    use super::*;
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

    #[test]
    fn test_fetch_outcome_basic() {
        let success: FetchOutcome<i32, &str> = FetchOutcome::success(42);
        assert_eq!(success.clone().ok(), Some(42));
        assert_eq!(success.clone().err(), None);

        let error: FetchOutcome<i32, &str> = FetchOutcome::error("failed");
        assert_eq!(error.clone().ok(), None);
        assert_eq!(error.clone().err(), Some("failed"));
    }

    #[test]
    fn test_fetch_outcome_map() {
        let success: FetchOutcome<i32, &str> = FetchOutcome::success(42);
        let mapped = success.map(|x| x * 2);
        assert_eq!(mapped.clone().ok(), Some(84));

        let error: FetchOutcome<i32, &str> = FetchOutcome::error("failed");
        let mapped_error = error.map(|x| x * 2);
        assert_eq!(mapped_error.clone().err(), Some("failed"));
    }

    #[test]
    fn test_fetch_result_constructors() {
        let success: FetchResult<&str, Vec<i32>, String> =
            FetchResult::success("api1", vec![1, 2, 3]);
        assert_eq!(success.source, "api1");
        assert!(matches!(success.outcome, FetchOutcome::Success(_)));
        assert_eq!(success.clone().ok(), Some(vec![1, 2, 3]));

        let error: FetchResult<&str, Vec<i32>, &str> =
            FetchResult::error("api2", "connection failed");
        assert_eq!(error.source, "api2");
        assert!(matches!(error.outcome, FetchOutcome::Error(_)));
        assert_eq!(error.clone().err(), Some("connection failed"));
    }

    #[test]
    fn test_fetch_result_new() {
        let result: FetchResult<&str, i32, String> =
            FetchResult::new("api", FetchOutcome::<i32, String>::success(100));
        assert_eq!(result.source, "api");
        assert_eq!(result.clone().ok(), Some(100));
    }

    #[test]
    fn test_fetch_result_map() {
        let result: FetchResult<&str, Vec<i32>, String> =
            FetchResult::success("api", vec![1, 2, 3]);
        let mapped = result.map(|v: Vec<i32>| v.len());
        assert_eq!(mapped.source, "api");
        assert_eq!(mapped.ok(), Some(3));

        let error_result: FetchResult<&str, Vec<i32>, &str> = FetchResult::error("api", "failed");
        let mapped_error = error_result.map(|v: Vec<i32>| v.len());
        assert_eq!(mapped_error.err(), Some("failed"));
    }

    #[test]
    fn test_fetch_result_display() {
        let success: FetchResult<&str, i32, &str> = FetchResult::success("api", 42);
        assert_eq!(format!("{success}"), "api: Success");

        let error: FetchResult<&str, i32, &str> = FetchResult::error("api", "timeout");
        assert_eq!(format!("{error}"), "api: Error - timeout");
    }

    #[test]
    fn test_error_collector_basic() {
        let collector: ErrorCollector<String, String> = ErrorCollector::new();
        assert!(!collector.has_errors());
        assert_eq!(collector.count(), 0);

        collector.record("api1".to_string(), "error1".to_string());
        assert!(collector.has_errors());
        assert_eq!(collector.count(), 1);

        collector.record("api2".to_string(), "error2".to_string());
        assert_eq!(collector.count(), 2);
    }

    #[test]
    fn test_error_collector_collect() {
        let collector: ErrorCollector<String, String> = ErrorCollector::new();
        collector.record("api1".to_string(), "error1".to_string());
        collector.record("api2".to_string(), "error2".to_string());

        let errors = collector.collect();
        assert_eq!(errors.len(), 2);
        assert_eq!(errors[0], ("api1".to_string(), "error1".to_string()));
        assert_eq!(errors[1], ("api2".to_string(), "error2".to_string()));
    }

    #[test]
    fn test_error_collector_clone() {
        let collector = ErrorCollector::new();
        let clone = collector.clone();

        clone.record("api", "error");
        assert_eq!(collector.count(), 1);
        assert_eq!(clone.count(), 1);
    }
}
