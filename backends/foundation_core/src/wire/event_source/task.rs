//! SSE client [`TaskIterator`](crate::valtron::TaskIterator) implementation.
//!
//! WHY: Clients need a non-blocking, state-machine-based SSE consumer that
//! integrates with the valtron executor system. Enables async-like event handling
//! without async/await.
//!
//! WHAT: Implements [`EventSourceTask`] which processes SSE connections through a series
//! of states (connecting, reading events). Uses `TaskIterator` trait to yield
//! `TaskStatus` variants for each SSE event.
//!
//! HOW: State machine where each `next()` call advances through states.
//! Uses `HttpConnectionPool` for connection management with pooling support.
//! Uses `HttpResponseReader` to parse HTTP response headers before SSE parsing,
//! ensuring HTTP headers are not incorrectly parsed as SSE events.
//!
//! PHASE 1 SCOPE: Basic SSE client with `TaskIterator` pattern.
//! PHASE 2 SCOPE: Automatic reconnection with exponential backoff.
//! PHASE 3 SCOPE: Idle timeout support.

use crate::extensions::result_ext::SendableBoxedError;
use crate::netcap::RawStream;
use crate::valtron::{BoxedSendExecutionAction, TaskIterator, TaskStatus};
use crate::wire::event_source::{EventSourceError, ParseResult, SseParser};
use crate::wire::simple_http::client::DnsResolver;
use crate::wire::simple_http::client::HttpClientConnection;
use crate::wire::simple_http::client::HttpConnectionPool;
use crate::wire::simple_http::timeout::{TimeoutCalculator, TimeoutContext};
use crate::wire::simple_http::url::Uri;
use crate::wire::simple_http::{
    Http11, HttpSendResponseReader, IncomingResponseParts, RenderHttp, SendSafeBody, SimpleHeader,
    SimpleHttpBody, SimpleIncomingRequest, SimpleMethod, Status,
};
use std::io::Write;
use std::sync::Arc;
use std::time::Instant;
use tracing::{debug, error, info, instrument, trace, warn};

/// [`EventSourceProgress`] indicates the current state of SSE connection.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum EventSourceProgress {
    Connecting,
    Reading,
}

/// [`EventSourceCloseReason`] indicates why an SSE connection was closed.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum EventSourceCloseReason {
    /// Server closed the connection normally (EOF).
    Eof,
    /// Parse error occurred.
    ParseError,
    /// Idle timeout exceeded.
    IdleTimeout,
    /// Connection error.
    ConnectionError,
}

/// [`EventSourceConfig`] holds the configuration for an SSE connection.
pub struct EventSourceConfig {
    pub url: String,
    pub method: SimpleMethod,
    pub headers: Vec<(SimpleHeader, String)>,
    pub body: Option<SendSafeBody>,
    pub last_event_id: Option<String>,
    pub timeout_calculator: TimeoutCalculator,
}

enum EventSourceState {
    Init(Box<EventSourceConfig>),
    Connecting {
        url: Uri,
        request: Box<SimpleIncomingRequest>,
    },
    /// Waiting for HTTP response headers to be parsed.
    /// This state ensures we properly parse HTTP response before SSE parsing.
    AwaitingHeaders {
        conn: HttpClientConnection,
        reader: HttpSendResponseReader<SimpleHttpBody, RawStream>,
    },
    #[allow(dead_code)] // Reading state is reserved for future use
    Reading {
        conn: HttpClientConnection,
        parser: SseParser<RawStream>,
        last_activity: Instant,
    },
    /// Reading from SSE stream iterator (when body was returned as SseStream).
    ReadingStream {
        conn: HttpClientConnection,
        iterator: Box<dyn Iterator<Item = Result<ParseResult, SendableBoxedError>> + Send>,
        last_activity: Instant,
    },
    Closed(EventSourceCloseReason),
}

pub struct EventSourceTask<R>
where
    R: DnsResolver + Send + 'static,
{
    state: Option<EventSourceState>,
    pool: Arc<HttpConnectionPool<R>>,
    last_event_id: Option<String>, // Track last event ID for reconnection
    timeout_calculator: TimeoutCalculator, // Timeout calculator for dynamic timeouts
}

impl<R> EventSourceTask<R>
where
    R: DnsResolver + Send + 'static,
{
    /// Connect to an SSE endpoint.
    ///
    /// # Errors
    ///
    /// Returns [`EventSourceError`] if the URL is invalid.
    #[instrument(skip(resolver, url), err)]
    pub fn connect(resolver: R, url: impl Into<String>) -> Result<Self, EventSourceError> {
        let url_str = url.into();
        info!(url = %url_str, "Connecting to SSE endpoint");

        // Validate URL upfront - must be a valid URI with http/https scheme
        let uri = Uri::parse(&url_str).map_err(|e| {
            error!(url = %url_str, error = ?e, "Failed to parse URL");
            EventSourceError::InvalidUrl(format!("Failed to parse URL: {url_str} - {e:?}"))
        })?;

        // Check scheme is http or https using Scheme methods
        if !uri.scheme().is_http() && !uri.scheme().is_https() {
            return Err(EventSourceError::InvalidUrl(format!(
                "Unsupported scheme: {}. Only http:// and https:// are supported.",
                uri.scheme()
            )));
        }

        debug!(scheme = ?uri.scheme(), host = ?uri.host_str(), "URL validated");

        let pool = Arc::new(HttpConnectionPool::new(
            crate::wire::simple_http::client::ConnectionPool::default(),
            resolver,
        ));

        Ok(Self {
            state: Some(EventSourceState::Init(Box::new(EventSourceConfig {
                url: url_str,
                method: SimpleMethod::GET,
                headers: Vec::new(),
                body: None,
                last_event_id: None,
                timeout_calculator: TimeoutCalculator::default(),
            }))),
            pool,
            last_event_id: None,
            timeout_calculator: TimeoutCalculator::default(),
        })
    }

    /// Connect to an SSE endpoint using an existing connection pool.
    ///
    /// WHY: Allows reuse of existing pool for connection pooling across multiple SSE connections.
    ///
    /// # Errors
    ///
    /// Returns [`EventSourceError`] if the URL is invalid.
    #[instrument(skip(pool, url), err)]
    pub fn connect_with_pool(
        url: impl Into<String>,
        pool: Arc<HttpConnectionPool<R>>,
    ) -> Result<Self, EventSourceError> {
        let url_str = url.into();
        info!(url = %url_str, "Connecting to SSE endpoint with pool");

        // Validate URL upfront - must be a valid URI with http/https scheme
        let uri = Uri::parse(&url_str).map_err(|e| {
            error!(url = %url_str, error = ?e, "Failed to parse URL");
            EventSourceError::InvalidUrl(format!("Failed to parse URL: {url_str} - {e:?}"))
        })?;

        // Check scheme is http or https using Scheme methods
        if !uri.scheme().is_http() && !uri.scheme().is_https() {
            return Err(EventSourceError::InvalidUrl(format!(
                "Unsupported scheme: {}. Only http:// and https:// are supported.",
                uri.scheme()
            )));
        }

        debug!(scheme = ?uri.scheme(), host = ?uri.host_str(), "URL validated");

        Ok(Self {
            state: Some(EventSourceState::Init(Box::new(EventSourceConfig {
                url: url_str,
                method: SimpleMethod::GET,
                headers: Vec::new(),
                body: None,
                last_event_id: None,
                timeout_calculator: TimeoutCalculator::default(),
            }))),
            pool,
            last_event_id: None,
            timeout_calculator: TimeoutCalculator::default(),
        })
    }

    /// Add a custom header to the request.
    #[must_use]
    pub fn with_header(mut self, name: SimpleHeader, value: impl Into<String>) -> Self {
        debug!("Adding custom header");
        if let Some(EventSourceState::Init(ref mut config)) = self.state {
            config.headers.push((name, value.into()));
        }
        self
    }

    #[must_use]
    pub fn with_last_event_id(mut self, last_event_id: impl Into<String>) -> Self {
        let id_string = last_event_id.into();
        debug!(last_event_id = %id_string, "Setting Last-Event-ID");
        if let Some(EventSourceState::Init(ref mut config)) = self.state {
            config.last_event_id = Some(id_string.clone());
        }
        self.last_event_id = Some(id_string);
        self
    }

    /// Set the request body.
    ///
    /// WHY: Some SSE endpoints (e.g. OpenAI chat completions) require POST with a JSON body.
    /// WHAT: Returns Self with the body configured. The method switches from GET to POST.
    #[must_use]
    pub fn with_body(mut self, body: SendSafeBody) -> Self {
        debug!("Setting request body");
        if let Some(EventSourceState::Init(ref mut config)) = self.state {
            config.method = SimpleMethod::POST;
            config.body = Some(body);
        }
        self
    }

    /// Set the HTTP method for the request.
    ///
    /// WHY: The reconnecting task needs to preserve the method across reconnections.
    #[must_use]
    pub fn with_method(mut self, method: SimpleMethod) -> Self {
        debug!(?method, "Setting request method");
        if let Some(EventSourceState::Init(ref mut config)) = self.state {
            config.method = method;
        }
        self
    }

    /// Get the last event ID seen.
    ///
    /// WHY: Reconnecting task needs to track last event ID for reconnection resume.
    /// WHAT: Returns reference to current last event ID.
    #[must_use]
    pub fn last_event_id(&self) -> Option<&str> {
        self.last_event_id.as_deref()
    }

    /// Set the timeout calculator for dynamic timeout configuration.
    ///
    /// WHY: SSE connections need configurable timeouts for idle detection and reconnection.
    /// The TimeoutCalculator provides dynamic timeout calculation based on context.
    /// WHAT: Returns Self with `timeout_calculator` configured.
    ///
    /// # Parameters
    ///
    /// * `calculator` - TimeoutCalculator for dynamic timeout computation
    #[must_use]
    pub fn with_timeout_calculator(mut self, calculator: TimeoutCalculator) -> Self {
        debug!("Setting timeout calculator");
        if let Some(EventSourceState::Init(ref mut config)) = self.state {
            config.timeout_calculator = calculator.clone();
        }
        self.timeout_calculator = calculator;
        self
    }

    /// Get the configured timeout calculator.
    #[must_use]
    pub fn timeout_calculator(&self) -> &TimeoutCalculator {
        &self.timeout_calculator
    }

    /// Get the close reason if the task is closed.
    ///
    /// WHY: Reconnecting task needs to know if closure was legitimate EOF or error.
    /// WHAT: Returns close reason if task is in Closed state.
    #[must_use]
    pub fn close_reason(&self) -> Option<EventSourceCloseReason> {
        match &self.state {
            Some(EventSourceState::Closed(reason)) => Some(*reason),
            _ => None,
        }
    }
}

impl<R> TaskIterator for EventSourceTask<R>
where
    R: DnsResolver + Send + 'static,
{
    type Ready = ParseResult;
    type Pending = EventSourceProgress;
    type Spawner = BoxedSendExecutionAction;

    fn next_status(&mut self) -> Option<TaskStatus<Self::Ready, Self::Pending, Self::Spawner>> {
        let state = self.state.take()?;

        match state {
            EventSourceState::Init(config) => {
                debug!(state = "Init", "Preparing HTTP request");

                let url = Uri::parse(&config.url).ok()?;

                // Build the full request using SimpleIncomingRequestBuilder
                let mut builder = SimpleIncomingRequest::builder()
                    .with_uri(url.clone())
                    .with_parsed_url(&config.url);

                if let Some(body) = config.body {
                    builder = builder.with_body(body);
                }

                builder = builder.with_method(config.method);

                // Add Accept and Cache-Control headers
                builder = builder.add_header_raw(SimpleHeader::ACCEPT, "text/event-stream");
                builder = builder.add_header_raw(SimpleHeader::CACHE_CONTROL, "no-cache");

                // Add custom headers
                for (name, value) in &config.headers {
                    builder = builder.add_header_raw(name.clone(), value);
                }

                // Add Last-Event-ID if present
                if let Some(ref last_id) = config.last_event_id {
                    builder = builder.add_header_raw("Last-Event-ID".to_string(), last_id);
                }

                let request = builder.build().ok()?;

                // Store last_event_id from config for initial connection
                if let Some(id) = config.last_event_id {
                    self.last_event_id = Some(id);
                }

                // Store timeout_calculator from config for initial connection
                self.timeout_calculator = config.timeout_calculator;

                // Transition to Connecting state
                self.state = Some(EventSourceState::Connecting {
                    url,
                    request: Box::new(request),
                });
                Some(TaskStatus::Pending(EventSourceProgress::Connecting))
            }

            EventSourceState::Connecting { url, request } => {
                debug!(state = "Connecting", host = %url.host_str().unwrap_or_else(|| "unknown".to_string()), "Establishing connection via pool");

                // Use HttpConnectionPool to establish connection (handles DNS + TLS)
                let Ok(connection) = self.pool.create_http_connection(&url, None) else {
                    error!("Failed to establish HTTP connection");
                    self.state = Some(EventSourceState::Closed(
                        EventSourceCloseReason::ConnectionError,
                    ));
                    return None;
                };

                debug!(
                    state = "Connecting",
                    "Connection established, sending request"
                );

                // Clone the stream for response reading (keeps connection handle for pool return)
                // clone_stream() returns SharedByteBufferStream<RawStream>
                let stream = connection.clone_stream();

                // Render full HTTP request (headers + body) and write to socket
                let mut stream_writer = stream.clone();
                let _ = Http11::Request(*request).http_render_to_writer(&mut stream_writer);
                let _ = stream_writer.flush();

                debug!(state = "Connecting", "Request sent, awaiting HTTP response");

                // Create HttpResponseReader to parse HTTP response headers FIRST
                // This ensures HTTP headers are not parsed as SSE events
                // stream is already SharedByteBufferStream<RawStream>, so we use new() directly
                let reader = HttpSendResponseReader::from(
                    crate::wire::simple_http::HttpResponseReader::new(
                        stream,
                        SimpleHttpBody::default(),
                    ),
                );

                self.state = Some(EventSourceState::AwaitingHeaders {
                    conn: connection,
                    reader,
                });
                Some(TaskStatus::Pending(EventSourceProgress::Connecting))
            }

            EventSourceState::AwaitingHeaders { conn, mut reader } => {
                debug!(state = "AwaitingHeaders", "Reading HTTP response");

                // Read through the response to get past headers
                // This ensures HTTP headers are parsed and not treated as SSE events
                let mut status: Option<Status> = None;
                let mut headers_validated = false;

                loop {
                    match reader.next() {
                        Some(Ok(IncomingResponseParts::Intro(s, _, _))) => {
                            debug!(status = ?s, "Got HTTP status");

                            // Verify status code is 200 OK
                            if s != Status::OK {
                                error!(status = ?s, "Unexpected HTTP status code");
                                self.state = Some(EventSourceState::Closed(
                                    EventSourceCloseReason::ConnectionError,
                                ));
                                return None;
                            }
                            status = Some(s);
                        }
                        Some(Ok(IncomingResponseParts::Headers(h))) => {
                            debug!(headers = ?h, "Got HTTP headers");

                            // Verify Content-Type is text/event-stream
                            let content_type = h
                                .get(&SimpleHeader::CONTENT_TYPE)
                                .and_then(|v| v.first())
                                .map(|s| s.as_str());

                            match content_type {
                                Some(ct) if ct.contains("text/event-stream") => {
                                    debug!(content_type = %ct, "Content-Type is valid for SSE");
                                    headers_validated = true;
                                }
                                Some(ct) => {
                                    error!(content_type = %ct, "Invalid Content-Type for SSE");
                                    self.state = Some(EventSourceState::Closed(
                                        EventSourceCloseReason::ConnectionError,
                                    ));
                                    return None;
                                }
                                None => {
                                    error!("Missing Content-Type header");
                                    self.state = Some(EventSourceState::Closed(
                                        EventSourceCloseReason::ConnectionError,
                                    ));
                                    return None;
                                }
                            }
                        }
                        Some(Ok(IncomingResponseParts::StreamedBody(SendSafeBody::SseStream(
                            opt_iter,
                        )))) => {
                            debug!("Got SSE stream body");
                            // Extract the iterator from SseStream
                            match opt_iter {
                                Some(iterator) => {
                                    // Transition to ReadingStream state with the iterator
                                    self.state = Some(EventSourceState::ReadingStream {
                                        conn,
                                        iterator,
                                        last_activity: Instant::now(),
                                    });
                                    return Some(TaskStatus::Pending(EventSourceProgress::Reading));
                                }
                                None => {
                                    error!("SseStream iterator is None");
                                    self.state = Some(EventSourceState::Closed(
                                        EventSourceCloseReason::ConnectionError,
                                    ));
                                    return None;
                                }
                            }
                        }
                        Some(Ok(IncomingResponseParts::SizedBody(_))) => {
                            error!("Expected streamed SSE body, got sized body");
                            self.state = Some(EventSourceState::Closed(
                                EventSourceCloseReason::ConnectionError,
                            ));
                            return None;
                        }
                        Some(Ok(IncomingResponseParts::NoBody)) => {
                            error!("Response has no body");
                            self.state = Some(EventSourceState::Closed(
                                EventSourceCloseReason::ConnectionError,
                            ));
                            return None;
                        }
                        Some(Err(e)) => {
                            error!(error = ?e, "Failed to read HTTP response");
                            self.state = Some(EventSourceState::Closed(
                                EventSourceCloseReason::ConnectionError,
                            ));
                            return None;
                        }
                        None => {
                            if !headers_validated {
                                error!("HTTP response ended before headers");
                                self.state = Some(EventSourceState::Closed(
                                    EventSourceCloseReason::ConnectionError,
                                ));
                                return None;
                            }
                            // End of response, but this shouldn't happen for SSE
                            break;
                        }
                        _ => {
                            // Skip unknown parts
                            continue;
                        }
                    }
                }

                // Ensure we got a valid status and headers
                if status.is_none() || !headers_validated {
                    error!("Incomplete HTTP response");
                    self.state = Some(EventSourceState::Closed(
                        EventSourceCloseReason::ConnectionError,
                    ));
                    return None;
                }

                // We should have transitioned to ReadingStream above
                // If we're here, something went wrong
                error!("Failed to transition to ReadingStream state");
                self.state = Some(EventSourceState::Closed(
                    EventSourceCloseReason::ConnectionError,
                ));
                None
            }

            EventSourceState::Reading {
                mut parser,
                last_activity,
                conn,
            } => {
                // Calculate idle timeout from calculator for streaming context
                let ctx = TimeoutContext::default().streaming();
                let idle_timeout = self.timeout_calculator.calculate_read_timeout(&ctx);

                if last_activity.elapsed() > idle_timeout {
                    warn!(
                        elapsed_secs = ?last_activity.elapsed().as_secs(),
                        timeout_secs = ?idle_timeout.as_secs(),
                        "Idle timeout exceeded"
                    );
                    // Idle timeout exceeded - close connection for reconnection
                    self.state = Some(EventSourceState::Closed(
                        EventSourceCloseReason::IdleTimeout,
                    ));
                    return None;
                }

                trace!(state = "Reading", "Polling for SSE events");

                match parser.next() {
                    Some(Ok(parse_result)) => {
                        // Track last event ID from ParseResult
                        if parse_result.last_known_id.is_some() {
                            self.last_event_id = parse_result.last_known_id.clone();
                        }
                        // Reset activity timestamp on successful event
                        self.state = Some(EventSourceState::Reading {
                            conn,
                            parser,
                            last_activity: Instant::now(),
                        });
                        Some(TaskStatus::Ready(parse_result))
                    }
                    Some(Err(e)) => {
                        error!(error = ?e, "SSE parse error");
                        // I/O or parse error - close the connection
                        self.state =
                            Some(EventSourceState::Closed(EventSourceCloseReason::ParseError));
                        None
                    }
                    None => {
                        debug!(state = "Reading", "Stream EOF");
                        // EOF - stream exhausted
                        self.state = Some(EventSourceState::Closed(EventSourceCloseReason::Eof));
                        None
                    }
                }
            }

            // EventSourceState::ReadingIter variant removed - we now use SseParser directly
            // which provides the same Iterator interface
            EventSourceState::ReadingStream {
                mut iterator,
                last_activity,
                conn,
            } => {
                // Calculate idle timeout from calculator for streaming context
                let ctx = TimeoutContext::default().streaming();
                let idle_timeout = self.timeout_calculator.calculate_read_timeout(&ctx);

                if last_activity.elapsed() > idle_timeout {
                    warn!(
                        elapsed_secs = ?last_activity.elapsed().as_secs(),
                        timeout_secs = ?idle_timeout.as_secs(),
                        "Idle timeout exceeded"
                    );
                    // Idle timeout exceeded - close connection for reconnection
                    self.state = Some(EventSourceState::Closed(
                        EventSourceCloseReason::IdleTimeout,
                    ));
                    return None;
                }

                trace!(state = "ReadingStream", "Polling for SSE events");

                match iterator.next() {
                    Some(Ok(parse_result)) => {
                        // Track last event ID from ParseResult
                        if parse_result.last_known_id.is_some() {
                            self.last_event_id = parse_result.last_known_id.clone();
                        }
                        // Reset activity timestamp on successful event
                        self.state = Some(EventSourceState::ReadingStream {
                            conn,
                            iterator,
                            last_activity: Instant::now(),
                        });
                        Some(TaskStatus::Ready(parse_result))
                    }
                    Some(Err(e)) => {
                        error!(error = ?e, "SSE parse error");
                        // I/O or parse error - close the connection
                        self.state =
                            Some(EventSourceState::Closed(EventSourceCloseReason::ParseError));
                        None
                    }
                    None => {
                        debug!(state = "ReadingStream", "Stream EOF");
                        // EOF - stream exhausted
                        self.state = Some(EventSourceState::Closed(EventSourceCloseReason::Eof));
                        None
                    }
                }
            }

            EventSourceState::Closed(reason) => {
                trace!(state = "Closed", reason = ?reason, "Task complete");
                None
            }
        }
    }
}
