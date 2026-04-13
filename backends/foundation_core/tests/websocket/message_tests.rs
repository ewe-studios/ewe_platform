#![cfg(test)]

//! WebSocket message tests (RFC 6455).

use foundation_core::wire::websocket::{Opcode, WebSocketFrame, WebSocketMessage};
use tracing_test::traced_test;

// Test 1: to_message - Text frame conversion
#[test]
#[traced_test]
fn test_text_frame_to_message() {
    let text = "Hello, WebSocket!";
    let frame = WebSocketFrame {
        fin: true,
        opcode: Opcode::Text,
        mask: None,
        payload: text.as_bytes().to_vec(),
    };

    let message = frame.to_message().expect("should convert to message");
    assert!(matches!(message, WebSocketMessage::Text(_)));
    if let WebSocketMessage::Text(t) = message {
        assert_eq!(t, text);
    }
}

// Test 2: to_message - Binary frame conversion
#[test]
#[traced_test]
fn test_binary_frame_to_message() {
    let data = vec![0x01, 0x02, 0x03, 0xFF, 0xFE];
    let frame = WebSocketFrame {
        fin: true,
        opcode: Opcode::Binary,
        mask: None,
        payload: data.clone(),
    };

    let message = frame.to_message().expect("should convert to message");
    assert!(matches!(message, WebSocketMessage::Binary(_)));
    if let WebSocketMessage::Binary(d) = message {
        assert_eq!(d, data);
    }
}

// Test 3: to_message - Ping frame conversion
#[test]
#[traced_test]
fn test_ping_frame_to_message() {
    let ping_data = vec![0x01, 0x02, 0x03];
    let frame = WebSocketFrame {
        fin: true,
        opcode: Opcode::Ping,
        mask: None,
        payload: ping_data.clone(),
    };

    let message = frame.to_message().expect("should convert to message");
    assert!(matches!(message, WebSocketMessage::Ping(_)));
    if let WebSocketMessage::Ping(d) = message {
        assert_eq!(d, ping_data);
    }
}

// Test 4: to_message - Pong frame conversion
#[test]
#[traced_test]
fn test_pong_frame_to_message() {
    let pong_data = vec![0x04, 0x05, 0x06];
    let frame = WebSocketFrame {
        fin: true,
        opcode: Opcode::Pong,
        mask: None,
        payload: pong_data.clone(),
    };

    let message = frame.to_message().expect("should convert to message");
    assert!(matches!(message, WebSocketMessage::Pong(_)));
    if let WebSocketMessage::Pong(d) = message {
        assert_eq!(d, pong_data);
    }
}

// Test 5: to_message - Close frame with code and reason
#[test]
#[traced_test]
fn test_close_frame_to_message() {
    let code: u16 = 1000;
    let reason = "normal closure";
    let mut payload = code.to_be_bytes().to_vec();
    payload.extend_from_slice(reason.as_bytes());

    let frame = WebSocketFrame {
        fin: true,
        opcode: Opcode::Close,
        mask: None,
        payload,
    };

    let message = frame.to_message().expect("should convert to message");
    assert!(matches!(message, WebSocketMessage::Close(_, _)));
    if let WebSocketMessage::Close(c, r) = message {
        assert_eq!(c, code);
        assert_eq!(r, reason);
    }
}

// Test 6: to_message - Close frame with empty payload
#[test]
#[traced_test]
fn test_close_frame_empty_payload() {
    let frame = WebSocketFrame {
        fin: true,
        opcode: Opcode::Close,
        mask: None,
        payload: vec![],
    };

    let message = frame.to_message().expect("should convert to message");
    assert!(matches!(message, WebSocketMessage::Close(_, _)));
    if let WebSocketMessage::Close(c, r) = message {
        assert_eq!(c, 1005); // No status received
        assert_eq!(r, "");
    }
}

// Test 7: to_message - Invalid UTF-8 in text frame
#[test]
#[traced_test]
fn test_text_frame_invalid_utf8() {
    // Invalid UTF-8 sequence
    let invalid_utf8 = vec![0xFF, 0xFE, 0xFD];
    let frame = WebSocketFrame {
        fin: true,
        opcode: Opcode::Text,
        mask: None,
        payload: invalid_utf8,
    };

    let result = frame.to_message();
    assert!(result.is_err(), "should reject invalid UTF-8 in text frame");
}

// Test 8: to_message - Unexpected continuation frame
#[test]
#[traced_test]
fn test_unexpected_continuation_frame() {
    let frame = WebSocketFrame {
        fin: true,
        opcode: Opcode::Continuation,
        mask: None,
        payload: vec![0x01, 0x02],
    };

    let result = frame.to_message();
    assert!(
        result.is_err(),
        "should reject unexpected continuation frame"
    );
}

// Test 9: WebSocketMessage variant_name
#[test]
#[traced_test]
fn test_message_variant_name() {
    assert_eq!(
        WebSocketMessage::Text("test".to_string()).variant_name(),
        "text"
    );
    assert_eq!(
        WebSocketMessage::Binary(vec![1, 2, 3]).variant_name(),
        "binary"
    );
    assert_eq!(WebSocketMessage::Ping(vec![]).variant_name(), "ping");
    assert_eq!(WebSocketMessage::Pong(vec![]).variant_name(), "pong");
    assert_eq!(
        WebSocketMessage::Close(1000, "bye".to_string()).variant_name(),
        "close"
    );
    assert_eq!(
        WebSocketMessage::ConnectionEstablished.variant_name(),
        "connection_established"
    );
}

// Test 10: WebSocketMessage clone
#[test]
#[traced_test]
fn test_message_clone() {
    let original = WebSocketMessage::Text("test".to_string());
    let cloned = original.clone();
    assert_eq!(original, cloned);
}

// Test 11: Text frame with various UTF-8 characters
#[test]
#[traced_test]
fn test_text_frame_various_utf8() {
    let test_cases = vec![
        ("ASCII only", "Hello, world!"),
        ("Latin accented", "café résumé naïve"),
        ("Chinese", "你好世界"),
        ("Emoji", "😀😁😂"),
        ("Mixed", "Hello 世界 🌍 café"),
    ];

    for (name, text) in test_cases {
        let frame = WebSocketFrame {
            fin: true,
            opcode: Opcode::Text,
            mask: None,
            payload: text.as_bytes().to_vec(),
        };

        let message = frame
            .to_message()
            .unwrap_or_else(|_| panic!("should convert {name} to message"));
        if let WebSocketMessage::Text(t) = message {
            assert_eq!(t, text, "{name} should roundtrip correctly");
        } else {
            panic!("expected Text message for {name}");
        }
    }
}
