//! Docker streaming decoders.
//!
//! WHY: Docker streaming endpoints return binary frame formats (container logs,
//! attach I/O) and JSON-line streams (events, stats, build progress) that need
//! incremental decoding — you can't just `serde_json::from_slice(&body)` the
//! whole response.
//!
//! WHAT: [`LogFrameDecoder`] for the 8-byte multiplexed log format,
//! [`LogOutput`] for typed stream frames, and [`JsonLineDecoder`] for newline-
//! delimited JSON streams (events, stats, pull/push/build progress).
//!
//! HOW: Feed incoming `Bytes` chunks into the decoder; drain completed frames
//! without buffering the entire stream.

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
    /// Returns `None` when there isn't enough data for a full frame (header +
    /// payload). The remaining bytes are kept for the next `feed()` + `decode()`
    /// cycle.
    ///
    /// # Panics
    ///
    /// Never panics — malformed frames (exhausted stream mid-payload) safely
    /// return `None`.
    #[must_use]
    pub fn decode(&mut self) -> Option<LogOutput> {
        const HEADER_SIZE: usize = 8;

        // Need at least a header
        if self.buffer.len() - self.cursor < HEADER_SIZE {
            return None;
        }

        let header = &self.buffer[self.cursor..self.cursor + HEADER_SIZE];
        let stream_type = header[0];
        // header[1..4] are reserved padding, ignore
        let length = u32::from_be_bytes([header[4], header[5], header[6], header[7]]) as usize;

        // Need the full payload
        if self.buffer.len() - self.cursor < HEADER_SIZE + length {
            return None;
        }

        let payload_start = self.cursor + HEADER_SIZE;
        let payload = Bytes::copy_from_slice(&self.buffer[payload_start..payload_start + length]);
        self.cursor = payload_start + length;

        match stream_type {
            0 => Some(LogOutput::StdIn { message: payload }),
            1 => Some(LogOutput::StdOut { message: payload }),
            2 => Some(LogOutput::StdErr { message: payload }),
            3 => Some(LogOutput::Console { message: payload }),
            _ => {
                // Unknown stream type — skip (shouldn't happen with well-formed
                // Docker output, but don't panic on malformed data).
                None
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
}

impl Default for LogFrameDecoder {
    fn default() -> Self {
        Self::new()
    }
}

// =============================================================================
// JSON-line stream decoder (events, stats, build/pull/push progress)
// =============================================================================

/// Incremental decoder for JSON-line (newline-delimited JSON) streams.
///
/// WHY: Docker's events, stats, and progress endpoints emit one JSON object per
/// line. [`serde_json::from_slice`] fails mid-stream because the buffer contains
/// trailing partial data.
///
/// WHAT: Buffers raw bytes; drains complete lines (terminated by `\n`).
/// Caller parses each line as JSON.
///
/// HOW: Feed bytes with `feed()`, drain lines with `decode()`.
pub struct JsonLineDecoder {
    buffer: Vec<u8>,
    cursor: usize,
}

impl JsonLineDecoder {
    /// Create a new decoder with an empty buffer.
    #[must_use]
    pub fn new() -> Self {
        Self {
            buffer: Vec::new(),
            cursor: 0,
        }
    }

    /// Feed a raw chunk from the stream into the decoder.
    pub fn feed(&mut self, chunk: &[u8]) {
        self.buffer.extend_from_slice(chunk);
    }

    /// Try to decode the next complete JSON line.
    ///
    /// Returns `None` when no complete line (terminated by `\n`) is available.
    /// The returned `Bytes` does **not** include the trailing `\n`.
    #[must_use]
    pub fn decode(&mut self) -> Option<Bytes> {
        let remaining = &self.buffer[self.cursor..];
        let nl_pos = remaining.iter().position(|&b| b == b'\n')?;

        let line_start = self.cursor;
        let line_end = self.cursor + nl_pos;
        let line = Bytes::copy_from_slice(&self.buffer[line_start..line_end]);
        self.cursor = line_end + 1; // skip past the \n

        // Skip empty lines (trailing \n after a complete frame)
        if line.is_empty() {
            return self.decode();
        }

        Some(line)
    }

    /// Compact the buffer by discarding already-decoded bytes.
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
}

impl Default for JsonLineDecoder {
    fn default() -> Self {
        Self::new()
    }
}
