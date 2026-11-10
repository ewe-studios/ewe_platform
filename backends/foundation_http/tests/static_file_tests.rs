//! Spec 70 feature 41 — the static file handler writes every byte once, in
//! order, and does not hold a worker while a slow client downloads.
//!
//! The server runs for real on loopback with a small valtron pool, so a
//! handler that blocks or sleeps on the worker shows up as a late answer to a
//! second request.

#![cfg(feature = "multi")]

use std::io::{Read, Write};
use std::net::TcpStream;
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use foundation_core::io::ioutils::SharedByteBufferStream;
use foundation_core::valtron::initialize_pool;
use foundation_http::native::handlers::static_asset::StaticAssetHandler;
use foundation_http::native::handlers::static_file::{StaticFileConfig, StaticFileHandler};
use foundation_http::native::serve::Serve;
use foundation_http::native::server::{HttpServer, KeepAliveConfig, ServerConfig};
use foundation_http::native::writer::{send_bytes, send_text};
use foundation_http::shared::app::HttpApp;
use foundation_http::shared::context::Ctx;
use foundation_http::shared::serve::{ConnectionResult, ServeOutcome};
use foundation_http::ShutdownSignal;
use foundation_netio::netcap::RawStream;
use foundation_netio::shared::http::SimpleIncomingRequest;
use serial_test::serial;
use sha2::{Digest, Sha256};

// ---------------------------------------------------------------------------
// Fixtures

/// A temporary directory removed on drop.
struct TempDir(PathBuf);

impl TempDir {
    fn new(tag: &str) -> Self {
        let dir = std::env::temp_dir().join(format!(
            "static_file_tests_{tag}_{}_{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .map_or(0, |d| d.as_nanos())
        ));
        std::fs::create_dir_all(&dir).expect("create temp dir");
        Self(dir)
    }

    fn path(&self) -> &Path {
        &self.0
    }
}

impl Drop for TempDir {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}

/// `len` bytes of a pattern that no shifted or repeated copy can match: every
/// 8-byte word is its own index, so a duplicated or lost range changes the hash.
fn pattern(len: usize) -> Vec<u8> {
    let mut out = Vec::with_capacity(len);
    let mut i: u64 = 0;
    while out.len() < len {
        out.extend_from_slice(&i.wrapping_mul(0x9E37_79B9_7F4A_7C15).to_le_bytes());
        i += 1;
    }
    out.truncate(len);
    out
}

fn sha256(bytes: &[u8]) -> Vec<u8> {
    Sha256::digest(bytes).to_vec()
}

/// What the wrapping handler saw: when the handler finished and how.
#[derive(Default)]
struct Observed {
    finished_at: Option<Instant>,
    closed: bool,
}

/// Runs the real `StaticFileHandler` and records when its work ends, whether
/// it finished at once (`serve` returned) or through an `Async` future.
struct Observing {
    inner: StaticFileHandler,
    seen: Arc<Mutex<Observed>>,
}

impl Serve for Observing {
    fn serve(
        &self,
        ctx: Ctx,
        req: SimpleIncomingRequest,
        conn: SharedByteBufferStream<RawStream>,
    ) -> ConnectionResult {
        let seen = self.seen.clone();
        let record = move |outcome: &ServeOutcome| {
            let mut s = seen.lock().expect("lock");
            s.finished_at = Some(Instant::now());
            s.closed = matches!(outcome, ServeOutcome::Close(_));
        };
        match self.inner.serve(ctx, req, conn).split() {
            Ok(fut) => ConnectionResult::Async(Box::pin(async move {
                let outcome = fut.await;
                record(&outcome);
                outcome
            })),
            Err(outcome) => {
                record(&outcome);
                outcome.into()
            }
        }
    }
}

struct Harness {
    addr: std::net::SocketAddr,
    shutdown: Arc<ShutdownSignal>,
    done_rx: std::sync::mpsc::Receiver<()>,
    seen: Arc<Mutex<Observed>>,
}

impl Harness {
    fn stop(&self) {
        self.shutdown.turn_on();
        let _ = self.done_rx.recv_timeout(Duration::from_secs(3));
    }
}

/// Serve `root` on `/*` (observed) and `GET /ping` on the same server.
fn start_server(root: &Path, config: StaticFileConfig) -> Harness {
    start_server_with(root, config, |_| {})
}

/// As [`start_server`], with `extra` adding more routes.
fn start_server_with(
    root: &Path,
    config: StaticFileConfig,
    extra: impl FnOnce(&mut HttpApp<Arc<dyn Serve>>),
) -> Harness {
    let seen = Arc::new(Mutex::new(Observed::default()));
    let files: Arc<dyn Serve> = Arc::new(Observing {
        inner: StaticFileHandler::with_config(root, config),
        seen: seen.clone(),
    });
    let ping: Arc<dyn Serve> = Arc::new(Ping);

    let mut app = HttpApp::new_serve();
    app.router.add_route_any("/*", &files);
    app.router.add_route_any("/ping", &ping);
    extra(&mut app);

    let shutdown = Arc::new(ShutdownSignal::new());
    let listener = std::net::TcpListener::bind("127.0.0.1:0").expect("bind");
    let addr = listener.local_addr().expect("local_addr");
    let server_config = ServerConfig::defaults()
        .with_keep_alive(KeepAliveConfig::defaults().with_idle_timeout(Duration::from_secs(5)));
    let server = HttpServer::with_config(app, &format!("127.0.0.1:{}", addr.port()), server_config);

    let shutdown_thread = shutdown.clone();
    let (done_tx, done_rx) = std::sync::mpsc::channel();
    std::thread::spawn(move || {
        server.serve_with_listener(&listener, &shutdown_thread);
        let _ = done_tx.send(());
    });
    Harness {
        addr,
        shutdown,
        done_rx,
        seen,
    }
}

struct Ping;

impl Serve for Ping {
    fn serve(
        &self,
        _ctx: Ctx,
        _req: SimpleIncomingRequest,
        conn: SharedByteBufferStream<RawStream>,
    ) -> ConnectionResult {
        send_text(conn, 200, "pong")
    }
}

/// Send `GET path` and read until the end of the head. Returns the head text
/// and any body bytes read past it.
fn request_head(stream: &mut TcpStream, path: &str) -> (String, Vec<u8>) {
    stream
        .write_all(format!("GET {path} HTTP/1.1\r\nHost: t\r\n\r\n").as_bytes())
        .expect("write request");
    stream
        .set_read_timeout(Some(Duration::from_secs(10)))
        .expect("read timeout");
    let mut acc = Vec::new();
    let mut buf = [0u8; 4096];
    loop {
        if let Some(pos) = acc.windows(4).position(|w| w == b"\r\n\r\n") {
            let head = String::from_utf8_lossy(&acc[..pos]).to_string();
            return (head, acc[pos + 4..].to_vec());
        }
        match stream.read(&mut buf) {
            Ok(0) => panic!("closed before the head ended: {acc:?}"),
            Ok(n) => acc.extend_from_slice(&buf[..n]),
            Err(e) => panic!("read head: {e}"),
        }
    }
}

fn content_length(head: &str) -> usize {
    head.lines()
        .find_map(|l| {
            let (k, v) = l.split_once(':')?;
            k.eq_ignore_ascii_case("content-length")
                .then(|| v.trim().parse().expect("content-length"))
        })
        .unwrap_or_else(|| panic!("no Content-Length in head: {head:?}"))
}

// ---------------------------------------------------------------------------
// Tests

/// A body larger than the socket send buffer, read by a slow client, arrives
/// byte for byte. A handler that restarts a partial write from byte 0 sends
/// duplicate bytes and fails the hash (or the length).
#[test]
#[foundation_macros::timeout(120_000)]
#[serial(http_test)]
fn static_file_large_body_is_byte_exact() {
    let _pool = initialize_pool(300, Some(2));
    let dir = TempDir::new("exact");
    let data = pattern(8 * 1024 * 1024);
    std::fs::write(dir.path().join("big.bin"), &data).expect("write file");
    let h = start_server(dir.path(), StaticFileConfig::new());

    let mut stream = TcpStream::connect(h.addr).expect("connect");
    let (head, mut body) = request_head(&mut stream, "/big.bin");
    assert!(head.starts_with("HTTP/1.1 200"), "head: {head:?}");
    let want = content_length(&head);
    assert_eq!(want, data.len(), "Content-Length must be the file size");

    // Slow reader: 16 KiB per read, 1 ms between reads, so the send buffer
    // fills and the server meets WouldBlock.
    let mut buf = vec![0u8; 16 * 1024];
    while body.len() < want {
        let n = stream.read(&mut buf).expect("read body");
        assert!(n > 0, "closed after {} of {want} bytes", body.len());
        body.extend_from_slice(&buf[..n]);
        std::thread::sleep(Duration::from_millis(1));
    }
    assert_eq!(body.len(), want, "received more bytes than Content-Length");
    assert_eq!(
        sha256(&body),
        sha256(&data),
        "body differs from the file (duplicated or missing bytes)"
    );
    h.stop();
}

/// While slow clients download a large file, a request to another route is
/// answered at once: the file writes do not occupy the workers. The smallest
/// valtron pool has two task workers, so four stalled downloads outnumber them.
#[test]
#[foundation_macros::timeout(120_000)]
#[serial(http_test)]
fn static_file_does_not_hold_worker() {
    let _pool = initialize_pool(301, Some(3));
    let dir = TempDir::new("worker");
    let file = std::fs::File::create(dir.path().join("huge.bin")).expect("create");
    file.set_len(64 * 1024 * 1024).expect("sparse file");
    drop(file);
    let h = start_server(dir.path(), StaticFileConfig::new());

    // Each slow client reads the head and then stalls: the server's socket
    // buffer fills and its write meets WouldBlock.
    let mut slow = Vec::new();
    for _ in 0..4 {
        let mut s = TcpStream::connect(h.addr).expect("connect slow");
        let (head, _) = request_head(&mut s, "/huge.bin");
        assert!(head.starts_with("HTTP/1.1 200"), "head: {head:?}");
        slow.push(s);
    }
    std::thread::sleep(Duration::from_millis(300));

    let t0 = Instant::now();
    let mut fast = TcpStream::connect(h.addr).expect("connect fast");
    fast.write_all(b"GET /ping HTTP/1.1\r\nHost: t\r\n\r\n")
        .expect("write ping");
    fast.set_read_timeout(Some(Duration::from_secs(10)))
        .expect("read timeout");
    let mut acc = Vec::new();
    let mut buf = [0u8; 1024];
    while !String::from_utf8_lossy(&acc).contains("pong") {
        let n = fast.read(&mut buf).expect("read ping");
        assert!(n > 0, "closed before the ping answer");
        acc.extend_from_slice(&buf[..n]);
    }
    let took = t0.elapsed();
    assert!(
        took < Duration::from_millis(500),
        "the second request took {took:?} while slow downloads ran"
    );

    drop(slow);
    h.stop();
}

/// A client that closes after the head ends the handler at once. The handler
/// does not retry a dead peer.
#[test]
#[foundation_macros::timeout(120_000)]
#[serial(http_test)]
fn static_file_peer_reset_closes_without_retry() {
    let _pool = initialize_pool(302, Some(2));
    let dir = TempDir::new("reset");
    let file = std::fs::File::create(dir.path().join("huge.bin")).expect("create");
    file.set_len(64 * 1024 * 1024).expect("sparse file");
    drop(file);
    let h = start_server(dir.path(), StaticFileConfig::new());

    let mut stream = TcpStream::connect(h.addr).expect("connect");
    let (head, _) = request_head(&mut stream, "/huge.bin");
    assert!(head.starts_with("HTTP/1.1 200"), "head: {head:?}");
    // Let the server fill the socket buffers and block on the write.
    std::thread::sleep(Duration::from_millis(300));

    let closed_at = Instant::now();
    drop(stream); // unread data in the receive buffer makes the kernel send RST

    let deadline = closed_at + Duration::from_secs(5);
    let finished_at = loop {
        if let Some(t) = h.seen.lock().expect("lock").finished_at {
            break t;
        }
        assert!(Instant::now() < deadline, "the handler never finished");
        std::thread::sleep(Duration::from_millis(5));
    };
    let took = finished_at.saturating_duration_since(closed_at);
    assert!(
        took < Duration::from_millis(200),
        "the handler took {took:?} to end after the peer closed"
    );
    assert!(
        h.seen.lock().expect("lock").closed,
        "a reset peer must close the connection"
    );
    h.stop();
}

/// Chunk size and write timeout come from `StaticFileConfig`: the builder sets
/// them, the defaults are documented values, and the handler obeys them.
#[test]
#[foundation_macros::timeout(120_000)]
#[serial(http_test)]
fn static_file_limits_come_from_config() {
    // Defaults: 64 KiB chunks (nginx output_buffers order), 60 s (nginx send_timeout).
    let default = StaticFileConfig::new();
    assert_eq!(default.chunk_size, 64 * 1024);
    assert_eq!(default.send_timeout, Duration::from_secs(60));

    let tuned = StaticFileConfig::new()
        .with_chunk_size(1000)
        .with_send_timeout(Duration::from_millis(400));
    assert_eq!(tuned.chunk_size, 1000);
    assert_eq!(tuned.send_timeout, Duration::from_millis(400));
    // A zero chunk size would never make progress: it is raised to 1.
    assert_eq!(StaticFileConfig::new().with_chunk_size(0).chunk_size, 1);

    let _pool = initialize_pool(303, Some(2));
    let dir = TempDir::new("config");
    let data = pattern(100_000);
    std::fs::write(dir.path().join("small.bin"), &data).expect("write file");
    let file = std::fs::File::create(dir.path().join("huge.bin")).expect("create");
    file.set_len(64 * 1024 * 1024).expect("sparse file");
    drop(file);
    let h = start_server(dir.path(), tuned);

    // A 1000-byte chunk size still serves the file byte for byte.
    let mut stream = TcpStream::connect(h.addr).expect("connect");
    let (head, mut body) = request_head(&mut stream, "/small.bin");
    let want = content_length(&head);
    let mut buf = [0u8; 8192];
    while body.len() < want {
        let n = stream.read(&mut buf).expect("read body");
        assert!(n > 0, "closed early");
        body.extend_from_slice(&buf[..n]);
    }
    assert_eq!(sha256(&body), sha256(&data), "chunked body differs");
    drop(stream);
    std::thread::sleep(Duration::from_millis(100));
    h.seen.lock().expect("lock").finished_at = None;

    // A client that never reads: the 400 ms write timeout closes it.
    let mut stalled = TcpStream::connect(h.addr).expect("connect stalled");
    let started = Instant::now();
    let (head, _) = request_head(&mut stalled, "/huge.bin");
    assert!(head.starts_with("HTTP/1.1 200"), "head: {head:?}");
    let deadline = started + Duration::from_secs(10);
    let finished_at = loop {
        if let Some(t) = h.seen.lock().expect("lock").finished_at {
            break t;
        }
        assert!(Instant::now() < deadline, "the write timeout never fired");
        std::thread::sleep(Duration::from_millis(10));
    };
    let took = finished_at.duration_since(started);
    assert!(
        took >= Duration::from_millis(300) && took < Duration::from_secs(3),
        "the stalled client closed after {took:?}, expected about 400 ms"
    );
    assert!(h.seen.lock().expect("lock").closed, "timeout must close");
    drop(stalled);
    h.stop();
}

/// `HEAD` sends the head with the file's `Content-Length` and no body; the
/// connection then serves a `GET` for the same file, and a missing file is 404.
#[test]
#[foundation_macros::timeout(120_000)]
#[serial(http_test)]
fn static_file_head_has_no_body_and_connection_is_kept() {
    let _pool = initialize_pool(304, Some(3));
    let dir = TempDir::new("head");
    let data = pattern(50_000);
    std::fs::write(dir.path().join("a.bin"), &data).expect("write file");
    let h = start_server(dir.path(), StaticFileConfig::new());

    let mut stream = TcpStream::connect(h.addr).expect("connect");
    stream
        .write_all(b"HEAD /a.bin HTTP/1.1\r\nHost: t\r\n\r\n")
        .expect("write HEAD");
    stream
        .set_read_timeout(Some(Duration::from_secs(10)))
        .expect("read timeout");
    let mut acc = Vec::new();
    let mut buf = [0u8; 4096];
    while !acc.windows(4).any(|w| w == b"\r\n\r\n") {
        let n = stream.read(&mut buf).expect("read head");
        assert!(n > 0, "closed before the head ended");
        acc.extend_from_slice(&buf[..n]);
    }
    let end = acc.windows(4).position(|w| w == b"\r\n\r\n").expect("end") + 4;
    let head = String::from_utf8_lossy(&acc[..end]).to_string();
    assert_eq!(content_length(&head), data.len(), "head: {head:?}");
    assert_eq!(acc.len(), end, "a HEAD response must carry no body");

    // Same connection: GET the file, byte for byte.
    let (head, mut body) = request_head(&mut stream, "/a.bin");
    let want = content_length(&head);
    while body.len() < want {
        let n = stream.read(&mut buf).expect("read body");
        assert!(n > 0, "closed early");
        body.extend_from_slice(&buf[..n]);
    }
    assert_eq!(sha256(&body), sha256(&data));

    // A missing file is a 404.
    let (head, _) = request_head(&mut stream, "/missing.bin");
    assert!(head.starts_with("HTTP/1.1 404"), "head: {head:?}");
    h.stop();
}

/// A handler that answers with a large in-memory body through the shared writer.
struct BigBody(Vec<u8>);

impl Serve for BigBody {
    fn serve(
        &self,
        _ctx: Ctx,
        _req: SimpleIncomingRequest,
        conn: SharedByteBufferStream<RawStream>,
    ) -> ConnectionResult {
        send_bytes(conn, 200, "application/octet-stream", self.0.clone())
    }
}

/// Read a response from `path` slowly (16 KiB per read, 1 ms between reads)
/// and return its body.
fn slow_get(addr: std::net::SocketAddr, path: &str) -> Vec<u8> {
    let mut stream = TcpStream::connect(addr).expect("connect");
    let (head, mut body) = request_head(&mut stream, path);
    assert!(head.starts_with("HTTP/1.1 200"), "head: {head:?}");
    let want = content_length(&head);
    let mut buf = vec![0u8; 16 * 1024];
    while body.len() < want {
        let n = stream.read(&mut buf).expect("read body");
        assert!(n > 0, "closed after {} of {want} bytes", body.len());
        body.extend_from_slice(&buf[..n]);
        std::thread::sleep(Duration::from_millis(1));
    }
    assert_eq!(body.len(), want, "received more bytes than Content-Length");
    body
}

/// A 4 MiB body written through the shared writer arrives byte for byte at a
/// slow client. (The synchronous `respond_to_buffer::*` path truncates it.)
#[test]
#[foundation_macros::timeout(120_000)]
#[serial(http_test)]
fn respond_large_body_is_byte_exact() {
    let _pool = initialize_pool(305, Some(3));
    let dir = TempDir::new("respond");
    let data = pattern(4 * 1024 * 1024);
    let big: Arc<dyn Serve> = Arc::new(BigBody(data.clone()));
    let h = start_server_with(dir.path(), StaticFileConfig::new(), |app| {
        app.router.add_route_any("/big", &big);
    });
    let body = slow_get(h.addr, "/big");
    assert_eq!(sha256(&body), sha256(&data), "body differs from the source");
    h.stop();
}

/// The same for `StaticAssetHandler`.
#[test]
#[foundation_macros::timeout(120_000)]
#[serial(http_test)]
fn static_asset_large_body_is_byte_exact() {
    let _pool = initialize_pool(306, Some(3));
    let dir = TempDir::new("asset");
    let data = pattern(4 * 1024 * 1024);
    let asset: Arc<dyn Serve> = Arc::new(StaticAssetHandler::new(
        data.clone(),
        "application/octet-stream",
    ));
    let h = start_server_with(dir.path(), StaticFileConfig::new(), |app| {
        app.router.add_route_any("/asset", &asset);
    });
    let body = slow_get(h.addr, "/asset");
    assert_eq!(sha256(&body), sha256(&data), "body differs from the source");
    h.stop();
}

/// `send_timeout` is the time between two writes, not the length of the whole
/// response: a client that reads up to 512 KiB every 100 ms (the socket is writable again every few hundred ms), with a 1 s
/// `send_timeout`, receives the whole file.
#[test]
#[foundation_macros::timeout(120_000)]
#[serial(http_test)]
fn send_timeout_is_idle_not_total() {
    let _pool = initialize_pool(307, Some(3));
    let dir = TempDir::new("idle");
    let data = pattern(16 * 1024 * 1024);
    std::fs::write(dir.path().join("slow.bin"), &data).expect("write file");
    let h = start_server(
        dir.path(),
        StaticFileConfig::new().with_send_timeout(Duration::from_secs(1)),
    );

    let mut stream = TcpStream::connect(h.addr).expect("connect");
    let started = Instant::now();
    let (head, mut body) = request_head(&mut stream, "/slow.bin");
    let want = content_length(&head);
    let mut buf = vec![0u8; 512 * 1024];
    while body.len() < want {
        let n = stream.read(&mut buf).expect("read body");
        assert!(n > 0, "closed after {} of {want} bytes", body.len());
        body.extend_from_slice(&buf[..n]);
        std::thread::sleep(Duration::from_millis(100));
    }
    let took = started.elapsed();
    assert!(
        took > Duration::from_millis(1500),
        "the download took only {took:?}; the test did not exercise the idle timer"
    );
    assert_eq!(sha256(&body), sha256(&data), "body differs from the file");
    assert!(
        !h.seen.lock().expect("lock").closed,
        "the connection must not close"
    );
    h.stop();
}

/// A client that stops reading is closed after about `send_timeout`.
#[test]
#[foundation_macros::timeout(120_000)]
#[serial(http_test)]
fn send_timeout_closes_stalled_client() {
    let _pool = initialize_pool(308, Some(3));
    let dir = TempDir::new("stalled");
    let file = std::fs::File::create(dir.path().join("huge.bin")).expect("create");
    file.set_len(64 * 1024 * 1024).expect("sparse file");
    drop(file);
    let h = start_server(
        dir.path(),
        StaticFileConfig::new().with_send_timeout(Duration::from_millis(300)),
    );

    let mut stalled = TcpStream::connect(h.addr).expect("connect");
    let (head, _) = request_head(&mut stalled, "/huge.bin");
    assert!(head.starts_with("HTTP/1.1 200"), "head: {head:?}");
    let started = Instant::now();
    let deadline = started + Duration::from_secs(10);
    let finished_at = loop {
        if let Some(t) = h.seen.lock().expect("lock").finished_at {
            break t;
        }
        assert!(Instant::now() < deadline, "the send timeout never fired");
        std::thread::sleep(Duration::from_millis(10));
    };
    let took = finished_at.duration_since(started);
    assert!(
        took < Duration::from_secs(3),
        "the stalled client closed after {took:?}, expected about 300 ms"
    );
    assert!(h.seen.lock().expect("lock").closed, "timeout must close");
    drop(stalled);
    h.stop();
}
