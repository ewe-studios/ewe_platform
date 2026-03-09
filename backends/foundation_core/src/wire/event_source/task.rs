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
//! Uses `SseParser` to parse SSE events from the connection stream.
//!
//! PHASE 1 SCOPE: Basic SSE client with `TaskIterator` pattern.
//! PHASE 2 SCOPE: Automatic reconnection with exponential backoff.
//! PHASE 3 SCOPE: Idle timeout support.

use crate::netcap::RawStream;
use crate::valtron::{BoxedSendExecutionAction, TaskIterator, TaskStatus};
use crate::wire::event_source::{EventSourceError, ParseResult, SseParser};
use crate::wire::simple_http::client::DnsResolver;
use crate::wire::simple_http::client::HttpClientConnection;
use crate::wire::simple_http::client::HttpConnectionPool;
use crate::wire::simple_http::url::Uri;
use crate::wire::simple_http::SimpleHeader;
use std::fmt::Write as FmtWrite;
use std::io::Write;
use std::sync::Arc;
use std::time::{Duration, Instant};
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
    pub headers: Vec<(SimpleHeader, String)>,
    pub last_event_id: Option<String>,
    pub idle_timeout: Option<Duration>,
}

enum EventSourceState {
    Init(EventSourceConfig),
    Connecting {
        url: Uri,
        request: String,
    },
    Reading {
        conn: HttpClientConnection,
        parser: SseParser<RawStream>,
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
    idle_timeout: Option<Duration>, // Track idle timeout configuration
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
            EventSourceError::InvalidUrl(format!("Failed to parse URL: {} - {:?}", url_str, e))
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
            state: Some(EventSourceState::Init(EventSourceConfig {
                url: url_str,
                headers: Vec::new(),
                last_event_id: None,
                idle_timeout: None,
            })),
            pool,
            last_event_id: None,
            idle_timeout: None,
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
            EventSourceError::InvalidUrl(format!("Failed to parse URL: {} - {:?}", url_str, e))
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
            state: Some(EventSourceState::Init(EventSourceConfig {
                url: url_str,
                headers: Vec::new(),
                last_event_id: None,
                idle_timeout: None,
            })),
            pool,
            last_event_id: None,
            idle_timeout: None,
        })
    }

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

    /// Get the last event ID seen.
    ///
    /// WHY: Reconnecting task needs to track last event ID for reconnection resume.
    /// WHAT: Returns reference to current last event ID.
    #[must_use]
    pub fn last_event_id(&self) -> Option<&str> {
        self.last_event_id.as_deref()
    }

    /// Set idle timeout for the SSE connection.
    ///
    /// WHY: Long-lived SSE connections may become stale if server stops sending events.
    /// Idle timeout triggers reconnection if no data received for specified duration.
    /// WHAT: Returns Self with idle_timeout configured.
    ///
    /// # Parameters
    ///
    /// * `timeout` - Duration of inactivity before triggering reconnection
    #[must_use]
    pub fn with_idle_timeout(mut self, timeout: Duration) -> Self {
        debug!("Setting idle timeout");
        if let Some(EventSourceState::Init(ref mut config)) = self.state {
            config.idle_timeout = Some(timeout);
        }
        self.idle_timeout = Some(timeout);
        self
    }

    /// Get the configured idle timeout.
    fn idle_timeout(&self) -> Option<Duration> {
        self.idle_timeout
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

    fn next(&mut self) -> Option<TaskStatus<Self::Ready, Self::Pending, Self::Spawner>> {
        let state = self.state.take()?;

        match state {
            EventSourceState::Init(config) => {
                debug!(state = "Init", "Preparing HTTP request");

                // Parse URL
                let url = Uri::parse(&config.url).ok()?;

                // Build request line and headers manually
                let path = url.path();
                let query = url.query();
                let path_query = match query {
                    Some(q) => format!("{path}?{q}"),
                    None => path.to_string(),
                };

                let host = url.host_str().map(|s| s.to_string()).unwrap_or_default();

                // Build HTTP/1.1 GET request
                let mut request = String::new();
                write!(&mut request, "GET {path_query} HTTP/1.1\r\n").unwrap();
                write!(&mut request, "Host: {host}\r\n").unwrap();
                write!(&mut request, "Accept: text/event-stream\r\n").unwrap();
                write!(&mut request, "Cache-Control: no-cache\r\n").unwrap();

                // Add custom headers
                for (name, value) in &config.headers {
                    write!(&mut request, "{name}: {value}\r\n").unwrap();
                }

                // Add Last-Event-ID if present
                if let Some(last_id) = &config.last_event_id {
                    write!(&mut request, "Last-Event-ID: {last_id}\r\n").unwrap();
                }

                write!(&mut request, "\r\n").unwrap();

                // Store last_event_id from config for initial connection
                if let Some(id) = config.last_event_id {
                    self.last_event_id = Some(id);
                }

                // Store idle_timeout from config for initial connection
                if let Some(timeout) = config.idle_timeout {
                    self.idle_timeout = Some(timeout);
                }

                // Transition to Connecting state
                self.state = Some(EventSourceState::Connecting { url, request });
                Some(TaskStatus::Pending(EventSourceProgress::Connecting))
            }

            EventSourceState::Connecting { url, request } => {
                debug!(state = "Connecting", host = %url.host_str().unwrap_or_else(|| "unknown".to_string()), "Establishing connection via pool");

                // Use HttpConnectionPool to establish connection (handles DNS + TLS)
                let Ok(connection) = self.pool.create_http_connection(&url, None) else {
                    error!("Failed to establish HTTP connection");
                    self.state = Some(EventSourceState::Closed(EventSourceCloseReason::ConnectionError));
                    return None;
                };

                debug!(
                    state = "Connecting",
                    "Connection established, sending request"
                );

                // Clone the stream for the parser (keeps connection handle for pool return)
                let stream = connection.clone_stream();

                // Write HTTP request through the cloned stream
                let mut stream_writer = stream.clone();
                let _ = stream_writer.write_all(request.as_bytes());
                let _ = stream_writer.flush();

                debug!(state = "Reading", "Request sent, streaming events");

                // Create parser from cloned stream
                let parser = SseParser::new(stream);

                self.state = Some(EventSourceState::Reading {
                    conn: connection,
                    parser,
                    last_activity: Instant::now(),
                });
                Some(TaskStatus::Pending(EventSourceProgress::Reading))
            }

            EventSourceState::Reading {
                mut parser,
                last_activity,
                conn,
            } => {
                // Check for idle timeout
                if let Some(timeout) = self.idle_timeout() {
                    if last_activity.elapsed() > timeout {
                        warn!(
                            elapsed_secs = ?last_activity.elapsed().as_secs(),
                            timeout_secs = ?timeout.as_secs(),
                            "Idle timeout exceeded"
                        );
                        // Idle timeout exceeded - close connection for reconnection
                        self.state = Some(EventSourceState::Closed(EventSourceCloseReason::IdleTimeout));
                        return None;
                    }
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
                        self.state = Some(EventSourceState::Closed(EventSourceCloseReason::ParseError));
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

            EventSourceState::Closed(reason) => {
                trace!(state = "Closed", reason = ?reason, "Task complete");
                None
            }
        }
    }
}
