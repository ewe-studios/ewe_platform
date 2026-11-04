//! Cloudflare Workers connection tests (moved from `src/wasm/cf_conn.rs`).
#![cfg(feature = "wasm-bindgen-http")]

use foundation_http::wasm::cf_conn::CfConn;

#[test]
fn test_cf_conn_default() {
    let conn = CfConn::default();
    assert_eq!(conn.status, 200);
    assert!(conn.headers.is_empty());
    assert!(conn.body.is_none());
}

#[test]
fn test_cf_conn_set_status() {
    let mut conn = CfConn::new();
    conn.set_status(404);
    assert_eq!(conn.status, 404);
}

#[test]
fn test_cf_conn_set_header() {
    let mut conn = CfConn::new();
    conn.set_header("Content-Type", "application/json");
    assert_eq!(conn.headers.len(), 1);
    assert_eq!(
        conn.headers[0],
        ("Content-Type".into(), "application/json".into())
    );
}

#[test]
fn test_cf_conn_set_header_replaces() {
    let mut conn = CfConn::new();
    conn.set_header("Content-Type", "text/plain");
    conn.set_header("Content-Type", "application/json");
    assert_eq!(conn.headers.len(), 1);
    assert_eq!(
        conn.headers[0],
        ("Content-Type".into(), "application/json".into())
    );
}

#[test]
fn test_cf_conn_append_header() {
    let mut conn = CfConn::new();
    conn.append_header("Set-Cookie", "a=1");
    conn.append_header("Set-Cookie", "b=2");
    assert_eq!(conn.headers.len(), 2);
}

#[test]
fn test_cf_conn_set_body() {
    let mut conn = CfConn::new();
    conn.set_body(b"hello".to_vec());
    assert_eq!(conn.body, Some(b"hello".to_vec()));
}

#[test]
fn test_cf_conn_full_response() {
    let mut conn = CfConn::new();
    conn.set_status(201);
    conn.set_header("Content-Type", "application/json");
    conn.set_body(br#"{"id":1}"#.to_vec());
    assert_eq!(conn.status, 201);
    assert_eq!(conn.headers.len(), 1);
    assert_eq!(conn.body, Some(br#"{"id":1}"#.to_vec()));
}

#[test]
fn test_cf_conn_various_statuses() {
    let statuses = [200, 201, 204, 301, 400, 401, 403, 404, 500, 502, 503];
    for s in statuses {
        let mut conn = CfConn::new();
        conn.set_status(s);
        assert_eq!(conn.status, s);
    }
}
