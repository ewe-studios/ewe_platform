//! Shared envelope framing (Decision 05 §Envelope Framing).
//!
//! WHY: All three protocols (Connect, gRPC, gRPC-Web) frame streaming messages
//! with the same 5-byte envelope — only the flag *interpretation* differs. This
//! is the shared layer beneath the protocol handlers: decode is zero-copy over
//! the arriving `Bytes`, and reads resume across partial socket reads via the
//! shared [`IncrementalDecoder`] seam (Decision 12 §11).
//!
//! WHAT: [`Envelope`], [`EnvelopeReader`] (frame-level only — never decodes
//! messages), [`EnvelopeWriter`], [`EnvelopeError`], and the gRPC timeout codec
//! ([`encode_grpc_timeout`] / [`decode_grpc_timeout`]).
//!
//! HOW: the wire frame is `flags(1) | len(4 BE) | payload(len)`. The reader is an
//! `IncrementalDecoder<Frame = Envelope>` over an [`AccumulatingBuffer`], slicing
//! payloads out zero-copy; the writer produces owned `Vec<u8>` frames (encoding
//! makes new bytes anyway) with per-envelope compression.

mod timeout;

pub use timeout::{
    decode_grpc_timeout, decode_grpc_timeout_connect, encode_grpc_timeout, TimeoutError,
};

use std::collections::HashMap;
use std::error::Error as StdError;
use std::io::Read;
use std::sync::Arc;

use bytes::Bytes;
use foundation_core::io::{
    read_frame_blocking, AccumulatingBuffer, DecodeError, DecodeStep, IncrementalDecoder,
};
use foundation_errstacks::ErrorTrace;
use foundation_netio::simple_http::shared::SimpleHeaders;

use crate::compression::Compressor;
use crate::error::{Code, ConnectError, ConnectResult, EndStreamResponse};

/// Fixed 5-byte envelope header size.
pub const ENVELOPE_HEADER_LEN: usize = 5;

/// One framed message: flags + payload (zero-copy slice of the arriving buffer).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Envelope {
    /// Envelope flags byte.
    pub flags: u8,
    /// The (decompressed) frame payload — a refcount-shared slice, not a copy.
    pub data: Bytes,
}

impl Envelope {
    /// Bit 0 — payload is compressed with the negotiated algorithm.
    pub const FLAG_COMPRESSED: u8 = 0x01;
    /// Bit 1 — Connect: this is the `EndStreamResponse` (final) frame.
    pub const FLAG_END_STREAM: u8 = 0x02;
    /// Bit 7 — gRPC-Web: this frame carries trailers, not a message.
    pub const FLAG_TRAILER: u8 = 0x80;

    /// Whether the payload is compressed.
    #[must_use]
    pub fn is_compressed(&self) -> bool {
        self.flags & Self::FLAG_COMPRESSED != 0
    }
    /// Whether this is the Connect end-stream frame.
    #[must_use]
    pub fn is_end_stream(&self) -> bool {
        self.flags & Self::FLAG_END_STREAM != 0
    }
    /// Whether this is a gRPC-Web trailer frame.
    #[must_use]
    pub fn is_trailer(&self) -> bool {
        self.flags & Self::FLAG_TRAILER != 0
    }

    /// Encode `flags | len | data` into a pooled/owned buffer (write path, RS8).
    pub fn encode_into(&self, buf: &mut Vec<u8>) {
        buf.reserve(ENVELOPE_HEADER_LEN + self.data.len());
        buf.push(self.flags);
        buf.extend_from_slice(&(self.data.len() as u32).to_be_bytes());
        buf.extend_from_slice(&self.data);
    }

    /// Decode one envelope from the head of `data`, slicing the payload
    /// **zero-copy** (`Bytes::slice` — a refcount bump, no memcpy). Returns the
    /// envelope and the number of bytes consumed.
    ///
    /// # Errors
    /// [`EnvelopeError::Incomplete`] when `data` holds less than a full frame.
    pub fn decode(data: &Bytes) -> Result<(Envelope, usize), EnvelopeError> {
        if data.len() < ENVELOPE_HEADER_LEN {
            return Err(EnvelopeError::Incomplete);
        }
        let flags = data[0];
        let len = u32::from_be_bytes([data[1], data[2], data[3], data[4]]) as usize;
        let total = ENVELOPE_HEADER_LEN + len;
        if data.len() < total {
            return Err(EnvelopeError::Incomplete);
        }
        let payload = data.slice(ENVELOPE_HEADER_LEN..total);
        Ok((Envelope { flags, data: payload }, total))
    }
}

/// An envelope-framing error (Decision 05). Maps into [`ConnectError`] at the RPC
/// boundary (Decision 03).
#[derive(Debug)]
pub enum EnvelopeError {
    /// Not enough bytes for a full frame yet (standalone decode only — the reader
    /// surfaces this as [`DecodeStep::Pending`], never an error).
    Incomplete,
    /// The declared frame length exceeds the configured `read_max_bytes`.
    TooLarge {
        /// The declared frame length.
        length: usize,
        /// The configured limit.
        limit: usize,
    },
    /// A compressed frame arrived but no decompressor was negotiated.
    CompressedWithoutDecompressor,
}

impl EnvelopeError {
    fn code(&self) -> Code {
        match self {
            EnvelopeError::TooLarge { .. } => Code::ResourceExhausted,
            _ => Code::InvalidArgument,
        }
    }
}

impl core::fmt::Display for EnvelopeError {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        match self {
            EnvelopeError::Incomplete => write!(f, "incomplete envelope frame"),
            EnvelopeError::TooLarge { length, limit } => {
                write!(f, "envelope length {length} exceeds read_max_bytes {limit}")
            }
            EnvelopeError::CompressedWithoutDecompressor => {
                write!(f, "compressed frame received but no decompressor was negotiated")
            }
        }
    }
}

impl StdError for EnvelopeError {}

impl From<EnvelopeError> for ConnectError {
    fn from(err: EnvelopeError) -> Self {
        ConnectError::new(err.code(), err.to_string())
    }
}

impl From<EnvelopeError> for ErrorTrace<ConnectError> {
    fn from(err: EnvelopeError) -> Self {
        let connect = ConnectError::from(&err);
        ErrorTrace::new(err).change_context(connect)
    }
}

impl From<&EnvelopeError> for ConnectError {
    fn from(err: &EnvelopeError) -> Self {
        ConnectError::new(err.code(), err.to_string())
    }
}

/// The `IncrementalDecoder` state: buffer + decompression policy. Kept separate
/// from the source so [`EnvelopeReader`] can borrow both disjointly.
struct EnvelopeFrameDecoder {
    buffer: AccumulatingBuffer,
    decompressor: Option<Arc<dyn Compressor>>,
    read_max_bytes: usize,
}

impl IncrementalDecoder for EnvelopeFrameDecoder {
    type Frame = Envelope;

    fn step(&mut self, src: &mut impl Read) -> Result<DecodeStep<Envelope>, DecodeError> {
        self.buffer.fill_from(src)?;
        let view = self.buffer.view();
        if view.len() < ENVELOPE_HEADER_LEN {
            return Ok(DecodeStep::Pending);
        }
        let flags = view[0];
        let len = u32::from_be_bytes([view[1], view[2], view[3], view[4]]) as usize;
        if self.read_max_bytes != 0 && len > self.read_max_bytes {
            return Err(DecodeError::protocol(format!(
                "envelope length {len} exceeds read_max_bytes {}",
                self.read_max_bytes
            )));
        }
        if view.len() < ENVELOPE_HEADER_LEN + len {
            return Ok(DecodeStep::Pending);
        }

        self.buffer.advance(ENVELOPE_HEADER_LEN);
        let payload = self.buffer.split_to(len);

        let data = if flags & Envelope::FLAG_COMPRESSED != 0 {
            match &self.decompressor {
                Some(c) => Bytes::from(
                    c.decompress(&payload, self.read_max_bytes)
                        .map_err(|e| DecodeError::protocol(e.to_string()))?,
                ),
                None => {
                    return Err(DecodeError::protocol(
                        "compressed frame received but no decompressor was negotiated",
                    ));
                }
            }
        } else {
            payload
        };

        Ok(DecodeStep::Frame(Envelope { flags, data }))
    }

    fn has_partial(&self) -> bool {
        !self.buffer.is_empty()
    }
}

/// Reads envelopes from a body byte stream, decompressing per-frame. **Frame-level
/// only** — yields the framed `Envelope` (flags + codec-encoded payload), never
/// decodes messages (that is the `MessageSource` facade's job, Decision 11). The
/// source is stored at construction (Decision 05 C8).
pub struct EnvelopeReader<S> {
    source: S,
    decoder: EnvelopeFrameDecoder,
}

impl<S: Read> EnvelopeReader<S> {
    /// Construct over a body source, with an optional decompressor and a
    /// `read_max_bytes` cap (`0` = unlimited).
    pub fn new(source: S, decompressor: Option<Arc<dyn Compressor>>, read_max_bytes: usize) -> Self {
        Self {
            source,
            decoder: EnvelopeFrameDecoder {
                buffer: AccumulatingBuffer::new(),
                decompressor,
                read_max_bytes,
            },
        }
    }

    /// Blocking read of the next envelope, or `None` at a clean end of stream.
    /// The 5-byte prefix + body may span multiple reads; partial state is
    /// retained and never surfaces as an error.
    ///
    /// # Errors
    /// A [`ConnectError`]-carrying trace on a malformed frame or I/O failure.
    pub fn read(&mut self) -> ConnectResult<Option<Envelope>> {
        read_frame_blocking(&mut self.decoder, &mut self.source).map_err(map_decode_error)
    }

    /// Non-blocking step for the transport seam (Decision 11): make progress with
    /// whatever bytes are available, returning [`DecodeStep::Pending`] on a short
    /// read.
    ///
    /// # Errors
    /// A [`ConnectError`]-carrying trace on a malformed frame or I/O failure.
    pub fn step(&mut self) -> ConnectResult<DecodeStep<Envelope>> {
        self.decoder.step(&mut self.source).map_err(map_decode_error)
    }

    /// Whether the reader holds a partial (mid-frame) buffer.
    #[must_use]
    pub fn has_partial(&self) -> bool {
        self.decoder.has_partial()
    }
}

fn map_decode_error(err: DecodeError) -> ErrorTrace<ConnectError> {
    let connect = match &err {
        DecodeError::Io(_) => ConnectError::unavailable(err.to_string()),
        DecodeError::Protocol(_) => ConnectError::invalid_argument(err.to_string()),
    };
    ErrorTrace::new(err).change_context(connect)
}

/// Writes envelopes to a byte sink, compressing per-frame. Frame-level only:
/// takes an already-encoded frame (the `MessageSink` facade did the typed encode).
pub struct EnvelopeWriter {
    compressor: Option<Arc<dyn Compressor>>,
    compress_min_bytes: usize,
    send_max_bytes: usize,
}

impl EnvelopeWriter {
    /// Construct with an optional compressor and the send-side size policy.
    #[must_use]
    pub fn new(
        compressor: Option<Arc<dyn Compressor>>,
        compress_min_bytes: usize,
        send_max_bytes: usize,
    ) -> Self {
        Self {
            compressor,
            compress_min_bytes,
            send_max_bytes,
        }
    }

    /// Envelope one already-encoded frame (compress iff negotiated and
    /// ≥ `compress_min_bytes`; `send_max_bytes` checked **after** compression,
    /// Decision 06 H10).
    ///
    /// # Errors
    /// A [`ConnectError`]-carrying trace on compression failure or a payload over
    /// `send_max_bytes`.
    pub fn write(&self, frame: Bytes) -> ConnectResult<Vec<u8>> {
        self.frame_with_flags(&frame, 0)
    }

    /// Write a Connect `EndStreamResponse` frame (final message).
    ///
    /// # Errors
    /// A [`ConnectError`]-carrying trace on serialization/compression failure.
    pub fn write_end_stream(
        &self,
        error: Option<&ErrorTrace<ConnectError>>,
        trailers: &SimpleHeaders,
    ) -> ConnectResult<Vec<u8>> {
        let end = EndStreamResponse {
            error: error.map(|t| t.current_context().to_wire()),
            metadata: headers_to_map(trailers),
        };
        let payload = serde_json::to_vec(&end).map_err(|e| {
            ConnectError::internal(format!("failed to encode EndStreamResponse: {e}"))
        })?;
        self.frame_with_flags(&payload, Envelope::FLAG_END_STREAM)
    }

    /// Write a gRPC-Web trailer frame (trailers rendered as an HTTP-1.1-style
    /// header block).
    ///
    /// # Errors
    /// A [`ConnectError`]-carrying trace on compression failure.
    pub fn write_trailer_frame(&self, trailers: &SimpleHeaders) -> ConnectResult<Vec<u8>> {
        let mut payload = Vec::new();
        for (name, values) in trailers {
            for value in values {
                payload.extend_from_slice(name.to_string().to_ascii_lowercase().as_bytes());
                payload.extend_from_slice(b": ");
                payload.extend_from_slice(value.as_bytes());
                payload.extend_from_slice(b"\r\n");
            }
        }
        self.frame_with_flags(&payload, Envelope::FLAG_TRAILER)
    }

    fn frame_with_flags(&self, payload: &[u8], extra_flags: u8) -> ConnectResult<Vec<u8>> {
        let mut flags = extra_flags;
        let body = match &self.compressor {
            Some(compressor) if payload.len() >= self.compress_min_bytes => {
                flags |= Envelope::FLAG_COMPRESSED;
                compressor.compress(payload)?
            }
            _ => payload.to_vec(),
        };

        // H10: check the send cap AFTER compression.
        if self.send_max_bytes != 0 && body.len() > self.send_max_bytes {
            return Err(ConnectError::resource_exhausted(format!(
                "message of {} bytes exceeds send_max_bytes of {}",
                body.len(),
                self.send_max_bytes
            ))
            .into());
        }

        let mut out = Vec::with_capacity(ENVELOPE_HEADER_LEN + body.len());
        out.push(flags);
        out.extend_from_slice(&(body.len() as u32).to_be_bytes());
        out.extend_from_slice(&body);
        Ok(out)
    }
}

/// Render trailing metadata as the `EndStreamResponse.metadata` map, or `None`
/// when empty (so the field is omitted on the wire).
fn headers_to_map(headers: &SimpleHeaders) -> Option<HashMap<String, Vec<String>>> {
    if headers.is_empty() {
        return None;
    }
    Some(
        headers
            .iter()
            .map(|(name, values)| (name.to_string().to_ascii_lowercase(), values.clone()))
            .collect(),
    )
}
