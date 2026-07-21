//! HTTP/3 proxy — terminates H3 from clients, forwards to backends over HTTP/1.1.
//!
//! WHY: The H3 analogue of [`crate::h2_proxy`]. `foundation_http` serves the H3
//! request (via the QUIC/H3 server layer) and hands this handler an
//! [`H3Request`]; the proxy forwards it upstream through the same pooled
//! `NativeHttpClient` the H1/H2 paths use — never a hand-rolled HTTP/1.1 — and
//! writes the response back as H3 HEADERS + DATA frames.
//!
//! HOW: read the request pseudo-headers + body from the `H3Request`, build a
//! client request, send it (blocking; runs on a valtron worker while the QUIC
//! driver pumps elsewhere), then stream the response back out as H3 frames.

#![cfg(all(feature = "quic", not(target_family = "wasm")))]

use std::io;
use std::sync::Arc;
use std::time::Duration;

use bytes::Bytes;
use foundation_core::valtron::Stream;
use foundation_netio::http::ClientRequestBuilder;
use foundation_netio::http3::H3Request;
use foundation_netio::netcap::ConnectionContext;
use foundation_netio::quic::QuinnBidiStream;
use foundation_netio::shared::client::body_reader::{
    SendSafeBodyBytesItem, SendSafeBodyBytesIterator,
};
use foundation_netio::shared::client::SystemDnsResolver;
use foundation_netio::shared::http::{SendSafeBody, SimpleHeader, SimpleHeaders, SimpleMethod};

use foundation_http::native::serve::{BoxFuture, H3Serve};
use foundation_http::shared::context::ContextBag;

use crate::forward::{forward_request_headers, strip_hop_by_hop, upstream_url, SharedHttpClient};
use crate::state::ProxyState;

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
            let fields = match read_headers(&mut request) {
                Some(f) => f,
                None => return Ok(()),
            };
            let method = pseudo(&fields, b":method").unwrap_or_default();
            let host = pseudo(&fields, b":authority").unwrap_or_default();
            let path = pseudo(&fields, b":path").unwrap_or_else(|| "/".to_string());
            let body = read_body(&mut request);

            // Route + pick a backend.
            let Some(service) = state.router().route(&host, &path) else {
                let _ = respond(&mut request, 404, &[], Bytes::new());
                return Ok(());
            };
            let Some((lease, _idx)) = service.pick_sticky(None) else {
                let _ = respond(&mut request, 503, &[], Bytes::new());
                return Ok(());
            };

            // Forward through the shared pooled client (same as H1/H2).
            let url = upstream_url(lease.backend().url(), &path);
            let mut headers = forward_request_headers(&request_headers(&fields), &client_ip, &scheme);
            headers.insert(SimpleHeader::HOST, vec![host]);

            let outcome = forward(&client, &method, &url, headers, body);
            drop(lease);

            match outcome {
                Ok((status, resp_headers, resp_body)) => {
                    let hdrs: Vec<(String, String)> = strip_hop_by_hop(&resp_headers)
                        .iter()
                        .flat_map(|(name, values)| {
                            let n = name.to_string();
                            values.iter().map(move |v| (n.clone(), v.clone()))
                        })
                        .collect();
                    let hdr_refs: Vec<(&str, &str)> =
                        hdrs.iter().map(|(n, v)| (n.as_str(), v.as_str())).collect();
                    let _ = respond(&mut request, status, &hdr_refs, resp_body);
                }
                Err(e) => {
                    tracing::warn!(%url, "H3 upstream forward failed: {e}");
                    let _ = respond(&mut request, 502, &[], Bytes::new());
                }
            }
            Ok(())
        })
    }
}

/// Blocking upstream forward via the shared client. Returns the response status,
/// headers, and fully-collected body.
fn forward(
    client: &SharedHttpClient,
    method: &str,
    url: &str,
    headers: SimpleHeaders,
    body: Vec<u8>,
) -> Result<(u16, SimpleHeaders, Bytes), String> {
    let builder = ClientRequestBuilder::<SystemDnsResolver>::new(method_of(method), url)
        .map_err(|e| format!("build upstream request: {e}"))?
        .headers(headers)
        .body(SendSafeBody::Bytes(body));
    let response = client
        .request(builder)
        .and_then(|req| req.send())
        .map_err(|e| format!("send upstream request: {e}"))?;
    let (status, resp_headers, resp_body, pool, conn) = response.into_parts();

    // Collect the response body via the shared body iterator (Content-Length,
    // chunked, or close-delimited — the client already handles all three).
    let mut collected = Vec::new();
    let mut chunks = SendSafeBodyBytesIterator::new(resp_body);
    loop {
        match chunks.next() {
            Some(Stream::Next(SendSafeBodyBytesItem::Chunk(bytes))) => collected.extend_from_slice(&bytes),
            Some(Stream::Next(SendSafeBodyBytesItem::StreamError(_))) => break,
            Some(Stream::Delayed(d)) => std::thread::sleep(d),
            Some(_) => std::thread::sleep(POLL_BACKOFF),
            None => break,
        }
    }

    if let (Some(pool), Some(mut stream)) = (pool, conn) {
        stream.drain_stream();
        pool.return_to_pool(stream);
    }

    let code = u16::try_from(Into::<usize>::into(status)).unwrap_or(502);
    Ok((code, resp_headers, Bytes::from(collected)))
}

// ── H3 stream helpers ─────────────────────────────────────────────────────

/// Read the request HEADERS field list off the stream.
fn read_headers(request: &mut H3Request<QuinnBidiStream>) -> Option<Vec<(Bytes, Bytes)>> {
    loop {
        match request.poll_headers() {
            Stream::Next(Ok(fields)) => return Some(fields),
            Stream::Next(Err(_)) => return None,
            _ => std::thread::sleep(POLL_BACKOFF),
        }
    }
}

/// Read the request body to end-of-stream.
fn read_body(request: &mut H3Request<QuinnBidiStream>) -> Vec<u8> {
    let mut body = Vec::new();
    loop {
        match request.poll_body() {
            Stream::Next(Ok(Some(chunk))) => body.extend_from_slice(&chunk),
            Stream::Next(Ok(None)) | Stream::Next(Err(_)) => return body,
            _ => std::thread::sleep(POLL_BACKOFF),
        }
    }
}

/// Write a full response (QPACK HEADERS + DATA + FIN) to the request stream.
fn respond(
    request: &mut H3Request<QuinnBidiStream>,
    status: u16,
    extra_headers: &[(&str, &str)],
    body: Bytes,
) -> io::Result<()> {
    let status_str = status.to_string();
    let mut fields: Vec<(&[u8], &[u8])> = vec![(b":status", status_str.as_bytes())];
    for (k, v) in extra_headers {
        fields.push((k.as_bytes(), v.as_bytes()));
    }
    let header_frame = H3Request::<QuinnBidiStream>::encode_headers(&fields);
    send_all(request, header_frame, true)?;

    if !body.is_empty() {
        let data_frame = H3Request::<QuinnBidiStream>::encode_data(body);
        send_all(request, data_frame, false)?;
    }

    let deadline = std::time::Instant::now() + Duration::from_secs(10);
    loop {
        match request.poll_finish() {
            Stream::Next(Ok(())) | Stream::Next(Err(_)) => return Ok(()),
            _ => {
                if std::time::Instant::now() >= deadline {
                    return Ok(());
                }
                std::thread::sleep(POLL_BACKOFF);
            }
        }
    }
}

/// Drain a fully-encoded frame to the send half.
fn send_all(
    request: &mut H3Request<QuinnBidiStream>,
    mut pending: Bytes,
    headers: bool,
) -> io::Result<()> {
    let deadline = std::time::Instant::now() + Duration::from_secs(10);
    loop {
        let step = if headers {
            request.poll_send_headers(&mut pending)
        } else {
            request.poll_send_data(&mut pending)
        };
        match step {
            Stream::Next(Ok(())) => return Ok(()),
            Stream::Next(Err(_)) => {
                return Err(io::Error::other("h3 send failed"));
            }
            _ => {
                if std::time::Instant::now() >= deadline {
                    return Err(io::Error::other("h3 send timed out"));
                }
                std::thread::sleep(POLL_BACKOFF);
            }
        }
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
