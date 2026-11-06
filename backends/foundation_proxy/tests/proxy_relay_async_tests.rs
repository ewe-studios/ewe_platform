//! The proxy relays an HTTP/1.1 request as an async future, not on a thread
//! (spec 70 feature 45, E1).
//!
//! WHY: the relay used to start one OS thread per proxied connection and return
//! `ConnectionResult::Take`. A burst of slow upstreams then grew the process
//! thread count without bound. The relay is now a `'static` future that parks on
//! socket readiness, so a one-worker pool carries any number of open relays.
//!
//! WHAT: a "gate" upstream holds its replies until the test releases them, so
//! every relay stays open at the same moment. The tests count process threads
//! while the relays are open, and check that a fast request is answered while a
//! slow relay is parked on the only worker.

use std::io::{Read, Write};
use std::net::{SocketAddr, TcpListener, TcpStream};
use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};
use std::sync::Arc;
use std::time::{Duration, Instant};

use foundation_core::valtron::{valtron_test, BlockingFutureExt};
use foundation_proxy::config::ServiceConfig;
use foundation_proxy::{ProxyConfig, ProxyServer};

/// A one-thread upstream. While `hold` is set it keeps every accepted request
/// unanswered; once `hold` is cleared it answers each held request. It never
/// starts a thread per connection, so the test's own thread count stays flat.
struct GateUpstream {
    port: u16,
    hold: Arc<AtomicBool>,
    held: Arc<AtomicUsize>,
    stop: Arc<AtomicBool>,
}

impl Drop for GateUpstream {
    fn drop(&mut self) {
        self.stop.store(true, Ordering::SeqCst);
    }
}

const BODY: &str = "upstream-ok";

/// The tests of this file run one at a time: `proxy_relay_does_not_spawn_threads`
/// counts the threads of the whole process, so no other test may start threads
/// while it measures.
static ONE_AT_A_TIME: std::sync::Mutex<()> = std::sync::Mutex::new(());

fn one_at_a_time() -> std::sync::MutexGuard<'static, ()> {
    ONE_AT_A_TIME
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner)
}

fn response() -> String {
    format!(
        "HTTP/1.1 200 OK\r\nContent-Type: text/plain\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{BODY}",
        BODY.len()
    )
}

impl GateUpstream {
    // `hold`/`held`/`stop` name the three probe flags.
    #[allow(clippy::similar_names)]
    fn start(hold_at_start: bool) -> Self {
        let listener = TcpListener::bind("127.0.0.1:0").expect("bind upstream");
        listener.set_nonblocking(true).expect("nonblocking");
        let port = listener.local_addr().expect("addr").port();
        let hold = Arc::new(AtomicBool::new(hold_at_start));
        let held = Arc::new(AtomicUsize::new(0));
        let stop = Arc::new(AtomicBool::new(false));
        let (t_hold, t_held, t_stop) = (Arc::clone(&hold), Arc::clone(&held), Arc::clone(&stop));
        std::thread::spawn(move || {
            // Each entry: the socket and the bytes of its request head so far.
            let mut open: Vec<(TcpStream, Vec<u8>, bool)> = Vec::new();
            while !t_stop.load(Ordering::SeqCst) {
                while let Ok((sock, _)) = listener.accept() {
                    sock.set_nonblocking(true).expect("nonblocking conn");
                    open.push((sock, Vec::new(), false));
                }
                let holding = t_hold.load(Ordering::SeqCst);
                let mut buf = [0u8; 4096];
                for (sock, head, answered) in &mut open {
                    if *answered {
                        continue;
                    }
                    if !head.windows(4).any(|w| w == b"\r\n\r\n") {
                        if let Ok(n) = sock.read(&mut buf) {
                            head.extend_from_slice(&buf[..n]);
                        }
                    }
                    if head.windows(4).any(|w| w == b"\r\n\r\n") && !holding {
                        // The reply is small; the socket send buffer takes it whole.
                        let _ = sock.write_all(response().as_bytes());
                        let _ = sock.flush();
                        *answered = true;
                    }
                }
                let waiting = open
                    .iter()
                    .filter(|(_, head, answered)| {
                        !*answered && head.windows(4).any(|w| w == b"\r\n\r\n")
                    })
                    .count();
                t_held.store(waiting, Ordering::SeqCst);
                std::thread::sleep(Duration::from_millis(5));
            }
        });
        Self {
            port,
            hold,
            held,
            stop,
        }
    }

    fn url(&self) -> String {
        format!("http://127.0.0.1:{}", self.port)
    }

    fn release(&self) {
        self.hold.store(false, Ordering::SeqCst);
    }

    fn wait_held(&self, count: usize, within: Duration) {
        let deadline = Instant::now() + within;
        while self.held.load(Ordering::SeqCst) < count {
            assert!(
                Instant::now() < deadline,
                "upstream saw {} of {count} requests in time",
                self.held.load(Ordering::SeqCst)
            );
            std::thread::sleep(Duration::from_millis(10));
        }
    }
}

fn start_proxy(services: Vec<ServiceConfig>) -> ProxyServer {
    start_proxy_with_limit(services, 1024 * 1024)
}

fn start_proxy_with_limit(services: Vec<ServiceConfig>, limit: usize) -> ProxyServer {
    let mut config = ProxyConfig::new("relay.local", "127.0.0.1")
        .bind("127.0.0.1:0")
        .max_request_body(limit);
    for svc in services {
        config = config.service(svc);
    }
    ProxyServer::start(
        config,
        std::sync::Arc::new(foundation_core::valtron::MonotonicClock::new()),
    )
    .block()
    .expect("start proxy")
}

fn open_request(addr: SocketAddr, host: &str) -> TcpStream {
    let mut stream = TcpStream::connect(addr).expect("connect");
    stream
        .set_read_timeout(Some(Duration::from_secs(20)))
        .expect("read timeout");
    stream
        .write_all(
            format!("GET / HTTP/1.1\r\nHost: {host}\r\nConnection: close\r\n\r\n").as_bytes(),
        )
        .expect("write request");
    stream
}

fn read_all(mut stream: TcpStream) -> String {
    let mut raw = Vec::new();
    let _ = stream.read_to_end(&mut raw);
    String::from_utf8_lossy(&raw).to_string()
}

#[cfg(target_os = "linux")]
fn thread_count() -> usize {
    std::fs::read_dir("/proc/self/task")
        .expect("read /proc/self/task")
        .count()
}

#[cfg(target_os = "linux")]
#[valtron_test(seed = 45, threads = 1, timeout = 60000)]
fn proxy_relay_does_not_spawn_threads() {
    let _serial = one_at_a_time();
    const OPEN_RELAYS: usize = 50;
    let upstream = GateUpstream::start(false);
    let proxy = start_proxy(vec![
        ServiceConfig::new("gate", "gate.local").backend(&upstream.url())
    ]);
    let addr = proxy.local_addr();

    // Warm up: the first request starts the lazy parts (reactor, client pool).
    let first = read_all(open_request(addr, "gate.local"));
    assert!(first.starts_with("HTTP/1.1 200"), "warm-up: {first}");

    upstream.hold.store(true, Ordering::SeqCst);
    let before = thread_count();
    let clients: Vec<TcpStream> = (0..OPEN_RELAYS)
        .map(|_| open_request(addr, "gate.local"))
        .collect();
    upstream.wait_held(OPEN_RELAYS, Duration::from_secs(30));
    let during = thread_count();

    upstream.release();
    for client in clients {
        let reply = read_all(client);
        assert!(reply.starts_with("HTTP/1.1 200"), "relayed reply: {reply}");
        assert!(reply.ends_with(BODY), "relayed body: {reply}");
    }
    proxy.shutdown();

    assert!(
        during <= before + 3,
        "{OPEN_RELAYS} open relays grew the process from {before} to {during} threads"
    );
}

#[valtron_test(seed = 46, threads = 1, timeout = 60000)]
fn proxy_relay_holds_no_worker() {
    let _serial = one_at_a_time();
    let slow = GateUpstream::start(true);
    let fast = GateUpstream::start(false);
    let proxy = start_proxy(vec![
        ServiceConfig::new("slow", "slow.local").backend(&slow.url()),
        ServiceConfig::new("fast", "fast.local").backend(&fast.url()),
    ]);
    let addr = proxy.local_addr();

    // The slow relay is open and unanswered.
    let slow_client = open_request(addr, "slow.local");
    slow.wait_held(1, Duration::from_secs(30));

    // A second request must be answered while the slow relay waits, although
    // the pool has one worker.
    let started = Instant::now();
    let fast_reply = read_all(open_request(addr, "fast.local"));
    let waited = started.elapsed();
    assert!(
        fast_reply.starts_with("HTTP/1.1 200"),
        "fast reply: {fast_reply}"
    );
    assert!(
        waited < Duration::from_secs(5),
        "a second request waited {waited:?} behind a slow upstream"
    );

    slow.release();
    let slow_reply = read_all(slow_client);
    assert!(
        slow_reply.starts_with("HTTP/1.1 200") && slow_reply.ends_with(BODY),
        "slow reply: {slow_reply}"
    );
    proxy.shutdown();
}

/// An upstream that runs `script` on the first connection: it reads the request
/// head, then the script writes whatever it wants.
fn scripted_upstream(script: impl FnOnce(TcpStream) + Send + 'static) -> u16 {
    let listener = TcpListener::bind("127.0.0.1:0").expect("bind scripted upstream");
    let port = listener.local_addr().expect("addr").port();
    std::thread::spawn(move || {
        let (mut sock, _) = listener.accept().expect("accept");
        let mut head = Vec::new();
        let mut buf = [0u8; 1024];
        while !head.windows(4).any(|w| w == b"\r\n\r\n") {
            match sock.read(&mut buf) {
                Ok(0) | Err(_) => break,
                Ok(n) => head.extend_from_slice(&buf[..n]),
            }
        }
        script(sock);
    });
    port
}

fn one_service_proxy(port: u16) -> ProxyServer {
    start_proxy(vec![
        ServiceConfig::new("one", "one.local").backend(&format!("http://127.0.0.1:{port}"))
    ])
}

#[valtron_test(seed = 47, threads = 1, timeout = 60000)]
fn proxy_relay_keeps_a_chunked_body_chunked() {
    let _serial = one_at_a_time();
    let port = scripted_upstream(|mut sock| {
        sock.write_all(
            b"HTTP/1.1 200 OK\r\nTransfer-Encoding: chunked\r\nConnection: close\r\n\r\n\
              5\r\nhello\r\n6\r\n world\r\n0\r\n\r\n",
        )
        .expect("write");
    });
    let proxy = one_service_proxy(port);
    let reply = read_all(open_request(proxy.local_addr(), "one.local"));
    proxy.shutdown();

    let lower = reply.to_lowercase();
    assert!(reply.starts_with("HTTP/1.1 200"), "status: {reply}");
    assert!(
        lower.contains("transfer-encoding: chunked"),
        "chunked framing kept: {reply}"
    );
    let body = reply.split_once("\r\n\r\n").expect("head end").1;
    assert_eq!(
        body, "5\r\nhello\r\n6\r\n world\r\n0\r\n\r\n",
        "body is re-framed chunk by chunk"
    );
}

#[valtron_test(seed = 48, threads = 1, timeout = 60000)]
fn proxy_relay_streams_a_large_sized_body_whole() {
    let _serial = one_at_a_time();
    const SIZE: usize = 4 * 1024 * 1024;
    let port = scripted_upstream(|mut sock| {
        sock.write_all(
            format!("HTTP/1.1 200 OK\r\nContent-Length: {SIZE}\r\nConnection: close\r\n\r\n")
                .as_bytes(),
        )
        .expect("head");
        let block = vec![b'x'; 64 * 1024];
        for _ in 0..(SIZE / block.len()) {
            sock.write_all(&block).expect("block");
        }
    });
    let proxy = one_service_proxy(port);
    let reply = read_all(open_request(proxy.local_addr(), "one.local"));
    proxy.shutdown();

    let (head, body) = reply.split_once("\r\n\r\n").expect("head end");
    assert!(head.starts_with("HTTP/1.1 200"), "status: {head}");
    assert!(
        head.to_lowercase()
            .contains(&format!("content-length: {SIZE}")),
        "length kept: {head}"
    );
    assert_eq!(body.len(), SIZE, "every byte arrived, none repeated");
}

#[valtron_test(seed = 49, threads = 1, timeout = 60000)]
fn proxy_relay_answers_502_when_the_upstream_hangs_up() {
    let _serial = one_at_a_time();
    let port = scripted_upstream(drop);
    let proxy = one_service_proxy(port);
    let reply = read_all(open_request(proxy.local_addr(), "one.local"));
    proxy.shutdown();

    assert!(reply.starts_with("HTTP/1.1 502"), "502 answer: {reply}");
    assert!(reply.ends_with("Bad Gateway"), "502 body: {reply}");
}

/// An upstream that answers `200` once it has read a request head and the whole
/// body (`Content-Length` or chunked). It starts one thread per connection (test code).
fn body_reading_upstream() -> u16 {
    let listener = TcpListener::bind("127.0.0.1:0").expect("bind body upstream");
    let port = listener.local_addr().expect("addr").port();
    std::thread::spawn(move || {
        while let Ok((mut sock, _)) = listener.accept() {
            std::thread::spawn(move || {
                let mut data = Vec::new();
                let mut buf = [0u8; 1024];
                loop {
                    if let Some(end) = data.windows(4).position(|w| w == b"\r\n\r\n") {
                        let head = String::from_utf8_lossy(&data[..end]).to_lowercase();
                        let want = head
                            .lines()
                            .find_map(|l| l.strip_prefix("content-length:"))
                            .and_then(|v| v.trim().parse::<usize>().ok())
                            .unwrap_or(0);
                        let chunked = head.contains("transfer-encoding: chunked");
                        if chunked && data.ends_with(b"0\r\n\r\n") {
                            break;
                        }
                        if !chunked && data.len() >= end + 4 + want {
                            break;
                        }
                    }
                    match sock.read(&mut buf) {
                        Ok(0) | Err(_) => return,
                        Ok(n) => data.extend_from_slice(&buf[..n]),
                    }
                }
                let _ = sock.write_all(response().as_bytes());
            });
        }
    });
    port
}

#[valtron_test(seed = 50, threads = 2, timeout = 60000)]
fn slow_client_upload_does_not_hold_worker() {
    let _serial = one_at_a_time();
    let upload_port = body_reading_upstream();
    let fast = GateUpstream::start(false);
    let proxy = start_proxy(vec![
        ServiceConfig::new("up", "up.local").backend(&format!("http://127.0.0.1:{upload_port}")),
        ServiceConfig::new("fast", "fast.local").backend(&fast.url()),
    ]);
    let addr = proxy.local_addr();

    // Two uploads of 40 bytes, one byte every 50 ms (2 s each): one per worker.
    let uploads: Vec<_> = (0..2)
        .map(|_| {
            std::thread::spawn(move || {
                let mut s = TcpStream::connect(addr).expect("connect");
                s.set_read_timeout(Some(Duration::from_secs(20))).ok();
                s.write_all(
                    b"POST / HTTP/1.1\r\nHost: up.local\r\nContent-Length: 40\r\nConnection: close\r\n\r\n",
                )
                .expect("head");
                for _ in 0..40 {
                    if s.write_all(b"x").is_err() {
                        break;
                    }
                    std::thread::sleep(Duration::from_millis(50));
                }
                read_all(s)
            })
        })
        .collect();
    std::thread::sleep(Duration::from_millis(400));

    // A complete request must answer while both uploads are open.
    let started = Instant::now();
    let fast_reply = read_all(open_request(addr, "fast.local"));
    let waited = started.elapsed();
    assert!(
        fast_reply.starts_with("HTTP/1.1 200"),
        "fast reply: {fast_reply}"
    );
    assert!(
        waited < Duration::from_millis(500),
        "a second request waited {waited:?} behind slow uploads"
    );

    // The slow bodies must arrive whole and be answered.
    for upload in uploads {
        let reply = upload.join().expect("upload client");
        assert!(
            reply.starts_with("HTTP/1.1 200") && reply.ends_with(BODY),
            "upload reply: {reply}"
        );
    }
    proxy.shutdown();
}

fn post_raw(addr: SocketAddr, head_extra: &str, body: &[u8]) -> String {
    let mut s = TcpStream::connect(addr).expect("connect");
    s.set_read_timeout(Some(Duration::from_secs(20))).ok();
    s.write_all(
        format!("POST / HTTP/1.1\r\nHost: up.local\r\nConnection: close\r\n{head_extra}\r\n")
            .as_bytes(),
    )
    .expect("head");
    let _ = s.write_all(body);
    read_all(s)
}

#[valtron_test(seed = 51, threads = 2, timeout = 60000)]
fn request_body_over_the_limit_is_413() {
    let _serial = one_at_a_time();
    let port = body_reading_upstream();
    let proxy = start_proxy_with_limit(
        vec![ServiceConfig::new("up", "up.local").backend(&format!("http://127.0.0.1:{port}"))],
        100,
    );
    let addr = proxy.local_addr();

    // A declared length over the limit is refused before the body is read.
    let declared = post_raw(addr, "Content-Length: 1000\r\n", &[b'x'; 1000]);
    assert!(declared.starts_with("HTTP/1.1 413"), "declared: {declared}");

    // A chunked body that passes the limit while streaming is refused too.
    let mut chunked = Vec::new();
    for _ in 0..3 {
        chunked.extend_from_slice(b"64\r\n");
        chunked.extend_from_slice(&[b'y'; 100]);
        chunked.extend_from_slice(b"\r\n");
    }
    chunked.extend_from_slice(b"0\r\n\r\n");
    let streamed = post_raw(addr, "Transfer-Encoding: chunked\r\n", &chunked);
    assert!(streamed.starts_with("HTTP/1.1 413"), "streamed: {streamed}");

    // A body at the limit passes; a limit of 0 turns the check off.
    let at_limit = post_raw(addr, "Content-Length: 100\r\n", &[b'z'; 100]);
    assert!(at_limit.starts_with("HTTP/1.1 200"), "at limit: {at_limit}");
    proxy.shutdown();

    let port = body_reading_upstream();
    let open = start_proxy_with_limit(
        vec![ServiceConfig::new("up", "up.local").backend(&format!("http://127.0.0.1:{port}"))],
        0,
    );
    let body = vec![b'q'; 300_000];
    let big = post_raw(open.local_addr(), "Content-Length: 300000\r\n", &body);
    assert!(big.starts_with("HTTP/1.1 200"), "no limit: {big}");
    open.shutdown();
}
