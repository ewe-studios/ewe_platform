//! HTTP request forwarding.
//!
//! WHY: The proxy is a reverse proxy: it must relay a client request to a
//! backend and relay the backend's response back, faithfully but with the
//! transformations a well-behaved proxy makes — strip hop-by-hop headers (they
//! describe a single TCP hop, not the end-to-end message), append
//! `X-Forwarded-For`, and set `X-Forwarded-Proto`/`X-Forwarded-Host` so the
//! backend can reconstruct the original request. WebSocket upgrades are relayed
//! by splicing the two TCP connections byte-for-byte after the handshake.
//!
//! WHAT: [`forward_http`] (ordinary requests, an async relay via the shared pooled client),
//! [`forward_upgrade`] (WebSocket/other `Upgrade`), plus the header helpers
//! [`strip_hop_by_hop`] and [`is_upgrade_request`].
//!
//! HOW: Ordinary requests go through `NativeHttpClient`, reusing its
//! `HttpConnectionPool` so upstream sockets are not reopened per request. The
//! response is rendered straight onto the client connection and the upstream
//! socket is returned to the pool. Upgrades bypass the client: the request is
//! re-serialized onto a raw upstream socket and both directions are spliced.

use std::collections::HashSet;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;

use foundation_core::io::ioutils::SharedByteBufferStream;
use foundation_http::native::writer::{ResponseWriter, WriterConfig};
use foundation_iogate::ServerIo;
use foundation_netio::http::{ClientRequestBuilder, NativeHttpClient};
use foundation_netio::netcap::RawStream;
use foundation_netio::shared::client::body_reader::{AsyncSendSafeBody, ReadWakeSource};
use foundation_netio::shared::client::SystemDnsResolver;
use foundation_netio::shared::http::{
    pushable_request_body_with_readiness, BodyReadiness, Http11, Http11Chunk, Proto,
    PushableRequestBody, RenderHttp, SendSafeBody, SimpleHeader, SimpleHeaders,
    SimpleIncomingRequest, SimpleMethod, SimpleResponse, Status, DEFAULT_PUSHABLE_DEPTH,
};
use futures_util::{FutureExt, StreamExt};

use crate::runtime::BackendRuntime;

/// The shared, pooled HTTP client type used for all upstream forwarding.
///
/// Native on purpose (group B of #140): the forwarders take the upstream
/// connection out of the `FinalizedResponse` and decide themselves whether it
/// goes back to the pool (only when the upstream keeps it alive). A
/// `DynNetClient` hides the connection, so it cannot express that.
pub type SharedHttpClient = NativeHttpClient<SystemDnsResolver>;

/// Hop-by-hop headers (RFC 7230 §6.1) that a proxy must not forward. Compared
/// case-insensitively against each header's canonical lowercase name.
const HOP_BY_HOP: &[&str] = &[
    "connection",
    "keep-alive",
    "proxy-authenticate",
    "proxy-authorization",
    "te",
    "trailer",
    "trailers",
    "transfer-encoding",
    "upgrade",
];

/// Whether the request is a protocol upgrade (WebSocket et al.).
///
/// True when `Connection` names `upgrade` and an `Upgrade` header is present.
#[must_use]
pub fn is_upgrade_request(req: &SimpleIncomingRequest) -> bool {
    let connection_upgrade = req
        .headers
        .get(&SimpleHeader::CONNECTION)
        .is_some_and(|vals| {
            vals.iter().any(|v| {
                v.split(',')
                    .any(|t| t.trim().eq_ignore_ascii_case("upgrade"))
            })
        });
    connection_upgrade && req.headers.contains_key(&SimpleHeader::UPGRADE)
}

/// Names to drop when forwarding: the fixed hop-by-hop set plus every token
/// listed in the message's own `Connection` header (RFC 7230 §6.1).
fn drop_set(headers: &SimpleHeaders) -> HashSet<String> {
    let mut names: HashSet<String> = HOP_BY_HOP.iter().map(|s| (*s).to_string()).collect();
    if let Some(values) = headers.get(&SimpleHeader::CONNECTION) {
        for value in values {
            for token in value.split(',') {
                let token = token.trim();
                if !token.is_empty() {
                    names.insert(token.to_ascii_lowercase());
                }
            }
        }
    }
    names
}

/// Return a copy of `headers` with all hop-by-hop headers removed.
///
/// Removes the fixed hop-by-hop set and anything named in `Connection`.
#[must_use]
pub fn strip_hop_by_hop(headers: &SimpleHeaders) -> SimpleHeaders {
    let drop = drop_set(headers);
    headers
        .iter()
        .filter(|(name, _)| !drop.contains(&name.to_string().to_ascii_lowercase()))
        .map(|(name, values)| (name.clone(), values.clone()))
        .collect()
}

/// Return a copy of `headers` with all hop-by-hop headers removed, **except** the
/// trailer handshake — `te: trailers`, `trailer`, and `trailers` — which a proxy
/// MUST forward for gRPC/RPC requests (RFC 9113 §8.2.2; the gRPC-over-HTTP/2 spec).
///
/// Used for the **request** direction only. Responses keep [`strip_hop_by_hop`]:
/// a response never carries a request `te`, and an upstream `Trailer` announcement
/// is stale once the body has been re-framed on the way back to the client.
#[must_use]
pub fn strip_hop_by_hop_request(headers: &SimpleHeaders) -> SimpleHeaders {
    let drop = drop_set(headers);
    headers
        .iter()
        .filter(|(name, values)| {
            let lower = name.to_string().to_ascii_lowercase();
            if !drop.contains(&lower) {
                return true;
            }
            // `te: trailers` is the one hop-by-hop header a proxy must forward
            // (gRPC depends on it). Preserve it when — and only when — it carries
            // the `trailers` token; any other `te` value stays stripped.
            if lower == "te" {
                return values.iter().any(|v| {
                    v.split(',')
                        .any(|t| t.trim().eq_ignore_ascii_case("trailers"))
                });
            }
            // The `trailer`/`trailers` announcement names are also preserved.
            lower == "trailer" || lower == "trailers"
        })
        .map(|(name, values)| (name.clone(), values.clone()))
        .collect()
}

/// Whether a request's `Content-Type` marks an RPC streaming exchange — gRPC,
/// Connect streaming, or gRPC-Web — whose response is always length- or
/// chunk-delimited (never EOF-delimited).
///
/// Such responses never need the upstream `Connection: close` that
/// [`forward_request_headers`] adds for plain requests; that header is hostile to
/// them, since it forces the backend to drop the connection after each stream and
/// defeats reuse for long-lived server-streaming.
#[must_use]
pub fn is_streaming_request(headers: &SimpleHeaders) -> bool {
    headers
        .get(&SimpleHeader::CONTENT_TYPE)
        .and_then(|values| values.first())
        .is_some_and(|ct| {
            ct == "application/grpc"
                || ct.starts_with("application/grpc+")
                || ct.starts_with("application/connect+")
                || ct.starts_with("application/grpc-web")
        })
}

/// Build the request headers to send upstream: hop-by-hop stripped, then
/// `X-Forwarded-For` appended and `X-Forwarded-Proto`/`-Host` set.
///
/// Shared with the HTTP/2 forward path ([`crate::h2_proxy`]) so both front-end
/// protocols apply an identical hop-by-hop and forwarding-header policy.
#[must_use]
pub fn forward_request_headers(
    original: &SimpleHeaders,
    client_ip: &str,
    scheme: &str,
) -> SimpleHeaders {
    let mut headers = strip_hop_by_hop_request(original);

    // X-Forwarded-For: append this hop's client IP to any existing chain.
    let xff = SimpleHeader::custom("x-forwarded-for");
    let mut chain: Vec<String> = headers
        .get(&xff)
        .map(|vals| {
            vals.iter()
                .flat_map(|v| v.split(','))
                .map(|s| s.trim().to_string())
                .collect()
        })
        .unwrap_or_default();
    chain.push(client_ip.to_string());
    headers.insert(xff, vec![chain.join(", ")]);

    // X-Forwarded-Proto: the scheme the client used to reach the proxy.
    headers.insert(
        SimpleHeader::custom("x-forwarded-proto"),
        vec![scheme.to_string()],
    );

    // X-Forwarded-Host: the Host the client asked for.
    if let Some(host) = original.get(&SimpleHeader::HOST).and_then(|v| v.first()) {
        headers.insert(SimpleHeader::custom("x-forwarded-host"), vec![host.clone()]);
    }

    // Ask the upstream to close after responding — but only for non-streaming
    // requests. Each proxied request is a single upstream exchange; without
    // `Connection: close`, origins that default to keep-alive (e.g. gunicorn)
    // hold the socket open and the client's body reader waits for an end-of-stream
    // that never comes. RPC streaming responses (gRPC / Connect streaming /
    // gRPC-Web) are always length- or chunk-delimited, so the header is
    // unnecessary there — and hostile to long-lived streams.
    if !is_streaming_request(original) {
        headers.insert(SimpleHeader::CONNECTION, vec!["close".to_string()]);
    }

    headers
}

/// Join a backend base URL with the request path+query.
pub(crate) fn upstream_url(backend_url: &str, path: &str) -> String {
    let base = backend_url.strip_suffix('/').unwrap_or(backend_url);
    if path.starts_with('/') {
        format!("{base}{path}")
    } else {
        format!("{base}/{path}")
    }
}

/// Why [`forward_http`] failed, and whether the client already has a byte of the
/// response.
#[derive(Debug)]
pub struct ForwardError {
    /// What failed.
    pub message: String,
    /// True when the response head is already on the wire. The caller must then
    /// only close the connection: a second response would corrupt the first.
    pub response_started: bool,
    /// The status the caller answers when `response_started` is false: `502`
    /// for an upstream failure, `413` for a request body over the limit.
    pub answer_status: u16,
}

impl std::fmt::Display for ForwardError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(&self.message)
    }
}

impl std::error::Error for ForwardError {}

impl ForwardError {
    pub(crate) fn before_response(message: String) -> Self {
        Self {
            message,
            response_started: false,
            answer_status: 502,
        }
    }

    pub(crate) fn too_large(limit: usize) -> Self {
        Self {
            message: format!("request body over the {limit} byte limit"),
            response_started: false,
            answer_status: 413,
        }
    }

    pub(crate) fn mid_response(message: String) -> Self {
        Self {
            message,
            response_started: true,
            answer_status: 502,
        }
    }
}

/// Forward an ordinary HTTP request to `backend` and relay the response,
/// injecting `extra_headers` into the response (Decision 23: writer-affinity
/// cookies, security headers, etc.).
///
/// WHAT: Sends the request through the pooled `client`, then streams the
/// upstream response onto `writer` with hop-by-hop headers stripped and
/// `Connection: close` set (the proxied client connection is single-shot).
///
/// HOW: Everything awaits. The upstream request is sent with `send_async`; the
/// upstream body is read as an [`AsyncSendSafeBody`] that parks on the upstream
/// socket's readiness; each piece is written with the [`ResponseWriter`], which
/// parks on the client socket's write readiness. No piece of the body is kept
/// after it is written, so a large body costs one chunk of memory (rule 4:
/// stream bodies). A body with a `Content-Length` keeps it; any other body is
/// sent `Transfer-Encoding: chunked` (HTTP/1.1 clients) or ends at the close
/// (HTTP/1.0 clients).
///
/// # Errors
/// [`ForwardError`] for any failure to reach the backend or write the response.
/// When `response_started` is false the caller should answer `502`; otherwise it
/// should only close the connection.
pub async fn forward_http(
    writer: &mut ResponseWriter,
    backend: &Arc<BackendRuntime>,
    mut req: SimpleIncomingRequest,
    ctx: &ForwardContext<'_>,
) -> Result<(), ForwardError> {
    let limit = ctx.max_request_body;
    // A declared length over the limit is refused before any byte is read
    // (nginx `client_max_body_size`).
    if limit != 0 {
        let declared = req
            .headers
            .get(&SimpleHeader::CONTENT_LENGTH)
            .and_then(|v| v.first())
            .and_then(|v| v.trim().parse::<usize>().ok());
        if declared.is_some_and(|len| len > limit) {
            return Err(ForwardError::too_large(limit));
        }
    }
    let body = req.body.take().unwrap_or(SendSafeBody::None);
    if matches!(body, SendSafeBody::None) {
        return exchange(
            writer,
            backend,
            req,
            SendSafeBody::None,
            None,
            ctx,
            &AtomicBool::new(false),
        )
        .await;
    }

    // The client's body is read as an async stream (parked on the client
    // socket's read readiness) and pushed through a bounded pipe into the
    // upstream request, so a slow upload holds no worker thread and no whole
    // body is buffered.
    let (producer, upstream_body, readiness) =
        pushable_request_body_with_readiness(DEFAULT_PUSHABLE_DEPTH);
    let pump = pump_request_body(body, ctx.client_read_wake.clone(), producer, limit).fuse();
    let started = AtomicBool::new(false);
    let exchange = exchange(
        writer,
        backend,
        req,
        upstream_body,
        Some(readiness),
        ctx,
        &started,
    )
    .fuse();
    futures_util::pin_mut!(pump, exchange);
    loop {
        futures_util::select! {
            done = exchange => return done,
            pumped = pump => {
                // A body that ends cleanly closed the pipe; the exchange goes on.
                // A body that failed or passed the limit must stop the exchange:
                // closing the pipe would look like a complete body to the
                // upstream. Dropping the exchange drops the upstream request.
                if let Err(e) = pumped {
                    let mut err = match e {
                        PumpError::TooLarge => ForwardError::too_large(limit),
                        PumpError::Read => ForwardError::before_response(
                            "client request body read failed".to_string(),
                        ),
                    };
                    err.response_started = started.load(Ordering::SeqCst);
                    return Err(err);
                }
            }
        }
    }
}

/// Why the request-body pump stopped early.
enum PumpError {
    /// The body is larger than the limit.
    TooLarge,
    /// The client connection failed while the body was read.
    Read,
}

/// Move the client's request body into the upstream request pipe, at most
/// `limit` bytes (0 = no limit). Ends the pipe when the body ends or fails, so
/// the upstream request finishes or fails and is never left waiting.
async fn pump_request_body(
    body: SendSafeBody,
    read_wake: Option<ReadWakeSource>,
    producer: PushableRequestBody,
    limit: usize,
) -> Result<(), PumpError> {
    let mut stream = AsyncSendSafeBody::from(body);
    if let Some(source) = read_wake {
        stream = stream.with_read_wake(source);
    }
    let mut total = 0usize;
    while let Some(piece) = stream.next().await {
        let bytes = piece.map_err(|e| {
            tracing::debug!("client request body read failed: {e}");
            PumpError::Read
        })?;
        total = total.saturating_add(bytes.len());
        if limit != 0 && total > limit {
            return Err(PumpError::TooLarge);
        }
        if producer.push(bytes).await.is_err() {
            // The upstream request is done with the body (it answered early).
            return Ok(());
        }
    }
    producer.close();
    Ok(())
}

/// What [`forward_http`] needs besides the writer, backend and request.
pub struct ForwardContext<'a> {
    /// The client's IP, appended to `X-Forwarded-For`.
    pub client_ip: &'a str,
    /// The scheme the client used, set as `X-Forwarded-Proto`.
    pub scheme: &'a str,
    /// The pooled upstream client.
    pub client: &'a SharedHttpClient,
    /// Headers added to the response (sticky cookie, ...).
    pub extra_headers: Option<&'a SimpleHeaders>,
    /// Registers a waker for read readiness of the client connection, so the
    /// request body read parks instead of spinning. `None` polls.
    pub client_read_wake: Option<ReadWakeSource>,
    /// The largest request body in bytes; 0 means no limit.
    pub max_request_body: usize,
}

/// Send the request upstream and relay the response. See [`forward_http`].
async fn exchange(
    writer: &mut ResponseWriter,
    backend: &Arc<BackendRuntime>,
    req: SimpleIncomingRequest,
    body: SendSafeBody,
    body_readiness: Option<BodyReadiness>,
    ctx: &ForwardContext<'_>,
    started: &AtomicBool,
) -> Result<(), ForwardError> {
    let (client_ip, scheme, client, extra_headers) =
        (ctx.client_ip, ctx.scheme, ctx.client, ctx.extra_headers);
    // `forward_request_headers` asks the upstream to `Connection: close` for
    // non-streaming requests (their single-shot exchange is EOF-delimited on the
    // way back). Such a connection must not be returned to the pool: the upstream
    // will drop it after responding, so a subsequent pooled send would time out.
    let reuse = is_streaming_request(&req.headers);
    let head_only = matches!(req.method, SimpleMethod::HEAD);
    let chunk_ok = matches!(req.proto, Proto::HTTP11);

    let url = upstream_url(backend.url(), &req.request_url.url);
    let headers = forward_request_headers(&req.headers, client_ip, scheme);

    let mut builder = ClientRequestBuilder::<SystemDnsResolver>::new(req.method.clone(), &url)
        .map_err(|e| {
            ForwardError::before_response(format!("build upstream request for {url}: {e}"))
        })?
        .headers(headers)
        .body(body);
    if let Some(readiness) = body_readiness {
        builder = builder.with_body_readiness(readiness);
    }

    let request = client
        .request(builder)
        .map_err(|e| ForwardError::before_response(format!("prepare upstream request: {e}")))?;

    let response = request.send_async().await.map_err(|e| {
        ForwardError::before_response(format!("send upstream request to {url}: {e}"))
    })?;

    let (status, headers, body, pool, upstream_conn) = response.into_parts();
    let read_wake = upstream_conn.as_ref().and_then(|c| c.read_wake_source());
    let mut body = AsyncSendSafeBody::from(body);
    if let Some(source) = read_wake {
        body = body.with_read_wake(source);
    }

    write_streamed_response(
        writer,
        status,
        &headers,
        body,
        &ResponseShape {
            extra_headers,
            head_only,
            chunk_ok,
        },
        started,
    )
    .await?;

    // Return the upstream socket to the pool for reuse only when the connection
    // is still alive (streaming requests). For non-streaming requests the
    // upstream was told to close, so dropping `upstream_conn` closes the socket.
    if reuse {
        if let (Some(pool), Some(mut stream)) = (pool, upstream_conn) {
            stream.drain_stream();
            pool.return_to_pool(stream);
        }
    }
    Ok(())
}

/// How a streamed response is framed on the client connection.
pub(crate) struct ResponseShape<'a> {
    /// Headers added after hop-by-hop stripping (the sticky cookie).
    pub extra_headers: Option<&'a SimpleHeaders>,
    /// The request was `HEAD`: send a head only.
    pub head_only: bool,
    /// The client speaks HTTP/1.1, so a body of unknown length may be chunked.
    pub chunk_ok: bool,
}

/// Write a response whose body is a stream, one piece at a time.
///
/// WHY: shared by the transparent relay and the gRPC-Web and Connect
/// translators, so every relayed response gets the same framing and the same
/// no-buffering behaviour.
///
/// WHAT: strips hop-by-hop headers, adds `shape.extra_headers`, sets
/// `Connection: close`, keeps a `Content-Length` the headers carry and otherwise
/// frames the body `chunked`. `started` is set just before the first byte is
/// written. The body is never held whole.
///
/// # Errors
/// [`ForwardError`]; `response_started` tells whether a byte was written.
pub(crate) async fn write_streamed_response<S, E>(
    writer: &mut ResponseWriter,
    status: Status,
    headers: &SimpleHeaders,
    mut body: S,
    shape: &ResponseShape<'_>,
    started: &AtomicBool,
) -> Result<(), ForwardError>
where
    S: futures_util::Stream<Item = Result<Vec<u8>, E>> + Unpin,
    E: std::fmt::Display,
{
    let code = status.clone().into_usize();
    let no_body = shape.head_only || matches!(code, 100..=199 | 204 | 304);
    let mut out = strip_hop_by_hop(headers);
    if let Some(extras) = shape.extra_headers {
        for (name, values) in extras {
            out.insert(name.clone(), values.clone());
        }
    }
    out.insert(SimpleHeader::CONNECTION, vec!["close".to_string()]);
    let chunked = !no_body && shape.chunk_ok && !out.contains_key(&SimpleHeader::CONTENT_LENGTH);
    if chunked {
        out.insert(SimpleHeader::TRANSFER_ENCODING, vec!["chunked".to_string()]);
    }

    let head = render_stream(Http11::ResponseHead(SimpleResponse::no_body(status, out)))
        .map_err(ForwardError::before_response)?;
    started.store(true, Ordering::SeqCst);
    writer
        .write_all(&head)
        .await
        .map_err(|e| ForwardError::before_response(format!("write response head: {e}")))?;

    if !no_body {
        while let Some(piece) = body.next().await {
            let bytes = piece
                .map_err(|e| ForwardError::mid_response(format!("read response body: {e}")))?;
            if bytes.is_empty() {
                // An empty chunk would end a chunked body early.
                continue;
            }
            let wire = if chunked {
                Http11Chunk::Chunked(bytes).render()
            } else {
                bytes
            };
            writer
                .write_all(&wire)
                .await
                .map_err(|e| ForwardError::mid_response(format!("write response body: {e}")))?;
        }
        if chunked {
            let end = render_stream(Http11::ResponseTrailers(SimpleHeaders::new()))
                .map_err(ForwardError::mid_response)?;
            writer
                .write_all(&end)
                .await
                .map_err(|e| ForwardError::mid_response(format!("write response end: {e}")))?;
        }
    }
    writer
        .flush()
        .await
        .map_err(|e| ForwardError::mid_response(format!("flush response: {e}")))
}

/// Render one part of a response (head, trailers) to wire bytes.
fn render_stream(part: Http11) -> Result<Vec<u8>, String> {
    let mut out = Vec::new();
    for piece in part
        .http_render()
        .map_err(|e| format!("render response: {e}"))?
    {
        out.extend_from_slice(&piece.map_err(|e| format!("render response: {e}"))?);
    }
    Ok(out)
}

/// Relay a WebSocket / `Upgrade` request by splicing the two sockets.
///
/// WHAT: Dials the backend, writes the re-serialized request head to it, then
/// relays bytes in both directions until either side closes: the backend
/// performs the `101` handshake and everything after is opaque frames.
///
/// HOW: Everything awaits. The dial parks on write readiness, the head write
/// parks through [`ResponseWriter`], and [`crate::aio::splice`] parks on read and
/// write readiness, so an open tunnel holds no thread.
///
/// # Errors
/// Returns `Err(String)` if the upstream cannot be dialed or the request head
/// cannot be written. The client connection is untouched then, so the caller can
/// still answer `502`.
#[allow(clippy::too_many_arguments)]
pub async fn forward_upgrade(
    conn: &SharedByteBufferStream<RawStream>,
    backend: &Arc<BackendRuntime>,
    req: &SimpleIncomingRequest,
    client_ip: &str,
    scheme: &str,
    io_mode: ServerIo,
    connect_timeout: std::time::Duration,
    writer_config: WriterConfig,
) -> Result<(), String> {
    let authority = backend.target().authority();
    // Dial through iogate (F50): `Completion` reads the upstream from the io_uring
    // inbox and writes via `IORING_OP_SEND`; `Std` is a plain non-blocking socket.
    let upstream = crate::aio::dial_tcp(&authority, io_mode, connect_timeout)
        .await
        .map_err(|e| format!("connect upstream {authority}: {e}"))?;

    // Preserve the upgrade-relevant headers (Connection/Upgrade must survive),
    // strip the rest of the hop-by-hop set, and add the forwarding headers.
    let mut headers = forward_request_headers(&req.headers, client_ip, scheme);
    if let Some(v) = req.headers.get(&SimpleHeader::CONNECTION) {
        headers.insert(SimpleHeader::CONNECTION, v.clone());
    }
    if let Some(v) = req.headers.get(&SimpleHeader::UPGRADE) {
        headers.insert(SimpleHeader::UPGRADE, v.clone());
    }

    let request_bytes =
        serialize_request_head(&req.method.to_string(), &req.request_url.url, &headers);
    let mut writer = ResponseWriter::new(upstream.clone(), writer_config);
    writer
        .write_all(request_bytes.as_bytes())
        .await
        .map_err(|e| format!("write upgrade request: {e}"))?;
    writer
        .flush()
        .await
        .map_err(|e| format!("flush upgrade request: {e}"))?;

    // From here the connection is opaque: splice both directions.
    crate::aio::splice(conn.clone(), upstream, writer_config).await;
    Ok(())
}

/// Serialize an HTTP/1.1 request head (request line + headers + blank line).
// Each header line is a short format; `format!` reads clearer than `write!`.
#[allow(clippy::format_push_string)]
fn serialize_request_head(method: &str, path: &str, headers: &SimpleHeaders) -> String {
    let mut out = format!("{method} {path} HTTP/1.1\r\n");
    for (name, values) in headers {
        for value in values {
            out.push_str(&format!("{name}: {value}\r\n"));
        }
    }
    out.push_str("\r\n");
    out
}
