#![cfg(test)]

//! WebSocket error tests.

use foundation_core::wire::websocket::WebSocketError;
use std::io;
use tracing_test::traced_test;

// Test 1: Display - UpgradeFailed
#[test]
#[traced_test]
fn test_error_display_upgrade_failed() {
    let err = WebSocketError::UpgradeFailed(404);
    let display = format!("{}", err);
    assert!(display.contains("upgrade failed"));
    assert!(display.contains("404"));
}

// Test 2: Display - InvalidAcceptKey
#[test]
#[traced_test]
fn test_error_display_invalid_accept_key() {
    let err = WebSocketError::InvalidAcceptKey;
    let display = format!("{}", err);
    assert!(display.contains("invalid Sec-WebSocket-Accept"));
}

// Test 3: Display - MissingAcceptKey
#[test]
#[traced_test]
fn test_error_display_missing_accept_key() {
    let err = WebSocketError::MissingAcceptKey;
    let display = format!("{}", err);
    assert!(display.contains("missing Sec-WebSocket-Accept"));
}

// Test 4: Display - MissingKey
#[test]
#[traced_test]
fn test_error_display_missing_key() {
    let err = WebSocketError::MissingKey;
    let display = format!("{}", err);
    assert!(display.contains("missing Sec-WebSocket-Key"));
}

// Test 5: Display - InvalidFrame
#[test]
#[traced_test]
fn test_error_display_invalid_frame() {
    let err = WebSocketError::InvalidFrame("bad opcode".to_string());
    let display = format!("{}", err);
    assert!(display.contains("invalid frame"));
    assert!(display.contains("bad opcode"));
}

// Test 6: Display - InvalidUtf8
#[test]
#[traced_test]
fn test_error_display_invalid_utf8() {
    let invalid_string = String::from_utf8(vec![0xFF, 0xFE]).unwrap_err();
    let err = WebSocketError::InvalidUtf8(invalid_string);
    let display = format!("{}", err);
    assert!(display.contains("invalid UTF-8"));
}

// Test 7: Display - ConnectionClosed
#[test]
#[traced_test]
fn test_error_display_connection_closed() {
    let err = WebSocketError::ConnectionClosed;
    let display = format!("{}", err);
    assert!(display.contains("connection closed"));
}

// Test 8: Display - ProtocolError
#[test]
#[traced_test]
fn test_error_display_protocol_error() {
    let err = WebSocketError::ProtocolError("masking violation".to_string());
    let display = format!("{}", err);
    assert!(display.contains("protocol error"));
    assert!(display.contains("masking violation"));
}

// Test 9: Display - IoError
#[test]
#[traced_test]
fn test_error_display_io_error() {
    let io_err = io::Error::new(io::ErrorKind::ConnectionReset, "connection reset");
    let err = WebSocketError::IoError(io_err);
    let display = format!("{}", err);
    assert!(display.contains("I/O error"));
    assert!(display.contains("connection reset"));
}

// Test 10: Display - InvalidUrl
#[test]
#[traced_test]
fn test_error_display_invalid_url() {
    let err = WebSocketError::InvalidUrl("not a valid url".to_string());
    let display = format!("{}", err);
    assert!(display.contains("invalid URL"));
    assert!(display.contains("not a valid url"));
}

// Test 11: From<io::Error>
#[test]
#[traced_test]
fn test_from_io_error() {
    let io_err = io::Error::new(io::ErrorKind::BrokenPipe, "broken pipe");
    let err: WebSocketError = io_err.into();
    assert!(matches!(err, WebSocketError::IoError(_)));
}

// Test 12: From<FromUtf8Error>
#[test]
#[traced_test]
fn test_from_utf8_error() {
    let utf8_err = String::from_utf8(vec![0xFF, 0xFE]).unwrap_err();
    let err: WebSocketError = utf8_err.into();
    assert!(matches!(err, WebSocketError::InvalidUtf8(_)));
}

// Test 13: Error trait
#[test]
#[traced_test]
fn test_error_trait() {
    let err = WebSocketError::ProtocolError("test".to_string());
    let std_err: &dyn std::error::Error = &err;
    // Just verifying it implements the trait - no panic means success
    let _ = std_err.to_string();
}

// Test 14: Debug output
#[test]
#[traced_test]
fn test_error_debug() {
    let err = WebSocketError::UpgradeFailed(500);
    let debug = format!("{:?}", err);
    assert!(debug.contains("UpgradeFailed"));
    assert!(debug.contains("500"));
}
