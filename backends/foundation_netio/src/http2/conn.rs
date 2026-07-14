//! `H2Conn` — concrete HTTP/2 connection over the platform's standard
//! non-blocking byte stream (F47).
//!
//! WHY: `H2Connection<S: Read+Write>` is generic. `H2Conn` pins it to
//! [`SharedByteBufferStream<RawStream>`] — matching how that type is the
//! concrete HTTP/1.1 connection, and providing a simple, non-generic API.
//!
//! WHAT: [`H2Conn`] wraps `H2Connection`, delegating handshake, frame I/O,
//! HPACK decoding, and per-stream `H2Frame` encoding through a clean surface.
//!
//! HOW: A frame is only read once it is entirely buffered. `H2Conn` first
//! `peek`s (which consumes nothing) to confirm the 9-byte header plus its
//! declared payload are in memory, and only then delegates — so the inner reads
//! are served from the buffer and cannot stall mid-frame. The handshake is
//! likewise driven one buffered frame at a time, so a `WouldBlock` between its
//! steps parks the caller rather than restarting it from the preface.
//! Frame encoding appends to the shared `write_buf`; the caller flushes.

use std::io;

use bytes::{Bytes, BytesMut};

use crate::netcap::RawStream;
use foundation_core::io::ioutils::{PeekError, PeekableReadStream, SharedByteBufferStream};

use crate::http2::connection::H2Connection;
use crate::http2::frame::{
    data_flags, headers_flags, ping_flags, DataFrame, Head, HeadersFrame, Kind, PingFrame,
    ResetFrame, HEADER_LEN,
};
use crate::http2::hpack;
use crate::http2::types::H2Frame;

/// Progress through the server handshake. Each step needs bytes the peer may not
/// have sent yet, so the handshake must be resumable across polls.
///
/// There is deliberately no "await the client's SETTINGS ACK" step. Per
/// RFC 9113 §3.4 the connection is usable as soon as the server has sent its own
/// SETTINGS; a client may send HEADERS before it ACKs. Blocking the handshake on
/// that ACK deadlocks against such a client. The ACK arrives later and is
/// absorbed by the ordinary frame loop.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum ServerHandshake {
    /// Awaiting the 24-byte client preface.
    Preface,
    /// Awaiting the client's initial SETTINGS frame.
    Settings,
    Done,
}

/// Concrete HTTP/2 connection — the h2 equivalent of
/// [`SharedByteBufferStream<RawStream>`] for HTTP/1.1.
pub struct H2Conn {
    inner: H2Connection<SharedByteBufferStream<RawStream>>,
    /// A second handle on the *same* shared buffer, used to `peek` ahead without
    /// consuming. Cloning shares the buffer; it does not duplicate it.
    stream: SharedByteBufferStream<RawStream>,
    /// HPACK decoding is stateful for the life of the connection: the peer may
    /// reference dynamic-table entries established by an earlier HEADERS frame.
    /// A per-call decoder would fail to resolve those indices.
    hpack_dec: hpack::Decoder,
    handshake: ServerHandshake,
}

/// Nothing is readable yet — the caller should park and retry.
fn would_block() -> io::Error {
    io::Error::new(io::ErrorKind::WouldBlock, "h2: frame not yet buffered")
}

impl H2Conn {
    /// Create a new server-side connection (allocates even push stream IDs).
    #[must_use]
    pub fn new_server(stream: SharedByteBufferStream<RawStream>) -> Self {
        Self {
            stream: stream.clone(),
            inner: H2Connection::new(stream, true),
            hpack_dec: hpack::Decoder::new(),
            handshake: ServerHandshake::Preface,
        }
    }

    /// Create a new client-side connection (allocates odd stream IDs).
    #[must_use]
    #[allow(dead_code)]
    pub fn new_client(stream: SharedByteBufferStream<RawStream>) -> Self {
        Self {
            stream: stream.clone(),
            inner: H2Connection::new(stream, false),
            hpack_dec: hpack::Decoder::new(),
            handshake: ServerHandshake::Done,
        }
    }

    /// Are at least `n` bytes already buffered? Peeking consumes nothing, so a
    /// `false` answer leaves the stream exactly as it was.
    fn buffered(&mut self, n: usize) -> io::Result<bool> {
        let mut probe = vec![0u8; n];
        match self.stream.peek(&mut probe) {
            Ok(got) => Ok(got >= n),
            Err(PeekError::IOError(ref e)) if e.kind() == io::ErrorKind::WouldBlock => Ok(false),
            Err(PeekError::IOError(e)) => Err(e),
            Err(e) => Err(io::Error::new(io::ErrorKind::InvalidData, e.to_string())),
        }
    }

    /// Is a whole frame — 9-byte header plus its declared payload — buffered?
    fn frame_buffered(&mut self) -> io::Result<bool> {
        if !self.buffered(HEADER_LEN)? {
            return Ok(false);
        }
        let mut header = [0u8; HEADER_LEN];
        match self.stream.peek(&mut header) {
            Ok(got) if got >= HEADER_LEN => {}
            Ok(_) => return Ok(false),
            Err(PeekError::IOError(ref e)) if e.kind() == io::ErrorKind::WouldBlock => {
                return Ok(false)
            }
            Err(PeekError::IOError(e)) => return Err(e),
            Err(e) => return Err(io::Error::new(io::ErrorKind::InvalidData, e.to_string())),
        }
        let (_head, payload_len) = Head::parse_with_len(&header);
        self.buffered(HEADER_LEN + payload_len as usize)
    }

    /// Drive the server handshake as far as the buffered bytes allow.
    ///
    /// Returns `WouldBlock` when it needs more from the peer; the caller parks
    /// and calls again. Progress is never lost: each step runs only once its
    /// bytes are fully buffered, so nothing is left half-consumed.
    ///
    /// # Errors
    /// `InvalidData` if the peer's preface or SETTINGS frame is malformed.
    pub fn server_handshake(&mut self) -> io::Result<()> {
        loop {
            match self.handshake {
                ServerHandshake::Preface => {
                    if !self.buffered(crate::http2::connection::CLIENT_PREFACE_LEN)? {
                        return Err(would_block());
                    }
                    self.inner.server_recv_preface()?;
                    self.handshake = ServerHandshake::Settings;
                }
                ServerHandshake::Settings => {
                    if !self.frame_buffered()? {
                        return Err(would_block());
                    }
                    let (head, payload) = self.inner.read_frame()?;
                    self.inner.server_recv_settings(&head, &payload)?;
                    self.handshake = ServerHandshake::Done;
                }
                ServerHandshake::Done => return Ok(()),
            }
        }
    }

    /// Run the client-side h2 handshake (write preface + SETTINGS exchange).
    #[allow(dead_code)]
    ///
    /// # Errors
    ///
    /// Returns an error if the operation fails.
    pub fn client_handshake(&mut self) -> io::Result<()> {
        self.inner.client_handshake()
    }

    /// Read the next frame, but only once it is entirely buffered.
    ///
    /// # Errors
    /// `WouldBlock` while the frame is still arriving — nothing is consumed.
    pub fn read_frame(&mut self) -> io::Result<(Head, Bytes)> {
        if !self.frame_buffered()? {
            return Err(would_block());
        }
        self.inner.read_frame()
    }

    /// Read the next **stream-level** frame, auto-handling connection-level
    /// frames (PING→ACK, SETTINGS→ACK, WINDOW_UPDATE) internally.
    ///
    /// Callers that only route stream frames (HEADERS/DATA/RST_STREAM) should
    /// use this instead of [`read_frame`] — otherwise PING frames from the peer
    /// are silently dropped, the peer's PING ACK wait times out, and the
    /// connection is torn down.
    ///
    /// Returns `None` for GOAWAY (caller should close the connection).
    ///
    /// # Errors
    /// `WouldBlock` while the frame is still arriving.
    pub fn read_stream_frame(&mut self) -> io::Result<Option<(Head, Bytes)>> {
        loop {
            if !self.frame_buffered()? {
                return Err(would_block());
            }
            let (head, payload) = self.inner.read_frame()?;
            match head.kind {
                Kind::Ping => {
                    let ack = PingFrame::ack(
                        PingFrame::parse(&head, &payload)
                            .map_err(|_| io::Error::new(io::ErrorKind::InvalidData, "bad PING"))?
                            .opaque_data,
                    );
                    let mut buf = BytesMut::new();
                    ack.encode(&mut buf);
                    self.inner.write_buf_mut().extend_from_slice(&buf);
                    // continue loop — this isn't a stream frame
                }
                Kind::Settings => {
                    self.inner.handle_settings(head, &payload)?;
                    // continue loop
                }
                Kind::WindowUpdate => {
                    self.inner.handle_window_update(head, &payload)?;
                    // continue loop
                }
                Kind::GoAway => return Ok(None),
                _ => return Ok(Some((head, payload))),
            }
        }
    }

    /// Flush the write buffer to the socket.
    pub fn flush(&mut self) -> io::Result<()> {
        self.inner.flush()
    }

    /// Decode an HPACK header block into (name, value) pairs, advancing the
    /// connection's dynamic table.
    pub fn decode_headers(&mut self, block: &[u8]) -> Result<Vec<(Bytes, Bytes)>, &'static str> {
        self.hpack_dec.decode(block)
    }

    /// Encode an [`H2Frame`] for a stream into the connection's write buffer.
    /// Call [`flush`](Self::flush) to send it over the socket.
    pub fn encode_frame(&mut self, stream_id: u32, frame: &H2Frame) {
        match frame {
            H2Frame::Headers {
                status,
                headers,
                end_stream,
            } => {
                // A fresh encoder never emits dynamic-table indices (its table is
                // empty), so every field goes out as a literal — wasteful but
                // always decodable. Sharing one encoder across frames would be
                // the optimisation, and is tracked separately.
                let mut hpack_enc = hpack::Encoder::new();
                let mut header_block = BytesMut::new();
                let status_bytes = status.to_string();
                hpack_enc.encode_header_no_index(
                    &bytes::Bytes::from_static(b":status"),
                    status_bytes.as_bytes(),
                    &mut header_block,
                );
                for (name, value) in headers {
                    hpack_enc.encode_header(name, value, &mut header_block);
                }

                let mut flags = headers_flags::END_HEADERS;
                if *end_stream {
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
                self.inner.write_buf_mut().extend_from_slice(&buf);
            }
            H2Frame::Data {
                payload,
                end_stream,
            } => {
                let df = DataFrame {
                    stream_id,
                    flags: if *end_stream {
                        data_flags::END_STREAM
                    } else {
                        0
                    },
                    data: payload.clone(),
                    pad_len: None,
                };
                let mut buf = BytesMut::new();
                df.encode(&mut buf);
                self.inner.write_buf_mut().extend_from_slice(&buf);
            }
            H2Frame::Reset { error_code } => {
                let rf = ResetFrame {
                    stream_id,
                    error_code: *error_code,
                };
                let mut buf = BytesMut::new();
                rf.encode(&mut buf);
                self.inner.write_buf_mut().extend_from_slice(&buf);
            }
        }
    }
}
