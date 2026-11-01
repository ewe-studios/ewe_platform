//! Docker's multiplexed stdout/stderr frame decoder.
//!
//! WHY: Container logs, attach and exec output (without a TTY) arrive as one
//! binary stream of 8-byte-header frames that must be decoded incrementally —
//! a frame can be split across any number of body chunks.
//!
//! WHAT: [`LogFrameDecoder`] for the 8-byte multiplexed format and
//! [`LogOutput`] for the typed frames it yields.
//!
//! HOW: Feed incoming `Bytes` chunks into the decoder; drain completed frames
//! without buffering the entire stream. The decoder itself does not bound a
//! frame — [`LogStream`](crate::client::body::LogStream) checks each frame's
//! announced length against its configured cap before buffering it.

use bytes::Bytes;

// =============================================================================
// Docker multiplexed log format
// =============================================================================

/// A decoded frame from Docker's multiplexed I/O stream.
///
/// Docker's log format: `[stream_type: u8][padding: u8; 3][length: u32 BE][payload]`
///
/// Stream types:
/// - 0 — stdin (multiplexed to container)
/// - 1 — stdout (from container)
/// - 2 — stderr (from container)
/// - 3 — console (from container, reserved)
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum LogOutput {
    /// Data sent **to** the container's stdin (stream type 0).
    StdIn {
        /// Raw payload bytes.
        message: Bytes,
    },
    /// Standard output from the container (stream type 1).
    StdOut {
        /// Raw payload bytes.
        message: Bytes,
    },
    /// Standard error from the container (stream type 2).
    StdErr {
        /// Raw payload bytes.
        message: Bytes,
    },
    /// Console output (stream type 3) — reserved by Docker.
    Console {
        /// Raw payload bytes.
        message: Bytes,
    },
}

/// Incremental decoder for Docker's 8-byte multiplexed stream format.
///
/// WHY: `GET /containers/{id}/logs?stdout=1&stderr=1` returns interleaved
/// frames — stdout and stderr in one stream. This decoder splits them out.
///
/// WHAT: Buffers partial data across `feed()` calls; `decode()` drains complete
/// frames. Stores a sliding-window cursor to avoid re-allocating the buffer on
/// each drain cycle.
///
/// HOW: Each call to `feed()` appends to an internal buffer. Each call to
/// `decode()` consumes as many complete frames as possible without copying.
pub struct LogFrameDecoder {
    buffer: Vec<u8>,
    cursor: usize,
}

/// What one [`LogFrameDecoder::decode`] call made of the buffered bytes.
///
/// WHY: `decode` used to return `None` for two different things — not enough
/// bytes yet, and a frame of an unknown stream type that was skipped. A caller
/// must tell them apart: after a skip there may be a next frame to decode
/// without feeding more, after [`NeedMore`](DecodeOutcome::NeedMore) there is
/// not.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum DecodeOutcome {
    /// Not enough bytes for the next full frame; feed more and call again.
    NeedMore,
    /// A frame with an unknown stream type was skipped; its payload is
    /// discarded. Call again for the next frame.
    Skipped,
    /// A complete frame.
    Frame(LogOutput),
}

impl LogFrameDecoder {
    /// Create a new decoder with an empty buffer.
    #[must_use]
    pub fn new() -> Self {
        Self {
            buffer: Vec::new(),
            cursor: 0,
        }
    }

    /// Feed a raw chunk from the stream into the decoder.
    ///
    /// Call this for every `Bytes` chunk received from the body observer.
    pub fn feed(&mut self, chunk: &[u8]) {
        self.buffer.extend_from_slice(chunk);
    }

    /// Try to decode the next complete frame.
    ///
    /// [`DecodeOutcome::NeedMore`] when there is not enough data for a full
    /// frame (header + payload); the remaining bytes are kept for the next
    /// `feed()` + `decode()` cycle. [`DecodeOutcome::Skipped`] when the header
    /// names an unknown stream type; the cursor moves past that frame's
    /// payload. [`DecodeOutcome::Frame`] for a decoded frame.
    ///
    /// # Panics
    ///
    /// Never panics — a malformed stream (exhausted mid-payload) safely
    /// returns [`DecodeOutcome::NeedMore`].
    #[must_use]
    pub fn decode(&mut self) -> DecodeOutcome {
        const HEADER_SIZE: usize = 8;

        // Need at least a header
        if self.buffer.len() - self.cursor < HEADER_SIZE {
            return DecodeOutcome::NeedMore;
        }

        let header = &self.buffer[self.cursor..self.cursor + HEADER_SIZE];
        let stream_type = header[0];
        // header[1..4] are reserved padding, ignore
        let length = u32::from_be_bytes([header[4], header[5], header[6], header[7]]) as usize;

        // Need the full payload
        if self.buffer.len() - self.cursor < HEADER_SIZE + length {
            return DecodeOutcome::NeedMore;
        }

        let payload_start = self.cursor + HEADER_SIZE;
        let payload = Bytes::copy_from_slice(&self.buffer[payload_start..payload_start + length]);
        self.cursor = payload_start + length;

        match stream_type {
            0 => DecodeOutcome::Frame(LogOutput::StdIn { message: payload }),
            1 => DecodeOutcome::Frame(LogOutput::StdOut { message: payload }),
            2 => DecodeOutcome::Frame(LogOutput::StdErr { message: payload }),
            3 => DecodeOutcome::Frame(LogOutput::Console { message: payload }),
            _ => {
                // Unknown stream type — skip (shouldn't happen with well-formed
                // Docker output, but don't panic on malformed data).
                DecodeOutcome::Skipped
            }
        }
    }

    /// Compact the buffer by discarding already-decoded bytes.
    ///
    /// Call this periodically to prevent unbounded growth when streaming
    /// long-lived logs (follow mode). After compact, the internal cursor
    /// resets to 0.
    pub fn compact(&mut self) {
        if self.cursor > 0 {
            self.buffer.drain(..self.cursor);
            self.cursor = 0;
        }
    }

    /// Number of bytes currently buffered (not yet decoded).
    #[must_use]
    pub fn buffered_len(&self) -> usize {
        self.buffer.len() - self.cursor
    }

    /// The payload length the next frame's header announces, once its
    /// 8-byte header has been buffered; `None` before that.
    ///
    /// WHY: lets a reader refuse a frame larger than it is willing to hold
    /// from its header alone, before buffering any of its payload.
    #[must_use]
    pub fn next_frame_len(&self) -> Option<usize> {
        let header = self.buffer.get(self.cursor..self.cursor + 8)?;
        Some(u32::from_be_bytes([header[4], header[5], header[6], header[7]]) as usize)
    }
}

impl Default for LogFrameDecoder {
    fn default() -> Self {
        Self::new()
    }
}
