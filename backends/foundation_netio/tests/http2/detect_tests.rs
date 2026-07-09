//! Tests for `http2::detect` — protocol detection from peeked bytes
//! (h2c prior-knowledge vs HTTP/1.1), Feature 30.

use foundation_netio::http2::connection::CLIENT_PREFACE;
use foundation_netio::http2::detect::*;

#[test]
fn detects_valid_preface() {
    assert!(is_h2c_preface(CLIENT_PREFACE));
}

#[test]
fn rejects_http11() {
    assert!(!is_h2c_preface(
        b"GET / HTTP/1.1\r\nHost: example.com\r\n\r\n"
    ));
}

#[test]
fn rejects_short_buffer() {
    assert!(!is_h2c_preface(b"PRI * HTT"));
}

#[test]
fn detect_returns_correct_variant() {
    assert_eq!(detect_protocol(CLIENT_PREFACE), DetectedProtocol::H2);
    assert_eq!(
        detect_protocol(b"GET / HTTP/1.1\r\nHost: local"),
        DetectedProtocol::Http11
    );
    assert_eq!(detect_protocol(b"PR"), DetectedProtocol::NeedMore);
    assert_eq!(detect_protocol(b"PR * HTTP/"), DetectedProtocol::NeedMore);
}

/// Nothing buffered yet is a viable (empty) prefix — the peer has not spoken.
#[test]
fn detect_empty_buffer_needs_more() {
    assert_eq!(detect_protocol(b""), DetectedProtocol::NeedMore);
}

/// A partial preface is still viable and must not be misread as HTTP/1.1, even
/// though it is far shorter than the full 24 bytes.
#[test]
fn detect_partial_preface_needs_more() {
    for n in 1..CLIENT_PREFACE.len() {
        assert_eq!(
            detect_protocol(&CLIENT_PREFACE[..n]),
            DetectedProtocol::NeedMore,
            "{n}-byte preface prefix is still viable"
        );
    }
}

/// A complete HTTP/1.1 request can be shorter than the 24-byte preface. It must
/// be decided immediately, not parked until the caller's detection timeout.
#[test]
fn detect_decides_short_http11_request_without_full_peek() {
    let req = b"GET / HTTP/1.1\r\n\r\n";
    assert!(
        req.len() < CLIENT_PREFACE.len(),
        "fixture must be under 24 bytes"
    );
    assert_eq!(detect_protocol(req), DetectedProtocol::Http11);
}

/// One byte is enough when it already diverges: `G` is not `P`.
#[test]
fn detect_decides_on_first_diverging_byte() {
    assert_eq!(detect_protocol(b"G"), DetectedProtocol::Http11);
}

/// Divergence in the preface's *tail* is caught too — a buffer that shares 23
/// bytes with the preface but differs at the last is not h2c.
#[test]
fn detect_rejects_preface_with_corrupted_tail() {
    let mut almost = CLIENT_PREFACE.to_vec();
    let last = almost.len() - 1;
    almost[last] = b'X';
    assert_eq!(detect_protocol(&almost), DetectedProtocol::Http11);
}

/// Bytes beyond the preface (the client's first frames arriving in the same
/// segment) do not disturb the h2 verdict.
#[test]
fn detect_preface_with_trailing_frame_bytes_is_h2() {
    let mut buf = CLIENT_PREFACE.to_vec();
    buf.extend_from_slice(&[0x00, 0x00, 0x00, 0x04, 0x00]);
    assert_eq!(detect_protocol(&buf), DetectedProtocol::H2);
}
