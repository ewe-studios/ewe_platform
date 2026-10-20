//! Container stats over a mock Docker daemon on a Unix socket.
//!
//! WHY: With `stream=true` (dockerd's default) `/containers/{id}/stats` never
//! ends: the daemon writes one stats object per line for as long as the
//! container runs. A client that reads the body whole before parsing it never
//! returns, and holds every sample it is sent. These tests prove the client
//! reads the feed one typed sample at a time, closes it when dropped, reads a
//! one-shot sample from a single line, and checks the status first.
//!
//! WHAT: A `UnixListener` HTTP mock daemon (one thread per connection, so an
//! endless feed does not hold up the next request) that answers `GET
//! /version` and `GET /containers/{id}/stats` per container id:
//!
//! - `one` — a single line (`stream=false`);
//! - `live` — an endless chunked feed, one line per chunk, 50ms apart; the
//!   mock reports, over a channel, the moment a write fails (the client went
//!   away);
//! - `bad` — a good line, a malformed line, then another good line;
//! - `empty` — `200` with no body;
//! - `missing` — `404` with dockerd's `{"message": …}` body;
//! - `huge-error` — `500` with a 256 MiB body.
//!
//! HOW: `#[valtron_test]` async fns drive `DockerClient::container_stats` and
//! `DockerClient::container_stats_stream`.

#![cfg(all(unix, feature = "docker"))]

use std::io::{Read, Write};
use std::os::unix::net::{UnixListener, UnixStream};
use std::path::PathBuf;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{mpsc, Arc, Mutex};
use std::time::{Duration, Instant};

use foundation_core::valtron::valtron_test;
use foundation_deployment_docker::DockerLimits;
use foundation_deployment_docker::generated::stats::ContainerStatsResponse;
use foundation_deployment_docker::{DockerClient, DockerError};

/// The gap between two samples of the `live` feed.
const LIVE_SAMPLE_GAP: Duration = Duration::from_millis(50);

/// How long the `live` feed runs at most if the client never goes away (so a
/// failing test does not leave a writer thread behind for ever).
const LIVE_FEED_CAP: Duration = Duration::from_secs(120);

/// The size of the `huge-error` body.
const HUGE_ERROR_BODY: usize = 256 * 1024 * 1024;

fn temp_socket_path(tag: &str) -> PathBuf {
    let pid = std::process::id();
    let nanos = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map_or(0, |d| d.as_nanos());
    let path = std::env::temp_dir().join(format!("ewe_docker_stats_{tag}_{pid}_{nanos}.sock"));
    let _ = std::fs::remove_file(&path);
    path
}

/// One stats sample as dockerd writes it (cgroup v2 host, one line), for
/// container `id`; `n` advances the counters from sample to sample.
fn stats_line(id: &str, n: u64) -> String {
    let total = 1_000_000_000 + n * 25_000_000;
    let system = 9_000_000_000_000 + n * 1_000_000_000;
    let sample = serde_json::json!({
        "name": "/web",
        "id": id,
        "os_type": "linux",
        "read": format!("2026-10-19T18:30:{:02}.123456789Z", n % 60),
        "preread": format!("2026-10-19T18:30:{:02}.023456789Z", n % 60),
        "num_procs": 0,
        "pids_stats": { "current": 7, "limit": 4_194_304 },
        "blkio_stats": {
            "io_service_bytes_recursive": [
                { "major": 254, "minor": 0, "op": "read", "value": 4_096 * n },
                { "major": 254, "minor": 0, "op": "write", "value": 0 }
            ],
            "io_serviced_recursive": null,
            "io_queue_recursive": null,
            "io_service_time_recursive": null,
            "io_wait_time_recursive": null,
            "io_merged_recursive": null,
            "io_time_recursive": null,
            "sectors_recursive": null
        },
        "storage_stats": {},
        "cpu_stats": {
            "cpu_usage": {
                "total_usage": total,
                "usage_in_kernelmode": total / 3,
                "usage_in_usermode": total - total / 3
            },
            "system_cpu_usage": system,
            "online_cpus": 4,
            "throttling_data": { "periods": 0, "throttled_periods": 0, "throttled_time": 0 }
        },
        "precpu_stats": {
            "cpu_usage": {
                "total_usage": total - 25_000_000,
                "usage_in_kernelmode": (total - 25_000_000) / 3,
                "usage_in_usermode": (total - 25_000_000) - (total - 25_000_000) / 3
            },
            "system_cpu_usage": system - 1_000_000_000,
            "online_cpus": 4,
            "throttling_data": { "periods": 0, "throttled_periods": 0, "throttled_time": 0 }
        },
        "memory_stats": {
            "usage": 12_582_912,
            "limit": 8_338_403_328_u64,
            "stats": {
                "active_anon": 4_096,
                "active_file": 1_048_576,
                "anon": 8_388_608,
                "file": 3_145_728,
                "inactive_anon": 8_384_512,
                "inactive_file": 2_097_152,
                "kernel_stack": 114_688
            }
        },
        "networks": {
            "eth0": {
                "rx_bytes": 1_296 + n * 90,
                "rx_packets": 16 + n,
                "rx_errors": 0,
                "rx_dropped": 0,
                "tx_bytes": 0,
                "tx_packets": 0,
                "tx_errors": 0,
                "tx_dropped": 0
            }
        }
    });
    let mut line = sample.to_string();
    line.push('\n');
    line
}

/// What the mock daemon reports to a test.
#[derive(Clone)]
struct Daemon {
    socket: PathBuf,
    stop: Arc<AtomicBool>,
    /// Request lines (`GET /v1.53/…?…`) as the daemon received them.
    requests: Arc<Mutex<Vec<String>>>,
    /// Sends the number of samples the `live` feed had written when a write
    /// failed: the client closed the connection.
    client_gone: Arc<Mutex<mpsc::Sender<u64>>>,
}

impl Daemon {
    /// Start a mock daemon; the receiver hears from the `live` feed.
    fn start(tag: &str) -> (Self, mpsc::Receiver<u64>) {
        let socket = temp_socket_path(tag);
        let (gone_tx, gone_rx) = mpsc::channel();
        let daemon = Self {
            socket,
            stop: Arc::new(AtomicBool::new(false)),
            requests: Arc::new(Mutex::new(Vec::new())),
            client_gone: Arc::new(Mutex::new(gone_tx)),
        };
        let listener = UnixListener::bind(&daemon.socket).expect("bind mock docker socket");
        listener.set_nonblocking(true).expect("set nonblocking");
        let accept = daemon.clone();
        std::thread::spawn(move || {
            while !accept.stop.load(Ordering::Relaxed) {
                match listener.accept() {
                    Ok((stream, _)) => {
                        let conn = accept.clone();
                        // One thread per connection: an endless feed must not
                        // hold up the next request.
                        std::thread::spawn(move || conn.serve(stream));
                    }
                    Err(ref e) if e.kind() == std::io::ErrorKind::WouldBlock => {
                        std::thread::sleep(Duration::from_millis(1));
                    }
                    Err(_) => break,
                }
            }
        });
        (daemon, gone_rx)
    }

    fn client(&self) -> DockerClient {
        DockerClient::connect_unix(&self.socket)
    }

    /// The stats requests received so far (`/version` left out).
    fn stats_requests(&self) -> Vec<String> {
        self.requests
            .lock()
            .expect("requests lock")
            .iter()
            .filter(|r| r.contains("/stats"))
            .cloned()
            .collect()
    }

    fn shutdown(&self) {
        self.stop.store(true, Ordering::Relaxed);
        let _ = std::fs::remove_file(&self.socket);
    }

    /// Answer one request on `stream`.
    fn serve(&self, mut stream: UnixStream) {
        stream.set_nonblocking(false).expect("blocking connection");
        let mut buf = [0u8; 8192];
        let n = stream.read(&mut buf).unwrap_or(0);
        let req = String::from_utf8_lossy(&buf[..n]);
        let first = req.lines().next().unwrap_or("").to_string();
        let mut parts = first.split_whitespace();
        let method = parts.next().unwrap_or("").to_string();
        let target = parts.next().unwrap_or("").to_string();
        self.requests
            .lock()
            .expect("requests lock")
            .push(first.clone());

        let path = target.split('?').next().unwrap_or("");
        if method == "GET" && path == "/version" {
            respond(
                &mut stream,
                200,
                r#"{"ApiVersion":"1.53","MinAPIVersion":"1.24"}"#,
            );
            return;
        }
        let container = path
            .strip_suffix("/stats")
            .and_then(|p| p.rsplit_once("/containers/"))
            .map(|(_, id)| id);
        match (method.as_str(), container) {
            ("GET", Some("one")) => respond(&mut stream, 200, &stats_line("one", 1)),
            ("GET", Some("empty")) => respond(&mut stream, 200, ""),
            ("GET", Some("missing")) => respond(
                &mut stream,
                404,
                r#"{"message":"No such container: missing"}"#,
            ),
            ("GET", Some("live")) => self.live_feed(&mut stream),
            ("GET", Some("bad")) => {
                start_chunked(&mut stream);
                let _ = write_chunk(&mut stream, stats_line("bad", 1).as_bytes());
                let _ = write_chunk(
                    &mut stream,
                    b"{\"id\":\"bad\",\"cpu_stats\":{\"cpu_usage\":\n",
                );
                let _ = write_chunk(&mut stream, stats_line("bad", 3).as_bytes());
                let _ = stream.write_all(b"0\r\n\r\n");
            }
            ("GET", Some("huge-error")) => huge_error(&mut stream),
            _ => respond(
                &mut stream,
                404,
                &format!("{{\"message\":\"no route for {method} {target}\"}}"),
            ),
        }
    }

    /// The endless feed: a sample per chunk, [`LIVE_SAMPLE_GAP`] apart, until
    /// a write fails — then report how many samples were written.
    fn live_feed(&self, stream: &mut UnixStream) {
        start_chunked(stream);
        let started = Instant::now();
        let mut written = 0;
        while started.elapsed() < LIVE_FEED_CAP {
            if write_chunk(stream, stats_line("live", written).as_bytes()).is_err() {
                let _ = self.client_gone.lock().expect("gone lock").send(written);
                return;
            }
            written += 1;
            std::thread::sleep(LIVE_SAMPLE_GAP);
        }
    }
}

fn respond(stream: &mut UnixStream, status: u16, body: &str) {
    let reason = match status {
        200 => "OK",
        404 => "Not Found",
        _ => "Error",
    };
    let response = format!(
        "HTTP/1.1 {status} {reason}\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{body}",
        body.len(),
    );
    let _ = stream.write_all(response.as_bytes());
    let _ = stream.flush();
}

fn start_chunked(stream: &mut UnixStream) {
    let _ = stream.write_all(
        b"HTTP/1.1 200 OK\r\nContent-Type: application/json\r\nTransfer-Encoding: chunked\r\nConnection: close\r\n\r\n",
    );
    let _ = stream.flush();
}

fn write_chunk(stream: &mut UnixStream, data: &[u8]) -> std::io::Result<()> {
    stream.write_all(format!("{:x}\r\n", data.len()).as_bytes())?;
    stream.write_all(data)?;
    stream.write_all(b"\r\n")?;
    stream.flush()
}

/// A `500` whose body is far larger than any error message: the client must
/// answer from the status and a bounded read.
fn huge_error(stream: &mut UnixStream) {
    let head = format!(
        "HTTP/1.1 500 Internal Server Error\r\nContent-Length: {HUGE_ERROR_BODY}\r\nConnection: close\r\n\r\n"
    );
    if stream.write_all(head.as_bytes()).is_err() {
        return;
    }
    let block = vec![b'e'; 64 * 1024];
    let mut written = 0;
    while written < HUGE_ERROR_BODY {
        let n = block.len().min(HUGE_ERROR_BODY - written);
        // Fails once the client has read what it wants and hung up.
        if stream.write_all(&block[..n]).is_err() {
            return;
        }
        written += n;
    }
}

fn total_usage(sample: &ContainerStatsResponse) -> Option<i64> {
    sample.cpu_stats.as_ref()?.cpu_usage.as_ref()?.total_usage
}

// ── (a) one-shot ────────────────────────────────────────────────────────

/// `container_stats` asks for `stream=false` and returns the single line the
/// daemon answers with, as the generated type.
#[valtron_test]
async fn container_stats_reads_a_single_sample() {
    let (daemon, _gone) = Daemon::start("one");
    let client = daemon.client();

    let sample = client
        .container_stats("one", true)
        .await
        .expect("one sample");

    assert_eq!(sample.id.as_deref(), Some("one"));
    assert_eq!(sample.name.as_deref(), Some("/web"));
    assert_eq!(sample.os_type.as_deref(), Some("linux"));
    assert_eq!(total_usage(&sample), Some(1_025_000_000));
    assert_eq!(
        sample.cpu_stats.as_ref().and_then(|c| c.online_cpus),
        Some(4)
    );
    let memory = sample.memory_stats.as_ref().expect("memory_stats");
    assert_eq!(memory.usage, Some(12_582_912));
    assert_eq!(memory.limit, Some(8_338_403_328));
    assert_eq!(sample.pids_stats.as_ref().and_then(|p| p.current), Some(7));
    let io = sample
        .blkio_stats
        .as_ref()
        .and_then(|b| b.io_service_bytes_recursive.as_ref())
        .expect("io_service_bytes_recursive");
    assert_eq!(io[0].op.as_deref(), Some("read"));
    assert_eq!(
        sample
            .networks
            .as_ref()
            .map(|n| n["eth0"]["rx_packets"].clone()),
        Some(serde_json::json!(17))
    );

    let requests = daemon.stats_requests();
    assert_eq!(requests.len(), 1, "{requests:?}");
    assert!(
        requests[0].contains("stream=false") && requests[0].contains("one-shot=true"),
        "{}",
        requests[0]
    );

    daemon.shutdown();
}

/// A `200` with no line is not a sample: it is a malformed answer.
#[valtron_test]
async fn container_stats_with_an_empty_body_is_an_error() {
    let (daemon, _gone) = Daemon::start("empty");
    let client = daemon.client();

    match client.container_stats("empty", false).await {
        Err(DockerError::JsonParse(message)) => {
            assert!(message.contains("empty body"), "{message}");
        }
        other => panic!("expected JsonParse, got {other:?}"),
    }

    daemon.shutdown();
}

// ── (b) the endless feed ────────────────────────────────────────────────

/// `container_stats_stream` hands out typed samples as they arrive from a feed
/// that never ends, and dropping it closes the connection: the daemon's next
/// write fails.
///
/// Against the old `container_stats(stream=true)`, which read the whole body
/// before parsing it, this never gets its first sample: the body has no end.
#[valtron_test]
async fn container_stats_stream_yields_samples_until_dropped() {
    let (daemon, gone) = Daemon::start("live");
    let client = daemon.client();

    let mut stats = client
        .container_stats_stream("live")
        .await
        .expect("the feed starts");

    let first = stats.next().await.expect("a first sample").expect("valid");
    let second = stats.next().await.expect("a second sample").expect("valid");
    // Through the `futures_lite::Stream` impl as well.
    let third = futures_lite::StreamExt::next(&mut stats)
        .await
        .expect("a third sample")
        .expect("valid");

    for (n, sample) in [&first, &second, &third].into_iter().enumerate() {
        assert_eq!(sample.id.as_deref(), Some("live"));
        let n = i64::try_from(n).expect("small");
        assert_eq!(total_usage(sample), Some(1_000_000_000 + n * 25_000_000));
    }

    let requests = daemon.stats_requests();
    assert_eq!(requests.len(), 1, "{requests:?}");
    assert!(
        requests[0].contains("stream=true") && requests[0].contains("one-shot=false"),
        "{}",
        requests[0]
    );

    let dropped = Instant::now();
    drop(stats);
    let written = gone
        .recv_timeout(Duration::from_secs(5))
        .expect("the daemon's writes fail within 5s of the stream being dropped");
    assert!(written >= 3, "the daemon wrote {written} samples");
    assert!(
        dropped.elapsed() < Duration::from_secs(5),
        "{:?}",
        dropped.elapsed()
    );

    daemon.shutdown();
}

// ── (c) a malformed line ────────────────────────────────────────────────

/// A line that is not a stats object is `JsonParse`, and the stream ends there
/// even though the daemon sends a valid line after it.
#[valtron_test]
async fn a_malformed_sample_ends_the_stream_with_json_parse() {
    let (daemon, _gone) = Daemon::start("bad");
    let client = daemon.client();

    let mut stats = client
        .container_stats_stream("bad")
        .await
        .expect("the feed starts");

    let first = stats.next().await.expect("a first sample").expect("valid");
    assert_eq!(first.id.as_deref(), Some("bad"));
    match stats.next().await {
        Some(Err(DockerError::JsonParse(_))) => {}
        other => panic!("expected JsonParse, got {other:?}"),
    }
    assert!(
        stats.next().await.is_none(),
        "the stream ends after the error"
    );

    daemon.shutdown();
}

// ── (d) the status comes first ──────────────────────────────────────────

/// A `404` is an error from the call itself — no stream is handed out — with
/// the daemon's message, for both the feed and the single sample.
#[valtron_test]
async fn a_non_2xx_status_is_an_error_before_the_body() {
    let (daemon, _gone) = Daemon::start("missing");
    let client = daemon.client();

    match client.container_stats_stream("missing").await {
        Err(DockerError::Api { status, message }) => {
            assert_eq!(status, 404);
            assert_eq!(message, "No such container: missing");
        }
        other => panic!("expected Api, got {other:?}"),
    }
    match client.container_stats("missing", false).await {
        Err(DockerError::Api { status, message }) => {
            assert_eq!(status, 404);
            assert_eq!(message, "No such container: missing");
        }
        other => panic!("expected Api, got {other:?}"),
    }

    daemon.shutdown();
}

/// However large an error body is, only a bounded prefix of it is read.
#[valtron_test]
async fn a_huge_error_body_is_not_read_whole() {
    let (daemon, _gone) = Daemon::start("huge_error");
    let client = daemon.client();

    let started = Instant::now();
    match client.container_stats_stream("huge-error").await {
        Err(DockerError::Api { status, message }) => {
            assert_eq!(status, 500);
            assert!(
                message.len() <= DockerLimits::default().max_error_body,
                "read {} bytes of the error body; at most {} should be",
                message.len(),
                DockerLimits::default().max_error_body
            );
        }
        other => panic!("expected Api, got {other:?}"),
    }
    assert!(
        started.elapsed() < Duration::from_secs(30),
        "the {HUGE_ERROR_BODY}-byte error body was read rather than abandoned"
    );

    daemon.shutdown();
}

/// A client configured with a smaller line cap refuses a sample longer than
/// it, as a parse error that ends the stream.
#[valtron_test]
async fn a_configured_line_cap_is_the_one_enforced() {
    let (daemon, _gone) = Daemon::start("line_cap");
    let client = daemon
        .client()
        .with_limits(DockerLimits::default().with_max_json_line(16));

    match client.container_stats("one", false).await {
        Err(DockerError::JsonParse(m)) => {
            assert!(m.contains("longer than 16 bytes"), "{m}");
        }
        other => panic!("expected JsonParse, got {other:?}"),
    }
    // The default cap reads the same sample.
    daemon
        .client()
        .container_stats("one", false)
        .await
        .expect("a sample under the default cap");

    daemon.shutdown();
}
