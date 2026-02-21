//! HTTP request task implementation using `TaskIterator` pattern.
//!
//! WHY: Provides a non-blocking, state-machine-based HTTP request executor that
//! integrates with the valtron executor system. Enables async-like request handling
//! without async/await.
//!
//! WHAT: Implements `HttpRequestTask` which processes HTTP requests through a series
//! of states (connecting, sending request, receiving response).
//! Uses `TaskIterator` trait to yield `TaskStatus` variants.
//!
//! HOW: State machine pattern where each `next()` call advances through states.
//! Phase 1 uses blocking connection for simplicity. Future phases will use
//! non-blocking connection spawning and TLS support.
//!
//! PHASE 1 SCOPE: HTTP-only (no HTTPS), blocking connection, basic GET requests.
//! PHASE 2 SCOPE: HTTPS support, non-blocking connection, advanced request handling.

use crate::valtron::{BoxedSendExecutionAction, TaskIterator, TaskStatus};
use crate::wire::simple_http::client::{DnsResolver, HttpConnectionPool, PreparedRequest};
use crate::wire::simple_http::{ClientRequestErrors, Http11, RenderHttp};
use std::io::Write;
use std::sync::Arc;

use super::{HttpOperationState, HttpStreamReady, SendRequest};

pub struct GetHttpRequestStreamTask<R>(Option<HttpOperationState>, SendRequest<R>)
where
    R: DnsResolver + Send + 'static;

impl<R> GetHttpRequestStreamTask<R>
where
    R: DnsResolver + Send + 'static,
{
    /// Creates a new HTTP request task with connection pool.
    ///
    /// # Arguments
    ///
    /// * `request` - The prepared HTTP request to execute
    /// * `resolver` - DNS resolver for hostname resolution
    /// * `max_redirects` - Maximum number of redirects to follow
    /// * `pool` - Optional connection pool for reuse
    ///
    /// # Returns
    ///
    /// A new `HttpRequestTask` in the `Init` state.
    #[must_use]
    pub fn new(
        request: PreparedRequest,
        max_redirects: u8,
        pool: Arc<HttpConnectionPool<R>>,
        timeouts: Option<super::OpTimeout>,
    ) -> Self {
        Self(
            Some(HttpOperationState::Init),
            SendRequest::new(request, max_redirects, pool, timeouts.unwrap_or_default()),
        )
    }

    /// Creates a new HTTP request task with connection pool
    /// from the provided [`GetHttpRequestStreamInner`].
    #[must_use]
    pub fn from_data(data: SendRequest<R>) -> Self {
        Self(Some(HttpOperationState::Init), data)
    }
}

impl<R> TaskIterator for GetHttpRequestStreamTask<R>
where
    R: DnsResolver + Send + 'static,
{
    type Pending = HttpOperationState;
    type Spawner = BoxedSendExecutionAction;
    type Ready = super::HttpStreamReady;

    fn next(&mut self) -> Option<TaskStatus<Self::Ready, Self::Pending, Self::Spawner>> {
        match self.0.take()? {
            HttpOperationState::Init => {
                if self.1.request.is_none() {
                    tracing::warn!("Request is missing");

                    self.0 = Some(HttpOperationState::Done);
                    return Some(TaskStatus::Ready(super::HttpStreamReady::Error(
                        ClientRequestErrors::InvalidState,
                    )));
                }

                // Transition to Connecting
                self.0 = Some(HttpOperationState::Connecting);

                Some(TaskStatus::Pending(HttpOperationState::Connecting))
            }
            HttpOperationState::Connecting => {
                if self.1.request.is_none() {
                    tracing::warn!("Request is missing");

                    self.0 = Some(HttpOperationState::Done);
                    return Some(TaskStatus::Ready(super::HttpStreamReady::Error(
                        ClientRequestErrors::InvalidState,
                    )));
                }

                // request
                let request = self.1.request.take()?;

                // Try to get connection from pool first
                let stream = self.1.pool.create_http_connection(&request.url, None);

                match stream {
                    Ok(mut connection) => {
                        tracing::debug!("Connected to {}", &request.url);

                        // Convert PreparedRequest to SimpleIncomingRequest for rendering
                        let simple_request = match request.into_simple_incoming_request() {
                            Ok(req) => req,
                            Err(e) => {
                                tracing::error!("Failed to convert request: {}", e);
                                self.0 = Some(HttpOperationState::Done);
                                return Some(TaskStatus::Ready(super::HttpStreamReady::Error(
                                    ClientRequestErrors::InvalidState,
                                )));
                            }
                        };

                        // Render HTTP request to string
                        let request_string =
                            match Http11::request(simple_request).http_render_string() {
                                Ok(s) => s,
                                Err(e) => {
                                    tracing::error!("Failed to render request: {:?}", e);
                                    self.0 = Some(HttpOperationState::Done);
                                    return Some(TaskStatus::Ready(HttpStreamReady::Error(
                                        ClientRequestErrors::InvalidState,
                                    )));
                                }
                            };

                        // Write request to connection stream BEFORE transferring ownership
                        let raw_stream = connection.stream_mut();
                        if let Err(e) = raw_stream.write_all(request_string.as_bytes()) {
                            tracing::error!("Failed to write request: {}", e);
                            self.0 = Some(HttpOperationState::Done);
                            return Some(TaskStatus::Ready(HttpStreamReady::Error(
                                ClientRequestErrors::InvalidState,
                            )));
                        }

                        if let Err(e) = raw_stream.flush() {
                            tracing::error!("Failed to write request: {}", e);
                            self.0 = Some(HttpOperationState::Done);
                            return Some(TaskStatus::Ready(HttpStreamReady::Error(
                                ClientRequestErrors::InvalidState,
                            )));
                        }

                        tracing::debug!("Request sent: {} bytes", request_string.len());

                        // Store stream and transition to receiving intro
                        tracing::debug!("Marking task as done");
                        self.0 = Some(HttpOperationState::Done);

                        tracing::debug!("Returning ready status to caller");
                        // No intro observed at this stage; return None for the intro slot.
                        Some(TaskStatus::Ready(HttpStreamReady::Done(connection)))
                    }
                    Err(e) => {
                        tracing::error!("Connection failed: {}", e);
                        self.0 = Some(HttpOperationState::Done);

                        Some(TaskStatus::Ready(HttpStreamReady::Error(
                            ClientRequestErrors::InvalidState,
                        )))
                    }
                }
            }
            HttpOperationState::Done => None,
        }
    }
}
