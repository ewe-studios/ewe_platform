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

mod mock_socket;

use std::io::Write;
use std::os::unix::net::UnixStream;
use std::path::PathBuf;
use std::sync::atomic::AtomicBool;
use std::sync::{mpsc, Arc, Mutex};
use std::time::{Duration, Instant};

use foundation_core::valtron::valtron_test;
use foundation_deployment_docker::{DockerClient, DockerError, DockerLimits};

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
        // Version negotiation: the client asks once, unversioned, before its
        // first versioned request.
        ("GET", "/version") => (200, r#"{"ApiVersion":"1.53","MinAPIVersion":"1.24"}"#.to_string()),
        // image_pull: POST /images/create — a 200 progress stream; a failed
        // pull reports `{"error": …}` inside it, as dockerd does.
        ("POST", p) if p.starts_with("images/create?") && p.contains("fromImage=missing") => (
            200,
            "{\"status\":\"Pulling repository missing\"}\n\
             {\"errorDetail\":{\"message\":\"pull access denied for missing\"},\"error\":\"pull access denied for missing\"}\n"
                .to_string(),
        ),
        ("POST", p) if p.starts_with("images/create?") => (
            200,
            "{\"status\":\"Pulling from library/alpine\",\"id\":\"latest\"}\n\
             {\"status\":\"Status: Downloaded newer image for alpine:latest\"}\n"
                .to_string(),
        ),
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
fn mock_images_daemon(socket_path: PathBuf, stop: Arc<AtomicBool>) {
    mock_images_daemon_gated(socket_path, stop, None);
}

/// [`mock_images_daemon`], plus a `gate` the `fromImage=gated` pull waits on
/// between its first progress line and the rest.
fn mock_images_daemon_gated(
    socket_path: PathBuf,
    stop: Arc<AtomicBool>,
    gate: Option<mpsc::Receiver<()>>,
) {
    let gate = Arc::new(Mutex::new(gate));
    mock_socket::serve(&socket_path, stop, move |stream| {
        let gate = Arc::clone(&gate);
        // One thread per connection: a streaming answer must not
        // hold up the next request.
        std::thread::spawn(move || serve(stream, &gate));
    });
}

/// Answer one request on `stream`.
fn serve(mut stream: UnixStream, gate: &Mutex<Option<mpsc::Receiver<()>>>) {
    let Some(req) = mock_socket::read_request(&mut stream) else {
        return;
    };
    let (method, path) = (req.method.as_str(), req.target.as_str());

    if method == "POST" && path.contains("images/create?") && path.contains("fromImage=gated") {
        // A chunked progress stream that stalls after its first line until the
        // test, having received that line, opens the gate — or 5s pass, in
        // which case the stream says so.
        let _ = stream.write_all(b"HTTP/1.1 200 OK\r\nTransfer-Encoding: chunked\r\nConnection: close\r\n\r\n");
        write_chunk(&mut stream, b"{\"status\":\"first\"}\n");
        let opened = gate
            .lock()
            .expect("gate lock")
            .take()
            .is_some_and(|rx| rx.recv_timeout(Duration::from_secs(5)).is_ok());
        let rest: &[u8] = if opened {
            b"{\"status\":\"second\"}\n{\"status\":\"Status: Downloaded newer image for gated:latest\"}\n"
        } else {
            b"{\"status\":\"the client never asked for the rest\"}\n"
        };
        write_chunk(&mut stream, rest);
        let _ = stream.write_all(b"0\r\n\r\n");
        let _ = stream.flush();
        return;
    }

    if method == "POST" && path.contains("images/create?") && path.contains("fromImage=huge-error") {
        // A 500 whose body never ends in practice: the client must answer from
        // the status and a bounded read, not wait for (or buffer) the body.
        let total = HUGE_ERROR_BODY;
        let head = format!(
            "HTTP/1.1 500 Internal Server Error\r\nContent-Length: {total}\r\nConnection: close\r\n\r\n"
        );
        if stream.write_all(head.as_bytes()).is_err() {
            return;
        }
        let block = [b'e'; 64 * 1024];
        let mut written = 0;
        while written < total {
            let n = block.len().min(total - written);
            // Fails once the client has read what it wants and hung up.
            if stream.write_all(&block[..n]).is_err() {
                return;
            }
            written += n;
        }
        return;
    }

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

/// The size of the `fromImage=huge-error` error body: far more than any
/// error message, and more than a client should ever hold.
const HUGE_ERROR_BODY: usize = 256 * 1024 * 1024;

fn write_chunk(stream: &mut UnixStream, data: &[u8]) {
    let _ = stream.write_all(format!("{:x}\r\n", data.len()).as_bytes());
    let _ = stream.write_all(data);
    let _ = stream.write_all(b"\r\n");
    let _ = stream.flush();
}

/// A pull's progress comes back as a stream of typed items, read as they
/// arrive; the pull has finished when the stream ends.
#[valtron_test]
async fn image_pull_streams_typed_progress_items() {
    let socket = temp_socket_path("image_pull");
    let stop = Arc::new(AtomicBool::new(false));
    mock_images_daemon(socket.clone(), stop.clone());

    let client = DockerClient::connect_unix(&socket);
    let mut progress = client
        .image_pull(Some("alpine"), None, None, Some("latest"), None, None)
        .await
        .expect("image_pull should start");
    let mut statuses = Vec::new();
    while let Some(item) = progress.next().await {
        let item = item.expect("a progress item");
        statuses.push(item.status.expect("every line has a status"));
    }
    assert_eq!(
        statuses,
        vec![
            "Pulling from library/alpine".to_string(),
            "Status: Downloaded newer image for alpine:latest".to_string(),
        ]
    );

    mock_socket::stop(&socket, &stop);
}

/// A failed pull is a `200` whose stream carries `errorDetail`: reading the
/// stream to its end must yield that as `PullFailed`, not success.
#[valtron_test]
async fn image_pull_reports_an_error_in_the_stream() {
    let socket = temp_socket_path("image_pull_err");
    let stop = Arc::new(AtomicBool::new(false));
    mock_images_daemon(socket.clone(), stop.clone());

    let client = DockerClient::connect_unix(&socket);
    let progress = client
        .image_pull(Some("missing"), None, None, Some("latest"), None, None)
        .await
        .expect("the daemon answers 200: the failure is in the stream");
    match progress.finish().await {
        Err(DockerError::PullFailed(m)) => assert_eq!(m, "pull access denied for missing"),
        other => panic!("expected PullFailed, got {other:?}"),
    }

    mock_socket::stop(&socket, &stop);
}

/// The status is checked before the body: a non-2xx answer is an error read
/// from at most a bounded prefix of its body, however large that body is.
#[valtron_test]
async fn image_pull_checks_the_status_before_the_body() {
    let socket = temp_socket_path("image_pull_500");
    let stop = Arc::new(AtomicBool::new(false));
    mock_images_daemon(socket.clone(), stop.clone());

    let client = DockerClient::connect_unix(&socket);
    let started = Instant::now();
    let err = client
        .image_pull(Some("huge-error"), None, None, Some("latest"), None, None)
        .await
        .expect_err("a 500 is an error");
    match err {
        DockerError::Api { status, message } => {
            assert_eq!(status, 500);
            assert!(
                message.len() <= DockerLimits::default().max_error_body,
                "read {} bytes of the error body; at most {} should be",
                message.len(),
                DockerLimits::default().max_error_body
            );
            assert!(message.starts_with("eeee"), "{}", &message[..message.len().min(64)]);
        }
        other => panic!("expected Api, got {other:?}"),
    }
    assert!(
        started.elapsed() < Duration::from_secs(30),
        "the {HUGE_ERROR_BODY}-byte error body was read rather than abandoned"
    );

    mock_socket::stop(&socket, &stop);
}

/// The progress stream is not buffered: the first item is handed out while the
/// daemon is still holding back the rest, which it sends only once the test has
/// that first item. A client that read the body whole would wait for the rest
/// (and see the daemon's "never asked" line after its timeout instead).
#[valtron_test]
async fn image_pull_hands_out_items_before_the_body_has_ended() {
    let socket = temp_socket_path("image_pull_gated");
    let stop = Arc::new(AtomicBool::new(false));
    let (open_gate, gate) = mpsc::channel();
    mock_images_daemon_gated(socket.clone(), stop.clone(), Some(gate));

    let client = DockerClient::connect_unix(&socket);
    let mut progress = client
        .image_pull(Some("gated"), None, None, Some("latest"), None, None)
        .await
        .expect("image_pull should start");

    let first = progress.next().await.expect("a first item").expect("valid");
    assert_eq!(first.status.as_deref(), Some("first"));
    open_gate.send(()).expect("the daemon is waiting on the gate");

    let mut rest = Vec::new();
    while let Some(item) = progress.next().await {
        rest.push(item.expect("valid").status.unwrap_or_default());
    }
    assert_eq!(
        rest,
        vec![
            "second".to_string(),
            "Status: Downloaded newer image for gated:latest".to_string(),
        ]
    );

    mock_socket::stop(&socket, &stop);
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

    mock_socket::stop(&socket, &stop);
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

    mock_socket::stop(&socket, &stop);
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

    mock_socket::stop(&socket, &stop);
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

    mock_socket::stop(&socket, &stop);
}

/// A client configured with a smaller error-body cap reads no more than that.
#[valtron_test]
async fn a_configured_error_body_cap_is_the_one_enforced() {
    let socket = temp_socket_path("image_pull_500_cap");
    let stop = Arc::new(AtomicBool::new(false));
    mock_images_daemon(socket.clone(), stop.clone());

    let client = DockerClient::connect_unix(&socket)
        .with_limits(DockerLimits::default().with_max_error_body(100));
    let err = client
        .image_pull(Some("huge-error"), None, None, Some("latest"), None, None)
        .await
        .expect_err("a 500 is an error");
    match err {
        DockerError::Api { status, message } => {
            assert_eq!(status, 500);
            assert_eq!(message, "e".repeat(100), "exactly the configured cap is read");
        }
        other => panic!("expected Api, got {other:?}"),
    }

    mock_socket::stop(&socket, &stop);
}
