//! The Docker client never sends `Expect: 100-continue` (spec 70 feature 43).
//!
//! WHY: Docker never sends `100 Continue`. A client that sends the header waits for it
//! before it sends the body, so each POST with a body (`containers/create`) stalled for
//! the whole wait: 2 minutes once the client was built with a 120 s read timeout.
//!
//! WHAT: A mock daemon on a Unix socket never sends `100 Continue` and records the
//! request head of each POST. The client must send no `Expect` header, and the POST must
//! finish at once.

#![cfg(all(unix, feature = "docker"))]

use std::io::{Read, Write};
use std::os::unix::net::{UnixListener, UnixStream};
use std::path::PathBuf;
use std::sync::mpsc;
use std::time::{Duration, Instant};

use foundation_core::valtron::valtron_test;
use foundation_deployment_docker::DockerClient;

fn temp_socket_path(tag: &str) -> PathBuf {
    let nanos = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map_or(0, |d| d.as_nanos());
    let path = PathBuf::from("/tmp").join(format!(
        "ewe_docker_expect_{tag}_{}_{nanos}.sock",
        std::process::id()
    ));
    let _ = std::fs::remove_file(&path);
    path
}

fn find(hay: &[u8], needle: &[u8]) -> Option<usize> {
    hay.windows(needle.len()).position(|w| w == needle)
}

/// Read one request and answer it. Never sends `100 Continue`. Reports the head of every
/// `POST` on `heads`.
fn serve_one(mut stream: UnixStream, heads: &mpsc::Sender<String>) {
    let mut buf = Vec::new();
    let mut chunk = [0u8; 4096];
    let head_end = loop {
        if let Some(i) = find(&buf, b"\r\n\r\n") {
            break i + 4;
        }
        match stream.read(&mut chunk) {
            Ok(0) | Err(_) => return,
            Ok(n) => buf.extend_from_slice(&chunk[..n]),
        }
    };
    let head = String::from_utf8_lossy(&buf[..head_end]).to_string();
    let length: usize = head
        .lines()
        .find_map(|l| {
            let (name, value) = l.split_once(':')?;
            name.eq_ignore_ascii_case("content-length")
                .then(|| value.trim().parse().ok())?
        })
        .unwrap_or(0);
    let mut have = buf.len() - head_end;
    while have < length {
        match stream.read(&mut chunk) {
            Ok(0) | Err(_) => return,
            Ok(n) => have += n,
        }
    }
    let (status, body) = if head.starts_with("POST ") {
        heads.send(head).expect("report head");
        ("201 Created", r#"{"Id":"abc123","Warnings":[]}"#)
    } else {
        ("200 OK", r#"{"ApiVersion":"1.53","MinAPIVersion":"1.24"}"#)
    };
    let reply = format!(
        "HTTP/1.1 {status}\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{body}",
        body.len()
    );
    let _ = stream.write_all(reply.as_bytes());
    let _ = stream.flush();
}

/// Run `create_container` against the mock daemon; return the POST head and how long the
/// call took.
async fn create_against_mock(tag: &str) -> (String, Duration) {
    let socket = temp_socket_path(tag);
    let listener = UnixListener::bind(&socket).expect("bind mock daemon");
    let (tx, rx) = mpsc::channel();
    std::thread::spawn(move || {
        for conn in listener.incoming() {
            let Ok(stream) = conn else { break };
            let tx = tx.clone();
            std::thread::spawn(move || serve_one(stream, &tx));
        }
    });

    let client = DockerClient::connect_unix(&socket);
    let started = Instant::now();
    let created = client
        .create_container(&serde_json::json!({"Image": "alpine"}), None)
        .await;
    let elapsed = started.elapsed();
    let _ = std::fs::remove_file(&socket);
    created.expect("create_container should succeed");
    let head = rx
        .recv_timeout(Duration::from_secs(5))
        .expect("the mock daemon saw the POST");
    (head, elapsed)
}

#[valtron_test(timeout = 60_000)]
async fn docker_client_sends_no_expect_header() {
    let (head, _) = create_against_mock("header").await;
    assert!(
        !head.to_lowercase().contains("expect:"),
        "the Docker client must not send Expect: 100-continue; head was:\n{head}"
    );
}

#[valtron_test(timeout = 60_000)]
async fn docker_post_with_body_does_not_wait_for_continue() {
    let (_, elapsed) = create_against_mock("timing").await;
    assert!(
        elapsed < Duration::from_secs(1),
        "POST /containers/create took {elapsed:?}; it waited for a `100 Continue`"
    );
}
