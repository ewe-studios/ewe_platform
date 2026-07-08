//! HTTP/2 connection state machine — shared by server and client multiplexers.
//!
//! WHY: Both server and client need the same core loop: exchange the connection
//! preface, negotiate SETTINGS, and then read/write frames managing per-stream
//! state (flow control, HPACK contexts, stream life cycles). This module
//! provides that shared core.
//!
//! WHAT: [`H2Connection`] owns the socket, frame I/O, HPACK encoder/decoder,
//! connection-level flow control, and per-stream bookkeeping. Server and client
//! layers plug in via callbacks: `on_new_stream` for incoming requests
//! (server), and per-stream write queues for outgoing frames.
//!
//! HOW: The main loop reads one frame at a time. SETTINGS/WINDOW_UPDATE/PING/
//! GOAWAY are handled connection-locally. HEADERS frames create new stream
//! entries and invoke the callback. DATA frames are routed to the stream's
//! handler. Frames destined for the peer are queued in `write_buf` and flushed.

use std::collections::BTreeMap;
use std::io::{self, Read, Write};

use bytes::{BufMut, Bytes, BytesMut};

use crate::http2::flow_control::{FlowControl, WindowSize};
use crate::http2::frame::*;
use crate::http2::hpack;
use crate::http2::settings::SettingsStore;
use crate::http2::stream::StreamState;

/// Client connection preface (RFC 7540 §3.5).
pub const CLIENT_PREFACE: &[u8] = b"PRI * HTTP/2.0\r\n\r\nSM\r\n\r\n";

/// Length of the client connection preface in bytes.
pub const CLIENT_PREFACE_LEN: usize = 24;

/// Wrapper around a stream identifier to enforce RFC 7540 §5.1.1 semantics.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub struct StreamId(pub u32);

impl StreamId {
    /// The connection-wide stream (0).
    pub const CONNECTION: Self = StreamId(0);

    /// Whether this is a client-initiated stream (odd stream ID).
    pub fn is_client_initiated(self) -> bool { self.0 % 2 == 1 }

    /// Whether this is a server-initiated (push) stream (even stream ID).
    pub fn is_server_initiated(self) -> bool { self.0 > 0 && self.0 % 2 == 0 }
}

impl From<u32> for StreamId {
    fn from(id: u32) -> Self { StreamId(id) }
}

impl From<StreamId> for u32 {
    fn from(s: StreamId) -> u32 { s.0 }
}

/// A decoded HTTP/2 request/response — pseudo-headers + regular headers + body.
#[derive(Debug, Clone)]
pub struct H2Request {
    pub method: Bytes,
    pub scheme: Bytes,
    pub authority: Bytes,
    pub path: Bytes,
    pub headers: Vec<(Bytes, Bytes)>,
    pub body: Option<Bytes>, // Non-streaming: full body if END_STREAM on HEADERS
    pub end_stream: bool,
}

/// A response to send on a stream.
#[derive(Debug, Clone)]
pub struct H2Response {
    pub status: u16,
    pub headers: Vec<(Bytes, Bytes)>,
    pub body: Option<Bytes>,
    pub end_stream: bool,
}

/// Callback invoked when the connection receives a new incoming stream.
pub trait StreamHandler: Send {
    /// Called when a new HEADERS frame arrives on a previously-idle stream.
    /// The handler should return the response to send.
    fn handle_request(&mut self, stream_id: StreamId, request: H2Request) -> H2Response;

    /// Called when a DATA frame arrives on an open stream.
    fn handle_data(&mut self, _stream_id: StreamId, _data: &[u8], _end_stream: bool) {
        // Default: accumulate (overridden by streaming handlers)
    }

    /// Called when a RST_STREAM arrives.
    fn handle_reset(&mut self, _stream_id: StreamId, _error_code: ErrorCode) {}
}

/// Per-stream entry tracked by the connection.
struct StreamEntry {
    state: StreamState,
    flow: FlowControl,
}

/// Core HTTP/2 connection state machine.
///
/// Owns the socket, frame I/O buffering, HPACK contexts, and per-stream state.
/// Both server and client layers compose with this.
pub struct H2Connection<S: Read + Write> {
    socket: S,
    write_buf: BytesMut,

    local_settings: SettingsStore,
    remote_settings: SettingsStore,

    hpack_dec: hpack::Decoder,
    hpack_enc: hpack::Encoder,

    conn_flow: FlowControl,
    remote_conn_window: i32, // Track the peer's connection window (for DATA sends)

    streams: BTreeMap<u32, StreamEntry>,

    /// Next stream ID we will use for outgoing requests (server: even, client: odd).
    next_outgoing_id: u32,
    /// The last stream ID we've seen from the peer.
    last_peer_stream_id: u32,

    preface_sent: bool,
    preface_received: bool,
    settings_sent: bool,
    waiting_for_settings_ack: bool,
    goaway_sent: bool,
    goaway_received: bool,
}

/// Convert a `&'static str` protocol error into an `io::Error`.
fn proto_err(msg: &'static str) -> io::Error {
    io::Error::new(io::ErrorKind::InvalidData, msg)
}

impl<S: Read + Write> H2Connection<S> {
    /// Create a new HTTP/2 connection.
    ///
    /// `is_server` selects the server-side stream-ID allocation (even IDs for pushes,
    /// responds to client-initiated odd IDs) and the preface order.
    pub fn new(socket: S, is_server: bool) -> Self {
        let mut hpack_enc = hpack::Encoder::new();
        hpack_enc.table_mut().set_max_size(4096);

        Self {
            socket,
            write_buf: BytesMut::with_capacity(16384),
            local_settings: SettingsStore::default(),
            remote_settings: SettingsStore::default(),
            hpack_dec: hpack::Decoder::new(),
            hpack_enc,
            conn_flow: FlowControl::new(),
            remote_conn_window: 65535,
            streams: BTreeMap::new(),
            next_outgoing_id: if is_server { 2 } else { 1 },
            last_peer_stream_id: 0,
            preface_sent: false,
            preface_received: false,
            settings_sent: false,
            waiting_for_settings_ack: false,
            goaway_sent: false,
            goaway_received: false,
        }
    }

    // ── Handshake ──────────────────────────────────────────────────────

    /// Run the client-side handshake: send preface + SETTINGS, read server
    /// preface + SETTINGS + ACK, send ACK back.
    pub fn client_handshake(&mut self) -> io::Result<()> {
        // 1. Send client preface
        self.write_buf.put_slice(CLIENT_PREFACE);
        self.flush_write()?;

        // 2. Send our SETTINGS
        let settings_frame = self.local_settings.to_frame();
        settings_frame.encode(&mut self.write_buf);
        self.flush_write()?;
        self.settings_sent = true;
        self.waiting_for_settings_ack = true;

        // 3. Read until we get SETTINGS + SETTINGS ACK from server
        let mut got_settings = false;
        let mut got_ack = false;
        while !got_settings || !got_ack {
            let (head, payload) = self.read_frame()?;
            match head.kind {
                Kind::Settings => {
                    if head.flag & settings_flags::ACK != 0 {
                        got_ack = true;
                    } else {
                        let sf = SettingsFrame::parse(&head, &payload).map_err(proto_err)?;
                        let _changes = self.remote_settings.apply(&sf.settings).map_err(proto_err)?;
                        // ACK the settings
                        SettingsFrame::ack().encode(&mut self.write_buf);
                        self.flush_write()?;
                        got_settings = true;
                    }
                }
                _ => return Err(io::Error::new(io::ErrorKind::InvalidData, "unexpected frame during handshake")),
            }
        }
        self.preface_sent = true;
        self.preface_received = true;
        Ok(())
    }

    /// Run the server-side handshake: read client preface + SETTINGS, send
    /// our SETTINGS, wait for client ACK.
    pub fn server_handshake(&mut self) -> io::Result<()> {
        // 1. Read client preface (24 bytes)
        let mut preface_buf = [0u8; CLIENT_PREFACE_LEN];
        self.socket.read_exact(&mut preface_buf)?;
        if &preface_buf != CLIENT_PREFACE {
            return Err(io::Error::new(io::ErrorKind::InvalidData, "invalid HTTP/2 connection preface"));
        }

        // 2. Read client SETTINGS
        let (head, payload) = self.read_frame()?;
        if head.kind != Kind::Settings || head.flag & settings_flags::ACK != 0 {
            return Err(io::Error::new(io::ErrorKind::InvalidData, "expected SETTINGS frame after preface"));
        }
        let sf = SettingsFrame::parse(&head, &payload).map_err(proto_err)?;
        let _changes = self.remote_settings.apply(&sf.settings).map_err(proto_err)?;

        // 3. ACK client SETTINGS
        SettingsFrame::ack().encode(&mut self.write_buf);
        self.flush_write()?;

        // 4. Send our SETTINGS
        self.local_settings.to_frame().encode(&mut self.write_buf);
        self.flush_write()?;
        self.settings_sent = true;
        self.waiting_for_settings_ack = true;

        // 5. Wait for client SETTINGS ACK
        let (head, payload) = self.read_frame()?;
        if head.kind != Kind::Settings || head.flag & settings_flags::ACK == 0 {
            return Err(io::Error::new(io::ErrorKind::InvalidData, "expected SETTINGS ACK"));
        }
        // Validate empty payload
        if !payload.is_empty() {
            return Err(io::Error::new(io::ErrorKind::InvalidData, "SETTINGS ACK must have empty payload"));
        }

        self.waiting_for_settings_ack = false;
        self.preface_sent = true;
        self.preface_received = true;
        Ok(())
    }

    // ── Frame I/O ──────────────────────────────────────────────────────

    /// Public: read a raw frame from the socket, returning `(head, payload_bytes)`.
    /// Useful for manual frame-processing loops (streaming servers, etc.).
    pub fn read_frame(&mut self) -> io::Result<(Head, Bytes)> {
        // Read the 9-byte header
        let mut header = [0u8; 9];
        self.socket.read_exact(&mut header)?;
        let (head, payload_len) = Head::parse_with_len(&header);

        // Validate payload length against remote max frame size
        let max_frame = self.remote_settings.get(SettingId::MaxFrameSize);
        if payload_len > max_frame {
            return Err(io::Error::new(io::ErrorKind::InvalidData,
                format!("frame payload {} exceeds SETTINGS_MAX_FRAME_SIZE {}", payload_len, max_frame)));
        }

        // Read payload
        let mut payload = vec![0u8; payload_len as usize];
        self.socket.read_exact(&mut payload)?;

        Ok((head, Bytes::from(payload)))
    }

    /// Write all buffered frames to the socket.
    fn flush_write(&mut self) -> io::Result<()> {
        if !self.write_buf.is_empty() {
            self.socket.write_all(&self.write_buf)?;
            self.socket.flush()?;
            self.write_buf.clear();
        }
        Ok(())
    }

    /// Queue a frame for writing (call `flush_write` to send).
    fn queue_frame(&mut self, head: &Head, payload: &[u8]) {
        head.encode(payload.len() as u32, &mut self.write_buf);
        self.write_buf.put_slice(payload);
    }

    // ── Stream management ──────────────────────────────────────────────

    /// Allocate a new outgoing stream ID.
    fn alloc_stream_id(&mut self) -> Option<u32> {
        let max = self.remote_settings.get(SettingId::MaxConcurrentStreams);
        if self.streams.len() >= max as usize {
            return None;
        }
        let id = self.next_outgoing_id;
        self.next_outgoing_id += 2;
        self.streams.insert(id, StreamEntry {
            state: StreamState::Idle,
            flow: FlowControl::new(),
        });
        Some(id)
    }

    // ── Main event loop ────────────────────────────────────────────────

    /// Process the next incoming frame, calling the handler as needed.
    /// Returns `true` if the connection should continue, `false` to close.
    pub fn process_next<H: StreamHandler>(&mut self, handler: &mut H) -> io::Result<bool> {
        let (head, payload) = self.read_frame()?;
        let stream_id = head.stream_id;

        match head.kind {
            Kind::Headers => {
                self.handle_headers(head, &payload, handler)?;
            }
            Kind::Data => {
                self.handle_data(head, &payload, handler)?;
            }
            Kind::Settings => {
                self.handle_settings(head, &payload)?;
            }
            Kind::WindowUpdate => {
                self.handle_window_update(head, &payload)?;
            }
            Kind::Ping => {
                self.handle_ping(head, &payload)?;
            }
            Kind::GoAway => {
                self.goaway_received = true;
            }
            Kind::Reset => {
                self.handle_reset(head, &payload, handler)?;
            }
            Kind::Priority => {
                // Priority is optional — ignore for now.
            }
            Kind::PushPromise => {
                // Reject PUSH_PROMISE per T6
                self.enqueue_reset(stream_id, ErrorCode::RefusedStream);
            }
            Kind::Continuation => {
                return Err(io::Error::new(io::ErrorKind::InvalidData,
                    "CONTINUATION not yet supported in this implementation"));
            }
        }

        self.flush_write()?;
        Ok(!self.goaway_received || !self.streams.is_empty())
    }

    // ── Frame handlers ─────────────────────────────────────────────────

    fn handle_headers<H: StreamHandler>(&mut self, head: Head, payload: &[u8], handler: &mut H) -> io::Result<()> {
        let hf = HeadersFrame::parse(&head, payload)
            .map_err(proto_err)?;

        // Decode HPACK header block
        let decoded = self.hpack_dec.decode(&hf.header_block)
            .map_err(proto_err)?;

        // Map pseudo-headers and regular headers
        let request = self.build_request(&decoded, hf.flags & headers_flags::END_STREAM != 0);

        let stream_id = StreamId::from(head.stream_id);

        // Check if this is a new stream
        let is_idle = self.streams.get(&head.stream_id)
            .map(|e| e.state == StreamState::Idle)
            .unwrap_or(false);

        if is_idle {
            // Transition to Open
            if let Some(entry) = self.streams.get_mut(&head.stream_id) {
                entry.state = StreamState::Open;
                let initial_window = self.local_settings.get(SettingId::InitialWindowSize);
                entry.flow.inc_window(initial_window).ok();
            }
        } else if !self.streams.contains_key(&head.stream_id) {
            // New stream from peer
            let mut state = StreamState::Idle;
            state.recv_headers(hf.flags & headers_flags::END_STREAM != 0)
                .map_err(|_| io::Error::new(io::ErrorKind::InvalidData, "illegal stream state for HEADERS"))?;

            let mut entry = StreamEntry {
                state,
                flow: FlowControl::new(),
            };
            let initial_window = self.local_settings.get(SettingId::InitialWindowSize);
            entry.flow.inc_window(initial_window).ok();
            self.streams.insert(head.stream_id, entry);
            self.last_peer_stream_id = head.stream_id;
        }

        // Dispatch to handler
        let response = handler.handle_request(stream_id, request);

        // Encode response
        self.send_response(head.stream_id, response)?;

        Ok(())
    }

    fn handle_data<H: StreamHandler>(&mut self, head: Head, payload: &[u8], handler: &mut H) -> io::Result<()> {
        let df = DataFrame::parse(&head, payload)
            .map_err(proto_err)?;

        let stream_id = StreamId::from(head.stream_id);
        let end_stream = df.flags & data_flags::END_STREAM != 0;

        // Update flow control: credit the peer's window
        let increment = payload.len() as WindowSize;
        if let Some(entry) = self.streams.get_mut(&head.stream_id) {
            entry.flow.assign_capacity(increment).ok();
        }
        // Send WINDOW_UPDATE if needed
        self.conn_flow.assign_capacity(increment).ok();
        if let Some(inc) = self.conn_flow.unclaimed_capacity() {
            let wu = WindowUpdateFrame { stream_id: 0, size_increment: inc };
            let mut buf = BytesMut::new();
            wu.encode(&mut buf);
            self.queue_frame(&Head { kind: Kind::WindowUpdate, flag: 0, stream_id: 0 }, &buf);
        }

        // Update stream state
        if let Some(entry) = self.streams.get_mut(&head.stream_id) {
            if end_stream {
                entry.state.recv_data(true).ok();
            }
        }

        handler.handle_data(stream_id, &df.data, end_stream);
        Ok(())
    }

    fn handle_settings(&mut self, head: Head, payload: &[u8]) -> io::Result<()> {
        if head.stream_id != 0 {
            return Err(io::Error::new(io::ErrorKind::InvalidData, "SETTINGS must be on stream 0"));
        }

        if head.flag & settings_flags::ACK != 0 {
            // Peer acknowledged our SETTINGS
            self.waiting_for_settings_ack = false;
            return Ok(());
        }

        let sf = SettingsFrame::parse(&head, payload)
            .map_err(proto_err)?;

        let changes = self.remote_settings.apply(&sf.settings)
            .map_err(proto_err)?;

        for change in &changes {
            if change.id == SettingId::InitialWindowSize {
                let diff = change.new as i64 - change.old as i64;
                if diff > 0 {
                    // Increase existing stream windows
                } else {
                    // Decrease — adjust flow control
                }
            }
        }

        // ACK
        let ack = SettingsFrame::ack();
        let mut buf = BytesMut::new();
        ack.encode(&mut buf);
        self.queue_frame(&Head { kind: Kind::Settings, flag: settings_flags::ACK, stream_id: 0 }, &buf);
        Ok(())
    }

    fn handle_window_update(&mut self, head: Head, payload: &[u8]) -> io::Result<()> {
        let wu = WindowUpdateFrame::parse(&head, payload)
            .map_err(proto_err)?;

        if head.stream_id == 0 {
            self.remote_conn_window = self.remote_conn_window.saturating_add(wu.size_increment as i32);
        } else if let Some(entry) = self.streams.get_mut(&head.stream_id) {
            entry.flow.inc_window(wu.size_increment).ok();
        }
        Ok(())
    }

    fn handle_ping(&mut self, head: Head, payload: &[u8]) -> io::Result<()> {
        if head.stream_id != 0 {
            return Err(io::Error::new(io::ErrorKind::InvalidData, "PING must be on stream 0"));
        }
        if head.flag & ping_flags::ACK != 0 {
            // PING ACK — nothing to do
            return Ok(());
        }
        // Respond with PING ACK
        let pf = PingFrame::parse(&head, payload)
            .map_err(proto_err)?;
        let ack = PingFrame::ack(pf.opaque_data);
        let mut buf = BytesMut::new();
        ack.encode(&mut buf);
        self.queue_frame(&Head { kind: Kind::Ping, flag: ping_flags::ACK, stream_id: 0 }, &buf);
        Ok(())
    }

    fn handle_reset<H: StreamHandler>(&mut self, head: Head, payload: &[u8], handler: &mut H) -> io::Result<()> {
        let rf = ResetFrame::parse(&head, payload)
            .map_err(proto_err)?;

        if let Some(entry) = self.streams.get_mut(&head.stream_id) {
            entry.state = StreamState::Closed;
        }
        handler.handle_reset(StreamId::from(head.stream_id), rf.error_code);
        Ok(())
    }

    // ── Sending ────────────────────────────────────────────────────────

    /// Send a response on the given stream.
    pub fn send_response(&mut self, stream_id: u32, response: H2Response) -> io::Result<()> {
        // Build HPACK-encoded response HEADERS
        let mut header_block = BytesMut::new();

        // Pseudo-header :status
        let status_bytes = response.status.to_string();
        self.hpack_enc.encode_header_no_index(b":status", status_bytes.as_bytes(), &mut header_block);

        for (name, value) in &response.headers {
            self.hpack_enc.encode_header(name, value, &mut header_block);
        }

        let mut flags = headers_flags::END_HEADERS;
        if response.body.is_none() && response.end_stream {
            flags |= headers_flags::END_STREAM;
        }

        let hf = HeadersFrame {
            stream_id,
            flags,
            header_block: header_block.freeze(),
            pad_len: None,
            priority: None,
        };

        let mut buf = BytesMut::new();
        hf.encode(&mut buf);
        self.write_buf.put_slice(&buf);

        // If there's a body, send it as a single DATA frame
        if let Some(body) = &response.body {
            let df = DataFrame {
                stream_id,
                flags: if response.end_stream { data_flags::END_STREAM } else { 0 },
                data: body.clone(),
                pad_len: None,
            };
            let mut buf = BytesMut::new();
            df.encode(&mut buf);
            self.write_buf.put_slice(&buf);
        }

        // Update stream state
        if let Some(entry) = self.streams.get_mut(&stream_id) {
            entry.state.send_headers(response.body.is_none() && response.end_stream).ok();
        }

        Ok(())
    }

    /// Send a GOAWAY frame.
    pub fn send_goaway(&mut self, last_stream_id: u32, error_code: ErrorCode) {
        let gf = GoAwayFrame {
            last_stream_id,
            error_code,
            debug_data: Bytes::new(),
        };
        let mut buf = BytesMut::new();
        gf.encode(&mut buf);
        self.queue_frame(&Head { kind: Kind::GoAway, flag: 0, stream_id: 0 }, &buf);
        self.goaway_sent = true;
    }

    /// Send RST_STREAM on the given stream.
    fn enqueue_reset(&mut self, stream_id: u32, error_code: ErrorCode) {
        let rf = ResetFrame { stream_id, error_code };
        let mut buf = BytesMut::new();
        rf.encode(&mut buf);
        self.queue_frame(&Head { kind: Kind::Reset, flag: 0, stream_id }, &buf);
    }

    /// Open a new outgoing stream (client-side) and send a request.
    pub fn send_request(&mut self, request: H2Request) -> io::Result<u32> {
        let stream_id = self.alloc_stream_id()
            .ok_or_else(|| io::Error::new(io::ErrorKind::WouldBlock, "no available streams"))?;

        // Build HPACK request
        let mut header_block = BytesMut::new();
        self.hpack_enc.encode_header(b":method", &request.method, &mut header_block);
        self.hpack_enc.encode_header(b":scheme", &request.scheme, &mut header_block);
        self.hpack_enc.encode_header(b":authority", &request.authority, &mut header_block);
        self.hpack_enc.encode_header(b":path", &request.path, &mut header_block);
        for (name, value) in &request.headers {
            self.hpack_enc.encode_header(name, value, &mut header_block);
        }

        let mut flags = headers_flags::END_HEADERS;
        if request.body.is_none() && request.end_stream {
            flags |= headers_flags::END_STREAM;
        }

        let hf = HeadersFrame {
            stream_id,
            flags,
            header_block: header_block.freeze(),
            pad_len: None,
            priority: None,
        };

        let mut buf = BytesMut::new();
        hf.encode(&mut buf);
        self.write_buf.put_slice(&buf);

        // Send body if present
        if let Some(body) = &request.body {
            let df = DataFrame {
                stream_id,
                flags: if request.end_stream { data_flags::END_STREAM } else { 0 },
                data: body.clone(),
                pad_len: None,
            };
            let mut buf = BytesMut::new();
            df.encode(&mut buf);
            self.write_buf.put_slice(&buf);
        }

        // Update stream state
        if let Some(entry) = self.streams.get_mut(&stream_id) {
            entry.state.send_headers(request.body.is_none() && request.end_stream).ok();
        }

        self.flush_write()?;
        Ok(stream_id)
    }

    /// Send response HEADERS on the given stream without a body, for streaming.
    /// The caller follows up with [`send_data_frame`] calls and ends with
    /// `send_data_frame(stream_id, data, true)` or an empty
    /// `send_data_frame(stream_id, &[], true)`.
    pub fn send_headers_response(&mut self, stream_id: u32, status: u16, headers: &[(Bytes, Bytes)], end_stream: bool) -> io::Result<()> {
        let mut header_block = BytesMut::new();
        let status_bytes = status.to_string();
        self.hpack_enc.encode_header_no_index(b":status", status_bytes.as_bytes(), &mut header_block);
        for (name, value) in headers {
            self.hpack_enc.encode_header(name, value, &mut header_block);
        }

        let mut flags = headers_flags::END_HEADERS;
        if end_stream { flags |= headers_flags::END_STREAM; }

        let hf = HeadersFrame { stream_id, flags, header_block: header_block.freeze(), pad_len: None, priority: None };
        let mut buf = BytesMut::new();
        hf.encode(&mut buf);
        self.write_buf.put_slice(&buf);

        if let Some(entry) = self.streams.get_mut(&stream_id) {
            entry.state.send_headers(end_stream).ok();
        }
        self.flush_write()?;
        Ok(())
    }

    /// Send a DATA frame on an already-open stream.
    /// The first call after `send_headers_response` or `send_request` must have
    /// `end_stream: false` for multi-frame streaming; the final frame sets
    /// `end_stream: true`.
    pub fn send_data_frame(&mut self, stream_id: u32, data: &[u8], end_stream: bool) -> io::Result<()> {
        let df = DataFrame {
            stream_id,
            flags: if end_stream { data_flags::END_STREAM } else { 0 },
            data: Bytes::copy_from_slice(data),
            pad_len: None,
        };
        let mut buf = BytesMut::new();
        df.encode(&mut buf);
        self.write_buf.put_slice(&buf);

        if let Some(entry) = self.streams.get_mut(&stream_id) {
            entry.state.send_data(end_stream).ok();
        }
        self.flush_write()?;
        Ok(())
    }

    /// Read the next frame, returning `Some((stream_id, data, end_stream))` for a
    /// DATA frame, or `Ok(None)` for non-DATA frames (SETTINGS/PING/etc handled
    /// internally). Call in a loop until `end_stream` is true.
    pub fn recv_data_frame(&mut self) -> io::Result<Option<(u32, Bytes, bool)>> {
        loop {
            let (head, payload) = self.read_frame()?;
            match head.kind {
                Kind::Data => {
                    let df = DataFrame::parse(&head, &payload).map_err(proto_err)?;
                    let end_stream = df.flags & data_flags::END_STREAM != 0;
                    return Ok(Some((head.stream_id, df.data, end_stream)));
                }
                Kind::Settings => self.handle_settings(head, &payload)?,
                Kind::WindowUpdate => self.handle_window_update(head, &payload)?,
                Kind::Ping => self.handle_ping(head, &payload)?,
                Kind::GoAway => { self.goaway_received = true; return Ok(None); }
                Kind::Reset => { /* pass through */ return Ok(None); }
                Kind::Headers => { /* unexpected — pass through */ return Ok(None); }
                _ => {}
            }
            self.flush_write()?;
        }
    }

    /// Read the next incoming frame, returning `Some(response)` if it's a
    /// response HEADERS frame for a stream we initiated.
    pub fn recv_response(&mut self) -> io::Result<Option<(u32, H2Request)>> {
        loop {
            let (head, payload) = self.read_frame()?;

            match head.kind {
                Kind::Headers => {
                    let hf = HeadersFrame::parse(&head, &payload)
                        .map_err(proto_err)?;

                    let decoded = self.hpack_dec.decode(&hf.header_block)
                        .map_err(proto_err)?;

                    let response = self.build_request(&decoded, hf.flags & headers_flags::END_STREAM != 0);
                    return Ok(Some((head.stream_id, response)));
                }
                Kind::Settings => self.handle_settings(head, &payload)?,
                Kind::WindowUpdate => self.handle_window_update(head, &payload)?,
                Kind::Ping => self.handle_ping(head, &payload)?,
                Kind::GoAway => { self.goaway_received = true; return Ok(None); }
                Kind::Reset => { /* ignore for client */ }
                _ => {} // ignore other frames on client
            }
            self.flush_write()?;
        }
    }

    /// Flush any pending writes.
    pub fn flush(&mut self) -> io::Result<()> {
        self.flush_write()
    }

    // ── Helpers ────────────────────────────────────────────────────────

    /// Reference to the underlying socket (for tests/inspection).
    pub fn socket_ref(&self) -> &S {
        &self.socket
    }

    /// Consume this connection and return the inner socket.
    pub fn into_inner(self) -> S {
        self.socket
    }

    /// Build an H2Request from decoded HPACK headers.
    #[doc(hidden)]
    pub fn build_request(&self, headers: &[(Bytes, Bytes)], end_stream: bool) -> H2Request {
        let mut method = Bytes::from_static(b"GET");
        let mut scheme = Bytes::from_static(b"https");
        let mut authority = Bytes::new();
        let mut path = Bytes::from_static(b"/");
        let mut regular_headers = Vec::new();

        for (name, value) in headers {
            if name.as_ref() == b":method" {
                method = value.clone();
            } else if name.as_ref() == b":scheme" {
                scheme = value.clone();
            } else if name.as_ref() == b":authority" {
                authority = value.clone();
            } else if name.as_ref() == b":path" {
                path = value.clone();
            } else if name.as_ref() == b":status" {
                // Response pseudo-header — keep as regular header for now
                regular_headers.push((name.clone(), value.clone()));
            } else {
                regular_headers.push((name.clone(), value.clone()));
            }
        }

        H2Request {
            method,
            scheme,
            authority,
            path,
            headers: regular_headers,
            body: None,
            end_stream,
        }
    }
}
