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

use std::collections::VecDeque;
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
            // The pump streams request DATA frames while concurrently draining
            // response frames, so a bidi call makes progress in both directions.
            full_duplex: true,
            h2_trailers: true,
            http_versions: &[Proto::HTTP20],
            // One TCP connection per call — h2 stream multiplexing across calls
            // is not wired up on the client side yet.
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
            stream_id: 0,
            body_complete: false,
            request_ended: false,
            pending_body: VecDeque::new(),
            response_ended: false,
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
    /// The stream this request occupies, known once HEADERS has been sent.
    stream_id: u32,
    /// `send_rx` has closed: the request body is complete.
    body_complete: bool,
    /// Set once the terminating `DATA(END_STREAM)` has gone out.
    request_ended: bool,
    /// Response chunks the body pipe was too full to accept. They must be
    /// delivered, in order, ahead of any later chunk — `try_send` dropping a
    /// `Full` chunk on the floor silently truncates the response body.
    pending_body: VecDeque<Bytes>,
    /// The peer signalled END_STREAM; close `body_tx` once `pending_body` drains.
    response_ended: bool,
}

impl H2Pump {
    /// Forward any newly-available request body chunks, and terminate the request
    /// direction once `send_rx` closes. Returns `false` on a fatal channel error.
    ///
    /// Called from every phase: a server may answer while the client is still
    /// sending (bidi), so the request direction cannot stall once we move on to
    /// reading the response.
    fn pump_request_body(&mut self) -> bool {
        if !self.request_sent || self.request_ended {
            return true;
        }

        loop {
            match self.send_rx.try_recv() {
                Ok(chunk) => {
                    if !chunk.is_empty()
                        && self
                            .channel
                            .send_data_frame(self.stream_id, &chunk, false)
                            .is_err()
                    {
                        return false;
                    }
                }
                Err(TryRecvError::Closed) => {
                    self.body_complete = true;
                    break;
                }
                Err(TryRecvError::Empty) => break,
            }
        }

        if self.body_complete && !self.request_ended {
            if self
                .channel
                .send_data_frame(self.stream_id, &[], true)
                .is_err()
            {
                return false;
            }
            self.request_ended = true;
        }
        true
    }

    /// Push queued response chunks into `body_tx`, respecting its capacity.
    /// Returns `false` if the consumer hung up.
    fn flush_response_body(&mut self) -> bool {
        while let Some(chunk) = self.pending_body.front() {
            match self.body_tx.try_send(chunk.clone()) {
                Ok(()) => {
                    self.pending_body.pop_front();
                }
                // Consumer is behind. Keep the chunk and retry next poll.
                Err(e) if e.is_full() => return true,
                Err(_) => return false,
            }
        }
        if self.response_ended {
            self.body_tx.close();
        }
        true
    }
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
                    // ── Collect whatever body is available right now ───
                    // `Empty` means "nothing buffered yet", NOT end-of-body. Only
                    // `Closed` ends the request.
                    let mut chunks: Vec<Bytes> = Vec::new();
                    loop {
                        match self.send_rx.try_recv() {
                            Ok(b) => chunks.push(b),
                            Err(TryRecvError::Closed) => {
                                self.body_complete = true;
                                break;
                            }
                            Err(TryRecvError::Empty) => break,
                        }
                    }

                    // ── HEADERS, on the first poll ────────────────────
                    // Emitted immediately rather than waiting for the body to
                    // complete: a bidi caller keeps `send_rx` open while awaiting
                    // responses, so waiting here would deadlock. END_STREAM rides
                    // the HEADERS only when we already know there is no body.
                    if !self.request_sent {
                        let no_body = self.body_complete && chunks.is_empty();
                        let authority = Bytes::from(format!("{}:{}", self.host, self.port));

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
                            // Body frames are streamed separately below, so the
                            // HEADERS never carries one. `end_stream` here means
                            // "no body at all" — `send_request` puts END_STREAM on
                            // HEADERS exactly when `body.is_none() && end_stream`.
                            body: None,
                            end_stream: no_body,
                        };

                        match self.channel.send_request(&req) {
                            Ok(sid) => {
                                self.stream_id = sid;
                                self.request_sent = true;
                                self.request_ended = no_body;
                            }
                            Err(_e) => {
                                let _ = self.head_tx.try_send((Status::BadGateway, SimpleHeaders::new()));
                                self.head_tx.close();
                                self.body_tx.close();
                                return None;
                            }
                        }
                    }

                    // ── Stream body chunks as they arrive ─────────────
                    if self.request_sent && !self.request_ended {
                        for chunk in chunks {
                            if chunk.is_empty() {
                                continue;
                            }
                            if self
                                .channel
                                .send_data_frame(self.stream_id, &chunk, false)
                                .is_err()
                            {
                                self.head_tx.close();
                                self.body_tx.close();
                                return None;
                            }
                        }

                        // `send_rx` closed: terminate the request direction. The
                        // frame may be empty — END_STREAM is what matters.
                        if self.body_complete {
                            if self
                                .channel
                                .send_data_frame(self.stream_id, &[], true)
                                .is_err()
                            {
                                self.head_tx.close();
                                self.body_tx.close();
                                return None;
                            }
                            self.request_ended = true;
                        }
                    }

                    // Flush output
                    let out = self.channel.drain_output();
                    if !out.is_empty() {
                        match self.stream.write(&out) {
                            Ok(_) => {
                                self.stream.flush().ok();
                            }
                            Err(ref e) if e.kind() == io::ErrorKind::WouldBlock => {
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

                    // Move on once the head is out. The remaining body chunks (if
                    // any) keep streaming from the WaitingResponse/Draining
                    // phases, so a server that answers mid-request still works.
                    if self.request_sent {
                        self.state = PumpPhase::WaitingResponse;
                    } else {
                        return Some(TaskStatus::Delayed(POLL_DELAY));
                    }
                }

                PumpPhase::WaitingResponse => {
                    // Keep feeding the request: a bidi server may not answer until
                    // it has seen some (or all) of the request body.
                    if !self.pump_request_body() {
                        self.head_tx.close();
                        self.body_tx.close();
                        return None;
                    }

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

                            // Queue body if present; `flush_response_body` honours
                            // the pipe's capacity rather than dropping on Full.
                            if let Some(body) = &resp.body {
                                if !body.is_empty() {
                                    self.pending_body.push_back(body.clone());
                                }
                            }
                            if resp.end_stream {
                                self.response_ended = true;
                            }
                            if !self.flush_response_body() {
                                return None;
                            }

                            if self.response_ended && self.pending_body.is_empty() {
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
                    // The request direction may still be open (bidi).
                    if !self.pump_request_body() {
                        self.body_tx.close();
                        return None;
                    }

                    // Retry anything the consumer was too slow to take. Only pull
                    // a new frame once the backlog has drained, so ordering holds.
                    if !self.flush_response_body() {
                        return None;
                    }
                    if !self.pending_body.is_empty() {
                        return Some(TaskStatus::Delayed(POLL_DELAY));
                    }
                    if self.response_ended {
                        self.state = PumpPhase::Done;
                        return Some(TaskStatus::Pending(()));
                    }

                    match self.channel.recv_data_frame() {
                        Ok(Some((_sid, data, end))) => {
                            if !data.is_empty() {
                                self.pending_body.push_back(data);
                            }
                            if end {
                                self.response_ended = true;
                            }
                            if !self.flush_response_body() {
                                return None;
                            }
                            if self.response_ended && self.pending_body.is_empty() {
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
                                self.stream.flush().ok();
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
