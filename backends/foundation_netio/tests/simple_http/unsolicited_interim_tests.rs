//! Unsolicited 1xx interim responses are skipped with `Expect` turned off (spec 70 feature 69).
//!
//! WHY: RFC 9110 section 15.2 says a client must accept a 1xx interim response even when it
//! did not ask for one. A server or proxy may send `100 Continue` or `103 Early Hints` anyway.
//! The client skips them in `ReadingHead`. No test proved it end to end with the `Expect`
//! handshake off, which is how the Docker client runs (feature 43).
//!
//! WHAT: A raw server writes canned replies that start with one or more 1xx responses. Each
//! test sends with `expect_continue(false)` and checks the final response only. Every test
//! runs under a hard time limit, so a hang is a failure. TCP and Unix socket helpers follow
//! `expect_continue_timeout_tests.rs`.

use std::io::{Read, Write};
use std::net::TcpListener;
use std::path::PathBuf;
use std::sync::mpsc;
use std::thread;
use std::time::Duration;

use foundation_core::valtron::valtron_test;
use foundation_netio::network_client::HttpClientBuilder;
use foundation_netio::shared::client::body_reader::collect_bytes_from_send_safe;
use foundation_netio::shared::client::http_client::HttpClient;
use foundation_netio::shared::http::{
    Extensions, SendSafeBody, SimpleHeader, SimpleHeaders, SimpleMethod, Status,
};
use foundation_netio::PreparedRequest;

const POST_BODY: &[u8] = b"{\"Image\":\"alpine\"}";

/// Time limit for one raw server read. A client that never finishes its request fails here.
const SERVER_READ_LIMIT: Duration = Duration::from_secs(10);

/// What the raw server saw.
struct Seen {
    head: String,
    body: Vec<u8>,
}

fn find(hay: &[u8], needle: &[u8]) -> Option<usize> {
    hay.windows(needle.len()).position(|w| w == needle)
}

/// Read one request head, then a `Content-Length` body if the head has one, then write
/// `reply` as is. The server never sends an interim response on its own: `reply` holds them.
fn serve<S: Read + Write>(mut stream: S, reply: &[u8]) -> Seen {
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
    let head = String::from_utf8_lossy(&buf[..head_end]).to_lowercase();
    let mut body = buf[head_end..].to_vec();
    let length: usize = head
        .lines()
        .find_map(|l| l.strip_prefix("content-length:"))
        .map_or(0, |v| v.trim().parse().expect("content-length"));
    while body.len() < length {
        let n = stream.read(&mut chunk).expect("server read body");
        assert!(n > 0, "client closed before it sent its body");
        body.extend_from_slice(&chunk[..n]);
    }
    stream.write_all(reply).expect("write reply");
    stream.flush().expect("flush reply");
    Seen { head, body }
}

fn post(url: &str) -> PreparedRequest {
    let mut headers = SimpleHeaders::new();
    headers.insert(SimpleHeader::HOST, vec!["localhost".to_string()]);
    headers.insert(
        SimpleHeader::CONTENT_LENGTH,
        vec![POST_BODY.len().to_string()],
    );
    PreparedRequest {
        method: SimpleMethod::POST,
        url: foundation_core::url::Uri::parse(url).expect("uri"),
        headers,
        body: SendSafeBody::Bytes(POST_BODY.to_vec()),
        extensions: Extensions::new(),
        body_readiness: None,
    }
}

fn get(url: &str) -> PreparedRequest {
    let mut headers = SimpleHeaders::new();
    headers.insert(SimpleHeader::HOST, vec!["localhost".to_string()]);
    PreparedRequest {
        method: SimpleMethod::GET,
        url: foundation_core::url::Uri::parse(url).expect("uri"),
        headers,
        body: SendSafeBody::None,
        extensions: Extensions::new(),
        body_readiness: None,
    }
}

/// Start a raw TCP server that answers one connection with `reply`.
fn tcp_server(reply: &'static [u8]) -> (u16, mpsc::Receiver<Seen>) {
    let listener = TcpListener::bind("127.0.0.1:0").expect("bind");
    let port = listener.local_addr().expect("addr").port();
    let (tx, rx) = mpsc::channel();
    thread::spawn(move || {
        let (socket, _) = listener.accept().expect("accept");
        socket
            .set_read_timeout(Some(SERVER_READ_LIMIT))
            .expect("read timeout");
        tx.send(serve(socket, reply)).expect("report");
    });
    (port, rx)
}

/// A unique temp path for a Unix socket (removed if it already exists).
fn temp_socket_path(tag: &str) -> PathBuf {
    let pid = std::process::id();
    let nanos = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map_or(0, |d| d.as_nanos());
    let path = std::env::temp_dir().join(format!("ewe_netio_interim_{tag}_{pid}_{nanos}.sock"));
    let _ = std::fs::remove_file(&path);
    path
}

/// Start a raw Unix socket server that answers one connection with `reply`.
#[cfg(unix)]
fn unix_server(tag: &str, reply: &'static [u8]) -> (PathBuf, mpsc::Receiver<Seen>) {
    use std::os::unix::net::UnixListener;

    let path = temp_socket_path(tag);
    let listener = UnixListener::bind(&path).expect("bind unix socket");
    let (tx, rx) = mpsc::channel();
    thread::spawn(move || {
        let (socket, _) = listener.accept().expect("accept");
        socket
            .set_read_timeout(Some(SERVER_READ_LIMIT))
            .expect("read timeout");
        tx.send(serve(socket, reply)).expect("report");
    });
    (path, rx)
}

/// The final response must be 200 with `body`. The request must carry no `Expect` header.
fn check_post_ok(seen: &mpsc::Receiver<Seen>, status: &Status) {
    let seen = seen
        .recv_timeout(Duration::from_secs(5))
        .expect("server report");
    assert!(
        !seen.head.contains("expect:"),
        "the client must not send Expect with expect_continue(false): {}",
        seen.head
    );
    assert_eq!(seen.body, POST_BODY, "the server must get the whole body");
    assert_eq!(*status, Status::OK);
}

const REPLY_100_200: &[u8] = b"HTTP/1.1 100 Continue\r\n\r\n\
HTTP/1.1 200 OK\r\nContent-Length: 11\r\nConnection: close\r\n\r\n{\"ok\":true}";

#[valtron_test(timeout = 30_000)]
fn unsolicited_100_before_final_is_skipped_tcp() {
    let (port, seen) = tcp_server(REPLY_100_200);
    let client = HttpClientBuilder::new()
        .max_redirects(0)
        .expect_continue(false)
        .build();
    let response = client
        .send(post(&format!("http://127.0.0.1:{port}/containers/create")))
        .expect("send");
    let status = response.get_status();
    let body = collect_bytes_from_send_safe(response.take_body());
    check_post_ok(&seen, &status);
    assert_eq!(String::from_utf8_lossy(&body), "{\"ok\":true}");
}

#[cfg(unix)]
#[valtron_test(timeout = 30_000)]
fn unsolicited_100_before_final_is_skipped_unix() {
    let (path, seen) = unix_server("100", REPLY_100_200);
    let client = HttpClientBuilder::new()
        .unix_socket(&path)
        .max_redirects(0)
        .expect_continue(false)
        .build();
    let result = client.send(post("http://localhost/containers/create"));
    let _ = std::fs::remove_file(&path);
    let response = result.expect("send");
    let status = response.get_status();
    let body = collect_bytes_from_send_safe(response.take_body());
    check_post_ok(&seen, &status);
    assert_eq!(String::from_utf8_lossy(&body), "{\"ok\":true}");
}

#[valtron_test(timeout = 30_000)]
fn early_hints_103_with_headers_is_skipped() {
    const REPLY: &[u8] = b"HTTP/1.1 103 Early Hints\r\nLink: </a.css>; rel=preload\r\n\r\n\
HTTP/1.1 200 OK\r\nContent-Length: 2\r\nConnection: close\r\n\r\nok";
    let (port, seen) = tcp_server(REPLY);
    let client = HttpClientBuilder::new()
        .max_redirects(0)
        .expect_continue(false)
        .build();
    let response = client
        .send(post(&format!("http://127.0.0.1:{port}/containers/create")))
        .expect("send");
    assert!(
        !response.get_headers_ref().contains_key(&SimpleHeader::LINK),
        "the Link header belongs to the 103, not to the final response"
    );
    let status = response.get_status();
    let body = collect_bytes_from_send_safe(response.take_body());
    check_post_ok(&seen, &status);
    assert_eq!(String::from_utf8_lossy(&body), "ok");
}

#[valtron_test(timeout = 30_000)]
fn several_interim_responses_are_skipped() {
    const REPLY: &[u8] = b"HTTP/1.1 100 Continue\r\n\r\n\
HTTP/1.1 102 Processing\r\n\r\n\
HTTP/1.1 200 OK\r\nContent-Length: 2\r\nConnection: close\r\n\r\nok";
    let (port, seen) = tcp_server(REPLY);
    let client = HttpClientBuilder::new()
        .max_redirects(0)
        .expect_continue(false)
        .build();
    let response = client
        .send(post(&format!("http://127.0.0.1:{port}/containers/create")))
        .expect("send");
    let status = response.get_status();
    let body = collect_bytes_from_send_safe(response.take_body());
    check_post_ok(&seen, &status);
    assert_eq!(String::from_utf8_lossy(&body), "ok");
}

#[valtron_test(timeout = 30_000)]
fn unsolicited_100_on_get_is_skipped() {
    const REPLY: &[u8] = b"HTTP/1.1 100 Continue\r\n\r\n\
HTTP/1.1 200 OK\r\nContent-Length: 2\r\nConnection: close\r\n\r\nok";
    let listener = TcpListener::bind("127.0.0.1:0").expect("bind");
    let port = listener.local_addr().expect("addr").port();
    let (tx, rx) = mpsc::channel();
    thread::spawn(move || {
        let (socket, _) = listener.accept().expect("accept");
        socket
            .set_read_timeout(Some(SERVER_READ_LIMIT))
            .expect("read timeout");
        tx.send(serve(socket, REPLY)).expect("report");
    });
    let client = HttpClientBuilder::new()
        .max_redirects(0)
        .expect_continue(false)
        .build();
    let response = client
        .send(get(&format!("http://127.0.0.1:{port}/version")))
        .expect("send");
    let seen = rx
        .recv_timeout(Duration::from_secs(5))
        .expect("server report");
    assert!(!seen.head.contains("expect:"), "head: {}", seen.head);
    assert_eq!(response.get_status(), Status::OK);
    let body = collect_bytes_from_send_safe(response.take_body());
    assert_eq!(String::from_utf8_lossy(&body), "ok");
}
