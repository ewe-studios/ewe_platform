//! Requires `--features wasm-test` on native or wasm32 target.

#![cfg(feature = "wasm-test")]

//! Tests for WasmResponse struct.
//! These run natively with `--features wasm-test`.

use foundation_http::wasm::response::WasmResponse;

#[test]
fn test_wasm_response_default() {
    let resp = WasmResponse::default();
    assert_eq!(resp.status, 0);
    assert!(resp.body.is_none());
    assert!(resp.headers.is_empty());
}

#[test]
fn test_wasm_response_new() {
    let resp = WasmResponse::new(201);
    assert_eq!(resp.status, 201);
    assert!(resp.body.is_none());
    assert!(resp.headers.is_empty());
}

#[test]
fn test_wasm_response_with_body() {
    let resp = WasmResponse::new(200).with_body(b"hello");
    assert_eq!(resp.status, 200);
    assert_eq!(resp.body, Some(b"hello".to_vec()));
}

#[test]
fn test_wasm_response_with_body_binary() {
    let data: Vec<u8> = vec![0x00, 0xFF, 0x80, 0x01];
    let resp = WasmResponse::new(200).with_body(data.clone());
    assert_eq!(resp.body, Some(data));
}

#[test]
fn test_wasm_response_with_single_header() {
    let resp = WasmResponse::new(200)
        .with_header("Content-Type", "application/json");
    assert_eq!(resp.headers.len(), 1);
    assert_eq!(resp.headers[0], ("Content-Type".into(), "application/json".into()));
}

#[test]
fn test_wasm_response_chained() {
    let resp = WasmResponse::new(201)
        .with_body(br#"{"id":1}"#)
        .with_header("Content-Type", "application/json")
        .with_header("X-Request-Id", "abc-123");
    assert_eq!(resp.status, 201);
    assert_eq!(resp.body, Some(br#"{"id":1}"#.to_vec()));
    assert_eq!(resp.headers.len(), 2);
}

#[test]
fn test_wasm_response_various_statuses() {
    let statuses = [200, 201, 204, 301, 302, 400, 401, 403, 404, 500, 502, 503];
    for status in statuses {
        let resp = WasmResponse::new(status);
        assert_eq!(resp.status, status);
    }
}

#[test]
fn test_wasm_response_empty_body() {
    let resp = WasmResponse::new(204).with_body(b"");
    assert_eq!(resp.body, Some(Vec::new()));
}

#[test]
fn test_wasm_response_multiple_same_header_name() {
    let resp = WasmResponse::new(200)
        .with_header("Set-Cookie", "a=1")
        .with_header("Set-Cookie", "b=2");
    assert_eq!(resp.headers.len(), 2);
    assert_eq!(resp.headers[0], ("Set-Cookie".into(), "a=1".into()));
    assert_eq!(resp.headers[1], ("Set-Cookie".into(), "b=2".into()));
}
