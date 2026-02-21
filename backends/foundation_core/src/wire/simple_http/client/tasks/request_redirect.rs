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

use crate::io::ioutils::ReadTimeoutOperations;
use crate::netcap::RawStream;
use crate::valtron::{BoxedSendExecutionAction, TaskIterator, TaskStatus};
use crate::wire::simple_http::client::{
    redirects, DnsResolver, HttpClientConnection, HttpConnectionPool,
};
use crate::wire::simple_http::{
    ClientRequestErrors, Http11, HttpResponseReader, IncomingResponseParts, RenderHttp,
    RequestDescriptor, SimpleHeader, SimpleHttpBody, SimpleIncomingRequest,
};
use std::io::Write;
use std::sync::Arc;
use std::time::Duration;

use super::{HttpOperationState, OpTimeout};

pub enum HttpRequestRedirectState<R: DnsResolver + Send + 'static> {
    Init(
        Option<
            Box<(
                SimpleIncomingRequest,
                OpTimeout,
                Arc<HttpConnectionPool<R>>,
                u8,
            )>,
        >,
    ),
    Trying(
        Option<
            Box<(
                SimpleIncomingRequest,
                OpTimeout,
                Arc<HttpConnectionPool<R>>,
                RequestDescriptor,
                u8,
            )>,
        >,
    ),
    WriteBody(
        Option<
            Box<(
                SimpleIncomingRequest,
                Arc<HttpConnectionPool<R>>,
                HttpClientConnection,
            )>,
        >,
    ),
    Done,
}

pub enum HttpRequestRedirectResponse {
    Done(HttpClientConnection),
    Error(ClientRequestErrors),
    FlushFailed(HttpClientConnection, std::io::Error),
}

/// Redirect-capable variant: small task wrapper that can be spawned in place of the
/// stream-only task to perform connect/send/probe/redirect-loop behavior.
/// It mirrors the shape of `GetHttpRequestStreamTask` so it can be constructed
/// from the same `GetHttpRequestStreamInner` data when needed.
pub struct GetHttpRequestRedirectTask<R: DnsResolver + Send + 'static>(
    Option<HttpRequestRedirectState<R>>,
);

impl<R: DnsResolver + Send + 'static> GetHttpRequestRedirectTask<R> {
    /// Create a new redirect-capable task from the provided inner data.
    #[must_use]
    pub fn new(
        data: SimpleIncomingRequest,
        timeout: Option<OpTimeout>,
        pool: Arc<HttpConnectionPool<R>>,
        max_redirects: u8,
    ) -> Self {
        Self(Some(HttpRequestRedirectState::Init(Some(Box::new((
            data,
            timeout.unwrap_or_default(),
            pool,
            max_redirects,
        ))))))
    }
}

impl<R: DnsResolver + Send + 'static> TaskIterator for GetHttpRequestRedirectTask<R> {
    type Pending = HttpOperationState;
    type Ready = HttpRequestRedirectResponse;
    type Spawner = BoxedSendExecutionAction;

    fn next(&mut self) -> Option<TaskStatus<Self::Ready, Self::Pending, Self::Spawner>> {
        match self.0.take()? {
            HttpRequestRedirectState::Init(mut inner_opt) => {
                if let Some(inner) = inner_opt.take() {
                    let (data, timeout, pool, remaining_redirects) = *inner;

                    // create the request descriptor
                    let request_descriptor = data.descriptor();

                    self.0 = Some(HttpRequestRedirectState::Trying(Some(Box::new((
                        data,
                        timeout,
                        pool,
                        request_descriptor,
                        remaining_redirects,
                    )))));

                    return Some(TaskStatus::Pending(HttpOperationState::Connecting));
                }

                self.0 = Some(HttpRequestRedirectState::Done);
                None
            }
            HttpRequestRedirectState::Trying(inner_opt) => {
                let Some(state) = inner_opt else {
                    self.0 = Some(HttpRequestRedirectState::Done);
                    return None;
                };

                let (data, timeout, pool, mut descriptor, remaining_redirects) = *state;

                // 1. Create connection
                let Ok(mut connection) = pool.create_http_connection(&descriptor.request_uri, None)
                else {
                    self.0 = Some(HttpRequestRedirectState::Done);
                    return Some(TaskStatus::Ready(HttpRequestRedirectResponse::Error(
                        ClientRequestErrors::ConnectionFailed,
                    )));
                };

                // add Expect header for 100-continue
                descriptor
                    .headers
                    .insert(SimpleHeader::EXPECT, vec!["100-continue".into()]);

                // 2. Render and send request
                let Ok(request_string) =
                    Http11::request_descriptor(descriptor.clone()).http_render_string()
                else {
                    self.0 = Some(HttpRequestRedirectState::Done);
                    return Some(TaskStatus::Ready(HttpRequestRedirectResponse::Error(
                        ClientRequestErrors::InvalidState,
                    )));
                };

                if let Err(err) = connection.stream_mut().write_all(request_string.as_bytes()) {
                    tracing::error!("Failed to write request: {}", err);

                    self.0 = Some(HttpRequestRedirectState::Done);
                    return Some(TaskStatus::Ready(HttpRequestRedirectResponse::Error(
                        ClientRequestErrors::WriteFailed,
                    )));
                }

                if let Err(err) = connection.stream_mut().flush() {
                    tracing::error!("Failed to write request: {}", err);
                    self.0 = Some(HttpRequestRedirectState::Done);
                    return Some(TaskStatus::Ready(HttpRequestRedirectResponse::Error(
                        ClientRequestErrors::WriteFailed,
                    )));
                }

                // 3. Set read timeout (preserve previous)
                let previous_timeout = connection
                    .stream_mut()
                    .get_current_read_timeout()
                    .unwrap_or(None);

                if let Err(err) = connection
                    .stream_mut()
                    .set_read_timeout_as(timeout.read_timeout)
                {
                    tracing::error!("Failed to set read timeout: {}", err);
                    self.0 = Some(HttpRequestRedirectState::Done);
                    return Some(TaskStatus::Ready(HttpRequestRedirectResponse::Error(
                        ClientRequestErrors::Timeout,
                    )));
                }

                // 4. Try to read response intro once
                let mut reader = HttpResponseReader::<SimpleHttpBody, RawStream>::new(
                    connection.clone_stream(),
                    SimpleHttpBody,
                );

                // Flattened: check intro and headers one by one, fallback to WriteBody if either missing
                let intro_result = reader.next();
                if !matches!(
                    &intro_result,
                    Some(Ok(IncomingResponseParts::Intro(_, _, _)))
                ) {
                    self.0 = Some(HttpRequestRedirectState::WriteBody(Some(Box::new((
                        data, pool, connection,
                    )))));
                    return Some(TaskStatus::Pending(HttpOperationState::Connecting));
                }

                let headers_result = reader.next();
                if !matches!(&headers_result, Some(Ok(IncomingResponseParts::Headers(_)))) {
                    self.0 = Some(HttpRequestRedirectState::WriteBody(Some(Box::new((
                        data, pool, connection,
                    )))));
                    return Some(TaskStatus::Pending(HttpOperationState::Connecting));
                }

                // Restore previous timeout
                let _ = connection
                    .stream_mut()
                    .set_read_timeout_as(previous_timeout.unwrap_or(Duration::from_secs(60)));

                // Both intro and headers are present
                let (status, _proto, _text) = match intro_result {
                    Some(Ok(IncomingResponseParts::Intro(status, proto, text))) => {
                        tracing::debug!("Received HTTP intro: status={}, proto={}, text={:?}", status, proto, text);
                        (status, proto, text)
                    }
                    _ => unreachable!("Intro must be present here due to prior matches! check; fallback to WriteBody if missing."),
                };
                let headers = match headers_result {
                    Some(Ok(IncomingResponseParts::Headers(ref h))) => {
                        tracing::debug!("Received HTTP headers: {:?}", h);
                        h
                    }
                    _ => unreachable!("Headers must be present here due to prior matches! check; fallback to WriteBody if missing."),
                };

                let is_redirect = (300..400).contains(&status.clone().into_usize());
                let location_header = headers.get(&SimpleHeader::LOCATION).and_then(|v| v.first());

                if is_redirect && location_header.is_some() && remaining_redirects > 0 {
                    tracing::info!(
                        "Redirect detected: status {} with Location header {:?}",
                        status,
                        location_header
                    );

                    #[allow(clippy::unnecessary_unwrap)]
                    let location = location_header.expect("Location header is missing");
                    let new_url =
                        match redirects::resolve_location(&descriptor.request_uri, location) {
                            Ok(url) => url,
                            Err(e) => {
                                tracing::error!("Failed to resolve redirect location: {}", e);
                                self.0 = Some(HttpRequestRedirectState::Done);
                                return Some(TaskStatus::Ready(
                                    HttpRequestRedirectResponse::Error(
                                        ClientRequestErrors::InvalidLocation,
                                    ),
                                ));
                            }
                        };

                    let new_descriptor =
                        match redirects::build_followup_request_from_request_descriptor(
                            &descriptor,
                            new_url.clone(),
                        ) {
                            Ok(desc) => desc,
                            Err(e) => {
                                tracing::error!(
                                    "Failed to build follow-up request descriptor: {}",
                                    e
                                );
                                self.0 = Some(HttpRequestRedirectState::Done);
                                return Some(TaskStatus::Ready(
                                    HttpRequestRedirectResponse::Error(
                                        ClientRequestErrors::InvalidState,
                                    ),
                                ));
                            }
                        };

                    tracing::debug!("Following redirect to new URL: {}", new_url);
                    self.0 = Some(HttpRequestRedirectState::Trying(Some(Box::new((
                        data,
                        timeout,
                        pool,
                        new_descriptor,
                        remaining_redirects - 1,
                    )))));
                    return Some(TaskStatus::Pending(HttpOperationState::Connecting));
                }

                tracing::info!(
                    "No redirect detected, transitioning to WriteBody state to send request body."
                );
                self.0 = Some(HttpRequestRedirectState::WriteBody(Some(Box::new((
                    data, pool, connection,
                )))));
                Some(TaskStatus::Pending(HttpOperationState::Connecting))
            }
            HttpRequestRedirectState::WriteBody(mut inner_opt) => {
                if let Some(inner) = inner_opt.take() {
                    let (data, _pool, mut connection) = *inner;
                    let body_renderer = Http11::request_body(data);

                    if let Err(err) = body_renderer.http_render_to_writer(connection.stream_mut()) {
                        tracing::error!("Failed to write request body: {}", err);

                        self.0 = Some(HttpRequestRedirectState::Done);
                        return Some(TaskStatus::Ready(HttpRequestRedirectResponse::Error(
                            ClientRequestErrors::WriteFailed,
                        )));
                    }

                    self.0 = Some(HttpRequestRedirectState::Done);
                    return match connection.stream_mut().flush() {
                        Ok(()) => Some(TaskStatus::Ready(HttpRequestRedirectResponse::Done(
                            connection,
                        ))),
                        Err(e) => Some(TaskStatus::Ready(
                            HttpRequestRedirectResponse::FlushFailed(connection, e),
                        )),
                    };
                }

                self.0 = Some(HttpRequestRedirectState::Done);
                Some(TaskStatus::Pending(HttpOperationState::Connecting))
            }
            HttpRequestRedirectState::Done => None,
        }
    }
}
