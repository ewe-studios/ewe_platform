//! `H2ConnectionHandler` — valtron-driven HTTP/2 multiplexed connection handler (F47).
//!
//! WHY: The HTTP/1.1 `ConnectionHandler` serves one exchange at a time, so it can
//! hand the socket straight to a handler. HTTP/2 multiplexes many concurrent
//! streams over one connection, so no handler may touch the socket: a handler
//! that blocked would stall every other stream on that connection. This task
//! owns `H2Conn` exclusively and is its only writer.
//!
//! WHAT: A valtron `TaskIterator` over one h2 connection. It demultiplexes
//! inbound frames into per-stream body pipes, spawns one `serve_h2` future per
//! stream on the valtron pool, and multiplexes the streams' response pipes back
//! onto the shared write buffer.
//!
//! HOW: Each poll performs the h2 handshake (once), reads at most one frame,
//! routes it, then drains every active stream's response pipe into `H2Conn`.
//! Nothing here blocks: request bodies stream through pipes rather than being
//! collected, and handler futures run elsewhere. `WouldBlock` parks the task on
//! a timer (see the reactor-parking gap tracked as feature 47b).

use std::collections::BTreeMap;
use std::io;
use std::sync::Arc;
use std::time::{Duration, Instant};

use bytes::Bytes;
use foundation_core::synca::{OnSignal, WaitGroupGuard};
use foundation_core::valtron::{
    BoxedSendExecutionAction, FutureTask, Pipe, PipeReceiver, PipeSender, TaskIterator, TaskStatus,
    TryRecvError,
};
use foundation_netio::http2::conn::H2Conn;
use foundation_netio::http2::frame::{
    data_flags, headers_flags, ErrorCode, Head, HeadersFrame, Kind, ResetFrame,
};
use foundation_netio::http2::types::{header_from_hpack, H2Frame, H2IncomingFrame};
use foundation_netio::netcap::ConnectionContext;

use crate::shared::app::HttpApp;
use crate::native::serve::H2Serve;

const POLL_DELAY: Duration = Duration::from_millis(10);
/// Aggressive poll delay during the H2 handshake — grpc-go may fire GOAWAY
/// within 10ms of completing its side; our SETTINGS + PING must arrive first.
const HANDSHAKE_POLL_DELAY: Duration = Duration::from_millis(1);
const IDLE_TIMEOUT: Duration = Duration::from_secs(60);

/// Depth of the per-stream body and response pipes, in frames. Bounded so a fast
/// peer cannot make us buffer an unbounded request body.
const STREAM_PIPE_DEPTH: usize = 32;

/// The connection's half of one live h2 stream.
struct ActiveStream {
    /// Inbound DATA/RST frames go here; the handler future reads them.
    body_tx: PipeSender<H2IncomingFrame>,
    /// The handler future's response frames arrive here; the poll loop drains them.
    resp_rx: PipeReceiver<H2Frame>,
    /// Set once a frame carrying END_STREAM has been serialized.
    response_done: bool,
}

/// HTTP/2 multiplexed connection handler — one valtron task per connection.
pub struct H2ConnectionHandler {
    conn: H2Conn,
    app: Arc<HttpApp<Arc<dyn H2Serve>>>,
    connection: Arc<ConnectionContext>,
    shutdown: Arc<OnSignal>,
    _drain_guard: WaitGroupGuard,
    idle_since: Option<Instant>,
    handshaked: bool,
    streams: BTreeMap<u32, ActiveStream>,
}

impl H2ConnectionHandler {
    #[must_use]
    pub fn new(
        conn: H2Conn,
        app: Arc<HttpApp<Arc<dyn H2Serve>>>,
        connection: Arc<ConnectionContext>,
        shutdown: Arc<OnSignal>,
        drain_guard: WaitGroupGuard,
    ) -> Self {
        Self {
            conn,
            app,
            connection,
            shutdown,
            _drain_guard: drain_guard,
            idle_since: None,
            handshaked: false,
            streams: BTreeMap::new(),
        }
    }
}

impl TaskIterator for H2ConnectionHandler {
    type Ready = ();
    type Pending = ();
    type Spawner = BoxedSendExecutionAction;

    fn next_status(&mut self) -> Option<TaskStatus<(), (), BoxedSendExecutionAction>> {
        if self.shutdown.probe() {
            eprintln!("[h2-srv] shutdown signaled, closing");
            return None;
        }

        // The socket is non-blocking, so the handshake — which reads the client
        // preface — may not complete on the first poll.
        if !self.handshaked {
            match self.conn.server_handshake() {
                Ok(()) => {
                    self.handshaked = true;
                    tracing::debug!("h2 server handshake complete");
                    // Send immediate PING to prevent grpc-go's transport from
                    // entering idle and firing GOAWAY 10ms after handshake.
                    // The PING creates bidirectional activity, keeping the transport
                    // alive until monitorHealth fires at 5s.
                    self.conn.send_ping(new_ping_opaque_h2srv());
                    self.conn.flush().ok();
                    eprintln!("[h2-srv] handshake done, sent keepalive PING");
                }
                Err(ref e) if e.kind() == io::ErrorKind::WouldBlock => {
                    // Handshake in progress — 1ms poll to minimise response latency.
                    // grpc-go's transport may fire GOAWAY within 10ms of completing
                    // its side of the handshake. Our SETTINGS + PING must arrive
                    // before that window closes.
                    return Some(TaskStatus::Delayed(HANDSHAKE_POLL_DELAY));
                }
                Err(e) => {
                    tracing::debug!(err = %e, "h2 server handshake failed");
                    return None;
                }
            }
        }

        // Use read_stream_frame — it auto-handles connection-level frames
        // (PING→ACK, SETTINGS→ACK, WINDOW_UPDATE) so we only see stream frames.
        match self.conn.read_stream_frame() {
            Ok(Some((head, payload))) => {
                self.idle_since = None;
                match head.kind {
                    Kind::Headers => {
                        tracing::debug!(stream = head.stream_id, kind = "HEADERS", "h2 frame received");
                        self.on_headers(&head, &payload)
                    }
                    Kind::Data => {
                        tracing::debug!(stream = head.stream_id, kind = "DATA", "h2 frame received");
                        self.on_data(&head, &payload)
                    }
                    Kind::Reset => {
                        tracing::debug!(stream = head.stream_id, kind = "RST_STREAM", "h2 frame received");
                        self.on_reset(&head, &payload)
                    }
                    _ => {}
                }
            }
            Ok(None) => {
                // GOAWAY — peer is shutting down.
                tracing::debug!("h2 GOAWAY received, closing connection");
                return None;
            }
            Err(ref e) if e.kind() == io::ErrorKind::WouldBlock => {
                self.pump_responses();
                // Log the first WouldBlock to confirm the server is alive and waiting.
                if self.idle_since.is_none() {
                    eprintln!("[h2-srv] WouldBlock — waiting for frames, 0 streams");
                }
                return self.park();
            }
            Err(e) => {
                eprintln!("[h2-srv] read_stream_frame error: {e}");
                return None;
            }
        }

        self.pump_responses();
        Some(TaskStatus::Pending(()))
    }
}

impl H2ConnectionHandler {
    /// No frame was available. Park on a timer; give up only once the connection
    /// has sat idle with no live streams for longer than [`IDLE_TIMEOUT`].
    fn park(&mut self) -> Option<TaskStatus<(), (), BoxedSendExecutionAction>> {
        if !self.streams.is_empty() {
            self.idle_since = None;
            return Some(TaskStatus::Delayed(POLL_DELAY));
        }
        match self.idle_since {
            None => {
                self.idle_since = Some(Instant::now());
                Some(TaskStatus::Delayed(POLL_DELAY))
            }
            Some(since) if since.elapsed() < IDLE_TIMEOUT => Some(TaskStatus::Delayed(POLL_DELAY)),
            Some(_) => None,
        }
    }

    /// A new stream: decode headers, route it, and spawn its handler future.
    fn on_headers(&mut self, head: &Head, payload: &[u8]) {
        let sid = head.stream_id;
        let hf = match HeadersFrame::parse(head, payload) {
            Ok(hf) => hf,
            Err(_) => return self.reset_stream(sid, ErrorCode::FrameSizeError),
        };

        let decoded = match self.conn.decode_headers(&hf.header_block) {
            Ok(d) => d,
            Err(_) => return self.reset_stream(sid, ErrorCode::CompressionError),
        };

        let header = match header_from_hpack(&decoded, self.connection.clone()) {
            Ok(h) => h,
            Err(e) => {
                tracing::debug!(stream = sid, err = %e, "h2 request URI malformed");
                return self.reset_stream(sid, ErrorCode::ProtocolError);
            }
        };

        let Some(handler) = self.app.router().dispatch(&header.method, &header.url.url) else {
            // No route is a complete 404 response, not a stream error.
            tracing::debug!(stream = sid, method = %header.method, path = %header.url.url, "h2: no route → 404");
            self.conn.encode_frame(
                sid,
                &H2Frame::Headers {
                    status: 404,
                    headers: Vec::new(),
                    end_stream: true,
                },
            );
            return;
        };
        tracing::debug!(stream = sid, method = %header.method, path = %header.url.url, "h2: request routed");

        let (body_tx, body_rx) = Pipe::<H2IncomingFrame>::with_depth(STREAM_PIPE_DEPTH);
        let (resp_tx, resp_rx) = Pipe::<H2Frame>::with_depth(STREAM_PIPE_DEPTH);

        // HEADERS carrying END_STREAM means the request has no body at all.
        if hf.flags & headers_flags::END_STREAM != 0 {
            body_tx.close();
        }

        let fut = handler.serve_h2(self.app.context().clone(), header, body_rx, resp_tx);
        if foundation_core::valtron::send(FutureTask::new(fut)).is_err() {
            tracing::error!(stream = sid, "failed to spawn h2 stream handler");
            return self.reset_stream(sid, ErrorCode::RefusedStream);
        }

        self.streams.insert(
            sid,
            ActiveStream {
                body_tx,
                resp_rx,
                response_done: false,
            },
        );
    }

    /// Inbound body bytes: forward them to the stream's handler future.
    fn on_data(&mut self, head: &Head, payload: &[u8]) {
        let sid = head.stream_id;
        let end_stream = head.flag & data_flags::END_STREAM != 0;

        let Some(stream) = self.streams.get(&sid) else {
            return;
        };

        if !payload.is_empty() {
            let frame = H2IncomingFrame::Data(Bytes::copy_from_slice(payload));
            if stream.body_tx.try_send(frame).is_err() {
                // The handler is slower than the peer, or has hung up. Flow
                // control should prevent the former; either way, refuse the
                // stream rather than silently dropping body bytes.
                return self.reset_stream(sid, ErrorCode::FlowControlError);
            }
        }

        if end_stream {
            stream.body_tx.close();
        }
    }

    /// The peer cancelled: tell the handler future, then stop tracking the stream.
    fn on_reset(&mut self, head: &Head, payload: &[u8]) {
        let sid = head.stream_id;
        let code = ResetFrame::parse(head, payload).map_or(ErrorCode::Cancel, |rf| rf.error_code);

        if let Some(stream) = self.streams.remove(&sid) {
            let _ = stream.body_tx.try_send(H2IncomingFrame::Reset(code));
            stream.body_tx.close();
        }
    }

    /// Serialize a stream error and drop the stream.
    fn reset_stream(&mut self, sid: u32, code: ErrorCode) {
        self.conn
            .encode_frame(sid, &H2Frame::Reset { error_code: code });
        if let Some(stream) = self.streams.remove(&sid) {
            stream.body_tx.close();
        }
    }

    /// Drain every live stream's response pipe onto the shared write buffer, then
    /// reap finished streams. This is the only place `H2Conn` is written, so
    /// frames from different streams never interleave mid-frame.
    fn pump_responses(&mut self) {
        let mut finished: Vec<u32> = Vec::new();

        for (&sid, stream) in &mut self.streams {
            loop {
                match stream.resp_rx.try_recv() {
                    Ok(frame) => {
                        if frame_ends_stream(&frame) {
                            stream.response_done = true;
                        }
                        self.conn.encode_frame(sid, &frame);
                    }
                    Err(TryRecvError::Empty) => break,
                    Err(TryRecvError::Closed) => {
                        // The handler dropped its sender. If it never signalled
                        // END_STREAM the response is truncated, so reset rather
                        // than leave the peer waiting forever.
                        if !stream.response_done {
                            self.conn.encode_frame(
                                sid,
                                &H2Frame::Reset {
                                    error_code: ErrorCode::InternalError,
                                },
                            );
                        }
                        finished.push(sid);
                        break;
                    }
                }
            }
        }

        for sid in finished {
            if let Some(stream) = self.streams.remove(&sid) {
                stream.body_tx.close();
            }
        }

        self.conn.flush().ok();
    }
}

/// Generate 8 opaque bytes for a keepalive PING.
fn new_ping_opaque_h2srv() -> [u8; 8] {
    use std::time::{SystemTime, UNIX_EPOCH};
    let ns = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_nanos() as u64;
    ns.to_be_bytes()
}

/// Whether serializing this frame completes the response direction.
fn frame_ends_stream(frame: &H2Frame) -> bool {
    match frame {
        H2Frame::Headers { end_stream, .. } | H2Frame::Data { end_stream, .. } => *end_stream,
        H2Frame::Reset { .. } => true,
    }
}
