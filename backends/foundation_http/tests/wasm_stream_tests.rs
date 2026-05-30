//! Requires `--features wasm-test` on native or wasm32 target.

#![cfg(feature = "wasm-test")]

//! Tests for WasmStream — memory-backed HTTP wire format collector and parser.

use foundation_http::wasm::stream::{WasmStream, parse_raw};
use std::io::Write;

#[test]
fn test_wasm_stream_new() {
    let stream = WasmStream::new();
    let response = stream.into_wasm_response();
    assert_eq!(response.status, 200);
    assert_eq!(response.body, Some(Vec::new()));
    assert!(response.headers.is_empty());
}

#[test]
fn test_wasm_stream_default() {
    let stream = WasmStream::default();
    let response = stream.into_wasm_response();
    assert_eq!(response.status, 200);
}

#[test]
fn test_wasm_stream_write_returns_len() {
    let mut stream = WasmStream::new();
    let n = stream.write(b"test").unwrap();
    assert_eq!(n, 4);
}

#[test]
fn test_wasm_stream_write_all() {
    let mut stream = WasmStream::new();
    stream.write_all(b"HTTP/1.1 200 OK\r\n\r\nhello").unwrap();
    let response = stream.into_wasm_response();
    assert_eq!(response.status, 200);
    assert_eq!(response.body, Some(b"hello".to_vec()));
}

#[test]
fn test_wasm_stream_parse_200() {
    let mut stream = WasmStream::new();
    stream.write_all(b"HTTP/1.1 200 OK\r\n\r\n").unwrap();
    let response = stream.into_wasm_response();
    assert_eq!(response.status, 200);
}

#[test]
fn test_wasm_stream_parse_404() {
    let mut stream = WasmStream::new();
    stream.write_all(b"HTTP/1.1 404 Not Found\r\n\r\n").unwrap();
    let response = stream.into_wasm_response();
    assert_eq!(response.status, 404);
}

#[test]
fn test_wasm_stream_parse_201() {
    let mut stream = WasmStream::new();
    stream.write_all(b"HTTP/1.1 201 Created\r\n\r\n").unwrap();
    let response = stream.into_wasm_response();
    assert_eq!(response.status, 201);
}

#[test]
fn test_wasm_stream_parse_500() {
    let mut stream = WasmStream::new();
    stream.write_all(b"HTTP/1.1 500 Internal Server Error\r\n\r\nerror").unwrap();
    let response = stream.into_wasm_response();
    assert_eq!(response.status, 500);
    assert_eq!(response.body, Some(b"error".to_vec()));
}

#[test]
fn test_wasm_stream_parse_301_with_location() {
    let mut stream = WasmStream::new();
    stream.write_all(b"HTTP/1.1 301 Moved Permanently\r\nLocation: /new\r\n\r\n").unwrap();
    let response = stream.into_wasm_response();
    assert_eq!(response.status, 301);
    assert_eq!(response.headers.len(), 1);
    assert_eq!(response.headers[0], ("Location".to_string(), "/new".to_string()));
}

#[test]
fn test_wasm_stream_full_response() {
    let mut stream = WasmStream::new();
    stream.write_all(b"HTTP/1.1 200 OK\r\nContent-Type: text/plain\r\nContent-Length: 5\r\n\r\nhello").unwrap();
    let response = stream.into_wasm_response();
    assert_eq!(response.status, 200);
    assert_eq!(response.body, Some(b"hello".to_vec()));
    assert_eq!(response.headers.len(), 2);
    assert!(response.headers.iter().any(|(k, v)| k == "Content-Type" && v == "text/plain"));
    assert!(response.headers.iter().any(|(k, v)| k == "Content-Length" && v == "5"));
}

#[test]
fn test_wasm_stream_json_response() {
    let mut stream = WasmStream::new();
    stream.write_all(b"HTTP/1.1 200 OK\r\nContent-Type: application/json\r\n\r\n{\"ok\":true}").unwrap();
    let response = stream.into_wasm_response();
    assert_eq!(response.status, 200);
    assert_eq!(response.body, Some(br#"{"ok":true}"#.to_vec()));
    assert_eq!(response.headers.len(), 1);
    assert_eq!(response.headers[0], ("Content-Type".to_string(), "application/json".to_string()));
}

#[test]
fn test_wasm_stream_multiple_set_cookie() {
    let mut stream = WasmStream::new();
    stream.write_all(b"HTTP/1.1 200 OK\r\nSet-Cookie: a=1\r\nSet-Cookie: b=2\r\n\r\n").unwrap();
    let response = stream.into_wasm_response();
    assert_eq!(response.status, 200);
    assert_eq!(response.headers.len(), 2);
    assert_eq!(response.headers[0], ("Set-Cookie".to_string(), "a=1".to_string()));
    assert_eq!(response.headers[1], ("Set-Cookie".to_string(), "b=2".to_string()));
}

#[test]
fn test_wasm_stream_204_no_body() {
    let mut stream = WasmStream::new();
    stream.write_all(b"HTTP/1.1 204 No Content\r\n\r\n").unwrap();
    let response = stream.into_wasm_response();
    assert_eq!(response.status, 204);
    assert_eq!(response.body, Some(Vec::new()));
}

#[test]
fn test_wasm_stream_malformed_defaults_to_200() {
    let mut stream = WasmStream::new();
    stream.write_all(b"GARBAGE\r\n\r\nbody").unwrap();
    let response = stream.into_wasm_response();
    assert_eq!(response.status, 200);
    assert_eq!(response.body, Some(b"body".to_vec()));
}

#[test]
fn test_wasm_stream_flush_triggers_parse() {
    let mut stream = WasmStream::new();
    stream.write_all(b"HTTP/1.1 201 Created\r\n\r\n").unwrap();
    stream.flush().unwrap();
    let response = stream.into_wasm_response();
    assert_eq!(response.status, 201);
}

#[test]
fn test_wasm_stream_multiple_writes() {
    let mut stream = WasmStream::new();
    stream.write(b"HTTP/1.1 ").unwrap();
    stream.write(b"200 OK\r\n").unwrap();
    stream.write(b"\r\n").unwrap();
    stream.write(b"done").unwrap();
    let response = stream.into_wasm_response();
    assert_eq!(response.status, 200);
    assert_eq!(response.body, Some(b"done".to_vec()));
}

#[test]
fn test_wasm_stream_into_bytes() {
    let mut stream = WasmStream::new();
    stream.write_all(b"raw bytes").unwrap();
    let bytes = stream.into_bytes();
    assert_eq!(bytes, b"raw bytes");
}

#[test]
fn test_wasm_stream_body_with_crlf_in_middle() {
    let mut stream = WasmStream::new();
    stream.write_all(b"HTTP/1.1 200 OK\r\n\r\nline1\r\n\r\nline2").unwrap();
    let response = stream.into_wasm_response();
    assert_eq!(response.status, 200);
    assert_eq!(response.body, Some(b"line1\r\n\r\nline2".to_vec()));
}

#[test]
fn test_wasm_stream_empty_buffer() {
    let stream = WasmStream::new();
    let response = stream.into_wasm_response();
    assert_eq!(response.status, 200);
    assert_eq!(response.body, Some(Vec::new()));
}

#[test]
fn test_wasm_stream_header_with_extra_space() {
    let mut stream = WasmStream::new();
    stream.write_all(b"HTTP/1.1 200 OK\r\nX-Custom: value\r\n\r\n").unwrap();
    let response = stream.into_wasm_response();
    assert_eq!(response.headers.len(), 1);
    assert_eq!(response.headers[0], ("X-Custom".to_string(), "value".to_string()));
}

#[test]
fn test_wasm_stream_header_without_space_after_colon() {
    let mut stream = WasmStream::new();
    stream.write_all(b"HTTP/1.1 200 OK\r\nBadHeader:value\r\n\r\n").unwrap();
    let response = stream.into_wasm_response();
    assert_eq!(response.status, 200);
    assert_eq!(response.headers.len(), 0);
}

#[test]
fn test_parse_raw_standalone() {
    let parsed = parse_raw(b"HTTP/1.1 418 I'm a teapot\r\nX-Teapot: yes\r\n\r\nbrewing");
    assert_eq!(parsed.status, 418);
    assert_eq!(parsed.headers.len(), 1);
    assert_eq!(parsed.headers[0], ("X-Teapot".to_string(), "yes".to_string()));
    assert_eq!(parsed.body, b"brewing");
}
