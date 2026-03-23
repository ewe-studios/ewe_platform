//! User-facing API for HTTP client request execution.
//!
//! WHY: Provides clean, ergonomic API that completely hides `TaskIterator` complexity.
//! Users interact with simple methods like `.introduction()`, `.body()`, and `.send()`
//! without needing to understand the underlying executor mechanics.
//!
//! WHAT: Implements `ClientRequest` which wraps HTTP request execution. Supports both
//! progressive reading (intro first, then body) and one-shot execution (send everything).
//! Manages internal state machine for request lifecycle.
//!
//! HOW: Wraps `HttpRequestTask` execution via `execute()`. Uses internal state enum
//! to track progress through request lifecycle. Platform-aware executor driving
//! (single-threaded on WASM/multi=off, multi-threaded with multi=on).

use crate::valtron::{
    self, DrivenStreamIterator, MapDone, SplitCollectorMapContinuation, SplitCollectorMapObserver,
    Stream, StreamIteratorExt, TaskIteratorExt,
};
use crate::wire::simple_http::client::{
    ClientConfig, DnsResolver, HttpClientConnection, HttpConnectionPool, HttpRequestPending,
    MiddlewareChain, PreparedRequest, RequestIntro, ResponseIntro, SendRequestTask,
};
use crate::wire::simple_http::{
    HttpClientError, IncomingResponseParts, SendSafeBody, SimpleHeaders, SimpleResponse, Status,
};
use std::sync::Arc;

pub type DrivenBodyStream<R> = DrivenStreamIterator<
    SplitCollectorMapContinuation<SendRequestTask<R>, (ResponseIntro, SimpleHeaders)>,
>;

pub type MappedDrivenBodyStream<R> = MapDone<
    DrivenBodyStream<R>,
    RequestIntro,
    HttpRequestPending,
    Result<(HttpClientConnection, SendSafeBody), HttpClientError>,
>;

pub type RequestIntroStream =
    SplitCollectorMapObserver<(ResponseIntro, SimpleHeaders), HttpRequestPending>;

/// Internal state for progressive request reading.
///
/// WHY: `ClientRequest` supports both progressive reading (introduction, then body)
/// and one-shot reading (send). This requires tracking execution state between
/// method calls.
///
/// WHAT: State machine tracking request lifecycle. Stores iterator and partial
/// results (intro, headers, stream) for progressive consumption.
///
/// HOW: Transitions from `NotStarted` -> Executing -> Completed. Executing state
/// holds all intermediate data needed for progressive reading.
#[derive(PartialEq, Eq, PartialOrd, Clone)]
pub enum ClientRequestState {
    NotStarted,
    Executing,
    Failed,
}

impl core::fmt::Debug for ClientRequestState {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Failed => write!(f, "Failed"),
            Self::Executing => write!(f, "Executing"),
            Self::NotStarted => write!(f, "NotStarted"),
        }
    }
}

pub struct FinalizedResponse<T, R: DnsResolver + 'static>(
    SimpleResponse<T>,
    Option<Arc<HttpConnectionPool<R>>>,
    Option<HttpClientConnection>,
);

impl<T, R: DnsResolver + 'static> FinalizedResponse<T, R> {
    pub fn new(
        response: SimpleResponse<T>,
        conn: HttpClientConnection,
        pool: Arc<HttpConnectionPool<R>>,
    ) -> Self {
        Self(response, Some(pool), Some(conn))
    }
}

impl<T, R: DnsResolver + 'static> FinalizedResponse<T, R> {
    pub fn get_status(&self) -> Status {
        self.0.get_status()
    }

    pub fn get_headers_ref(&self) -> &SimpleHeaders {
        self.0.get_headers_ref()
    }

    pub fn get_headers_mut(&mut self) -> &mut SimpleHeaders {
        self.0.get_headers_mut()
    }

    pub fn get_body_ref(&self) -> &T {
        self.0.get_body_ref()
    }

    pub fn get_body_mut(&mut self) -> &mut T {
        self.0.get_body_mut()
    }
}

impl<T, R: DnsResolver + 'static> Drop for FinalizedResponse<T, R> {
    fn drop(&mut self) {
        // Return stream to pool if we have one
        if let (Some(pool), Some(stream)) = (self.1.take(), self.2.take()) {
            pool.return_to_pool(stream);
        }
    }
}

/// User-facing HTTP request execution handle.
///
/// WHY: Provides ergonomic API for executing HTTP requests with multiple consumption
/// patterns. Hides all `TaskIterator` and executor complexity from users.
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
    /// Client configuration (timeouts, redirects, etc.)
    config: ClientConfig,

    /// Connection pool for reuse
    pool: Option<Arc<HttpConnectionPool<R>>>,

    /// Middleware chain for request/response interception
    middleware_chain: Arc<MiddlewareChain>,

    /// Original request for middleware response processing
    original_request: Option<PreparedRequest>,

    /// The prepared HTTP request to execute
    prepared_request: Option<PreparedRequest>,

    /// Internal state machine for progressive reading
    task_state: ClientRequestState,
}

impl<R: DnsResolver + 'static> ClientRequest<R> {
    /// Creates a new client request (internal constructor).
    ///
    /// WHY: `SimpleHttpClient` needs to construct `ClientRequest` instances from
    /// `PreparedRequest`. This constructor is internal to the client module.
    ///
    /// WHAT: Initializes request in `NotStarted` state with all necessary data.
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
    #[must_use]
    pub fn new(
        prepared: PreparedRequest,
        config: ClientConfig,
        pool: Arc<HttpConnectionPool<R>>,
        middleware_chain: Arc<MiddlewareChain>,
    ) -> Self {
        Self {
            config,
            middleware_chain,
            pool: Some(pool),
            original_request: None,
            prepared_request: Some(prepared),
            task_state: ClientRequestState::NotStarted,
        }
    }

    /// Executes complete request and returns full response.
    ///
    /// WHY: Most use cases want the full response in one shot. This is the most
    /// ergonomic API for typical HTTP requests.
    ///
    /// WHAT: Executes HTTP request, drives executor to completion, collects all
    /// parts (intro, headers, body), constructs `SimpleResponse`, returns stream
    /// to pool if enabled.
    ///
    /// HOW: Creates and spawns `HttpRequestTask`, drives executor (platform-aware),
    /// collects all `IncomingResponseParts`, assembles `SimpleResponse`, handles
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
    /// # Panics
    ///
    /// Panics if the response intro or body is unexpectedly missing after successful execution.
    ///
    /// # Examples
    ///
    /// ```ignore
    /// let response = client.get("http://example.com")?.send()?;
    /// assert_eq!(response.get_status(), Status::OK);
    /// ```
    #[tracing::instrument(skip(self))]
    pub fn send(mut self) -> Result<FinalizedResponse<SendSafeBody, R>, HttpClientError> {
        let (intro_stream, body_stream) = self.start()?;

        let mut intro_data: Option<(ResponseIntro, SimpleHeaders)> = None;
        for intro_element in intro_stream {
            if let Stream::Next(value) = intro_element {
                intro_data = Some(value);
                break;
            }
        }

        // If no intro data, check body stream for errors (e.g., TooManyRedirects)
        if intro_data.is_none() {
            for body_element in body_stream {
                if let Stream::Next(Err(err)) = body_element {
                    return Err(err);
                }
            }
            return Err(HttpClientError::InvalidRequestState);
        }

        let mut response_body: Option<(HttpClientConnection, SendSafeBody)> = None;
        for body_element in body_stream {
            if let Stream::Next(value) = body_element {
                match value {
                    Ok(res) => {
                        response_body = Some(res);
                        break;
                    }
                    Err(err) => return Err(err),
                }
            }
        }

        // Build complete response
        let (intro, headers) = intro_data.expect("should have intro");
        let (conn, body) = response_body.expect("should have body");
        let mut response = SimpleResponse::new(intro.status, headers, body);

        // Apply middleware to response (after receiving)
        if let Some(request) = &self.original_request {
            self.middleware_chain
                .process_response(request, &mut response)?;
        }

        Ok(FinalizedResponse::new(
            response,
            conn,
            self.pool.take().expect("should have pool"),
        ))
    }

    /// Internal helper to start request execution.
    ///
    /// WHY: Multiple methods need to start execution if not already started.
    /// Centralizes the logic.
    ///
    /// WHAT: Creates `HttpRequestTask`, spawns via `execute_task()`, transitions
    /// state to Executing.
    ///
    /// HOW: Takes `PreparedRequest`, creates task with resolver and config,
    /// spawns using platform-appropriate executor, stores iterator in state.
    ///
    /// # Errors
    ///
    /// Returns `HttpClientError` if:
    /// - No connection pool is available
    /// - Request is not in the correct state to start
    /// - No prepared request to send
    /// - Middleware processing fails
    /// - Task spawning fails
    #[tracing::instrument(skip(self))]
    pub fn start(
        &mut self,
    ) -> Result<(RequestIntroStream, MappedDrivenBodyStream<R>), HttpClientError> {
        if self.pool.is_none() {
            return Err(HttpClientError::NoPool);
        }

        if self.task_state != ClientRequestState::NotStarted {
            return Err(HttpClientError::InvalidReadState);
        }

        if self.task_state == ClientRequestState::Failed {
            return Err(HttpClientError::FailedExecution);
        }

        // Take the prepared request to avoid cloning
        let Some(mut request) = self.prepared_request.take() else {
            return Err(HttpClientError::NoRequestToSend);
        };

        // Apply middleware to request (before sending)
        self.middleware_chain.process_request(&mut request)?;

        // Store request metadata for response middleware (without body)
        self.original_request = Some(PreparedRequest {
            method: request.method.clone(),
            url: request.url.clone(),
            headers: request.headers.clone(),
            body: SendSafeBody::None,
            extensions: std::mem::take(&mut request.extensions),
        });

        // Transition to Executing state
        self.task_state = ClientRequestState::Executing;

        // Create HttpRequestTask with pool and control
        let (observer, task) = SendRequestTask::new(
            request,
            self.config.max_redirects,
            self.pool.clone().ok_or(HttpClientError::NoPool)?,
            self.config.clone(),
        )
        .split_collect_one_map(|ready| match ready {
            RequestIntro::Success {
                stream: _,
                conn: _,
                intro,
                headers,
            } => {
                tracing::debug!("RequestIntro::Success received response: intro={:?}", intro);
                let response_intro: ResponseIntro = intro.clone().into();
                (true, Some((response_intro, headers.clone())))
            }
            RequestIntro::Failed(err) => {
                tracing::debug!("RequestIntro::Failed during execution: {err:?}");
                (false, None)
            }
        });

        // Spawn task via execute_task
        let body_stream = valtron::execute(task, None)
            .map_err(|e| HttpClientError::FailedWith(format!("Failed to spawn task: {e}").into()))
            .map(|iter| {
                iter.map_done(|done| match done {
                    RequestIntro::Success {
                        stream,
                        conn,
                        intro,
                        headers: _,
                    } => {
                        tracing::debug!("Body stream recevied RequestIntro::Success={:?}", intro);
                        for next_element in stream {
                            match next_element {
                                Ok(response_value) => match response_value {
                                    IncomingResponseParts::SKIP => {
                                        tracing::debug!("IncomingResponseParts::Skip seen");
                                    }
                                    IncomingResponseParts::Intro(_, _, _)
                                    | IncomingResponseParts::Headers(_) => {
                                        tracing::debug!(
                                            "IncomingResponseParts::Intro or Headers invalid state"
                                        );
                                        return Err(HttpClientError::InvalidReadState);
                                    }
                                    IncomingResponseParts::NoBody => {
                                        tracing::debug!("IncomingResponseParts::NoBody seen");
                                        return Ok((conn, SendSafeBody::None));
                                    }
                                    IncomingResponseParts::SizedBody(inner)
                                    | IncomingResponseParts::StreamedBody(inner) => {
                                        tracing::debug!(
                                            "IncomingResponseParts::Sized/Streamed body seen"
                                        );
                                        return Ok((conn, inner));
                                    }
                                },
                                Err(err) => return Err(HttpClientError::ReaderError(err)),
                            }
                        }
                        Err(HttpClientError::InvalidState)
                    }
                    RequestIntro::Failed(err) => {
                        tracing::error!("Body stream received RequestIntro::Failed: {:?}", err);
                        Err(err)
                    }
                })
            })?;

        Ok((observer, body_stream))
    }
}
