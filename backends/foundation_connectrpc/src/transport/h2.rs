//! Native HTTP/2 `Transport` implementation (Feature 30).
//!
//! WHY: `Transport` byte-level client-seam contract for HTTP/2 cleartext (h2c
//! prior-knowledge). `open()` connects over TCP, negotiates the h2 handshake,
//! spawns a valtron `TaskIterator` pump, and returns `TransportStream` synchronously.
//!
//! WHAT: [`H2Transport`] wraps nothing — it is a stateless factory. The pump
//! ([`H2Pump`]) is a [`TaskIterator`] that owns a non-blocking `TcpStream` and an
//! [`H2Channel`]. Each poll: read fd bytes → feed channel → step → drain output →
//! write fd. On `WouldBlock` it yields `TaskStatus::Delayed` so the valtron
//! executor re-polls without busy-spinning.
//!
//! HOW: Pattern matches `H1Transport` — `valtron::send(pump)` spawns the task
//! on the pool; the caller gets `TransportStream { send_body, head, recv_body }`
//! and drains response bytes from the pipe handles.

use std::io::{self, Read, Write};
use std::net::TcpStream;
use std::sync::Arc;
use std::time::Duration;

use bytes::Bytes;
use foundation_core::valtron::{
    self, BoxedSendExecutionAction, Pipe, PipeReceiver, PipeSender, TaskIterator, TaskStatus,
    TryRecvError,
};
use foundation_netio::simple_http::shared::{
    Proto, RequestDescriptor, SimpleHeader, SimpleHeaders, Status,
};
use foundation_netio::http2::channel::H2Channel;
use foundation_netio::http2::connection::H2Request;

use super::{
    body_stream_from_pipe, head_stream_from_pipe, BodyStream, HeadStream,
    Transport, TransportCapabilities, TransportError, TransportStream,
};

const HANDSHAKE_TIMEOUT: Duration = Duration::from_secs(10);
const POLL_DELAY: Duration = Duration::from_millis(10);

// ── H2Transport ────────────────────────────────────────────────────────────

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
        let host = request.request_uri.host_str()
            .map(|s| s.to_string())
            .unwrap_or_else(|| "localhost".into());
        let port = request.request_uri.port_or_default();
        let path = request.request_uri.path().to_string();
        let method = request.method.clone();
        let req_headers = request.headers.clone();

        // ── TCP connect (blocking — handshake must complete before pump starts) ──
        let addr = format!("{host}:{port}");
        let mut stream = TcpStream::connect_timeout(
            &addr.parse().map_err(|e| TransportError::Connect(Arc::new(e)))?,
            HANDSHAKE_TIMEOUT,
        ).map_err(|e| TransportError::Connect(Arc::new(e)))?;

        // ── h2 handshake (blocking, on caller's thread) ────────────────────────
        let mut channel = H2Channel::new(false); // is_server = false (client)
        // Feed nothing yet — the handshake starts by sending preface+SETTINGS
        loop {
            match channel.client_handshake_step() {
                Ok(()) => break, // done
                Err(e) if e.kind() == io::ErrorKind::WouldBlock => {
                    // Drain output to socket, then read more
                    let out = channel.drain_output();
                    if !out.is_empty() {
                        stream.write_all(&out).map_err(|e| TransportError::Connect(Arc::new(e)))?;
                    }
                    // Read bytes from socket, feed to channel
                    let mut buf = [0u8; 8192];
                    match stream.read(&mut buf) {
                        Ok(0) => return Err(TransportError::Connect(Arc::new(io::Error::new(
                            io::ErrorKind::UnexpectedEof, "connection closed during handshake"
                        )))),
                        Ok(n) => channel.feed_input(&buf[..n]),
                        Err(e) => return Err(TransportError::Connect(Arc::new(e))),
                    }
                }
                Err(e) => return Err(TransportError::Connect(Arc::new(e))),
            }
        }
        // Flush any final output
        let out = channel.drain_output();
        if !out.is_empty() {
            stream.write_all(&out).map_err(|e| TransportError::Connect(Arc::new(e)))?;
        }
        stream.flush().ok();

        // ── Pipes ────────────────────────────────────────────────────────────
        let (send_tx, send_rx) = Pipe::<Bytes>::new();
        let (head_tx, head_rx) = Pipe::<(Status, SimpleHeaders)>::new();
        let (body_tx, body_rx) = Pipe::<Bytes>::new();

        // ── Spawn the valtron pump ────────────────────────────────────────────
        let pump = H2Pump {
            stream,
            channel,
            state: PumpPhase::SendingRequest,
            send_rx,
            head_tx,
            body_tx,
            method: method.to_string(),
            host,
            port,
            path,
            req_headers,
            request_sent: false,
            response_head_sent: false,
            body_buf: Vec::new(),
            body_complete: false,
        };

        valtron::send(pump).map_err(|e| {
            TransportError::Connect(Arc::new(io::Error::new(
                io::ErrorKind::Other,
                e.to_string(),
            )))
        })?;

        let head: HeadStream = head_stream_from_pipe(head_rx);
        let recv_body: BodyStream = body_stream_from_pipe(body_rx);

        Ok(TransportStream { send_body: send_tx, head, recv_body })
    }
}

// ── H2Pump — valtron TaskIterator ──────────────────────────────────────────

enum PumpPhase {
    SendingRequest,
    WaitingResponse,
    Draining,
    Done,
}

struct H2Pump {
    stream: TcpStream,
    channel: H2Channel,
    state: PumpPhase,
    send_rx: PipeReceiver<Bytes>,
    head_tx: PipeSender<(Status, SimpleHeaders)>,
    body_tx: PipeSender<Bytes>,
    method: String,
    host: String,
    port: u16,
    path: String,
    req_headers: SimpleHeaders,
    request_sent: bool,
    response_head_sent: bool,
    /// Request body accumulated across polls. The pump emits the request as a
    /// single HEADERS(+DATA), so it must hold every byte until `send_rx` closes.
    body_buf: Vec<u8>,
    /// `send_rx` has closed: the request body is complete.
    body_complete: bool,
}

impl TaskIterator for H2Pump {
    type Ready = ();
    type Pending = ();
    type Spawner = BoxedSendExecutionAction;

    fn next_status(&mut self) -> Option<TaskStatus<(), (), BoxedSendExecutionAction>> {
        // Set non-blocking
        self.stream.set_nonblocking(true).ok();

        loop {
            // ── Read from socket, feed to channel ──────────────────────
            let mut buf = [0u8; 8192];
            match self.stream.read(&mut buf) {
                Ok(0) => {
                    // Connection closed
                    self.head_tx.close();
                    self.body_tx.close();
                    return None;
                }
                Ok(n) => self.channel.feed_input(&buf[..n]),
                Err(ref e) if e.kind() == io::ErrorKind::WouldBlock => {
                    // No data — continue to process channel
                }
                Err(_e) => {
                    self.head_tx.close();
                    self.body_tx.close();
                    return None;
                }
            }

            match self.state {
                PumpPhase::SendingRequest => {
                    // ── Drain request body from pipe ──────────────────
                    // Accumulate into `self.body_buf`: a poll that drains some
                    // bytes and then sees `Empty` must not lose them.
                    loop {
                        match self.send_rx.try_recv() {
                            Ok(b) => self.body_buf.extend_from_slice(&b),
                            Err(TryRecvError::Closed) => {
                                self.body_complete = true;
                                break;
                            }
                            // Nothing buffered *right now*. The body may still be
                            // on its way — this is not end-of-body.
                            Err(TryRecvError::Empty) => break,
                        }
                    }

                    // ── Send the h2 request ───────────────────────────
                    // The request goes out as one HEADERS(+DATA), so it cannot be
                    // sent until the body is complete. Committing earlier would
                    // set END_STREAM on HEADERS while bytes were still queued,
                    // silently dropping the request body.
                    if !self.request_sent && self.body_complete {
                        let body_bytes = std::mem::take(&mut self.body_buf);
                        let authority = Bytes::from(format!("{}:{}", self.host, self.port));
                        let has_body = !body_bytes.is_empty();

                        let mut h2_headers = Vec::new();
                        for (k, vals) in &self.req_headers {
                            for v in vals {
                                h2_headers.push((
                                    Bytes::copy_from_slice(k.to_string().as_bytes()),
                                    Bytes::copy_from_slice(v.as_bytes()),
                                ));
                            }
                        }

                        let req = H2Request {
                            method: Bytes::copy_from_slice(self.method.to_uppercase().as_bytes()),
                            scheme: Bytes::from_static(b"http"),
                            authority,
                            path: Bytes::copy_from_slice(self.path.as_bytes()),
                            headers: h2_headers,
                            body: if has_body { Some(Bytes::from(body_bytes)) } else { None },
                            // "The request is complete." `send_request` puts
                            // END_STREAM on HEADERS when there is no body, and on
                            // the DATA frame when there is. Passing `!has_body`
                            // here marks neither, so the peer waits forever.
                            end_stream: true,
                        };

                        match self.channel.send_request(&req) {
                            Ok(_sid) => self.request_sent = true,
                            Err(ref e) if e.kind() == io::ErrorKind::WouldBlock => {
                                // Shouldn't happen — send_request doesn't read
                            }
                            Err(_e) => {
                                let _ = self.head_tx.try_send((Status::BadGateway, SimpleHeaders::new()));
                                self.head_tx.close();
                                self.body_tx.close();
                                return None;
                            }
                        }
                    }

                    // Flush output
                    let out = self.channel.drain_output();
                    if !out.is_empty() {
                        match self.stream.write(&out) {
                            Ok(_) => {}
                            Err(ref e) if e.kind() == io::ErrorKind::WouldBlock => {
                                // Put bytes back? Just yield and retry.
                                return Some(TaskStatus::Delayed(POLL_DELAY));
                            }
                            Err(_e) => {
                                let _ = self.head_tx.try_send((Status::BadGateway, SimpleHeaders::new()));
                                self.head_tx.close();
                                self.body_tx.close();
                                return None;
                            }
                        }
                    }

                    if self.request_sent {
                        self.state = PumpPhase::WaitingResponse;
                    }
                }

                PumpPhase::WaitingResponse => {
                    // ── Try to read a response ─────────────────────────
                    match self.channel.recv_response() {
                        Ok(Some((_sid, resp))) => {
                            // Extract pseudo-header :status from response headers
                            let mut status = Status::OK;
                            let mut resp_headers = SimpleHeaders::new();

                            for (name, value) in &resp.headers {
                                let n = String::from_utf8_lossy(name);
                                if n == ":status" {
                                    if let Ok(code) = String::from_utf8_lossy(value).trim().parse::<u16>() {
                                        status = status_from_u16(code);
                                    }
                                } else {
                                    resp_headers.entry(SimpleHeader::from(n.to_string()))
                                        .or_default()
                                        .push(String::from_utf8_lossy(value).to_string());
                                }
                            }

                            let _ = self.head_tx.try_send((status, resp_headers));
                            self.head_tx.close();
                            self.response_head_sent = true;

                            // Push body if present
                            if let Some(body) = &resp.body {
                                if !body.is_empty() {
                                    let _ = self.body_tx.try_send(body.clone());
                                }
                            }

                            if resp.end_stream {
                                self.body_tx.close();
                                self.state = PumpPhase::Done;
                                return Some(TaskStatus::Pending(()));
                            }

                            self.state = PumpPhase::Draining;
                        }
                        Ok(None) => {
                            // GOAWAY or connection closed
                            let _ = self.head_tx.try_send((Status::BadGateway, SimpleHeaders::new()));
                            self.head_tx.close();
                            self.body_tx.close();
                            return None;
                        }
                        Err(ref e) if e.kind() == io::ErrorKind::WouldBlock => {
                            // Need more data — flush writes, then yield
                            let out = self.channel.drain_output();
                            if !out.is_empty() {
                                match self.stream.write(&out) {
                                    Ok(_) => { self.stream.flush().ok(); }
                                    Err(ref e) if e.kind() == io::ErrorKind::WouldBlock => {}
                                    Err(_e) => {
                                        self.head_tx.close();
                                        self.body_tx.close();
                                        return None;
                                    }
                                }
                            }
                            return Some(TaskStatus::Delayed(POLL_DELAY));
                        }
                        Err(_e) => {
                            let _ = self.head_tx.try_send((Status::BadGateway, SimpleHeaders::new()));
                            self.head_tx.close();
                            self.body_tx.close();
                            return None;
                        }
                    }
                }

                PumpPhase::Draining => {
                    // Read more DATA frames
                    match self.channel.recv_data_frame() {
                        Ok(Some((_sid, data, end))) => {
                            if !data.is_empty() {
                                let _ = self.body_tx.try_send(data);
                            }
                            if end {
                                self.body_tx.close();
                                self.state = PumpPhase::Done;
                                return Some(TaskStatus::Pending(()));
                            }
                        }
                        Ok(None) => {
                            self.body_tx.close();
                            self.state = PumpPhase::Done;
                            return Some(TaskStatus::Pending(()));
                        }
                        Err(ref e) if e.kind() == io::ErrorKind::WouldBlock => {
                            let out = self.channel.drain_output();
                            if !out.is_empty() {
                                let _ = self.stream.write(&out);
                            }
                            return Some(TaskStatus::Delayed(POLL_DELAY));
                        }
                        Err(_e) => {
                            self.body_tx.close();
                            return None;
                        }
                    }
                }

                PumpPhase::Done => {
                    return Some(TaskStatus::Ready(()));
                }
            }

            // Flush output
            let out = self.channel.drain_output();
            if !out.is_empty() {
                match self.stream.write(&out) {
                    Ok(_) => { self.stream.flush().ok(); }
                    Err(ref e) if e.kind() == io::ErrorKind::WouldBlock => {
                        return Some(TaskStatus::Delayed(POLL_DELAY));
                    }
                    Err(_e) => {
                        self.head_tx.close();
                        self.body_tx.close();
                        return None;
                    }
                }
            }
        }
    }
}

fn status_from_u16(code: u16) -> Status {
    match code {
        200 => Status::OK,
        201 => Status::Created,
        204 => Status::NoContent,
        400 => Status::BadRequest,
        401 => Status::Unauthorized,
        403 => Status::Forbidden,
        404 => Status::NotFound,
        500 => Status::InternalServerError,
        502 => Status::BadGateway,
        503 => Status::ServiceUnavailable,
        _ => Status::InternalServerError,
    }
}
