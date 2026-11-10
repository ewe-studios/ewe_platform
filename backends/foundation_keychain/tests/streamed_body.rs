//! Request body handling of the keychain native server (spec 70 feature 64).
//!
//! The body must be read inside the async handler future, as a bounded stream.
//! A slow upload must not hold a worker.

// The valtron stream chain (`execute(from_future(..))`) nests past the default
// trait-solver recursion limit (128); see `foundation_db/src/lib.rs`.
#![recursion_limit = "256"]

use std::io::{ErrorKind, Read, Write};
use std::net::{SocketAddr, TcpStream};
use std::sync::Arc;
use std::time::{Duration, Instant};

use foundation_core::valtron::{
    collect_one, execute, from_future, valtron_test, BlockingFutureExt,
};
use foundation_db::core::storage_provider::QueryStore;
use foundation_db::{StorageBackend, StorageProvider};
use foundation_http::native::server::{HttpServer, KeepAliveConfig, ServerConfig};
use foundation_http::ShutdownSignal;
use foundation_keychain::core::store::apply_schema;
use foundation_keychain::server::native::KeychainServer;
use foundation_keychain::KeychainContext;
use tempfile::TempDir;

struct TestServer {
    addr: SocketAddr,
    shutdown: Arc<ShutdownSignal>,
    _dir: TempDir,
}

impl Drop for TestServer {
    fn drop(&mut self) {
        self.shutdown.turn_on();
    }
}

fn drive<T, F>(future: F) -> T
where
    T: Send + 'static,
    F: std::future::Future<Output = T> + Send + 'static,
{
    collect_one(execute(from_future(future), None).expect("execute")).expect("result")
}

fn start_server(configure: impl FnOnce(KeychainServer) -> KeychainServer) -> TestServer {
    let dir = tempfile::tempdir().expect("tempdir");
    let db_path = dir.path().join("kc.db");
    let provider = BlockingFutureExt::block({
        let url = db_path.to_str().unwrap().to_string();
        async move { StorageProvider::new(StorageBackend::Turso { url }).await }
    })
    .expect("init turso");
    let db: Arc<dyn QueryStore> = Arc::new(provider);
    let d = Arc::clone(&db);
    drive(async move { apply_schema(d.as_ref()).await }).expect("apply schema");

    let app = configure(KeychainServer::new(KeychainContext::new(
        db,
        foundation_core::valtron::Ctx::host(),
    )))
    .http_app();

    let shutdown = Arc::new(ShutdownSignal::new());
    let listener = std::net::TcpListener::bind("127.0.0.1:0").expect("bind");
    let addr = listener.local_addr().expect("addr");
    let cfg = ServerConfig::defaults()
        .with_keep_alive(KeepAliveConfig::defaults().with_idle_timeout(Duration::from_secs(3)));
    let server = HttpServer::with_config(app, &format!("127.0.0.1:{}", addr.port()), cfg);
    let sc = shutdown.clone();
    std::thread::spawn(move || server.serve_with_listener(&listener, &sc));

    TestServer {
        addr,
        shutdown,
        _dir: dir,
    }
}

/// Why a client read did not return a complete response.
#[derive(Debug)]
enum ReadError {
    /// The socket receive timeout expired before a full response arrived.
    Timeout,
    /// The socket reported `WouldBlock` before its read timeout expired.
    WouldBlock,
    /// Any other socket error, including a closed connection or a malformed
    /// response.
    Io(std::io::Error),
}

impl std::fmt::Display for ReadError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Timeout => write!(f, "no complete response before the read timeout"),
            Self::WouldBlock => write!(f, "the socket would block before its read timeout"),
            Self::Io(error) => write!(f, "socket read failed: {error}"),
        }
    }
}

impl std::error::Error for ReadError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::Io(error) => Some(error),
            Self::Timeout | Self::WouldBlock => None,
        }
    }
}

/// Classify a failed socket read.
///
/// macOS reports an expired socket receive timeout (`SO_RCVTIMEO`) as
/// `WouldBlock`, the same error a non-blocking socket returns. The time the read
/// took tells them apart: a `WouldBlock` after the timeout elapsed is a timeout;
/// a `WouldBlock` before it elapsed comes from a non-blocking socket.
fn classify_read_error(error: std::io::Error, elapsed: Duration, timeout: Duration) -> ReadError {
    match error.kind() {
        ErrorKind::TimedOut => ReadError::Timeout,
        ErrorKind::WouldBlock if elapsed >= timeout => ReadError::Timeout,
        ErrorKind::WouldBlock => ReadError::WouldBlock,
        _ => ReadError::Io(error),
    }
}

/// A malformed response is an I/O error for the helper.
fn invalid_response(message: &str) -> ReadError {
    ReadError::Io(std::io::Error::new(ErrorKind::InvalidData, message))
}

/// Parse one complete response from `buf`, if it holds one.
///
/// The body is framed by `Content-Length`; a response without that header has an
/// empty body.
fn parse_response(buf: &[u8]) -> Result<Option<(u16, String)>, ReadError> {
    let Some(head_end) = buf.windows(4).position(|window| window == b"\r\n\r\n") else {
        return Ok(None);
    };
    let head = String::from_utf8_lossy(&buf[..head_end]);
    let status = head
        .split_whitespace()
        .nth(1)
        .and_then(|code| code.parse().ok())
        .ok_or_else(|| invalid_response("bad status line"))?;
    let len = head
        .lines()
        .find_map(|line| {
            let (key, value) = line.split_once(':')?;
            key.eq_ignore_ascii_case("content-length")
                .then(|| value.trim().parse::<usize>().ok())?
        })
        .unwrap_or(0);
    let body_start = head_end + 4;
    let Some(body_end) = body_start.checked_add(len) else {
        return Err(invalid_response("response content-length is too large"));
    };
    if buf.len() < body_end {
        return Ok(None);
    }
    Ok(Some((
        status,
        String::from_utf8_lossy(&buf[body_start..body_end]).to_string(),
    )))
}

/// Read one response: the status code and the body (`Content-Length` framed).
///
/// `timeout` bounds each socket read, so a caller cannot hang. A timeout is
/// reported as [`ReadError::Timeout`], never as [`ReadError::WouldBlock`].
fn read_response(stream: &mut TcpStream, timeout: Duration) -> Result<(u16, String), ReadError> {
    stream
        .set_read_timeout(Some(timeout))
        .map_err(ReadError::Io)?;
    let mut buf = Vec::new();
    let mut chunk = [0u8; 1024];
    loop {
        if let Some(response) = parse_response(&buf)? {
            return Ok(response);
        }
        let started = Instant::now();
        let n = match stream.read(&mut chunk) {
            Ok(0) => {
                return Err(ReadError::Io(std::io::Error::new(
                    ErrorKind::UnexpectedEof,
                    "connection closed before a full response",
                )))
            }
            Ok(n) => n,
            Err(error) => return Err(classify_read_error(error, started.elapsed(), timeout)),
        };
        buf.extend_from_slice(&chunk[..n]);
    }
}

/// A test HTTP client with a bounded read wait.
struct TestClient {
    addr: SocketAddr,
    read_timeout: Duration,
}

impl TestClient {
    /// A client for `addr`. The read timeout is 10 s: a healthy localhost
    /// request answers in milliseconds, and 10 s is long enough for a loaded
    /// continuous-integration host. The bound makes a request that a slow body
    /// wrongly holds fail instead of hang.
    #[must_use]
    fn new(addr: SocketAddr) -> Self {
        Self {
            addr,
            read_timeout: Duration::from_secs(10),
        }
    }

    /// Replace the read timeout.
    #[must_use]
    fn with_read_timeout(mut self, timeout: Duration) -> Self {
        self.read_timeout = timeout;
        self
    }

    /// Send one request and read the complete response.
    fn post(&self, path: &str, body: &str) -> Result<(u16, String), ReadError> {
        let mut stream = TcpStream::connect(self.addr).map_err(ReadError::Io)?;
        stream
            .write_all(format!("{}{body}", head(self.addr, path, body.len())).as_bytes())
            .map_err(ReadError::Io)?;
        read_response(&mut stream, self.read_timeout)
    }
}

/// Email of exactly 28 bytes, so its prelogin body is exactly 40 bytes.
const EMAIL: &str = "slowuploaduser00@example.com";
/// KDF iteration count the registered user has; prelogin echoes it back, so a
/// truncated request body (unknown user) is visible in the response.
const ITERATIONS: u32 = 123_456;

fn prelogin_body() -> String {
    format!(r#"{{"email":"{EMAIL}"}}"#)
}

/// One request head. Every client opens its own connection, so it asks the
/// server to close the connection after the response.
fn head(addr: SocketAddr, path: &str, len: usize) -> String {
    format!(
        "POST {path} HTTP/1.1\r\nHost: {addr}\r\n\
         Content-Type: application/json\r\nContent-Length: {len}\r\n\
         Connection: close\r\n\r\n"
    )
}

fn register_user(client: &TestClient) {
    let body = format!(
        r#"{{"email":"{EMAIL}","name":"S","masterPasswordHash":"mph","key":"2.k|e","kdf":0,"kdfIterations":{ITERATIONS}}}"#
    );
    let (status, text) = client
        .post("/identity/accounts/register", &body)
        .expect("register request");
    assert_eq!(status, 200, "register: {text}");
}

#[valtron_test(threads = 2, timeout = 60000)]
fn slow_request_body_does_not_hold_worker() {
    let server = start_server(|s| s);
    let addr = server.addr;
    // A slow body must not hold a worker, so the third request must answer while
    // both bodies are still incomplete. The read bound is explicit: a healthy
    // localhost request answers in milliseconds, and 10 s lets a loaded
    // continuous-integration host answer. A server that holds a worker returns
    // `Timeout` instead of hanging the test.
    let client = TestClient::new(addr).with_read_timeout(Duration::from_secs(10));
    register_user(&client);
    let read_timeout = client.read_timeout;

    // Both uploads send the request head and every body byte but the last, then
    // park until released. Their connections stay open with an incomplete body.
    // The ready channels are the explicit synchronization point: both bodies are
    // open before the third request starts.
    let mut ready_rxs = Vec::new();
    let mut release_txs = Vec::new();
    let mut slow = Vec::new();
    for _ in 0..2 {
        let (ready_tx, ready_rx) = std::sync::mpsc::channel();
        let (release_tx, release_rx) = std::sync::mpsc::channel();
        ready_rxs.push(ready_rx);
        release_txs.push(release_tx);
        slow.push(std::thread::spawn(move || {
            let body = prelogin_body();
            assert_eq!(body.len(), 40);
            let mut stream = TcpStream::connect(addr).expect("connect");
            stream
                .write_all(head(addr, "/identity/accounts/prelogin", body.len()).as_bytes())
                .expect("request head");
            stream
                .write_all(&body.as_bytes()[..body.len() - 1])
                .expect("partial body");
            ready_tx.send(()).expect("signal ready");
            // Keep the body incomplete until the main thread releases it.
            release_rx.recv().expect("wait for release");
            stream
                .write_all(&body.as_bytes()[body.len() - 1..])
                .expect("last body byte");
            read_response(&mut stream, read_timeout)
        }));
    }

    // Wait until both uploads carry an incomplete body.
    for ready in &ready_rxs {
        ready.recv().expect("upload ready");
    }

    // A complete request must answer while both bodies are still incomplete.
    // The read bound is the timeout: a server that parks a worker on the uploads
    // cannot answer, and the read returns `Timeout` instead of hanging.
    let third = client.post("/identity/accounts/prelogin", &prelogin_body());

    // Release and join the uploads on every path, so no thread stays parked.
    // One at a time: each release runs a prelogin on the shared store, and this
    // test checks worker concurrency, not store concurrency.
    let mut slow_results = Vec::new();
    for (release, handle) in release_txs.into_iter().zip(slow) {
        release.send(()).expect("release upload");
        slow_results.push(handle.join().expect("slow client thread"));
    }

    let (status, text) =
        third.expect("the third request must answer while both upload bodies are open");
    assert_eq!(status, 200);
    assert!(
        text.contains(&ITERATIONS.to_string()),
        "fast prelogin: {text}"
    );

    // The slow bodies must arrive whole: the user is found, not defaulted.
    for result in slow_results {
        let (status, text) = result.expect("slow client response");
        assert_eq!(status, 200, "slow client: {text}");
        assert!(
            text.contains(&ITERATIONS.to_string()),
            "slow body was truncated, prelogin answered: {text}"
        );
    }
}

#[valtron_test(threads = 2, timeout = 60000)]
fn request_body_over_cap_is_413() {
    let server = start_server(|s| s.with_max_request_body(1024));
    let client = TestClient::new(server.addr);

    // A 2 KiB body is over the 1 KiB cap: 413, then the connection closes.
    let big = format!(r#"{{"email":"{}"}}"#, "a".repeat(2048 - 12));
    assert_eq!(big.len(), 2048);
    let (status, text) = client
        .post("/identity/accounts/prelogin", &big)
        .expect("over-cap request");
    assert_eq!(status, 413, "over the cap: {text}");

    // A body of exactly the cap is read whole and answered.
    let exact = format!(r#"{{"email":"{}"}}"#, "a".repeat(1024 - 12));
    assert_eq!(exact.len(), 1024);
    let (status, text) = client
        .post("/identity/accounts/prelogin", &exact)
        .expect("at-cap request");
    assert_eq!(status, 200, "at the cap: {text}");
}
