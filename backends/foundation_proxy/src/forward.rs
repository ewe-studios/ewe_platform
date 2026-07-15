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
//! WHAT: [`forward_http`] (ordinary requests, via the shared pooled client),
//! [`forward_upgrade`] (WebSocket/other `Upgrade`), plus the header helpers
//! [`strip_hop_by_hop`] and [`is_upgrade_request`].
//!
//! HOW: Ordinary requests go through `NativeHttpClient`, reusing its
//! `HttpConnectionPool` so upstream sockets are not reopened per request. The
//! response is rendered straight onto the client connection and the upstream
//! socket is returned to the pool. Upgrades bypass the client: the request is
//! re-serialized onto a raw upstream socket and both directions are spliced.

use std::collections::HashSet;
use std::io::{ErrorKind, Write};
use std::net::ToSocketAddrs;
use std::sync::Arc;
use std::time::{Duration, Instant};

use foundation_core::io::ioutils::SharedByteBufferStream;
use foundation_iogate::ServerIo;
use foundation_netio::netcap::{Connection, RawStream};
use foundation_netio::http::{NativeHttpClient, ClientRequestBuilder};
use foundation_netio::shared::client::SystemDnsResolver;
use foundation_netio::shared::http::{
    Http11, RenderHttp, SendSafeBody, SimpleHeader, SimpleHeaders, SimpleIncomingRequest,
    SimpleOutgoingResponse, Status,
};

use crate::passthrough::splice_bidirectional;
use crate::runtime::BackendRuntime;

/// The shared, pooled HTTP client type used for all upstream forwarding.
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
            vals.iter()
                .any(|v| v.split(',').any(|t| t.trim().eq_ignore_ascii_case("upgrade")))
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

/// Build the request headers to send upstream: hop-by-hop stripped, then
/// `X-Forwarded-For` appended and `X-Forwarded-Proto`/`-Host` set.
fn forward_request_headers(
    original: &SimpleHeaders,
    client_ip: &str,
    scheme: &str,
) -> SimpleHeaders {
    let mut headers = strip_hop_by_hop(original);

    // X-Forwarded-For: append this hop's client IP to any existing chain.
    let xff = SimpleHeader::custom("x-forwarded-for");
    let mut chain: Vec<String> = headers
        .get(&xff)
        .map(|vals| vals.iter().flat_map(|v| v.split(',')).map(|s| s.trim().to_string()).collect())
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
        headers.insert(
            SimpleHeader::custom("x-forwarded-host"),
            vec![host.clone()],
        );
    }

    // Ask the upstream to close after responding. Each proxied request is a
    // single upstream exchange; without this, origins that default to
    // `Connection: keep-alive` (e.g. gunicorn) hold the socket open and the
    // client's body reader waits for an end-of-stream that never comes.
    headers.insert(SimpleHeader::CONNECTION, vec!["close".to_string()]);

    headers
}

/// Join a backend base URL with the request path+query.
fn upstream_url(backend_url: &str, path: &str) -> String {
    let base = backend_url.strip_suffix('/').unwrap_or(backend_url);
    if path.starts_with('/') {
        format!("{base}{path}")
    } else {
        format!("{base}/{path}")
    }
}

/// Forward an ordinary HTTP request to `backend` and relay the response.
///
/// WHAT: Sends the request through the pooled `client`, then renders the
/// upstream response back onto `conn` with hop-by-hop headers stripped and
/// `Connection: close` set (the proxied client connection is single-shot).
///
/// # Errors
/// Returns `Err(String)` describing any failure to reach the backend or write
/// the response; the caller logs it and closes the connection.
pub fn forward_http(
    conn: &mut SharedByteBufferStream<RawStream>,
    backend: &Arc<BackendRuntime>,
    req: SimpleIncomingRequest,
    client_ip: &str,
    scheme: &str,
    client: &SharedHttpClient,
) -> Result<(), String> {
    let url = upstream_url(backend.url(), &req.request_url.url);
    let headers = forward_request_headers(&req.headers, client_ip, scheme);
    let body = req.body.unwrap_or(SendSafeBody::None);

    let builder = ClientRequestBuilder::<SystemDnsResolver>::new(req.method.clone(), &url)
        .map_err(|e| format!("build upstream request for {url}: {e}"))?
        .headers(headers)
        .body(body);

    let request = client
        .request(builder)
        .map_err(|e| format!("prepare upstream request: {e}"))?;

    let response = request
        .send()
        .map_err(|e| format!("send upstream request to {url}: {e}"))?;

    let (status, headers, body, pool, upstream_conn) = response.into_parts();
    write_response(conn, status, &headers, body)?;

    // Return the upstream socket to the pool for reuse (drain any residue first),
    // mirroring FinalizedResponse's own drop-time pooling.
    if let (Some(pool), Some(mut stream)) = (pool, upstream_conn) {
        stream.drain_stream();
        pool.return_to_pool(stream);
    }
    Ok(())
}

/// Render a proxied response onto the client connection.
fn write_response(
    conn: &mut SharedByteBufferStream<RawStream>,
    status: Status,
    headers: &SimpleHeaders,
    body: SendSafeBody,
) -> Result<(), String> {
    let mut out = strip_hop_by_hop(headers);
    out.insert(SimpleHeader::CONNECTION, vec!["close".to_string()]);

    let response = SimpleOutgoingResponse::builder()
        .with_status(status)
        .with_headers(out)
        .with_body(body)
        .build()
        .map_err(|e| format!("build response: {e}"))?;

    Http11::response(response)
        .http_render_to_writer(conn)
        .map_err(|e| format!("write response: {e}"))?;
    conn.flush().map_err(|e| format!("flush response: {e}"))?;
    Ok(())
}

/// Relay a WebSocket / `Upgrade` request by splicing raw sockets.
///
/// WHAT: Re-serializes the request onto a fresh raw TCP socket to the backend,
/// then shuttles bytes in both directions until either side closes — the
/// backend performs the `101` handshake and everything after is opaque frames.
///
/// # Errors
/// Returns `Err(String)` if the upstream socket cannot be opened or the request
/// cannot be written.
pub fn forward_upgrade(
    conn: SharedByteBufferStream<RawStream>,
    backend: &Arc<BackendRuntime>,
    req: &SimpleIncomingRequest,
    client_ip: &str,
    scheme: &str,
    io_mode: ServerIo,
) -> Result<(), String> {
    let authority = backend.target().authority();
    let addr = authority
        .to_socket_addrs()
        .map_err(|e| format!("resolve upstream {authority}: {e}"))?
        .next()
        .ok_or_else(|| format!("no address for upstream {authority}"))?;
    // Dial through iogate (F50): `Completion` reads the upstream from the io_uring
    // inbox and writes via `IORING_OP_SEND`; `Std` is a plain non-blocking socket.
    let mut upstream = foundation_iogate::connect_completion(addr, io_mode)
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
    // The dialed upstream is non-blocking; write the head tolerating `WouldBlock`
    // (in `Completion` mode `flush` parks until the SEND's CQE lands).
    write_head_blocking(&mut upstream, request_bytes.as_bytes())?;

    // From here the connection is opaque: splice both directions.
    splice_bidirectional(conn, upstream, io_mode.uses_reactor());
    Ok(())
}

/// Write `bytes` in full to a non-blocking upstream and flush, retrying on
/// `WouldBlock` up to a bounded deadline. `flush` on a completion socket returns
/// `WouldBlock` until its SEND completes; on a plain socket it is a no-op.
fn write_head_blocking(upstream: &mut Connection, bytes: &[u8]) -> Result<(), String> {
    let deadline = Instant::now() + Duration::from_secs(5);
    let mut written = 0;
    while written < bytes.len() {
        match upstream.write(&bytes[written..]) {
            Ok(0) => return Err("upstream closed during upgrade write".to_string()),
            Ok(n) => written += n,
            Err(ref e) if e.kind() == ErrorKind::WouldBlock => {
                if Instant::now() > deadline {
                    return Err("upgrade request write timed out".to_string());
                }
                std::thread::sleep(Duration::from_millis(1));
            }
            Err(e) => return Err(format!("write upgrade request: {e}")),
        }
    }
    loop {
        match upstream.flush() {
            Ok(()) => return Ok(()),
            Err(ref e) if e.kind() == ErrorKind::WouldBlock => {
                if Instant::now() > deadline {
                    return Err("upgrade request flush timed out".to_string());
                }
                std::thread::sleep(Duration::from_millis(1));
            }
            Err(e) => return Err(format!("flush upgrade request: {e}")),
        }
    }
}

/// Serialize an HTTP/1.1 request head (request line + headers + blank line).
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
