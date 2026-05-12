//! `ConnectionHandler` — valtron-driven keep-alive connection handler.
//!
//! Each accepted TCP connection becomes a valtron task that the executor
//! multiplexes. When no data is available (WouldBlock), the task yields
//! `TaskStatus::Delayed(duration)` with dynamic timeout calculation.
//!
//! WHY: Replaces `BackgroundJobRegistry::submit()` which blocked a thread
//! per connection. Now idle connections yield via `Delayed`, freeing the
//! thread for other work — enabling true HTTP/1.1 keep-alive multiplexing.

use std::time::Instant;

use foundation_core::io::ioutils::SharedByteBufferStream;
use foundation_core::netcap::RawStream;
use foundation_core::valtron::{BoxedSendExecutionAction, TaskIterator, TaskStatus};
use foundation_core::wire::simple_http::{
    HTTPStreams, Http11, HttpReaderError, RenderHttp, SimpleHeader, SimpleIncomingRequest,
    SimpleOutgoingResponse,
};
use foundation_core::wire::simple_http::timeout::{TimeoutCalculator, TimeoutContext};
use foundation_errstacks::ErrorTrace;

use crate::reader::read_next_request;
use crate::serve::{respond, ConnectionResult, ServeError};

use crate::app::HttpApp;
use crate::server::KeepAliveConfig;

// ---------------------------------------------------------------------------
// HandlerState

/// State machine for the connection handler.
enum HandlerState {
    /// Waiting for the next request on an open connection.
    Idle,
    /// Request was read successfully, ready to process.
    Processing {
        req: SimpleIncomingRequest,
        should_close: bool,
    },
}

// ---------------------------------------------------------------------------
// ConnectionHandler

/// Owns one TCP connection for its lifetime, multiplexed by the valtron executor.
pub struct ConnectionHandler {
    app: std::sync::Arc<HttpApp>,
    streams: HTTPStreams<RawStream>,
    conn: SharedByteBufferStream<RawStream>,
    client_ip: String,
    timeout_calculator: TimeoutCalculator,
    escalation_threshold: u32,
    max_delay_cycles: u32,
    state: Option<HandlerState>,
    idle_poll_count: u32,
    idle_since: Option<Instant>,
    total_delay_cycles: u32,
}

impl ConnectionHandler {
    /// Create a new connection handler.
    pub fn new(
        app: std::sync::Arc<HttpApp>,
        streams: HTTPStreams<RawStream>,
        conn: SharedByteBufferStream<RawStream>,
        client_ip: String,
        config: crate::server::KeepAliveConfig,
    ) -> Self {
        Self {
            app,
            streams,
            conn,
            client_ip,
            timeout_calculator: config.timeout_calculator,
            escalation_threshold: config.escalation_threshold,
            max_delay_cycles: config.max_delay_cycles,
            state: Some(HandlerState::Idle),
            idle_poll_count: 0,
            idle_since: None,
            total_delay_cycles: 0,
        }
    }

    /// Compute delay using the timeout calculator.
    /// Called after `idle_poll_count >= escalation_threshold`.
    fn compute_delay(&self) -> std::time::Duration {
        // Use streaming context for SSE/WebSocket-capable server connections
        let ctx = TimeoutContext::default().streaming();
        self.timeout_calculator.calculate_sleep_duration(&ctx)
    }

    /// Get idle timeout from calculator.
    fn idle_timeout(&self) -> std::time::Duration {
        // Use streaming context for server connections
        let ctx = TimeoutContext::default().streaming();
        self.timeout_calculator.calculate_read_timeout(&ctx)
    }

    /// Classify an error: transient errors (WouldBlock-like) are treated as
    /// "no data available". All other errors are hard errors — send 400.
    ///
    /// Since `HttpReaderError` wraps `SendableBoxedError` (not `io::Error`),
    /// we classify by variant and error message content.
    fn is_transient_error(e: &HttpReaderError) -> bool {
        // ReadFailed = stream returned no data, likely WouldBlock or EOF.
        // Treat as transient — the idle_timeout will eventually close stale connections.
        if matches!(e, HttpReaderError::ReadFailed) {
            return true;
        }
        // LineReadFailed wraps SendableBoxedError — check message for WouldBlock.
        if matches!(e, HttpReaderError::LineReadFailed(_)) {
            let msg = e.to_string().to_lowercase();
            return msg.contains("wouldblock")
                || msg.contains("timed out")
                || msg.contains("interrupted");
        }
        false
    }

    /// Update idle tracking state.
    fn track_idle(&mut self) {
        if self.idle_since.is_none() {
            self.idle_since = Some(Instant::now());
        }
    }

    /// Check if idle timeout has been exceeded.
    fn idle_exceeded(&self) -> bool {
        match self.idle_since {
            Some(since) => since.elapsed() >= self.idle_timeout(),
            None => false,
        }
    }

    /// Handle the Idle state.
    #[tracing::instrument(skip(self))]
    fn handle_idle(&mut self) -> Option<TaskStatus<(), (), BoxedSendExecutionAction>> {
        tracing::trace!("Running idle connection handling");
        // Check idle timeout.
        if self.idle_exceeded() {
            tracing::trace!(
                client_ip = %self.client_ip,
                total_idle_duration = ?self.total_idle_duration,
                "Idle timeout exceeded, closing connection"
            );
            return None;
        }

        // Check delay cycle limit.
        tracing::trace!(
            "Checking if wait cycle is below max: {} < {}",
            &self.total_delay_cycles,
            &self.max_delay_cycles
        );
        if self.total_delay_cycles >= self.max_delay_cycles {
            tracing::trace!(
                client_ip = %self.client_ip,
                total_delay_cycles = self.total_delay_cycles,
                "Max delay cycles exceeded, closing connection"
            );
            return None;
        }

        // Attempt to read the next request.
        tracing::trace!(
            "Reading next request from stream: {} < {}",
            &self.total_delay_cycles,
            &self.max_delay_cycles
        );
        match read_next_request(&self.streams, &self.client_ip) {
            Some(Ok(req)) => {
                // Data received — reset idle tracking.
                let should_close = req
                    .headers
                    .get(&SimpleHeader::CONNECTION)
                    .map(|values| {
                        values.iter().any(|v| {
                            let v = v.trim();
                            v.eq_ignore_ascii_case("close")
                        })
                    })
                    .unwrap_or(false);

                self.idle_poll_count = 0;
                self.idle_since = None;
                self.total_delay_cycles = 0;

                tracing::trace!(
                    client_ip = %self.client_ip,
                    method = ?req.method,
                    path = %req.request_url.url,
                    should_close = should_close,
                    "Request received, transitioning to Processing"
                );

                self.state = Some(HandlerState::Processing { req, should_close });
                Some(TaskStatus::Pending(()))
            }
            Some(Err(e)) => {
                if Self::is_transient_error(&e) {
                    // No data available — treat as idle.
                    self.idle_poll_count += 1;
                    self.track_idle();

                    if self.idle_poll_count < self.escalation_threshold {
                        // Fast polling phase — keep the executor spinning.
                        self.state = Some(HandlerState::Idle);
                        return Some(TaskStatus::Pending(()));
                    }

                    // Delayed phase — compute exponential backoff.
                    let delay = self.compute_delay();

                    // Track delay cycles: each time we cross another multiple
                    // of escalation_threshold, increment the cycle counter.
                    let new_cycle_count = self.idle_poll_count / self.escalation_threshold;
                    if new_cycle_count > self.total_delay_cycles {
                        self.total_delay_cycles = new_cycle_count;
                    }

                    tracing::trace!(
                        client_ip = %self.client_ip,
                        idle_poll_count = self.idle_poll_count,
                        total_delay_cycles = self.total_delay_cycles,
                        delay = ?delay,
                        "No data (transient error), returning Delayed"
                    );

                    self.state = Some(HandlerState::Idle);
                    Some(TaskStatus::Delayed(delay))
                } else {
                    // Hard parse error — send 400 and close.
                    let err = ErrorTrace::new(ServeError::BadRequest {
                        status: 400,
                        reason: e.to_string(),
                    });
                    tracing::error!(
                        client_ip = %self.client_ip,
                        err = ?err,
                        "Hard parse error from client"
                    );
                    let _ = respond::text(&mut self.conn.clone(), 400, "Bad Request");
                    None
                }
            }
            None => {
                // No data at all (WouldBlock with no bytes read).
                self.idle_poll_count += 1;
                self.track_idle();

                if self.idle_poll_count < self.escalation_threshold {
                    self.state = Some(HandlerState::Idle);
                    return Some(TaskStatus::Pending(()));
                }

                let delay = self.compute_delay();
                let new_cycle_count = self.idle_poll_count / self.escalation_threshold;
                if new_cycle_count > self.total_delay_cycles {
                    self.total_delay_cycles = new_cycle_count;
                }

                tracing::trace!(
                    client_ip = %self.client_ip,
                    idle_poll_count = self.idle_poll_count,
                    total_delay_cycles = self.total_delay_cycles,
                    delay = ?delay,
                    "No data (None), returning Delayed"
                );

                self.state = Some(HandlerState::Idle);
                Some(TaskStatus::Delayed(delay))
            }
        }
    }

    /// Handle the Processing state.
    #[tracing::instrument(skip(self))]
    fn handle_processing(
        &mut self,
        mut req: SimpleIncomingRequest,
        should_close: bool,
    ) -> Option<TaskStatus<(), (), BoxedSendExecutionAction>> {
        let bag = self.app.context().clone();

        tracing::trace!(
            request_proto = %req.proto,
            request_uri = %req.request_uri,
            request_url = %req.request_url,
            request_method = %req.method,
            request_headers = ?req.headers,
            "Request received for processing"
        );

        // Run middleware chain — middleware takes &mut req.
        let mut middleware_response: Option<SimpleOutgoingResponse> = None;
        for mw in self.app.middleware_chain() {
            match mw.handle(&bag, &mut req) {
                crate::middleware::MiddlewareResult::Continue => {}
                crate::middleware::MiddlewareResult::Response(resp) => {
                    middleware_response = Some(resp);
                    break;
                }
            }
        }

        tracing::trace!(
            request_proto = %req.proto,
            request_uri = %req.request_uri,
            request_url = %req.request_url,
            request_method = %req.method,
            request_headers = ?req.headers,
            "Applied middleware chain"
        );

        if let Some(mut resp) = middleware_response {
            tracing::trace!(
                request_proto = %req.proto,
                request_uri = %req.request_uri,
                request_url = %req.request_url,
                request_method = %req.method,
                request_headers = ?req.headers,
                "Received middleware response"
            );
            if should_close {
                resp.headers
                    .insert(SimpleHeader::CONNECTION, vec!["close".to_string()]);
            }
            let _ = Http11::response(resp).http_render_to_writer(&mut self.conn.clone());
            tracing::trace!(
                request_proto = %req.proto,
                request_uri = %req.request_uri,
                request_url = %req.request_url,
                request_method = %req.method,
                request_headers = ?req.headers,
                "Sent middleware response as request response"
            );
            if should_close {
                tracing::trace!(
                    request_proto = %req.proto,
                    request_uri = %req.request_uri,
                    request_url = %req.request_url,
                    request_method = %req.method,
                    request_headers = ?req.headers,
                    "Ending request processing"
                );
                return None;
            }
            self.state = Some(HandlerState::Idle);
            return Some(TaskStatus::Pending(()));
        }

        // Route dispatch.
        let method = &req.method;
        let path = &req.request_url.url;

        tracing::trace!(
            request_proto = %req.proto,
            request_uri = %req.request_uri,
            request_url = %req.request_url,
            request_method = %req.method,
            request_headers = ?req.headers,
            "Passing request to router"
        );

        match self.app.router().dispatch(method, path) {
            Some(handler) => {
                tracing::trace!(
                    request_proto = %req.proto,
                    request_uri = %req.request_uri,
                    request_url = %req.request_url,
                    request_method = %req.method,
                    request_headers = ?req.headers,
                    "Router returns handler"
                );

                let result = handler.serve(bag, req, self.conn.clone());

                match result {
                    ConnectionResult::Take => {
                        tracing::trace!(
                            client_ip = %self.client_ip,
                            "Handler returned Take (connection taken)"
                        );
                        None
                    }
                    ConnectionResult::Close(err) => {
                        if let Some(e) = &err {
                            tracing::error!(
                                client_ip = %self.client_ip,
                                err = ?e,
                                "Connection closed with error"
                            );
                        }
                        None
                    }
                    ConnectionResult::Keep => {
                        if should_close {
                            return None;
                        }
                        self.state = Some(HandlerState::Idle);
                        Some(TaskStatus::Pending(()))
                    }
                }
            }
            None => {
                tracing::trace!(
                    client_ip = %self.client_ip,
                    method = ?method,
                    path = %path,
                    "No route matched, returning 404"
                );
                let _ = respond::not_found(&mut self.conn.clone());
                if should_close {
                    return None;
                }
                self.state = Some(HandlerState::Idle);
                Some(TaskStatus::Pending(()))
            }
        }
    }
}

// ---------------------------------------------------------------------------
// TaskIterator implementation

impl TaskIterator for ConnectionHandler {
    type Ready = ();
    type Pending = ();
    type Spawner = BoxedSendExecutionAction;

    fn next_status(&mut self) -> Option<TaskStatus<Self::Ready, Self::Pending, Self::Spawner>> {
        let Some(state) = self.state.take() else {
            tracing::trace!("Ending Connection multiplexing");
            return None;
        };

        tracing::trace!("Calling Connection multiplexing");
        match state {
            HandlerState::Idle => self.handle_idle(),
            HandlerState::Processing { req, should_close } => {
                self.handle_processing(req, should_close)
            }
        }
    }
}
