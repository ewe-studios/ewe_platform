//! User-facing API for HTTP client request execution.
//!
//! WHY: Provides clean, ergonomic API that completely hides TaskIterator complexity.
//! Users interact with simple methods like `.introduction()`, `.body()`, and `.send()`
//! without needing to understand the underlying executor mechanics.
//!
//! WHAT: Implements `ClientRequest` which wraps HTTP request execution. Supports both
//! progressive reading (intro first, then body) and one-shot execution (send everything).
//! Manages internal state machine for request lifecycle.
//!
//! HOW: Wraps `HttpRequestTask` execution via `execute_task()`. Uses internal state enum
//! to track progress through request lifecycle. Platform-aware executor driving
//! (single-threaded on WASM/multi=off, multi-threaded with multi=on).

use crate::io::ioutils::SharedByteBufferStream;
use crate::netcap::RawStream;
use crate::synca::mpp::RecvIterator;
use crate::valtron::{self, ReadyValues, TaskStatus};
use crate::wire::simple_http::client::{
    executor::execute_task, ClientConfig, ClientRequestBuilder, ConnectionPool, DnsResolver,
    HttpClientAction, HttpClientError, HttpRequestState, HttpRequestTask, HttpTaskReady,
    PreparedRequest, RequestControl, ResponseIntro,
};
use crate::wire::simple_http::{
    HttpResponseReader, IncomingResponseParts, SimpleBody, SimpleHeaders, SimpleHttpBody,
    SimpleResponse,
};
use std::sync::Arc;

/// Internal state for progressive request reading.
///
/// WHY: ClientRequest supports both progressive reading (introduction, then body)
/// and one-shot reading (send). This requires tracking execution state between
/// method calls.
///
/// WHAT: State machine tracking request lifecycle. Stores iterator and partial
/// results (intro, headers, stream) for progressive consumption.
///
/// HOW: Transitions from NotStarted -> Executing -> Completed. Executing state
/// holds all intermediate data needed for progressive reading.
enum ClientRequestState<R: DnsResolver + 'static> {
    /// Request hasn't been executed yet
    NotStarted,
    /// Request is currently executing or has partial results
    Executing {
        /// TaskIterator wrapped in RecvIterator for progress updates
        iter: RecvIterator<TaskStatus<HttpTaskReady, HttpRequestState, HttpClientAction<R>>>,
        /// Response introduction (status line) if received
        intro: Option<ResponseIntro>,
        /// Response headers if received
        headers: Option<SimpleHeaders>,
        /// Shared stream reference for connection pooling (future use)
        stream: Option<SharedByteBufferStream<RawStream>>,
    },
    /// Request completed (terminal state)
    Completed,
}

impl<R: DnsResolver + 'static> core::fmt::Debug for ClientRequestState<R> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::NotStarted => write!(f, "NotStarted"),
            Self::Completed => write!(f, "Completed"),
            Self::Executing {
                #[allow(unused_variables)]
                iter,
                intro,
                headers,
                #[allow(unused_variables)]
                stream,
            } => f
                .debug_struct("Executing")
                .field("intro", intro)
                .field("headers", headers)
                .finish(),
        }
    }
}

/// User-facing HTTP request execution handle.
///
/// WHY: Provides ergonomic API for executing HTTP requests with multiple consumption
/// patterns. Hides all TaskIterator and executor complexity from users.
///
/// WHAT: Wrapper around HTTP request execution that supports:
/// - Progressive reading: `.introduction()` then `.body()`
/// - One-shot execution: `.send()`
/// - Streaming: `.parts()` iterator
/// - Collection: `.collect()` all parts
///
/// HOW: Internally manages `HttpRequestTask` lifecycle via state machine.
/// Platform-aware executor driving (single vs multi).
/// Generic over DNS resolver for flexibility.
///
/// # Type Parameters
///
/// * `R` - DNS resolver type implementing `DnsResolver` trait
///
/// # Examples
///
/// ```ignore
/// // Progressive reading
/// let mut request = client.get("http://example.com")?;
/// let (intro, headers) = request.introduction()?;
/// let body = request.body()?;
///
/// // One-shot execution
/// let response = client.get("http://example.com")?.send()?;
///
/// // Streaming parts
/// for part in client.get("http://example.com")?.parts() {
///     match part? {
///         IncomingResponseParts::Intro(status, proto, reason) => { /* ... */ }
///         IncomingResponseParts::Headers(headers) => { /* ... */ }
///         // ...
///     }
/// }
/// ```
pub struct ClientRequest<R: DnsResolver + 'static> {
    /// The prepared HTTP request to execute
    prepared_request: PreparedRequest,
    /// DNS resolver for hostname resolution
    resolver: R,
    /// Client configuration (timeouts, redirects, etc.)
    config: ClientConfig,
    /// Connection pool for reuse
    pool: Option<Arc<ConnectionPool>>,
    /// Shared coordination with task
    control: Option<RequestControl>,
    /// Internal state machine for progressive reading
    task_state: Option<ClientRequestState<R>>,
    /// Stream for body reading and pool return
    stream: Option<SharedByteBufferStream<RawStream>>,
    /// Host for pool return
    host: Option<String>,
    /// Port for pool return
    port: Option<u16>,
}

impl<R: DnsResolver + 'static> ClientRequest<R> {
    /// Creates a new client request (internal constructor).
    ///
    /// WHY: SimpleHttpClient needs to construct ClientRequest instances from
    /// PreparedRequest. This constructor is internal to the client module.
    ///
    /// WHAT: Initializes request in NotStarted state with all necessary data.
    ///
    /// HOW: Stores all parameters for later execution. Doesn't start execution
    /// until user calls a consumption method.
    ///
    /// # Arguments
    ///
    /// * `prepared` - The prepared HTTP request with method, URL, headers, body
    /// * `resolver` - DNS resolver for hostname resolution
    /// * `config` - Client configuration (timeouts, redirects, pooling)
    /// * `pool` - Optional connection pool for reuse
    ///
    /// # Returns
    ///
    /// A new `ClientRequest` ready to execute.
    pub fn new(
        prepared: PreparedRequest,
        resolver: R,
        config: ClientConfig,
        pool: Option<Arc<ConnectionPool>>,
    ) -> Self {
        Self {
            prepared_request: prepared,
            resolver,
            config,
            pool,
            control: None,
            task_state: Some(ClientRequestState::NotStarted),
            stream: None,
            host: None,
            port: None,
        }
    }

    /// Executes request until introduction and headers are received.
    ///
    /// WHY: Some use cases need to inspect status and headers before reading body
    /// (e.g., conditional body processing, header validation).
    ///
    /// WHAT: Executes HTTP request, drives executor until ResponseIntro and Headers
    /// are received, returns both. Leaves request state ready for `.body()` call.
    ///
    /// HOW: Creates HttpRequestTask, spawns via execute_task(), drives executor
    /// (platform-aware), collects intro and headers from iterator, stores state
    /// for subsequent calls.
    ///
    /// # Returns
    ///
    /// Tuple of `(ResponseIntro, SimpleHeaders)` on success.
    ///
    /// # Errors
    ///
    /// Returns `HttpClientError` if:
    /// - Request already completed
    /// - DNS resolution fails
    /// - Connection fails
    /// - Request/response parsing fails
    ///
    /// # Examples
    ///
    /// ```ignore
    /// let mut request = client.get("http://example.com")?;
    /// let (intro, headers) = request.introduction()?;
    /// println!("Status: {}", intro.status);
    /// ```
    #[tracing::instrument(skip(self))]
    pub fn introduction(&mut self) -> Result<(ResponseIntro, SimpleHeaders), HttpClientError> {
        println!("Get current state");

        // Take state to check current phase
        let state = self
            .task_state
            .take()
            .ok_or_else(|| HttpClientError::Other("Request state missing".into()))?;

        println!("Running introduction process: {:?}", &state);

        match state {
            ClientRequestState::NotStarted => {
                println!("Entering not started state, starting execution");
                // tracing::info!("Entering not started state, starting execution");
                // First call - need to start execution
                self.start_execution()?;

                // Recursively call to process Executing state
                self.introduction()

                // Err(HttpClientError::Other("bad state".into()))
            }
            ClientRequestState::Executing {
                mut iter,
                intro,
                headers,
                stream,
            } => {
                tracing::info!("Entering executing state");

                // Check if we already have intro and headers
                if let (Some(intro_val), Some(headers_val)) = (&intro, &headers) {
                    self.task_state = Some(ClientRequestState::Executing {
                        iter,
                        intro: Some(intro_val.clone()),     // Consumed
                        headers: Some(headers_val.clone()), // Consumed
                        stream,
                    });
                    return Ok((intro_val.clone(), headers_val.clone()));
                }

                println!("Try to get the headers");

                // Need to collect intro and/or headers from iterator
                let mut collected_intro = intro;
                let mut collected_headers = headers;

                // Drive executor
                valtron::run_until_complete();

                println!("completed execution");

                // Collect from ReadyValues iterator
                for ready_item in ReadyValues::new(&mut iter) {
                    if let Some(HttpTaskReady::IntroAndHeaders { intro, headers }) =
                        ready_item.inner()
                    {
                        collected_intro = Some(intro);
                        collected_headers = Some(headers);
                        break;
                    }
                }

                println!("prepapre url and port");
                // Extract host and port from URL for pool return
                self.host = self.prepared_request.url.host_str().map(|s| s.to_string());
                self.port = Some(self.prepared_request.url.port().unwrap_or(80));

                // Check if we got both
                if collected_intro.is_none() || collected_headers.is_none() {
                    self.task_state = Some(ClientRequestState::Completed);
                    return Err(HttpClientError::Other(
                        "Failed to receive response introduction/headers".into(),
                    ));
                }

                let intro = collected_intro.unwrap();
                let headers = collected_headers.unwrap();

                // Store state for potential body() call
                // Keep iter for body() to continue
                self.task_state = Some(ClientRequestState::Executing {
                    iter,
                    intro: Some(intro.clone()),
                    headers: Some(headers.clone()),
                    stream,
                });

                Ok((intro, headers))
            }
            ClientRequestState::Completed => {
                tracing::debug!("Entering completed state");

                self.task_state = Some(ClientRequestState::Completed);
                Err(HttpClientError::Other("Request already completed".into()))
            }
        }
    }

    /// Continues execution to read response body.
    ///
    /// WHY: After inspecting introduction/headers, user may want to read the body.
    ///
    /// WHAT: Continues driving executor until body is fully received, returns as
    /// `SimpleBody`.
    ///
    /// HOW: Drives iterator from state stored by `.introduction()`, collects body
    /// parts, constructs SimpleBody.
    ///
    /// # Returns
    ///
    /// `SimpleBody` containing the response body.
    ///
    /// # Errors
    ///
    /// Returns `HttpClientError` if:
    /// - `.introduction()` hasn't been called yet
    /// - Request already completed
    /// - Body reading fails
    ///
    /// # Examples
    ///
    /// ```ignore
    /// let mut request = client.get("http://example.com")?;
    /// let (intro, headers) = request.introduction()?;
    /// let body = request.body()?;
    /// ```
    #[tracing::instrument(skip(self))]
    pub fn body(&mut self) -> Result<SimpleBody, HttpClientError> {
        // Signal task that we want the body
        if let Some(control) = &self.control {
            control.set_body_requested();
        } else {
            return Err(HttpClientError::Other(
                "Must call introduction() before body()".into(),
            ));
        }

        // Take state
        let state = self
            .task_state
            .take()
            .ok_or_else(|| HttpClientError::Other("Request state missing".into()))?;

        match state {
            ClientRequestState::NotStarted => {
                tracing::info!("Request in not started state");
                self.task_state = Some(ClientRequestState::NotStarted);
                Err(HttpClientError::Other(
                    "Must call introduction() before body()".into(),
                ))
            }
            ClientRequestState::Executing { mut iter, .. } => {
                tracing::info!("Starting executing state");

                // Drive executor to get stream ownership
                crate::valtron::run_until_complete();

                // Get stream from task
                for ready_item in ReadyValues::new(&mut iter) {
                    if let Some(HttpTaskReady::StreamOwnership(stream)) = ready_item.inner() {
                        self.stream = Some(stream.clone());

                        // Read body from stream using HttpResponseReader
                        let mut reader = HttpResponseReader::<SimpleHttpBody, RawStream>::new(
                            stream,
                            SimpleHttpBody,
                        );

                        // Collect body parts
                        let mut body = SimpleBody::None;
                        for part_result in &mut reader {
                            match part_result {
                                Ok(IncomingResponseParts::SizedBody(sized_body)) => {
                                    body = sized_body;
                                    break;
                                }
                                Ok(IncomingResponseParts::StreamedBody(streamed_body)) => {
                                    body = streamed_body;
                                    break;
                                }
                                Ok(_) => {
                                    // Skip other parts (intro, headers already processed)
                                    continue;
                                }
                                Err(e) => {
                                    self.task_state = Some(ClientRequestState::Completed);
                                    return Err(HttpClientError::Other(
                                        format!("Failed to read body: {:?}", e).into(),
                                    ));
                                }
                            }
                        }

                        self.task_state = Some(ClientRequestState::Completed);
                        return Ok(body);
                    }
                }

                self.task_state = Some(ClientRequestState::Completed);
                Err(HttpClientError::Other("Failed to receive stream".into()))
            }
            ClientRequestState::Completed => {
                self.task_state = Some(ClientRequestState::Completed);
                Err(HttpClientError::Other("Request already completed".into()))
            }
        }
    }

    /// Executes complete request and returns full response.
    ///
    /// WHY: Most use cases want the full response in one shot. This is the most
    /// ergonomic API for typical HTTP requests.
    ///
    /// WHAT: Executes HTTP request, drives executor to completion, collects all
    /// parts (intro, headers, body), constructs SimpleResponse, returns stream
    /// to pool if enabled.
    ///
    /// HOW: Creates and spawns HttpRequestTask, drives executor (platform-aware),
    /// collects all IncomingResponseParts, assembles SimpleResponse, handles
    /// connection pooling if configured.
    ///
    /// # Returns
    ///
    /// `SimpleResponse<SimpleBody>` with status, headers, and body.
    ///
    /// # Errors
    ///
    /// Returns `HttpClientError` if:
    /// - Request already executed
    /// - DNS resolution fails
    /// - Connection fails
    /// - Request/response parsing fails
    ///
    /// # Examples
    ///
    /// ```ignore
    /// let response = client.get("http://example.com")?.send()?;
    /// assert_eq!(response.get_status(), Status::OK);
    /// ```
    pub fn send(mut self) -> Result<SimpleResponse<SimpleBody>, HttpClientError> {
        // Get intro and headers first
        let (intro, headers) = self.introduction()?;

        // Get body
        let body = self.body()?;

        // Build complete response
        Ok(SimpleResponse::new(intro.status, headers, body))
    }

    /// Returns an iterator over response parts.
    ///
    /// WHY: Advanced users may want fine-grained control over response processing,
    /// handling each part as it arrives.
    ///
    /// WHAT: Returns iterator adapter that yields `IncomingResponseParts` as they
    /// are received from the server.
    ///
    /// HOW: Wraps internal TaskIterator, drives executor on each `next()` call,
    /// translates TaskStatus to IncomingResponseParts.
    ///
    /// # Returns
    ///
    /// Iterator yielding `Result<IncomingResponseParts, HttpClientError>`.
    ///
    /// # Examples
    ///
    /// ```ignore
    /// for part in client.get("http://example.com")?.parts() {
    ///     match part? {
    ///         IncomingResponseParts::Intro(status, proto, reason) => {
    ///             println!("Status: {}", status);
    ///         }
    ///         IncomingResponseParts::Headers(headers) => {
    ///             println!("Headers: {:?}", headers);
    ///         }
    ///         IncomingResponseParts::SizedBody(body) => {
    ///             println!("Body received");
    ///         }
    ///         _ => {}
    ///     }
    /// }
    /// ```
    pub fn parts(mut self) -> impl Iterator<Item = Result<IncomingResponseParts, HttpClientError>> {
        // Start execution if not already started
        if matches!(self.task_state, Some(ClientRequestState::NotStarted) | None) {
            if let Err(e) = self.start_execution() {
                // Return error iterator
                return PartsIterator::Error(Some(e));
            }
        }

        // Extract control for signaling
        let control = self.control.clone();

        // Extract iterator from state
        let state = self.task_state.take();
        match state {
            Some(ClientRequestState::Executing { iter, .. }) => {
                PartsIterator::Active(PartsIteratorInner {
                    iter,
                    reader: None,
                    intro_yielded: false,
                    headers_cache: None,
                    control,
                })
            }
            _ => PartsIterator::Error(Some(HttpClientError::Other("Invalid request state".into()))),
        }
    }

    /// Collects all response parts into a vector.
    ///
    /// WHY: Convenience method for users who want all parts but don't want to
    /// manually iterate.
    ///
    /// WHAT: Drives request to completion, collects all IncomingResponseParts
    /// into Vec.
    ///
    /// HOW: Uses `.parts()` iterator and collects all results.
    ///
    /// # Returns
    ///
    /// `Vec<IncomingResponseParts>` with all response parts.
    ///
    /// # Errors
    ///
    /// Returns `HttpClientError` on first error encountered.
    ///
    /// # Examples
    ///
    /// ```ignore
    /// let parts = client.get("http://example.com")?.collect()?;
    /// for part in parts {
    ///     // Process each part
    /// }
    /// ```
    pub fn collect(self) -> Result<Vec<IncomingResponseParts>, HttpClientError> {
        self.parts().collect()
    }

    /// Internal helper to start request execution.
    ///
    /// WHY: Multiple methods need to start execution if not already started.
    /// Centralizes the logic.
    ///
    /// WHAT: Creates HttpRequestTask, spawns via execute_task(), transitions
    /// state to Executing.
    ///
    /// HOW: Takes PreparedRequest, creates task with resolver and config,
    /// spawns using platform-appropriate executor, stores iterator in state.
    #[tracing::instrument(skip(self))]
    fn start_execution(&mut self) -> Result<(), HttpClientError> {
        // Take the prepared request to avoid cloning
        let request = std::mem::replace(
            &mut self.prepared_request,
            ClientRequestBuilder::get("http://placeholder.invalid")
                .unwrap()
                .build(),
        );

        // Create shared control
        let control = RequestControl::new();

        // Create HttpRequestTask with pool and control
        let task = HttpRequestTask::with_control(
            request,
            self.resolver.clone(),
            self.config.max_redirects,
            self.pool.clone(),
            control.clone(),
        );

        // Spawn task via execute_task
        let iter = execute_task(task)
            .map_err(|e| HttpClientError::Other(format!("Failed to spawn task: {}", e).into()))?;

        // Store control for later signaling
        self.control = Some(control);

        // Transition to Executing state
        self.task_state = Some(ClientRequestState::Executing {
            iter,
            intro: None,
            headers: None,
            stream: None,
        });

        Ok(())
    }
}

impl<R: DnsResolver + 'static> Drop for ClientRequest<R> {
    fn drop(&mut self) {
        // Return stream to pool if we have one
        if let (Some(pool), Some(stream), Some(host), Some(port)) =
            (&self.pool, &self.stream, &self.host, &self.port)
        {
            pool.checkin(host, *port, stream.clone());
        }
    }
}

// Iterator adapter for `.parts()` method.
///
/// WHY: Need custom iterator type to handle initialization errors and drive
/// the underlying TaskIterator.
///
/// WHAT: Wraps TaskIterator, translates TaskStatus to IncomingResponseParts.
///
/// HOW: Enum with Active and Error variants. Active drives executor on each
/// next() call.
enum PartsIterator<R: DnsResolver + 'static> {
    Active(PartsIteratorInner<R>),
    Error(Option<HttpClientError>),
}

/// Inner active iterator implementation.
struct PartsIteratorInner<R: DnsResolver + 'static> {
    iter: RecvIterator<TaskStatus<HttpTaskReady, HttpRequestState, HttpClientAction<R>>>,
    reader: Option<HttpResponseReader<SimpleHttpBody, RawStream>>,
    intro_yielded: bool,
    headers_cache: Option<SimpleHeaders>,
    control: Option<RequestControl>,
}

impl<R: DnsResolver + 'static> Iterator for PartsIterator<R> {
    type Item = Result<IncomingResponseParts, HttpClientError>;

    fn next(&mut self) -> Option<Self::Item> {
        match self {
            PartsIterator::Active(inner) => {
                // If we have a reader, continue reading from it
                if let Some(reader) = &mut inner.reader {
                    // First check if we have cached headers to yield
                    if !inner.intro_yielded && inner.headers_cache.is_some() {
                        inner.intro_yielded = true;
                        let headers = inner.headers_cache.take().unwrap();

                        // After yielding headers, signal task to provide stream
                        // (Do this after headers but before reading body)
                        if let Some(control) = &inner.control {
                            control.set_body_requested();
                        }

                        return Some(Ok(IncomingResponseParts::Headers(headers)));
                    }

                    // Drive executor (platform-aware)
                    crate::valtron::run_once();

                    // Read next part from reader
                    match reader.next() {
                        Some(Ok(part)) => return Some(Ok(part)),
                        Some(Err(e)) => {
                            return Some(Err(HttpClientError::Other(
                                format!("Failed to read response part: {:?}", e).into(),
                            )))
                        }
                        None => return None,
                    }
                }

                // Drive executor (platform-aware)
                valtron::run_once();

                // Get next ready value from task
                for ready_item in ReadyValues::new(&mut inner.iter).take(1) {
                    if let Some(ready) = ready_item.inner() {
                        match ready {
                            HttpTaskReady::IntroAndHeaders { intro, headers } => {
                                // Cache headers for next call, return intro first
                                inner.headers_cache = Some(headers);
                                return Some(Ok(IncomingResponseParts::Intro(
                                    intro.status,
                                    intro.proto,
                                    intro.reason,
                                )));
                            }
                            HttpTaskReady::StreamOwnership(stream) => {
                                // Create reader from stream
                                let reader = HttpResponseReader::<SimpleHttpBody, RawStream>::new(
                                    stream,
                                    SimpleHttpBody,
                                );
                                inner.reader = Some(reader);

                                // Recursively call to start reading from reader
                                return self.next();
                            }
                        }
                    }
                }

                None
            }
            PartsIterator::Error(err) => err.take().map(Err),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::wire::simple_http::client::{ClientRequestBuilder, MockDnsResolver};

    // ========================================================================
    // ClientRequest Construction Tests
    // ========================================================================

    /// WHY: Verify ClientRequest::new creates request in NotStarted state
    /// WHAT: Tests that constructor initializes correctly
    #[test]
    fn test_client_request_new() {
        let prepared = ClientRequestBuilder::get("http://example.com")
            .unwrap()
            .build();
        let resolver = MockDnsResolver::new();
        let config = ClientConfig::default();

        let request = ClientRequest::new(prepared, resolver, config, None);

        assert!(matches!(
            request.task_state,
            Some(ClientRequestState::NotStarted)
        ));
    }

    /// WHY: Verify ClientRequest stores configuration correctly
    /// WHAT: Tests that config parameters are preserved
    #[test]
    fn test_client_request_stores_config() {
        let prepared = ClientRequestBuilder::get("http://example.com")
            .unwrap()
            .build();
        let resolver = MockDnsResolver::new();
        let mut config = ClientConfig::default();
        config.max_redirects = 3;

        let request = ClientRequest::new(prepared, resolver, config, None);

        assert_eq!(request.config.max_redirects, 3);
    }

    // ========================================================================
    // State Machine Tests
    // ========================================================================

    /// WHY: Verify ClientRequestState enum has expected variants
    /// WHAT: Tests that state enum compiles with correct structure
    #[test]
    fn test_client_request_state_variants() {
        // Compile-time check that variants exist
        let _not_started: ClientRequestState<MockDnsResolver> = ClientRequestState::NotStarted;
        let _completed: ClientRequestState<MockDnsResolver> = ClientRequestState::Completed;
    }

    // ========================================================================
    // API Method Signature Tests
    // ========================================================================

    /// WHY: Verify ClientRequest has correct public API methods
    /// WHAT: Compile-time check that methods exist with correct signatures
    #[test]
    fn test_client_request_has_expected_methods() {
        crate::valtron::initialize_pool(20, None);

        let prepared = ClientRequestBuilder::get("http://example.com")
            .unwrap()
            .build();
        let resolver = MockDnsResolver::new();
        let config = ClientConfig::default();

        let mut request = ClientRequest::new(prepared, resolver, config, None);

        // These should compile (even if they fail at runtime due to mock resolver)
        let _intro_result = request.introduction();
        let _body_result = request.body();

        // These consume self
        let prepared2 = ClientRequestBuilder::get("http://example.com")
            .unwrap()
            .build();
        let request2 = ClientRequest::new(
            prepared2,
            MockDnsResolver::new(),
            ClientConfig::default(),
            None,
        );
        let _send_result = request2.send();

        let prepared3 = ClientRequestBuilder::get("http://example.com")
            .unwrap()
            .build();
        let request3 = ClientRequest::new(
            prepared3,
            MockDnsResolver::new(),
            ClientConfig::default(),
            None,
        );
        let _parts_iter = request3.parts();

        let prepared4 = ClientRequestBuilder::get("http://example.com")
            .unwrap()
            .build();
        let request4 = ClientRequest::new(
            prepared4,
            MockDnsResolver::new(),
            ClientConfig::default(),
            None,
        );
        let _collect_result = request4.collect();
    }

    /// WHY: Verify parts() returns iterator with correct item type
    /// WHAT: Type check that iterator yields Result<IncomingResponseParts, HttpClientError>
    #[test]
    fn test_client_request_parts_iterator_type() {
        crate::valtron::initialize_pool(20, None);

        let prepared = ClientRequestBuilder::get("http://example.com")
            .unwrap()
            .build();
        let resolver = MockDnsResolver::new();
        let config = ClientConfig::default();

        let request = ClientRequest::new(prepared, resolver, config, None);
        let mut parts_iter = request.parts();

        // Type check
        let _item: Option<Result<IncomingResponseParts, HttpClientError>> = parts_iter.next();
    }

    /// WHY: Verify ClientRequest API compiles and types are correct
    /// WHAT: Basic compile-time check for the complete flow
    #[test]
    fn test_client_request_api_compiles() {
        // This test just verifies the API compiles correctly
        // Real end-to-end tests with actual HTTP server are in integration tests
        assert!(true, "API compiles successfully");
    }

    // Note: Full integration tests with real HTTP requests are in tests/ directory
    // These tests focus on structure, types, and state machine logic
}
