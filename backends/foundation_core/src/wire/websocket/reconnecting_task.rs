//! Reconnecting WebSocket client [`TaskIterator`](crate::valtron::TaskIterator) implementation.
//!
//! WHY: WebSocket connections can drop at any time. Clients need automatic reconnection
//! with exponential backoff to maintain persistent connections.
//!
//! WHAT: Wraps [`WebSocketTask`] with reconnection logic. When the inner task
//! closes (EOF or error), transitions to a backoff state and creates a new
//! inner task.
//!
//! HOW: State machine: Connected → Waiting → Reconnecting → Connected (loop).
//! Uses [`ExponentialBackoffDecider`] for backoff timing. Tracks connection
//! state and respects max retries and max reconnect duration.
//!
//! PHASE 2 SCOPE: Max reconnect duration support, exponential backoff with jitter.

use crate::retries::{ExponentialBackoffDecider, RetryDecider, RetryState};
use crate::valtron::{BoxedSendExecutionAction, TaskIterator, TaskStatus};
use crate::wire::simple_http::client::DnsResolver;
use crate::wire::simple_http::SimpleHeader;
use concurrent_queue::ConcurrentQueue;
use std::sync::Arc;
use std::time::{Duration, Instant};
use tracing::{debug, error, info, instrument, trace};

use super::error::WebSocketError;
use super::message::WebSocketMessage;
use super::task::{WebSocketProgress, WebSocketTask};

/// Configuration for reconnecting WebSocket client.
pub struct ReconnectingConfig {
    url: String,
    subprotocols: Option<String>,
    extra_headers: Vec<(SimpleHeader, String)>,
    max_retries: u32,
    max_reconnect_duration: Option<Duration>,
    read_timeout: Duration,
}

impl ReconnectingConfig {
    fn new(url: String) -> Self {
        Self {
            url,
            subprotocols: None,
            extra_headers: Vec::new(),
            max_retries: 5,
            max_reconnect_duration: None,
            read_timeout: Duration::from_secs(5),
        }
    }
}

/// Progress state for reconnecting WebSocket task.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ReconnectingWebSocketProgress {
    /// Initial connection or reconnection in progress.
    Connecting,
    /// Handshake in progress.
    Handshaking,
    /// Reading messages from active connection.
    Reading,
    /// Waiting for backoff before reconnection.
    Reconnecting,
}

enum ReconnectingWebSocketState<R: DnsResolver + Send + 'static> {
    /// Active connection with inner task.
    Connected(WebSocketTask<R>),
    /// Backoff wait before reconnection.
    Waiting(Duration),
    /// Creating new connection after backoff.
    Reconnecting,
    /// Max retries exhausted or unrecoverable error.
    Exhausted,
}

/// [`ReconnectingWebSocketTask`] wraps [`WebSocketTask`] with automatic
/// reconnection using exponential backoff.
///
/// WHY: WebSocket connections are long-lived and can drop. Automatic reconnection
/// with exponential backoff is required for robust client implementations.
///
/// WHAT: [`TaskIterator`] that transparently reconnects on connection loss,
/// preserving configuration across reconnections.
pub struct ReconnectingWebSocketTask<R: DnsResolver + Clone + Send + 'static> {
    state: Option<ReconnectingWebSocketState<R>>,
    config: ReconnectingConfig,
    resolver: R,
    retry_state: RetryState,
    backoff: ExponentialBackoffDecider,
    start_time: Option<Instant>,
    delivery_queue: Option<Arc<ConcurrentQueue<WebSocketMessage>>>,
}

impl<R> ReconnectingWebSocketTask<R>
where
    R: DnsResolver + Clone + Send + 'static,
{
    /// Create a reconnecting WebSocket task.
    ///
    /// # Errors
    ///
    /// Returns [`WebSocketError`] if the URL is invalid.
    #[instrument(skip(resolver, url), err)]
    pub fn connect(resolver: R, url: impl Into<String>) -> Result<Self, WebSocketError> {
        let url_str = url.into();
        info!(url = %url_str, "Creating reconnecting WebSocket client");

        // Validate URL upfront (same as WebSocketTask)
        let uri = crate::wire::simple_http::url::Uri::parse(&url_str).map_err(|e| {
            error!(url = %url_str, error = ?e, "Failed to parse URL");
            WebSocketError::InvalidUrl(format!("Failed to parse URL: {url_str} - {e:?}"))
        })?;

        if !uri.scheme().is_ws() && !uri.scheme().is_wss() {
            return Err(WebSocketError::InvalidUrl(format!(
                "Unsupported scheme: {}. Only ws:// and wss:// are supported.",
                uri.scheme()
            )));
        }

        debug!(scheme = ?uri.scheme(), host = ?uri.host_str(), "URL validated");

        let inner = WebSocketTask::connect(resolver.clone(), &url_str)?;
        let config = ReconnectingConfig::new(url_str);

        info!("Reconnecting WebSocket client created");

        Ok(Self {
            state: Some(ReconnectingWebSocketState::Connected(inner)),
            config,
            resolver,
            retry_state: RetryState::new(0, 5, None),
            backoff: ExponentialBackoffDecider::from_duration(
                Duration::from_secs(1),
                Some(Duration::from_secs(30)),
            ),
            start_time: Some(Instant::now()),
            delivery_queue: None,
        })
    }

    /// Set the maximum number of reconnection attempts.
    #[must_use]
    pub fn with_max_retries(mut self, max_retries: u32) -> Self {
        debug!(max_retries, "Setting max retries");
        self.config.max_retries = max_retries;
        self.retry_state = RetryState::new(0, max_retries, None);
        self
    }

    /// Set the maximum duration for reconnection attempts.
    ///
    /// WHY: Long-running WebSocket clients should give up after a certain total duration
    /// to avoid infinite reconnection loops in production systems.
    /// WHAT: Returns Self with `max_reconnect_duration` configured.
    ///
    /// # Parameters
    ///
    /// * `duration` - Total duration allowed for reconnection attempts
    #[must_use]
    pub fn with_max_reconnect_duration(mut self, duration: Duration) -> Self {
        debug!(duration_secs = ?duration.as_secs(), "Setting max reconnect duration");
        self.config.max_reconnect_duration = Some(duration);
        self
    }

    /// Set a custom backoff decider.
    #[must_use]
    pub fn with_backoff(mut self, backoff: ExponentialBackoffDecider) -> Self {
        debug!("Setting custom backoff decider");
        self.backoff = backoff;
        self
    }

    /// Add a subprotocol (applied to all connections including reconnections).
    #[must_use]
    pub fn with_subprotocol(mut self, subprotocol: impl Into<String>) -> Self {
        let protocol_str = subprotocol.into();
        debug!(subprotocol = %protocol_str, "Setting subprotocol");
        self.config.subprotocols = Some(protocol_str);
        self
    }

    /// Add subprotocols (applied to all connections including reconnections).
    #[must_use]
    pub fn with_subprotocols(mut self, subprotocols: &[impl AsRef<str>]) -> Self {
        let protocols: Vec<String> = subprotocols
            .iter()
            .map(|s| s.as_ref().to_string())
            .collect();
        let protocols_str = protocols.join(", ");
        debug!(protocols = %protocols_str, "Setting subprotocols");
        self.config.subprotocols = Some(protocols_str);
        self
    }

    /// Add a custom header (applied to all connections including reconnections).
    #[must_use]
    pub fn with_header(mut self, name: SimpleHeader, value: impl Into<String>) -> Self {
        debug!("Adding custom header");
        self.config.extra_headers.push((name, value.into()));
        self
    }

    /// Set the read timeout for connections.
    #[must_use]
    pub fn with_read_timeout(mut self, timeout: Duration) -> Self {
        debug!(timeout_secs = ?timeout.as_secs(), "Setting read timeout");
        self.config.read_timeout = timeout;
        self
    }

    /// Set the delivery queue for sending messages.
    #[must_use]
    pub fn with_delivery_queue(mut self, queue: Arc<ConcurrentQueue<WebSocketMessage>>) -> Self {
        debug!("Setting delivery queue");
        self.delivery_queue = Some(queue);
        self
    }

    /// Create a new inner [`WebSocketTask`] for (re)connection.
    fn create_inner_task(&self) -> Option<WebSocketTask<R>> {
        let mut task = WebSocketTask::connect(self.resolver.clone(), &self.config.url).ok()?;

        // Apply subprotocol
        if let Some(ref subprotocol) = self.config.subprotocols {
            task = task.with_subprotocol(subprotocol.clone());
        }

        // Apply headers
        for (name, value) in &self.config.extra_headers {
            task = task.with_header(name.clone(), value.clone());
        }

        Some(task)
    }
}

impl<R> TaskIterator for ReconnectingWebSocketTask<R>
where
    R: DnsResolver + Clone + Send + 'static,
{
    type Ready = Result<WebSocketMessage, WebSocketError>;
    type Pending = ReconnectingWebSocketProgress;
    type Spawner = BoxedSendExecutionAction;

    fn next_status(&mut self) -> Option<TaskStatus<Self::Ready, Self::Pending, Self::Spawner>> {
        let state = self.state.take()?;

        match state {
            ReconnectingWebSocketState::Connected(mut inner) => {
                trace!(state = "Connected", "Forwarding from inner task");

                match inner.next_status() {
                    Some(TaskStatus::Ready(msg_result)) => {
                        trace!("Message received from inner task");

                        // Check if this is a close message - trigger reconnection
                        if let Ok(WebSocketMessage::Close(_, _)) = msg_result {
                            debug!("Received Close message, will attempt reconnection");
                            // Connection closed gracefully - still attempt reconnect
                        }

                        // Reset retry state on successful message
                        self.retry_state = RetryState::new(0, self.config.max_retries, None);
                        self.state = Some(ReconnectingWebSocketState::Connected(inner));
                        Some(TaskStatus::Ready(msg_result))
                    }
                    Some(TaskStatus::Pending(progress)) => {
                        self.state = Some(ReconnectingWebSocketState::Connected(inner));
                        let mapped = match progress {
                            WebSocketProgress::Connecting => {
                                ReconnectingWebSocketProgress::Connecting
                            }
                            WebSocketProgress::Handshaking => {
                                ReconnectingWebSocketProgress::Handshaking
                            }
                            WebSocketProgress::Reading => ReconnectingWebSocketProgress::Reading,
                        };
                        Some(TaskStatus::Pending(mapped))
                    }
                    Some(TaskStatus::Delayed(d)) => {
                        self.state = Some(ReconnectingWebSocketState::Connected(inner));
                        Some(TaskStatus::Delayed(d))
                    }
                    Some(TaskStatus::Spawn(s)) => {
                        self.state = Some(ReconnectingWebSocketState::Connected(inner));
                        Some(TaskStatus::Spawn(s))
                    }
                    Some(TaskStatus::Init) => {
                        self.state = Some(ReconnectingWebSocketState::Connected(inner));
                        Some(TaskStatus::Init)
                    }
                    Some(TaskStatus::Ignore) => {
                        self.state = Some(ReconnectingWebSocketState::Connected(inner));
                        Some(TaskStatus::Ignore)
                    }
                    None => {
                        // Inner task closed - attempt reconnection
                        debug!("Inner task closed, attempting reconnection");

                        // Check max reconnect duration first
                        if let Some(max_duration) = self.config.max_reconnect_duration {
                            if let Some(start) = self.start_time {
                                if start.elapsed() > max_duration {
                                    error!(
                                        elapsed_secs = ?start.elapsed().as_secs(),
                                        max_secs = ?max_duration.as_secs(),
                                        "Max reconnect duration exceeded"
                                    );
                                    self.state = Some(ReconnectingWebSocketState::Exhausted);
                                    return None;
                                }
                            }
                        }

                        // Use backoff to decide next action
                        let next_state = self.backoff.decide(self.retry_state.clone());

                        if let Some(next_retry) = next_state {
                            let wait = next_retry.wait.unwrap_or(Duration::from_secs(1));
                            self.retry_state = next_retry;
                            self.state = Some(ReconnectingWebSocketState::Waiting(wait));
                            Some(TaskStatus::Pending(
                                ReconnectingWebSocketProgress::Reconnecting,
                            ))
                        } else {
                            error!(
                                attempt = self.retry_state.attempt,
                                max_retries = self.config.max_retries,
                                "Max retries exhausted"
                            );
                            self.state = Some(ReconnectingWebSocketState::Exhausted);
                            None
                        }
                    }
                }
            }

            ReconnectingWebSocketState::Waiting(duration) => {
                debug!(state = "Waiting", backoff_ms = ?duration.as_millis(), "Backoff delay");
                // Signal backoff delay to executor, then transition to Reconnecting
                self.state = Some(ReconnectingWebSocketState::Reconnecting);
                Some(TaskStatus::Delayed(duration))
            }

            ReconnectingWebSocketState::Reconnecting => {
                info!(attempt = self.retry_state.attempt, "Reconnecting WebSocket");
                // Create new inner task
                if let Some(inner) = self.create_inner_task() {
                    self.state = Some(ReconnectingWebSocketState::Connected(inner));
                    Some(TaskStatus::Pending(
                        ReconnectingWebSocketProgress::Connecting,
                    ))
                } else {
                    error!("Failed to create inner task");
                    self.state = Some(ReconnectingWebSocketState::Exhausted);
                    None
                }
            }

            ReconnectingWebSocketState::Exhausted => {
                trace!(state = "Exhausted", "Task exhausted");
                None
            }
        }
    }
}
