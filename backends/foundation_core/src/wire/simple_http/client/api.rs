//! User-facing API for HTTP client request execution.
//!
//! WHY: Provides clean, ergonomic API that completely hides `TaskIterator` complexity.
//! Users interact with simple methods like `.introduction()`, `.body()`, and `.send()`
//! without needing to understand the underlying executor mechanics.
//!
//! WHAT: Implements `ClientRequest` which wraps HTTP request execution. Supports both
//! progressive reading (intro first, then body) and one-shot execution (send everything).
//! Uses `split_collect_until()` to fork intro/headers from body continuation.
//!
//! HOW: Uses `split_collect_until()` on SendRequestTask:
//! - Observer: receives RequestIntro::Success, extracts (intro, headers, conn), completes
//! - Continuation: keeps stream for body reading
//! Platform-aware executor driving (single-threaded on WASM/multi=off, multi-threaded with multi=on).

use foundation_nostd::primitives::wait_duration;

use crate::netcap::RawStream;
use crate::valtron::{self, SplitUntilObserverMap, Stream, TaskStatus, TaskIteratorExt};
use crate::wire::simple_http::client::{
    ClientConfig, DnsResolver, GetHttpRequestRedirectTask, HttpClientConnection,
    HttpConnectionPool, HttpRequestPending, HttpRequestRedirectResponse, IncomingResponseMapper,
    MiddlewareChain, PreparedRequest, RequestIntro, RequestIntroData, ResponseIntro, SendRequestTask,
};
use crate::wire::simple_http::{
    HttpClientError, HttpResponseReader, IncomingResponseParts, SendSafeBody, SimpleHeaders,
    SimpleHttpBody, SimpleResponse,
};
use std::io::Write;
use std::marker::PhantomData;
use std::sync::Arc;

/// Observer branch from split_collect_until_map - receives RequestIntroData then completes.
type IntroObserver = SplitUntilObserverMap<RequestIntroData, HttpRequestPending>;

/// Continuation branch - keeps stream for body reading.
/// We box the continuation to erase the closure type.
type BodyContinuation = Box<
    dyn Iterator<
            Item = TaskStatus<
                RequestIntro,
                HttpRequestPending,
                crate::valtron::BoxedSendExecutionAction,
            >,
        > + Send
        + 'static,
>;

/// Internal state for progressive request reading using split_collect_until.
///
/// WHY: split_collect_until forks the task into observer (intro) and continuation (body).
/// Need to store both branches between method calls.
///
/// WHAT: State machine with split collector branches instead of manual iterator driving.
///
/// HOW: NotStarted → Split(observer, continuation) → IntroReady(stored results) → Completed.
pub enum ClientRequestState<R: DnsResolver + 'static> {
    /// Request hasn't been executed yet
    NotStarted(PhantomData<R>),
    /// Split into observer (gets intro/headers) and continuation (keeps stream)
    Split {
        observer: Option<IntroObserver>,
        continuation: Option<BodyContinuation>,
        /// Predicate function - needs to be stored
        predicate: Option<fn(&RequestIntro) -> bool>,
    },
    /// Intro/headers received and stored, continuation ready for body()
    IntroReady(RequestIntroData, Option<BodyContinuation>),
    /// Request completed (terminal state)
    Completed,
}

impl<R: DnsResolver + 'static> core::fmt::Debug for ClientRequestState<R> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Completed => write!(f, "Completed"),
            Self::NotStarted(_) => write!(f, "NotStarted"),
            Self::Split { .. } => write!(f, "Split"),
            Self::IntroReady(_, _) => write!(f, "IntroReady"),
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
    /// The prepared HTTP request to execute
    prepared_request: Option<PreparedRequest>,
    /// Client configuration (timeouts, redirects, etc.)
    config: ClientConfig,
    /// Connection pool for reuse
    pool: Option<Arc<HttpConnectionPool<R>>>,
    /// Internal state machine for progressive reading
    task_state: Option<ClientRequestState<R>>,
    /// Stream for body reading and pool return
    stream: Option<HttpClientConnection>,
    /// Middleware chain for request/response interception
    middleware_chain: Arc<MiddlewareChain>,
    /// Original request for middleware response processing
    original_request: Option<PreparedRequest>,
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
            prepared_request: Some(prepared),
            config,
            stream: None,
            pool: Some(pool),
            task_state: Some(ClientRequestState::NotStarted(PhantomData)),
            middleware_chain,
            original_request: None,
        }
    }

    /// Executes request until introduction and headers are received.
    ///
    /// WHY: Some use cases need to inspect status and headers before reading body
    /// (e.g., conditional body processing, header validation).
    ///
    /// WHAT: Executes HTTP request, drives executor until `ResponseIntro` and Headers
    /// are received, returns both. Leaves request state ready for `.body()` call.
    ///
    /// HOW: Creates `HttpRequestTask`, spawns via `execute_task()`, drives executor
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
        self.introduction_with_connection()
            .map(|(_, intro, headers)| (intro, headers))
    }

    /// Executes request and returns connection, introduction, and headers.
    ///
    /// WHY: Internal method that provides both the connection and response intro.
    /// Used by `parts()` to return the connection for pool management.
    ///
    /// WHAT: Same as `introduction()` but also returns the `HttpClientConnection`
    /// for connection pooling.
    ///
    /// # Returns
    ///
    /// Tuple of `(HttpClientConnection, ResponseIntro, SimpleHeaders)` on success.
    ///
    /// # Errors
    ///
    /// Returns `HttpClientError::FailedExecution` if request execution fails.
    /// Returns `HttpClientError::InvalidRequestState` if called in wrong state.
    /// Returns `HttpClientError` variants for DNS, connection, or parsing errors.
    #[tracing::instrument(skip(self))]
    pub fn introduction_with_connection(
        &mut self,
    ) -> Result<(HttpClientConnection, ResponseIntro, SimpleHeaders), HttpClientError> {
        // Check if already in IntroReady state
        if let Some(ClientRequestState::IntroReady(intro_data, _)) = &self.task_state {
            return Ok((
                intro_data.conn.clone(),
                intro_data.intro.clone(),
                intro_data.headers.clone(),
            ));
        }

        // Drive the observer until it completes
        loop {
            tracing::debug!("Get next observer state");
            if let Some(val) = self.task_state.take() {
                tracing::debug!("Running introduction process: {:?}", &val);
                match val {
                    ClientRequestState::NotStarted(_) => {
                        self.start()?;
                        continue;
                    }
                    ClientRequestState::Split {
                        observer,
                        continuation,
                        predicate,
                    } => {
                        tracing::debug!("Driving observer branch");

                        // Observer should be Some at this point
                        let Some(mut observer) = observer else {
                            tracing::debug!("Observer is None in Split state");
                            self.task_state = Some(ClientRequestState::Completed);
                            return Err(HttpClientError::FailedExecution);
                        };

                        let Some(stream_item) = observer.next() else {
                            // Observer completed without getting intro data
                            tracing::debug!("Observer completed without intro data");
                            self.task_state = Some(ClientRequestState::Completed);
                            return Err(HttpClientError::FailedExecution);
                        };

                        match stream_item {
                            Stream::Ignore => {
                                // Still waiting for data, put observer back
                                tracing::debug!("Observer returned Ignore, still waiting");
                                self.task_state = Some(ClientRequestState::Split {
                                    observer: Some(observer),
                                    continuation,
                                    predicate,
                                });
                                continue;
                            }
                            Stream::Next(intro_data) => {
                                tracing::debug!(
                                    "Observer received RequestIntroData: intro={:?}",
                                    &intro_data.intro
                                );

                                // Observer is now closed (predicate was met), transition to IntroReady
                                self.task_state =
                                    Some(ClientRequestState::IntroReady(intro_data, continuation));

                                let ClientRequestState::IntroReady(data, _) =
                                    self.task_state.as_ref().unwrap()
                                else {
                                    unreachable!()
                                };

                                return Ok((
                                    data.conn.clone(),
                                    data.intro.clone(),
                                    data.headers.clone(),
                                ));
                            }
                            Stream::Pending(_) => {
                                tracing::debug!("Observer returned Pending");
                                self.task_state = Some(ClientRequestState::Split {
                                    observer: Some(observer),
                                    continuation,
                                    predicate,
                                });
                                continue;
                            }
                            Stream::Delayed(dur) => {
                                tracing::debug!("Observer returned Delayed: {:?}", dur);
                                self.task_state = Some(ClientRequestState::Split {
                                    observer: Some(observer),
                                    continuation,
                                    predicate,
                                });
                                wait_duration(dur);
                                continue;
                            }
                            Stream::Init => {
                                tracing::debug!("Observer returned Init");
                                self.task_state = Some(ClientRequestState::Split {
                                    observer: Some(observer),
                                    continuation,
                                    predicate,
                                });
                                continue;
                            }
                        }
                    }
                    ClientRequestState::IntroReady(data, _) => {
                        return Ok((
                            data.conn.clone(),
                            data.intro.clone(),
                            data.headers.clone(),
                        ));
                    }
                    ClientRequestState::Completed => {
                        return Err(HttpClientError::FailedWith(
                            "Request response already completely read".into(),
                        ));
                    }
                }
            }

            break;
        }

        self.task_state = Some(ClientRequestState::Completed);
        Err(HttpClientError::FailedWith("Request state missing".into()))
    }

    /// Internal helper to start request execution using split_collect_until_map.
    ///
    /// WHY: split_collect_until_map forks the task into observer (gets RequestIntroData) and continuation (body).
    ///
    /// WHAT: Creates SendRequestTask, applies split_collect_until_map with transform to extract RequestIntroData, stores both branches.
    ///
    /// HOW: Takes PreparedRequest, creates SendRequestTask, splits it with transform, stores observer and continuation.
    #[tracing::instrument(skip(self))]
    fn start(&mut self) -> Result<(), HttpClientError> {
        if self.pool.is_none() {
            return Err(HttpClientError::NoPool);
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

        // Create SendRequestTask with pool and control
        let task = SendRequestTask::new(
            request,
            self.config.max_redirects,
            self.pool.clone().ok_or(HttpClientError::NoPool)?,
            self.config.clone(),
        );

        // Define predicate: close observer when we get RequestIntro::Success
        const fn is_intro_success(item: &RequestIntro) -> bool {
            matches!(item, RequestIntro::Success { .. })
        }

        // Define transform: extract RequestIntroData from RequestIntro::Success
        fn extract_intro_data(item: &RequestIntro) -> Option<RequestIntroData> {
            item.to_cloneable_data()
        }

        // Split using split_collect_until_map: observer gets RequestIntroData, continuation keeps stream
        let (observer, continuation) =
            task.split_collect_until_map(is_intro_success, extract_intro_data, 1);

        // Transition to Split state with both branches
        self.task_state = Some(ClientRequestState::Split {
            observer: Some(observer),
            continuation: Some(Box::new(continuation)),
            predicate: Some(is_intro_success),
        });

        Ok(())
    }

    /// Continues execution to read response body.
    ///
    /// WHY: After inspecting introduction/headers, user may want to read the body.
    ///
    /// WHAT: Continues driving executor until body is fully received, returns as
    /// `SimpleBody`.
    ///
    /// HOW: Drives iterator from state stored by `.introduction()`, collects body
    /// parts, constructs `SimpleBody`.
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
    pub fn body(&mut self) -> Result<SendSafeBody, HttpClientError> {
        tracing::debug!("Requesting response body next");

        let Some(state) = self.task_state.take() else {
            return Err(HttpClientError::InvalidRequestState);
        };

        tracing::debug!("Reading request processing state: {state:?}");

        match state {
            ClientRequestState::NotStarted(_) | ClientRequestState::Split { .. } => {
                tracing::error!("client found in invalid state - introduction() not called first");
                self.task_state = Some(ClientRequestState::Completed);
                Err(HttpClientError::FailedWith(
                    "request client in invalid state - call introduction() first".into(),
                ))
            }
            ClientRequestState::Completed => {
                tracing::error!("client found in completed state");
                Err(HttpClientError::FailedWith(
                    "request already completed".into(),
                ))
            }
            ClientRequestState::IntroReady(_intro_data, Some(mut continuation)) => {
                tracing::info!("Pulling body from continuation");

                // Drive continuation to get the stream
                // The continuation should yield the RequestIntro::Success with the stream
                loop {
                    let Some(task_status) = continuation.next() else {
                        tracing::error!("Continuation exhausted without yielding stream");
                        self.task_state = Some(ClientRequestState::Completed);
                        return Err(HttpClientError::FailedToReadBody);
                    };

                    match task_status {
                        TaskStatus::Ready(request_intro) => {
                            match request_intro {
                                RequestIntro::Success { stream, .. } => {
                                    tracing::info!(
                                        "Continuation yielded stream, reading body"
                                    );

                                    // Read body from stream
                                    for next_value in stream {
                                        match next_value {
                                            Ok(next_res) => {
                                                match next_res {
                                                    IncomingResponseParts::Intro(_, _, _)
                                                    | IncomingResponseParts::Headers(_) => {
                                                        tracing::debug!("IncomingResponseParts::Intro or Headers invalid state for body reading");
                                                        return Err(HttpClientError::InvalidReadState);
                                                    }
                                                    IncomingResponseParts::SKIP => {
                                                        tracing::debug!("IncomingResponseParts::Skip seen");
                                                    }
                                                    IncomingResponseParts::NoBody => {
                                                        tracing::debug!("IncomingResponseParts::NoBody seen");
                                                        self.task_state =
                                                            Some(ClientRequestState::Completed);
                                                        return Ok(SendSafeBody::None);
                                                    }
                                                    IncomingResponseParts::SizedBody(inner)
                                                    | IncomingResponseParts::StreamedBody(
                                                        inner,
                                                    ) => {
                                                        tracing::debug!(
                                                            "IncomingResponseParts::Sized/Streamed body seen"
                                                        );
                                                        self.task_state = Some(
                                                            ClientRequestState::Completed,
                                                        );
                                                        return Ok(inner);
                                                    }
                                                }
                                            }
                                            Err(err) => {
                                                tracing::debug!(
                                                    "Body retrieved failed with error: {err:?}"
                                                );
                                                return Err(HttpClientError::ReaderError(err));
                                            }
                                        }
                                    }

                                    tracing::debug!("Body stream exhausted without body");
                                    self.task_state = Some(ClientRequestState::Completed);
                                    return Err(HttpClientError::FailedToReadBody);
                                }
                                RequestIntro::Failed(err) => {
                                    tracing::debug!("RequestIntro::Failed from continuation: {err:?}");
                                    return Err(err);
                                }
                            }
                        }
                        TaskStatus::Pending(_) => {
                            tracing::debug!("Continuation returned Pending, still waiting");
                            continue;
                        }
                        TaskStatus::Delayed(dur) => {
                            tracing::debug!("Continuation returned Delayed: {:?}", dur);
                            wait_duration(dur);
                            continue;
                        }
                        TaskStatus::Init => {
                            tracing::debug!("Continuation returned Init");
                            continue;
                        }
                        TaskStatus::Ignore => {
                            tracing::debug!("Continuation returned Ignore");
                            continue;
                        }
                        TaskStatus::Spawn(_) => {
                            tracing::debug!("Continuation returned Spawn");
                            continue;
                        }
                    }
                }
            }
            ClientRequestState::IntroReady(_, None) => {
                tracing::error!("body() called but continuation already consumed");
                Err(HttpClientError::FailedWith(
                    "body already consumed".into(),
                ))
            }
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
    /// # Examples
    ///
    /// ```ignore
    /// let response = client.get("http://example.com")?.send()?;
    /// assert_eq!(response.get_status(), Status::OK);
    /// ```
    #[tracing::instrument(skip(self))]
    pub fn send(mut self) -> Result<SimpleResponse<SendSafeBody>, HttpClientError> {
        // Get intro and headers first
        let (intro, headers) = self.introduction()?;

        // Get body
        let body = self.body()?;

        // Build complete response
        let mut response = SimpleResponse::new(intro.status, headers, body);

        // Apply middleware to response (after receiving)
        if let Some(request) = &self.original_request {
            self.middleware_chain
                .process_response(request, &mut response)?;
        }

        Ok(response)
    }

    /// Returns an iterator over response parts.
    ///
    /// WHY: Advanced users may want fine-grained control over response processing,
    /// handling each part as it arrives.
    ///
    /// WHAT: Returns iterator adapter that yields `IncomingResponseParts` as they
    /// are received from the server.
    ///
    /// HOW: Wraps internal `TaskIterator`, drives executor on each `next()` call,
    /// translates `TaskStatus` to `IncomingResponseParts`.
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
    ///
    /// # Returns
    ///
    /// Iterator yielding `Result<IncomingResponseParts, HttpClientError>`.
    ///
    /// # Errors
    ///
    /// Returns `HttpClientError::NoPool` if connection pool is not initialized.
    /// Returns `HttpClientError::NoRequestToSend` if request was already executed.
    /// Returns `HttpClientError::FailedExecution` if request execution fails.
    /// Returns `HttpClientError` variants for DNS, connection, or parsing errors.
    ///
    #[tracing::instrument(skip(self))]
    pub fn parts(
        mut self,
    ) -> Result<
        (
            impl Iterator<Item = Result<IncomingResponseParts, HttpClientError>>,
            HttpClientConnection,
        ),
        HttpClientError,
    > {
        if self.pool.is_none() {
            return Err(HttpClientError::NoPool);
        }

        if let Some(ClientRequestState::IntroReady(_, Some(_))) = &self.task_state {
            let (conn, intro, headers) = self.introduction_with_connection()?;
            let body = self.body()?;

            let items: Vec<Result<IncomingResponseParts, HttpClientError>> = vec![
                Ok(IncomingResponseParts::Intro(
                    intro.status,
                    intro.proto,
                    intro.reason,
                )),
                Ok(IncomingResponseParts::Headers(headers)),
                Ok(body.into()),
            ];

            return Ok((IncomingResponseMapper::List(items.into_iter()), conn));
        }

        if let Some(ClientRequestState::NotStarted(_)) = &self.task_state {
            // Take the prepared request to avoid cloning
            let Some(request) = self.prepared_request.take() else {
                return Err(HttpClientError::NoRequestToSend);
            };

            // Create GetHttpRequestStreamTask with pool if provided
            // which will get us the http stream which we can then
            // wrap in a ResponseReader.
            let Ok(into_incoming) = request.into_simple_incoming_request() else {
                return Err(HttpClientError::FailedExecution);
            };

            let Some(pool) = self.pool.clone() else {
                return Err(HttpClientError::NoPoolProvided);
            };

            let task = GetHttpRequestRedirectTask::new(
                into_incoming,
                pool,
                self.config.clone(),
                self.config.max_redirects,
            );

            // Spawn task via execute_task
            let iter = valtron::execute(task, None).map_err(|e| {
                HttpClientError::FailedWith(format!("Failed to spawn task: {e}").into())
            })?;

            for next_value in iter {
                match next_value {
                    Stream::Init | Stream::Ignore | Stream::Pending(_) => {}
                    Stream::Delayed(inner) => {
                        wait_duration(inner);
                    }
                    Stream::Next(inner) => match inner {
                        HttpRequestRedirectResponse::Error(err) => {
                            tracing::error!("Request failed with error: {}", err);

                            self.task_state = Some(ClientRequestState::Completed);
                            return Err(HttpClientError::FailedExecution);
                        }
                        HttpRequestRedirectResponse::Done(conn, reader, boxed_optional_intro) => {
                            if let Some(intro) = *boxed_optional_intro {
                                tracing::debug!("Received intro message already");

                                let [first, second] = intro;

                                let mut contents: Vec<
                                    Result<IncomingResponseParts, HttpClientError>,
                                > = vec![Ok(first), Ok(second)];

                                for item in reader {
                                    match item {
                                        Ok(part) => contents.push(Ok(part)),
                                        Err(err) => {
                                            tracing::error!(
                                                "Failed to read response part: {}",
                                                err
                                            );
                                            contents.push(Err(HttpClientError::FailedExecution));
                                        }
                                    }
                                }

                                return Ok((
                                    IncomingResponseMapper::List(contents.into_iter()),
                                    conn,
                                ));
                            }

                            let reader = IncomingResponseMapper::from_reader(reader);
                            return Ok((reader.into_iter(), conn));
                        }
                        HttpRequestRedirectResponse::FlushFailed(mut conn, err) => {
                            tracing::error!("Failed to flush request: {}", err);

                            if let Err(flush_err) = conn.stream_mut().flush() {
                                tracing::error!(
                                    "Failed to re-attempt flush connection: {}",
                                    flush_err
                                );
                            }

                            let items = IncomingResponseMapper::from_reader(HttpResponseReader::<
                                SimpleHttpBody,
                                RawStream,
                            >::new(
                                conn.clone_stream(),
                                SimpleHttpBody::default(),
                            ));

                            return Ok((items.into_iter(), conn));
                        }
                    },
                }
            }
        }

        self.task_state = Some(ClientRequestState::Completed);
        Err(HttpClientError::InvalidReadState)
    }

    /// Collects all response parts into a vector.
    ///
    /// WHY: Convenience method for users who want all parts but don't want to
    /// manually iterate.
    ///
    /// WHAT: Drives request to completion, collects all `IncomingResponseParts`
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
    #[tracing::instrument(skip(self))]
    pub fn collect(self) -> Result<Vec<IncomingResponseParts>, HttpClientError> {
        let (iter, _) = self.parts()?;
        iter.collect()
    }
}

impl<R: DnsResolver + 'static> Drop for ClientRequest<R> {
    fn drop(&mut self) {
        // Return stream to pool if we have one
        if let (Some(pool), Some(stream)) = (self.pool.take(), self.stream.take()) {
            pool.return_to_pool(stream);
        }
    }
}
