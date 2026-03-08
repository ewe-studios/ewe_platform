//! Reconnecting SSE client [`TaskIterator`](crate::valtron::TaskIterator) implementation.
//!
//! WHY: SSE connections can drop at any time. Clients need automatic reconnection
//! with exponential backoff and Last-Event-ID resume to avoid missing events.
//!
//! WHAT: Wraps [`EventSourceTask`] with reconnection logic. When the inner task
//! closes (EOF or error), transitions to a backoff state and creates a new
//! inner task with the tracked Last-Event-ID.
//!
//! HOW: State machine: Connected → Waiting → Reconnecting → Connected (loop).
//! Uses [`ExponentialBackoffDecider`] for backoff timing. Tracks `last_event_id`
//! from received events. Respects server `retry:` field.

use crate::retries::{ExponentialBackoffDecider, RetryDecider, RetryState};
use crate::valtron::{BoxedSendExecutionAction, TaskIterator, TaskStatus};
use crate::wire::event_source::{Event, EventSourceProgress, EventSourceTask};
use crate::wire::simple_http::client::DnsResolver;
use crate::wire::simple_http::SimpleHeader;
use std::time::Duration;

/// Configuration for reconnecting SSE client.
pub struct ReconnectingConfig {
    url: String,
    headers: Vec<(SimpleHeader, String)>,
    max_retries: u32,
    server_retry: Option<Duration>,
}

impl ReconnectingConfig {
    fn new(url: String) -> Self {
        Self {
            url,
            headers: Vec::new(),
            max_retries: 5,
            server_retry: None,
        }
    }
}

/// Progress state for reconnecting SSE task.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ReconnectingProgress {
    /// Initial connection or reconnection in progress.
    Connecting,
    /// Reading events from active stream.
    Reading,
    /// Waiting for backoff before reconnection.
    Reconnecting,
}

enum ReconnectingState<R: DnsResolver + Send + 'static> {
    /// Active connection with inner task.
    Connected(EventSourceTask<R>),
    /// Backoff wait before reconnection.
    Waiting(Duration),
    /// Creating new connection after backoff.
    Reconnecting,
    /// Max retries exhausted or unrecoverable error.
    Exhausted,
}

/// [`ReconnectingEventSourceTask`] wraps [`EventSourceTask`] with automatic
/// reconnection using exponential backoff.
///
/// WHY: SSE connections are long-lived and can drop. Automatic reconnection
/// with Last-Event-ID resume is required by the W3C SSE specification.
///
/// WHAT: [`TaskIterator`] that transparently reconnects on connection loss,
/// preserving Last-Event-ID across reconnections.
pub struct ReconnectingEventSourceTask<R: DnsResolver + Clone + Send + 'static> {
    state: Option<ReconnectingState<R>>,
    config: ReconnectingConfig,
    resolver: R,
    last_event_id: Option<String>,
    retry_state: RetryState,
    backoff: ExponentialBackoffDecider,
}

impl<R> ReconnectingEventSourceTask<R>
where
    R: DnsResolver + Clone + Send + 'static,
{
    /// Create a reconnecting SSE task.
    ///
    /// # Errors
    ///
    /// Returns [`super::EventSourceError`] if the URL is invalid.
    pub fn connect(
        resolver: R,
        url: impl Into<String>,
    ) -> Result<Self, crate::wire::event_source::EventSourceError> {
        let url_str = url.into();

        // Validate URL upfront (same as EventSourceTask)
        let uri = crate::wire::simple_http::url::Uri::parse(&url_str).map_err(|e| {
            crate::wire::event_source::EventSourceError::InvalidUrl(format!(
                "Failed to parse URL: {url_str} - {e:?}"
            ))
        })?;

        if !uri.scheme().is_http() && !uri.scheme().is_https() {
            return Err(crate::wire::event_source::EventSourceError::InvalidUrl(
                format!(
                    "Unsupported scheme: {}. Only http:// and https:// are supported.",
                    uri.scheme()
                ),
            ));
        }

        let inner = EventSourceTask::connect(resolver.clone(), &url_str)?;
        let config = ReconnectingConfig::new(url_str);

        Ok(Self {
            state: Some(ReconnectingState::Connected(inner)),
            config,
            resolver,
            last_event_id: None,
            retry_state: RetryState::new(0, 5, None),
            backoff: ExponentialBackoffDecider::from_duration(
                Duration::from_secs(1),
                Some(Duration::from_secs(30)),
            ),
        })
    }

    /// Set the maximum number of reconnection attempts.
    #[must_use]
    pub fn with_max_retries(mut self, max_retries: u32) -> Self {
        self.config.max_retries = max_retries;
        self.retry_state = RetryState::new(0, max_retries, None);
        self
    }

    /// Set a custom backoff decider.
    #[must_use]
    pub fn with_backoff(mut self, backoff: ExponentialBackoffDecider) -> Self {
        self.backoff = backoff;
        self
    }

    /// Add a custom header (applied to all connections including reconnections).
    #[must_use]
    pub fn with_header(mut self, name: SimpleHeader, value: impl Into<String>) -> Self {
        self.config.headers.push((name, value.into()));
        self
    }

    /// Set initial Last-Event-ID for resuming from a known position.
    #[must_use]
    pub fn with_last_event_id(mut self, last_event_id: impl Into<String>) -> Self {
        self.last_event_id = Some(last_event_id.into());
        self
    }

    /// Create a new inner [`EventSourceTask`] for (re)connection.
    fn create_inner_task(&self) -> Option<EventSourceTask<R>> {
        let mut task = EventSourceTask::connect(self.resolver.clone(), &self.config.url).ok()?;

        // Apply headers
        for (name, value) in &self.config.headers {
            task = task.with_header(name.clone(), value);
        }

        // Apply Last-Event-ID if tracked
        if let Some(ref last_id) = self.last_event_id {
            task = task.with_last_event_id(last_id);
        }

        Some(task)
    }

    /// Track the last event ID from a received event.
    fn track_event_id(&mut self, event: &Event) {
        if let Event::Message {
            id: Some(id),
            retry,
            ..
        } = event
        {
            self.last_event_id = Some(id.clone());

            // Respect server retry field
            if let Some(ms) = retry {
                self.config.server_retry = Some(Duration::from_millis(*ms));
            }
        }
    }
}

impl<R> TaskIterator for ReconnectingEventSourceTask<R>
where
    R: DnsResolver + Clone + Send + 'static,
{
    type Ready = Event;
    type Pending = ReconnectingProgress;
    type Spawner = BoxedSendExecutionAction;

    fn next(&mut self) -> Option<TaskStatus<Self::Ready, Self::Pending, Self::Spawner>> {
        let state = self.state.take()?;

        match state {
            ReconnectingState::Connected(mut inner) => {
                match inner.next() {
                    Some(TaskStatus::Ready(event)) => {
                        self.track_event_id(&event);
                        // Reset retry state on successful event
                        self.retry_state =
                            RetryState::new(0, self.config.max_retries, None);
                        self.state = Some(ReconnectingState::Connected(inner));
                        Some(TaskStatus::Ready(event))
                    }
                    Some(TaskStatus::Pending(progress)) => {
                        self.state = Some(ReconnectingState::Connected(inner));
                        let mapped = match progress {
                            EventSourceProgress::Connecting => {
                                ReconnectingProgress::Connecting
                            }
                            EventSourceProgress::Reading => ReconnectingProgress::Reading,
                        };
                        Some(TaskStatus::Pending(mapped))
                    }
                    Some(TaskStatus::Delayed(d)) => {
                        self.state = Some(ReconnectingState::Connected(inner));
                        Some(TaskStatus::Delayed(d))
                    }
                    Some(TaskStatus::Spawn(s)) => {
                        self.state = Some(ReconnectingState::Connected(inner));
                        Some(TaskStatus::Spawn(s))
                    }
                    Some(TaskStatus::Init) => {
                        self.state = Some(ReconnectingState::Connected(inner));
                        Some(TaskStatus::Init)
                    }
                    None => {
                        // Inner task closed — attempt reconnection
                        // Use server retry duration if set, otherwise use backoff
                        let next_state = self.backoff.decide(self.retry_state.clone());

                        if let Some(next_retry) = next_state {
                            let wait = self
                                .config
                                .server_retry
                                .unwrap_or_else(|| {
                                    next_retry.wait.unwrap_or(Duration::from_secs(1))
                                });
                            self.retry_state = next_retry;
                            self.state = Some(ReconnectingState::Waiting(wait));
                            Some(TaskStatus::Pending(
                                ReconnectingProgress::Reconnecting,
                            ))
                        } else {
                            // Max retries exhausted
                            self.state = Some(ReconnectingState::Exhausted);
                            None
                        }
                    }
                }
            }

            ReconnectingState::Waiting(duration) => {
                // Signal backoff delay to executor, then transition to Reconnecting
                self.state = Some(ReconnectingState::Reconnecting);
                Some(TaskStatus::Delayed(duration))
            }

            ReconnectingState::Reconnecting => {
                // Create new inner task
                if let Some(inner) = self.create_inner_task() {
                    self.state = Some(ReconnectingState::Connected(inner));
                    Some(TaskStatus::Pending(ReconnectingProgress::Connecting))
                } else {
                    // Failed to create task — exhaust
                    self.state = Some(ReconnectingState::Exhausted);
                    None
                }
            }

            ReconnectingState::Exhausted => None,
        }
    }
}
