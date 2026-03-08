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

use crate::io::ioutils::SharedByteBufferStream;
use crate::netcap::RawStream;
use crate::valtron::{BoxedSendExecutionAction, TaskIterator, TaskStatus};
use crate::wire::event_source::{EventSourceError, ParseResult, SseParser};
use crate::wire::simple_http::client::DnsResolver;
use crate::wire::simple_http::SimpleHeader;
use std::fmt::Write as FmtWrite;
use std::io::Write;

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
}

enum EventSourceState {
    Init(EventSourceConfig),
    Resolving(ResolvingState), // DNS lookup in progress
    Connecting, // TCP/TLS handshake in progress (intermediate state for observability)
    Reading(SseParser<RawStream>),
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
    pub fn connect(resolver: R, url: impl Into<String>) -> Result<Self, EventSourceError> {
        let url_str = url.into();

        // Validate URL upfront - must be a valid URI with http/https scheme
        let uri = crate::wire::simple_http::url::Uri::parse(&url_str).map_err(|e| {
            EventSourceError::InvalidUrl(format!("Failed to parse URL: {} - {:?}", url_str, e))
        })?;

        // Check scheme is http or https using Scheme methods
        if !uri.scheme().is_http() && !uri.scheme().is_https() {
            return Err(EventSourceError::InvalidUrl(format!(
                "Unsupported scheme: {}. Only http:// and https:// are supported.",
                uri.scheme()
            )));
        }

        Ok(Self {
            state: Some(EventSourceState::Init(EventSourceConfig {
                url: url_str,
                headers: Vec::new(),
                last_event_id: None,
            })),
            resolver,
            last_event_id: None,
        })
    }

    #[must_use]
    pub fn with_header(mut self, name: SimpleHeader, value: impl Into<String>) -> Self {
        if let Some(EventSourceState::Init(ref mut config)) = self.state {
            config.headers.push((name, value.into()));
        }
        self
    }

    #[must_use]
    pub fn with_last_event_id(mut self, last_event_id: impl Into<String>) -> Self {
        let id_string = last_event_id.into();
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

                // Transition to Resolving state - DNS lookup starting
                self.state = Some(EventSourceState::Resolving(ResolvingState {
                    request,
                    host,
                    port,
                }));
                Some(TaskStatus::Pending(EventSourceProgress::Resolving))
            }

            EventSourceState::Resolving(resolving) => {
                // DNS resolution
                let Ok(addrs) = self.resolver.resolve(&resolving.host, resolving.port) else {
                    // DNS failed - transition to Connecting then Closed for observability
                    self.state = Some(EventSourceState::Connecting);
                    return Some(TaskStatus::Pending(EventSourceProgress::Connecting));
                };

                // Use first resolved address
                let Some(addr) = addrs.first() else {
                    self.state = Some(EventSourceState::Connecting);
                    return Some(TaskStatus::Pending(EventSourceProgress::Connecting));
                };

                // Create connection (no timeout for simplicity)
                let Ok(connection) = crate::netcap::Connection::without_timeout(*addr) else {
                    // Connection failed - transition to Connecting state first for observability
                    self.state = Some(EventSourceState::Connecting);
                    return Some(TaskStatus::Pending(EventSourceProgress::Connecting));
                };

                // Convert Connection to RawStream
                let Ok(mut raw_stream) = crate::netcap::RawStream::from_connection(connection)
                else {
                    self.state = Some(EventSourceState::Connecting);
                    return Some(TaskStatus::Pending(EventSourceProgress::Connecting));
                };

                // Write HTTP request
                let _ = raw_stream.write_all(resolving.request.as_bytes());
                let _ = raw_stream.flush();

                // Wrap RawStream with SharedByteBufferStream for buffered reading
                let buffer = SharedByteBufferStream::rwrite(raw_stream);
                let parser = SseParser::new(buffer);

                self.state = Some(EventSourceState::Reading(parser));
                Some(TaskStatus::Pending(EventSourceProgress::Reading))
            }

            EventSourceState::Connecting => {
                // Connection failed, now transition to Closed
                // This state provides observability - caller sees Pending before None
                self.state = Some(EventSourceState::Closed);
                None
            }

            EventSourceState::Reading(mut parser) => {
                match parser.next() {
                    Some(Ok(parse_result)) => {
                        // Track last event ID from ParseResult
                        if parse_result.last_known_id.is_some() {
                            self.last_event_id = parse_result.last_known_id.clone();
                        }
                        self.state = Some(EventSourceState::Reading(parser));
                        Some(TaskStatus::Ready(parse_result))
                    }
                    Some(Err(_)) => {
                        // I/O or parse error - close the connection
                        self.state = Some(EventSourceState::Closed);
                        None
                    }
                    None => {
                        // EOF - stream exhausted
                        self.state = Some(EventSourceState::Closed);
                        None
                    }
                }
            }

            EventSourceState::Closed => None,
        }
    }
}
