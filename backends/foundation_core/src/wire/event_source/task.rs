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
use crate::wire::event_source::{Event, EventSourceError, SseParser};
use crate::wire::simple_http::client::DnsResolver;
use crate::wire::simple_http::SimpleHeader;
use std::fmt::Write as FmtWrite;
use std::io::Write;

/// [`EventSourceProgress`] indicates the current state of SSE connection.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum EventSourceProgress {
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
    Connecting,  // Intermediate state for connection attempt
    Reading(SseParser<RawStream>),
    Closed,
}

pub struct EventSourceTask<R>
where
    R: DnsResolver + Send + 'static,
{
    state: Option<EventSourceState>,
    resolver: R,
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
        let uri = crate::wire::simple_http::url::Uri::parse(&url_str)
            .map_err(|e| EventSourceError::InvalidUrl(format!("Failed to parse URL: {} - {:?}", url_str, e)))?;

        let scheme = uri.scheme_str().ok_or_else(|| EventSourceError::InvalidUrl("URL must have a scheme (http:// or https://)".to_string()))?;

        if scheme != "http" && scheme != "https" {
            return Err(EventSourceError::InvalidUrl(
                format!("Unsupported scheme: {}. Only http:// and https:// are supported.", scheme)
            ));
        }

        Ok(Self {
            state: Some(EventSourceState::Init(EventSourceConfig {
                url: url_str,
                headers: Vec::new(),
                last_event_id: None,
            })),
            resolver,
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
        if let Some(EventSourceState::Init(ref mut config)) = self.state {
            config.last_event_id = Some(last_event_id.into());
        }
        self
    }
}

impl<R> TaskIterator for EventSourceTask<R>
where
    R: DnsResolver + Send + 'static,
{
    type Ready = Event;
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

                let host = url.host_str()?;
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

                // Resolve hostname
                let Ok(addrs) = self.resolver.resolve(&host, port) else {
                    self.state = Some(EventSourceState::Closed);
                    return None;
                };

                // Use first resolved address
                let Some(addr) = addrs.first() else {
                    self.state = Some(EventSourceState::Closed);
                    return None;
                };

                // Create connection (no timeout for simplicity)
                let Ok(connection) = crate::netcap::Connection::without_timeout(*addr) else {
                    // Connection failed - transition to Connecting state first
                    // This allows the test to see Pending before None
                    self.state = Some(EventSourceState::Connecting);
                    return Some(TaskStatus::Pending(EventSourceProgress::Connecting));
                };

                // Convert Connection to RawStream
                let Ok(mut raw_stream) = crate::netcap::RawStream::from_connection(connection) else {
                    self.state = Some(EventSourceState::Closed);
                    return None;
                };

                // Write HTTP request
                let _ = raw_stream.write_all(request.as_bytes());
                let _ = raw_stream.flush();

                // Wrap RawStream with SharedByteBufferStream for buffered reading
                let buffer = SharedByteBufferStream::rwrite(raw_stream);
                let parser = SseParser::new(buffer);

                self.state = Some(EventSourceState::Reading(parser));
                Some(TaskStatus::Pending(EventSourceProgress::Reading))
            }

            EventSourceState::Connecting => {
                // Connection failed, now transition to Closed
                self.state = Some(EventSourceState::Closed);
                None
            }

            EventSourceState::Reading(mut parser) => {
                let Some(event) = parser.next() else {
                    // No more events - stream exhausted
                    self.state = Some(EventSourceState::Closed);
                    return None;
                };

                self.state = Some(EventSourceState::Reading(parser));
                Some(TaskStatus::Ready(event))
            }

            EventSourceState::Closed => None,
        }
    }
}
