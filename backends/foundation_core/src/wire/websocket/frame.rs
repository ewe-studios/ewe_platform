//! WebSocket frame encoding and decoding (RFC 6455).
//!
//! WHY: WebSocket communication requires framing at the wire level.
//! WHAT: Implements [`WebSocketFrame`] with encode/decode per RFC 6455 Section 5.2.
//! HOW: Binary encoding with FIN bit, opcode, mask, and variable-length payload.

use std::io::Read;

use super::error::WebSocketError;
use bytes::BytesMut;

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

/// Generate a random 4-byte masking key as required by RFC 6455.
///
/// WHY: Clients MUST mask all frames sent to the server.
///
/// WHAT: Returns a random 4-byte mask.
#[must_use]
pub fn generate_mask() -> [u8; 4] {
    // Use a simple random generator for the mask
    // In production, you might want to use a cryptographically secure RNG
    [
        fastrand::u8(..),
        fastrand::u8(..),
        fastrand::u8(..),
        fastrand::u8(..),
    ]
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
        // Read the first header byte using `read` (not `read_exact`) to preserve
        // WouldBlock/TimedOut errors. `read_exact` converts these to UnexpectedEof,
        // which makes it impossible for callers to distinguish "no data yet" from
        // "connection closed."
        let mut header = [0u8; 2];
        let first_byte_read = match reader.read(&mut header[..1]) {
            Ok(0) => {
                tracing::debug!(
                    "Received Ok(0) — no data available yet on TCP stream, signal retry"
                );

                // On a TCP stream, Ok(0) doesn't necessarily mean EOF.
                // The stream may simply have no data available yet.
                // Return WouldBlock so callers retry on the next poll cycle.
                return Err(WebSocketError::IoError(std::io::Error::new(
                    std::io::ErrorKind::WouldBlock,
                    "zero bytes read from stream, retry later",
                )));
            }
            Ok(n) => {
                tracing::debug!(
                    "Read first header byte: {:#04x} (read {} bytes)",
                    header[0],
                    n
                );
                true
            }
            Err(e) => {
                tracing::debug!("Error occured reading data into buf: {:?}", e);
                // Propagate WouldBlock/TimedOut directly so callers can retry
                return Err(WebSocketError::IoError(e));
            }
        };
        debug_assert!(first_byte_read);

        // IMPORTANT: From this point on, we have consumed the first header byte.
        // Any I/O error (timeout, WouldBlock) means the stream is now in an
        // inconsistent state — the caller CANNOT retry frame decoding from scratch.
        // We wrap such errors as ProtocolError to signal stream corruption.
        //
        // Helper macro to convert I/O errors after partial read into ProtocolError.
        let map_partial_read_err = |e: WebSocketError| -> WebSocketError {
            match &e {
                WebSocketError::IoError(io_err)
                    if io_err.kind() == std::io::ErrorKind::WouldBlock
                        || io_err.kind() == std::io::ErrorKind::TimedOut =>
                {
                    tracing::error!(
                        "I/O error during partial frame read (stream corrupted): {:?}",
                        io_err
                    );
                    WebSocketError::ProtocolError(format!(
                        "Partial frame read interrupted by I/O error (stream corrupted): {}",
                        io_err
                    ))
                }
                _ => e,
            }
        };

        // Now read the second header byte — data is flowing, read_exact is safe
        reader
            .read_exact(&mut header[1..2])
            .map_err(|e| map_partial_read_err(e.into()))?;
        tracing::debug!(
            "Read second header byte: {:#04x}, full header: [{:#04x}, {:#04x}]",
            header[1],
            header[0],
            header[1]
        );

        let fin = (header[0] & 0x80) != 0;
        let opcode = Opcode::from_byte(header[0] & 0x0F)?;
        let masked = (header[1] & 0x80) != 0;
        let length_byte = header[1] & 0x7F;

        // Determine payload length
        let payload_len: usize = match length_byte {
            126 => {
                let mut buf = [0u8; 2];
                reader
                    .read_exact(&mut buf)
                    .map_err(|e| map_partial_read_err(e.into()))?;
                u16::from_be_bytes(buf) as usize
            }
            127 => {
                let mut buf = [0u8; 8];
                reader
                    .read_exact(&mut buf)
                    .map_err(|e| map_partial_read_err(e.into()))?;
                #[allow(clippy::cast_possible_truncation)] // WebSocket frames won't exceed usize
                let len = u64::from_be_bytes(buf) as usize;
                len
            }
            n => n as usize,
        };

        // Read masking key if present
        let mask = if masked {
            let mut key = [0u8; 4];
            reader
                .read_exact(&mut key)
                .map_err(|e| map_partial_read_err(e.into()))?;
            Some(key)
        } else {
            None
        };

        // Read payload
        let mut payload = vec![0u8; payload_len];
        reader
            .read_exact(&mut payload)
            .map_err(|e| map_partial_read_err(e.into()))?;

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

    /// WHY: After decoding a frame, callers need to convert it to a high-level message.
    ///
    /// WHAT: Converts this frame into a [`WebSocketMessage`] variant.
    ///
    /// HOW: Matches on opcode and extracts payload. For Close frames, parses the
    /// 2-byte status code and UTF-8 reason. For Text frames, validates UTF-8.
    ///
    /// # Errors
    /// Returns [`WebSocketError::InvalidUtf8`] if Text payload is not valid UTF-8.
    /// Returns [`WebSocketError::InvalidFrame`] for unexpected Continuation frames.
    ///
    /// # Panics
    /// Never panics.
    pub fn to_message(self) -> Result<super::message::WebSocketMessage, WebSocketError> {
        use super::message::WebSocketMessage;

        match self.opcode {
            Opcode::Text => {
                let text = String::from_utf8(self.payload)?;
                Ok(WebSocketMessage::Text(text))
            }
            Opcode::Binary => Ok(WebSocketMessage::Binary(self.payload)),
            Opcode::Ping => Ok(WebSocketMessage::Ping(self.payload)),
            Opcode::Pong => Ok(WebSocketMessage::Pong(self.payload)),
            Opcode::Close => {
                if self.payload.is_empty() {
                    Ok(WebSocketMessage::Close(1005, String::new()))
                } else if self.payload.len() == 1 {
                    // Invalid close payload (must be 0 or >=2 bytes)
                    Ok(WebSocketMessage::Close(
                        1002,
                        "Invalid close payload".to_string(),
                    ))
                } else {
                    let code = u16::from_be_bytes([self.payload[0], self.payload[1]]);
                    let reason = if self.payload.len() > 2 {
                        String::from_utf8_lossy(&self.payload[2..]).to_string()
                    } else {
                        String::new()
                    };
                    Ok(WebSocketMessage::Close(code, reason))
                }
            }
            Opcode::Continuation => {
                // Continuation frames should be handled by MessageAssembler, not directly
                Err(WebSocketError::InvalidFrame(
                    "unexpected Continuation frame (use MessageAssembler for fragmented messages)"
                        .to_string(),
                ))
            }
        }
    }

    /// Decode a WebSocket frame using a provided buffer for zero-copy reads.
    ///
    /// WHY: Reduces allocation overhead by reusing buffers for frame payloads.
    ///
    /// WHAT: Same as `decode()` but uses caller-supplied buffer for payload storage.
    ///
    /// HOW: Reads header, then reads payload directly into the provided buffer.
    ///
    /// # Arguments
    ///
    /// * `reader` - The stream to read from
    /// * `payload_buf` - Buffer to store payload (cleared before use)
    ///
    /// # Returns
    ///
    /// Decoded WebSocketFrame with payload in the provided buffer.
    ///
    /// # Errors
    ///
    /// Same as `decode()`.
    ///
    /// # Panics
    /// Never panics.
    pub fn decode_with_buffer(
        reader: &mut impl Read,
        payload_buf: &mut BytesMut,
    ) -> Result<Self, WebSocketError> {
        // Read the first header byte using `read` (not `read_exact`) to preserve
        // WouldBlock/TimedOut errors.
        let mut header = [0u8; 2];
        let first_byte_read = match reader.read(&mut header[..1]) {
            Ok(0) => {
                tracing::debug!(
                    "Received Ok(0) — no data available yet on TCP stream, signal retry"
                );
                return Err(WebSocketError::IoError(std::io::Error::new(
                    std::io::ErrorKind::WouldBlock,
                    "zero bytes read from stream, retry later",
                )));
            }
            Ok(n) => {
                tracing::debug!(
                    "Read first header byte: {:#04x} (read {} bytes)",
                    header[0],
                    n
                );
                true
            }
            Err(e) => {
                tracing::debug!("Error occurred reading data into buf: {:?}", e);
                return Err(WebSocketError::IoError(e));
            }
        };
        debug_assert!(first_byte_read);

        // Helper macro for partial read errors
        let map_partial_read_err = |e: WebSocketError| -> WebSocketError {
            match &e {
                WebSocketError::IoError(io_err)
                    if io_err.kind() == std::io::ErrorKind::WouldBlock
                        || io_err.kind() == std::io::ErrorKind::TimedOut =>
                {
                    tracing::error!(
                        "I/O error during partial frame read (stream corrupted): {:?}",
                        io_err
                    );
                    WebSocketError::ProtocolError(format!(
                        "Partial frame read interrupted by I/O error (stream corrupted): {}",
                        io_err
                    ))
                }
                _ => e,
            }
        };

        // Read the second header byte
        reader
            .read_exact(&mut header[1..2])
            .map_err(|e| map_partial_read_err(e.into()))?;
        tracing::debug!(
            "Read second header byte: {:#04x}, full header: [{:#04x}, {:#04x}]",
            header[1],
            header[0],
            header[1]
        );

        let fin = (header[0] & 0x80) != 0;
        let opcode = Opcode::from_byte(header[0] & 0x0F)?;
        let masked = (header[1] & 0x80) != 0;
        let length_byte = header[1] & 0x7F;

        // Determine payload length
        let payload_len: usize = match length_byte {
            126 => {
                let mut buf = [0u8; 2];
                reader
                    .read_exact(&mut buf)
                    .map_err(|e| map_partial_read_err(e.into()))?;
                u16::from_be_bytes(buf) as usize
            }
            127 => {
                let mut buf = [0u8; 8];
                reader
                    .read_exact(&mut buf)
                    .map_err(|e| map_partial_read_err(e.into()))?;
                #[allow(clippy::cast_possible_truncation)]
                let len = u64::from_be_bytes(buf) as usize;
                len
            }
            n => n as usize,
        };

        // Read masking key if present
        let mask = if masked {
            let mut key = [0u8; 4];
            reader
                .read_exact(&mut key)
                .map_err(|e| map_partial_read_err(e.into()))?;
            Some(key)
        } else {
            None
        };

        // Read payload into provided buffer
        payload_buf.clear();
        payload_buf.resize(payload_len, 0);
        reader
            .read_exact(&mut payload_buf[..])
            .map_err(|e| map_partial_read_err(e.into()))?;

        // Unmask payload if masked
        if let Some(mask_key) = mask {
            for (i, byte) in payload_buf.iter_mut().enumerate() {
                *byte ^= mask_key[i % 4];
            }
        }

        Ok(WebSocketFrame {
            fin,
            opcode,
            mask,
            payload: payload_buf.to_vec(),
        })
    }
}
