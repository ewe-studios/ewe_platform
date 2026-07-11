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

use crate::netcap::RawStream;
use crate::shared::client::{redirects, ClientConfig, DnsResolver};
use crate::simple_http::client::{HttpClientConnection, HttpConnectionPool};
use crate::simple_http::shared::{
    ensure_chunked_transfer_encoding, Http11, HttpClientError, HttpResponseReader,
    IncomingResponseParts, RenderHttp, RequestDescriptor, SendSafeBody, SimpleHeader,
    SimpleHttpBody, SimpleIncomingRequest, Status,
};
use foundation_core::io::ioutils::ReadTimeoutOperations;
use foundation_core::valtron::{BoxedSendExecutionAction, TaskIterator, TaskStatus};
use std::io::Write;
use std::sync::Arc;

use super::HttpOperationState;

// Type aliases for complex enum variant data
type InitData<R> = Box<(
    SimpleIncomingRequest,
    Arc<HttpConnectionPool<R>>,
    crate::simple_http::client::shared::ClientConfig,
    u8,
)>;

type TryingData<R> = Box<(
    SimpleIncomingRequest,
    Arc<HttpConnectionPool<R>>,
    crate::simple_http::client::shared::ClientConfig,
    RequestDescriptor,
    u8,
)>;

type WriteBodyData<R> = Box<(
    Option<[IncomingResponseParts; 2]>,
    SimpleIncomingRequest,
    Arc<HttpConnectionPool<R>>,
    HttpClientConnection,
    HttpResponseReader<SimpleHttpBody, RawStream>,
)>;

pub enum HttpRequestRedirectState<R: DnsResolver + Send + 'static> {
    Init(Option<InitData<R>>),
    Trying(Option<TryingData<R>>),
    WriteBody(Option<WriteBodyData<R>>),
    Done,
}

pub enum HttpRequestRedirectResponse {
    Done(
        HttpClientConnection,
        HttpResponseReader<SimpleHttpBody, RawStream>,
        Box<Option<[IncomingResponseParts; 2]>>,
    ),
    Error(HttpClientError),
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
        pool: Arc<HttpConnectionPool<R>>,
        config: ClientConfig,
        max_redirects: u8,
    ) -> Self {
        Self(Some(HttpRequestRedirectState::Init(Some(Box::new((
            data,
            pool,
            config,
            max_redirects,
        ))))))
    }
}

impl<R: DnsResolver + Send + 'static> TaskIterator for GetHttpRequestRedirectTask<R> {
    type Pending = HttpOperationState;
    type Ready = HttpRequestRedirectResponse;
    type Spawner = BoxedSendExecutionAction;

    #[allow(clippy::too_many_lines)]
    fn next_status(&mut self) -> Option<TaskStatus<Self::Ready, Self::Pending, Self::Spawner>> {
        let trace_span = tracing::info_span!("GetHttpRequestRedirectTask");
        trace_span.in_scope(|| {
            match self.0.take()? {
                HttpRequestRedirectState::Init(mut inner_opt) => {
                    tracing::trace!("HttpRequestRedirectState::Init");
                    if let Some(inner) = inner_opt.take() {
                        let (data, pool, config, remaining_redirects) = *inner;

                        // create the request descriptor
                        let request_descriptor = data.descriptor();
                        tracing::info!("HttpRequestRedirectState::Init -> Connecting to URL: {} with headers: {:?}", &request_descriptor.request_uri, &request_descriptor.headers);

                        tracing::info!("HttpRequestRedirectState::Init -> Going to Trying state with max redirects: {}", &remaining_redirects);

                        self.0 = Some(HttpRequestRedirectState::Trying(Some(Box::new((
                            data,
                            pool,
                            config,
                            request_descriptor,
                            remaining_redirects,
                        )))));

                        return Some(TaskStatus::Pending(HttpOperationState::Connecting));
                    }

                    self.0 = Some(HttpRequestRedirectState::Done);
                    None
                }
                HttpRequestRedirectState::Trying(inner_opt) => {
                    tracing::trace!("HttpRequestRedirectState::Trying");
                    let Some(state) = inner_opt else {
                        self.0 = Some(HttpRequestRedirectState::Done);
                        return None;
                    };


                    let (data, pool, config, mut descriptor, remaining_redirects) = *state;

                    // Calculate dynamic timeout based on request context
                    // For HEAD requests: use min timeout (no body expected)
                    // For other requests: use TTFB timeout for headers, dynamic for body

                    let read_timeout =
                        config.get_expect_continue_read_timeout();

                    // let is_head_request = matches!(data.method, crate::simple_http::shared::SimpleMethod::HEAD);
                    // let read_timeout = if is_head_request {
                    //     // HEAD requests have no body, use minimum timeout
                    //     config.get_expect_continue_read_timeout()
                    // } else {
                    //     // For other requests, use the calculated read timeout
                    //     // For header reading, we use a shorter TTFB-based timeout
                    //     config.timeout_calculator.config().ttfb_timeout
                    // };

                    tracing::debug!("Set read timeout to {:?} (method={:?})",
                        read_timeout, data.method);

                    // Determine effective proxy configuration
                    let env_proxy = if config.proxy_from_env {
                        crate::simple_http::client::ProxyConfig::from_env(
                            descriptor.request_uri.scheme(),
                        )
                    } else {
                        None
                    };

                    let proxy_config = env_proxy.as_ref().or({
                        config.proxy.as_ref()
                    });

                    // 1. Create connection (with proxy support)
                    let Ok(mut connection) =
                        pool.create_connection_with_proxy(&descriptor.request_uri, proxy_config, None)
                    else {
                        tracing::error!("Failed to connect with proxy");
                        self.0 = Some(HttpRequestRedirectState::Done);
                        return Some(TaskStatus::Ready(HttpRequestRedirectResponse::Error(
                            HttpClientError::ConnectionError,
                        )));
                    };

                    tracing::debug!("Retreived connection from pool");

                    // Only send Expect: 100-continue when the request has a body
                    // AND the handshake is enabled in config. Bodyless requests
                    // (e.g. GET) must never carry it — an interim `100 Continue`
                    // from the server would otherwise be mistaken for the final
                    // response. The toggle lets callers disable the handshake for
                    // servers/CDNs that handle it poorly.
                    let has_body = !matches!(data.body, None | Some(SendSafeBody::None));
                    let use_expect = has_body && config.expect_continue_enabled;
                    if use_expect {
                        tracing::debug!("Adding EXPECT: 100-continue header for request with body");
                        descriptor
                            .headers
                            .insert(SimpleHeader::EXPECT, vec!["100-continue".into()]);
                    } else {
                        tracing::debug!("Removing EXPECT: 100-continue header for request without body");
                        descriptor.headers.remove(&SimpleHeader::EXPECT);
                    }

                    // RFC 7230 §3.3.1 — the body renderer frames all three
                    // streaming variants (Stream/ChunkedStream/LineFeedStream) as
                    // chunked transfer encoding, so the head MUST advertise
                    // `Transfer-Encoding: chunked` (and carry no Content-Length).
                    // Without it a receiver finds neither framing header, treats
                    // the request as bodyless, drops the chunked bytes, and
                    // mishandles the Expect: 100-continue handshake above. The
                    // builders inject this too; do it here to cover descriptors
                    // assembled by other paths.
                    if matches!(
                        data.body,
                        Some(SendSafeBody::Stream(_))
                            | Some(SendSafeBody::ChunkedStream(_))
                            | Some(SendSafeBody::LineFeedStream(_))
                    ) {
                        ensure_chunked_transfer_encoding(&mut descriptor.headers);
                    }

                    tracing::debug!("Rendering and sending request");
                    // 2. Render and send request
                    let Ok(request_string) =
                        Http11::request_descriptor(descriptor.clone()).http_render_string()
                    else {
                        tracing::error!("Failed to render request");
                        self.0 = Some(HttpRequestRedirectState::Done);
                        return Some(TaskStatus::Ready(HttpRequestRedirectResponse::Error(
                            HttpClientError::InvalidState,
                        )));
                    };

                    if let Err(err) = connection.stream_mut().write_all(request_string.as_bytes()) {
                        tracing::error!("Failed to write request: {}", err);

                        self.0 = Some(HttpRequestRedirectState::Done);
                        return Some(TaskStatus::Ready(HttpRequestRedirectResponse::Error(
                            HttpClientError::WriteFailed,
                        )));
                    }

                    if let Err(err) = connection.stream_mut().flush() {
                        tracing::error!("Failed to write request: {}", err);
                        self.0 = Some(HttpRequestRedirectState::Done);
                        return Some(TaskStatus::Ready(HttpRequestRedirectResponse::Error(
                            HttpClientError::WriteFailed,
                        )));
                    }

                    // 3. Set read timeout based on request context
                    // For HEAD requests: use minimum timeout
                    // For other requests: use dynamic calculation
                    tracing::debug!("Set read timeout to {:?}", read_timeout);

                    if let Err(err) = connection
                        .stream_mut()
                        .set_read_timeout_as(read_timeout)
                    {
                        tracing::error!("Failed to set read timeout: {}", err);
                        self.0 = Some(HttpRequestRedirectState::Done);
                        return Some(TaskStatus::Ready(HttpRequestRedirectResponse::Error(
                            HttpClientError::Timeout,
                        )));
                    }

                    tracing::debug!("Get response reader from stream for 100-continue");

                    // 4. Try to read response intro once
                    let simple_http_body = config.into_simple_http_body();

                    let mut reader = HttpResponseReader::<SimpleHttpBody, RawStream>::new(
                        connection.clone_stream(),
                        simple_http_body,
                    );

                    // Body present but the Expect/100-continue handshake is disabled:
                    // the server is waiting for the body before it responds, so write
                    // it immediately without trying to read an interim response. The
                    // real response is read by the caller after WriteBody completes.
                    if has_body && !use_expect {
                        tracing::trace!("Body present, expect/100-continue disabled — writing body without probe");
                        self.0 = Some(HttpRequestRedirectState::WriteBody(Some(Box::new((
                            None, data, pool, connection, reader,
                        )))));
                        return Some(TaskStatus::Pending(HttpOperationState::Connecting));
                    }

                    // For requests without a body, skip the 100-continue probe but still
                    // read intro/headers to check for redirects before going to WriteBody.
                    if !has_body {
                        tracing::trace!("No body — skipping 100-continue probe, reading response directly");
                        let mut intro_result = reader.next();

                        // Defensively skip any interim `100 Continue` the server may
                        // emit unsolicited. Each interim is an Intro(Continue) followed
                        // by an (empty) Headers part; consume both and read the next
                        // intro so we land on the real final response.
                        let mut interim_guard = 0;
                        while matches!(
                            &intro_result,
                            Some(Ok(IncomingResponseParts::Intro(status, _, _))) if status == &Status::Continue
                        ) && interim_guard < 8
                        {
                            tracing::trace!("Skipping interim 100 Continue on no-body request");
                            let _ = reader.next(); // consume the interim headers
                            intro_result = reader.next();
                            interim_guard += 1;
                        }

                        if !matches!(
                            &intro_result,
                            Some(Ok(IncomingResponseParts::Intro(_, _, _)))
                        ) {
                            tracing::trace!("No intro response received with timeout");
                            self.0 = Some(HttpRequestRedirectState::Done);
                            return Some(TaskStatus::Ready(HttpRequestRedirectResponse::Error(
                                HttpClientError::Timeout,
                            )));
                        }

                        let headers_result = reader.next();
                        if !matches!(&headers_result, Some(Ok(IncomingResponseParts::Headers(_)))) {
                            tracing::error!("Headers not received");
                            self.0 = Some(HttpRequestRedirectState::Done);
                            return Some(TaskStatus::Ready(HttpRequestRedirectResponse::Error(
                                HttpClientError::Timeout,
                            )));
                        }

                        // Redirect check for no-body requests
                        let Some(Ok(IncomingResponseParts::Intro(status, _proto, _text))) =
                            &intro_result
                        else {
                            unreachable!()
                        };
                        let Some(Ok(IncomingResponseParts::Headers(headers))) = &headers_result
                        else {
                            unreachable!()
                        };

                        let is_redirect = (300..400).contains(&status.clone().into_usize());
                        let location_header = headers.get(&SimpleHeader::LOCATION).and_then(|v| v.first());

                        if is_redirect && location_header.is_some() {
                            if remaining_redirects == 0 {
                                tracing::error!("Redirect limit exceeded ({} redirects)", remaining_redirects);
                                self.0 = Some(HttpRequestRedirectState::Done);
                                return Some(TaskStatus::Ready(HttpRequestRedirectResponse::Error(
                                    HttpClientError::TooManyRedirects,
                                )));
                            }
                            let Some(location) = location_header else {
                                self.0 = Some(HttpRequestRedirectState::Done);
                                return Some(TaskStatus::Ready(HttpRequestRedirectResponse::Error(
                                    HttpClientError::FailedWith("Location header missing in redirect".into())
                                )));
                            };
                            let new_url =
                                match redirects::resolve_location(&descriptor.request_uri, location) {
                                    Ok(url) => url,
                                    Err(e) => {
                                        tracing::error!("Failed to resolve redirect location: {}", e);
                                        self.0 = Some(HttpRequestRedirectState::Done);
                                        return Some(TaskStatus::Ready(
                                            HttpRequestRedirectResponse::Error(
                                                HttpClientError::InvalidLocation(location.clone()),
                                            ),
                                        ));
                                    }
                                };

                            let new_descriptor =
                                match redirects::build_followup_request_from_request_descriptor(
                                    &descriptor,
                                    new_url.clone(),
                                    config.redirect.preserve_auth_on_redirect,
                                    config.redirect.preserve_cookies_on_redirect,
                                ) {
                                    Ok(desc) => {
                                        tracing::info!("Redirected to new location: {:?}", &desc);
                                        desc
                                    }
                                    Err(e) => {
                                        tracing::error!("Failed to build follow-up request descriptor: {}", e);
                                        self.0 = Some(HttpRequestRedirectState::Done);
                                        return Some(TaskStatus::Ready(
                                            HttpRequestRedirectResponse::Error(
                                                HttpClientError::InvalidState,
                                            ),
                                        ));
                                    }
                                };

                            tracing::debug!("Following redirect to new URL: {}", new_url);
                            self.0 = Some(HttpRequestRedirectState::Trying(Some(Box::new((
                                data,
                                pool,
                                config,
                                new_descriptor,
                                remaining_redirects - 1,
                            )))));
                            return Some(TaskStatus::Pending(HttpOperationState::Connecting));
                        }

                        tracing::debug!("No redirect detected for no-body request — skipping WriteBody");
                        let intro = intro_result.and_then(std::result::Result::ok).expect("intro checked above");
                        let headers = headers_result.and_then(std::result::Result::ok).expect("headers checked above");
                        // No body to write — done. Caller gets the connection, reader, and
                        // the intro+headers so it can read the response body.
                        self.0 = Some(HttpRequestRedirectState::Done);
                        return Some(TaskStatus::Ready(HttpRequestRedirectResponse::Done(
                            connection,
                            reader,
                            Box::new(Some([intro, headers])),
                        )));
                    }

                    // Flattened: check intro and headers one by one, fallback to WriteBody if either missing
                    tracing::trace!("[100-Continue] Waitiing for server response for 100-continue expect header with timeout: {:?}", read_timeout);
                    let intro_result = reader.next();
                    if !matches!(
                        &intro_result,
                        Some(Ok(IncomingResponseParts::Intro(_, _, _)))
                    ) {
                        tracing::trace!("No starter response received with timeout, moving to write body");
                        self.0 = Some(HttpRequestRedirectState::WriteBody(Some(Box::new((
                            None, data, pool, connection, reader,
                        )))));
                        return Some(TaskStatus::Pending(HttpOperationState::Connecting));
                    }

                    tracing::trace!("[100-continue, headers] Got intro response from server, moving to reading headers");
                    let headers_result = reader.next();
                    if !matches!(&headers_result, Some(Ok(IncomingResponseParts::Headers(_)))) {
                        tracing::error!("Headers not received, returning timeout");
                        self.0 = Some(HttpRequestRedirectState::Done);
                        return Some(TaskStatus::Ready(HttpRequestRedirectResponse::Error(
                            HttpClientError::Timeout,
                        )));
                    }

                    tracing::debug!("Received request response intro: {:?}", &intro_result);

                    // Both intro and headers are present
                    let (status, _proto, _text) = match &intro_result {
                        Some(Ok(IncomingResponseParts::Intro(status, proto, text))) => {
                            tracing::debug!("Received HTTP intro: status={}, proto={}, text={:?}", status, proto, text);
                            (status, proto, text)
                        }
                        _ => unreachable!("Intro must be present here due to prior matches! check; fallback to WriteBody if missing."),
                    };
                    let headers = match &headers_result {
                        Some(Ok(IncomingResponseParts::Headers(ref h))) => {
                            tracing::debug!("Received HTTP headers: {:?}", h);
                            h
                        }
                        _ => unreachable!("Headers must be present here due to prior matches! check; fallback to WriteBody if missing."),
                    };

                    let is_100_continue = status == &Status::Continue;
                    tracing::trace!("Is 100-continue: {}", is_100_continue);

                    if is_100_continue {
                        tracing::trace!("Moving to write body state for task: {}", is_100_continue);
                        self.0 = Some(HttpRequestRedirectState::WriteBody(Some(Box::new((
                            None,
                            data,
                            pool,
                            connection,
                            reader,
                        )))));

                        return Some(TaskStatus::Pending(HttpOperationState::Connecting));
                    }

                    // Extract Content-Length and calculate dynamic timeout for body reading.
                    // Only applied for non-100-Continue responses (actual response with body).
                    let content_length = headers_result.as_ref().and_then(|h| {
                        h.as_ref().ok().and_then(|parts| {
                            if let IncomingResponseParts::Headers(hdrs) = parts {
                                hdrs.get(&SimpleHeader::CONTENT_LENGTH)
                                    .and_then(|v| v.first())
                                    .and_then(|s| s.parse::<usize>().ok())
                            } else {
                                None
                            }
                        })
                    });

                    let body_read_timeout = config.calculate_read_timeout(content_length, false);
                    tracing::debug!("Setting body read timeout to {:?} for Content-Length {:?}",
                        body_read_timeout, content_length);

                    if let Err(err) = connection
                        .stream_mut()
                        .set_read_timeout_as(body_read_timeout)
                    {
                        tracing::error!("Failed to set body read timeout: {}", err);
                    }

                    tracing::trace!("No 100-continue, checking if redirect: {}", is_100_continue);
                    let is_redirect = (300..400).contains(&status.clone().into_usize());
                    tracing::debug!("Is redirect: {}", is_redirect);

                    let location_header = headers.get(&SimpleHeader::LOCATION).and_then(|v| v.first());
                    tracing::debug!("Location header: {:?}", location_header);

                    tracing::error!(
                        "Redirect limit at ({} redirects)",
                        remaining_redirects
                    );
                    if is_redirect && location_header.is_some() {
                        if remaining_redirects == 0 {
                            tracing::error!(
                                "Redirect limit exceeded ({} redirects)",
                                remaining_redirects
                            );
                            self.0 = Some(HttpRequestRedirectState::Done);
                            return Some(TaskStatus::Ready(HttpRequestRedirectResponse::Error(
                                HttpClientError::TooManyRedirects,
                            )));
                        }
                        tracing::info!(
                            "Redirect detected: status {} with Location header {:?}",
                            status,
                            location_header
                        );

                        let Some(location) = location_header else {
                            self.0 = Some(HttpRequestRedirectState::Done);
                            return Some(TaskStatus::Ready(HttpRequestRedirectResponse::Error(
                                HttpClientError::FailedWith("Location header missing in redirect".into())
                            )));
                        };
                        let new_url =
                            match redirects::resolve_location(&descriptor.request_uri, location) {
                                Ok(url) => url,
                                Err(e) => {
                                    tracing::error!("Failed to resolve redirect location: {}", e);
                                    self.0 = Some(HttpRequestRedirectState::Done);
                                    return Some(TaskStatus::Ready(
                                        HttpRequestRedirectResponse::Error(
                                            HttpClientError::InvalidLocation(location.clone()),
                                        ),
                                    ));
                                }
                            };

                        let new_descriptor =
                            match redirects::build_followup_request_from_request_descriptor(
                                &descriptor,
                                new_url.clone(),
                                config.redirect.preserve_auth_on_redirect,
                                config.redirect.preserve_cookies_on_redirect,
                            ) {
                                Ok(desc) => {
                                    tracing::info!("Redirected to new location: {:?}", &desc);
                                    desc
                                }
                                Err(e) => {
                                    tracing::error!(
                                        "Failed to build follow-up request descriptor: {}",
                                        e
                                    );
                                    self.0 = Some(HttpRequestRedirectState::Done);
                                    return Some(TaskStatus::Ready(
                                        HttpRequestRedirectResponse::Error(
                                            HttpClientError::InvalidState,
                                        ),
                                    ));
                                }
                            };

                        tracing::debug!("Following redirect to new URL: {}", new_url);
                        self.0 = Some(HttpRequestRedirectState::Trying(Some(Box::new((
                            data,
                            pool,
                            config,
                            new_descriptor,
                            remaining_redirects - 1,
                        )))));

                        return Some(TaskStatus::Pending(HttpOperationState::Connecting));
                    }

                    tracing::debug!(
                        "No redirect detected, transitioning to WriteBody state to send request body."
                    );

                    let Some(intro) = intro_result.and_then(std::result::Result::ok) else {
                        self.0 = Some(HttpRequestRedirectState::Done);
                        return Some(TaskStatus::Ready(HttpRequestRedirectResponse::Error(
                            HttpClientError::FailedWith("Missing intro".into())
                        )));
                    };

                    let Some(headers) = headers_result.and_then(std::result::Result::ok) else {
                        self.0 = Some(HttpRequestRedirectState::Done);
                        return Some(TaskStatus::Ready(HttpRequestRedirectResponse::Error(
                            HttpClientError::FailedWith("Missing headers".into())
                        )));
                    };

                    self.0 = Some(HttpRequestRedirectState::WriteBody(Some(Box::new((
                        Some([intro, headers]),
                        data,
                        pool,
                        connection,
                        reader,
                    )))));
                    Some(TaskStatus::Pending(HttpOperationState::Connecting))
                }
                HttpRequestRedirectState::WriteBody(mut inner_opt) => {
                    tracing::trace!("HttpRequestRedirectState::WriteBody");
                    if let Some(inner) = inner_opt.take() {
                        tracing::trace!("HttpRequestRedirectState::WriteBody: taking connection state data pointers");
                        let (optional_starters, data, _pool, mut connection, reader) = *inner;

                        // Guard: no body to write — skip rendering and go straight to Done.
                        if matches!(data.body, None | Some(SendSafeBody::None)) {
                            tracing::trace!("HttpRequestRedirectState::WriteBody: no body, skipping write");
                            self.0 = Some(HttpRequestRedirectState::Done);
                            return Some(TaskStatus::Ready(HttpRequestRedirectResponse::Done(
                                connection,
                                reader,
                                Box::new(optional_starters),
                            )));
                        }

                        let body_renderer = Http11::request_body(data);
                        tracing::trace!("HttpRequestRedirectState::WriteBody: creating body renderer and writing body to stream");

                        if let Err(err) = body_renderer.http_render_to_writer(connection.stream_mut()) {
                            tracing::error!("Failed to write request body: {}", err);

                            self.0 = Some(HttpRequestRedirectState::Done);
                            return Some(TaskStatus::Ready(HttpRequestRedirectResponse::Error(
                                HttpClientError::WriteFailed,
                            )));
                        }

                        tracing::trace!("HttpRequestRedirectState::WriteBody: written body to stream");
                        self.0 = Some(HttpRequestRedirectState::Done);
                        return match connection.stream_mut().flush() {
                            Ok(()) => {
                                tracing::trace!("HttpRequestRedirectState::WriteBody: flushed body to stream");
                                Some(TaskStatus::Ready(HttpRequestRedirectResponse::Done(
                                connection,
                                reader,
                                Box::new(optional_starters),
                            )))},
                            Err(e) => {
                                tracing::trace!("HttpRequestRedirectState::WriteBody: failed to flush body to stream due to: {:?}", e);

                                Some(TaskStatus::Ready(
                                HttpRequestRedirectResponse::FlushFailed(connection, e),
                            ))},
                        };
                    }

                    self.0 = Some(HttpRequestRedirectState::Done);
                    Some(TaskStatus::Pending(HttpOperationState::Done))
                }
                HttpRequestRedirectState::Done => None,
            }
        })
    }
}

