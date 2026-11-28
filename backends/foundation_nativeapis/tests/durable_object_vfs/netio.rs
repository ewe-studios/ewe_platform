//! Tests for the native object transport over a local `TcpListener` server.
//!
//! The server records the request line, the headers and the body, then sends a
//! canned response. No external service is involved.

#![cfg(feature = "vfs-object-netio")]

use foundation_core::valtron::valtron_test;
use foundation_nativeapis::native::vfs::object_http::{
    HttpMethod, ObjectRequest, ObjectTransport,
};
use foundation_nativeapis::native::vfs::object_netio::NetioObjectTransport;
use foundation_netio::HttpClientBuilder;
use std::io::{Read, Write};
use std::net::{SocketAddr, TcpListener};
use std::sync::{Arc, Mutex};
use std::thread;
use std::time::Duration;

#[derive(Clone, Debug, Default)]
struct Recorded {
    method: String,
    target: String,
    version: String,
    headers: Vec<(String, String)>,
    body: Vec<u8>,
}

impl Recorded {
    fn header(&self, name: &str) -> Option<&str> {
        self.headers
            .iter()
            .find(|(key, _)| key.eq_ignore_ascii_case(name))
            .map(|(_, value)| value.as_str())
    }
}

fn find(haystack: &[u8], needle: &[u8]) -> Option<usize> {
    haystack
        .windows(needle.len())
        .position(|window| window == needle)
}

/// Start a one-connection server that records one request and writes
/// `response`.
fn serve(response: Vec<u8>) -> (SocketAddr, Arc<Mutex<Recorded>>, thread::JoinHandle<()>) {
    let listener = TcpListener::bind(("127.0.0.1", 0)).expect("bind test server");
    let addr = listener.local_addr().expect("local addr");
    let recorded = Arc::new(Mutex::new(Recorded::default()));
    let sink = Arc::clone(&recorded);
    let handle = thread::spawn(move || {
        let (mut stream, _peer) = listener.accept().expect("accept");
        stream
            .set_read_timeout(Some(Duration::from_secs(5)))
            .expect("read timeout");
        let mut buffer = Vec::new();
        let mut chunk = [0_u8; 1024];
        let header_end = loop {
            if let Some(index) = find(&buffer, b"\r\n\r\n") {
                break index + 4;
            }
            let read = stream.read(&mut chunk).expect("read request");
            assert!(read > 0, "the client closed before it sent its headers");
            buffer.extend_from_slice(&chunk[..read]);
        };

        let head = String::from_utf8_lossy(&buffer[..header_end]).into_owned();
        let mut lines = head.split("\r\n");
        let request_line = lines.next().unwrap_or_default();
        let mut parts = request_line.split_whitespace();
        let method = parts.next().unwrap_or_default().to_string();
        let target = parts.next().unwrap_or_default().to_string();
        let version = parts.next().unwrap_or_default().to_string();
        let mut headers = Vec::new();
        for line in lines {
            if line.is_empty() {
                continue;
            }
            if let Some((name, value)) = line.split_once(':') {
                headers.push((name.trim().to_string(), value.trim().to_string()));
            }
        }
        let content_length = headers
            .iter()
            .find(|(name, _)| name.eq_ignore_ascii_case("content-length"))
            .and_then(|(_, value)| value.parse::<usize>().ok())
            .unwrap_or(0);
        let mut body = buffer[header_end..].to_vec();
        while body.len() < content_length {
            let read = stream.read(&mut chunk).expect("read body");
            assert!(read > 0, "the client closed before it sent its body");
            body.extend_from_slice(&chunk[..read]);
        }
        body.truncate(content_length);

        {
            let mut sink = sink.lock().expect("record lock");
            sink.method = method;
            sink.target = target;
            sink.version = version;
            sink.headers = headers;
            sink.body = body;
        }
        stream.write_all(&response).expect("write response");
        stream.flush().expect("flush response");
    });
    (addr, recorded, handle)
}

fn client() -> NetioObjectTransport {
    // The tests drive one request at a time. `Expect: 100-continue` would make
    // the one-shot server answer twice, so it is off.
    let client = HttpClientBuilder::new().expect_continue(false).build();
    NetioObjectTransport::with_client(client)
}

#[valtron_test]
async fn netio_transport_sends_a_signed_request() {
    let response = b"HTTP/1.1 200 OK\r\nETag: \"v1\"\r\nContent-Length: 2\r\nConnection: close\r\n\r\nok".to_vec();
    let (addr, recorded, server) = serve(response);
    let transport = client();
    let request = ObjectRequest {
        method: HttpMethod::Put,
        url: format!("http://{addr}/bucket/durable/a/HEAD"),
        query: Vec::new(),
        headers: vec![
            ("host".to_string(), addr.to_string()),
            ("if-none-match".to_string(), "*".to_string()),
            (
                "authorization".to_string(),
                "AWS4-HMAC-SHA256 Credential=AKID/test".to_string(),
            ),
            ("x-amz-date".to_string(), "20260101T000000Z".to_string()),
        ],
        body: b"payload".to_vec(),
        max_response_bytes: 1024,
    };
    let response = transport.send(request).await.expect("send");
    server.join().expect("server thread");

    assert_eq!(response.status, 200);
    assert_eq!(response.body, b"ok");
    assert_eq!(response.header("etag"), Some("\"v1\""));

    let recorded = recorded.lock().expect("record lock");
    assert_eq!(recorded.method, "PUT");
    assert_eq!(recorded.target, "/bucket/durable/a/HEAD");
    assert_eq!(recorded.version, "HTTP/1.1");
    assert_eq!(recorded.header("if-none-match"), Some("*"));
    assert_eq!(
        recorded.header("authorization"),
        Some("AWS4-HMAC-SHA256 Credential=AKID/test")
    );
    assert_eq!(recorded.header("host"), Some(addr.to_string().as_str()));
    assert_eq!(recorded.body, b"payload");
}

#[valtron_test]
async fn netio_transport_bounds_the_body() {
    let payload = vec![b'x'; 4096];
    let mut response = format!(
        "HTTP/1.1 200 OK\r\nContent-Length: {}\r\nConnection: close\r\n\r\n",
        payload.len()
    )
    .into_bytes();
    response.extend_from_slice(&payload);
    let (addr, _recorded, server) = serve(response);
    let transport = client().with_max_response_bytes(64);
    let request = ObjectRequest {
        method: HttpMethod::Get,
        url: format!("http://{addr}/bucket/durable/large"),
        query: Vec::new(),
        headers: Vec::new(),
        body: Vec::new(),
        max_response_bytes: 4096,
    };
    let response = transport.send(request).await.expect("send");
    server.join().expect("server thread");

    assert_eq!(
        response.body.len(),
        64,
        "the transport stops at the configured bound"
    );
    assert!(response.body.iter().all(|byte| *byte == b'x'));
}

#[valtron_test]
async fn netio_transport_returns_a_non_2xx_status() {
    let response = b"HTTP/1.1 404 Not Found\r\nContent-Length: 0\r\nConnection: close\r\n\r\n".to_vec();
    let (addr, _recorded, server) = serve(response);
    let transport = client();
    let request = ObjectRequest {
        method: HttpMethod::Head,
        url: format!("http://{addr}/bucket/durable/missing"),
        query: Vec::new(),
        headers: Vec::new(),
        body: Vec::new(),
        max_response_bytes: 1024,
    };
    let response = transport.send(request).await.expect("send");
    server.join().expect("server thread");
    assert_eq!(
        response.status, 404,
        "a non-2xx status is a response, not a transport error"
    );
}

#[valtron_test]
async fn netio_transport_reads_no_head_body() {
    // A HEAD response names a length but sends no body. The server holds the
    // connection open so a reader that waits for the named bytes is visible.
    let response = b"HTTP/1.1 200 OK\r\nContent-Length: 123\r\nConnection: keep-alive\r\n\r\n".to_vec();
    let listener = TcpListener::bind(("127.0.0.1", 0)).expect("bind test server");
    let addr = listener.local_addr().expect("local addr");
    let server = thread::spawn(move || {
        let (mut stream, _peer) = listener.accept().expect("accept");
        let mut buffer = [0_u8; 1024];
        let _ = stream.read(&mut buffer).expect("read request");
        stream.write_all(&response).expect("write response");
        stream.flush().expect("flush response");
        thread::sleep(Duration::from_secs(2));
    });
    let transport = client();
    let request = ObjectRequest {
        method: HttpMethod::Head,
        url: format!("http://{addr}/bucket/durable/a/HEAD"),
        query: Vec::new(),
        headers: Vec::new(),
        body: Vec::new(),
        max_response_bytes: 1024,
    };
    let start = std::time::Instant::now();
    let response = transport.send(request).await.expect("send");
    assert!(
        start.elapsed() < Duration::from_secs(1),
        "the transport does not wait for a body that a HEAD response never sends"
    );
    assert_eq!(response.status, 200);
    assert!(response.body.is_empty(), "a HEAD response has no body");
    assert_eq!(response.header("content-length"), Some("123"));
    drop(server);
}

