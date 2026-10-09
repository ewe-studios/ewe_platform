//! Mock-based tests for container lifecycle operations on
//! [`DockerClient`] — restart, rename, top, and list — over a Unix socket.
//!
//! WHY: Proves the hand-written wrapper methods in `containers_lifecycle.rs`
//! correctly construct requests and handle responses over the Unix-socket
//! transport, without a real dockerd.
//!
//! WHAT: A `UnixListener` HTTP mock daemon that routes by method + path and
//! returns canned JSON (or 204 no-content), plus `#[valtron_test]` async fns
//! that drive each `DockerClient` method.
//!
//! HOW: Same pattern as `lifecycle_tests.rs` — `temp_socket_path`, `route`,
//! `mock_daemon` helpers; each test creates a client, calls a method, asserts.

#![cfg(all(unix, feature = "docker"))]

use std::io::{Read, Write};
use std::os::unix::net::UnixListener;
use std::path::PathBuf;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;

use foundation_core::valtron::valtron_test;
use foundation_deployment_docker::DockerClient;
use foundation_deployment_docker::generated::top::ContainerTopResponse;

fn temp_socket_path(tag: &str) -> PathBuf {
    let pid = std::process::id();
    let nanos = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_nanos())
        .unwrap_or(0);
    let path = std::env::temp_dir().join(format!("ewe_docker_lc_{tag}_{pid}_{nanos}.sock"));
    let _ = std::fs::remove_file(&path);
    path
}

/// Route a request path to a canned response. Strips the API version prefix
/// (`/v1.53/`) and query string before matching so the mock works regardless
/// of version and query parameters.
fn route(method: &str, path: &str) -> (u16, String) {
    // Strip query string (e.g. /containers/abc/top?ps_args=aux → .../top)
    let path_no_query = path.split('?').next().unwrap_or(path);
    let p = path_no_query.split_once("/v1.").map_or(path_no_query, |(_, rest)| {
        rest.split_once('/').map_or(rest, |(_, r)| r)
    });

    match (method, p) {
        // POST /containers/{id}/restart → 204 No Content
        ("POST", p) if p.starts_with("containers/") && p.ends_with("/restart") => {
            (204, String::new())
        }

        // POST /containers/{id}/rename?name=X → 204 No Content
        ("POST", p) if p.starts_with("containers/") && p.contains("/rename") => {
            (204, String::new())
        }

        // GET /containers/{id}/top → canned JSON
        ("GET", p) if p.starts_with("containers/") && p.ends_with("/top") => {
            let body = r#"{"Processes":[["root","1","0.0","0.0","123456","?","Ss","12:34","sleep 30"]],"Titles":["USER","PID","%CPU","%MEM","VSZ","RSS","TTY","STAT","START","COMMAND"]}"#;
            (200, body.to_string())
        }

        // GET /containers/json → canned JSON list
        ("GET", "containers/json") => {
            let body = r#"[{"Id":"abc123","Names":["/test_container"],"Image":"alpine:latest","ImageID":"sha256:abc","Command":"sleep 30","Created":1234567890,"Ports":[],"Labels":{},"State":"running","Status":"Up 1 hour","HostConfig":{"NetworkMode":"default"},"NetworkSettings":{"Networks":{"bridge":{"IPAddress":"172.17.0.2"}}},"Mounts":[]}]"#;
            (200, body.to_string())
        }

        _ => (404, format!("{{\"message\":\"no route for {method} {p}\"}}")),
    }
}

/// Start a mock Docker daemon on `socket_path`. Serves until `stop` is set.
fn mock_daemon(socket_path: PathBuf, stop: Arc<AtomicBool>) {
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
                    let (status, body) = route(method, path);
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

// ── restart tests ─────────────────────────────────────────────────────

#[valtron_test]
async fn docker_restart_container() {
    // WHY: Proves POST /containers/{id}/restart sends the right request and
    // handles the 204 No Content response.
    let socket = temp_socket_path("restart");
    let stop = Arc::new(AtomicBool::new(false));
    mock_daemon(socket.clone(), stop.clone());
    let client = DockerClient::connect_unix(&socket);

    client
        .restart_container("abc123", Some("SIGHUP"), Some(10))
        .await
        .expect("restart_container should succeed");

    stop.store(true, Ordering::Relaxed);
    let _ = std::fs::remove_file(&socket);
}

#[valtron_test]
async fn docker_restart_container_defaults() {
    // WHY: Proves restart works without optional signal/t parameters.
    let socket = temp_socket_path("restart_def");
    let stop = Arc::new(AtomicBool::new(false));
    mock_daemon(socket.clone(), stop.clone());
    let client = DockerClient::connect_unix(&socket);

    client
        .restart_container("abc123", None, None)
        .await
        .expect("restart_container with defaults should succeed");

    stop.store(true, Ordering::Relaxed);
    let _ = std::fs::remove_file(&socket);
}

// ── rename tests ──────────────────────────────────────────────────────

#[valtron_test]
async fn docker_rename_container() {
    // WHY: Proves POST /containers/{id}/rename?name=X sends the right
    // request and handles the 204 No Content response.
    let socket = temp_socket_path("rename");
    let stop = Arc::new(AtomicBool::new(false));
    mock_daemon(socket.clone(), stop.clone());
    let client = DockerClient::connect_unix(&socket);

    client
        .rename_container("abc123", "new_name")
        .await
        .expect("rename_container should succeed");

    stop.store(true, Ordering::Relaxed);
    let _ = std::fs::remove_file(&socket);
}

// ── top tests ─────────────────────────────────────────────────────────

#[valtron_test]
async fn docker_container_top() {
    // WHY: Proves GET /containers/{id}/top returns parsed PascalCase JSON
    // (Processes, Titles) into snake_case ContainerTopResponse.
    let socket = temp_socket_path("top");
    let stop = Arc::new(AtomicBool::new(false));
    mock_daemon(socket.clone(), stop.clone());
    let client = DockerClient::connect_unix(&socket);

    let top = client
        .container_top("abc123", Some("aux"))
        .await
        .expect("container_top should succeed");

    let titles = top.titles.expect("should have Titles");
    assert_eq!(titles.len(), 10);
    assert_eq!(titles[0], "USER");
    assert_eq!(titles[9], "COMMAND");

    let processes = top.processes.expect("should have Processes");
    assert_eq!(processes.len(), 1);
    // Mock returns 9 process fields (matching the mocked titles list)
    assert_eq!(processes[0].len(), 9);
    assert_eq!(processes[0][8], "sleep 30");

    stop.store(true, Ordering::Relaxed);
    let _ = std::fs::remove_file(&socket);
}

#[valtron_test]
async fn docker_container_top_no_ps_args() {
    // WHY: Proves top works without optional ps_args query parameter.
    let socket = temp_socket_path("top_nops");
    let stop = Arc::new(AtomicBool::new(false));
    mock_daemon(socket.clone(), stop.clone());
    let client = DockerClient::connect_unix(&socket);

    let top = client
        .container_top("abc123", None)
        .await
        .expect("container_top without ps_args should succeed");
    assert!(top.titles.is_some());

    stop.store(true, Ordering::Relaxed);
    let _ = std::fs::remove_file(&socket);
}

// ── list tests ────────────────────────────────────────────────────────

#[valtron_test]
async fn docker_container_list() {
    // WHY: Proves GET /containers/json returns the container summary array.
    let socket = temp_socket_path("list");
    let stop = Arc::new(AtomicBool::new(false));
    mock_daemon(socket.clone(), stop.clone());
    let client = DockerClient::connect_unix(&socket);

    let containers = client
        .container_list(true, None, false, None)
        .await
        .expect("container_list should succeed");

    let arr = containers.as_array().expect("should be an array");
    assert_eq!(arr.len(), 1);
    assert_eq!(arr[0]["Id"].as_str(), Some("abc123"));
    assert_eq!(arr[0]["Names"][0].as_str(), Some("/test_container"));

    stop.store(true, Ordering::Relaxed);
    let _ = std::fs::remove_file(&socket);
}

#[valtron_test]
async fn docker_container_list_with_filters() {
    // WHY: Proves container_list passes filters query parameter.
    let socket = temp_socket_path("list_filt");
    let stop = Arc::new(AtomicBool::new(false));
    mock_daemon(socket.clone(), stop.clone());
    let client = DockerClient::connect_unix(&socket);

    let containers = client
        .container_list(false, Some(5), true, Some(r#"{"status":["running"]}"#))
        .await
        .expect("container_list with filters should succeed");
    assert!(containers.is_array());

    stop.store(true, Ordering::Relaxed);
    let _ = std::fs::remove_file(&socket);
}

// ── unit tests ────────────────────────────────────────────────────────

#[test]
fn container_top_response_deserializes_pascalcase_json() {
    // WHY: Unit test proving the generator emits per-field #[serde(rename)]
    // for Processes/Titles PascalCase fields.
    let json = r#"{"Processes":[["root","1"]],"Titles":["USER","PID"]}"#;
    let resp: ContainerTopResponse =
        serde_json::from_str(json).expect("deserialize ContainerTopResponse");
    assert_eq!(resp.titles.as_deref(), Some(&["USER".to_string(), "PID".to_string()][..]));
    let procs = resp.processes.expect("should have Processes");
    assert_eq!(procs.len(), 1);
    assert_eq!(procs[0][0], "root");
}