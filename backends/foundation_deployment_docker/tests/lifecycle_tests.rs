//! End-to-end tests for `DockerClient` over a mock Docker daemon on a Unix socket.
//!
//! WHY: Proves the generated async `*_request` functions + hand-written
//! `DockerClient` actually round-trip over the Unix-socket transport (Feature 02)
//! — version negotiation and serde (de)serialization of PascalCase Docker JSON
//! via per-field `#[serde(rename)]` — without a real dockerd.
//!
//! WHAT: A tiny `UnixListener` HTTP server that routes by request path and
//! returns canned JSON, plus `#[valtron_test]` async fns that drive each
//! `DockerClient` method via `.await`.
//!
//! HOW: `#[valtron_test]` accepts `async fn` (feature 00-F3); no manual
//! `block_on_future` needed.

#![cfg(all(unix, feature = "docker"))]

use std::io::{Read, Write};
use std::os::unix::net::UnixListener;
use std::path::PathBuf;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;

use foundation_core::valtron::valtron_test;
use foundation_deployment_docker::DockerClient;
use foundation_deployment_docker::generated::version::SystemVersion;

fn temp_socket_path(tag: &str) -> PathBuf {
    let pid = std::process::id();
    let nanos = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_nanos())
        .unwrap_or(0);
    let path = std::env::temp_dir().join(format!("ewe_docker_{tag}_{pid}_{nanos}.sock"));
    let _ = std::fs::remove_file(&path);
    path
}

/// Route a request path to a canned JSON response body.
/// `api_version` overrides the `ApiVersion` value in `GET /version`
/// (defaults to `"1.53"` when `None`).
fn route(method: &str, path: &str, api_version: Option<&str>) -> (u16, String) {
    let version = api_version.unwrap_or("1.53");
    let p = path.split_once("/v1.").map_or(path, |(_, rest)| {
        rest.split_once('/').map_or(rest, |(_, r)| r)
    });
    match (method, p) {
        ("GET", "version") => (
            200,
            format!(r#"{{"ApiVersion":"{version}","Os":"linux","Arch":"amd64","Version":"27.0.0"}}"#),
        ),
        _ => (404, format!("{{\"message\":\"no route for {method} {p}\"}}")),
    }
}

/// Start a mock Docker daemon on `socket_path`. Serves until `stop` is set.
/// `api_version` is the `ApiVersion` value the mock reports in `GET /version`.
fn mock_daemon(socket_path: PathBuf, stop: Arc<AtomicBool>, api_version: Option<&'static str>) {
    let listener = UnixListener::bind(&socket_path).expect("bind mock docker socket");
    listener
        .set_nonblocking(true)
        .expect("set nonblocking");
    std::thread::spawn(move || {
        while !stop.load(Ordering::Relaxed) {
            match listener.accept() {
                Ok((mut stream, _)) => {
                    let mut buf = [0u8; 8192];
                    let n = stream.read(&mut buf).unwrap_or(0);
                    let req = String::from_utf8_lossy(&buf[..n]);
                    let first = req.lines().next().unwrap_or("");
                    let mut parts = first.split_whitespace();
                    let method = parts.next().unwrap_or("");
                    let path = parts.next().unwrap_or("");
                    let (status, body) = route(method, path, api_version);
                    let reason = if status == 204 { "No Content" } else { "OK" };
                    let response = format!(
                        "HTTP/1.1 {status} {reason}\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{}",
                        body.len(),
                        body
                    );
                    let _ = stream.write_all(response.as_bytes());
                    let _ = stream.flush();
                }
                Err(ref e) if e.kind() == std::io::ErrorKind::WouldBlock => {
                    std::thread::yield_now();
                }
                Err(_) => break,
            }
        }
    });
}

#[valtron_test]
async fn docker_version_parses_pascalcase_json() {
    // WHY: Proves per-field #[serde(rename)] is emitted by the generator so
    // PascalCase Docker JSON (ApiVersion, Os, Arch) deserializes into snake_case
    // Rust fields (api_version, os, arch).
    let socket = temp_socket_path("version");
    let stop = Arc::new(AtomicBool::new(false));
    mock_daemon(socket.clone(), stop.clone(), None);
    let client = DockerClient::connect_unix(&socket);

    let v = client.version().await.expect("version() should succeed");
    assert_eq!(v.api_version.as_deref(), Some("1.53"));
    assert_eq!(v.os.as_deref(), Some("linux"));
    assert_eq!(v.arch.as_deref(), Some("amd64"));
    assert_eq!(v.version.as_deref(), Some("27.0.0"));

    stop.store(true, Ordering::Relaxed);
    let _ = std::fs::remove_file(&socket);
}

#[valtron_test]
async fn docker_version_twice_sequential() {
    // WHY: Proves the mock daemon handles multiple sequential requests over the
    // same connection without leaking state.
    let socket = temp_socket_path("version2x");
    let stop = Arc::new(AtomicBool::new(false));
    mock_daemon(socket.clone(), stop.clone(), None);
    let client = DockerClient::connect_unix(&socket);

    let v1 = client.version().await.expect("first version()");
    assert_eq!(v1.api_version.as_deref(), Some("1.53"));

    let v2 = client.version().await.expect("second version()");
    assert_eq!(v2.api_version.as_deref(), Some("1.53"));

    stop.store(true, Ordering::Relaxed);
    let _ = std::fs::remove_file(&socket);
}

#[valtron_test]
async fn docker_negotiate_version_lowers_to_older_server() {
    let socket = temp_socket_path("negotiate_down");
    let stop = Arc::new(AtomicBool::new(false));
    mock_daemon(socket.clone(), stop.clone(), Some("1.41"));

    let mut client = DockerClient::connect_unix(&socket);
    assert_eq!(client.api_version(), "1.53", "starts at client default");

    client
        .negotiate_version()
        .await
        .expect("negotiate_version should succeed");

    assert_eq!(
        client.api_version(),
        "1.41",
        "lowered to server cap when server is older"
    );

    stop.store(true, Ordering::Relaxed);
    let _ = std::fs::remove_file(&socket);
}

#[valtron_test]
async fn docker_negotiate_version_stays_when_server_is_newer() {
    let socket = temp_socket_path("negotiate_up");
    let stop = Arc::new(AtomicBool::new(false));
    mock_daemon(socket.clone(), stop.clone(), Some("1.99"));

    let mut client = DockerClient::connect_unix(&socket);
    client
        .negotiate_version()
        .await
        .expect("negotiate_version should succeed");

    assert_eq!(
        client.api_version(),
        "1.53",
        "stays at client default when server is newer"
    );

    stop.store(true, Ordering::Relaxed);
    let _ = std::fs::remove_file(&socket);
}

#[test]
fn system_version_deserializes_pascalcase_json_via_per_field_rename() {
    // WHY: Unit test proving the generator emits per-field #[serde(rename)] for
    // properties whose PascalCase original name differs from the snake_case Rust
    // field name. This handles Docker's mixed-casing (ApiVersion + architecture).
    let json = r#"{"ApiVersion":"1.53","Os":"linux","Arch":"amd64","Version":"27.0.0"}"#;
    let v: SystemVersion = serde_json::from_str(json).expect("deserialize PascalCase JSON");
    assert_eq!(v.api_version.as_deref(), Some("1.53"));
    assert_eq!(v.os.as_deref(), Some("linux"));
    assert_eq!(v.arch.as_deref(), Some("amd64"));
    assert_eq!(v.version.as_deref(), Some("27.0.0"));
}
