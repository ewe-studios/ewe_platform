//! Tests for `http2::detect` — protocol detection from peeked bytes
//! (h2c prior-knowledge vs HTTP/1.1), Feature 30.

use foundation_netio::http2::detect::*;
use foundation_netio::http2::connection::CLIENT_PREFACE;

#[test]
fn detects_valid_preface() {
    assert!(is_h2c_preface(CLIENT_PREFACE));
}

#[test]
fn rejects_http11() {
    assert!(!is_h2c_preface(b"GET / HTTP/1.1\r\nHost: example.com\r\n\r\n"));
}

#[test]
fn rejects_short_buffer() {
    assert!(!is_h2c_preface(b"PRI * HTT"));
}

#[test]
fn detect_returns_correct_variant() {
    assert_eq!(detect_protocol(CLIENT_PREFACE), DetectedProtocol::H2);
    assert_eq!(detect_protocol(b"GET / HTTP/1.1\r\nHost: local"), DetectedProtocol::Http11);
    assert_eq!(detect_protocol(b"PR"), DetectedProtocol::NeedMore);
}
