//! Resumable WebSocket frame decoder (`IncrementalDecoder` impl, F36).
//!
//! WHY: The existing `WebSocketFrame::decode()` is one-shot — it blocks on every
//! byte. Non-blocking fds deliver frames across many reads, so a decoder must
//! save partial header/payload progress and resume. The shared
//! `IncrementalDecoder` seam (Decision 12 §11) already serves Connect envelopes
//! and HTTP/2 frames; WebSocket is the next protocol to ride it.
//!
//! WHAT: [`WebSocketFrameDecoder`] — an `IncrementalDecoder<Frame = WebSocketFrame>`
//! over an `AccumulatingBuffer`. Each `step()` advances a byte-level state
//! machine (header → length → mask → payload) with whatever bytes are available;
//! `Pending` means "feed more."
//!
//! HOW: The state machine tracks exactly which bytes are still needed for the
//! header (2 base + 0/2/8 extended length + 0/4 mask). Once the header is
//! complete, `payload_len` is known and the decoder drains payload bytes
//! directly from the buffer. Masks are applied at the end so partial reads
//! accumulate unmasked bytes incrementally; the output payload is always
//! unmasked — the caller never sees the mask bytes.
//!
//! The existing blocking `decode()` is preserved unchanged; the incremental
//! decoder is additive.

use foundation_core::io::{AccumulatingBuffer, DecodeError, DecodeStep, IncrementalDecoder};
use std::io::{self, Read};

use super::error::WebSocketError;

use super::frame::{apply_mask, Opcode, WebSocketFrame};

/// How many header bytes (before payload) are still needed, and where they go.
#[derive(Debug)]
enum HeaderPhase {
    /// Reading the 2 base header bytes into `base_hdr[pos..]`.
    Base { pos: usize },
    /// Reading a 2- or 8-byte extended payload length.
    ExtLen {
        need: usize,
        buf: [u8; 8],
        pos: usize,
    },
    /// Reading the 4-byte mask key.
    Mask { buf: [u8; 4], pos: usize },
    /// Header complete. `payload_len` bytes of payload expected.
    Payload,
}

/// Resumable WebSocket frame decoder backed by an [`AccumulatingBuffer`].
pub struct WebSocketFrameDecoder {
    buffer: AccumulatingBuffer,
    phase: HeaderPhase,
    /// The 2 base header bytes once fully read.
    base_hdr: [u8; 2],
    /// Whether the MASK bit was set.
    masked: bool,
    /// The 4-byte mask key (valid only when `masked && phase == Payload`).
    mask_key: [u8; 4],
    /// Determined payload length in bytes.
    payload_len: usize,
    max_payload: usize,
    /// Accumulated payload bytes (unmasked once complete).
    payload: Vec<u8>,
}

impl WebSocketFrameDecoder {
    /// Create a fresh decoder ready for the next frame.
    #[must_use]
    pub fn new() -> Self {
        Self::with_limit(usize::MAX)
    }

    /// Rejects a frame that exceeds the byte limit before reading its payload.
    #[must_use]
    pub fn with_limit(max_payload: usize) -> Self {
        Self {
            max_payload,
            buffer: AccumulatingBuffer::new(),
            phase: HeaderPhase::Base { pos: 0 },
            base_hdr: [0u8; 2],
            masked: false,
            mask_key: [0u8; 4],
            payload_len: 0,
            payload: Vec::new(),
        }
    }

    fn check_length(&self) -> Result<(), DecodeError> {
        if self.payload_len > self.max_payload {
            return Err(DecodeError::protocol(
                "WebSocket frame exceeds the configured limit",
            ));
        }
        Ok(())
    }

    /// How many payload bytes have been accumulated so far.
    #[must_use]
    pub fn payload_got(&self) -> usize {
        self.payload.len()
    }

    /// Reset internal state for the next frame.
    fn reset(&mut self) {
        self.phase = HeaderPhase::Base { pos: 0 };
        self.base_hdr = [0u8; 2];
        self.masked = false;
        self.mask_key = [0u8; 4];
        self.payload_len = 0;
        self.payload.clear();
    }

    /// Consume the completed frame and reset for the next.
    fn take_frame(&mut self) -> Result<WebSocketFrame, DecodeError> {
        let fin = (self.base_hdr[0] & 0x80) != 0;
        let opcode = Opcode::from_byte(self.base_hdr[0] & 0x0F)
            .map_err(|e| DecodeError::protocol(e.to_string()))?;

        if opcode.is_control() {
            if !fin {
                return Err(DecodeError::protocol(
                    "control frames must not be fragmented (FIN must be set)",
                ));
            }
            if self.payload.len() > 125 {
                return Err(DecodeError::protocol(format!(
                    "control frame payload too large: {} bytes (max 125)",
                    self.payload.len()
                )));
            }
        }

        let mask = if self.masked {
            Some(self.mask_key)
        } else {
            None
        };
        let payload = std::mem::take(&mut self.payload);
        self.reset();
        Ok(WebSocketFrame {
            fin,
            opcode,
            mask,
            payload,
        })
    }

    /// Read one frame from a socket, retaining partial bytes across retries.
    ///
    /// # Errors
    /// Returns the socket error or a protocol error. A zero-byte socket read
    /// returns unexpected end of stream.
    pub fn read_frame(&mut self, source: &mut impl Read) -> Result<WebSocketFrame, WebSocketError> {
        loop {
            match self.parse_buffer().map_err(|error| match error {
                DecodeError::Io(error) => WebSocketError::IoError(error),
                DecodeError::Protocol(message) => WebSocketError::ProtocolError(message.into()),
            })? {
                DecodeStep::Frame(frame) => return Ok(frame),
                DecodeStep::Pending => {
                    // The shared socket reader fills the requested slice before
                    // returning. Request only bytes needed for the current frame.
                    let remaining = match self.phase {
                        HeaderPhase::Base { pos } => 2 - pos,
                        HeaderPhase::ExtLen { need, pos, .. } => need - pos,
                        HeaderPhase::Mask { pos, .. } => 4 - pos,
                        HeaderPhase::Payload => self.payload_len - self.payload.len(),
                    };
                    if self.buffer.fill_from(&mut SocketRead(source, remaining))? == 0 {
                        return Err(io::Error::from(io::ErrorKind::WouldBlock).into());
                    }
                }
            }
        }
    }

    #[allow(clippy::too_many_lines)]
    fn parse_buffer(&mut self) -> Result<DecodeStep<WebSocketFrame>, DecodeError> {
        loop {
            match self.phase {
                HeaderPhase::Base { pos } => {
                    let need = 2 - pos;
                    let view = self.buffer.view();
                    if view.len() < need {
                        let n = view.len();
                        self.base_hdr[pos..pos + n].copy_from_slice(&view[..n]);
                        self.buffer.advance(n);
                        self.phase = HeaderPhase::Base { pos: pos + n };
                        return Ok(DecodeStep::Pending);
                    }
                    self.base_hdr[pos..2].copy_from_slice(&view[..need]);
                    self.buffer.advance(need);
                    self.masked = (self.base_hdr[1] & 0x80) != 0;
                    let len7 = self.base_hdr[1] & 0x7F;
                    self.phase = match len7 {
                        126 => HeaderPhase::ExtLen {
                            need: 2,
                            buf: [0u8; 8],
                            pos: 0,
                        },
                        127 => HeaderPhase::ExtLen {
                            need: 8,
                            buf: [0u8; 8],
                            pos: 0,
                        },
                        n => {
                            self.payload_len = usize::from(n);
                            self.check_length()?;
                            if self.masked {
                                HeaderPhase::Mask {
                                    buf: [0u8; 4],
                                    pos: 0,
                                }
                            } else {
                                HeaderPhase::Payload
                            }
                        }
                    };
                }
                HeaderPhase::ExtLen { need, mut buf, pos } => {
                    let bytes_needed = need - pos;
                    let view = self.buffer.view();
                    if view.len() < bytes_needed {
                        let n = view.len();
                        buf[pos..pos + n].copy_from_slice(&view[..n]);
                        self.buffer.advance(n);
                        self.phase = HeaderPhase::ExtLen {
                            need,
                            buf,
                            pos: pos + n,
                        };
                        return Ok(DecodeStep::Pending);
                    }
                    buf[pos..need].copy_from_slice(&view[..bytes_needed]);
                    self.buffer.advance(bytes_needed);
                    self.payload_len = if need == 2 {
                        u16::from_be_bytes([buf[0], buf[1]]) as usize
                    } else {
                        usize::try_from(u64::from_be_bytes(buf)).map_err(|_| {
                            DecodeError::protocol("WebSocket frame length exceeds address space")
                        })?
                    };
                    self.check_length()?;
                    self.phase = if self.masked {
                        HeaderPhase::Mask {
                            buf: [0u8; 4],
                            pos: 0,
                        }
                    } else {
                        HeaderPhase::Payload
                    };
                }
                HeaderPhase::Mask { mut buf, pos } => {
                    let need = 4 - pos;
                    let view = self.buffer.view();
                    if view.len() < need {
                        let n = view.len();
                        buf[pos..pos + n].copy_from_slice(&view[..n]);
                        self.buffer.advance(n);
                        self.phase = HeaderPhase::Mask { buf, pos: pos + n };
                        return Ok(DecodeStep::Pending);
                    }
                    buf[pos..4].copy_from_slice(&view[..need]);
                    self.buffer.advance(need);
                    self.mask_key = buf;
                    self.phase = HeaderPhase::Payload;
                }
                HeaderPhase::Payload => {
                    let remaining = self.payload_len - self.payload.len();
                    if remaining == 0 {
                        return Ok(DecodeStep::Frame(self.take_frame()?));
                    }
                    let view = self.buffer.view();
                    let take = remaining.min(view.len());
                    self.payload.extend_from_slice(&view[..take]);
                    self.buffer.advance(take);
                    if self.payload.len() == self.payload_len {
                        // Unmask the complete payload once all bytes are in.
                        if self.masked {
                            apply_mask(&mut self.payload, self.mask_key);
                        }
                        return Ok(DecodeStep::Frame(self.take_frame()?));
                    }
                    return Ok(DecodeStep::Pending);
                }
            }
        }
    }
}

impl Default for WebSocketFrameDecoder {
    fn default() -> Self {
        Self::new()
    }
}

struct SocketRead<'a, R>(&'a mut R, usize);

impl<R: Read> Read for SocketRead<'_, R> {
    fn read(&mut self, buffer: &mut [u8]) -> io::Result<usize> {
        let length = buffer.len().min(self.1);
        let buffer = &mut buffer[..length];
        loop {
            match self.0.read(buffer) {
                Ok(0) if !buffer.is_empty() => {
                    return Err(io::Error::from(io::ErrorKind::UnexpectedEof));
                }
                Err(error) if error.kind() == io::ErrorKind::Interrupted => {}
                result => return result,
            }
        }
    }
}

impl IncrementalDecoder for WebSocketFrameDecoder {
    type Frame = WebSocketFrame;

    fn step(&mut self, source: &mut impl Read) -> Result<DecodeStep<Self::Frame>, DecodeError> {
        match self.parse_buffer()? {
            frame @ DecodeStep::Frame(_) => Ok(frame),
            DecodeStep::Pending => {
                self.buffer.fill_from(source)?;
                self.parse_buffer()
            }
        }
    }

    fn has_partial(&self) -> bool {
        !matches!(self.phase, HeaderPhase::Base { pos: 0 }) || !self.buffer.view().is_empty()
    }
}
