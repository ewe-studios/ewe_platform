//! `H2Channel` — non-blocking, I/O-agnostic HTTP/2 connection state machine.
//!
//! WHY: `H2Connection<S: Read+Write>` works for blocking sockets, but valtron
//! needs push/pull byte I/O — the pump feeds bytes from the fd, drives the
//! state machine, and drains output to send. This module provides that core.
//!
//! WHAT: All of `H2Connection`'s logic but with byte-buffer I/O instead of
//! `Read + Write`. Methods that used `read_exact` consume from `read_buf` and
//! return `WouldBlock` when more data is needed. Methods that used `write_all`
//! append to `write_buf` for the caller to drain.
//!
//! HOW: Reuses the same internal state (hpack, settings, streams, flow control)
//! from `H2Connection`. The blocking path simply wraps this in a `loop { fill +
//! step + flush }`; the valtron path does the same across many poll cycles.

use std::collections::BTreeMap;
use std::io;

use bytes::{Buf, BufMut, Bytes, BytesMut};

use crate::http2::flow_control::FlowControl;
use crate::http2::frame::{
    data_flags, headers_flags, ping_flags, settings_flags, DataFrame, ErrorCode, GoAwayFrame, Head,
    HeadersFrame, Kind, PingFrame, ResetFrame, SettingId, SettingsFrame, WindowUpdateFrame,
};
use crate::http2::hpack;
use crate::http2::settings::SettingsStore;
use crate::http2::stream::StreamState;

use super::connection::{H2Request, H2Response, CLIENT_PREFACE, CLIENT_PREFACE_LEN};

/// Sentinel I/O error for "not enough data buffered — feed more".
fn would_block() -> io::Error {
    io::Error::new(io::ErrorKind::WouldBlock, "h2 channel: need more data")
}

fn proto_err(msg: &'static str) -> io::Error {
    io::Error::new(io::ErrorKind::InvalidData, msg)
}

/// Per-stream entry.
struct StreamEntry {
    state: StreamState,
    flow: FlowControl,
}

/// Batch consumed-byte credits and flush once this many are pending (half the
/// default 64 KiB stream window). See
/// [`H2Channel::credit_received_data`].
const CREDIT_FLUSH_THRESHOLD: u32 = 32 * 1024;

/// Non-blocking HTTP/2 connection state machine.
///
/// Same state as `H2Connection` but replaces the `S: Read+Write` socket with
/// two `BytesMut` buffers. Callers push received bytes via [`feed_input`],
/// pull output bytes via [`drain_output`], and drive the state machine via
/// the regular connection methods (handshake, `send_request`, `recv_response`,
/// `send_data_frame`, `recv_data_frame`, etc.).
///
/// Every method that needs more data returns `Err(io::ErrorKind::WouldBlock)` —
/// the caller feeds more bytes and retries.
///
/// [`feed_input`]: Self::feed_input
/// [`drain_output`]: Self::drain_output
pub struct H2Channel {
    read_buf: BytesMut,
    write_buf: BytesMut,

    local_settings: SettingsStore,
    remote_settings: SettingsStore,

    hpack_dec: hpack::Decoder,
    hpack_enc: hpack::Encoder,

    _conn_flow: FlowControl,
    remote_conn_window: i32,
    /// Received DATA bytes not yet credited back to the peer's connection
    /// window — flushed as one WINDOW_UPDATE at [`CREDIT_FLUSH_THRESHOLD`].
    uncredited_conn: u32,
    /// Same, per stream (keyed by stream id; entries removed as they flush).
    uncredited_streams: BTreeMap<u32, u32>,

    streams: BTreeMap<u32, StreamEntry>,

    next_outgoing_id: u32,
    _last_peer_stream_id: u32,

    preface_sent: bool,
    preface_received: bool,
    settings_sent: bool,
    waiting_for_settings_ack: bool,
    goaway_sent: bool,
    goaway_received: bool,
}

/// One event on a stream after the initial response headers have been delivered.
#[derive(Debug)]
pub enum H2StreamEvent {
    /// A DATA frame chunk (may be empty, `end_stream` is the flag).
    Data {
        stream_id: u32,
        data: Bytes,
        end_stream: bool,
    },
    /// A HEADERS frame carrying trailing metadata (`END_STREAM` + `END_HEADERS`).
    Trailers {
        stream_id: u32,
        headers: Vec<(Bytes, Bytes)>,
    },
}

impl H2Channel {
    /// Create a new channel.
    ///
    /// `is_server`: `true` allocates even push IDs, `false` allocates odd client IDs.
    #[must_use]
    pub fn new(is_server: bool) -> Self {
        let mut hpack_enc = hpack::Encoder::new();
        hpack_enc.table_mut().set_max_size(4096);
        Self {
            read_buf: BytesMut::with_capacity(16384),
            write_buf: BytesMut::with_capacity(16384),
            local_settings: SettingsStore::default(),
            remote_settings: SettingsStore::default(),
            hpack_dec: hpack::Decoder::new(),
            hpack_enc,
            _conn_flow: FlowControl::new(),
            remote_conn_window: 65535,
            uncredited_conn: 0,
            uncredited_streams: BTreeMap::new(),
            streams: BTreeMap::new(),
            next_outgoing_id: if is_server { 2 } else { 1 },
            _last_peer_stream_id: 0,
            preface_sent: false,
            preface_received: false,
            settings_sent: false,
            waiting_for_settings_ack: false,
            goaway_sent: false,
            goaway_received: false,
        }
    }

    // ── Byte I/O ───────────────────────────────────────────────────────

    /// Push received bytes into the read buffer.
    pub fn feed_input(&mut self, data: &[u8]) {
        self.read_buf.extend_from_slice(data);
    }

    /// Pull and clear all pending output bytes (frames to send).
    pub fn drain_output(&mut self) -> Bytes {
        self.write_buf.split().freeze()
    }

    /// True if there are pending output bytes.
    #[must_use]
    pub fn has_output(&self) -> bool {
        !self.write_buf.is_empty()
    }

    /// True if the channel is waiting for input (read buffer empty or partial).
    #[must_use]
    pub fn wants_input(&self) -> bool {
        true // the caller should always feed available bytes
    }

    /// Try to read a 9-byte frame header + payload from the buffer.
    pub fn read_frame(&mut self) -> io::Result<(Head, Bytes)> {
        if self.read_buf.remaining() < 9 {
            return Err(would_block());
        }
        let header = &self.read_buf[..9];
        let (head, payload_len) = Head::parse_with_len(&[
            header[0], header[1], header[2], header[3], header[4], header[5], header[6], header[7],
            header[8],
        ]);
        let max_frame = self.remote_settings.get(SettingId::MaxFrameSize);
        if payload_len > max_frame {
            return Err(io::Error::new(
                io::ErrorKind::InvalidData,
                format!("frame payload {payload_len} exceeds SETTINGS_MAX_FRAME_SIZE {max_frame}"),
            ));
        }
        if self.read_buf.remaining() < 9 + payload_len as usize {
            return Err(would_block());
        }
        self.read_buf.advance(9);
        let payload = self.read_buf.split_to(payload_len as usize).freeze();
        Ok((head, payload))
    }

    // ── Stream management ──────────────────────────────────────────────

    fn alloc_stream_id(&mut self) -> Option<u32> {
        let max = self.remote_settings.get(SettingId::MaxConcurrentStreams);
        if self.streams.len() >= max as usize {
            return None;
        }
        let id = self.next_outgoing_id;
        self.next_outgoing_id += 2;
        self.streams.insert(
            id,
            StreamEntry {
                state: StreamState::Idle,
                flow: FlowControl::new(),
            },
        );
        Some(id)
    }

    // ── Handshake ──────────────────────────────────────────────────────

    /// Run one client handshake step. Returns `Ok(())` when complete,
    /// `Err(WouldBlock)` when more data needs to be fed. Caller drains output,
    /// feeds input, and calls again until `Ok(())`.
    ///
    /// # Errors
    ///
    /// Returns an error if the operation fails.
    pub fn client_handshake_step(&mut self) -> io::Result<()> {
        if !self.preface_sent {
            self.write_buf.put_slice(CLIENT_PREFACE);
            self.preface_sent = true;
        }

        if !self.settings_sent {
            let sf = self.local_settings.to_frame();
            sf.encode(&mut self.write_buf);
            self.settings_sent = true;
            self.waiting_for_settings_ack = true;
        }

        if self.waiting_for_settings_ack {
            let (head, payload) = match self.read_frame() {
                Ok(v) => v,
                Err(e) if e.kind() == io::ErrorKind::WouldBlock => return Err(e),
                Err(e) => return Err(e),
            };
            match head.kind {
                Kind::Settings => {
                    if head.flag & settings_flags::ACK != 0 {
                        self.waiting_for_settings_ack = false;
                        self.preface_received = true;
                        return Ok(());
                    }
                    let sf = SettingsFrame::parse(&head, &payload).map_err(proto_err)?;
                    self.remote_settings
                        .apply(&sf.settings)
                        .map_err(proto_err)?;
                    let ack = SettingsFrame::ack();
                    let mut buf = BytesMut::new();
                    ack.encode(&mut buf);
                    self.write_buf.extend_from_slice(&buf);
                }
                _ => {
                    return Err(io::Error::new(
                        io::ErrorKind::InvalidData,
                        "unexpected frame during handshake",
                    ))
                }
            }
        }

        self.preface_received = true;
        Ok(())
    }

    /// Run one server handshake step. Returns `Ok(())` when complete,
    /// `Err(WouldBlock)` when more data needed.
    ///
    /// # Errors
    ///
    /// Returns an error if the operation fails.
    pub fn server_handshake_step(&mut self) -> io::Result<()> {
        // 1. Read client preface
        if !self.preface_received {
            if self.read_buf.remaining() < CLIENT_PREFACE_LEN {
                return Err(would_block());
            }
            let preface = self.read_buf.split_to(CLIENT_PREFACE_LEN);
            if &preface[..] != CLIENT_PREFACE {
                return Err(io::Error::new(
                    io::ErrorKind::InvalidData,
                    "invalid h2 preface",
                ));
            }
            self.preface_received = true;
        }

        // 2. Read client SETTINGS
        if !self.settings_sent {
            let (head, payload) = match self.read_frame() {
                Ok(v) => v,
                Err(e) if e.kind() == io::ErrorKind::WouldBlock => return Err(e),
                Err(e) => return Err(e),
            };
            if head.kind != Kind::Settings || head.flag & settings_flags::ACK != 0 {
                return Err(io::Error::new(
                    io::ErrorKind::InvalidData,
                    "expected SETTINGS",
                ));
            }
            let sf = SettingsFrame::parse(&head, &payload).map_err(proto_err)?;
            self.remote_settings
                .apply(&sf.settings)
                .map_err(proto_err)?;

            // 3. Send own SETTINGS first, then ACK the client's SETTINGS.
            //
            // RFC 7540 §3.5: the server connection preface MUST be a SETTINGS
            // frame that is *the first frame the server sends*. Emitting the ACK
            // before our own SETTINGS violates that ordering; a strict client
            // (e.g. grpc-go, which BuildKit's session uses) treats it as a
            // connection error and tears the connection down. Our own H2 client
            // tolerated the wrong order, which hid the bug until now.
            self.local_settings.to_frame_server().encode(&mut self.write_buf);
            self.settings_sent = true;
            self.waiting_for_settings_ack = true;

            let ack = SettingsFrame::ack();
            let mut buf = BytesMut::new();
            ack.encode(&mut buf);
            self.write_buf.extend_from_slice(&buf);
        }

        // 4. Wait for client SETTINGS ACK
        if self.waiting_for_settings_ack {
            let (head, _payload) = match self.read_frame() {
                Ok(v) => v,
                Err(e) if e.kind() == io::ErrorKind::WouldBlock => return Err(e),
                Err(e) => return Err(e),
            };
            if head.kind != Kind::Settings || head.flag & settings_flags::ACK == 0 {
                return Err(io::Error::new(
                    io::ErrorKind::InvalidData,
                    "expected SETTINGS ACK",
                ));
            }
            self.waiting_for_settings_ack = false;
        }

        Ok(())
    }

    // ── Request sender ─────────────────────────────────────────────────

    /// Send a request. Appends frames to `write_buf`. Caller drains output.
    /// Returns the stream ID.
    ///
    /// # Errors
    ///
    /// Returns an error if the operation fails.
    pub fn send_request(&mut self, request: &H2Request) -> io::Result<u32> {
        let stream_id = self
            .alloc_stream_id()
            .ok_or_else(|| io::Error::new(io::ErrorKind::WouldBlock, "no streams"))?;

        let mut header_block = BytesMut::new();
        self.hpack_enc
            .encode_header(b":method", &request.method, &mut header_block);
        self.hpack_enc
            .encode_header(b":scheme", &request.scheme, &mut header_block);
        self.hpack_enc
            .encode_header(b":authority", &request.authority, &mut header_block);
        self.hpack_enc
            .encode_header(b":path", &request.path, &mut header_block);
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

        if let Some(body) = &request.body {
            let df = DataFrame {
                stream_id,
                flags: if request.end_stream {
                    data_flags::END_STREAM
                } else {
                    0
                },
                data: body.clone(),
                pad_len: None,
            };
            let mut buf = BytesMut::new();
            df.encode(&mut buf);
            self.write_buf.put_slice(&buf);
        }

        if let Some(entry) = self.streams.get_mut(&stream_id) {
            entry
                .state
                .send_headers(request.body.is_none() && request.end_stream)
                .ok();
        }
        Ok(stream_id)
    }

    /// Send a response on an existing stream.
    ///
    /// # Errors
    ///
    /// Returns an error if the operation fails.
    pub fn send_response(&mut self, stream_id: u32, response: &H2Response) -> io::Result<()> {
        let mut header_block = BytesMut::new();
        let status_bytes = response.status.to_string();
        self.hpack_enc.encode_header_no_index(
            b":status",
            status_bytes.as_bytes(),
            &mut header_block,
        );
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

        if let Some(body) = &response.body {
            let df = DataFrame {
                stream_id,
                flags: if response.end_stream {
                    data_flags::END_STREAM
                } else {
                    0
                },
                data: body.clone(),
                pad_len: None,
            };
            let mut buf = BytesMut::new();
            df.encode(&mut buf);
            self.write_buf.put_slice(&buf);
        }
        if let Some(entry) = self.streams.get_mut(&stream_id) {
            entry
                .state
                .send_headers(response.body.is_none() && response.end_stream)
                .ok();
        }
        Ok(())
    }

    /// Send headers-only response for streaming.
    ///
    /// # Errors
    ///
    /// Returns an error if the operation fails.
    pub fn send_headers_response(
        &mut self,
        stream_id: u32,
        status: u16,
        headers: &[(Bytes, Bytes)],
        end_stream: bool,
    ) -> io::Result<()> {
        let mut header_block = BytesMut::new();
        let status_bytes = status.to_string();
        self.hpack_enc.encode_header_no_index(
            b":status",
            status_bytes.as_bytes(),
            &mut header_block,
        );
        for (name, value) in headers {
            self.hpack_enc.encode_header(name, value, &mut header_block);
        }
        let mut flags = headers_flags::END_HEADERS;
        if end_stream {
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
        if let Some(entry) = self.streams.get_mut(&stream_id) {
            entry.state.send_headers(end_stream).ok();
        }
        Ok(())
    }

    /// Send a DATA frame on an already-open stream.
    ///
    /// # Errors
    ///
    /// Returns an error if the operation fails.
    pub fn send_data_frame(
        &mut self,
        stream_id: u32,
        data: &[u8],
        end_stream: bool,
    ) -> io::Result<()> {
        let df = DataFrame {
            stream_id,
            flags: if end_stream {
                data_flags::END_STREAM
            } else {
                0
            },
            data: Bytes::copy_from_slice(data),
            pad_len: None,
        };
        let mut buf = BytesMut::new();
        df.encode(&mut buf);
        self.write_buf.put_slice(&buf);
        if let Some(entry) = self.streams.get_mut(&stream_id) {
            entry.state.send_data(end_stream).ok();
        }
        Ok(())
    }

    /// Send GOAWAY.
    pub fn send_goaway(&mut self, last_stream_id: u32, error_code: ErrorCode) {
        let gf = GoAwayFrame {
            last_stream_id,
            error_code,
            debug_data: Bytes::new(),
        };
        // `GoAwayFrame::encode` emits the complete frame — do not re-wrap it.
        gf.encode(&mut self.write_buf);
        self.goaway_sent = true;
    }

    // ── Response reader ─────────────────────────────────────────────────

    /// Try to read the next frame. Returns:
    /// - `Ok(Some((stream_id, response)))` for a HEADERS frame
    /// - `Ok(None)` for GOAWAY
    /// - `Err(WouldBlock)` if more data needed
    ///
    /// # Errors
    ///
    /// Returns an error if the operation fails.
    pub fn recv_response(&mut self) -> io::Result<Option<(u32, H2Request)>> {
        loop {
            let (head, payload) = self.read_frame()?;
            tracing::trace!(
                stream = head.stream_id,
                kind = head.kind as u8,
                flags = head.flag,
                len = payload.len(),
                "h2 channel: recv_response frame"
            );
            match head.kind {
                Kind::Headers => {
                    let hf = HeadersFrame::parse(&head, &payload).map_err(proto_err)?;
                    let decoded = self.hpack_dec.decode(&hf.header_block).map_err(proto_err)?;
                    let resp =
                        self.build_response(&decoded, hf.flags & headers_flags::END_STREAM != 0);
                    return Ok(Some((head.stream_id, resp)));
                }
                Kind::Settings => self.handle_settings(head, &payload)?,
                Kind::WindowUpdate => self.handle_window_update(head, &payload)?,
                Kind::Ping => self.handle_ping(head, &payload)?,
                Kind::GoAway => {
                    self.goaway_received = true;
                    return Ok(None);
                }
                _ => {}
            }
        }
    }

    /// Try to read a DATA frame. Returns:
    /// - `Ok(Some((stream_id, data, end_stream)))`
    /// - `Ok(None)` for GOAWAY
    /// - `Err(WouldBlock)` if more data needed
    ///
    /// # Errors
    ///
    /// Returns an error if the operation fails.
    pub fn recv_data_frame(&mut self) -> io::Result<Option<(u32, Bytes, bool)>> {
        loop {
            let (head, payload) = self.read_frame()?;
            match head.kind {
                Kind::Data => {
                    let df = DataFrame::parse(&head, &payload).map_err(proto_err)?;
                    let end = df.flags & data_flags::END_STREAM != 0;
                    self.credit_received_data(head.stream_id, df.data.len() as u32);
                    return Ok(Some((head.stream_id, df.data, end)));
                }
                Kind::Settings => self.handle_settings(head, &payload)?,
                Kind::WindowUpdate => self.handle_window_update(head, &payload)?,
                Kind::Ping => self.handle_ping(head, &payload)?,
                Kind::GoAway => {
                    self.goaway_received = true;
                    return Ok(None);
                }
                _ => {}
            }
        }
    }

    /// Try to read the next **post-headers** event on any stream.
    ///
    /// Returns `Ok(Some((stream_id, event)))` for DATA or trailing HEADERS,
    /// `Ok(None)` for GOAWAY, and `Err(WouldBlock)` when the buffer is drained.
    ///
    /// # Errors
    ///
    /// Returns an error if the operation fails.
    pub fn recv_stream_event(&mut self) -> io::Result<Option<(u32, H2StreamEvent)>> {
        loop {
            let (head, payload) = self.read_frame()?;
            tracing::trace!(
                stream = head.stream_id,
                kind = head.kind as u8,
                flags = head.flag,
                len = payload.len(),
                "h2 channel: recv_stream_event frame"
            );
            match head.kind {
                Kind::Data => {
                    let df = DataFrame::parse(&head, &payload).map_err(proto_err)?;
                    let end = df.flags & data_flags::END_STREAM != 0;
                    self.credit_received_data(head.stream_id, df.data.len() as u32);
                    return Ok(Some((
                        head.stream_id,
                        H2StreamEvent::Data {
                            stream_id: head.stream_id,
                            data: df.data,
                            end_stream: end,
                        },
                    )));
                }
                Kind::Headers => {
                    let hf = HeadersFrame::parse(&head, &payload).map_err(proto_err)?;
                    let decoded = self.hpack_dec.decode(&hf.header_block).map_err(proto_err)?;
                    return Ok(Some((
                        head.stream_id,
                        H2StreamEvent::Trailers {
                            stream_id: head.stream_id,
                            headers: decoded,
                        },
                    )));
                }
                Kind::Settings => self.handle_settings(head, &payload)?,
                Kind::WindowUpdate => self.handle_window_update(head, &payload)?,
                Kind::Ping => self.handle_ping(head, &payload)?,
                Kind::GoAway => {
                    self.goaway_received = true;
                    return Ok(None);
                }
                _ => {}
            }
        }
    }

    // ── Internal frame handlers ─────────────────────────────────────────

    /// Credit consumed DATA back to the peer's flow-control windows
    /// (RFC 9113 §5.2). Without this, the peer stalls for good once its
    /// 64 KiB initial windows are spent — flow-control windows are cumulative
    /// over the connection's lifetime. Credits are batched and flushed at
    /// [`CREDIT_FLUSH_THRESHOLD`]: per-frame WINDOW_UPDATE pairs fragment the
    /// sender's view of the window into ever-smaller DATA frames and drown
    /// the connection in 13-byte control frames.
    fn credit_received_data(&mut self, stream_id: u32, n: u32) {
        if n == 0 {
            return;
        }
        let stream_credit = self.uncredited_streams.entry(stream_id).or_insert(0);
        *stream_credit += n;
        if *stream_credit >= CREDIT_FLUSH_THRESHOLD {
            let credit = *stream_credit;
            self.uncredited_streams.remove(&stream_id);
            WindowUpdateFrame { stream_id, size_increment: credit }.encode(&mut self.write_buf);
        }
        self.uncredited_conn += n;
        if self.uncredited_conn >= CREDIT_FLUSH_THRESHOLD {
            let credit = std::mem::take(&mut self.uncredited_conn);
            WindowUpdateFrame { stream_id: 0, size_increment: credit }
                .encode(&mut self.write_buf);
        }
    }

    pub fn handle_settings(&mut self, head: Head, payload: &[u8]) -> io::Result<()> {
        if head.stream_id != 0 {
            return Err(io::Error::new(
                io::ErrorKind::InvalidData,
                "SETTINGS on non-zero stream",
            ));
        }
        if head.flag & settings_flags::ACK != 0 {
            self.waiting_for_settings_ack = false;
            return Ok(());
        }
        let sf = SettingsFrame::parse(&head, payload).map_err(proto_err)?;
        self.remote_settings
            .apply(&sf.settings)
            .map_err(proto_err)?;
        // `SettingsFrame::encode` emits the complete frame (header + payload);
        // wrapping it in `queue_frame` again would prepend a second header and
        // produce a SETTINGS ACK with a non-zero length — a connection error
        // (RFC 7540 §6.5, FRAME_SIZE_ERROR) that strict peers kill the
        // connection over.
        SettingsFrame::ack().encode(&mut self.write_buf);
        Ok(())
    }

    pub fn handle_window_update(&mut self, head: Head, payload: &[u8]) -> io::Result<()> {
        let wu = WindowUpdateFrame::parse(&head, payload).map_err(proto_err)?;
        if head.stream_id == 0 {
            self.remote_conn_window = self
                .remote_conn_window
                .saturating_add(wu.size_increment as i32);
        } else if let Some(e) = self.streams.get_mut(&head.stream_id) {
            e.flow.inc_window(wu.size_increment).ok();
        }
        Ok(())
    }

    fn handle_ping(&mut self, head: Head, payload: &[u8]) -> io::Result<()> {
        if head.stream_id != 0 {
            return Err(io::Error::new(
                io::ErrorKind::InvalidData,
                "PING on non-zero stream",
            ));
        }
        if head.flag & ping_flags::ACK != 0 {
            return Ok(());
        }
        let pf = PingFrame::parse(&head, payload).map_err(proto_err)?;
        // `PingFrame::encode` emits the complete frame (header + 8-byte opaque
        // data). Double-wrapping it via `queue_frame` produced a PING with
        // length=17 — a connection error (RFC 7540 §6.7, FRAME_SIZE_ERROR).
        // grpc-go tears the whole connection down on it, which killed BuildKit
        // sessions ~10ms after their first DATA frame (the ACK to grpc-go's
        // BDP-estimator PING was malformed).
        PingFrame::ack(pf.opaque_data).encode(&mut self.write_buf);
        Ok(())
    }

    // ── Connection-level frame dispatch (for multiplex pumps) ──────────

    /// Handle one connection-level frame (SETTINGS, PING, WINDOW_UPDATE, GOAWAY).
    /// Returns `true` if GOAWAY was received (caller should drain and close).
    /// Stream-level frames (HEADERS, DATA, RST_STREAM, etc.) are NOT handled —
    /// the caller must route those to the correct stream.
    ///
    /// # Errors
    /// Returns a non-WouldBlock I/O error if the frame is malformed.
    pub fn handle_conn_frame(&mut self, head: &Head, payload: &[u8]) -> io::Result<bool> {
        match head.kind {
            Kind::Settings => self.handle_settings(*head, payload)?,
            Kind::WindowUpdate => self.handle_window_update(*head, payload)?,
            Kind::Ping => self.handle_ping(*head, payload)?,
            Kind::GoAway => { self.goaway_received = true; return Ok(true); }
            _ => {} // stream-level frames — caller handles
        }
        Ok(self.goaway_received)
    }

    /// Whether the peer has sent GOAWAY.
    #[must_use]
    pub fn goaway_received(&self) -> bool { self.goaway_received }

    /// Send `RST_STREAM` for the given stream.
    pub fn send_rst_stream(&mut self, stream_id: u32) {
        let rf = ResetFrame { stream_id, error_code: crate::http2::frame::ErrorCode::Cancel };
        // `ResetFrame::encode` emits the complete frame — re-wrapping it via
        // `queue_frame` produced RST_STREAM with length=13 instead of 4, a
        // connection error (RFC 7540 §6.4, FRAME_SIZE_ERROR).
        rf.encode(&mut self.write_buf);
    }

    /// Decode an HPACK header block from a HEADERS frame, returning
    /// `(name, value)` pairs. Advances the connection's dynamic HPACK table.
    pub fn decode_headers_for_response(
        &mut self,
        header_block: &[u8],
    ) -> Result<Vec<(Bytes, Bytes)>, &'static str> {
        self.hpack_dec.decode(header_block)
    }

    // ── Helpers ────────────────────────────────────────────────────────

    fn build_response(&self, headers: &[(Bytes, Bytes)], end_stream: bool) -> H2Request {
        let (_st, mut met, mut sch, mut auth, mut path) =
            (0u16, Bytes::new(), Bytes::new(), Bytes::new(), Bytes::new());
        let mut regular = Vec::new();
        for (name, value) in headers {
            match name.as_ref() {
                b":status" => {
                    if let Ok(c) = String::from_utf8_lossy(value).trim().parse::<u16>() {
                        let _ = c;
                    }
                    regular.push((name.clone(), value.clone()));
                }
                b":method" => met = value.clone(),
                b":scheme" => sch = value.clone(),
                b":authority" => auth = value.clone(),
                b":path" => path = value.clone(),
                _ => regular.push((name.clone(), value.clone())),
            }
        }
        H2Request {
            method: met,
            scheme: sch,
            authority: auth,
            path,
            headers: regular,
            body: None,
            end_stream,
        }
    }
}
