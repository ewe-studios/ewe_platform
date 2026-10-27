//! The plain-HTTP → HTTPS redirect answer and its head parsing (moved from the
//! inline test module of `src/server.rs`).

use foundation_proxy::server::{extract_header, extract_path, ssl_redirect_response};

#[test]
fn extract_header_finds_host() {
    let head = "GET /some/path HTTP/1.1\r\nHost: example.com:443\r\nConnection: close\r\n\r\n";
    assert_eq!(extract_header(head, "host:"), Some("example.com:443"));
}

#[test]
fn extract_header_case_insensitive() {
    let head = "GET / HTTP/1.1\r\nHOST: myapp.local\r\n\r\n";
    assert_eq!(extract_header(head, "host:"), Some("myapp.local"));
}

#[test]
fn extract_header_absent_returns_none() {
    let head = "GET / HTTP/1.1\r\nConnection: close\r\n\r\n";
    assert_eq!(extract_header(head, "host:"), None);
}

#[test]
fn extract_header_strips_whitespace() {
    let head = "GET / HTTP/1.1\r\nHost:   padded.example.com  \r\n\r\n";
    assert_eq!(extract_header(head, "host:"), Some("padded.example.com"));
}

#[test]
fn extract_path_from_get() {
    assert_eq!(extract_path("GET /api/health HTTP/1.1\r\nHost: x\r\n\r\n"), "/api/health");
}

#[test]
fn extract_path_root() {
    assert_eq!(extract_path("GET / HTTP/1.1\r\n\r\n"), "/");
}

#[test]
fn extract_path_fallback() {
    assert_eq!(extract_path(""), "/");
}

#[test]
fn redirect_response_points_at_https_with_the_same_host_and_path() {
    let redirect = ssl_redirect_response("GET /login HTTP/1.1\r\nHost: example.com\r\n\r\n");
    assert!(redirect.starts_with("HTTP/1.1 301"));
    assert!(redirect.contains("Location: https://example.com/login\r\n"));
    assert!(redirect.contains("Connection: close"));
    assert!(redirect.ends_with("\r\n\r\n"));
}

#[test]
fn redirect_response_without_host_uses_localhost() {
    let redirect = ssl_redirect_response("GET / HTTP/1.1\r\n\r\n");
    assert!(redirect.contains("Location: https://localhost/\r\n"));
}
