//! Non-Docker end-to-end tests for wired-but-previously-unverified proxy
//! capabilities: SSL redirect (F12), writer-affinity / sticky cookie (F13), and
//! zero-downtime drain (F18). Each starts a real `ProxyServer` + real backends
//! and drives it over a socket — no stubs.

use std::io::{Read, Write};
use std::net::TcpStream;
use std::sync::Arc;
use std::time::Duration;

use std::sync::atomic::{AtomicBool, Ordering};

use foundation_core::valtron::initialize_pool;
use foundation_proxy::config::{ServiceConfig, SslConfig};
use foundation_proxy::{ProxyConfig, ProxyServer};

// ── Backend: a plain std TCP HTTP/1.1 responder returning a fixed identifier. ──
//
// Deliberately NOT a foundation_http server — these run on their own threads,
// off the proxy's valtron pool, so several backends + the proxy front end never
// contend for pool workers in one process.

struct Backend {
    stop: Arc<AtomicBool>,
}
impl Drop for Backend {
    fn drop(&mut self) {
        self.stop.store(true, Ordering::SeqCst);
    }
}

fn start_id_backend(id: &str) -> (u16, Backend) {
    let listener = std::net::TcpListener::bind("127.0.0.1:0").expect("bind backend");
    listener.set_nonblocking(true).ok();
    let port = listener.local_addr().unwrap().port();
    let stop = Arc::new(AtomicBool::new(false));
    let body = id.to_string();
    let thread_stop = Arc::clone(&stop);
    std::thread::spawn(move || {
        while !thread_stop.load(Ordering::SeqCst) {
            match listener.accept() {
                Ok((sock, _)) => {
                    // Keep-alive per connection: the proxy pools upstream
                    // connections for reuse, so a real HTTP/1.1 backend must serve
                    // several requests on one socket rather than closing each time.
                    let body = body.clone();
                    std::thread::spawn(move || {
                        let mut sock = sock;
                        sock.set_read_timeout(Some(Duration::from_secs(5))).ok();
                        let mut buf = [0u8; 4096];
                        loop {
                            match sock.read(&mut buf) {
                                Ok(0) => break, // peer closed
                                Ok(_) => {
                                    let resp = format!(
                                        "HTTP/1.1 200 OK\r\nContent-Type: text/plain\r\nContent-Length: {}\r\n\r\n{body}",
                                        body.len()
                                    );
                                    if sock.write_all(resp.as_bytes()).is_err() || sock.flush().is_err() {
                                        break;
                                    }
                                }
                                Err(_) => break,
                            }
                        }
                    });
                }
                Err(ref e) if e.kind() == std::io::ErrorKind::WouldBlock => {
                    std::thread::sleep(Duration::from_millis(5));
                }
                Err(_) => break,
            }
        }
    });
    (port, Backend { stop })
}

// ── Raw HTTP/1.1 client ──────────────────────────────────────────────────

struct RawResp {
    status: u16,
    headers: Vec<(String, String)>,
    body: String,
}

fn send(addr: std::net::SocketAddr, path: &str, host: &str, extra: &[(&str, &str)]) -> RawResp {
    let mut stream = TcpStream::connect(addr).expect("connect");
    stream.set_read_timeout(Some(Duration::from_secs(10))).ok();
    let mut req = format!("GET {path} HTTP/1.1\r\nHost: {host}\r\n");
    for (k, v) in extra {
        req.push_str(&format!("{k}: {v}\r\n"));
    }
    req.push_str("Connection: close\r\n\r\n");
    stream.write_all(req.as_bytes()).expect("write");
    stream.flush().ok();
    let mut raw = Vec::new();
    stream.read_to_end(&mut raw).ok();
    parse(&raw)
}

fn parse(raw: &[u8]) -> RawResp {
    let text = String::from_utf8_lossy(raw);
    let split = text.find("\r\n\r\n").unwrap_or(text.len());
    let head = &text[..split];
    let body = text.get(split + 4..).unwrap_or("").to_string();
    let mut lines = head.lines();
    let status = lines
        .next()
        .and_then(|l| l.split_whitespace().nth(1))
        .and_then(|s| s.parse().ok())
        .unwrap_or(0);
    let headers = lines
        .filter_map(|l| l.split_once(':').map(|(k, v)| (k.trim().to_lowercase(), v.trim().to_string())))
        .collect();
    RawResp { status, headers, body }
}

impl RawResp {
    fn header(&self, name: &str) -> Option<&str> {
        self.headers.iter().find(|(k, _)| k == name).map(|(_, v)| v.as_str())
    }
}

fn write_certs() -> (String, String) {
    let dir = std::env::temp_dir().join(format!("ewe_feat_{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    let cert = dir.join("cert.pem");
    let key = dir.join("key.pem");
    std::fs::write(&cert, include_bytes!("fixtures/quic_cert.pem")).unwrap();
    std::fs::write(&key, include_bytes!("fixtures/quic_key.pem")).unwrap();
    (cert.display().to_string(), key.display().to_string())
}

// ── F13: writer affinity (sticky cookie) ─────────────────────────────────

#[test]
fn sticky_cookie_pins_requests_to_one_backend() {
    let _guard = initialize_pool(61, Some(12));
    let (pa, _a) = start_id_backend("BACKEND-A");
    let (pb, _b) = start_id_backend("BACKEND-B");

    let svc = ServiceConfig::new("echo", "test.local")
        .backend(&format!("http://127.0.0.1:{pa}"))
        .backend(&format!("http://127.0.0.1:{pb}"));
    let proxy = ProxyServer::start(
        ProxyConfig::new("test.local", "127.0.0.1").bind("127.0.0.1:0").service(svc),
    )
    .expect("start proxy");
    let addr = proxy.local_addr();

    // Wait for the proxy + both backends to be serving (accept loops ready).
    let mut first = send(addr, "/", "test.local", &[]);
    for _ in 0..40 {
        if first.status == 200 {
            break;
        }
        std::thread::sleep(Duration::from_millis(50));
        first = send(addr, "/", "test.local", &[]);
    }
    assert_eq!(first.status, 200, "proxy forwards to a backend once ready");
    let set_cookie = first.header("set-cookie").expect("multi-backend responses set a sticky cookie").to_string();
    let cookie = set_cookie.split(';').next().unwrap().to_string(); // __proxy_sticky=<idx>
    let pinned = first.body.clone();
    assert!(pinned == "BACKEND-A" || pinned == "BACKEND-B", "served a real backend: {pinned}");

    // Subsequent requests carrying the cookie all hit the same backend.
    for _ in 0..6 {
        let r = send(addr, "/", "test.local", &[("Cookie", &cookie)]);
        assert_eq!(r.status, 200);
        assert_eq!(r.body, pinned, "sticky cookie must pin to the same backend");
    }

    proxy.shutdown();
}

// ── F18: zero-downtime drain ─────────────────────────────────────────────

#[test]
fn draining_returns_503() {
    let _guard = initialize_pool(62, Some(12));
    let (pa, _a) = start_id_backend("ONLY");

    let svc = ServiceConfig::new("echo", "test.local").backend(&format!("http://127.0.0.1:{pa}"));
    let proxy = ProxyServer::start(
        ProxyConfig::new("test.local", "127.0.0.1").bind("127.0.0.1:0").service(svc),
    )
    .expect("start proxy");
    let addr = proxy.local_addr();
    std::thread::sleep(Duration::from_millis(150));

    // Healthy: 200.
    assert_eq!(send(addr, "/", "test.local", &[]).status, 200, "serves before drain");

    // Begin draining: new requests get 503 (Decision 22).
    proxy.start_drain();
    std::thread::sleep(Duration::from_millis(50));
    let r = send(addr, "/", "test.local", &[]);
    assert_eq!(r.status, 503, "draining proxy answers 503 so clients retry elsewhere");

    proxy.shutdown();
}

// ── F12: SSL redirect (plain HTTP → HTTPS 301) ───────────────────────────

#[test]
fn ssl_redirect_301_to_https() {
    let _guard = initialize_pool(63, Some(12));
    let (pa, _a) = start_id_backend("SECURE");
    let (cert, key) = write_certs();

    let svc = ServiceConfig::new("echo", "test.local").backend(&format!("http://127.0.0.1:{pa}"));
    let proxy = ProxyServer::start(
        ProxyConfig::new("test.local", "127.0.0.1")
            .bind("127.0.0.1:0")
            .ssl(SslConfig::static_cert(&cert, &key))
            .redirect_bind("127.0.0.1:0")
            .service(svc),
    )
    .expect("start proxy with TLS");
    let redirect_addr = proxy.redirect_local_addr().expect("redirect listener bound");
    std::thread::sleep(Duration::from_millis(150));

    // Plain HTTP to the redirect port → 301 to the https URL.
    let r = send(redirect_addr, "/login", "test.local", &[]);
    assert_eq!(r.status, 301, "plain HTTP is redirected, not served");
    assert_eq!(
        r.header("location"),
        Some("https://test.local/login"),
        "redirect points at the https URL for the same host+path"
    );

    proxy.shutdown();
}

// ── WebSocket / Upgrade relay ────────────────────────────────────────────

/// A backend that completes a `101 Switching Protocols` handshake and then
/// echoes every byte — a stand-in for a WebSocket server. Exercises the proxy's
/// upgrade path (`forward_upgrade` + `splice_bidirectional`).
fn start_ws_echo_backend() -> (u16, Backend) {
    let listener = std::net::TcpListener::bind("127.0.0.1:0").expect("bind ws backend");
    listener.set_nonblocking(true).ok();
    let port = listener.local_addr().unwrap().port();
    let stop = Arc::new(AtomicBool::new(false));
    let thread_stop = Arc::clone(&stop);
    std::thread::spawn(move || {
        while !thread_stop.load(Ordering::SeqCst) {
            match listener.accept() {
                Ok((sock, _)) => {
                    std::thread::spawn(move || {
                        let mut sock = sock;
                        sock.set_read_timeout(Some(Duration::from_secs(5))).ok();
                        // Read the (proxied) upgrade request head.
                        let mut buf = [0u8; 4096];
                        let _ = sock.read(&mut buf);
                        let resp = "HTTP/1.1 101 Switching Protocols\r\nUpgrade: websocket\r\nConnection: Upgrade\r\n\r\n";
                        if sock.write_all(resp.as_bytes()).is_err() {
                            return;
                        }
                        let _ = sock.flush();
                        // Post-upgrade: echo raw frames.
                        loop {
                            match sock.read(&mut buf) {
                                Ok(0) | Err(_) => break,
                                Ok(n) => {
                                    if sock.write_all(&buf[..n]).is_err() {
                                        break;
                                    }
                                    let _ = sock.flush();
                                }
                            }
                        }
                    });
                }
                Err(ref e) if e.kind() == std::io::ErrorKind::WouldBlock => {
                    std::thread::sleep(Duration::from_millis(5));
                }
                Err(_) => break,
            }
        }
    });
    (port, Backend { stop })
}

#[test]
fn websocket_upgrade_relays_and_tunnels_bytes() {
    let _guard = initialize_pool(64, Some(12));
    let (pa, _a) = start_ws_echo_backend();

    let svc = ServiceConfig::new("ws", "test.local").backend(&format!("http://127.0.0.1:{pa}"));
    let proxy = ProxyServer::start(
        ProxyConfig::new("test.local", "127.0.0.1").bind("127.0.0.1:0").service(svc),
    )
    .expect("start proxy");
    let addr = proxy.local_addr();
    std::thread::sleep(Duration::from_millis(200));

    let mut stream = TcpStream::connect(addr).expect("connect proxy");
    stream.set_read_timeout(Some(Duration::from_secs(5))).ok();

    // Send a WebSocket upgrade request.
    let req = "GET /ws HTTP/1.1\r\nHost: test.local\r\nUpgrade: websocket\r\n\
               Connection: Upgrade\r\nSec-WebSocket-Key: dGhlIHNhbXBsZSBub25jZQ==\r\n\
               Sec-WebSocket-Version: 13\r\n\r\n";
    stream.write_all(req.as_bytes()).expect("write upgrade");
    stream.flush().ok();

    // Read the 101 response head.
    let mut head = Vec::new();
    let mut byte = [0u8; 1];
    while !head.windows(4).any(|w| w == b"\r\n\r\n") {
        match stream.read(&mut byte) {
            Ok(1) => head.push(byte[0]),
            _ => break,
        }
    }
    let head_str = String::from_utf8_lossy(&head);
    assert!(
        head_str.starts_with("HTTP/1.1 101"),
        "proxy must relay the 101 Switching Protocols, got: {head_str:?}"
    );

    // Post-upgrade the connection is an opaque tunnel — send bytes, get the echo.
    stream.write_all(b"hello-websocket").expect("write frame");
    stream.flush().ok();
    let mut echo = vec![0u8; 15];
    stream.read_exact(&mut echo).expect("read echo");
    assert_eq!(&echo, b"hello-websocket", "bytes tunnel through the upgrade splice");

    proxy.shutdown();
}
