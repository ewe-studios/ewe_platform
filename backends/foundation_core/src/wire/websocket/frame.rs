//! WebSocket frame encoding and decoding (RFC 6455).
//!
//! WHY: WebSocket communication requires framing at the wire level.
//! WHAT: Implements [`WebSocketFrame`] with encode/decode per RFC 6455 Section 5.2.
//! HOW: Binary encoding with FIN bit, opcode, mask, and variable-length payload.

use std::io::Read;

use super::error::WebSocketError;

/// WHY: RFC 6455 defines specific opcodes for different frame types.
///
/// WHAT: Represents the 4-bit opcode field in a WebSocket frame header.
///
/// HOW: Each variant maps to its RFC-defined numeric value.
///
/// # Panics
/// Never panics.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Opcode {
    /// Continuation frame for fragmented messages.
    Continuation = 0x0,
    /// Text data frame (UTF-8 encoded).
    Text = 0x1,
    /// Binary data frame.
    Binary = 0x2,
    /// Connection close control frame.
    Close = 0x8,
    /// Ping control frame.
    Ping = 0x9,
    /// Pong control frame.
    Pong = 0xA,
}

impl Opcode {
    /// WHY: Incoming frames contain a raw byte opcode that must be validated.
    ///
    /// WHAT: Converts a byte value to an [`Opcode`] variant.
    ///
    /// HOW: Matches the byte against known opcode values.
    ///
    /// # Errors
    /// Returns [`WebSocketError::InvalidFrame`] if the byte is not a valid opcode.
    ///
    /// # Panics
    /// Never panics.
    pub fn from_byte(byte: u8) -> Result<Self, WebSocketError> {
        match byte {
            0x0 => Ok(Opcode::Continuation),
            0x1 => Ok(Opcode::Text),
            0x2 => Ok(Opcode::Binary),
            0x8 => Ok(Opcode::Close),
            0x9 => Ok(Opcode::Ping),
            0xA => Ok(Opcode::Pong),
            _ => Err(WebSocketError::InvalidFrame(format!(
                "unknown opcode: 0x{byte:X}"
            ))),
        }
    }

    /// WHY: Need to check if a frame is a control frame for validation rules.
    ///
    /// WHAT: Returns true if this opcode represents a control frame.
    ///
    /// HOW: Control frames are Close (0x8), Ping (0x9), and Pong (0xA).
    ///
    /// # Panics
    /// Never panics.
    #[must_use]
    pub fn is_control(&self) -> bool {
        matches!(self, Opcode::Close | Opcode::Ping | Opcode::Pong)
    }
}

/// WHY: WebSocket communication is frame-based; each message is one or more frames.
///
/// WHAT: Represents a single WebSocket frame with header fields and payload.
///
/// HOW: Stores the FIN bit, opcode, optional mask, and payload data.
///
/// # Panics
/// Never panics.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct WebSocketFrame {
    /// Whether this is the final fragment in a message.
    pub fin: bool,
    /// Frame type identifier.
    pub opcode: Opcode,
    /// Optional 4-byte masking key (clients must mask, servers must not).
    pub mask: Option<[u8; 4]>,
    /// Frame payload data.
    pub payload: Vec<u8>,
}

/// WHY: Masking is required by RFC 6455 for client-to-server frames.
///
/// WHAT: Applies or removes XOR masking on payload data in-place.
///
/// HOW: Each byte is XOR-ed with `mask[i % 4]`. The operation is its own inverse.
///
/// # Panics
/// Never panics.
pub fn apply_mask(payload: &mut [u8], mask: [u8; 4]) {
    for (i, byte) in payload.iter_mut().enumerate() {
        *byte ^= mask[i % 4];
    }
}

impl WebSocketFrame {
    /// WHY: Frames must be serialized to bytes for transmission over the wire.
    ///
    /// WHAT: Encodes this frame into a byte vector per RFC 6455 Section 5.2.
    ///
    /// HOW: Builds the 2-byte header (FIN + opcode, MASK + length), followed by
    /// extended length bytes if needed, masking key if present, and payload
    /// (masked if a key is set).
    ///
    /// # Panics
    /// Never panics.
    #[must_use]
    pub fn encode(&self) -> Vec<u8> {
        let mut buf = Vec::new();

        // First byte: FIN + RSV(0) + opcode
        let first_byte = if self.fin { 0x80 } else { 0x00 } | (self.opcode as u8);
        buf.push(first_byte);

        // Second byte: MASK bit + payload length
        let mask_bit: u8 = if self.mask.is_some() { 0x80 } else { 0x00 };
        let len = self.payload.len();

        if len < 126 {
            #[allow(clippy::cast_possible_truncation)] // len < 126 fits in u8
            buf.push(mask_bit | (len as u8));
        } else if len <= 65535 {
            buf.push(mask_bit | 126);
            #[allow(clippy::cast_possible_truncation)] // len <= 65535 fits in u16
            buf.extend_from_slice(&(len as u16).to_be_bytes());
        } else {
            buf.push(mask_bit | 127);
            buf.extend_from_slice(&(len as u64).to_be_bytes());
        }

        // Masking key + masked payload
        if let Some(mask) = self.mask {
            buf.extend_from_slice(&mask);
            let mut masked_payload = self.payload.clone();
            apply_mask(&mut masked_payload, mask);
            buf.extend_from_slice(&masked_payload);
        } else {
            buf.extend_from_slice(&self.payload);
        }

        buf
    }

    /// WHY: Received bytes must be parsed back into frame structures.
    ///
    /// WHAT: Decodes a WebSocket frame from a byte stream.
    ///
    /// HOW: Reads the 2-byte header, determines payload length (handling extended
    /// lengths), reads the masking key if present, reads payload, and unmasks it.
    ///
    /// # Errors
    /// Returns [`WebSocketError::IoError`] on read failure.
    /// Returns [`WebSocketError::InvalidFrame`] if the frame header is malformed.
    ///
    /// # Panics
    /// Never panics.
    pub fn decode(reader: &mut impl Read) -> Result<Self, WebSocketError> {
        // Read first 2 bytes
        let mut header = [0u8; 2];
        reader.read_exact(&mut header)?;

        let fin = (header[0] & 0x80) != 0;
        let opcode = Opcode::from_byte(header[0] & 0x0F)?;
        let masked = (header[1] & 0x80) != 0;
        let length_byte = header[1] & 0x7F;

        // Determine payload length
        let payload_len: usize = match length_byte {
            126 => {
                let mut buf = [0u8; 2];
                reader.read_exact(&mut buf)?;
                u16::from_be_bytes(buf) as usize
            }
            127 => {
                let mut buf = [0u8; 8];
                reader.read_exact(&mut buf)?;
                #[allow(clippy::cast_possible_truncation)] // WebSocket frames won't exceed usize
                let len = u64::from_be_bytes(buf) as usize;
                len
            }
            n => n as usize,
        };

        // Read masking key if present
        let mask = if masked {
            let mut key = [0u8; 4];
            reader.read_exact(&mut key)?;
            Some(key)
        } else {
            None
        };

        // Read payload
        let mut payload = vec![0u8; payload_len];
        reader.read_exact(&mut payload)?;

        // Unmask payload if masked
        if let Some(mask_key) = mask {
            apply_mask(&mut payload, mask_key);
        }

        Ok(WebSocketFrame {
            fin,
            opcode,
            mask,
            payload,
        })
    }

    /// WHY: Control frames have specific constraints per RFC 6455 Section 5.5.
    ///
    /// WHAT: Validates that this frame conforms to WebSocket protocol rules.
    ///
    /// HOW: Checks that control frames have FIN=1 and payload <= 125 bytes.
    ///
    /// # Errors
    /// Returns [`WebSocketError::ProtocolError`] if validation fails.
    ///
    /// # Panics
    /// Never panics.
    pub fn validate(&self) -> Result<(), WebSocketError> {
        if self.opcode.is_control() {
            if !self.fin {
                return Err(WebSocketError::ProtocolError(
                    "control frames must not be fragmented (FIN must be set)".to_string(),
                ));
            }
            if self.payload.len() > 125 {
                return Err(WebSocketError::ProtocolError(format!(
                    "control frame payload too large: {} bytes (max 125)",
                    self.payload.len()
                )));
            }
        }
        Ok(())
    }
}
