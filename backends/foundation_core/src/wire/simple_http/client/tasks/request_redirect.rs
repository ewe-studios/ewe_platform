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
    Http11, HttpClientError, HttpResponseReader, IncomingResponseParts, RenderHttp,
    RequestDescriptor, SimpleHeader, SimpleHttpBody, SimpleIncomingRequest, Status,
};
use std::io::Write;
use std::sync::Arc;

use super::HttpOperationState;

// Type aliases for complex enum variant data
type InitData<R> = Box<(
    SimpleIncomingRequest,
    Arc<HttpConnectionPool<R>>,
    crate::wire::simple_http::client::ClientConfig,
    u8,
)>;

type TryingData<R> = Box<(
    SimpleIncomingRequest,
    Arc<HttpConnectionPool<R>>,
    crate::wire::simple_http::client::ClientConfig,
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
        config: crate::wire::simple_http::client::ClientConfig,
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
                    if let Some(inner) = inner_opt.take() {
                        let (data, pool, config, remaining_redirects) = *inner;

                        // create the request descriptor
                        let request_descriptor = data.descriptor();
                        tracing::info!("Connecting to URL: {}", &request_descriptor.request_uri);

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
                    let Some(state) = inner_opt else {
                        self.0 = Some(HttpRequestRedirectState::Done);
                        return None;
                    };


                    let (data, pool, config, mut descriptor, remaining_redirects) = *state;
                    let (_connect_timeout, read_timeout, _write_timeout) = config.get_op_timeout();

                    tracing::debug!("REDIRECTIONS: Remaining redirects: {}", remaining_redirects);

                    // Determine effective proxy configuration
                    let env_proxy = if config.proxy_from_env {
                        crate::wire::simple_http::client::ProxyConfig::from_env(
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
                        self.0 = Some(HttpRequestRedirectState::Done);
                        return Some(TaskStatus::Ready(HttpRequestRedirectResponse::Error(
                            HttpClientError::ConnectionError,
                        )));
                    };

                    tracing::debug!("Adding EXPECT: 100-continue header");

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

                    // 3. Set read timeout (preserve previous)
                    let previous_timeout = connection
                        .stream_mut()
                        .get_current_read_timeout()
                        .unwrap_or(None);

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

                    tracing::debug!("Get response reader from stream");

                    // 4. Try to read response intro once
                    let simple_http_body = config.into_simple_http_body();

                    let mut reader = HttpResponseReader::<SimpleHttpBody, RawStream>::new(
                        connection.clone_stream(),
                        simple_http_body,
                    );

                    // Flattened: check intro and headers one by one, fallback to WriteBody if either missing
                    let intro_result = reader.next();
                    if !matches!(
                        &intro_result,
                        Some(Ok(IncomingResponseParts::Intro(_, _, _)))
                    ) {
                        self.0 = Some(HttpRequestRedirectState::WriteBody(Some(Box::new((
                            None, data, pool, connection, reader,
                        )))));
                        return Some(TaskStatus::Pending(HttpOperationState::Connecting));
                    }

                    let headers_result = reader.next();
                    if !matches!(&headers_result, Some(Ok(IncomingResponseParts::Headers(_)))) {
                        self.0 = Some(HttpRequestRedirectState::Done);
                        return Some(TaskStatus::Ready(HttpRequestRedirectResponse::Error(
                            HttpClientError::Timeout,
                        )));
                    }

                    // Restore previous timeout
                    let _ = connection
                        .stream_mut()
                        .set_read_timeout_as(previous_timeout.unwrap_or(read_timeout));

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
                    tracing::debug!("Is 100-continue: {}", is_100_continue);

                    if is_100_continue {
                        self.0 = Some(HttpRequestRedirectState::WriteBody(Some(Box::new((
                            None,
                            data,
                            pool,
                            connection,
                            reader,
                        )))));

                        return Some(TaskStatus::Pending(HttpOperationState::Connecting));
                    }

                    let is_redirect = (300..400).contains(&status.clone().into_usize());
                    tracing::debug!("Is redirect: {}", is_redirect);

                    let location_header = headers.get(&SimpleHeader::LOCATION).and_then(|v| v.first());
                    tracing::debug!("Location header: {:?}", location_header);

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
                                config.preserve_auth_on_redirect,
                                config.preserve_cookies_on_redirect,
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
                    if let Some(inner) = inner_opt.take() {
                        let (optional_starters, data, _pool, mut connection, reader) = *inner;
                        let body_renderer = Http11::request_body(data);

                        if let Err(err) = body_renderer.http_render_to_writer(connection.stream_mut()) {
                            tracing::error!("Failed to write request body: {}", err);

                            self.0 = Some(HttpRequestRedirectState::Done);
                            return Some(TaskStatus::Ready(HttpRequestRedirectResponse::Error(
                                HttpClientError::WriteFailed,
                            )));
                        }

                        self.0 = Some(HttpRequestRedirectState::Done);
                        return match connection.stream_mut().flush() {
                            Ok(()) => Some(TaskStatus::Ready(HttpRequestRedirectResponse::Done(
                                connection,
                                reader,
                                Box::new(optional_starters),
                            ))),
                            Err(e) => Some(TaskStatus::Ready(
                                HttpRequestRedirectResponse::FlushFailed(connection, e),
                            )),
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
