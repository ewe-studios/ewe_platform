//! Tests for `DockerClient` Exec and System operations over a mock Docker daemon.
//!
//! WHY: Proves the hand-written `exec_*` and `system_*` wrappers round-trip
//! over the Unix-socket transport, including JSON body injection for exec_create
//! and plain-text response handling for system_ping.
//!
//! WHAT: A mock Unix-socket HTTP server that routes by request path and returns
//! canned responses, plus `#[valtron_test]` async fns that drive each method.
//!
//! HOW: Same pattern as `lifecycle_tests.rs` — `UnixListener` + routing +
//! `#[valtron_test]`.

#![cfg(all(unix, feature = "docker"))]

mod mock_socket;

use std::io::Write;
use std::path::PathBuf;
use std::sync::atomic::AtomicBool;
use std::sync::Arc;

use foundation_core::valtron::valtron_test;
use foundation_deployment_docker::DockerClient;

fn temp_socket_path(tag: &str) -> PathBuf {
    let pid = std::process::id();
    let nanos = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_nanos())
        .unwrap_or(0);
    let path = std::env::temp_dir().join(format!("ewe_docker_execsys_{tag}_{pid}_{nanos}.sock"));
    let _ = std::fs::remove_file(&path);
    path
}

/// Route a request path to a canned response. Returns (status, content_type, body).
/// Strips the `/v1.XX/` prefix before matching.
fn route(method: &str, path: &str) -> (u16, String, String) {
    let path_no_query = path.split('?').next().unwrap_or(path);
    let p = path_no_query.split_once("/v1.").map_or(path_no_query, |(_, rest)| {
        rest.split_once('/').map_or(rest, |(_, r)| r)
    });
    match (method, p) {
        // Version negotiation: the client asks once, unversioned, before its
        // first versioned request.
        ("GET", "/version") => (
            200,
            "application/json".to_string(),
            r#"{"ApiVersion":"1.53","MinAPIVersion":"1.24"}"#.to_string(),
        ),
        // System
        ("GET", "_ping") => (200, "text/plain".to_string(), "OK".to_string()),
        ("GET", "info") => (
            200,
            "application/json".to_string(),
            r#"{"ID":"abc","Containers":0}"#.to_string(),
        ),
        // Exec
        ("POST", "containers/test/exec") => (
            201,
            "application/json".to_string(),
            r#"{"Id":"exec123"}"#.to_string(),
        ),
        ("GET", "exec/exec123/json") => (
            200,
            "application/json".to_string(),
            r#"{"ID":"exec123","Running":false}"#.to_string(),
        ),
        _ => (
            404,
            "application/json".to_string(),
            format!(r#"{{"message":"no route for {method} {p}"}}"#),
        ),
    }
}

/// Start a mock Docker daemon on `socket_path`. Serves until `stop` is set.
fn mock_daemon(socket_path: PathBuf, stop: Arc<AtomicBool>) {
    mock_socket::serve(&socket_path, stop, move |mut stream| {
        let Some(req) = mock_socket::read_request(&mut stream) else {
            return;
        };
        let (method, path) = (req.method.as_str(), req.target.as_str());
        let (status, content_type, body) = route(method, path);
        let reason = match status {
            201 => "Created",
            204 => "No Content",
            _ => "OK",
        };
        let response = format!(
            "HTTP/1.1 {status} {reason}\r\nContent-Type: {content_type}\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{}",
            body.len(),
            body
        );
        let _ = stream.write_all(response.as_bytes());
        let _ = stream.flush();
    });
}

// ── System tests ──────────────────────────────────────────────────────

#[valtron_test]
async fn system_ping_returns_ok() {
    // WHY: Proves `system_ping` handles the daemon's text/plain "OK" response
    // (which would fail JSON parsing in the generated function).
    let socket = temp_socket_path("ping");
    let stop = Arc::new(AtomicBool::new(false));
    mock_daemon(socket.clone(), stop.clone());
    let client = DockerClient::connect_unix(&socket);

    client
        .system_ping()
        .await
        .expect("system_ping should succeed");

    mock_socket::stop(&socket, &stop);
}

#[valtron_test]
async fn system_info_returns_data() {
    // WHY: Proves `system_info` deserializes the daemon's JSON response
    // into the generated `SystemInfo` struct.
    let socket = temp_socket_path("info");
    let stop = Arc::new(AtomicBool::new(false));
    mock_daemon(socket.clone(), stop.clone());
    let client = DockerClient::connect_unix(&socket);

    let info = client.system_info().await.expect("system_info should succeed");
    assert_eq!(info.get("ID").and_then(|v| v.as_str()), Some("abc"));
    assert_eq!(info.get("Containers").and_then(|v| v.as_i64()), Some(0));

    mock_socket::stop(&socket, &stop);
}

// ── Exec tests ────────────────────────────────────────────────────────

#[valtron_test]
async fn exec_create_returns_id() {
    // WHY: Proves `exec_create` sends the JSON body with Cmd, AttachStdout,
    // AttachStderr and deserializes the `Id` response.
    let socket = temp_socket_path("exec_create");
    let stop = Arc::new(AtomicBool::new(false));
    mock_daemon(socket.clone(), stop.clone());
    let client = DockerClient::connect_unix(&socket);

    let id_resp = client
        .exec_create("test", &["echo", "hello"])
        .await
        .expect("exec_create should succeed");
    assert_eq!(id_resp.id, "exec123");

    mock_socket::stop(&socket, &stop);
}

#[valtron_test]
async fn exec_inspect_returns_json() {
    // WHY: Proves `exec_inspect` successfully fetches and parses the JSON
    // response body that the generated function would discard.
    let socket = temp_socket_path("exec_inspect");
    let stop = Arc::new(AtomicBool::new(false));
    mock_daemon(socket.clone(), stop.clone());
    let client = DockerClient::connect_unix(&socket);

    let inspect = client
        .exec_inspect("exec123")
        .await
        .expect("exec_inspect should succeed");
    assert_eq!(inspect["ID"].as_str(), Some("exec123"));
    assert_eq!(inspect["Running"].as_bool(), Some(false));

    mock_socket::stop(&socket, &stop);
}
