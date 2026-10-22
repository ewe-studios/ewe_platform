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
use std::time::{Duration, Instant};

use bytes::Bytes;
use foundation_core::valtron::{Ctx, 
    self, Pipe, PipeReceiver, PipeSender, TaskIterator, TaskStatus,
    TryRecvError,
};
use foundation_netio::shared::http::{
    Proto, RequestDescriptor, SimpleHeader, SimpleHeaders, Status,
};
use foundation_netio::http2::channel::{H2Channel, H2StreamEvent};
use foundation_netio::http2::connection::H2Request;

use crate::shared::transport::{
    body_stream_from_pipe, BodyStream, HeadStream, Transport, TransportCapabilities,
    TransportError, TransportStream,
};

const POLL_DELAY: Duration = Duration::from_millis(10);

/// The response head as the pump publishes it: the head, or the pre-head
/// failure (h2 handshake error or handshake timeout) that stopped the call.
type HeadItem = Result<(Status, SimpleHeaders), TransportError>;

// ── H2Socket — the byte stream a pump drives ───────────────────────────────

/// The socket an [`H2Pump`] reads/writes — TCP (`host:port` from the request
/// URI) or a Unix domain socket (fixed path supplied at transport build time).
/// Both sides of the enum expose the same non-blocking I/O surface the pump
/// needs, so the pump body is socket-agnostic.
enum H2Socket {
    Tcp(TcpStream),
    #[cfg(unix)]
    Unix(std::os::unix::net::UnixStream),
}

impl H2Socket {
    fn set_nonblocking(&self, nonblocking: bool) -> io::Result<()> {
        match self {
            Self::Tcp(s) => s.set_nonblocking(nonblocking),
            #[cfg(unix)]
            Self::Unix(s) => s.set_nonblocking(nonblocking),
        }
    }
}

impl Read for H2Socket {
    fn read(&mut self, buf: &mut [u8]) -> io::Result<usize> {
        match self {
            Self::Tcp(s) => s.read(buf),
            #[cfg(unix)]
            Self::Unix(s) => s.read(buf),
        }
    }
}

impl Write for H2Socket {
    fn write(&mut self, buf: &[u8]) -> io::Result<usize> {
        match self {
            Self::Tcp(s) => s.write(buf),
            #[cfg(unix)]
            Self::Unix(s) => s.write(buf),
        }
    }
    fn flush(&mut self) -> io::Result<()> {
        match self {
            Self::Tcp(s) => s.flush(),
            #[cfg(unix)]
            Self::Unix(s) => s.flush(),
        }
    }
}

// ── H2Transport ────────────────────────────────────────────────────────────

/// HTTP/2 cleartext (h2c prior-knowledge) transport — one connection per call.
///
/// Dials TCP by default (`host:port` from the request URI). Build with
/// [`unix`](Self::unix) to dial a Unix domain socket instead — the request
/// URI's authority is then only used for the `:authority` pseudo-header
/// (gRPC servers on Unix sockets ignore it).
///
/// `open()` never waits on the peer's application: it dials, then hands the
/// socket to the pump task, which runs the h2 handshake (preface + SETTINGS
/// exchange) non-blocking ahead of the request. `open()` is called from async
/// code running on a valtron worker, and the peer is often served by the same
/// pool (an in-process server, or a proxy and its backend). A handshake that
/// blocked the worker in `read(2)` until the peer's SETTINGS arrived could
/// starve the very task that sends them and deadlock the pool.
#[derive(Clone)]
pub struct H2Transport {
    /// When set, every `open()` dials this Unix socket instead of TCP.
    #[cfg(unix)]
    unix_path: Option<std::path::PathBuf>,
    /// Longest `open()` waits for the TCP dial. See [`Self::with_connect_timeout`].
    connect_timeout: Duration,
    /// Longest the call waits for the peer's h2 SETTINGS. See
    /// [`Self::with_handshake_timeout`].
    handshake_timeout: Duration,
}

impl Default for H2Transport {
    fn default() -> Self {
        Self {
            #[cfg(unix)]
            unix_path: None,
            connect_timeout: Self::DEFAULT_CONNECT_TIMEOUT,
            handshake_timeout: Self::DEFAULT_HANDSHAKE_TIMEOUT,
        }
    }
}

impl H2Transport {
    /// Default for [`Self::with_connect_timeout`]: 10 s, the dial budget this
    /// transport has always used.
    pub const DEFAULT_CONNECT_TIMEOUT: Duration = Duration::from_secs(10);

    /// Default for [`Self::with_handshake_timeout`]: 10 s, the same budget Go's
    /// `net/http` `DefaultTransport` gives the comparable step
    /// (`TLSHandshakeTimeout`): the connection is up but the peer has not
    /// finished its preamble.
    pub const DEFAULT_HANDSHAKE_TIMEOUT: Duration = Duration::from_secs(10);

    #[must_use]
    pub fn new() -> Self { Self::default() }

    /// An `H2Transport` that dials the given Unix domain socket for every
    /// call — h2c-over-Unix, the shape `buildkitd` listens on by default
    /// (`unix:///run/buildkit/buildkitd.sock`).
    #[cfg(unix)]
    #[must_use]
    pub fn unix(path: impl Into<std::path::PathBuf>) -> Self {
        Self { unix_path: Some(path.into()), ..Self::default() }
    }

    /// Set how long `open()` waits for the TCP (or Unix socket) dial.
    /// Default [`Self::DEFAULT_CONNECT_TIMEOUT`].
    #[must_use]
    pub fn with_connect_timeout(mut self, timeout: Duration) -> Self {
        self.connect_timeout = timeout;
        self
    }

    /// Set how long a call waits for the peer's h2 SETTINGS after the dial. A
    /// peer that accepts the connection but never answers the preface fails the
    /// call with [`TransportError::Timeout`] on its response head instead of
    /// hanging it. Default [`Self::DEFAULT_HANDSHAKE_TIMEOUT`].
    #[must_use]
    pub fn with_handshake_timeout(mut self, timeout: Duration) -> Self {
        self.handshake_timeout = timeout;
        self
    }
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

        // ── Connect ──────────────────────────────────────────────────────────
        // The dial still blocks (bounded by `connect_timeout`). The kernel
        // completes it without the peer's application, so it never waits on a
        // task of this pool. A non-blocking dial is tracked in the bug backlog.
        #[cfg(unix)]
        let stream = match &self.unix_path {
            Some(path) => H2Socket::Unix(
                std::os::unix::net::UnixStream::connect(path)
                    .map_err(|e| TransportError::Connect(Arc::new(e)))?,
            ),
            None => {
                let addr = format!("{host}:{port}");
                H2Socket::Tcp(
                    TcpStream::connect_timeout(
                        &addr.parse().map_err(|e| TransportError::Connect(Arc::new(e)))?,
                        self.connect_timeout,
                    )
                    .map_err(|e| TransportError::Connect(Arc::new(e)))?,
                )
            }
        };
        #[cfg(not(unix))]
        let stream = {
            let addr = format!("{host}:{port}");
            H2Socket::Tcp(
                TcpStream::connect_timeout(
                    &addr.parse().map_err(|e| TransportError::Connect(Arc::new(e)))?,
                    self.connect_timeout,
                )
                .map_err(|e| TransportError::Connect(Arc::new(e)))?,
            )
        };
        stream
            .set_nonblocking(true)
            .map_err(|e| TransportError::Connect(Arc::new(e)))?;

        // ── h2 handshake ───────────────────────────────────────────────────────
        // Not run here. The pump runs it (`PumpPhase::Handshaking`) without
        // blocking a worker and reports a failure on the head stream. Waiting
        // for the peer's SETTINGS on this thread deadlocked the pool whenever
        // every worker was blocked here on a peer that needed a worker to answer.
        let channel = H2Channel::new(false); // is_server = false (client)

        // ── Pipes ────────────────────────────────────────────────────────────
        let (send_tx, send_rx) = Pipe::<Bytes>::new();
        let (head_tx, head_rx) = Pipe::<HeadItem>::new();
        let (body_tx, body_rx) = Pipe::<Bytes>::new();
        let (trailer_tx, trailer_rx) = Pipe::<SimpleHeaders>::with_depth(1);

        // ── Spawn the valtron pump ────────────────────────────────────────────
        let pump = H2Pump {
            stream,
            channel,
            state: PumpPhase::Handshaking,
            handshake_deadline: Instant::now() + self.handshake_timeout,
            pending_out: Vec::new(),
            send_rx,
            head_tx,
            body_tx,
            trailer_tx,
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
            TransportError::Connect(Arc::new(io::Error::other(e.to_string())))
        })?;

        // The head pipe carries `Result`s so a handshake failure reaches the
        // caller as `Err(TransportError)`, as it did when `open()` returned it.
        let head: HeadStream = Box::pin(futures::stream::unfold(head_rx, |rx| async move {
            rx.receive().await.map(|item| (item, rx))
        }));
        let recv_body: BodyStream = body_stream_from_pipe(body_rx);

        Ok(TransportStream { send_body: Arc::new(send_tx), head, recv_body, trailers: trailer_rx })
    }
}

// ── H2Pump — valtron TaskIterator ──────────────────────────────────────────

enum PumpPhase {
    /// Exchanging the connection preface and SETTINGS with the peer.
    Handshaking,
    SendingRequest,
    WaitingResponse,
    Draining,
    Done,
}

struct H2Pump {
    stream: H2Socket,
    channel: H2Channel,
    state: PumpPhase,
    /// When `Handshaking` gives up on the peer's SETTINGS.
    handshake_deadline: Instant,
    /// Bytes drained from `channel` that the non-blocking socket has not taken
    /// yet. Written ahead of any later output so frames are never split or lost.
    pending_out: Vec<u8>,
    send_rx: PipeReceiver<Bytes>,
    head_tx: PipeSender<HeadItem>,
    body_tx: PipeSender<Bytes>,
    trailer_tx: PipeSender<SimpleHeaders>,
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

    /// Tell the peer to stop this stream: send `RST_STREAM(CANCEL)` and flush it.
    ///
    /// Used when the response consumer goes away mid-call. Dropping the socket
    /// would also stop the flow, but an explicit reset is the protocol-correct
    /// abort and reaches a peer whose server may not treat a bare FIN as an
    /// error — `foundation_http`'s h2 server peeks for whole frames and reads
    /// EOF as `WouldBlock`, so it would otherwise keep streaming into a socket
    /// whose reader is gone.
    fn abort_peer_stream(&mut self) {
        if !self.request_sent {
            return;
        }
        self.channel.send_rst_stream(self.stream_id);
        // Best effort: the call is being torn down either way, and a socket
        // that cannot take the reset is closed when the pump drops it.
        if let Err(e) = self.write_out() {
            tracing::debug!(err = %e, "h2 pump: could not send RST_STREAM");
        }
    }

    /// Queue the channel's new output behind any unwritten bytes and write as
    /// much as the non-blocking socket takes.
    ///
    /// Returns `Ok(true)` once everything is written, `Ok(false)` when the socket
    /// is full (the rest stays in `pending_out` for the next poll), or the write
    /// error. A bare `write` that ignored its count, or dropped the drained
    /// bytes on `WouldBlock`, would cut h2 frames and corrupt the connection.
    fn write_out(&mut self) -> io::Result<bool> {
        let out = self.channel.drain_output();
        if !out.is_empty() {
            self.pending_out.extend_from_slice(&out);
        }
        while !self.pending_out.is_empty() {
            match self.stream.write(&self.pending_out) {
                Ok(0) => {
                    return Err(io::Error::new(
                        io::ErrorKind::WriteZero,
                        "h2 socket accepted no bytes",
                    ))
                }
                Ok(n) => {
                    self.pending_out.drain(..n);
                }
                Err(ref e) if e.kind() == io::ErrorKind::WouldBlock => return Ok(false),
                Err(ref e) if e.kind() == io::ErrorKind::Interrupted => {}
                Err(e) => return Err(e),
            }
        }
        self.stream.flush()?;
        Ok(true)
    }

    /// End the call before a response head: publish `err` on the head stream
    /// (so the caller sees why, as when `open()` returned it) and close the
    /// response pipes.
    fn fail_before_head(&mut self, err: TransportError) {
        let _ = self.head_tx.try_send(Err(err));
        self.head_tx.close();
        self.body_tx.close();
        self.trailer_tx.close();
    }
}

impl TaskIterator for H2Pump {
    type Ready = ();
    type Pending = ();

    fn next_status(&mut self, _ctx: Ctx) -> Option<TaskStatus<(), ()>> {
        // Set non-blocking
        self.stream.set_nonblocking(true).ok();

        loop {
            // ── Read from socket, feed to channel ──────────────────────
            let mut buf = [0u8; 8192];
            let mut read_some = false;
            match self.stream.read(&mut buf) {
                Ok(0) if matches!(self.state, PumpPhase::Handshaking) => {
                    self.fail_before_head(TransportError::Connect(Arc::new(io::Error::new(
                        io::ErrorKind::UnexpectedEof,
                        "connection closed during handshake",
                    ))));
                    return None;
                }
                Ok(0) => {
                    // Connection closed
                    tracing::debug!(
                        phase = match self.state {
                            PumpPhase::Handshaking => "Handshaking",
                            PumpPhase::SendingRequest => "SendingRequest",
                            PumpPhase::WaitingResponse => "WaitingResponse",
                            PumpPhase::Draining => "Draining",
                            PumpPhase::Done => "Done",
                        },
                        "h2 pump: connection closed by peer"
                    );
                    self.head_tx.close();
                    self.body_tx.close();
                    return None;
                }
                Ok(n) => {
                    self.channel.feed_input(&buf[..n]);
                    read_some = true;
                }
                Err(ref e) if e.kind() == io::ErrorKind::WouldBlock => {
                    // No data — continue to process channel
                }
                Err(e) if matches!(self.state, PumpPhase::Handshaking) => {
                    self.fail_before_head(TransportError::Connect(Arc::new(e)));
                    return None;
                }
                Err(_e) => {
                    tracing::warn!(err = %_e, "h2 pump: socket read error — exiting");
                    self.head_tx.close();
                    self.body_tx.close();
                    return None;
                }
            }

            match self.state {
                PumpPhase::Handshaking => {
                    // The caller dropped the call before the connection was up.
                    if self.head_tx.is_closed() {
                        self.body_tx.close();
                        self.trailer_tx.close();
                        return None;
                    }
                    match self.channel.client_handshake_step() {
                        // Done: fall through to the shared flush below, and the
                        // next loop turn sends the request.
                        Ok(()) => self.state = PumpPhase::SendingRequest,
                        Err(ref e) if e.kind() == io::ErrorKind::WouldBlock => {
                            if let Err(e) = self.write_out() {
                                self.fail_before_head(TransportError::Connect(Arc::new(e)));
                                return None;
                            }
                            if Instant::now() >= self.handshake_deadline {
                                tracing::warn!(
                                    host = %self.host,
                                    port = self.port,
                                    "h2 pump: peer sent no SETTINGS before the handshake timeout"
                                );
                                self.fail_before_head(TransportError::Timeout);
                                return None;
                            }
                            // More bytes may already be waiting; otherwise yield
                            // the worker instead of blocking it on the peer.
                            if read_some {
                                continue;
                            }
                            return Some(TaskStatus::Delayed(POLL_DELAY));
                        }
                        Err(e) => {
                            self.fail_before_head(TransportError::Connect(Arc::new(e)));
                            return None;
                        }
                    }
                }

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
                                let _ = self.head_tx.try_send(Ok((Status::BadGateway, SimpleHeaders::new())));
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
                    match self.write_out() {
                        Ok(true) => {}
                        Ok(false) => return Some(TaskStatus::Delayed(POLL_DELAY)),
                        Err(_e) => {
                            let _ = self.head_tx.try_send(Ok((Status::BadGateway, SimpleHeaders::new())));
                            self.head_tx.close();
                            self.body_tx.close();
                            return None;
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
                    // The response consumer went away (it dropped the head/body
                    // pipes, e.g. on cancellation) before the head arrived. Nothing
                    // will ever read this response — close up rather than leak the
                    // socket waiting on a peer whose reader is gone.
                    if self.head_tx.is_closed() {
                        self.abort_peer_stream();
                        self.body_tx.close();
                        self.trailer_tx.close();
                        return None;
                    }
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

                            let _ = self.head_tx.try_send(Ok((status, resp_headers)));
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
                            let _ = self.head_tx.try_send(Ok((Status::BadGateway, SimpleHeaders::new())));
                            self.head_tx.close();
                            self.body_tx.close();
                            return None;
                        }
                        Err(ref e) if e.kind() == io::ErrorKind::WouldBlock => {
                            // Need more data — flush writes, then yield
                            if self.write_out().is_err() {
                                self.head_tx.close();
                                self.body_tx.close();
                                return None;
                            }
                            return Some(TaskStatus::Delayed(POLL_DELAY));
                        }
                        Err(_e) => {
                            let _ = self.head_tx.try_send(Ok((Status::BadGateway, SimpleHeaders::new())));
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

                    // The response consumer went away mid-body (dropped
                    // `recv_body`, e.g. on cancellation) while nothing was buffered
                    // to reveal it through `flush_response_body`. Close up rather
                    // than leak the socket. `response_ended` is still false, so the
                    // pump has not closed `body_tx` itself — a closed pipe here
                    // means the receiver is gone.
                    if self.body_tx.is_closed() && !self.response_ended {
                        self.abort_peer_stream();
                        self.trailer_tx.close();
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

                    match self.channel.recv_stream_event() {
                        Ok(Some((_sid, H2StreamEvent::Data { data, end_stream: end, .. }))) => {
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
                        Ok(Some((_sid, H2StreamEvent::Trailers { headers, .. }))) => {
                            // Decode trailing headers and push them into the
                            // trailer pipe so the caller can read grpc-status etc.
                            let mut trailers = SimpleHeaders::new();
                            for (name, value) in &headers {
                                let n = String::from_utf8_lossy(name);
                                let v = String::from_utf8_lossy(value);
                                // Skip pseudo-headers (:status etc.).
                                if !n.starts_with(':') {
                                    trailers
                                        .entry(SimpleHeader::from(n.to_string()))
                                        .or_default()
                                        .push(v.to_string());
                                }
                            }
                            let _ = self.trailer_tx.try_send(trailers);
                            self.trailer_tx.close();
                            // Trailers carry END_STREAM, so the body is also done.
                            self.response_ended = true;
                            if !self.flush_response_body() {
                                return None;
                            }
                            if self.pending_body.is_empty() {
                                self.state = PumpPhase::Done;
                                return Some(TaskStatus::Pending(()));
                            }
                        }
                        Ok(None) => {
                            self.body_tx.close();
                            self.trailer_tx.close();
                            self.state = PumpPhase::Done;
                            return Some(TaskStatus::Pending(()));
                        }
                        Err(ref e) if e.kind() == io::ErrorKind::WouldBlock => {
                            if self.write_out().is_err() {
                                self.body_tx.close();
                                self.trailer_tx.close();
                                return None;
                            }
                            return Some(TaskStatus::Delayed(POLL_DELAY));
                        }
                        Err(_e) => {
                            self.body_tx.close();
                            self.trailer_tx.close();
                            return None;
                        }
                    }
                }

                PumpPhase::Done => {
                    // The response direction is finished, and `Done` is terminal.
                    //
                    // Closing `body_tx` is what tells the consumer end-of-stream.
                    // Leaving it to the sender's eventual drop parks the reader
                    // until the *peer's* idle timeout closes the socket — a 60s
                    // stall on a call that completed in milliseconds.
                    //
                    // Returning `None` completes the task. `TaskStatus::Ready`
                    // only yields a value, so the executor polls again, lands on
                    // `Done` again, and spins: nobody consumes this task's value
                    // (`valtron::send` is fire-and-forget), so it never ended.
                    self.body_tx.close();
                    self.head_tx.close();
                    self.trailer_tx.close();
                    return None;
                }
            }

            // Flush output
            match self.write_out() {
                Ok(true) => {}
                Ok(false) => return Some(TaskStatus::Delayed(POLL_DELAY)),
                Err(_e) => {
                    self.head_tx.close();
                    self.body_tx.close();
                    return None;
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

