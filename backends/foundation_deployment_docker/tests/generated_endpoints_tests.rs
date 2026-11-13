//! Mock-daemon tests for the Docker client methods that used to build their
//! requests by hand.
//!
//! WHY: Feature 12 routes seven methods through the generated request/response
//! functions. These tests prove the new behaviour over the Unix-socket mock:
//! a typed `system_data_usage`, a streamed (never whole-buffered) tar upload,
//! a capped exec-inspect error body, and the HEAD archive-info stat header.
//!
//! WHAT: A mock daemon that answers the plain endpoints with canned JSON, plus
//! a second mock that reads a chunked upload body while counting the bytes.
//!
//! HOW: `mock_socket::serve` + `#[valtron_test]`, the same pattern as
//! `exec_system_tests.rs`.

#![cfg(all(unix, feature = "docker"))]

mod mock_socket;

use std::io::{Read, Write};
use std::os::unix::net::UnixStream;
use std::path::PathBuf;
use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};
use std::sync::Arc;

use foundation_core::io::readers::Data;
use foundation_core::valtron::valtron_test;
use foundation_deployment_docker::{DockerClient, DockerError, DockerLimits};
use foundation_netio::shared::http::SendSafeBody;

/// One chunk of the generated tar body: 64 KiB, far below the 8 MiB the test
/// allows to be in flight at once.
const UPLOAD_CHUNK: usize = 64 * 1024;
/// The generated tar is 64 MiB (1024 chunks of 64 KiB).
const UPLOAD_CHUNKS: usize = 1024;
/// The peak bytes the test allows in flight — the stream must not be buffered
/// whole.
const UPLOAD_PEAK_CAP: usize = 8 * 1024 * 1024;

fn temp_socket_path(tag: &str) -> PathBuf {
    let pid = std::process::id();
    let nanos = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map_or(0, |d| d.as_nanos());
    let path =
        std::path::PathBuf::from("/tmp").join(format!("ewe_docker_gen_{tag}_{pid}_{nanos}.sock"));
    let _ = std::fs::remove_file(&path);
    path
}

/// `Some(i)` where `haystack[i..]` starts with `needle`.
fn find_sub(haystack: &[u8], needle: &[u8]) -> Option<usize> {
    haystack.windows(needle.len()).position(|w| w == needle)
}

/// (status, extra header lines, body). `extra` is `"Name: value\r\n"` lines
/// (empty when there are none). The version prefix is stripped first.
fn route(method: &str, path: &str) -> (u16, String, String) {
    let path_no_query = path.split('?').next().unwrap_or(path);
    let p = path_no_query
        .split_once("/v1.")
        .map_or(path_no_query, |(_, rest)| {
            rest.split_once('/').map_or(rest, |(_, r)| r)
        });
    match (method, p) {
        ("GET", "/version") => (
            200,
            String::new(),
            r#"{"ApiVersion":"1.53","MinAPIVersion":"1.24"}"#.to_string(),
        ),
        ("GET", "system/df") => (
            200,
            String::new(),
            r#"{"ImageUsage":{"TotalSize":123}}"#.to_string(),
        ),
        // A large error body so the cap is observable: 4 KiB of `x`.
        ("GET", "exec/bad/json") => (
            500,
            String::new(),
            format!(r#"{{"message":"{}"}}"#, "x".repeat(4 * 1024)),
        ),
        ("HEAD", "containers/abc/archive") => (
            200,
            "X-Docker-Container-Path-Stat: e30=\r\n".to_string(),
            String::new(),
        ),
        _ => (
            404,
            String::new(),
            format!(r#"{{"message":"no route for {method} {p}"}}"#),
        ),
    }
}

/// Write one HTTP/1.1 response with the given status, extra headers and body.
fn write_response(stream: &mut UnixStream, status: u16, extra: &str, body: &str) {
    let reason = match status {
        200 => "OK",
        500 => "Internal Server Error",
        _ => "Not Found",
    };
    let head = format!(
        "HTTP/1.1 {status} {reason}\r\nContent-Type: application/json\r\nContent-Length: {}\r\n{extra}Connection: close\r\n\r\n",
        body.len()
    );
    let _ = stream.write_all(head.as_bytes());
    if !body.is_empty() {
        let _ = stream.write_all(body.as_bytes());
    }
    let _ = stream.flush();
}

/// A mock daemon for the plain (Content-Length / no-body) endpoints.
fn mock_daemon(socket_path: PathBuf, stop: Arc<AtomicBool>) {
    mock_socket::serve(socket_path, stop, move |mut stream| {
        let Some(req) = mock_socket::read_request(&mut stream) else {
            return;
        };
        let (status, extra, body) = route(&req.method, &req.target);
        write_response(&mut stream, status, &extra, &body);
    });
}

/// A mock daemon that answers `GET /version` and reads one chunked
/// `PUT /containers/{id}/archive` body, counting the payload bytes it reads.
///
/// The chunked read is what keeps the upload honest: the client must stream
/// the body over the wire, and `consumed` records how much the daemon read.
fn upload_mock_daemon(socket_path: PathBuf, stop: Arc<AtomicBool>, consumed: Arc<AtomicUsize>) {
    mock_socket::serve(socket_path, stop, move |mut stream| {
        // Read the request head (through the blank line).
        let mut buf = Vec::new();
        let mut chunk = [0u8; 8192];
        let head_end = loop {
            if let Some(i) = find_sub(&buf, b"\r\n\r\n") {
                break i + 4;
            }
            let Ok(n) = stream.read(&mut chunk) else {
                return;
            };
            if n == 0 {
                return;
            }
            buf.extend_from_slice(&chunk[..n]);
        };
        let head = String::from_utf8_lossy(&buf[..head_end]).into_owned();
        let mut lines = head.lines();
        let Some(request_line) = lines.next() else {
            return;
        };
        let mut parts = request_line.split_whitespace();
        let (method, target) = (parts.next(), parts.next());
        let (Some(method), Some(target)) = (method, target) else {
            return;
        };

        if method == "GET" && target == "/version" {
            write_response(
                &mut stream,
                200,
                "",
                r#"{"ApiVersion":"1.53","MinAPIVersion":"1.24"}"#,
            );
            return;
        }

        if method != "PUT" {
            write_response(&mut stream, 404, "", r#"{"message":"method not handled"}"#);
            return;
        }

        // Answer `Expect: 100-continue` before the body, if asked.
        if head.to_ascii_lowercase().contains("expect: 100-continue") {
            let _ = stream.write_all(b"HTTP/1.1 100 Continue\r\n\r\n");
            let _ = stream.flush();
        }

        // The bytes after the head are the start of the chunked body.
        let mut rest = buf[head_end..].to_vec();
        let mut total = 0usize;
        loop {
            let line_end = loop {
                if let Some(i) = find_sub(&rest, b"\r\n") {
                    break i;
                }
                let Ok(n) = stream.read(&mut chunk) else {
                    return;
                };
                if n == 0 {
                    return;
                }
                rest.extend_from_slice(&chunk[..n]);
            };
            let line = String::from_utf8_lossy(&rest[..line_end]).into_owned();
            let size = usize::from_str_radix(line.trim(), 16).unwrap_or(0);
            rest.drain(..line_end + 2);
            if size == 0 {
                break;
            }
            while rest.len() < size + 2 {
                let Ok(n) = stream.read(&mut chunk) else {
                    return;
                };
                if n == 0 {
                    return;
                }
                rest.extend_from_slice(&chunk[..n]);
            }
            total += size;
            consumed.fetch_add(size, Ordering::SeqCst);
            rest.drain(..size + 2);
        }
        let _ = total;
        write_response(&mut stream, 200, "", "");
    });
}

// ── system_data_usage ───────────────────────────────────────────────────

#[valtron_test]
async fn system_data_usage_returns_typed_struct() {
    // WHY: Proves `system_data_usage` returns the generated
    // `SystemDataUsageResponse`, not untyped JSON.
    let socket = temp_socket_path("df");
    let stop = Arc::new(AtomicBool::new(false));
    mock_daemon(socket.clone(), stop.clone());
    let client = DockerClient::connect_unix(&socket);

    let usage = client
        .system_data_usage(None)
        .await
        .expect("system_data_usage should succeed");
    assert_eq!(
        usage.image_usage.and_then(|i| i.total_size),
        Some(123),
        "typed ImageUsage.TotalSize"
    );

    mock_socket::stop(&socket, &stop);
}

// ── upload_to_container streams, never buffers whole ────────────────────

#[valtron_test]
async fn upload_to_container_streams_without_buffering() {
    // WHY: Proves `upload_to_container` sends the tar as a body stream, so the
    // 64 MiB archive is never held whole in memory.
    let socket = temp_socket_path("upload");
    let stop = Arc::new(AtomicBool::new(false));
    let consumed = Arc::new(AtomicUsize::new(0));
    let produced = Arc::new(AtomicUsize::new(0));
    let peak_in_flight = Arc::new(AtomicUsize::new(0));

    upload_mock_daemon(socket.clone(), stop.clone(), Arc::clone(&consumed));

    let client = DockerClient::connect_unix(&socket);

    // The body source: 1024 chunks of 64 KiB, counted as they are yielded.
    let body = {
        let produced = Arc::clone(&produced);
        let iter = (0..UPLOAD_CHUNKS).map(move |_| {
            produced.fetch_add(1, Ordering::SeqCst);
            Ok(Data::Bytes(vec![0u8; UPLOAD_CHUNK]))
        });
        SendSafeBody::Stream(Some(Box::new(iter)))
    };

    // Update the peak in-flight estimate as the daemon reads: bytes the source
    // has yielded minus bytes the daemon has read.
    let watcher_peak = Arc::clone(&peak_in_flight);
    let watcher_produced = Arc::clone(&produced);
    let watcher_consumed = Arc::clone(&consumed);
    let watcher = std::thread::spawn(move || {
        for _ in 0..20_000 {
            let produced_now = watcher_produced.load(Ordering::SeqCst);
            let consumed_now = watcher_consumed.load(Ordering::SeqCst);
            let in_flight = produced_now
                .saturating_mul(UPLOAD_CHUNK)
                .saturating_sub(consumed_now);
            watcher_peak.fetch_max(in_flight, Ordering::SeqCst);
            std::thread::sleep(std::time::Duration::from_micros(100));
        }
    });

    client
        .upload_to_container("abc", "/tmp", body, Some(false), Some(false))
        .await
        .expect("upload_to_container should succeed");
    watcher.join().expect("watcher thread");

    assert_eq!(
        produced.load(Ordering::SeqCst),
        UPLOAD_CHUNKS,
        "every chunk of the tar was streamed"
    );
    assert_eq!(
        consumed.load(Ordering::SeqCst),
        UPLOAD_CHUNKS * UPLOAD_CHUNK,
        "the daemon read the full tar"
    );
    assert!(
        peak_in_flight.load(Ordering::SeqCst) < UPLOAD_PEAK_CAP,
        "peak bytes in flight {} must stay under {}",
        peak_in_flight.load(Ordering::SeqCst),
        UPLOAD_PEAK_CAP
    );

    mock_socket::stop(&socket, &stop);
}

// ── exec_inspect error body is capped and reported ──────────────────────

#[valtron_test]
async fn exec_inspect_error_body_is_capped_and_reported() {
    // WHY: Proves a non-2xx exec-inspect answer is reported with its error
    // body read up to the configured cap, never the whole body.
    let socket = temp_socket_path("exec_err");
    let stop = Arc::new(AtomicBool::new(false));
    mock_daemon(socket.clone(), stop.clone());
    let client = DockerClient::connect_unix(&socket)
        .with_limits(DockerLimits::default().with_max_error_body(64));

    let err = client
        .exec_inspect("bad")
        .await
        .expect_err("a 500 answer must be an error");
    match err {
        DockerError::Api { status, message } => {
            assert_eq!(status, 500);
            assert!(
                message.len() <= 64,
                "the error body must be cut at the 64-byte cap, got {} bytes",
                message.len()
            );
            assert!(
                message.len() < 4 * 1024,
                "the whole 4 KiB error body must not be reported"
            );
        }
        other => panic!("expected DockerError::Api, got {other:?}"),
    }

    mock_socket::stop(&socket, &stop);
}

// ── get_container_archive_info ──────────────────────────────────────────

#[valtron_test]
async fn get_container_archive_info_returns_stat_header() {
    // WHY: Proves the HEAD archive-info method reads the
    // `X-Docker-Container-Path-Stat` header from the generated request's
    // response.
    let socket = temp_socket_path("archinfo");
    let stop = Arc::new(AtomicBool::new(false));
    mock_daemon(socket.clone(), stop.clone());
    let client = DockerClient::connect_unix(&socket);

    let stat = client
        .get_container_archive_info("abc", "/etc/hostname")
        .await
        .expect("archive info should succeed");
    assert_eq!(stat, "e30=");

    mock_socket::stop(&socket, &stop);
}
