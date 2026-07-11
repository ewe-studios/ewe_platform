//! HTTP/3 stream framing over the QUIC trait set (F34).
//!
//! WHY: `FrameDecoder` speaks `io::Read`; a [`QuicRecvStream`] speaks
//! `Stream<Result<Option<Bytes>>>`. Something has to sit between them, and it has
//! to preserve the property that makes the decoder work at all: a partial frame is
//! `Pending`, never an error.
//!
//! WHAT: [`FramedRecv`] — a QUIC receive stream that yields whole [`Frame`]s.
//!
//! HOW: bytes arriving from the QUIC stream accumulate in a buffer; each call
//! steps the decoder over that buffer. `Pending` from the QUIC stream and
//! `Pending` from the decoder both mean the same thing to the caller, so they
//! collapse into one `Stream::Pending(())`.
//!
//! ## End of stream is not the same as end of frame
//!
//! When a QUIC stream FINs, any bytes the decoder still holds are a **truncated
//! frame** — a protocol error, not a clean close. `FrameDecoder::has_partial()`
//! exists precisely to tell those apart, and this module is where that
//! distinction becomes visible to HTTP/3.

use bytes::Bytes;

use foundation_core::io::{DecodeStep, IncrementalDecoder};
use foundation_core::valtron::Stream;

use crate::quic::{QuicRecvStream, QuicStreamError};

use super::frame::{Frame, FrameDecoder};

/// Why a stream could not produce its next frame.
#[derive(Debug)]
pub enum StreamError {
    /// The QUIC stream failed or the connection went away.
    Quic(QuicStreamError),
    /// The bytes on the stream are not valid HTTP/3 framing.
    Protocol(String),
    /// The stream ended mid-frame.
    Truncated,
}

impl std::fmt::Display for StreamError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Quic(e) => write!(f, "QUIC stream error: {e}"),
            Self::Protocol(m) => write!(f, "HTTP/3 protocol error: {m}"),
            Self::Truncated => write!(f, "HTTP/3 stream ended mid-frame"),
        }
    }
}

impl std::error::Error for StreamError {}

impl From<QuicStreamError> for StreamError {
    fn from(e: QuicStreamError) -> Self {
        Self::Quic(e)
    }
}

/// A QUIC receive stream, framed as HTTP/3.
///
/// Call [`FramedRecv::poll_frame`] repeatedly: `Next(Ok(Some(frame)))` per frame,
/// `Next(Ok(None))` at a clean end of stream, `Pending` when more bytes are needed.
#[derive(Debug)]
pub struct FramedRecv<R: QuicRecvStream> {
    recv: R,
    decoder: FrameDecoder,
    /// Bytes read from QUIC but not yet consumed by the decoder.
    pending: Vec<u8>,
    /// The peer sent FIN; no more bytes will arrive.
    finished: bool,
}

impl<R: QuicRecvStream> FramedRecv<R> {
    /// Frame `recv` as an HTTP/3 stream.
    pub fn new(recv: R) -> Self {
        Self {
            recv,
            decoder: FrameDecoder::new(),
            pending: Vec::new(),
            finished: false,
        }
    }

    /// Refuse frames larger than `max` bytes.
    #[must_use]
    pub fn with_max_frame_size(mut self, max: usize) -> Self {
        self.decoder = FrameDecoder::new().with_max_frame_size(max);
        self
    }

    /// The underlying receive stream.
    pub fn get_ref(&self) -> &R {
        &self.recv
    }

    /// The underlying stream, mutably. On a bidirectional stream this is also the
    /// write half, which is how a response is sent on the request's stream.
    pub fn get_mut(&mut self) -> &mut R {
        &mut self.recv
    }

    /// Consume and return the underlying receive stream.
    pub fn into_inner(self) -> R {
        self.recv
    }

    /// Try to decode one frame out of whatever is already buffered.
    ///
    /// Steps the decoder even when `pending` is empty: the decoder keeps its *own*
    /// accumulating buffer, and a previous `step` may have left one or more whole
    /// frames sitting in it. Skipping the step when we have no new bytes would
    /// strand those frames, and then a FIN would look like a truncated frame.
    fn decode_buffered(&mut self) -> Result<Option<Frame>, StreamError> {
        let mut cursor = &self.pending[..];
        let before = cursor.len();

        let step = self
            .decoder
            .step(&mut cursor)
            .map_err(|e| StreamError::Protocol(e.to_string()))?;

        // The decoder consumed `before - cursor.len()` bytes from the front.
        let consumed = before - cursor.len();
        self.pending.drain(..consumed);

        match step {
            DecodeStep::Frame(frame) => Ok(Some(frame)),
            DecodeStep::Pending => Ok(None),
        }
    }

    /// WHY: HTTP/3 is a sequence of frames on a QUIC stream, and both layers can
    /// independently say "not yet".
    ///
    /// WHAT: the next frame, end of stream, or `Pending`.
    ///
    /// HOW: decode from the buffer first — a previous call may have left a whole
    /// frame behind — then pull more bytes from QUIC and try again. A FIN with
    /// bytes still held by the decoder is [`StreamError::Truncated`], not a clean
    /// close.
    pub fn poll_frame(&mut self) -> Stream<Result<Option<Frame>, StreamError>, ()> {
        // 1. Anything already buffered?
        match self.decode_buffered() {
            Ok(Some(frame)) => return Stream::Next(Ok(Some(frame))),
            Ok(None) => {}
            Err(e) => return Stream::Next(Err(e)),
        }

        if self.finished {
            // A clean end of stream leaves the decoder holding nothing. Anything
            // else is a frame the peer never finished sending.
            return if self.decoder.has_partial() || !self.pending.is_empty() {
                Stream::Next(Err(StreamError::Truncated))
            } else {
                Stream::Next(Ok(None))
            };
        }

        // 2. Pull more bytes from QUIC.
        match self.recv.read() {
            Stream::Next(Ok(Some(bytes))) => {
                self.pending.extend_from_slice(&bytes);
                match self.decode_buffered() {
                    Ok(Some(frame)) => Stream::Next(Ok(Some(frame))),
                    // More bytes arrived but the frame is still incomplete.
                    Ok(None) => Stream::Pending(()),
                    Err(e) => Stream::Next(Err(e)),
                }
            }
            Stream::Next(Ok(None)) => {
                self.finished = true;
                if self.decoder.has_partial() || !self.pending.is_empty() {
                    Stream::Next(Err(StreamError::Truncated))
                } else {
                    Stream::Next(Ok(None))
                }
            }
            Stream::Next(Err(e)) => Stream::Next(Err(StreamError::Quic(e))),
            // Nothing to read yet.
            _ => Stream::Pending(()),
        }
    }
}

/// Read the stream-type varint that prefixes every unidirectional stream
/// (RFC 9114 §6.2), one byte at a time.
///
/// WHY: a unidirectional stream announces what it is before it says anything
/// else, and the announcement is itself a varint — so it can span reads.
#[derive(Debug)]
pub struct StreamTypeReader<R: QuicRecvStream> {
    /// `None` once the type has been read and the receiver handed to the caller.
    recv: Option<R>,
    buffer: Vec<u8>,
}

impl<R: QuicRecvStream> StreamTypeReader<R> {
    /// Begin reading a unidirectional stream's type.
    pub fn new(recv: R) -> Self {
        Self {
            recv: Some(recv),
            buffer: Vec::with_capacity(8),
        }
    }

    /// WHAT: the stream type, plus the receive stream and any bytes that followed
    /// the varint in the same read.
    ///
    /// HOW: `Pending` until the varint is complete. `Next(Ok(None))` if the peer
    /// closed the stream before announcing its type, or if the type has already
    /// been taken.
    ///
    /// The receiver is moved out on success, which is why it lives in an
    /// `Option`: `mem::replace` with an "unreachable" placeholder would actually
    /// construct — and therefore panic on — that placeholder.
    pub fn poll_type(&mut self) -> Stream<Result<Option<(u64, R, Vec<u8>)>, StreamError>, ()> {
        loop {
            let Some(recv) = self.recv.as_mut() else {
                return Stream::Next(Ok(None));
            };

            if let Ok(Some((ty, used))) = super::varint::VarInt::decode(&self.buffer) {
                let rest = self.buffer[used..].to_vec();
                let recv = self.recv.take().expect("checked above");
                return Stream::Next(Ok(Some((ty.value(), recv, rest))));
            }

            match recv.read() {
                Stream::Next(Ok(Some(bytes))) => self.buffer.extend_from_slice(&bytes),
                Stream::Next(Ok(None)) => return Stream::Next(Ok(None)),
                Stream::Next(Err(e)) => return Stream::Next(Err(StreamError::Quic(e))),
                _ => return Stream::Pending(()),
            }
        }
    }
}

/// Bytes are what a DATA frame carries.
pub type Body = Bytes;
