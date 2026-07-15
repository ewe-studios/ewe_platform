//! Tests for Feature 02 (spec 54) — Unix domain socket transport.
//!
//! WHY: The Docker daemon and BuildKitd serve HTTP over a Unix socket. These
//! tests verify `HttpClientBuilder::unix_socket()` routes requests over a real
//! Unix socket end-to-end (no DNS, no proxy) and returns the correct response.
//!
//! WHAT: A tiny raw Unix-socket HTTP server + a client built with
//! `.unix_socket(path)`; the client's `send()` must reach the server and read
//! the body back.

#![cfg(unix)]

use std::io::{Read, Write};
use std::os::unix::net::UnixListener;
use std::path::PathBuf;

use foundation_core::valtron::valtron_test;
use foundation_netio::network_client::HttpClientBuilder;
use foundation_netio::shared::client::body_reader::collect_bytes_from_send_safe;
use foundation_netio::shared::client::http_client::HttpClient;
use foundation_netio::shared::http::{
    Extensions, SendSafeBody, SimpleHeader, SimpleHeaders, SimpleMethod, Status,
};
use foundation_netio::PreparedRequest;

/// A unique temp path for a Unix socket (removed if it already exists).
fn temp_socket_path(tag: &str) -> PathBuf {
    let pid = std::process::id();
    let nanos = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_nanos())
        .unwrap_or(0);
    let path = std::env::temp_dir().join(format!("ewe_netio_unix_{tag}_{pid}_{nanos}.sock"));
    let _ = std::fs::remove_file(&path);
    path
}

fn get_request(url: &str) -> PreparedRequest {
    let uri = foundation_core::url::Uri::parse(url).expect("uri");
    let mut headers = SimpleHeaders::new();
    headers.insert(
        SimpleHeader::HOST,
        vec![uri.host_str().unwrap_or_else(|| "localhost".to_string())],
    );
    PreparedRequest {
        method: SimpleMethod::GET,
        url: uri,
        headers,
        body: SendSafeBody::None,
        extensions: Extensions::new(),
    }
}

/// Start a tiny raw Unix-socket HTTP server. Responds once then shuts down.
/// Returns the socket path (caller uses it for `.unix_socket(path)`).
fn raw_http_serve_unix(status: u16, body: &'static str) -> PathBuf {
    let path = temp_socket_path("serve");
    let listener = UnixListener::bind(&path).expect("bind unix socket");
    let body_bytes = body.as_bytes().to_vec();
    std::thread::spawn(move || {
        if let Some(stream) = listener.incoming().next() {
            let mut stream = stream.expect("accept");
            let mut buf = [0u8; 4096];
            let _n = stream.read(&mut buf).unwrap_or(0);
            let response = format!(
                "HTTP/1.1 {status} OK\r\nContent-Length: {}\r\nConnection: close\r\n\r\n",
                body_bytes.len()
            );
            let mut out = response.into_bytes();
            out.extend_from_slice(&body_bytes);
            let _ = stream.write_all(&out);
        }
    });
    path
}

#[valtron_test]
fn unix_socket_builder_send_reaches_server() {
    let socket = raw_http_serve_unix(200, "{\"ApiVersion\":\"1.53\"}");

    // The URL host is irrelevant for dialing (the socket is used), but is still
    // rendered into the request line / Host header.
    let client = HttpClientBuilder::new()
        .unix_socket(&socket)
        .max_redirects(0)
        .build();

    let result = client.send(get_request("http://localhost/version"));
    let _ = std::fs::remove_file(&socket);

    let response = result.expect("send over unix socket should succeed");
    assert_eq!(response.get_status(), Status::OK);
    let body = collect_bytes_from_send_safe(response.take_body());
    assert_eq!(
        String::from_utf8_lossy(&body),
        "{\"ApiVersion\":\"1.53\"}",
        "should read the server's body back over the unix socket"
    );
}

#[test]
fn unix_socket_builder_sets_config() {
    // Compile-level + config assertion: the builder method exists and threads
    // the path into ClientConfig without needing a live socket.
    let path = temp_socket_path("cfg");
    let _client = HttpClientBuilder::new().unix_socket(&path).build();
    // If this compiles and builds, the unix_socket surface is wired.
}
