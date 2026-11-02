//! Feature 43 (spec 70): a POST must not stall when the server never sends `100 Continue`.
//!
//! WHY: Against Docker every POST with a body waited 2 minutes before its body went out.
//! The client sends `Expect: 100-continue`. RFC 9110 section 10.1.1 says it must send the
//! body after a short wait if no `100` arrives. The wait is `expect_continue_read_timeout`.
//!
//! WHAT: A raw server reads the request head, never sends `100 Continue`, and records when
//! the body arrives. The client sets `expect_continue_read_timeout` to 300 ms. The server
//! must receive the body within 1 second, over TCP and over a Unix socket.

use std::io::{Read, Write};
use std::net::TcpListener;
use std::sync::mpsc;
use std::thread;
use std::time::{Duration, Instant};

use foundation_core::valtron::valtron_test;
use foundation_netio::network_client::HttpClientBuilder;
use foundation_netio::shared::client::http_client::HttpClient;
use foundation_netio::shared::http::{
    Extensions, SendSafeBody, SimpleHeader, SimpleHeaders, SimpleMethod, Status,
};
use foundation_netio::PreparedRequest;

const BODY: &[u8] = b"{\"Image\":\"alpine\"}";

/// What the raw server saw.
struct Seen {
    head: String,
    body: Vec<u8>,
    /// Time from the end of the head to the end of the body.
    body_after_head: Duration,
}

fn find(hay: &[u8], needle: &[u8]) -> Option<usize> {
    hay.windows(needle.len()).position(|w| w == needle)
}

/// Read one request (head, then a `Content-Length` body), never send `100`, reply `200`.
fn handle<S: Read + Write>(mut stream: S) -> Seen {
    let mut buf = Vec::new();
    let mut chunk = [0u8; 4096];
    let head_end = loop {
        if let Some(i) = find(&buf, b"\r\n\r\n") {
            break i + 4;
        }
        let n = stream.read(&mut chunk).expect("server read head");
        assert!(n > 0, "client closed before it sent its head");
        buf.extend_from_slice(&chunk[..n]);
    };
    let head_at = Instant::now();
    let head = String::from_utf8_lossy(&buf[..head_end]).to_lowercase();
    let mut body = buf[head_end..].to_vec();
    let length: usize = head
        .lines()
        .find_map(|l| l.strip_prefix("content-length:"))
        .map(|v| v.trim().parse().expect("content-length"))
        .expect("the request has a content-length");
    while body.len() < length {
        let n = stream.read(&mut chunk).expect("server read body");
        assert!(n > 0, "client closed before it sent its body");
        body.extend_from_slice(&chunk[..n]);
    }
    let body_after_head = head_at.elapsed();
    let reply = "HTTP/1.1 200 OK\r\nContent-Length: 2\r\nConnection: close\r\n\r\nok";
    stream.write_all(reply.as_bytes()).expect("write reply");
    stream.flush().expect("flush reply");
    Seen {
        head,
        body,
        body_after_head,
    }
}

fn post(url: &str) -> PreparedRequest {
    let mut headers = SimpleHeaders::new();
    headers.insert(SimpleHeader::HOST, vec!["localhost".to_string()]);
    headers.insert(SimpleHeader::CONTENT_LENGTH, vec![BODY.len().to_string()]);
    PreparedRequest {
        method: SimpleMethod::POST,
        url: foundation_core::url::Uri::parse(url).expect("uri"),
        headers,
        body: SendSafeBody::Bytes(BODY.to_vec()),
        extensions: Extensions::new(),
        body_readiness: None,
    }
}

fn check(seen: &mpsc::Receiver<Seen>, status: Status, total: Duration) {
    assert_eq!(status, Status::OK);
    let seen = seen
        .recv_timeout(Duration::from_secs(5))
        .expect("server report");
    assert!(
        seen.head.contains("expect: 100-continue"),
        "the test needs the handshake on: {}",
        seen.head
    );
    assert_eq!(seen.body, BODY);
    assert!(
        seen.body_after_head < Duration::from_secs(1),
        "the body arrived {:?} after the head; the 300 ms timer was lost",
        seen.body_after_head
    );
    assert!(total < Duration::from_secs(2), "the POST took {total:?}");
}

#[valtron_test(timeout = 30_000)]
fn expect_continue_sends_body_after_timeout_tcp() {
    let listener = TcpListener::bind("127.0.0.1:0").expect("bind");
    let port = listener.local_addr().expect("addr").port();
    let (tx, rx) = mpsc::channel();
    thread::spawn(move || {
        let (socket, _) = listener.accept().expect("accept");
        tx.send(handle(socket)).expect("report");
    });

    let client = HttpClientBuilder::new()
        .max_redirects(0)
        .expect_continue_timeout(Duration::from_millis(300))
        .build();
    let started = Instant::now();
    let response = client
        .send(post(&format!("http://127.0.0.1:{port}/containers/create")))
        .expect("send");
    check(&rx, response.get_status(), started.elapsed());
}

#[cfg(unix)]
#[valtron_test(timeout = 30_000)]
fn expect_continue_sends_body_after_timeout_unix() {
    use std::os::unix::net::UnixListener;

    let path = std::env::temp_dir().join(format!(
        "ewe_netio_expect_{}_{}.sock",
        std::process::id(),
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map_or(0, |d| d.as_nanos())
    ));
    let _ = std::fs::remove_file(&path);
    let listener = UnixListener::bind(&path).expect("bind unix socket");
    let (tx, rx) = mpsc::channel();
    thread::spawn(move || {
        let (socket, _) = listener.accept().expect("accept");
        tx.send(handle(socket)).expect("report");
    });

    let client = HttpClientBuilder::new()
        .unix_socket(&path)
        .max_redirects(0)
        .expect_continue_timeout(Duration::from_millis(300))
        .build();
    let started = Instant::now();
    let result = client.send(post("http://localhost/containers/create"));
    let elapsed = started.elapsed();
    let _ = std::fs::remove_file(&path);
    let response = result.expect("send");
    check(&rx, response.get_status(), elapsed);
}

/// Root cause of the Docker stall: `HttpClientBuilder::read_timeout` also set the
/// `Expect: 100-continue` wait. Docker built its client with `read_timeout(120 s)`, so a
/// POST waited 120 s for a `100` that never came. A read timeout must not change the
/// handshake wait.
#[cfg(unix)]
#[valtron_test(timeout = 60_000)]
fn read_timeout_does_not_lengthen_expect_continue_wait_unix() {
    use std::os::unix::net::UnixListener;

    let path = std::env::temp_dir().join(format!(
        "ewe_netio_expect_rt_{}_{}.sock",
        std::process::id(),
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map_or(0, |d| d.as_nanos())
    ));
    let _ = std::fs::remove_file(&path);
    let listener = UnixListener::bind(&path).expect("bind unix socket");
    let (tx, rx) = mpsc::channel();
    thread::spawn(move || {
        let (socket, _) = listener.accept().expect("accept");
        tx.send(handle(socket)).expect("report");
    });

    // The handshake wait is set first and `read_timeout` second: the old code let the
    // second call overwrite the first.
    let client = HttpClientBuilder::new()
        .unix_socket(&path)
        .max_redirects(0)
        .expect_continue_timeout(Duration::from_millis(300))
        .read_timeout(Duration::from_secs(10))
        .build();
    let started = Instant::now();
    let result = client.send(post("http://localhost/containers/create"));
    let elapsed = started.elapsed();
    let _ = std::fs::remove_file(&path);
    let response = result.expect("send");
    check(&rx, response.get_status(), elapsed);
}

#[valtron_test(timeout = 60_000)]
fn read_timeout_does_not_lengthen_expect_continue_wait_tcp() {
    let listener = TcpListener::bind("127.0.0.1:0").expect("bind");
    let port = listener.local_addr().expect("addr").port();
    let (tx, rx) = mpsc::channel();
    thread::spawn(move || {
        let (socket, _) = listener.accept().expect("accept");
        tx.send(handle(socket)).expect("report");
    });

    let client = HttpClientBuilder::new()
        .max_redirects(0)
        .expect_continue_timeout(Duration::from_millis(300))
        .read_timeout(Duration::from_secs(10))
        .build();
    let started = Instant::now();
    let response = client
        .send(post(&format!("http://127.0.0.1:{port}/containers/create")))
        .expect("send");
    check(&rx, response.get_status(), started.elapsed());
}
