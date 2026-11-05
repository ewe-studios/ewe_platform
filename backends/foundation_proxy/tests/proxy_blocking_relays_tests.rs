//! The upgrade splice, the raw TCP tunnel and the TCP passthrough relay as
//! async futures, not threads (spec 70 feature 65, E1 and E2).
//!
//! WHY: each of these paths started one OS thread per connection and polled its
//! sockets with a thread sleep. The thread count grew with every open tunnel.
//!
//! WHAT: 50 tunnels are held open at once through each path. The tests count
//! the process threads while the tunnels are open and check that bytes still
//! flow both ways. Linux only: the count comes from `/proc/self/task`.

#![cfg(target_os = "linux")]

use std::io::{Read, Write};
use std::net::{SocketAddr, TcpListener, TcpStream};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::time::{Duration, Instant};

use foundation_core::valtron::{valtron_test, BlockingFutureExt};
use foundation_proxy::config::ServiceConfig;
use foundation_proxy::{ProxyConfig, ProxyServer, TcpPassthrough};

const TUNNELS: usize = 50;

/// The tests of this file run one at a time: each counts the threads of the
/// whole process.
static ONE_AT_A_TIME: std::sync::Mutex<()> = std::sync::Mutex::new(());

fn one_at_a_time() -> std::sync::MutexGuard<'static, ()> {
    ONE_AT_A_TIME
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner)
}

fn thread_count() -> usize {
    std::fs::read_dir("/proc/self/task")
        .expect("read /proc/self/task")
        .count()
}

/// A one-thread echo server. With `upgrade` set it first answers a request head
/// with `101 Switching Protocols`. It starts no thread per connection.
struct EchoServer {
    port: u16,
    stop: Arc<AtomicBool>,
}

impl Drop for EchoServer {
    fn drop(&mut self) {
        self.stop.store(true, Ordering::SeqCst);
    }
}

impl EchoServer {
    fn start(upgrade: bool) -> Self {
        let listener = TcpListener::bind("127.0.0.1:0").expect("bind echo");
        listener.set_nonblocking(true).expect("nonblocking");
        let port = listener.local_addr().expect("addr").port();
        let stop = Arc::new(AtomicBool::new(false));
        let t_stop = Arc::clone(&stop);
        std::thread::spawn(move || {
            // Each entry: socket, head bytes seen so far, head done.
            let mut open: Vec<(TcpStream, Vec<u8>, bool)> = Vec::new();
            while !t_stop.load(Ordering::SeqCst) {
                while let Ok((sock, _)) = listener.accept() {
                    sock.set_nonblocking(true).expect("nonblocking conn");
                    open.push((sock, Vec::new(), !upgrade));
                }
                let mut buf = [0u8; 4096];
                for (sock, head, done) in &mut open {
                    let Ok(n) = sock.read(&mut buf) else { continue };
                    if n == 0 {
                        continue;
                    }
                    if *done {
                        let _ = sock.write_all(&buf[..n]);
                        continue;
                    }
                    head.extend_from_slice(&buf[..n]);
                    if let Some(end) = head.windows(4).position(|w| w == b"\r\n\r\n") {
                        let _ = sock.write_all(
                            b"HTTP/1.1 101 Switching Protocols\r\nUpgrade: websocket\r\nConnection: Upgrade\r\n\r\n",
                        );
                        // Bytes after the head are tunnel data: echo them.
                        let rest = head[end + 4..].to_vec();
                        if !rest.is_empty() {
                            let _ = sock.write_all(&rest);
                        }
                        *done = true;
                    }
                }
                std::thread::sleep(Duration::from_millis(2));
            }
        });
        Self { port, stop }
    }
}

fn read_exact_within(stream: &mut TcpStream, want: usize, within: Duration) -> Vec<u8> {
    stream
        .set_read_timeout(Some(Duration::from_millis(200)))
        .expect("timeout");
    let deadline = Instant::now() + within;
    let mut got = Vec::new();
    let mut buf = [0u8; 1024];
    while got.len() < want && Instant::now() < deadline {
        if let Ok(n) = stream.read(&mut buf) {
            if n == 0 {
                break;
            }
            got.extend_from_slice(&buf[..n]);
        }
    }
    got
}

fn start_proxy(backend: &str) -> ProxyServer {
    ProxyServer::start(
        ProxyConfig::new("tunnel.local", "127.0.0.1")
            .bind("127.0.0.1:0")
            .service(ServiceConfig::new("t", "t.local").backend(backend)),
    )
    .block()
    .expect("start proxy")
}

/// Open `TUNNELS` tunnels with `open`, echo one message through each, hold them
/// all open, and return the thread counts before and while they are open.
fn hold_tunnels(addr: SocketAddr, open: impl Fn(SocketAddr) -> TcpStream) -> (usize, usize) {
    // Warm up so lazy threads (reactor, timer) exist before the baseline.
    let mut warm = open(addr);
    warm.write_all(b"warm").expect("warm write");
    assert_eq!(
        read_exact_within(&mut warm, 4, Duration::from_secs(10)),
        b"warm"
    );
    drop(warm);
    std::thread::sleep(Duration::from_millis(300));

    let before = thread_count();
    let mut held: Vec<TcpStream> = (0..TUNNELS).map(|_| open(addr)).collect();
    for (i, stream) in held.iter_mut().enumerate() {
        let msg = format!("hello-{i:03}");
        stream.write_all(msg.as_bytes()).expect("write");
        let got = read_exact_within(stream, msg.len(), Duration::from_secs(20));
        assert_eq!(got, msg.as_bytes(), "tunnel {i} echoes its bytes");
    }
    let during = thread_count();
    drop(held);
    (before, during)
}

fn upgrade_open(addr: SocketAddr) -> TcpStream {
    let mut s = TcpStream::connect(addr).expect("connect");
    s.write_all(
        b"GET /ws HTTP/1.1\r\nHost: t.local\r\nConnection: Upgrade\r\nUpgrade: websocket\r\n\r\n",
    )
    .expect("upgrade request");
    // The 101 comes back before tunnel data does.
    let head = read_exact_within(&mut s, 12, Duration::from_secs(20));
    assert!(
        head.starts_with(b"HTTP/1.1 101"),
        "upgrade answer: {head:?}"
    );
    // Drain the rest of the 101 head so only echoed data is left.
    let mut seen = head;
    while !seen.windows(4).any(|w| w == b"\r\n\r\n") {
        seen.extend(read_exact_within(&mut s, 1, Duration::from_secs(10)));
    }
    s
}

#[valtron_test(seed = 65, threads = 1, timeout = 120_000)]
fn proxy_upgrade_does_not_spawn_threads() {
    let _serial = one_at_a_time();
    let echo = EchoServer::start(true);
    let proxy = start_proxy(&format!("http://127.0.0.1:{}", echo.port));
    let (before, during) = hold_tunnels(proxy.local_addr(), upgrade_open);
    proxy.shutdown();
    assert!(
        during <= before + 3,
        "{TUNNELS} open upgrade tunnels grew the process from {before} to {during} threads"
    );
}

#[valtron_test(seed = 66, threads = 1, timeout = 120_000)]
fn proxy_tcp_tunnel_does_not_spawn_threads() {
    let _serial = one_at_a_time();
    let echo = EchoServer::start(false);
    let proxy = start_proxy(&format!("tcp://127.0.0.1:{}", echo.port));
    let open = |addr: SocketAddr| {
        let mut s = TcpStream::connect(addr).expect("connect");
        // The HTTP request is consumed by the router; the tunnel carries only
        // what follows it.
        s.write_all(b"GET / HTTP/1.1\r\nHost: t.local\r\n\r\n")
            .expect("request");
        s
    };
    let (before, during) = hold_tunnels(proxy.local_addr(), open);
    proxy.shutdown();
    assert!(
        during <= before + 3,
        "{TUNNELS} open tcp tunnels grew the process from {before} to {during} threads"
    );
}

#[valtron_test(seed = 67, threads = 1, timeout = 120_000)]
fn tcp_passthrough_does_not_spawn_threads() {
    let _serial = one_at_a_time();
    let echo = EchoServer::start(false);
    let passthrough = TcpPassthrough::start("127.0.0.1:0", &format!("127.0.0.1:{}", echo.port))
        .expect("start passthrough");
    let open = |addr: SocketAddr| TcpStream::connect(addr).expect("connect");
    let (before, during) = hold_tunnels(passthrough.local_addr(), open);
    passthrough.shutdown();
    assert!(
        during <= before + 3,
        "{TUNNELS} open passthrough clients grew the process from {before} to {during} threads"
    );
}
