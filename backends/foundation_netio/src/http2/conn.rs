//! `H2Conn` — concrete HTTP/2 connection over the platform's standard
//! non-blocking byte stream (F47).
//!
//! WHY: `H2Connection<S: Read+Write>` is generic. `H2Conn` pins it to
//! [`SharedByteBufferStream<RawStream>`] — matching how that type is the
//! concrete HTTP/1.1 connection, and providing a simple, non-generic API.
//!
//! WHAT: [`H2Conn`] wraps `H2Connection`, delegating handshake, frame I/O,
//! HPACK decoding, and per-stream H2Frame encoding through a clean surface.
//!
//! HOW: Every method delegates directly to the inner `H2Connection`. Frame
//! encoding appends to the shared `write_buf`; the caller flushes.

use std::io;

use bytes::{Bytes, BytesMut};

use foundation_core::io::ioutils::SharedByteBufferStream;
use crate::netcap::RawStream;

use crate::http2::connection::H2Connection;
use crate::http2::frame::{Head, HeadersFrame, DataFrame, ResetFrame, headers_flags, data_flags};
use crate::http2::hpack;
use crate::http2::types::H2Frame;

/// Concrete HTTP/2 connection — the h2 equivalent of
/// [`SharedByteBufferStream<RawStream>`] for HTTP/1.1.
pub struct H2Conn {
    inner: H2Connection<SharedByteBufferStream<RawStream>>,
    /// HPACK decoding is stateful for the life of the connection: the peer may
    /// reference dynamic-table entries established by an earlier HEADERS frame.
    /// A per-call decoder would fail to resolve those indices.
    hpack_dec: hpack::Decoder,
}

impl H2Conn {
    /// Create a new server-side connection (allocates even push stream IDs).
    #[must_use]
    pub fn new_server(stream: SharedByteBufferStream<RawStream>) -> Self {
        Self { inner: H2Connection::new(stream, true), hpack_dec: hpack::Decoder::new() }
    }

    /// Create a new client-side connection (allocates odd stream IDs).
    #[must_use]
    #[allow(dead_code)]
    pub fn new_client(stream: SharedByteBufferStream<RawStream>) -> Self {
        Self { inner: H2Connection::new(stream, false), hpack_dec: hpack::Decoder::new() }
    }

    /// Run the server-side h2 handshake (read client preface + SETTINGS exchange).
    pub fn server_handshake(&mut self) -> io::Result<()> {
        self.inner.server_handshake()
    }

    /// Run the client-side h2 handshake (write preface + SETTINGS exchange).
    #[allow(dead_code)]
    pub fn client_handshake(&mut self) -> io::Result<()> {
        self.inner.client_handshake()
    }

    /// Read the next frame from the socket.
    pub fn read_frame(&mut self) -> io::Result<(Head, Bytes)> {
        self.inner.read_frame()
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
            H2Frame::Headers { status, headers, end_stream } => {
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
                if *end_stream { flags |= headers_flags::END_STREAM; }

                let hf = HeadersFrame {
                    stream_id, flags,
                    header_block: header_block.freeze(),
                    pad_len: None, priority: None,
                };
                let mut buf = BytesMut::new();
                hf.encode(&mut buf);
                self.inner.write_buf_mut().extend_from_slice(&buf);
            }
            H2Frame::Data { payload, end_stream } => {
                let df = DataFrame {
                    stream_id,
                    flags: if *end_stream { data_flags::END_STREAM } else { 0 },
                    data: payload.clone(),
                    pad_len: None,
                };
                let mut buf = BytesMut::new();
                df.encode(&mut buf);
                self.inner.write_buf_mut().extend_from_slice(&buf);
            }
            H2Frame::Reset { error_code } => {
                let rf = ResetFrame { stream_id, error_code: *error_code };
                let mut buf = BytesMut::new();
                rf.encode(&mut buf);
                self.inner.write_buf_mut().extend_from_slice(&buf);
            }
        }
    }
}
