//! HTTP/2 proxy — terminates H2 from clients, forwards to backends.
//!
//! foundation_netio has the full H2 stack. The proxy terminates H2 on the
//! frontend and forwards via HTTP/1.1 to backends. Body chunks from H2 DATA
//! frames are written to the upstream using chunked transfer encoding.
//! Response chunks are read via chunked transfer decoding and encoded as
//! H2 DATA frames on the response pipe. No O(stream) buffering.

use std::io::{self, Read, Write};
use std::net::TcpStream;
use std::pin::Pin;
use std::sync::Arc;

use bytes::Bytes;
use foundation_core::valtron::{PipeReceiver, PipeSender, TryRecvError};
use foundation_netio::http2::types::{H2Frame, H2IncomingFrame, SimpleIncomingRequestHeader};
use foundation_netio::shared::http::{
    SimpleHeader, SimpleHeaders, SimpleMethod,
};
use foundation_http::native::serve::{BoxFuture, H2Serve};
use foundation_http::shared::context::ContextBag;

use crate::state::ProxyState;

pub struct H2ProxyHandler {
    state: Arc<ProxyState>,
}

impl H2ProxyHandler {
    #[must_use]
    pub fn new(state: Arc<ProxyState>) -> Self {
        Self { state }
    }
}

impl H2Serve for H2ProxyHandler {
    fn serve_h2(
        &self,
        _bag: Arc<ContextBag>,
        header: SimpleIncomingRequestHeader,
        body: PipeReceiver<H2IncomingFrame>,
        tx: PipeSender<H2Frame>,
    ) -> BoxFuture<'static, io::Result<()>> {
        let host = header.authority.clone();
        let path = header.url.url.clone();
        let method = header.method.clone();
        let scheme = header.scheme.clone();
        let client_ip = header
            .connection
            .peer_addr
            .as_ref()
            .map(|a| a.ip().to_string())
            .unwrap_or_else(|| "unknown".to_string());
        let state = self.state.clone();

        Box::pin(async move {
            let Some(service) = state.router().route(&host, &path) else {
                let _ = tx.send(h2_status(404, true));
                return Ok(());
            };
            let Some((_lease, _idx)) = service.pick_sticky(None) else {
                let _ = tx.send(h2_status(503, true));
                return Ok(());
            };

            let backend_url = _lease.backend().url().to_string();
            let upstream_url = build_upstream_url(&backend_url, &path);
            let authority = extract_authority(&backend_url);

            // Open TCP to backend, send request line + headers.
            let mut upstream = match TcpStream::connect(&authority) {
                Ok(s) => s,
                Err(e) => {
                    let _ = tx.send(h2_status(502, true));
                    tracing::warn!(%upstream_url, "H2 connect failed: {e}");
                    return Ok(());
                }
            };
            upstream.set_read_timeout(Some(std::time::Duration::from_secs(120))).ok();

            // Write request head.
            let _ = write!(upstream, "{} {} HTTP/1.1\r\n", method, upstream_url);
            let _ = write!(upstream, "Host: {}\r\n", host);
            let _ = write!(upstream, "X-Forwarded-For: {}\r\n", client_ip);
            let _ = write!(upstream, "X-Forwarded-Proto: {}\r\n", scheme);
            let _ = write!(upstream, "X-Forwarded-Host: {}\r\n", host);
            let _ = write!(upstream, "Connection: close\r\n");
            let _ = write!(upstream, "Transfer-Encoding: chunked\r\n");
            let _ = write!(upstream, "\r\n");
            let _ = upstream.flush();

            // Write body chunks from H2 pipe to upstream chunked body on a
            // dedicated thread (blocking I/O).
            let mut body_writer = upstream.try_clone().map_err(|e| io::Error::new(io::ErrorKind::Other, e))?;
            let body_done = Arc::new(std::sync::atomic::AtomicBool::new(false));
            let body_done_w = body_done.clone();
            std::thread::spawn(move || {
                loop {
                    match body.try_recv() {
                        Ok(H2IncomingFrame::Data(bytes)) => {
                            let _ = write!(body_writer, "{:x}\r\n", bytes.len());
                            let _ = body_writer.write_all(&bytes);
                            let _ = body_writer.write_all(b"\r\n");
                            let _ = body_writer.flush();
                        }
                        Ok(H2IncomingFrame::Reset(_)) => break,
                        Err(TryRecvError::Empty) => {
                            std::thread::sleep(std::time::Duration::from_millis(1));
                        }
                        Err(TryRecvError::Closed) => break,
                    }
                    if body_done_w.load(std::sync::atomic::Ordering::Relaxed) {
                        break;
                    }
                }
                let _ = body_writer.write_all(b"0\r\n\r\n");
                let _ = body_writer.flush();
            });

            // Read response from upstream, parse HTTP/1.1 → H2 frames.
            let result = read_upstream_response(&mut upstream, &tx);
            body_done.store(true, std::sync::atomic::Ordering::Relaxed);
            result
        })
    }
}

/// Read an HTTP/1.1 response from upstream, encode as H2 frames on tx.
/// Uses a single `buffer: Vec<u8>` to accumulate data, avoiding borrow conflicts.
fn read_upstream_response(
    upstream: &mut TcpStream,
    tx: &PipeSender<H2Frame>,
) -> io::Result<()> {
    let mut read_buf = [0u8; 8192];
    let mut buffer: Vec<u8> = Vec::new();  // single accumulation buffer
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
            if let Some(body_pos) = find_header_end(&buffer) {
                // Extract header bytes, parse, then drain the parsed portion.
                let status = parse_status_code(&buffer[..body_pos]).unwrap_or(502);
                let resp_headers = parse_response_headers(&buffer[..body_pos]);

                let mut h2_hdrs: Vec<(Bytes, Bytes)> = Vec::new();
                for (name, values) in resp_headers.iter() {
                    let nb = Bytes::from(name.to_string().into_bytes());
                    for v in values {
                        h2_hdrs.push((nb.clone(), Bytes::from(v.clone().into_bytes())));
                    }
                }

                // Drain: keep only body bytes after \r\n\r\n.
                buffer.drain(..body_pos + 4);
                headers_sent = true;

                let has_body = !buffer.is_empty();
                let _ = tx.send(H2Frame::Headers {
                    status: status as u16,
                    headers: h2_hdrs,
                    end_stream: !has_body,
                });
                if !has_body {
                    return Ok(());
                }
            }
        } else {
            // Chunked transfer decoding from `buffer`.
            loop {
                if let Some(size) = chunk_size {
                    if buffer.len() >= size + 2 {
                        let payload = buffer[..size].to_vec();
                        buffer.drain(..size + 2);
                        chunk_size = None;
                        if payload.is_empty() {
                            return Ok(());
                        }
                        let _ = tx.send(H2Frame::Data {
                            payload: Bytes::from(payload),
                            end_stream: false,
                        });
                    } else {
                        break;
                    }
                } else if let Some(line_end) = buffer.iter().position(|&b| b == b'\n') {
                    let hex_str = String::from_utf8_lossy(&buffer[..line_end]);
                    let size = usize::from_str_radix(hex_str.trim(), 16).unwrap_or(0);
                    buffer.drain(..line_end + 1);
                    if size == 0 {
                        let _ = tx.send(H2Frame::Data {
                            payload: Bytes::new(),
                            end_stream: true,
                        });
                        return Ok(());
                    }
                    chunk_size = Some(size);
                } else {
                    break;
                }
            }
        }
    }
}

fn find_header_end(data: &[u8]) -> Option<usize> {
    data.windows(4).position(|w| w == b"\r\n\r\n")
}

fn parse_status_code(header_bytes: &[u8]) -> Option<u16> {
    let line = std::str::from_utf8(header_bytes).ok()?.lines().next()?;
    line.split_whitespace().nth(1)?.parse().ok()
}

fn parse_response_headers(header_bytes: &[u8]) -> SimpleHeaders {
    let mut h = SimpleHeaders::new();
    let text = match std::str::from_utf8(header_bytes) {
        Ok(t) => t,
        Err(_) => return h,
    };
    for line in text.lines().skip(1) {
        if let Some((name, value)) = line.split_once(':') {
            let key = SimpleHeader::custom(name.trim());
            h.entry(key).or_insert_with(Vec::new).push(value.trim().to_string());
        }
    }
    h
}

fn h2_status(code: u16, end_stream: bool) -> H2Frame {
    H2Frame::Headers {
        status: code,
        headers: vec![],
        end_stream,
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
    without.split('/').next().unwrap_or("localhost:80").to_string()
}
