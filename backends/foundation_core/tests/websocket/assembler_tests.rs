#![cfg(test)]

//! WebSocket `MessageAssembler` tests (RFC 6455 Section 4.5 - Fragmentation).

use foundation_core::wire::websocket::{
    MessageAssembler, Opcode, WebSocketError, WebSocketFrame, WebSocketMessage,
};

/// Helper to create a text frame with given payload and FIN bit.
fn text_frame(payload: &[u8], fin: bool) -> WebSocketFrame {
    WebSocketFrame {
        fin,
        opcode: Opcode::Text,
        mask: None,
        payload: payload.to_vec(),
    }
}

/// Helper to create a binary frame with given payload and FIN bit.
fn binary_frame(payload: &[u8], fin: bool) -> WebSocketFrame {
    WebSocketFrame {
        fin,
        opcode: Opcode::Binary,
        mask: None,
        payload: payload.to_vec(),
    }
}

/// Helper to create a continuation frame with given payload and FIN bit.
fn continuation_frame(payload: &[u8], fin: bool) -> WebSocketFrame {
    WebSocketFrame {
        fin,
        opcode: Opcode::Continuation,
        mask: None,
        payload: payload.to_vec(),
    }
}

/// Helper to create a ping frame.
fn ping_frame(payload: &[u8]) -> WebSocketFrame {
    WebSocketFrame {
        fin: true,
        opcode: Opcode::Ping,
        mask: None,
        payload: payload.to_vec(),
    }
}

#[test]
fn test_unfragmented_text_message() {
    let mut assembler = MessageAssembler::default();
    let frame = text_frame(b"hello world", true);
    let result = assembler.process_frame(frame).unwrap();

    assert!(result.is_some());
    let msg = result.unwrap();
    assert!(matches!(msg, WebSocketMessage::Text(s) if s == "hello world"));
}

#[test]
fn test_unfragmented_binary_message() {
    let mut assembler = MessageAssembler::default();
    let frame = binary_frame(&[1, 2, 3, 4], true);
    let result = assembler.process_frame(frame).unwrap();

    assert!(result.is_some());
    let msg = result.unwrap();
    assert!(matches!(msg, WebSocketMessage::Binary(d) if d == vec![1, 2, 3, 4]));
}

#[test]
fn test_fragmented_text_message() {
    let mut assembler = MessageAssembler::default();

    // First fragment
    let frame1 = text_frame(b"hello ", false);
    let result1 = assembler.process_frame(frame1).unwrap();
    assert!(result1.is_none());
    assert!(assembler.is_assembling());

    // Second fragment
    let frame2 = continuation_frame(b"world", true);
    let result2 = assembler.process_frame(frame2).unwrap();

    assert!(result2.is_some());
    let msg = result2.unwrap();
    assert!(matches!(msg, WebSocketMessage::Text(s) if s == "hello world"));
    assert!(!assembler.is_assembling());
}

#[test]
fn test_fragmented_binary_message() {
    let mut assembler = MessageAssembler::default();

    // First fragment
    let frame1 = binary_frame(&[1, 2], false);
    let _ = assembler.process_frame(frame1).unwrap();

    // Second fragment
    let frame2 = continuation_frame(&[3, 4], true);
    let result = assembler.process_frame(frame2).unwrap();

    assert!(result.is_some());
    let msg = result.unwrap();
    assert!(matches!(msg, WebSocketMessage::Binary(d) if d == vec![1, 2, 3, 4]));
}

#[test]
fn test_three_fragment_message() {
    let mut assembler = MessageAssembler::default();

    let frame1 = text_frame(b"part1-", false);
    assert!(assembler.process_frame(frame1).unwrap().is_none());

    let frame2 = continuation_frame(b"part2-", false);
    assert!(assembler.process_frame(frame2).unwrap().is_none());

    let frame3 = continuation_frame(b"part3", true);
    let result = assembler.process_frame(frame3).unwrap();

    assert!(result.is_some());
    let msg = result.unwrap();
    assert!(matches!(msg, WebSocketMessage::Text(s) if s == "part1-part2-part3"));
}

#[test]
fn test_unexpected_continuation_frame() {
    let mut assembler = MessageAssembler::default();

    // Send continuation without starting message
    let frame = continuation_frame(b"test", true);
    let result = assembler.process_frame(frame);

    assert!(result.is_err());
    assert!(matches!(
        result.unwrap_err(),
        WebSocketError::ProtocolError(_)
    ));
}

#[test]
fn test_new_message_before_completion() {
    let mut assembler = MessageAssembler::default();

    // Start fragmented message
    let frame1 = text_frame(b"hello", false);
    let _ = assembler.process_frame(frame1).unwrap();

    // Try to start new message before completing
    let frame2 = text_frame(b"world", true);
    let result = assembler.process_frame(frame2);

    assert!(result.is_err());
    assert!(matches!(
        result.unwrap_err(),
        WebSocketError::ProtocolError(_)
    ));
}

#[test]
fn test_fragmented_control_frame_error() {
    let mut assembler = MessageAssembler::default();

    // Control frames must not be fragmented
    // Create a non-FIN ping frame directly
    let frame = WebSocketFrame {
        fin: false,
        opcode: Opcode::Ping,
        mask: None,
        payload: b"test".to_vec(),
    };

    let result = assembler.process_frame(frame);
    assert!(result.is_err());
    assert!(matches!(
        result.unwrap_err(),
        WebSocketError::ProtocolError(_)
    ));
}

#[test]
fn test_control_frame_passthrough() {
    let mut assembler = MessageAssembler::default();

    // Send a ping frame during (what would be) fragmentation
    // Control frames should be processed immediately
    let frame = ping_frame(b"ping data");
    let result = assembler.process_frame(frame).unwrap();

    assert!(result.is_some());
    assert!(matches!(result.unwrap(), WebSocketMessage::Ping(d) if d == b"ping data"));
}

#[test]
fn test_message_size_limit() {
    let mut assembler = MessageAssembler::new(100); // 100 byte limit

    // Send fragment that exceeds limit
    let large_payload = vec![b'a'; 101];
    let frame = text_frame(&large_payload, true);
    let result = assembler.process_frame(frame);

    assert!(result.is_err());
    assert!(matches!(
        result.unwrap_err(),
        WebSocketError::ProtocolError(_)
    ));
}

#[test]
fn test_fragmented_size_limit() {
    let mut assembler = MessageAssembler::new(100); // 100 byte limit

    // Send first fragment (50 bytes)
    let frame1 = text_frame(&[b'a'; 50], false);
    let _ = assembler.process_frame(frame1).unwrap();

    // Send second fragment that pushes over limit
    let frame2 = continuation_frame(&[b'b'; 51], true);
    let result = assembler.process_frame(frame2);

    assert!(result.is_err());
    assert!(matches!(
        result.unwrap_err(),
        WebSocketError::ProtocolError(_)
    ));
}

#[test]
fn test_invalid_utf8_in_fragment() {
    let mut assembler = MessageAssembler::default();

    // Send invalid UTF-8 in first fragment
    // 0xC0 is an invalid start byte in UTF-8
    let frame = text_frame(&[0xC0, 0x80], true);
    let result = assembler.process_frame(frame);

    assert!(result.is_err());
    assert!(matches!(
        result.unwrap_err(),
        WebSocketError::InvalidUtf8(_)
    ));
}

#[test]
fn test_split_utf8_sequence() {
    let mut assembler = MessageAssembler::default();

    // UTF-8 character '€' (U+20AC) is encoded as [0xE2, 0x82, 0xAC]
    // Split it across two fragments
    let frame1 = text_frame(&[0xE2, 0x82], false); // Incomplete sequence
    let result1 = assembler.process_frame(frame1).unwrap();
    assert!(result1.is_none());

    let frame2 = continuation_frame(&[0xAC], true); // Complete the sequence
    let result2 = assembler.process_frame(frame2).unwrap();

    assert!(result2.is_some());
    let msg = result2.unwrap();
    assert!(matches!(msg, WebSocketMessage::Text(s) if s == "€"));
}

#[test]
fn test_invalid_utf8_split_across_fragments() {
    let mut assembler = MessageAssembler::default();

    // Send incomplete UTF-8 sequence
    let frame1 = text_frame(&[0xE2, 0x82], false);
    let _ = assembler.process_frame(frame1).unwrap();

    // Send invalid continuation (0x00 is not a valid UTF-8 continuation)
    let frame2 = continuation_frame(&[0x00], true);
    let result = assembler.process_frame(frame2);

    assert!(result.is_err());
    assert!(matches!(
        result.unwrap_err(),
        WebSocketError::InvalidUtf8(_)
    ));
}

#[test]
fn test_reset_aborts_fragmentation() {
    let mut assembler = MessageAssembler::default();

    // Start fragmented message
    let frame1 = text_frame(b"hello", false);
    let _ = assembler.process_frame(frame1).unwrap();
    assert!(assembler.is_assembling());

    // Reset
    assembler.reset();
    assert!(!assembler.is_assembling());

    // Now can start fresh
    let frame2 = text_frame(b"world", true);
    let result = assembler.process_frame(frame2).unwrap();
    assert!(result.is_some());
}

#[test]
fn test_accumulated_size() {
    let mut assembler = MessageAssembler::default();

    assert_eq!(assembler.accumulated_size(), 0);

    let frame1 = text_frame(&[0; 100], false);
    let _ = assembler.process_frame(frame1).unwrap();
    assert_eq!(assembler.accumulated_size(), 100);

    let frame2 = continuation_frame(&[0; 50], false);
    let _ = assembler.process_frame(frame2).unwrap();
    assert_eq!(assembler.accumulated_size(), 150);

    let frame3 = continuation_frame(&[0; 50], true);
    let _ = assembler.process_frame(frame3).unwrap();
    assert_eq!(assembler.accumulated_size(), 0);
}
