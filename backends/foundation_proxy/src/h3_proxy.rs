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
use std::time::{Duration, Instant};

use bytes::Bytes;
use foundation_core::valtron::{sleep_async, Stream};
use foundation_netio::http::{ClientRequestBuilder, HttpClientConnection, HttpConnectionPool};
use foundation_netio::http3::connection::error_code;
use foundation_netio::http3::{H3Error, H3Request};
use foundation_netio::netcap::ConnectionContext;
use foundation_netio::quic::QuinnBidiStream;
use foundation_netio::shared::client::body_reader::AsyncSendSafeBody;
use foundation_netio::shared::client::SystemDnsResolver;
use foundation_netio::shared::http::{SendSafeBody, SimpleHeader, SimpleHeaders, SimpleMethod};
use futures_util::future::{select, Either};
use futures_util::StreamExt;

use foundation_http::native::serve::{BoxFuture, H3Serve};
use foundation_http::shared::context::ContextBag;

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
        _bag: Arc<ContextBag>,
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
            // Read the request head + body off the H3 stream.
            let fields = match read_headers(&mut request).await {
                Some(f) => f,
                None => return Ok(()),
            };
            let method = pseudo(&fields, b":method").unwrap_or_default();
            let host = pseudo(&fields, b":authority").unwrap_or_default();
            let path = pseudo(&fields, b":path").unwrap_or_else(|| "/".to_string());
            let body = read_body(&mut request).await;

            // Route + pick a backend.
            let Some(service) = state.router().route(&host, &path) else {
                let _ = respond(
                    &mut request,
                    404,
                    &[],
                    Bytes::new(),
                    state.writer_config().send_timeout,
                )
                .await;
                return Ok(());
            };
            let Some((lease, _idx)) = service.pick_sticky(None) else {
                let _ = respond(
                    &mut request,
                    503,
                    &[],
                    Bytes::new(),
                    state.writer_config().send_timeout,
                )
                .await;
                return Ok(());
            };

            // Forward through the shared pooled client (same as H1/H2).
            let url = upstream_url(lease.backend().url(), &path);
            let mut headers =
                forward_request_headers(&request_headers(&fields), &client_ip, &scheme);
            headers.insert(SimpleHeader::HOST, vec![host]);

            let send_timeout = state.writer_config().send_timeout;
            match forward(&client, &method, &url, headers, body).await {
                Ok(upstream) => {
                    relay_response(&mut request, upstream, state.h3_body_gauge(), send_timeout)
                        .await;
                }
                Err(e) => {
                    tracing::warn!(%url, "H3 upstream forward failed: {e}");
                    let _ = respond(&mut request, 502, &[], Bytes::new(), send_timeout).await;
                }
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
    body: Vec<u8>,
) -> Result<UpstreamResponse, String> {
    let builder = ClientRequestBuilder::<SystemDnsResolver>::new(method_of(method), url)
        .map_err(|e| format!("build upstream request: {e}"))?
        .headers(headers)
        .body(SendSafeBody::Bytes(body));
    let response = client
        .request(builder)
        .map_err(|e| format!("prepare upstream request: {e}"))?
        .send_async()
        .await
        .map_err(|e| format!("send upstream request: {e}"))?;
    let (status, resp_headers, resp_body, pool, conn) = response.into_parts();

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
    if let Err(e) = send_all(request, head, true, send_timeout).await {
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
        let sent = send_all(request, frame, false, send_timeout).await;
        gauge.sub(held);
        if let Err(e) = sent {
            tracing::debug!("H3 client stopped taking the body: {e}");
            request.reset(error_code::H3_REQUEST_CANCELLED);
            return;
        }
    }

    if finish(request, send_timeout).await.is_err() {
        return;
    }
    if let (Some(pool), Some(mut stream)) = (pool, conn) {
        stream.drain_stream();
        pool.return_to_pool(stream);
    }
}

// ── H3 stream helpers ─────────────────────────────────────────────────────

/// Read the request HEADERS field list off the stream.
async fn read_headers(request: &mut H3Request<QuinnBidiStream>) -> Option<Vec<(Bytes, Bytes)>> {
    loop {
        match request.poll_headers() {
            Stream::Next(Ok(fields)) => return Some(fields),
            Stream::Next(Err(_)) => return None,
            _ => sleep_async(POLL_BACKOFF).await,
        }
    }
}

/// Read the request body to end-of-stream.
async fn read_body(request: &mut H3Request<QuinnBidiStream>) -> Vec<u8> {
    let mut body = Vec::new();
    loop {
        match request.poll_body() {
            Stream::Next(Ok(Some(chunk))) => body.extend_from_slice(&chunk),
            Stream::Next(Ok(None)) | Stream::Next(Err(_)) => return body,
            _ => sleep_async(POLL_BACKOFF).await,
        }
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
    mut op: impl FnMut(&mut H3Request<QuinnBidiStream>) -> Stream<Result<T, H3Error>, ()>,
) -> Option<Result<T, H3Error>> {
    let deadline = Instant::now() + limit;
    loop {
        let remaining = deadline.checked_duration_since(Instant::now())?;
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
) -> io::Result<()> {
    let status_str = status.to_string();
    let mut fields: Vec<(&[u8], &[u8])> = vec![(b":status", status_str.as_bytes())];
    for (k, v) in extra_headers {
        fields.push((k.as_bytes(), v.as_bytes()));
    }
    let header_frame = H3Request::<QuinnBidiStream>::encode_headers(&fields);
    send_all(request, header_frame, true, send_timeout).await?;

    if !body.is_empty() {
        let data_frame = H3Request::<QuinnBidiStream>::encode_data(body);
        send_all(request, data_frame, false, send_timeout).await?;
    }
    finish(request, send_timeout).await
}

/// End the send half (FIN), waiting at most `send_timeout`.
async fn finish(
    request: &mut H3Request<QuinnBidiStream>,
    send_timeout: Duration,
) -> io::Result<()> {
    match park_op(request, send_timeout, H3Request::poll_finish).await {
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
) -> io::Result<()> {
    let sent = park_op(request, send_timeout, |r| {
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
