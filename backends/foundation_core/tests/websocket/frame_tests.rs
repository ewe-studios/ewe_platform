#![cfg(test)]

//! WebSocket frame encoding/decoding tests (RFC 6455).

use foundation_core::wire::websocket::{Opcode, WebSocketError, WebSocketFrame};
use std::io::Cursor;

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
#[allow(clippy::cast_possible_truncation)]
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
    let mut cursor = Cursor::new(encoded);
    let decoded = WebSocketFrame::decode(&mut cursor).unwrap();
    assert!(decoded.fin);
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

    let mut cursor = Cursor::new(encoded);
    let decoded = WebSocketFrame::decode(&mut cursor).unwrap();
    assert!(decoded.fin);
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

    let mut cursor = Cursor::new(encoded);
    let decoded = WebSocketFrame::decode(&mut cursor).unwrap();
    assert!(decoded.fin);
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
    let mut cursor = Cursor::new(encoded);
    let decoded = WebSocketFrame::decode(&mut cursor).unwrap();
    assert!(decoded.fin);
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
    foundation_core::wire::websocket::apply_mask(&mut data, mask);
    assert_ne!(data, original, "masked data should differ from original");

    // Unmask (same operation)
    foundation_core::wire::websocket::apply_mask(&mut data, mask);
    assert_eq!(data, original, "double-masking should restore original");
}

#[test]
fn test_apply_mask_empty_payload() {
    let mask = [0xFF, 0xFF, 0xFF, 0xFF];
    let mut data: Vec<u8> = vec![];
    foundation_core::wire::websocket::apply_mask(&mut data, mask);
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
    let mut cursor = Cursor::new(encoded);
    let decoded = WebSocketFrame::decode(&mut cursor).unwrap();

    let decoded_text = String::from_utf8(decoded.payload).expect("payload should be valid UTF-8");
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
    let mut cursor = Cursor::new(encoded);
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
    let mut cursor = Cursor::new(encoded);
    let decoded = WebSocketFrame::decode(&mut cursor).unwrap();
    assert_eq!(decoded.opcode, Opcode::Close);
    assert!(decoded.payload.is_empty());
}
