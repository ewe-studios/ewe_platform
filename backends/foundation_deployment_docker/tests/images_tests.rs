//! End-to-end tests for image operations over a mock Docker daemon on a Unix socket.
//!
//! WHY: Proves the hand-written `DockerClient` image methods round-trip over
//! the Unix-socket transport, routing HTTP requests to the correct endpoint and
//! parsing the canned JSON responses.
//!
//! WHAT: A tiny `UnixListener` HTTP server that routes by request path and
//! returns canned JSON, plus `#[valtron_test]` async fns that drive each
//! image method.
//!
//! HOW: `#[valtron_test]` accepts `async fn`; no manual `block_on_future` needed.

#![cfg(all(unix, feature = "docker"))]

use std::io::{Read, Write};
use std::os::unix::net::UnixListener;
use std::path::PathBuf;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;

use foundation_core::valtron::valtron_test;
use foundation_deployment_docker::DockerClient;

fn temp_socket_path(tag: &str) -> PathBuf {
    let pid = std::process::id();
    let nanos = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_nanos())
        .unwrap_or(0);
    let path = std::env::temp_dir().join(format!("ewe_docker_images_{tag}_{pid}_{nanos}.sock"));
    let _ = std::fs::remove_file(&path);
    path
}

/// Route a request to a canned JSON response body for image endpoints.
fn images_route(method: &str, path: &str) -> (u16, String) {
    // Strip API version prefix: /v1.53/images/json -> images/json
    let p = path
        .split_once("/v1.")
        .map(|(_, rest)| {
            rest.split_once('/')
                .map_or(rest, |(_, r)| r)
        })
        .unwrap_or(path);

    match (method, p) {
        // image_list: GET /images/json
        ("GET", "images/json") => (
            200,
            r#"[{"Id":"sha256:abc","RepoTags":["alpine:latest"],"Created":1680000000,"Size":7000000,"VirtualSize":7000000,"SharedSize":0,"Containers":0}]"#
                .to_string(),
        ),
        // image_inspect: GET /images/{name}/json
        ("GET", p) if p.starts_with("images/") && p.ends_with("/json") => (
            200,
            r#"{"Id":"sha256:abc","RepoTags":["alpine:latest"],"Created":"2024-01-01T00:00:00Z","Size":7000000}"#
                .to_string(),
        ),
        // image_tag: POST /images/{name}/tag
        ("POST", p) if p.contains("/tag") => (201, String::new()),
        // image_delete: DELETE /images/{name}
        ("DELETE", p) if p.starts_with("images/") => (
            200,
            r#"[{"Untagged":"alpine:latest"},{"Deleted":"sha256:abc"}]"#.to_string(),
        ),
        _ => (
            404,
            format!("{{\"message\":\"no route for {method} {p}\"}}"),
        ),
    }
}

/// Start a mock Docker daemon on `socket_path`. Serves until `stop` is set.
fn mock_images_daemon(
    socket_path: PathBuf,
    stop: Arc<AtomicBool>,
) {
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
                    let (status, body) = images_route(method, path);
                    let reason = match status {
                        201 => "Created",
                        204 => "No Content",
                        _ => "OK",
                    };
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
async fn image_list_returns_json_array() {
    // WHY: Proves image_list hits GET /images/json and deserializes the canned
    // array of image summaries.
    let socket = temp_socket_path("image_list");
    let stop = Arc::new(AtomicBool::new(false));
    mock_images_daemon(socket.clone(), stop.clone());

    let client = DockerClient::connect_unix(&socket);
    let result = client
        .image_list(None, None, None, None)
        .await
        .expect("image_list should succeed");

    let arr = result.as_array().expect("response should be a JSON array");
    assert_eq!(arr.len(), 1);
    assert_eq!(arr[0]["Id"], "sha256:abc");
    assert_eq!(arr[0]["RepoTags"][0], "alpine:latest");

    stop.store(true, Ordering::Relaxed);
    let _ = std::fs::remove_file(&socket);
}

#[valtron_test]
async fn image_inspect_returns_image_details() {
    // WHY: Proves image_inspect hits GET /images/{name}/json and returns the
    // image inspect data.
    let socket = temp_socket_path("image_inspect");
    let stop = Arc::new(AtomicBool::new(false));
    mock_images_daemon(socket.clone(), stop.clone());

    let client = DockerClient::connect_unix(&socket);
    let result = client
        .image_inspect("alpine")
        .await
        .expect("image_inspect should succeed");

    assert_eq!(result.id.as_deref(), Some("sha256:abc"));
    assert_eq!(result.size, Some(7000000));

    stop.store(true, Ordering::Relaxed);
    let _ = std::fs::remove_file(&socket);
}

#[valtron_test]
async fn image_tag_returns_created() {
    // WHY: Proves image_tag hits POST /images/{name}/tag with query params
    // and returns Ok on 201 Created.
    let socket = temp_socket_path("image_tag");
    let stop = Arc::new(AtomicBool::new(false));
    mock_images_daemon(socket.clone(), stop.clone());

    let client = DockerClient::connect_unix(&socket);
    // image_tag returns Result<(), DockerError>
    client
        .image_tag("alpine", Some("alpine"), Some("v2"))
        .await
        .expect("image_tag should succeed");

    stop.store(true, Ordering::Relaxed);
    let _ = std::fs::remove_file(&socket);
}

#[valtron_test]
async fn image_delete_returns_deleted_layers() {
    // WHY: Proves image_delete hits DELETE /images/{name} and returns the
    // list of deleted/untagged layers.
    let socket = temp_socket_path("image_delete");
    let stop = Arc::new(AtomicBool::new(false));
    mock_images_daemon(socket.clone(), stop.clone());

    let client = DockerClient::connect_unix(&socket);
    let result = client
        .image_delete("alpine", None, None)
        .await
        .expect("image_delete should succeed");

    let arr = result.as_array().expect("response should be a JSON array");
    assert_eq!(arr.len(), 2);
    assert_eq!(arr[0]["Untagged"], "alpine:latest");
    assert_eq!(arr[1]["Deleted"], "sha256:abc");

    stop.store(true, Ordering::Relaxed);
    let _ = std::fs::remove_file(&socket);
}
