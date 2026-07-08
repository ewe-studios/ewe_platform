//! Tests for `http2::connection` — H2Connection, StreamId, pseudo-header
//! mapping, connection preface (Feature 29/30).

use std::io::Cursor;
use bytes::Bytes;
use foundation_netio::http2::connection::*;

#[test]
fn client_preface_is_correct_length() {
    assert_eq!(CLIENT_PREFACE.len(), 24);
    assert_eq!(&CLIENT_PREFACE[0..3], b"PRI");
}

#[test]
fn stream_id_parity() {
    assert!(StreamId(1).is_client_initiated());
    assert!(StreamId(3).is_client_initiated());
    assert!(StreamId(2).is_server_initiated());
    assert!(!StreamId(0).is_client_initiated());
    assert!(!StreamId(0).is_server_initiated());
}

#[test]
fn h2_request_pseudo_header_mapping() {
    let conn = H2Connection::new(Cursor::new(Vec::new()), true);
    let headers: Vec<(Bytes, Bytes)> = vec![
        (Bytes::from_static(b":method"), Bytes::from_static(b"POST")),
        (Bytes::from_static(b":scheme"), Bytes::from_static(b"https")),
        (Bytes::from_static(b":authority"), Bytes::from_static(b"example.com")),
        (Bytes::from_static(b":path"), Bytes::from_static(b"/api/test")),
        (Bytes::from_static(b"content-type"), Bytes::from_static(b"application/json")),
    ];
    let req = conn.build_request(&headers, true);
    assert_eq!(&req.method[..], b"POST");
    assert_eq!(&req.scheme[..], b"https");
    assert_eq!(&req.authority[..], b"example.com");
    assert_eq!(&req.path[..], b"/api/test");
    assert_eq!(req.headers.len(), 1);
    assert_eq!(&req.headers[0].0[..], b"content-type");
}
