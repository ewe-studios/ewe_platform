//! `ConnectionHandler` — valtron-driven keep-alive connection handler.
//!
//! Each accepted TCP connection becomes a valtron task that the executor
//! multiplexes. When no data is available (`WouldBlock`), the task yields
//! `TaskStatus::Delayed(duration)` with dynamic timeout calculation.
//!
//! WHY: Replaces `BackgroundJobRegistry::submit()` which blocked a thread
//! per connection. Now idle connections yield via `Delayed`, freeing the
//! thread for other work — enabling true HTTP/1.1 keep-alive multiplexing.

use std::sync::Arc;
use std::time::{Duration, Instant};

use foundation_core::io::ioutils::SharedByteBufferStream;
use foundation_core::netcap::RawStream;
use foundation_core::valtron::{BoxedSendExecutionAction, TaskIterator, TaskStatus};
use foundation_core::wire::simple_http::timeout::{TimeoutCalculator, TimeoutContext};
use foundation_core::wire::simple_http::{
    HTTPStreams, Http11, HttpReaderError, RenderHttp, SimpleHeader, SimpleIncomingRequest,
    SimpleOutgoingResponse,
};
use foundation_errstacks::ErrorTrace;

use crate::native::reader::read_next_request;
use crate::shared::serve::{respond, ConnectionResult, Serve, ServeError};

// ---------------------------------------------------------------------------
// HandlerState

/// State machine for the connection handler.
enum HandlerState {
    /// Waiting for the next request on an open connection.
    Idle,
    /// Retry 100-continue write after transient failure.
    CheckExpect {
        req: SimpleIncomingRequest,
        should_close: bool,
        continue_retries: usize,
    },
    /// After sending 100 Continue, waiting for client body data.
    WaitingForBody {
        req: SimpleIncomingRequest,
        should_close: bool,
        attempt: usize,
    },
    /// Run middleware chain, route, and dispatch to handler.
    Processing {
        req: SimpleIncomingRequest,
        should_close: bool,
    },
}

// ---------------------------------------------------------------------------
// ConnectionHandler

/// Owns one TCP connection for its lifetime, multiplexed by the valtron executor.
pub struct ConnectionHandler {
    app: std::sync::Arc<crate::shared::app::HttpApp<Arc<dyn Serve>>>,
    streams: HTTPStreams<RawStream>,
    conn: SharedByteBufferStream<RawStream>,
    client_ip: String,
    /// Cloned calculator for computing expect-continue delays.
    timeout_calculator: TimeoutCalculator,
    max_expect_attempts: usize,
    max_continue_retries: usize,
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
        app: std::sync::Arc<crate::shared::app::HttpApp<Arc<dyn Serve>>>,
        streams: HTTPStreams<RawStream>,
        conn: SharedByteBufferStream<RawStream>,
        client_ip: String,
        config: &super::KeepAliveConfig,
    ) -> Self {
        let max_expect_attempts = config
            .timeout_calculator
            .config()
            .expect_continue
            .max_attempts;
        Self {
            app,
            streams,
            conn,
            client_ip,
            timeout_calculator: config.timeout_calculator.clone(),
            max_expect_attempts,
            max_continue_retries: 3,
            escalation_threshold: config.escalation_threshold,
            max_delay_cycles: config.max_delay_cycles,
            state: Some(HandlerState::Idle),
            idle_poll_count: 0,
            idle_since: None,
            total_delay_cycles: 0,
        }
    }

    /// Compute delay using the timeout calculator.
    fn compute_delay(&self) -> Duration {
        let ctx = TimeoutContext::default().streaming();
        self.timeout_calculator.calculate_sleep_duration(&ctx)
    }

    /// Get idle timeout from calculator.
    fn idle_timeout(&self) -> Duration {
        let ctx = TimeoutContext::default().streaming();
        self.timeout_calculator.calculate_read_timeout(&ctx)
    }

    fn is_transient_error(e: &HttpReaderError) -> bool {
        if matches!(e, HttpReaderError::ReadFailed) {
            return true;
        }
        if matches!(e, HttpReaderError::LineReadFailed(_)) {
            let msg = e.to_string().to_lowercase();
            return msg.contains("wouldblock")
                || msg.contains("timed out")
                || msg.contains("interrupted");
        }
        false
    }

    fn has_body(req: &SimpleIncomingRequest) -> bool {
        req.headers
            .get(&SimpleHeader::CONTENT_LENGTH)
            .is_some_and(|values| {
                values
                    .iter()
                    .any(|v| v.trim().parse::<u64>().is_ok_and(|n| n > 0))
            })
            || req
                .headers
                .get(&SimpleHeader::TRANSFER_ENCODING)
                .is_some_and(|values| {
                    values.iter().any(|v| {
                        v.to_lowercase()
                            .split(',')
                            .any(|part| part.trim() == "chunked")
                    })
                })
    }

    fn has_expect_continue(req: &SimpleIncomingRequest) -> bool {
        req.headers
            .get(&SimpleHeader::EXPECT)
            .is_some_and(|values| {
                values
                    .iter()
                    .any(|v| v.trim().eq_ignore_ascii_case("100-continue"))
            })
    }

    fn track_idle(&mut self) {
        if self.idle_since.is_none() {
            self.idle_since = Some(Instant::now());
        }
    }

    fn idle_exceeded(&self) -> bool {
        match self.idle_since {
            Some(since) => since.elapsed() >= self.idle_timeout(),
            None => false,
        }
    }

    // ---- Idle state -------------------------------------------------------

    fn handle_idle(&mut self) -> Option<TaskStatus<(), (), BoxedSendExecutionAction>> {
        if self.idle_exceeded() {
            tracing::trace!(
                client_ip = %self.client_ip,
                "Idle timeout exceeded, closing connection"
            );
            return None;
        }
        if self.total_delay_cycles >= self.max_delay_cycles {
            tracing::trace!(
                client_ip = %self.client_ip,
                max_delay_cycles = self.max_delay_cycles,
                "Max delay cycles exceeded, closing connection"
            );
            return None;
        }

        tracing::trace!(client_ip = %self.client_ip, "Idle: attempting to read next request");

        match read_next_request(&self.streams, &self.client_ip) {
            Some(Ok(req)) => {
                let should_close =
                    req.headers
                        .get(&SimpleHeader::CONNECTION)
                        .is_some_and(|values| {
                            values
                                .iter()
                                .any(|v| v.trim().eq_ignore_ascii_case("close"))
                        });

                tracing::trace!(
                    client_ip = %self.client_ip,
                    method = %req.method,
                    path = %req.request_url.url,
                    should_close = should_close,
                    "Idle: received request, transitioning to CheckExpect"
                );

                self.idle_poll_count = 0;
                self.idle_since = None;
                self.total_delay_cycles = 0;

                self.state = Some(HandlerState::CheckExpect {
                    req,
                    should_close,
                    continue_retries: 0,
                });
                Some(TaskStatus::Pending(()))
            }
            Some(Err(e)) => {
                if Self::is_transient_error(&e) {
                    tracing::trace!(
                        client_ip = %self.client_ip,
                        err = ?e,
                        idle_poll_count = self.idle_poll_count,
                        "Idle: transient read error, delaying"
                    );
                    self.idle_poll_count += 1;
                    self.track_idle();
                    Some(self.maybe_delay_idle())
                } else {
                    tracing::warn!(
                        client_ip = %self.client_ip,
                        err = ?e,
                        "Idle: non-transient read error, sending 400"
                    );
                    let _err = ErrorTrace::new(ServeError::BadRequest {
                        status: 400,
                        reason: e.to_string(),
                    });
                    let _ = respond::text(&mut self.conn.clone(), 400, "Bad Request");
                    None
                }
            }
            None => {
                tracing::trace!(
                    client_ip = %self.client_ip,
                    "Idle: no data available (WouldBlock)"
                );
                self.idle_poll_count += 1;
                self.track_idle();
                Some(self.maybe_delay_idle())
            }
        }
    }

    fn maybe_delay_idle(&mut self) -> TaskStatus<(), (), BoxedSendExecutionAction> {
        if self.idle_poll_count < self.escalation_threshold {
            tracing::trace!(
                client_ip = %self.client_ip,
                idle_poll_count = self.idle_poll_count,
                "Idle: spinning without delay (below escalation threshold)"
            );
            self.state = Some(HandlerState::Idle);
            return TaskStatus::Pending(());
        }
        let delay = self.compute_delay();
        let new_cycle_count = self.idle_poll_count / self.escalation_threshold;
        if new_cycle_count > self.total_delay_cycles {
            self.total_delay_cycles = new_cycle_count;
        }
        tracing::trace!(
            client_ip = %self.client_ip,
            idle_poll_count = self.idle_poll_count,
            delay_cycles = self.total_delay_cycles,
            delay_ms = delay.as_millis(),
            "Idle: escalating delay due to prolonged inactivity"
        );
        self.state = Some(HandlerState::Idle);
        TaskStatus::Delayed(delay)
    }

    // ---- CheckExpect state ------------------------------------------------

    fn handle_check_expect(
        &mut self,
        req: SimpleIncomingRequest,
        should_close: bool,
        continue_retries: usize,
    ) -> Option<TaskStatus<(), (), BoxedSendExecutionAction>> {
        if Self::has_expect_continue(&req) {
            // Always send 100-continue then skip waiting for no body there.
            // If there's no body, skip WaitingForBody and go straight to Processing.
            if !Self::has_body(&req) {
                tracing::trace!(
                    client_ip = %self.client_ip,
                    method = %req.method,
                    path = %req.request_url.url,
                    "Expect: 100-continue with no body, skipping to Processing"
                );
                self.state = Some(HandlerState::Processing { req, should_close });
                return Some(TaskStatus::Pending(()));
            }

            // Send 100 Continue and wait for body data.
            match respond::continue_100(&mut self.conn.clone()) {
                Ok(()) => {
                    tracing::trace!(
                        client_ip = %self.client_ip,
                        "Sent 100 Continue, entering WaitingForBody state"
                    );
                    self.state = Some(HandlerState::WaitingForBody {
                        req,
                        should_close,
                        attempt: 0,
                    });
                    let delay = self
                        .timeout_calculator
                        .calculate_expect_continue_delay(
                            &TimeoutContext::default(),
                            0,
                            self.max_expect_attempts,
                        )
                        .expect("first attempt must yield a delay");
                    return Some(TaskStatus::Delayed(delay));
                }
                Err(e) => {
                    let next = continue_retries + 1;
                    if next <= self.max_continue_retries {
                        tracing::trace!(
                            client_ip = %self.client_ip,
                            err = ?e,
                            retry = next,
                            max_retries = self.max_continue_retries,
                            "100 Continue write failed, retrying"
                        );
                        // Re-enter CheckExpect with incremented counter.
                        self.state = Some(HandlerState::CheckExpect {
                            req,
                            should_close,
                            continue_retries: next,
                        });
                        // Short delay before retry — use base delay from calculator.
                        let delay = self
                            .timeout_calculator
                            .calculate_expect_continue_delay(
                                &TimeoutContext::default(),
                                continue_retries,
                                self.max_expect_attempts.max(1),
                            )
                            .unwrap_or(Duration::from_millis(100));
                        return Some(TaskStatus::Delayed(delay));
                    }
                    tracing::warn!(
                        client_ip = %self.client_ip,
                        err = ?e,
                        retries = continue_retries,
                        max_retries = self.max_continue_retries,
                        "Failed to write 100 Continue after all retries"
                    );
                    let _ = respond::text(
                        &mut self.conn.clone(),
                        502,
                        "Bad Gateway — Expect: failed to write response",
                    );
                    return None;
                }
            }
        }
        // No expect header — go straight to processing.
        tracing::trace!(
            client_ip = %self.client_ip,
            method = %req.method,
            path = %req.request_url.url,
            "No Expect: 100-continue, transitioning to Processing"
        );
        self.state = Some(HandlerState::Processing { req, should_close });
        Some(TaskStatus::Pending(()))
    }

    // ---- WaitingForBody state ---------------------------------------------

    #[allow(clippy::too_many_lines)]
    fn handle_waiting_for_body(
        &mut self,
        req: SimpleIncomingRequest,
        should_close: bool,
        attempt: usize,
    ) -> Option<TaskStatus<(), (), BoxedSendExecutionAction>> {
        let next_attempt = attempt + 1;

        tracing::trace!(
            client_ip = %self.client_ip,
            attempt = attempt,
            "WaitingForBody: probing for body data"
        );

        match self.conn.probe() {
            Ok(len) if len > 0 => {
                tracing::trace!(
                    client_ip = %self.client_ip,
                    attempt = attempt,
                    bytes_available = len,
                    "WaitingForBody: body data available, transitioning to Processing"
                );
                self.state = Some(HandlerState::Processing { req, should_close });
                Some(TaskStatus::Pending(()))
            }
            Ok(_) => self.waiting_for_body_retry(req, should_close, attempt, next_attempt),
            Err(e) => {
                let kind = e.kind();
                if kind == std::io::ErrorKind::WouldBlock
                    || kind == std::io::ErrorKind::TimedOut
                    || kind == std::io::ErrorKind::Interrupted
                {
                    self.waiting_for_body_retry(req, should_close, attempt, next_attempt)
                } else {
                    tracing::warn!(
                        client_ip = %self.client_ip,
                        attempt = attempt,
                        err = ?e,
                        "WaitingForBody: hard error on probe"
                    );
                    let _ = respond::text(&mut self.conn.clone(), 400, "Bad Request");
                    None
                }
            }
        }
    }

    fn waiting_for_body_retry(
        &mut self,
        req: SimpleIncomingRequest,
        should_close: bool,
        attempt: usize,
        next_attempt: usize,
    ) -> Option<TaskStatus<(), (), BoxedSendExecutionAction>> {
        tracing::trace!(
            client_ip = %self.client_ip,
            attempt = attempt,
            "WaitingForBody: no data, computing next delay"
        );
        let prev = self.timeout_calculator.calculate_expect_continue_delay(
            &TimeoutContext::default(),
            attempt,
            self.max_expect_attempts,
        );
        let ctx = match prev {
            Some(d) => TimeoutContext::default().with_previous_timeout(d),
            None => TimeoutContext::default(),
        };
        if let Some(delay) = self.timeout_calculator.calculate_expect_continue_delay(
            &ctx,
            next_attempt,
            self.max_expect_attempts,
        ) {
            tracing::trace!(
                client_ip = %self.client_ip,
                attempt = attempt,
                delay_ms = delay.as_millis(),
                "WaitingForBody: delaying before next retry"
            );
            self.state = Some(HandlerState::WaitingForBody {
                req,
                should_close,
                attempt: next_attempt,
            });
            Some(TaskStatus::Delayed(delay))
        } else {
            tracing::warn!(
                client_ip = %self.client_ip,
                attempt = attempt,
                max_attempts = self.max_expect_attempts,
                "WaitingForBody: max attempts reached, sending 408"
            );
            let _ = respond::text(
                &mut self.conn.clone(),
                408,
                "Request Timeout — body not received after 100 Continue",
            );
            None
        }
    }

    // ---- Processing state -------------------------------------------------

    fn handle_processing(
        &mut self,
        mut req: SimpleIncomingRequest,
        should_close: bool,
    ) -> Option<TaskStatus<(), (), BoxedSendExecutionAction>> {
        tracing::trace!(
            client_ip = %self.client_ip,
            method = %req.method,
            path = %req.request_url.url,
            "Processing: entering middleware chain"
        );

        let bag = self.app.context().clone();

        // Run middleware chain.
        let mut middleware_response: Option<SimpleOutgoingResponse> = None;
        for mw in self.app.middleware_chain() {
            match mw.handle(&bag, &mut req) {
                crate::shared::middleware::MiddlewareResult::Continue => {}
                crate::shared::middleware::MiddlewareResult::Response(resp) => {
                    tracing::trace!(
                        client_ip = %self.client_ip,
                        status = %resp.status,
                        "Processing: middleware short-circuited with response"
                    );
                    middleware_response = Some(resp);
                    break;
                }
                crate::shared::middleware::MiddlewareResult::InterimResponse(resp) => {
                    tracing::trace!(
                        client_ip = %self.client_ip,
                        status = %resp.status,
                        "Processing: middleware returned interim response"
                    );

                    let client_ip = self.client_ip.clone();
                    let client_status = resp.status.clone();

                    if let Err(err) =
                        Http11::response(resp).http_render_to_writer(&mut self.conn.clone())
                    {
                        tracing::trace!(
                            client_ip = %client_ip,
                            status = %client_status,
                            err = ?err,
                            "Failed to write response to reader, stopping: {:?}", err);
                        // stop immediately
                        return None;
                    }
                }
            }
        }

        if let Some(mut resp) = middleware_response {
            tracing::trace!("Running middleware response process");
            if should_close {
                resp.headers
                    .insert(SimpleHeader::CONNECTION, vec!["close".to_string()]);
            }
            let _ = Http11::response(resp).http_render_to_writer(&mut self.conn.clone());
            if should_close {
                tracing::trace!(client_ip = %self.client_ip, "Processing: connection closed after middleware response");
                return None;
            }
            tracing::trace!(client_ip = %self.client_ip, "Processing: returning to Idle after middleware response");
            self.state = Some(HandlerState::Idle);
            return Some(TaskStatus::Pending(()));
        }

        tracing::trace!("[NORMAL FLOW] Running normal request response flow");

        // Route dispatch.
        let method = &req.method;
        let path = &req.request_url.url;

        tracing::trace!(
            client_ip = %self.client_ip,
            method = %method,
            path = %path,
            "Processing: dispatching to route handler"
        );

        if let Some(handler) = self.app.router().dispatch(method, path) {
            let result = handler.serve(bag, req, self.conn.clone());
            match result {
                ConnectionResult::Take => {
                    tracing::trace!(client_ip = %self.client_ip, "Processing: handler took connection (e.g. WebSocket upgrade)");
                    None
                }
                ConnectionResult::Close(err) => {
                    if let Some(e) = &err {
                        tracing::error!(client_ip = %self.client_ip, err = ?e, "Connection closed with error");
                    }
                    None
                }
                ConnectionResult::Keep => {
                    if should_close {
                        return None;
                    }
                    tracing::trace!(client_ip = %self.client_ip, "Processing: handler kept connection, returning to Idle");
                    self.state = Some(HandlerState::Idle);
                    Some(TaskStatus::Pending(()))
                }
            }
        } else {
            tracing::trace!(
                client_ip = %self.client_ip,
                method = %method,
                path = %path,
                "Processing: no route matched, returning 404"
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

// ---------------------------------------------------------------------------
// TaskIterator implementation

impl TaskIterator for ConnectionHandler {
    type Ready = ();
    type Pending = ();
    type Spawner = BoxedSendExecutionAction;

    fn next_status(&mut self) -> Option<TaskStatus<Self::Ready, Self::Pending, Self::Spawner>> {
        let state = self.state.take()?;

        match state {
            HandlerState::Idle => self.handle_idle(),
            HandlerState::CheckExpect {
                req,
                should_close,
                continue_retries,
            } => self.handle_check_expect(req, should_close, continue_retries),
            HandlerState::WaitingForBody {
                req,
                should_close,
                attempt,
            } => self.handle_waiting_for_body(req, should_close, attempt),
            HandlerState::Processing { req, should_close } => {
                self.handle_processing(req, should_close)
            }
        }
    }
}
