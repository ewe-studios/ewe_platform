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
//! Uses `SseParser` to parse SSE events directly from the connection stream.
//!
//! PHASE 1 SCOPE: Basic SSE client with `TaskIterator` pattern.
//! PHASE 2 SCOPE: Automatic reconnection with exponential backoff.
//! PHASE 3 SCOPE: Idle timeout support.

use crate::io::ioutils::SharedByteBufferStream;
use crate::netcap::RawStream;
use crate::valtron::{BoxedSendExecutionAction, TaskIterator, TaskStatus};
use crate::wire::event_source::{EventSourceError, ParseResult, SseParser};
use crate::wire::simple_http::client::DnsResolver;
use crate::wire::simple_http::SimpleHeader;
use std::fmt::Write as FmtWrite;
use std::io::Write;
use std::time::{Duration, Instant};
use tracing::{debug, error, info, instrument, trace, warn};

/// [`EventSourceProgress`] indicates the current state of SSE connection.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum EventSourceProgress {
    Resolving,
    Connecting,
    Reading,
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
    Resolving(ResolvingState), // DNS lookup in progress
    Connecting, // TCP/TLS handshake in progress (intermediate state for observability)
    Reading {
        parser: SseParser<RawStream>,
        last_activity: Instant,
    },
    Closed,
}

/// State data during DNS resolution
struct ResolvingState {
    request: String,
    host: String,
    port: u16,
}

pub struct EventSourceTask<R>
where
    R: DnsResolver + Send + 'static,
{
    state: Option<EventSourceState>,
    resolver: R,
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
        let uri = crate::wire::simple_http::url::Uri::parse(&url_str).map_err(|e| {
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
            resolver,
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
                debug!(state = "Init", "Resolving DNS");

                // Parse URL
                let url = crate::wire::simple_http::url::Uri::parse(&config.url).ok()?;

                // Build request line and headers manually
                let path = url.path();
                let query = url.query();
                let path_query = match query {
                    Some(q) => format!("{path}?{q}"),
                    None => path.to_string(),
                };

                let host = url.host_str().map(|s| s.to_string()).unwrap_or_default();
                let port = url.port_or_default();

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

                // Transition to Resolving state - DNS lookup starting
                self.state = Some(EventSourceState::Resolving(ResolvingState {
                    request,
                    host,
                    port,
                }));
                Some(TaskStatus::Pending(EventSourceProgress::Resolving))
            }

            EventSourceState::Resolving(resolving) => {
                debug!(state = "Resolving", host = %resolving.host, port = resolving.port, "DNS resolution");

                // DNS resolution
                let Ok(addrs) = self.resolver.resolve(&resolving.host, resolving.port) else {
                    error!(host = %resolving.host, "DNS resolution failed");
                    // DNS failed - transition to Connecting then Closed for observability
                    self.state = Some(EventSourceState::Connecting);
                    return Some(TaskStatus::Pending(EventSourceProgress::Connecting));
                };

                // Use first resolved address
                let Some(addr) = addrs.first() else {
                    error!(host = %resolving.host, "No addresses resolved");
                    self.state = Some(EventSourceState::Connecting);
                    return Some(TaskStatus::Pending(EventSourceProgress::Connecting));
                };

                debug!(state = "Resolving", addr = ?addr, "DNS resolved, connecting");

                // Create connection (no timeout for simplicity)
                let Ok(connection) = crate::netcap::Connection::without_timeout(*addr) else {
                    error!(addr = ?addr, "Connection failed");
                    // Connection failed - transition to Connecting state first for observability
                    self.state = Some(EventSourceState::Connecting);
                    return Some(TaskStatus::Pending(EventSourceProgress::Connecting));
                };

                // Convert Connection to RawStream
                let Ok(mut raw_stream) = crate::netcap::RawStream::from_connection(connection)
                else {
                    error!("Failed to create RawStream");
                    self.state = Some(EventSourceState::Connecting);
                    return Some(TaskStatus::Pending(EventSourceProgress::Connecting));
                };

                // Write HTTP request
                let _ = raw_stream.write_all(resolving.request.as_bytes());
                let _ = raw_stream.flush();

                debug!(state = "Connecting", "Request sent, reading response");

                // Wrap RawStream with SharedByteBufferStream for buffered reading
                let buffer = SharedByteBufferStream::rwrite(raw_stream);
                let parser = SseParser::new(buffer);

                self.state = Some(EventSourceState::Reading {
                    parser,
                    last_activity: Instant::now(),
                });
                debug!(state = "Reading", "Connected, streaming events");
                Some(TaskStatus::Pending(EventSourceProgress::Reading))
            }

            EventSourceState::Connecting => {
                // Connection failed, now transition to Closed
                // This state provides observability - caller sees Pending before None
                debug!(state = "Connecting", "Connection failed, closing");
                self.state = Some(EventSourceState::Closed);
                None
            }

            EventSourceState::Reading {
                mut parser,
                last_activity,
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
                        self.state = Some(EventSourceState::Closed);
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
                            parser,
                            last_activity: Instant::now(),
                        });
                        Some(TaskStatus::Ready(parse_result))
                    }
                    Some(Err(e)) => {
                        error!(error = ?e, "SSE parse error");
                        // I/O or parse error - close the connection
                        self.state = Some(EventSourceState::Closed);
                        None
                    }
                    None => {
                        debug!(state = "Reading", "Stream EOF");
                        // EOF - stream exhausted
                        self.state = Some(EventSourceState::Closed);
                        None
                    }
                }
            }

            EventSourceState::Closed => {
                trace!(state = "Closed", "Task complete");
                None
            }
        }
    }
}
