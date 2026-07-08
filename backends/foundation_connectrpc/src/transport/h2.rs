//! Native HTTP/2 `Transport` implementation (Feature 30/31).
//!
//! WHY: `Transport` byte-level client-seam contract for HTTP/2 cleartext (h2c
//! prior-knowledge). `open()` connects over TCP, runs the h2 handshake, spawns
//! a pump OS-thread, and returns `TransportStream` synchronously.
//!
//! WHAT: [`H2Transport`] wraps nothing — it is a stateless factory. Each `open()`
//! opens a fresh TCP connection, negotiates h2, and runs the entire exchange on
//! a dedicated stdlib thread. The pump reads request bytes from the `send_body`
//! pipe (via `try_recv` spin), frames them as h2 DATA, then reads h2 response
//! frames and pushes HEADERS→head pipe, DATA→body pipe.
//!
//! HOW: The h2 module uses blocking `Read + Write`; a thread-per-exchange pump
//! is the natural model. A valtron-native non-blocking pump is deferred.

use std::io;
use std::net::TcpStream;
use std::sync::Arc;
use std::thread;
use std::time::Duration;

use bytes::Bytes;
use foundation_core::valtron::{Pipe, PipeSender, PipeReceiver, TryRecvError};
use foundation_netio::simple_http::shared::{
    Proto, RequestDescriptor, SimpleHeader, SimpleHeaders, SimpleMethod, Status,
};
use foundation_netio::http2::connection::{H2Connection, H2Request};

use super::{
    body_stream_from_pipe, head_stream_from_pipe, BodyStream, ByteSink, HeadStream,
    Transport, TransportCapabilities, TransportError, TransportStream,
};

const CONNECT_TIMEOUT: Duration = Duration::from_secs(10);

/// HTTP/2 cleartext (h2c prior-knowledge) transport — one connection per call.
#[derive(Clone, Default)]
pub struct H2Transport;

impl H2Transport {
    #[must_use]
    pub fn new() -> Self { Self }
}

impl Transport for H2Transport {
    fn capabilities(&self) -> TransportCapabilities {
        TransportCapabilities {
            request_streaming: true,
            full_duplex: false,
            h2_trailers: true,
            http_versions: &[Proto::HTTP20],
            multiplexed: false,
        }
    }

    fn open(&self, request: RequestDescriptor) -> Result<TransportStream, TransportError> {
        let host = request.request_uri.host_str().map(|s| s.to_string()).unwrap_or_else(|| "localhost".into());
        let port = request.request_uri.port_or_default();
        let path = request.request_uri.path().to_string();
        let scheme = "http".to_string();
        let method = request.method.clone();
        let req_headers = request.headers.clone();

        let addr = format!("{host}:{port}");
        let stream = TcpStream::connect_timeout(
            &addr.parse().unwrap(),
            CONNECT_TIMEOUT,
        ).map_err(|e| TransportError::Connect(Arc::new(e)))?;

        // ── h2 handshake (synchronous, on caller's thread) ───────────────
        let mut conn = H2Connection::new(stream, false);
        conn.client_handshake().map_err(|e| {
            TransportError::Connect(Arc::new(io::Error::new(io::ErrorKind::Other, e.to_string())))
        })?;

        // ── Pipes (Pipe::new returns (sender, receiver)) ─────────────────
        let (send_tx, send_rx) = Pipe::<Bytes>::new();
        let (head_tx, head_rx) = Pipe::<(Status, SimpleHeaders)>::new();
        let (body_tx, body_rx) = Pipe::<Bytes>::new();

        // ── Pump thread ──────────────────────────────────────────────────
        thread::spawn(move || {
            let result = run_pump(&mut conn, send_rx, &head_tx, &body_tx,
                &method, &scheme, &host, port, &path, &req_headers);
            if let Err(e) = result {
                let _ = head_tx.try_send((Status::BadGateway, SimpleHeaders::new()));
                let _ = body_tx.try_send(Bytes::from(format!("h2 pump error: {e}")));
            }
            head_tx.close();
            body_tx.close();
        });

        let head: HeadStream = head_stream_from_pipe(head_rx);
        let recv_body: BodyStream = body_stream_from_pipe(body_rx);

        Ok(TransportStream { send_body: send_tx, head, recv_body })
    }
}

/// Map a u16 status code to a Status variant (best-effort).
fn status_from_code(code: u16) -> Status {
    match code {
        200 => Status::OK,
        201 => Status::Created,
        204 => Status::NoContent,
        301 => Status::MovedPermanently,
        302 => Status::Found,
        304 => Status::NotModified,
        400 => Status::BadRequest,
        401 => Status::Unauthorized,
        403 => Status::Forbidden,
        404 => Status::NotFound,
        405 => Status::MethodNotAllowed,
        408 => Status::RequestTimeout,
        429 => Status::TooManyRequests,
        500 => Status::InternalServerError,
        502 => Status::BadGateway,
        503 => Status::ServiceUnavailable,
        _ => Status::InternalServerError,
    }
}

/// Run the h2 exchange on the pump thread.
fn run_pump(
    conn: &mut H2Connection<TcpStream>,
    send_rx: PipeReceiver<Bytes>,
    head_tx: &PipeSender<(Status, SimpleHeaders)>,
    body_tx: &PipeSender<Bytes>,
    method: &SimpleMethod,
    scheme: &str,
    host: &str,
    port: u16,
    path: &str,
    req_headers: &SimpleHeaders,
) -> io::Result<()> {
    // ── Drain request body from pipe ─────────────────────────────────────
    let mut body_buf = Vec::new();
    loop {
        match send_rx.try_recv() {
            Ok(bytes) => body_buf.extend_from_slice(&bytes),
            Err(TryRecvError::Closed) => break,
            Err(TryRecvError::Empty) => {
                std::thread::sleep(Duration::from_millis(1));
            }
        }
    }

    // ── Send h2 request ──────────────────────────────────────────────────
    let method_str = method.to_string().to_uppercase();
    let method_bytes = Bytes::copy_from_slice(method_str.as_bytes());
    let authority_str = format!("{host}:{port}");
    let authority_bytes = Bytes::copy_from_slice(authority_str.as_bytes());
    let scheme_bytes = Bytes::copy_from_slice(scheme.as_bytes());
    let path_bytes = Bytes::copy_from_slice(path.as_bytes());

    let mut h2_headers: Vec<(Bytes, Bytes)> = Vec::new();
    for (k, vals) in req_headers {
        for v in vals {
            h2_headers.push((
                Bytes::copy_from_slice(k.to_string().as_bytes()),
                Bytes::copy_from_slice(v.as_bytes()),
            ));
        }
    }

    let has_body = !body_buf.is_empty();
    let req = H2Request {
        method: method_bytes,
        scheme: scheme_bytes,
        authority: authority_bytes,
        path: path_bytes,
        headers: h2_headers,
        body: if has_body { Some(Bytes::from(body_buf)) } else { None },
        end_stream: !has_body,
    };

    conn.send_request(req).map_err(|e| io::Error::new(io::ErrorKind::Other, e))?;

    // ── Read response ────────────────────────────────────────────────────
    match conn.recv_response()? {
        Some((_stream_id, response)) => {
            let mut status = Status::OK;
            let mut resp_headers = SimpleHeaders::new();

            for (name, value) in &response.headers {
                let n = String::from_utf8_lossy(name);
                if n == ":status" {
                    let s = String::from_utf8_lossy(value);
                    if let Ok(code) = s.trim().parse::<u16>() {
                        status = status_from_code(code);
                    }
                } else {
                    let k = SimpleHeader::from(n.to_string());
                    let v = String::from_utf8_lossy(value).to_string();
                    resp_headers.entry(k).or_default().push(v);
                }
            }

            head_tx.try_send((status, resp_headers)).ok();

            // Push body if present
            if let Some(body) = &response.body {
                if !body.is_empty() {
                    body_tx.try_send(body.clone()).ok();
                }
            }

            // If not end_stream, read more DATA frames
            if !response.end_stream {
                loop {
                    match conn.recv_data_frame()? {
                        Some((_sid, data, end)) => {
                            if !data.is_empty() {
                                body_tx.try_send(data).ok();
                            }
                            if end { break; }
                        }
                        None => break,
                    }
                }
            }
        }
        None => {
            head_tx.try_send((Status::BadGateway, SimpleHeaders::new())).ok();
        }
    }

    Ok(())
}
