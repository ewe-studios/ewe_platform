//! Mock-based tests for [`wait_for_stdout`] — the stdout log readiness wait.
//!
//! WHY: `wait_for_stdout` used to poll `container_logs` with
//! `ContainerLogsOptions::default()` (`follow=false`), so a message split
//! across the end of one poll's buffer and the start of the next was never
//! seen, and a message written only to stderr was matched anyway (the old
//! polling code appended both streams). Nothing exercised the log stream, so
//! neither failure showed.
//!
//! WHAT is proven, against a mock Docker daemon on a Unix socket:
//! - a message split across two stdout frames is matched;
//! - a message written only to stderr is not matched (the wait times out);
//! - one follow request reads the stream once: the mock's total bytes equal
//!   the single serving, not a multiple from repeated polls.
//!
//! HOW: `mock_socket::serve` runs a blocking accept loop; the `/logs` handler
//! writes Docker multiplexed frames (stream type byte, 3 zero bytes, 4-byte
//! big-endian length, payload). `#[valtron_test]` gives the async client its
//! executor pool.

#![cfg(all(unix, feature = "docker"))]

mod mock_socket;

use std::io::Write;
use std::os::unix::net::UnixStream;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};
use std::time::Duration;

use foundation_core::valtron::valtron_test;
use foundation_deployment_platform::docker::{DockerClient, DockerError, wait_for_stdout};

/// A unique socket path so parallel test runs never collide.
fn temp_socket_path(tag: &str) -> PathBuf {
    let pid = std::process::id();
    let nanos = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map_or(0, |d| d.as_nanos());
    let path = std::env::temp_dir().join(format!("ewe_plat_wfs_{tag}_{pid}_{nanos}.sock"));
    let _ = std::fs::remove_file(&path);
    path
}

/// One Docker multiplexed log frame: `[stream type][3 zero bytes][u32 BE
/// length][payload]`. Stream type 1 is stdout, 2 is stderr.
fn frame(stream_type: u8, payload: &[u8]) -> Vec<u8> {
    let mut out = Vec::with_capacity(8 + payload.len());
    out.push(stream_type);
    out.extend_from_slice(&[0u8; 3]);
    out.extend_from_slice(
        &u32::try_from(payload.len())
            .unwrap_or(u32::MAX)
            .to_be_bytes(),
    );
    out.extend_from_slice(payload);
    out
}

/// Write an HTTP response with a `Content-Length` body.
fn write_response(stream: &mut UnixStream, status: u16, body: &[u8]) {
    let reason = if status == 204 { "No Content" } else { "OK" };
    let head = format!(
        "HTTP/1.1 {status} {reason}\r\nContent-Length: {}\r\nConnection: close\r\n\r\n",
        body.len()
    );
    let _ = stream.write_all(head.as_bytes());
    let _ = stream.write_all(body);
    let _ = stream.flush();
}

/// What the mock `/logs` endpoint streams for a test.
#[derive(Clone, Copy)]
enum LogScript {
    /// The message is split across two stdout frames on one connection.
    SplitAcrossFrames {
        first: &'static [u8],
        second: &'static [u8],
    },
    /// The message appears only on stderr; stdout is empty.
    StderrOnly { payload: &'static [u8] },
    /// `follow=true` serves two stdout frames on one connection; `follow=false`
    /// serves only the first frame, so a re-polling caller gets it repeatedly.
    FollowVsPoll {
        first: &'static [u8],
        second: &'static [u8],
    },
}

/// Serve one log response according to `script`, counting the frame bytes it
/// writes into `bytes`.
fn serve_logs(stream: &mut UnixStream, follow: bool, script: LogScript, bytes: &Arc<AtomicUsize>) {
    match script {
        LogScript::SplitAcrossFrames { first, second } => {
            let mut body = frame(1, first);
            body.extend_from_slice(&frame(1, second));
            bytes.fetch_add(body.len(), Ordering::Relaxed);
            write_response(stream, 200, &body);
        }
        LogScript::StderrOnly { payload } => {
            let body = frame(2, payload);
            bytes.fetch_add(body.len(), Ordering::Relaxed);
            write_response(stream, 200, &body);
        }
        LogScript::FollowVsPoll { first, second } => {
            let first_frame = frame(1, first);
            if !follow {
                bytes.fetch_add(first_frame.len(), Ordering::Relaxed);
                write_response(stream, 200, &first_frame);
                return;
            }
            let mut body = first_frame;
            body.extend_from_slice(&frame(1, second));
            bytes.fetch_add(body.len(), Ordering::Relaxed);
            write_response(stream, 200, &body);
        }
    }
}

/// Start a mock Docker daemon on `socket_path`, serving until `stop` is set.
fn mock_daemon(
    socket_path: &Path,
    stop: Arc<AtomicBool>,
    bytes: Arc<AtomicUsize>,
    script: LogScript,
) {
    mock_socket::serve(socket_path, stop, move |mut stream| {
        let Some(req) = mock_socket::read_request(&mut stream) else {
            return;
        };
        let (method, path) = (req.method.as_str(), req.target.as_str());

        // Version negotiation: the client asks once, unversioned, before its
        // first versioned request.
        if method == "GET" && path.starts_with("/version") {
            let body = r#"{"ApiVersion":"1.53","MinAPIVersion":"1.24"}"#;
            write_response(&mut stream, 200, body.as_bytes());
            return;
        }

        let path_no_query = path.split('?').next().unwrap_or(path);
        let p = path_no_query
            .split_once("/v1.")
            .map_or(path_no_query, |(_, rest)| {
                rest.split_once('/').map_or(rest, |(_, r)| r)
            });
        if method == "GET" && p.starts_with("containers/") && p.ends_with("/logs") {
            let follow = path.contains("follow=true");
            // A long-lived stream must not hold the accept loop.
            let bytes = bytes.clone();
            std::thread::spawn(move || serve_logs(&mut stream, follow, script, &bytes));
            return;
        }

        write_response(&mut stream, 404, b"{}");
    });
}

// ── tests ───────────────────────────────────────────────────────────────

/// The message spans two stdout frames: the rolling buffer must match across
/// the frame boundary.
#[valtron_test]
async fn wait_for_stdout_matches_across_frames() {
    let socket = temp_socket_path("across");
    let stop = Arc::new(AtomicBool::new(false));
    let bytes = Arc::new(AtomicUsize::new(0));
    mock_daemon(
        &socket,
        stop.clone(),
        bytes.clone(),
        LogScript::SplitAcrossFrames {
            first: b"hello ",
            second: b"world",
        },
    );
    let client = DockerClient::connect_unix(&socket);

    wait_for_stdout(
        &client,
        "abc123",
        "hello world",
        Duration::from_secs(5),
        &foundation_core::valtron::MonotonicClock::new(),
    )
    .await
    .expect("a message split across two frames should be found");

    mock_socket::stop(&socket, &stop);
}

/// The message arrives only on stderr: only stdout bytes count, so the wait
/// must time out instead of matching.
#[valtron_test]
async fn wait_for_stdout_ignores_stderr() {
    let socket = temp_socket_path("stderr");
    let stop = Arc::new(AtomicBool::new(false));
    let bytes = Arc::new(AtomicUsize::new(0));
    mock_daemon(
        &socket,
        stop.clone(),
        bytes.clone(),
        LogScript::StderrOnly {
            payload: b"hello world",
        },
    );
    let client = DockerClient::connect_unix(&socket);

    let result = wait_for_stdout(
        &client,
        "abc123",
        "hello world",
        Duration::from_secs(5),
        &foundation_core::valtron::MonotonicClock::new(),
    )
    .await;
    assert!(
        matches!(result, Err(ref e) if matches!(e.current_context(), DockerError::WaitTimeout { .. })),
        "a stderr-only message must not match, got: {result:?}"
    );

    mock_socket::stop(&socket, &stop);
}

/// One follow request reads the stream once. The mock serves `hello` + `world`
/// (26 bytes) to a follow request but only `hello` (13 bytes) to a poll
/// request; the total must be the single serving, not a multiple of 13.
#[valtron_test]
async fn wait_for_stdout_reads_each_byte_once() {
    let socket = temp_socket_path("once");
    let stop = Arc::new(AtomicBool::new(false));
    let bytes = Arc::new(AtomicUsize::new(0));
    mock_daemon(
        &socket,
        stop.clone(),
        bytes.clone(),
        LogScript::FollowVsPoll {
            first: b"hello",
            second: b"world",
        },
    );
    let client = DockerClient::connect_unix(&socket);

    wait_for_stdout(
        &client,
        "abc123",
        "world",
        Duration::from_secs(5),
        &foundation_core::valtron::MonotonicClock::new(),
    )
    .await
    .expect("the message should be found in the followed stream");

    mock_socket::stop(&socket, &stop);

    let expected = frame(1, b"hello").len() + frame(1, b"world").len();
    assert_eq!(
        bytes.load(Ordering::Relaxed),
        expected,
        "one follow request reads each byte once, not repeated polls"
    );
}
