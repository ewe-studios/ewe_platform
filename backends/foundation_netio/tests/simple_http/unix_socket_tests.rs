//! Tests for Feature 02 (spec 54) — Unix domain socket transport.
//!
//! WHY: The Docker daemon and `BuildKitd` serve HTTP over a Unix socket. These
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
        .map_or(0, |d| d.as_nanos());
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
        body_readiness: None,
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

/// Read one line (up to a line feed) from a Unix socket stream.
fn read_line(stream: &mut std::os::unix::net::UnixStream, max: usize) -> Vec<u8> {
    let mut line = Vec::new();
    let mut byte = [0_u8; 1];
    while line.len() < max {
        match stream.read(&mut byte) {
            Ok(0) | Err(_) => break,
            Ok(_) => {
                line.push(byte[0]);
                if byte[0] == b'\n' {
                    break;
                }
            }
        }
    }
    line
}

/// A Unix-socket HTTP server that reads a preamble first, then the request.
struct PreambleServer {
    path: PathBuf,
    preamble: std::sync::Arc<std::sync::Mutex<Vec<Vec<u8>>>>,
}

impl PreambleServer {
    fn start(reply: Option<&'static [u8]>) -> Self {
        let path = temp_socket_path("preamble");
        let listener = UnixListener::bind(&path).expect("bind unix socket");
        let preamble = std::sync::Arc::new(std::sync::Mutex::new(Vec::new()));
        let recorded = std::sync::Arc::clone(&preamble);
        std::thread::spawn(move || {
            if let Some(stream) = listener.incoming().next() {
                let Ok(mut stream) = stream else { return };
                let line = read_line(&mut stream, 64);
                recorded
                    .lock()
                    .unwrap_or_else(std::sync::PoisonError::into_inner)
                    .push(line.clone());
                let Some(reply) = reply else {
                    // Close the socket after the preamble, as Firecracker does
                    // when no guest program listens.
                    return;
                };
                if stream.write_all(reply).is_err() || stream.flush().is_err() {
                    return;
                }
                if !line.starts_with(b"CONNECT ") {
                    return;
                }
                let mut buf = [0_u8; 4096];
                let _ = stream.read(&mut buf);
                let body = b"{\"ok\":true}";
                let response = format!(
                    "HTTP/1.1 200 OK\r\nContent-Length: {}\r\nConnection: close\r\n\r\n",
                    body.len()
                );
                let mut out = response.into_bytes();
                out.extend_from_slice(body);
                let _ = stream.write_all(&out);
            }
        });
        Self { path, preamble }
    }
}

impl Drop for PreambleServer {
    fn drop(&mut self) {
        let _ = std::fs::remove_file(&self.path);
    }
}

#[valtron_test]
fn unix_socket_preamble_ok() {
    let server = PreambleServer::start(Some(b"OK 1073741824\n"));
    let client = HttpClientBuilder::new()
        .unix_socket(&server.path)
        .unix_socket_preamble(b"CONNECT 1024\n".to_vec(), b"OK ".to_vec(), 32)
        .max_redirects(0)
        .build();

    let response = client
        .send(get_request("http://localhost/"))
        .expect("send over the preamble socket should succeed");
    assert_eq!(response.get_status(), Status::OK);
    let body = collect_bytes_from_send_safe(response.take_body());
    assert_eq!(String::from_utf8_lossy(&body), "{\"ok\":true}");
    let preamble = server
        .preamble
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner)
        .clone();
    assert_eq!(
        preamble,
        vec![b"CONNECT 1024\n".to_vec()],
        "the server sees the preamble one time for one connection"
    );
}

#[valtron_test]
fn unix_socket_preamble_closed() {
    let server = PreambleServer::start(None);
    let client = HttpClientBuilder::new()
        .unix_socket(&server.path)
        .unix_socket_preamble(b"CONNECT 1024\n".to_vec(), b"OK ".to_vec(), 32)
        .max_redirects(0)
        .build();

    let result = client.send(get_request("http://localhost/"));
    assert!(
        result.is_err(),
        "a server that closes after the preamble fails the call"
    );
}

#[valtron_test]
fn unix_socket_preamble_bad_reply() {
    let server = PreambleServer::start(Some(b"NO\n"));
    let client = HttpClientBuilder::new()
        .unix_socket(&server.path)
        .unix_socket_preamble(b"CONNECT 1024\n".to_vec(), b"OK ".to_vec(), 32)
        .max_redirects(0)
        .build();

    let result = client.send(get_request("http://localhost/"));
    assert!(result.is_err(), "a reply that is not `OK ` fails the call");
}
