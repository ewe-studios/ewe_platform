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
}

/// `PR ` is not the preface (`PRI `), but `PR` *is* a syntactically valid method
/// token and the space ends it, so this opens like a request line and belongs to
/// the HTTP/1.x parser. Waiting for more bytes would be wrong — no continuation
/// can turn it back into h2c — and so would refusing it, since `PR` is a legal
/// (if unregistered) method.
///
/// It is then the HTTP/1.x version gate that rejects it, because the version
/// token `HTTP/` is neither HTTP/1.0 nor HTTP/1.1. Detection routes; the gate
/// judges. Conflating the two is what let unknown versions through before.
#[test]
fn detect_preface_lookalike_is_a_request_line_not_a_stall() {
    assert_eq!(detect_protocol(b"PR * HTTP/"), DetectedProtocol::Http11);
    assert_eq!(detect_protocol(b"PR "), DetectedProtocol::Http11);
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

/// A lone `G` diverges from the preface, so it is not h2c — but it is also not
/// yet a request line: `G` may still be growing into `GET `. Divergence from the
/// preface answers "not h2c", never "therefore HTTP/1.1".
#[test]
fn detect_diverging_byte_alone_is_not_yet_a_request_line() {
    assert_eq!(detect_protocol(b"G"), DetectedProtocol::NeedMore);
    assert_eq!(detect_protocol(b"GET"), DetectedProtocol::NeedMore);
    assert_eq!(detect_protocol(b"GET "), DetectedProtocol::Http11);
}

/// Bytes that cannot be an HTTP request line are refused rather than handed to
/// the HTTP/1.1 parser. Each of these previously classified as `Http11` purely
/// because it was not the h2c preface.
#[test]
fn detect_refuses_non_http_protocols() {
    // TLS ClientHello: record type 0x16, version 0x0301. `0x16` is not a tchar.
    assert_eq!(
        detect_protocol(&[0x16, 0x03, 0x01, 0x00, 0xff]),
        DetectedProtocol::Unsupported,
        "a TLS handshake must not be parsed as HTTP/1.1"
    );

    // SSH banner: every byte is a tchar, but no space ever arrives.
    assert_eq!(
        detect_protocol(b"SSH-2.0-OpenSSH_9.6\r\n"),
        DetectedProtocol::Unsupported,
        "an SSH banner must not be parsed as HTTP/1.1"
    );

    // A method token longer than any real method, still unterminated.
    assert_eq!(
        detect_protocol(b"AAAAAAAAAAAAAAAAAAAAAAAA"),
        DetectedProtocol::Unsupported
    );

    // A leading space: there is no method at all.
    assert_eq!(
        detect_protocol(b" GET / HTTP/1.1"),
        DetectedProtocol::Unsupported
    );

    // Raw binary noise.
    assert_eq!(
        detect_protocol(&[0x00, 0x01, 0x02, 0x03]),
        DetectedProtocol::Unsupported
    );
}

/// A method may sit right at the length limit and still be accepted, and the
/// byte past it is refused. Pins the boundary rather than assuming it.
#[test]
fn detect_method_length_boundary() {
    let at_limit = b"AAAAAAAAAAAAAAAA "; // 16 tchars, then a space
    assert_eq!(detect_protocol(at_limit), DetectedProtocol::Http11);

    let past_limit = b"AAAAAAAAAAAAAAAAA "; // 17 tchars before the space
    assert_eq!(detect_protocol(past_limit), DetectedProtocol::Unsupported);
}

/// Real methods, including the long WebDAV ones, must survive the guard.
#[test]
fn detect_accepts_registered_and_webdav_methods() {
    for method in [
        "GET ",
        "PUT ",
        "HEAD ",
        "POST ",
        "PATCH ",
        "TRACE ",
        "DELETE ",
        "CONNECT ",
        "OPTIONS ",
        "PROPFIND ",
        "PROPPATCH ",
        "UNSUBSCRIBE ",
    ] {
        assert_eq!(
            detect_protocol(method.as_bytes()),
            DetectedProtocol::Http11,
            "{method:?} is a valid request-line opening"
        );
    }
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
