//! Request body handling of the keychain native server (spec 70 feature 64).
//!
//! The body must be read inside the async handler future, as a bounded stream.
//! A slow upload must not hold a worker thread.

// The valtron stream chain (`execute(from_future(..))`) nests past the default
// trait-solver recursion limit (128); see `foundation_db/src/lib.rs`.
#![recursion_limit = "256"]

use std::io::{Read, Write};
use std::net::TcpStream;
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
    addr: std::net::SocketAddr,
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
        Arc::new(foundation_core::valtron::MonotonicClock::new()),
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

/// Email of exactly 28 bytes, so its prelogin body is exactly 40 bytes.
const EMAIL: &str = "slowuploaduser00@example.com";
/// KDF iteration count the registered user has; prelogin echoes it back, so a
/// truncated request body (unknown user) is visible in the response.
const ITERATIONS: u32 = 123_456;

fn prelogin_body() -> String {
    format!(r#"{{"email":"{EMAIL}"}}"#)
}

fn head(addr: std::net::SocketAddr, path: &str, len: usize) -> String {
    format!(
        "POST {path} HTTP/1.1\r\nHost: {addr}\r\n\
         Content-Type: application/json\r\nContent-Length: {len}\r\n\r\n"
    )
}

/// Read one response: the status code and the body (`Content-Length` framed).
fn read_response(stream: &mut TcpStream) -> (u16, String) {
    stream
        .set_read_timeout(Some(Duration::from_secs(10)))
        .expect("read timeout");
    let mut buf = Vec::new();
    let mut chunk = [0u8; 1024];
    loop {
        if let Some(end) = buf.windows(4).position(|w| w == b"\r\n\r\n") {
            let head = String::from_utf8_lossy(&buf[..end]).to_string();
            let status = head
                .split_whitespace()
                .nth(1)
                .and_then(|c| c.parse().ok())
                .unwrap_or_else(|| panic!("bad status line: {head}"));
            let len = head
                .lines()
                .find_map(|l| {
                    let (k, v) = l.split_once(':')?;
                    k.eq_ignore_ascii_case("content-length")
                        .then(|| v.trim().parse::<usize>().ok())?
                })
                .unwrap_or(0);
            if buf.len() >= end + 4 + len {
                let body = String::from_utf8_lossy(&buf[end + 4..end + 4 + len]).to_string();
                return (status, body);
            }
        }
        let n = stream.read(&mut chunk).expect("read response");
        assert!(n > 0, "connection closed before a full response");
        buf.extend_from_slice(&chunk[..n]);
    }
}

fn post(addr: std::net::SocketAddr, path: &str, body: &str) -> (u16, String) {
    let mut s = TcpStream::connect(addr).expect("connect");
    s.write_all(format!("{}{body}", head(addr, path, body.len())).as_bytes())
        .expect("request");
    read_response(&mut s)
}

fn register_user(addr: std::net::SocketAddr) {
    let body = format!(
        r#"{{"email":"{EMAIL}","name":"S","masterPasswordHash":"mph","key":"2.k|e","kdf":0,"kdfIterations":{ITERATIONS}}}"#
    );
    let (status, text) = post(addr, "/identity/accounts/register", &body);
    assert_eq!(status, 200, "register: {text}");
}

#[valtron_test(threads = 2, timeout = 60000)]
fn slow_request_body_does_not_hold_worker() {
    let server = start_server(|s| s);
    let addr = server.addr;
    register_user(addr);

    // Two uploads of 40 bytes, 1 byte every 50 ms (2 s each): one per worker.
    // The uploads start 250 ms apart so their database calls do not overlap:
    // the Turso store allows one use at a time (see `docs/bug-backlog.md`).
    let slow: Vec<_> = (0..2)
        .map(|i| {
            std::thread::sleep(Duration::from_millis(250 * i));
            std::thread::spawn(move || {
                let body = prelogin_body();
                assert_eq!(body.len(), 40);
                let mut s = TcpStream::connect(addr).expect("connect");
                s.write_all(head(addr, "/identity/accounts/prelogin", body.len()).as_bytes())
                    .expect("head");
                for b in body.as_bytes() {
                    s.write_all(&[*b]).expect("byte");
                    std::thread::sleep(Duration::from_millis(50));
                }
                read_response(&mut s)
            })
        })
        .collect();
    std::thread::sleep(Duration::from_millis(400));

    // A third, complete request must answer while both uploads are open.
    let started = Instant::now();
    let (status, text) = post(addr, "/identity/accounts/prelogin", &prelogin_body());
    let elapsed = started.elapsed();
    assert_eq!(status, 200);
    assert!(
        text.contains(&ITERATIONS.to_string()),
        "fast prelogin: {text}"
    );
    assert!(
        elapsed < Duration::from_millis(500),
        "a second request waited {elapsed:?} behind slow uploads"
    );

    // The slow bodies must arrive whole: the user is found, not defaulted.
    for t in slow {
        let (status, text) = t.join().expect("slow client");
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
    let addr = server.addr;

    // A 2 KiB body is over the 1 KiB cap: 413, then the connection closes.
    let big = format!(r#"{{"email":"{}"}}"#, "a".repeat(2048 - 12));
    assert_eq!(big.len(), 2048);
    let (status, text) = post(addr, "/identity/accounts/prelogin", &big);
    assert_eq!(status, 413, "over the cap: {text}");

    // A body of exactly the cap is read whole and answered.
    let exact = format!(r#"{{"email":"{}"}}"#, "a".repeat(1024 - 12));
    assert_eq!(exact.len(), 1024);
    let (status, text) = post(addr, "/identity/accounts/prelogin", &exact);
    assert_eq!(status, 200, "at the cap: {text}");
}
