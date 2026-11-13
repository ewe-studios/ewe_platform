//! `WebSocketEchoServer` tests moved out of `src/http/websocket_server.rs`
//! (spec 70 feature 59, rule 7).
//!
//! `compute_accept_key` was private; it is now public and re-exported from
//! `foundation_testing::http` so the RFC test vector stays covered.

use foundation_testing::http::{compute_accept_key, WebSocketEchoServer};

#[test]
fn test_server_start() {
    let server = WebSocketEchoServer::start();
    assert!(server.base_url().starts_with("ws://127.0.0.1:"));
}

#[test]
fn test_compute_accept_key_rfc_vector() {
    let client_key = "dGhlIHNhbXBsZSBub25jZQ==";
    let expected = "s3pPLMBiTxaQ9kYGzzhZRbK+xOo=";
    let result = compute_accept_key(client_key);
    assert_eq!(result, expected);
}
