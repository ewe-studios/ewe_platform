//! HTTP/3 (QUIC) proxy — terminates H3 from clients, forwards to backends.
//!
//! foundation_netio has the full QUIC + HTTP/3 stack (H3Connection, QuicDriver,
//! QuinnBidiStream), and foundation_http provides H3Serve. The proxy terminates
//! H3 on the frontend, forwards via HTTP/1.1 to backends, and encodes responses
//! as H3 frames back through the H3Request.
//!
//! Streaming: H3 request body chunks are written to the upstream via chunked
//! transfer encoding (one H3 DATA frame = one chunk). Upstream response chunks
//! are read via chunked transfer decoding and pushed as H3 DATA frames.

#[cfg(all(feature = "quic", not(target_family = "wasm")))]
use std::io::{self, Read, Write};
#[cfg(all(feature = "quic", not(target_family = "wasm")))]
use std::net::TcpStream;
#[cfg(all(feature = "quic", not(target_family = "wasm")))]
use std::sync::Arc;

#[cfg(all(feature = "quic", not(target_family = "wasm")))]
use bytes::Bytes;
#[cfg(all(feature = "quic", not(target_family = "wasm")))]
use foundation_netio::netcap::ConnectionContext;
#[cfg(all(feature = "quic", not(target_family = "wasm")))]
use foundation_netio::http3::connection::{H3Error, H3Request};
#[cfg(all(feature = "quic", not(target_family = "wasm")))]
use foundation_netio::quic::QuinnBidiStream;
#[cfg(all(feature = "quic", not(target_family = "wasm")))]
use foundation_netio::shared::http::{SimpleHeader, SimpleMethod};
#[cfg(all(feature = "quic", not(target_family = "wasm")))]
use foundation_http::native::serve::{BoxFuture, H3Serve};
#[cfg(all(feature = "quic", not(target_family = "wasm")))]
use foundation_http::shared::context::ContextBag;

#[cfg(all(feature = "quic", not(target_family = "wasm")))]
use foundation_core::valtron::{Stream, StreamIterator};

use crate::state::ProxyState;

#[cfg(all(feature = "quic", not(target_family = "wasm")))]
pub struct H3ProxyHandler {
    state: Arc<ProxyState>,
}

#[cfg(all(feature = "quic", not(target_family = "wasm")))]
impl H3ProxyHandler {
    #[must_use]
    pub fn new(state: Arc<ProxyState>) -> Self {
        Self { state }
    }

    /// Extract :authority pseudo-header from QPACK-decoded headers.
    fn authority_from_h3(headers: &[(Bytes, Bytes)]) -> String {
        headers
            .iter()
            .find(|(k, _)| k == &Bytes::from_static(b":authority"))
            .map(|(_, v)| String::from_utf8_lossy(v).to_string())
            .unwrap_or_default()
    }

    /// Extract :path pseudo-header.
    fn path_from_h3(headers: &[(Bytes, Bytes)]) -> String {
        headers
            .iter()
            .find(|(k, _)| k == &Bytes::from_static(b":path"))
            .map(|(_, v)| String::from_utf8_lossy(v).to_string())
            .unwrap_or_else(|| "/".to_string())
    }

    /// Extract :method pseudo-header.
    fn method_from_h3(headers: &[(Bytes, Bytes)]) -> SimpleMethod {
        headers
            .iter()
            .find(|(k, _)| k == &Bytes::from_static(b":method"))
            .map(|(_, v)| {
                let s = String::from_utf8_lossy(v);
                match s.as_ref() {
                    "GET" => SimpleMethod::GET,
                    "POST" => SimpleMethod::POST,
                    "PUT" => SimpleMethod::PUT,
                    "DELETE" => SimpleMethod::DELETE,
                    "PATCH" => SimpleMethod::PATCH,
                    "HEAD" => SimpleMethod::HEAD,
                    "OPTIONS" => SimpleMethod::OPTIONS,
                    _ => SimpleMethod::POST,
                }
            })
            .unwrap_or(SimpleMethod::GET)
    }
}

#[cfg(all(feature = "quic", not(target_family = "wasm")))]
impl H3Serve for H3ProxyHandler {
    fn serve_h3(
        &self,
        _bag: Arc<ContextBag>,
        _connection: Arc<ConnectionContext>,
        mut request: H3Request<QuinnBidiStream>,
    ) -> BoxFuture<'static, io::Result<()>> {
        let state = self.state.clone();

        Box::pin(async move {
            // 1. Poll headers from the H3 request (Stream-based).
            let h3_headers = poll_headers(&mut request)?;
            let host = Self::authority_from_h3(&h3_headers);
            let path = Self::path_from_h3(&h3_headers);
            let method = Self::method_from_h3(&h3_headers);

            // 2. Route through the shared router.
            let Some(service) = state.router().route(&host, &path) else {
                let status = Bytes::from_static(b":status");
                let val = Bytes::from_static(b"404");
                send_h3_response(&mut request, 404, &[])?;
                return Ok(());
            };
            let Some((_lease, _idx)) = service.pick_sticky(None) else {
                send_h3_response(&mut request, 503, &[])?;
                return Ok(());
            };

            // 3. Open TCP connection to backend.
            let backend_url = _lease.backend().url().to_string();
            let upstream_url = build_upstream_url(&backend_url, &path);
            let authority = extract_authority(&backend_url);

            let mut upstream = match TcpStream::connect(&authority) {
                Ok(s) => s,
                Err(e) => {
                    tracing::warn!(%upstream_url, "H3 connect failed: {e}");
                    send_h3_response(&mut request, 502, &[])?;
                    return Ok(());
                }
            };
            upstream
                .set_read_timeout(Some(std::time::Duration::from_secs(120)))
                .ok();

            // 4. Write request head to upstream.
            write!(
                upstream,
                "{} {} HTTP/1.1\r\n",
                method, upstream_url
            )?;
            write!(upstream, "Host: {host}\r\n")?;
            write!(upstream, "X-Forwarded-Proto: https\r\n")?;
            write!(upstream, "X-Forwarded-Host: {host}\r\n")?;
            write!(upstream, "Connection: close\r\n")?;
            write!(upstream, "Transfer-Encoding: chunked\r\n")?;
            write!(upstream, "\r\n")?;
            upstream.flush()?;

            // 5. Stream body from H3 → upstream as chunked. The H3 body poll is
            // blocking in this context — this runs on a dedicated valtron task.
            let mut body_writer = upstream
                .try_clone()
                .map_err(|e| io::Error::new(io::ErrorKind::Other, e))?;
            let body_done = Arc::new(std::sync::atomic::AtomicBool::new(false));
            let body_done_clone = body_done.clone();

            std::thread::spawn(move || {
                let mut buf = Bytes::new();
                loop {
                    let chunk = loop {
                        match request.poll_body() {
                            Stream::Next(Ok(Some(data))) => break data,
                            Stream::Next(Ok(None)) => {
                                // End of body.
                                let _ = body_writer.write_all(b"0\r\n\r\n");
                                let _ = body_writer.flush();
                                return;
                            }
                            Stream::Next(Err(_)) => {
                                let _ = body_writer.write_all(b"0\r\n\r\n");
                                return;
                            }
                            Stream::Pending(_) | Stream::Delayed(_) | Stream::Wait => {
                                std::thread::sleep(std::time::Duration::from_millis(1));
                            }
                            Stream::Init | Stream::Ignore => {
                                std::thread::sleep(std::time::Duration::from_millis(1));
                            }
                            Stream::Spread(_) => break Bytes::new(),
                            _ => {
                                // End of stream.
                                let _ = body_writer.write_all(b"0\r\n\r\n");
                                let _ = body_writer.flush();
                                return;
                            }
                        }
                        if body_done_clone.load(std::sync::atomic::Ordering::Relaxed) {
                            return;
                        }
                    };
                    let _ = write!(body_writer, "{:x}\r\n", chunk.len());
                    let _ = body_writer.write_all(&chunk);
                    let _ = body_writer.write_all(b"\r\n");
                    let _ = body_writer.flush();
                }
            });

            // 6. Read upstream response, encode as H3 frames.
            h3_read_upstream_response(&mut upstream, &mut request)?;
            body_done.store(true, std::sync::atomic::Ordering::Relaxed);
            Ok(())
        })
    }
}

#[cfg(all(feature = "quic", not(target_family = "wasm")))]
fn poll_headers(req: &mut H3Request<QuinnBidiStream>) -> io::Result<Vec<(Bytes, Bytes)>> {
    loop {
        match req.poll_headers() {
            Stream::Next(Ok(headers)) => return Ok(headers),
            Stream::Next(Err(_)) => return Err(io::Error::new(io::ErrorKind::Other, "H3 header error")),
            Stream::Spread(items) => {
                for item in items {
                    if let foundation_core::valtron::StreamSpread::Done(Ok(h)) = item {
                        return Ok(h);
                    }
                }
                return Err(io::Error::new(io::ErrorKind::Other, "H3 spread no headers"));
            }
            Stream::Pending(_) | Stream::Delayed(_) | Stream::Wait | Stream::Init | Stream::Ignore => {
                std::thread::sleep(std::time::Duration::from_millis(1));
            }
            _ => return Err(io::Error::new(io::ErrorKind::Other, "H3 stream ended without headers")),
        }
    }
}

#[cfg(all(feature = "quic", not(target_family = "wasm")))]
fn send_h3_response(
    req: &mut H3Request<QuinnBidiStream>,
    status: u16,
    extra_headers: &[(&str, &str)],
) -> io::Result<()> {
    let status_line = format!(":status\t{status}\r\n");
    let mut header_data = Bytes::from(status_line.into_bytes());
    for (k, v) in extra_headers {
        header_data.extend_from_slice(format!("{k}\t{v}\r\n").as_bytes());
    }
    // Encode as QPACK-style headers. For simplicity, use raw header bytes
    // prefixed with a simple encoding. The H3 poll_send_headers expects raw bytes
    // that the H3 layer QPACK-encodes.
    loop {
        match req.poll_send_headers(&mut header_data) {
            Stream::Next(Ok(())) => break,
            Stream::Next(Err(_)) => return Err(io::Error::new(io::ErrorKind::Other, "send headers error")),
            Stream::Pending(_) | Stream::Delayed(_) | Stream::Wait | Stream::Init | Stream::Ignore => {
                std::thread::sleep(std::time::Duration::from_millis(1));
            }
            _ => return Err(io::Error::new(io::ErrorKind::Other, "send headers failed")),
        }
    }
    loop {
        match req.poll_finish() {
            Stream::Next(Ok(())) => break,
            Stream::Next(Err(_)) => return Ok(()),
            Stream::Pending(_) | Stream::Delayed(_) | Stream::Wait | Stream::Init | Stream::Ignore => {
                std::thread::sleep(std::time::Duration::from_millis(1));
            }
            _ => return Ok(()),
        }
    }
    Ok(())
}

#[cfg(all(feature = "quic", not(target_family = "wasm")))]
fn h3_read_upstream_response(
    upstream: &mut TcpStream,
    req: &mut H3Request<QuinnBidiStream>,
) -> io::Result<()> {
    let mut read_buf = [0u8; 8192];
    let mut buffer: Vec<u8> = Vec::new();
    let mut headers_sent = false;
    let mut chunk_size: Option<usize> = None;

    loop {
        let n = match upstream.read(&mut read_buf) {
            Ok(0) => return Ok(()),
            Ok(n) => n,
            Err(ref e) if e.kind() == io::ErrorKind::WouldBlock => {
                std::thread::sleep(std::time::Duration::from_millis(5));
                continue;
            }
            Err(e) => return Err(e),
        };
        buffer.extend_from_slice(&read_buf[..n]);

        if !headers_sent {
            if let Some(body_pos) = find_hdr_end(&buffer) {
                let status = parse_status(&buffer[..body_pos]).unwrap_or(502);
                let mut header_data = Bytes::from(format!(":status\t{status}\r\n").into_bytes());
                // Forward other response headers.
                if let Ok(text) = std::str::from_utf8(&buffer[..body_pos]) {
                    for line in text.lines().skip(1) {
                        if let Some((name, value)) = line.split_once(':') {
                            header_data
                                .extend_from_slice(format!("{name}\t{}\r\n", value.trim()).as_bytes());
                        }
                    }
                }
                buffer.drain(..body_pos + 4);
                headers_sent = true;

                let has_body = !buffer.is_empty();
                loop {
                    match req.poll_send_headers(&mut header_data) {
                        Stream::Next(Ok(())) => break,
                        Stream::Next(Err(_)) => return Err(io::Error::new(io::ErrorKind::Other, "h3 send hdr err")),
                        Stream::Pending(_) | Stream::Delayed(_) | Stream::Wait | Stream::Init | Stream::Ignore => {
                            std::thread::sleep(std::time::Duration::from_millis(1));
                        }
                        _ => return Ok(()),
                    }
                }
                if !has_body {
                    return finish_h3(req);
                }
            }
        } else {
            // Chunked transfer decoding.
            loop {
                if let Some(size) = chunk_size {
                    if buffer.len() >= size + 2 {
                        let mut payload = Bytes::from(buffer[..size].to_vec());
                        buffer.drain(..size + 2);
                        chunk_size = None;
                        let is_empty = payload.is_empty();
                        if !is_empty {
                            loop {
                                match req.poll_send_data(&mut payload) {
                                    Stream::Next(Ok(())) => break,
                                    Stream::Next(Err(_)) => return Err(io::Error::new(io::ErrorKind::Other, "h3 send data err")),
                                    Stream::Pending(_) | Stream::Delayed(_) | Stream::Wait | Stream::Init | Stream::Ignore => {
                                        std::thread::sleep(std::time::Duration::from_millis(1));
                                    }
                                    _ => return Ok(()),
                                }
                            }
                        }
                        if is_empty {
                            return finish_h3(req);
                        }
                    } else {
                        break;
                    }
                } else if let Some(line_end) = buffer.iter().position(|&b| b == b'\n') {
                    let hex = String::from_utf8_lossy(&buffer[..line_end]);
                    let size = usize::from_str_radix(hex.trim(), 16).unwrap_or(0);
                    buffer.drain(..line_end + 1);
                    if size == 0 {
                        return finish_h3(req);
                    }
                    chunk_size = Some(size);
                } else {
                    break;
                }
            }
        }
    }
}

#[cfg(all(feature = "quic", not(target_family = "wasm")))]
fn find_hdr_end(data: &[u8]) -> Option<usize> {
    data.windows(4).position(|w| w == b"\r\n\r\n")
}

#[cfg(all(feature = "quic", not(target_family = "wasm")))]
fn parse_status(data: &[u8]) -> Option<u16> {
    let line = std::str::from_utf8(data).ok()?.lines().next()?;
    line.split_whitespace().nth(1)?.parse().ok()
}

#[cfg(all(feature = "quic", not(target_family = "wasm")))]
fn finish_h3(req: &mut H3Request<QuinnBidiStream>) -> io::Result<()> {
    loop {
        match req.poll_finish() {
            Stream::Next(Ok(())) => return Ok(()),
            Stream::Next(Err(_)) => return Ok(()),
            Stream::Pending(_) | Stream::Delayed(_) | Stream::Wait | Stream::Init | Stream::Ignore => {
                std::thread::sleep(std::time::Duration::from_millis(1));
            }
            _ => return Ok(()),
        }
    }
}

fn build_upstream_url(backend_url: &str, path: &str) -> String {
    if backend_url.ends_with('/') {
        format!("{}{path}", &backend_url[..backend_url.len() - 1])
    } else {
        format!("{backend_url}{path}")
    }
}

fn extract_authority(url: &str) -> String {
    let without = url
        .strip_prefix("http://")
        .or_else(|| url.strip_prefix("https://"))
        .unwrap_or(url);
    without
        .split('/')
        .next()
        .unwrap_or("localhost:80")
        .to_string()
}
