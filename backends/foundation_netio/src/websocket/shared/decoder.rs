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
use std::io::Read;

use super::frame::{apply_mask, Opcode, WebSocketFrame};

/// How many header bytes (before payload) are still needed, and where they go.
#[derive(Debug)]
enum HeaderPhase {
    /// Reading the 2 base header bytes into `base_hdr[pos..]`.
    Base { pos: u8 },
    /// Reading a 2- or 8-byte extended payload length.
    ExtLen { need: u8, buf: [u8; 8], pos: u8 },
    /// Reading the 4-byte mask key.
    Mask { buf: [u8; 4], pos: u8 },
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
    /// Accumulated payload bytes (unmasked once complete).
    payload: Vec<u8>,
}

impl WebSocketFrameDecoder {
    /// Create a fresh decoder ready for the next frame.
    #[must_use]
    pub fn new() -> Self {
        Self {
            buffer: AccumulatingBuffer::new(),
            phase: HeaderPhase::Base { pos: 0 },
            base_hdr: [0u8; 2],
            masked: false,
            mask_key: [0u8; 4],
            payload_len: 0,
            payload: Vec::new(),
        }
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
}

impl Default for WebSocketFrameDecoder {
    fn default() -> Self {
        Self::new()
    }
}

impl IncrementalDecoder for WebSocketFrameDecoder {
    type Frame = WebSocketFrame;

    fn step(&mut self, src: &mut impl Read) -> Result<DecodeStep<Self::Frame>, DecodeError> {
        self.buffer.fill_from(src)?;

        loop {
            match self.phase {
                HeaderPhase::Base { pos } => {
                    let pos = pos as usize;
                    let need = 2 - pos;
                    let view = self.buffer.view();
                    if view.len() < need {
                        let n = view.len();
                        self.base_hdr[pos..pos + n].copy_from_slice(&view[..n]);
                        self.buffer.advance(n);
                        self.phase = HeaderPhase::Base {
                            pos: (pos + n) as u8,
                        };
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
                            self.payload_len = n as usize;
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
                    let need = need as usize;
                    let pos = pos as usize;
                    let bytes_needed = need - pos;
                    let view = self.buffer.view();
                    if view.len() < bytes_needed {
                        let n = view.len();
                        buf[pos..pos + n].copy_from_slice(&view[..n]);
                        self.buffer.advance(n);
                        self.phase = HeaderPhase::ExtLen {
                            need: need as u8,
                            buf,
                            pos: (pos + n) as u8,
                        };
                        return Ok(DecodeStep::Pending);
                    }
                    buf[pos..need].copy_from_slice(&view[..bytes_needed]);
                    self.buffer.advance(bytes_needed);
                    self.payload_len = if need == 2 {
                        u16::from_be_bytes([buf[0], buf[1]]) as usize
                    } else {
                        #[allow(clippy::cast_possible_truncation)]
                        {
                            u64::from_be_bytes(buf) as usize
                        }
                    };
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
                    let pos = pos as usize;
                    let need = 4 - pos;
                    let view = self.buffer.view();
                    if view.len() < need {
                        let n = view.len();
                        buf[pos..pos + n].copy_from_slice(&view[..n]);
                        self.buffer.advance(n);
                        self.phase = HeaderPhase::Mask {
                            buf,
                            pos: (pos + n) as u8,
                        };
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

    fn has_partial(&self) -> bool {
        !matches!(self.phase, HeaderPhase::Base { pos: 0 }) || !self.buffer.view().is_empty()
    }
}

// ── tests ───────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Cursor;

    /// A frame split across 1-byte reads — the canonical incremental test.
    #[test]
    fn frame_split_across_many_reads_decodes() {
        let wire: Vec<u8> = vec![0x81, 0x05, b'H', b'e', b'l', b'l', b'o'];
        let mut dec = WebSocketFrameDecoder::new();

        for (i, &b) in wire.iter().enumerate() {
            let buf = [b];
            let mut cursor = Cursor::new(&buf);
            let result = dec.step(&mut cursor).expect("step");
            if i == wire.len() - 1 {
                match result {
                    DecodeStep::Frame(frame) => {
                        assert!(frame.fin);
                        assert_eq!(frame.opcode, Opcode::Text);
                        assert_eq!(frame.payload, b"Hello".to_vec());
                        assert!(frame.mask.is_none());
                    }
                    DecodeStep::Pending => panic!("expected Frame on last byte"),
                }
            } else {
                assert_eq!(result, DecodeStep::Pending, "byte {i} = {b:#04x}");
            }
        }
    }

    /// Two complete frames back-to-back in a single read.
    #[test]
    fn two_frames_in_one_read() {
        let wire: Vec<u8> = vec![0x81, 0x02, b'H', b'i', 0x81, 0x02, b'Y', b'o'];
        let cursor = Cursor::new(wire);
        let mut dec = WebSocketFrameDecoder::new();

        let f1 = match dec.step(&mut cursor.clone()).expect("step") {
            DecodeStep::Frame(f) => f,
            other => panic!("expected Frame, got {other:?}"),
        };
        assert_eq!(f1.payload, b"Hi");

        // After consuming f1, the remaining bytes are still in our cursor.
        // Actually step() advances the cursor. Let's re-approach.
        // The cursor must be the same across steps.
        // Let's just pass all bytes at once and step twice.
        let wire: Vec<u8> = vec![0x81, 0x02, b'H', b'i', 0x81, 0x02, b'Y', b'o'];
        let mut cursor = Cursor::new(&wire);
        let mut dec = WebSocketFrameDecoder::new();

        let f1 = match dec.step(&mut cursor).expect("step") {
            DecodeStep::Frame(f) => f,
            other => panic!("expected Frame, got {other:?}"),
        };
        assert_eq!(f1.payload, b"Hi");

        let f2 = match dec.step(&mut cursor).expect("step") {
            DecodeStep::Frame(f) => f,
            other => panic!("expected Frame, got {other:?}"),
        };
        assert_eq!(f2.payload, b"Yo");
    }

    /// Masked frame (client→server, per RFC 6455).
    #[test]
    fn masked_frame_decodes() {
        let mask: [u8; 4] = [0x01, 0x02, 0x03, 0x04];
        let payload: &[u8] = b"Hello";
        let mut wire = vec![0x81, 0x85];
        wire.extend_from_slice(&mask);
        for (i, &b) in payload.iter().enumerate() {
            wire.push(b ^ mask[i % 4]);
        }

        let mut cursor = Cursor::new(wire);
        let mut dec = WebSocketFrameDecoder::new();
        let frame = match dec.step(&mut cursor).expect("step") {
            DecodeStep::Frame(f) => f,
            other => panic!("expected Frame, got {other:?}"),
        };
        assert_eq!(frame.payload, b"Hello");
        assert!(frame.mask.is_some());
        assert_eq!(frame.mask.unwrap(), mask);
    }

    /// 16-bit extended payload length (126 → 2-byte BE).
    #[test]
    fn extended_length_16bit() {
        let payload = vec![b'x'; 256];
        let mut wire = vec![0x82, 0x7E];
        wire.extend_from_slice(&(256u16).to_be_bytes());
        wire.extend_from_slice(&payload);

        let mut cursor = Cursor::new(wire);
        let mut dec = WebSocketFrameDecoder::new();
        let frame = match dec.step(&mut cursor).expect("step") {
            DecodeStep::Frame(f) => f,
            other => panic!("expected Frame, got {other:?}"),
        };
        assert_eq!(frame.opcode, Opcode::Binary);
        assert_eq!(frame.payload.len(), 256);
        assert_eq!(frame.payload, payload);
    }

    /// 64-bit extended payload length (127 → 8-byte BE).
    #[test]
    fn extended_length_64bit() {
        let payload = vec![b'z'; 70000];
        let mut wire = vec![0x81, 0x7F];
        wire.extend_from_slice(&(70000u64).to_be_bytes());
        wire.extend_from_slice(&payload);

        let mut cursor = Cursor::new(wire);
        let mut dec = WebSocketFrameDecoder::new();
        // 70000 bytes exceeds the default 8KB fill_from chunk — loop until Frame.
        let frame = loop {
            match dec.step(&mut cursor).expect("step") {
                DecodeStep::Frame(f) => break f,
                DecodeStep::Pending => continue,
            }
        };
        assert_eq!(frame.payload.len(), 70000);
        assert_eq!(frame.payload, payload);
    }

    /// Close frame (control frame, must have FIN=1).
    #[test]
    fn close_frame_decodes() {
        let wire: Vec<u8> = vec![0x88, 0x02, 0x03, 0xE8];
        let mut cursor = Cursor::new(wire);
        let mut dec = WebSocketFrameDecoder::new();
        let frame = match dec.step(&mut cursor).expect("step") {
            DecodeStep::Frame(f) => f,
            other => panic!("expected Frame, got {other:?}"),
        };
        assert_eq!(frame.opcode, Opcode::Close);
        assert!(frame.fin);
        assert_eq!(frame.payload, vec![0x03, 0xE8]);
    }

    /// has_partial reports true mid-frame.
    #[test]
    fn has_partial_mid_frame() {
        let mut dec = WebSocketFrameDecoder::new();
        assert!(!dec.has_partial());

        let buf = [0x81u8];
        let mut cursor = Cursor::new(&buf);
        let _ = dec.step(&mut cursor);
        assert!(dec.has_partial());

        let buf = [0x05, b'H', b'e', b'l', b'l', b'o'];
        let mut cursor = Cursor::new(&buf);
        let _ = dec.step(&mut cursor);
        assert!(!dec.has_partial());
    }

    /// Blocking decode wrapper produces same result as one-shot decode().
    #[test]
    fn blocking_wrapper_matches_decode() {
        let wire: Vec<u8> = vec![0x81, 0x05, b'H', b'e', b'l', b'l', b'o'];
        let mut cursor = Cursor::new(&wire);

        let frame = foundation_core::io::read_frame_blocking(
            &mut WebSocketFrameDecoder::new(),
            &mut cursor,
        )
        .expect("decode")
        .expect("frame");

        assert_eq!(frame.payload, b"Hello");
    }
}
