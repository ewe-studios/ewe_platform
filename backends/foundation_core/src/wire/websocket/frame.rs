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

#[cfg(test)]
mod tests {
    use super::*;

    // Test 1: Opcode encoding/decoding
    #[test]
    fn test_opcode_from_byte_all_valid() {
        assert_eq!(Opcode::from_byte(0x0).unwrap(), Opcode::Continuation);
        assert_eq!(Opcode::from_byte(0x1).unwrap(), Opcode::Text);
        assert_eq!(Opcode::from_byte(0x2).unwrap(), Opcode::Binary);
        assert_eq!(Opcode::from_byte(0x8).unwrap(), Opcode::Close);
        assert_eq!(Opcode::from_byte(0x9).unwrap(), Opcode::Ping);
        assert_eq!(Opcode::from_byte(0xA).unwrap(), Opcode::Pong);
    }

    #[test]
    fn test_opcode_from_byte_invalid() {
        assert!(Opcode::from_byte(0x3).is_err());
        assert!(Opcode::from_byte(0x7).is_err());
        assert!(Opcode::from_byte(0xF).is_err());
    }

    #[test]
    fn test_opcode_is_control() {
        assert!(!Opcode::Continuation.is_control());
        assert!(!Opcode::Text.is_control());
        assert!(!Opcode::Binary.is_control());
        assert!(Opcode::Close.is_control());
        assert!(Opcode::Ping.is_control());
        assert!(Opcode::Pong.is_control());
    }

    // Test 2: Small payload frame encode/decode (< 126 bytes)
    #[test]
    fn test_small_payload_unmasked_roundtrip() {
        let payload = b"Hello, WebSocket!".to_vec();
        let frame = WebSocketFrame {
            fin: true,
            opcode: Opcode::Text,
            mask: None,
            payload: payload.clone(),
        };

        let encoded = frame.encode();

        // Verify header bytes manually
        assert_eq!(encoded[0], 0x81); // FIN=1, opcode=0x1 (text)
        assert_eq!(encoded[1], payload.len() as u8); // no mask bit, length < 126

        // Decode and verify roundtrip
        let mut cursor = std::io::Cursor::new(encoded);
        let decoded = WebSocketFrame::decode(&mut cursor).unwrap();
        assert_eq!(decoded.fin, true);
        assert_eq!(decoded.opcode, Opcode::Text);
        assert_eq!(decoded.mask, None);
        assert_eq!(decoded.payload, payload);
    }

    // Test 3: Medium payload frame encode/decode (126-65535 bytes)
    #[test]
    fn test_medium_payload_roundtrip() {
        let payload = vec![0xAB; 300]; // 300 bytes, triggers 2-byte extended length
        let frame = WebSocketFrame {
            fin: true,
            opcode: Opcode::Binary,
            mask: None,
            payload: payload.clone(),
        };

        let encoded = frame.encode();

        // Verify header: FIN=1, opcode=0x2, length marker=126, then 2-byte BE length
        assert_eq!(encoded[0], 0x82); // FIN + Binary
        assert_eq!(encoded[1], 126); // extended 16-bit length marker
        assert_eq!(&encoded[2..4], &(300u16).to_be_bytes());

        let mut cursor = std::io::Cursor::new(encoded);
        let decoded = WebSocketFrame::decode(&mut cursor).unwrap();
        assert_eq!(decoded.fin, true);
        assert_eq!(decoded.opcode, Opcode::Binary);
        assert_eq!(decoded.payload, payload);
    }

    // Test 4: Large payload frame encode/decode (>= 65536 bytes)
    #[test]
    fn test_large_payload_roundtrip() {
        let payload = vec![0xCD; 70_000]; // triggers 8-byte extended length
        let frame = WebSocketFrame {
            fin: true,
            opcode: Opcode::Binary,
            mask: None,
            payload: payload.clone(),
        };

        let encoded = frame.encode();

        // Verify header
        assert_eq!(encoded[0], 0x82); // FIN + Binary
        assert_eq!(encoded[1], 127); // extended 64-bit length marker
        assert_eq!(&encoded[2..10], &(70_000u64).to_be_bytes());

        let mut cursor = std::io::Cursor::new(encoded);
        let decoded = WebSocketFrame::decode(&mut cursor).unwrap();
        assert_eq!(decoded.fin, true);
        assert_eq!(decoded.opcode, Opcode::Binary);
        assert_eq!(decoded.payload.len(), 70_000);
        assert_eq!(decoded.payload, payload);
    }

    // Test 5: Masked frame encode/decode
    #[test]
    fn test_masked_frame_roundtrip() {
        let mask_key = [0x37, 0xFA, 0x21, 0x3D];
        let payload = b"Hello".to_vec();
        let frame = WebSocketFrame {
            fin: true,
            opcode: Opcode::Text,
            mask: Some(mask_key),
            payload: payload.clone(),
        };

        let encoded = frame.encode();

        // Verify mask bit is set
        assert_ne!(encoded[1] & 0x80, 0, "mask bit should be set");

        // Verify the wire payload is actually masked (not plaintext)
        let header_len = 2 + 4; // 2 header + 4 mask key (payload < 126)
        let wire_payload = &encoded[header_len..];
        assert_ne!(wire_payload, &payload[..], "wire payload should be masked");

        // Decode and verify we get back the original plaintext
        let mut cursor = std::io::Cursor::new(encoded);
        let decoded = WebSocketFrame::decode(&mut cursor).unwrap();
        assert_eq!(decoded.fin, true);
        assert_eq!(decoded.opcode, Opcode::Text);
        assert_eq!(decoded.mask, Some(mask_key));
        assert_eq!(decoded.payload, payload);
    }

    // Test 6: Unmasked frame encode/decode
    #[test]
    fn test_unmasked_frame_payload_is_plaintext_on_wire() {
        let payload = b"No mask here".to_vec();
        let frame = WebSocketFrame {
            fin: true,
            opcode: Opcode::Text,
            mask: None,
            payload: payload.clone(),
        };

        let encoded = frame.encode();

        // Mask bit should NOT be set
        assert_eq!(encoded[1] & 0x80, 0, "mask bit should not be set");

        // Wire payload should be plaintext
        let header_len = 2; // no mask key
        assert_eq!(&encoded[header_len..], &payload[..]);
    }

    // Test 7: apply_mask function
    #[test]
    fn test_apply_mask_is_self_inverse() {
        let mask = [0x12, 0x34, 0x56, 0x78];
        let original = b"Test data for masking!".to_vec();
        let mut data = original.clone();

        // Mask
        apply_mask(&mut data, mask);
        assert_ne!(data, original, "masked data should differ from original");

        // Unmask (same operation)
        apply_mask(&mut data, mask);
        assert_eq!(data, original, "double-masking should restore original");
    }

    #[test]
    fn test_apply_mask_empty_payload() {
        let mask = [0xFF, 0xFF, 0xFF, 0xFF];
        let mut data: Vec<u8> = vec![];
        apply_mask(&mut data, mask);
        assert!(data.is_empty());
    }

    // Test 8: Control frame validation
    #[test]
    fn test_control_frame_must_have_fin() {
        let frame = WebSocketFrame {
            fin: false, // invalid for control frames
            opcode: Opcode::Ping,
            mask: None,
            payload: vec![],
        };
        let err = frame.validate().unwrap_err();
        assert!(
            matches!(err, WebSocketError::ProtocolError(_)),
            "expected ProtocolError, got: {err:?}"
        );
    }

    #[test]
    fn test_control_frame_payload_max_125() {
        let frame = WebSocketFrame {
            fin: true,
            opcode: Opcode::Close,
            mask: None,
            payload: vec![0u8; 126], // 1 byte over limit
        };
        let err = frame.validate().unwrap_err();
        assert!(matches!(err, WebSocketError::ProtocolError(_)));
    }

    #[test]
    fn test_control_frame_valid() {
        let frame = WebSocketFrame {
            fin: true,
            opcode: Opcode::Pong,
            mask: None,
            payload: vec![0u8; 125], // exactly at limit
        };
        assert!(frame.validate().is_ok());
    }

    #[test]
    fn test_data_frame_can_be_fragmented() {
        // Data frames are allowed to have FIN=false
        let frame = WebSocketFrame {
            fin: false,
            opcode: Opcode::Text,
            mask: None,
            payload: vec![0u8; 200],
        };
        assert!(frame.validate().is_ok());
    }

    // Test 9: Text frame with valid UTF-8
    #[test]
    fn test_text_frame_utf8_roundtrip() {
        let text = "Hello, world! \u{1F600} \u{00E9}\u{00F1}\u{00FC}"; // includes emoji and accented chars
        let frame = WebSocketFrame {
            fin: true,
            opcode: Opcode::Text,
            mask: None,
            payload: text.as_bytes().to_vec(),
        };

        let encoded = frame.encode();
        let mut cursor = std::io::Cursor::new(encoded);
        let decoded = WebSocketFrame::decode(&mut cursor).unwrap();

        let decoded_text =
            String::from_utf8(decoded.payload).expect("payload should be valid UTF-8");
        assert_eq!(decoded_text, text);
    }

    // Test 10: Close frame payload parsing (2-byte code + reason)
    #[test]
    fn test_close_frame_payload_parsing() {
        // Close frame payload: 2-byte big-endian status code + UTF-8 reason
        let status_code: u16 = 1000; // Normal Closure
        let reason = "goodbye";
        let mut close_payload = status_code.to_be_bytes().to_vec();
        close_payload.extend_from_slice(reason.as_bytes());

        let frame = WebSocketFrame {
            fin: true,
            opcode: Opcode::Close,
            mask: None,
            payload: close_payload,
        };
        assert!(frame.validate().is_ok());

        let encoded = frame.encode();
        let mut cursor = std::io::Cursor::new(encoded);
        let decoded = WebSocketFrame::decode(&mut cursor).unwrap();

        assert_eq!(decoded.opcode, Opcode::Close);
        assert!(decoded.payload.len() >= 2);

        // Parse the close payload
        let code = u16::from_be_bytes([decoded.payload[0], decoded.payload[1]]);
        let decoded_reason = String::from_utf8(decoded.payload[2..].to_vec())
            .expect("close reason should be valid UTF-8");

        assert_eq!(code, 1000);
        assert_eq!(decoded_reason, "goodbye");
    }

    #[test]
    fn test_close_frame_empty_payload() {
        // Close frame with no payload is valid (no status code)
        let frame = WebSocketFrame {
            fin: true,
            opcode: Opcode::Close,
            mask: None,
            payload: vec![],
        };
        assert!(frame.validate().is_ok());

        let encoded = frame.encode();
        let mut cursor = std::io::Cursor::new(encoded);
        let decoded = WebSocketFrame::decode(&mut cursor).unwrap();
        assert_eq!(decoded.opcode, Opcode::Close);
        assert!(decoded.payload.is_empty());
    }
}
