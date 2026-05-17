//! Simplified consumer-facing API for Server-Sent Events.
//!
//! WHY: Most users want to consume SSE streams without dealing with `TaskIterator`
//! mechanics. This module provides simple iterator-based APIs that hide the
//! underlying executor complexity while still exposing state information.
//!
//! WHAT: Provides `SseStream` - a simple iterator wrapper around `EventSourceTask`
//! that yields `Result<SseEvent, EventSourceError>` where `SseEvent` indicates
//! whether to process an event or skip (pending/delayed).
//!
//! HOW: Uses `execute_stream()` internally and maps Stream states to user-visible
//! variants.

use crate::valtron::{execute, DrivenStreamIterator, Stream};
use crate::wire::event_source::{
    Event, EventSourceError, EventSourceTask, ParseResult, ReconnectingEventSourceTask,
};
use crate::wire::simple_http::client::DnsResolver;
use crate::wire::simple_http::client::HttpConnectionPool;
use std::sync::Arc;

/// SSE event wrapper that indicates whether to process or skip.
///
/// WHY: Users need to know when the stream is still working vs. when an actual
/// event is available. This allows them to decide how to handle pending/delayed states.
///
/// WHAT: Enum with `Event` variant containing the actual event and `Skip` variant
/// for pending/delayed states.
#[derive(Debug, Clone)]
pub enum SseStreamEvent {
    /// An actual SSE event is available.
    Event(Event),
    /// Stream is still working, no event yet. User should call `next()` again.
    Skip,
}

impl From<ParseResult> for SseStreamEvent {
    fn from(result: ParseResult) -> Self {
        Self::Event(result.event)
    }
}

/// A simplified SSE event stream consumer.
///
/// WHY: Users want to consume SSE events without understanding `TaskIterator` internals.
///
/// WHAT: Wraps the executor's stream iterator and presents a simple iterator interface
/// that yields `SseEvent` to indicate when events are available vs. when to skip.
///
/// # Examples
///
/// ```ignore
/// use foundation_core::wire::event_source::{SseStream, SseStreamEvent};
/// use foundation_core::wire::simple_http::client::SystemDnsResolver;
///
/// let stream = SseStream::connect(SystemDnsResolver, "https://api.example.com/events")?;
///
/// for result in stream {
///     match result? {
///         SseStreamEvent::Event(event) => println!("Got event: {:?}", event),
///         SseStreamEvent::Skip => continue, // Still working, no event yet
///     }
/// }
/// ```
pub struct SseStream<R: DnsResolver + Send + 'static> {
    inner: DrivenStreamIterator<EventSourceTask<R>>,
}

impl<R: DnsResolver + Send + 'static> SseStream<R> {
    /// Connect to an SSE endpoint and create a stream.
    ///
    /// # Errors
    ///
    /// Returns `EventSourceError` if:
    /// - URL is invalid
    /// - Executor fails to schedule the task
    pub fn connect(resolver: R, url: impl Into<String>) -> Result<Self, EventSourceError> {
        let task = EventSourceTask::connect(resolver, url)?;
        let inner = execute(task, None)
            .map_err(|e| EventSourceError::Http(format!("Executor error: {e}")))?;
        Ok(Self { inner })
    }

    /// Connect using an existing connection pool.
    ///
    /// WHY: Allows connection pooling across multiple SSE connections.
    ///
    /// # Errors
    ///
    /// Returns `EventSourceError` if:
    /// - URL is invalid
    /// - Executor fails to schedule the task
    pub fn with_pool(
        url: impl Into<String>,
        pool: Arc<HttpConnectionPool<R>>,
    ) -> Result<Self, EventSourceError> {
        let task = EventSourceTask::connect_with_pool(url, pool)?;
        let inner = execute(task, None)
            .map_err(|e| EventSourceError::Http(format!("Executor error: {e}")))?;
        Ok(Self { inner })
    }
}

impl<R: DnsResolver + Send + 'static> Iterator for SseStream<R> {
    type Item = Result<SseStreamEvent, EventSourceError>;

    fn next(&mut self) -> Option<Self::Item> {
        match self.inner.next() {
            Some(Stream::Next(parse_result)) => Some(Ok(SseStreamEvent::Event(parse_result.event))),
            Some(Stream::Pending(_)) => Some(Ok(SseStreamEvent::Skip)),
            Some(Stream::Delayed(_)) => Some(Ok(SseStreamEvent::Skip)),
            Some(Stream::Init | Stream::Ignore) => Some(Ok(SseStreamEvent::Skip)),
            None => None,
        }
    }
}

/// Reconnecting SSE stream with automatic reconnection support.
///
/// WHY: Production SSE connections can drop. Automatic reconnection with
/// Last-Event-ID tracking is required for reliable event consumption.
///
/// WHAT: Wraps `ReconnectingEventSourceTask` with simplified iterator interface
/// that yields `SseEvent` to indicate event availability.
pub struct ReconnectingSseStream<R: DnsResolver + Clone + Send + 'static> {
    inner: DrivenStreamIterator<ReconnectingEventSourceTask<R>>,
}

impl<R: DnsResolver + Clone + Send + 'static> ReconnectingSseStream<R> {
    /// Connect to an SSE endpoint with automatic reconnection.
    ///
    /// # Errors
    ///
    /// Returns `EventSourceError` if:
    /// - URL is invalid
    /// - Executor fails to schedule the task
    pub fn connect(resolver: R, url: impl Into<String>) -> Result<Self, EventSourceError> {
        let task = ReconnectingEventSourceTask::connect(resolver, url)?;
        let inner = execute(task, None)
            .map_err(|e| EventSourceError::Http(format!("Executor error: {e}")))?;
        Ok(Self { inner })
    }
}

impl<R: DnsResolver + Clone + Send + 'static> Iterator for ReconnectingSseStream<R> {
    type Item = Result<SseStreamEvent, EventSourceError>;

    fn next(&mut self) -> Option<Self::Item> {
        match self.inner.next() {
            Some(Stream::Next(parse_result)) => Some(Ok(SseStreamEvent::Event(parse_result.event))),
            Some(Stream::Pending(_)) => Some(Ok(SseStreamEvent::Skip)),
            Some(Stream::Delayed(_)) => Some(Ok(SseStreamEvent::Skip)),
            Some(Stream::Init | Stream::Ignore) => Some(Ok(SseStreamEvent::Skip)),
            None => None,
        }
    }
}
