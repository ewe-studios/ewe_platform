//! HTTP/3 proxy — terminates H3 from clients, forwards to backends over HTTP/1.1.
//!
//! WHY: The H3 analogue of [`crate::h2_proxy`]. `foundation_http` serves the H3
//! request (via the QUIC/H3 server layer) and hands this handler an
//! [`H3Request`]; the proxy forwards it upstream through the same pooled
//! `NativeHttpClient` the H1/H2 paths use — never a hand-rolled HTTP/1.1 — and
//! writes the response back as H3 HEADERS + DATA frames.
//!
//! HOW: read the request pseudo-headers + body from the `H3Request`, build a
//! client request, send it (async; parks while the QUIC driver pumps elsewhere),
//! then stream the response back out as H3 frames: the HEADERS frame first, then
//! one DATA frame per chunk the upstream body yields. A chunk is sent with the
//! H3 stream's flow control (the send parks on the connection's waker while the
//! window is full) before the next one is read, so the proxy holds one chunk at
//! a time, never the whole body.

#![cfg(all(feature = "quic", not(target_family = "wasm")))]

use std::future::poll_fn;
use std::io;
use std::sync::Arc;
use std::task::Poll;
use std::time::Duration;

use bytes::Bytes;
use foundation_core::valtron::{sleep_async, Stream};
use foundation_netio::http::{ClientRequestBuilder, HttpClientConnection, HttpConnectionPool};
use foundation_netio::http3::connection::error_code;
use foundation_netio::http3::{H3Error, H3Request};
use foundation_netio::netcap::ConnectionContext;
use foundation_netio::quic::QuinnBidiStream;
use foundation_netio::shared::client::body_reader::AsyncSendSafeBody;
use foundation_netio::shared::client::SystemDnsResolver;
use foundation_netio::shared::http::{
    pushable_request_body_with_readiness, BodyReadiness, PushableRequestBody, SendSafeBody,
    SimpleHeader, SimpleHeaders, SimpleMethod, DEFAULT_PUSHABLE_DEPTH,
};
use futures_util::future::{select, Either};
use futures_util::{pin_mut, select, FutureExt, StreamExt};

use foundation_http::native::serve::{BoxFuture, H3Serve};
use foundation_http::shared::context::Ctx;

use crate::forward::{forward_request_headers, strip_hop_by_hop, upstream_url, SharedHttpClient};
use crate::state::{BufferedBytesGauge, ProxyState};

/// The wait, on the valtron timer, between two tries of an H3 stream operation
/// that has no waker yet (`poll_*` on the QUIC stream). A timer park, not a thread
/// sleep.
const POLL_BACKOFF: Duration = Duration::from_millis(1);

pub struct H3ProxyHandler {
    state: Arc<ProxyState>,
}

impl H3ProxyHandler {
    #[must_use]
    pub fn new(state: Arc<ProxyState>) -> Self {
        Self { state }
    }
}

impl H3Serve for H3ProxyHandler {
    fn serve_h3(
        &self,
        _ctx: Ctx,
        connection: Arc<ConnectionContext>,
        mut request: H3Request<QuinnBidiStream>,
    ) -> BoxFuture<'static, io::Result<()>> {
        let state = self.state.clone();
        let client = self.state.client().clone();
        let scheme = self.state.scheme().to_string();
        let client_ip = connection
            .peer_addr
            .as_ref()
            .map(|a| a.ip().to_string())
            .unwrap_or_else(|| "unknown".to_string());

        Box::pin(async move {
            let send_timeout = state.writer_config().send_timeout;
            let read_timeout = state.client_read_timeout();
            let limit = state.max_request_body();
            let ctx = state.ctx().clone();
            // Read the request head off the H3 stream.
            let Some(fields) =
                read_headers(&mut request, read_timeout, state.h3_body_gauge(), &ctx).await
            else {
                return Ok(());
            };
            let method = pseudo(&fields, b":method").unwrap_or_default();
            let host = pseudo(&fields, b":authority").unwrap_or_default();
            let path = pseudo(&fields, b":path").unwrap_or_else(|| "/".to_string());

            // Route + pick a backend.
            let Some(service) = state.router().route(&host, &path) else {
                let _ = respond(&mut request, 404, &[], Bytes::new(), send_timeout, &ctx).await;
                return Ok(());
            };
            let Some((lease, _idx)) = service.pick_sticky(None) else {
                let _ = respond(&mut request, 503, &[], Bytes::new(), send_timeout, &ctx).await;
                return Ok(());
            };

            // A declared length over the limit is refused before any byte is
            // read (nginx `client_max_body_size`).
            let request_headers = request_headers(&fields);
            if limit != 0
                && request_headers
                    .get(&SimpleHeader::CONTENT_LENGTH)
                    .and_then(|v| v.first())
                    .and_then(|v| v.trim().parse::<usize>().ok())
                    .is_some_and(|len| len > limit)
            {
                reject_too_large(&mut request, send_timeout, &ctx).await;
                return Ok(());
            }

            // Forward through the shared pooled client (same as H1/H2).
            let url = upstream_url(lease.backend().url(), &path);
            let mut headers = forward_request_headers(&request_headers, &client_ip, &scheme);
            headers.insert(SimpleHeader::HOST, vec![host]);

            // The first body read tells whether the request has a body at all: a
            // bodyless request is sent bodyless, not chunked.
            let first = match park_op(
                &mut request,
                read_timeout,
                &ctx,
                counted(state.h3_body_gauge(), H3Request::poll_body),
            )
            .await
            {
                Some(Ok(first)) => first,
                _ => {
                    request.reset(error_code::H3_REQUEST_CANCELLED);
                    return Ok(());
                }
            };
            let gauge = state.h3_body_gauge();
            let outcome = if let Some(first) = first {
                let (producer, body, readiness) =
                    pushable_request_body_with_readiness(DEFAULT_PUSHABLE_DEPTH);
                let (upstream, pumped) = {
                    let fwd =
                        forward(&client, &method, &url, headers, body, Some(readiness)).fuse();
                    let pump = pump_request_body(
                        &mut request,
                        first,
                        producer,
                        limit,
                        gauge,
                        read_timeout,
                        &ctx,
                    )
                    .fuse();
                    pin_mut!(fwd, pump);
                    let mut pump_done = false;
                    let result = loop {
                        select! {
                            upstream = fwd => break Ok(upstream),
                            pumped = pump => match pumped {
                                Ok(()) => pump_done = true,
                                Err(end) => break Err(end),
                            },
                        }
                    };
                    (result, pump_done)
                };
                if upstream.is_ok() && !pumped {
                    // The upstream answered before the upload ended: tell the
                    // client to stop sending (RFC 9114 section 4.1.1).
                    request.stop_sending(error_code::H3_NO_ERROR);
                }
                upstream
            } else {
                Ok(forward(&client, &method, &url, headers, SendSafeBody::None, None).await)
            };

            match outcome {
                Ok(Ok(upstream)) => {
                    relay_response(&mut request, upstream, gauge, send_timeout, &ctx).await;
                }
                Ok(Err(e)) => {
                    tracing::warn!(%url, "H3 upstream forward failed: {e}");
                    let _ = respond(&mut request, 502, &[], Bytes::new(), send_timeout, &ctx).await;
                }
                Err(BodyEnd::TooLarge) => {
                    reject_too_large(&mut request, send_timeout, &ctx).await;
                }
                Err(BodyEnd::Read) => request.reset(error_code::H3_REQUEST_CANCELLED),
            }
            drop(lease);
            Ok(())
        })
    }
}

/// The upstream response, head read and body not yet read.
struct UpstreamResponse {
    status: u16,
    headers: SimpleHeaders,
    body: AsyncSendSafeBody,
    /// The pool and connection the body reads from; the connection goes back to
    /// the pool only after a clean end of body.
    pool: Option<Arc<HttpConnectionPool<SystemDnsResolver>>>,
    conn: Option<HttpClientConnection>,
}

/// Send the request upstream through the shared client and return the response
/// as soon as its head is read. The body stays unread.
async fn forward(
    client: &SharedHttpClient,
    method: &str,
    url: &str,
    headers: SimpleHeaders,
    body: SendSafeBody,
    readiness: Option<BodyReadiness>,
) -> Result<UpstreamResponse, String> {
    let mut builder = ClientRequestBuilder::<SystemDnsResolver>::new(method_of(method), url)
        .map_err(|e| format!("build upstream request: {e}"))?
        .headers(headers)
        .body(body);
    if let Some(readiness) = readiness {
        builder = builder.with_body_readiness(readiness);
    }
    let response = client
        .request(builder)
        .map_err(|e| format!("prepare upstream request: {e}"))?
        .send_async()
        .await
        .map_err(|e| format!("send upstream request: {e}"))?;
    let (status, resp_headers, resp_body, pool, conn) = response.into_parts();

    // `send_async` hands the response socket back in blocking mode, but this leg
    // streams the body asynchronously. Keep the socket non-blocking so the body
    // read parks on the reactor (`read_wake_source`) instead of blocking this
    // valtron worker and every task it hosts (spec 70 feature 121).
    if let Some(conn) = conn.as_ref() {
        if let Err(e) = conn.set_nonblocking(true) {
            tracing::debug!(error = %e, "cannot set upstream response socket non-blocking");
        }
    }

    // The body is an async stream that parks on the upstream socket
    // (Content-Length, chunked, or close-delimited; the client handles all three).
    let mut chunks = AsyncSendSafeBody::from(resp_body);
    if let Some(source) = conn.as_ref().and_then(|c| c.read_wake_source()) {
        chunks = chunks.with_read_wake(source);
    }
    Ok(UpstreamResponse {
        status: u16::try_from(Into::<usize>::into(status)).unwrap_or(502),
        headers: resp_headers,
        body: chunks,
        pool,
        conn,
    })
}

/// Write the response head, then every upstream body chunk as a DATA frame, then
/// end the stream.
///
/// WHY: the body is never collected. Each chunk is read from the upstream, sent
/// with the H3 stream's flow control, and dropped before the next is read, so a
/// slow client slows the upstream read (back pressure) and the proxy holds one
/// chunk at a time. `gauge` counts the bytes held.
///
/// If the upstream fails after the head is sent, or the client stops taking data,
/// the stream is reset so the client sees a failure and not a short body that
/// looks complete, and the upstream connection is closed, not pooled.
async fn relay_response(
    request: &mut H3Request<QuinnBidiStream>,
    upstream: UpstreamResponse,
    gauge: &BufferedBytesGauge,
    send_timeout: Duration,
    ctx: &Ctx,
) {
    let UpstreamResponse {
        status,
        headers,
        mut body,
        pool,
        conn,
    } = upstream;
    let fields: Vec<(String, String)> = strip_hop_by_hop(&headers)
        .iter()
        .flat_map(|(name, values)| {
            let n = name.to_string();
            values.iter().map(move |v| (n.clone(), v.clone()))
        })
        .collect();
    let status_text = status.to_string();
    let mut head: Vec<(&[u8], &[u8])> = vec![(b":status", status_text.as_bytes())];
    head.extend(fields.iter().map(|(n, v)| (n.as_bytes(), v.as_bytes())));
    let head = H3Request::<QuinnBidiStream>::encode_headers(&head);
    if let Err(e) = send_all(request, head, true, send_timeout, ctx).await {
        tracing::debug!("H3 response head not sent: {e}");
        request.reset(error_code::H3_REQUEST_CANCELLED);
        return;
    }

    while let Some(piece) = body.next().await {
        let chunk = match piece {
            Ok(chunk) => chunk,
            Err(e) => {
                tracing::warn!("H3 upstream body failed after the head was sent: {e}");
                request.reset(error_code::H3_INTERNAL_ERROR);
                return;
            }
        };
        if chunk.is_empty() {
            continue;
        }
        let held = chunk.len();
        gauge.add(held);
        let frame = H3Request::<QuinnBidiStream>::encode_data(Bytes::from(chunk));
        let sent = send_all(request, frame, false, send_timeout, ctx).await;
        gauge.sub(held);
        if let Err(e) = sent {
            tracing::debug!("H3 client stopped taking the body: {e}");
            request.reset(error_code::H3_REQUEST_CANCELLED);
            return;
        }
    }

    if finish(request, send_timeout, ctx).await.is_err() {
        return;
    }
    if let (Some(pool), Some(mut stream)) = (pool, conn) {
        stream.drain_stream();
        pool.return_to_pool(stream);
    }
}

// ── H3 stream helpers ─────────────────────────────────────────────────────

/// Read the request HEADERS field list off the stream, waiting at most
/// `timeout` for it. The wait parks on the connection's waker.
async fn read_headers(
    request: &mut H3Request<QuinnBidiStream>,
    timeout: Duration,
    gauge: &BufferedBytesGauge,
    ctx: &Ctx,
) -> Option<Vec<(Bytes, Bytes)>> {
    match park_op(
        request,
        timeout,
        ctx,
        counted(gauge, H3Request::poll_headers),
    )
    .await
    {
        Some(Ok(fields)) => Some(fields),
        _ => None,
    }
}

/// Why the request body stopped before its end.
enum BodyEnd {
    /// The body is larger than `max_request_body`.
    TooLarge,
    /// The client stream failed or went idle past the read timeout.
    Read,
}

/// Move the client's request body into the upstream request pipe, starting with
/// `first`, at most `limit` bytes (0 = no limit). Closes the pipe at the end of
/// the body. Each chunk is held only while it is pushed; the pipe is bounded, so
/// a slow upstream slows the read (back pressure). `gauge` counts the bytes held.
async fn pump_request_body(
    request: &mut H3Request<QuinnBidiStream>,
    first: Bytes,
    producer: PushableRequestBody,
    limit: usize,
    gauge: &BufferedBytesGauge,
    read_timeout: Duration,
    ctx: &Ctx,
) -> Result<(), BodyEnd> {
    let mut chunk = first;
    let mut total = 0usize;
    loop {
        total = total.saturating_add(chunk.len());
        if limit != 0 && total > limit {
            return Err(BodyEnd::TooLarge);
        }
        let held = chunk.len();
        gauge.add(held);
        let pushed = producer.push(chunk).await;
        gauge.sub(held);
        if pushed.is_err() {
            // The upstream request is done with the body (it answered early).
            return Ok(());
        }
        chunk = loop {
            match park_op(
                request,
                read_timeout,
                ctx,
                counted(gauge, H3Request::poll_body),
            )
            .await
            {
                Some(Ok(Some(next))) if next.is_empty() => {}
                Some(Ok(Some(next))) => break next,
                Some(Ok(None)) => {
                    producer.close();
                    return Ok(());
                }
                _ => return Err(BodyEnd::Read),
            }
        };
    }
}

/// Answer `413` and tell the client to stop sending.
async fn reject_too_large(
    request: &mut H3Request<QuinnBidiStream>,
    send_timeout: Duration,
    ctx: &Ctx,
) {
    let _ = respond(request, 413, &[], Bytes::new(), send_timeout, ctx).await;
    request.stop_sending(error_code::H3_NO_ERROR);
}

/// Wrap `op` so each call counts as one poll of the client's stream on `gauge`.
fn counted<'a, T>(
    gauge: &'a BufferedBytesGauge,
    mut op: impl FnMut(&mut H3Request<QuinnBidiStream>) -> Stream<Result<T, H3Error>, ()> + 'a,
) -> impl FnMut(&mut H3Request<QuinnBidiStream>) -> Stream<Result<T, H3Error>, ()> + 'a {
    move |request| {
        gauge.note_poll();
        op(request)
    }
}

/// Run `op` on the request until it is not `Pending`, parking the task on the
/// connection's waker between tries. `None` means `limit` passed first.
///
/// A transport that cannot wake parks on the valtron timer for [`POLL_BACKOFF`]
/// per try instead. No call sleeps a thread.
async fn park_op<T>(
    request: &mut H3Request<QuinnBidiStream>,
    limit: Duration,
    ctx: &Ctx,
    mut op: impl FnMut(&mut H3Request<QuinnBidiStream>) -> Stream<Result<T, H3Error>, ()>,
) -> Option<Result<T, H3Error>> {
    let clock = ctx.clock();
    let deadline = clock.now() + limit;
    loop {
        let remaining = deadline.checked_duration_since(clock.now())?;
        // Register the waker before the try, so a change between the try and the
        // park wakes this task.
        let attempt = poll_fn(|cx| {
            let registered = request.register_waker(cx.waker());
            match op(request) {
                Stream::Next(result) => Poll::Ready(Some(result)),
                _ if registered => Poll::Pending,
                _ => Poll::Ready(None),
            }
        });
        match select(Box::pin(attempt), Box::pin(sleep_async(remaining))).await {
            Either::Left((Some(result), _)) => return Some(result),
            Either::Left((None, _)) => sleep_async(POLL_BACKOFF.min(remaining)).await,
            Either::Right(_) => return None,
        }
    }
}

/// Write a full response (QPACK HEADERS + DATA + FIN) to the request stream.
async fn respond(
    request: &mut H3Request<QuinnBidiStream>,
    status: u16,
    extra_headers: &[(&str, &str)],
    body: Bytes,
    send_timeout: Duration,
    ctx: &Ctx,
) -> io::Result<()> {
    let status_str = status.to_string();
    let mut fields: Vec<(&[u8], &[u8])> = vec![(b":status", status_str.as_bytes())];
    for (k, v) in extra_headers {
        fields.push((k.as_bytes(), v.as_bytes()));
    }
    let header_frame = H3Request::<QuinnBidiStream>::encode_headers(&fields);
    send_all(request, header_frame, true, send_timeout, ctx).await?;

    if !body.is_empty() {
        let data_frame = H3Request::<QuinnBidiStream>::encode_data(body);
        send_all(request, data_frame, false, send_timeout, ctx).await?;
    }
    finish(request, send_timeout, ctx).await
}

/// End the send half (FIN), waiting at most `send_timeout`.
async fn finish(
    request: &mut H3Request<QuinnBidiStream>,
    send_timeout: Duration,
    ctx: &Ctx,
) -> io::Result<()> {
    match park_op(request, send_timeout, ctx, H3Request::poll_finish).await {
        Some(Ok(())) => Ok(()),
        Some(Err(e)) => Err(io::Error::other(format!("h3 finish failed: {e}"))),
        None => Err(io::Error::other("h3 finish timed out")),
    }
}

/// Send one fully-encoded frame, with the stream's flow control: the send parks
/// while the peer's window is full and fails if `send_timeout` passes before the
/// whole frame is taken.
async fn send_all(
    request: &mut H3Request<QuinnBidiStream>,
    mut pending: Bytes,
    headers: bool,
    send_timeout: Duration,
    ctx: &Ctx,
) -> io::Result<()> {
    let sent = park_op(request, send_timeout, ctx, |r| {
        if headers {
            r.poll_send_headers(&mut pending)
        } else {
            r.poll_send_data(&mut pending)
        }
    })
    .await;
    match sent {
        Some(Ok(())) => Ok(()),
        Some(Err(e)) => Err(io::Error::other(format!("h3 send failed: {e}"))),
        None => Err(io::Error::other("h3 send timed out")),
    }
}

// ── field helpers ─────────────────────────────────────────────────────────

/// Extract a pseudo-header value (e.g. `:path`) from the H3 field list.
fn pseudo(fields: &[(Bytes, Bytes)], name: &[u8]) -> Option<String> {
    fields
        .iter()
        .find(|(k, _)| k.as_ref() == name)
        .map(|(_, v)| String::from_utf8_lossy(v).to_string())
}

/// Build a `SimpleHeaders` from the non-pseudo request fields (for forwarding).
fn request_headers(fields: &[(Bytes, Bytes)]) -> SimpleHeaders {
    let mut headers = SimpleHeaders::new();
    for (name, value) in fields {
        if name.first() == Some(&b':') {
            continue; // pseudo-header
        }
        let key = SimpleHeader::custom(String::from_utf8_lossy(name).as_ref());
        headers
            .entry(key)
            .or_default()
            .push(String::from_utf8_lossy(value).to_string());
    }
    headers
}

/// Parse an HTTP method string into a `SimpleMethod`.
fn method_of(m: &str) -> SimpleMethod {
    match m {
        "GET" => SimpleMethod::GET,
        "POST" => SimpleMethod::POST,
        "PUT" => SimpleMethod::PUT,
        "DELETE" => SimpleMethod::DELETE,
        "HEAD" => SimpleMethod::HEAD,
        "PATCH" => SimpleMethod::PATCH,
        "OPTIONS" => SimpleMethod::OPTIONS,
        other => SimpleMethod::Custom(other.to_string()),
    }
}
