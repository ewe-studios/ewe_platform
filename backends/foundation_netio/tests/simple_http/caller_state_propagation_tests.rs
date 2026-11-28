//! Issue #1222: `HttpClient::send` and `send_async` must carry the caller's
//! `PreparedRequest` state to the exchange task.
//!
//! WHY: `NativeHttpClient::build_request` rebuilt a request builder from the
//! incoming `PreparedRequest` and copied only the method, URL and headers.
//! `ClientRequestBuilder::build` then produced a request with empty
//! `extensions` and no `body_readiness`, so a request `Ctx` (which
//! `continue_ctx` reads for the `Expect: 100-continue` wait) and a pushable
//! body's pipe readiness were both dropped on the trait `send` path.
//!
//! WHAT: one test sets a request `Ctx` over a simulated clock and proves the
//! `100-continue` wait uses it; the clock only drives that wait if the
//! extension survived to the `SimpleIncomingRequest` the exchange task builds.
//! One test sets a counting pipe readiness and proves the body writer consulted
//! it while the producer had nothing yet.

use std::io::{Read, Write};
use std::net::{SocketAddr, TcpListener};
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::{mpsc, Arc};
use std::thread;
use std::time::{Duration, Instant};

use foundation_context::{Clock, Ctx, Instant as ClockInstant};
use foundation_core::url::Uri;
use foundation_core::valtron::{valtron_test, EventReadiness, ReadinessWaker, SimulatedClock};
use foundation_netio::network_client::HttpClientBuilder;
use foundation_netio::shared::client::http_client::HttpClient;
use foundation_netio::shared::client::PreparedRequest;
use foundation_netio::shared::http::{
    pushable_request_body_with_readiness, BodyReadiness, Extensions, SendSafeBody, SimpleHeader,
    SimpleHeaders, SimpleMethod, Status,
};

/// The POST body of the first test.
const BODY: &[u8] = b"payload";

fn find(hay: &[u8], needle: &[u8]) -> Option<usize> {
    hay.windows(needle.len())
        .position(|window| window == needle)
}

/// Serve one POST: read the head, never send `100 Continue`, record the time
/// between the head and the body, then reply `200`.
fn serve_without_continue() -> (SocketAddr, mpsc::Receiver<Duration>) {
    let listener = TcpListener::bind("127.0.0.1:0").expect("bind");
    let addr = listener.local_addr().expect("local address");
    let (tx, rx) = mpsc::channel();
    thread::spawn(move || {
        let (mut socket, _) = listener.accept().expect("accept");
        let mut buf = Vec::new();
        let mut chunk = [0u8; 4096];
        let head_end = loop {
            if let Some(end) = find(&buf, b"\r\n\r\n") {
                break end + 4;
            }
            let read = socket.read(&mut chunk).expect("read the request head");
            assert!(read > 0, "the client closed before sending its head");
            buf.extend_from_slice(&chunk[..read]);
        };
        let head_at = Instant::now();
        let head = String::from_utf8_lossy(&buf[..head_end]).to_ascii_lowercase();
        let length: usize = head
            .lines()
            .find_map(|line| line.strip_prefix("content-length:"))
            .map(|value| value.trim().parse().expect("content-length"))
            .expect("the request declares a content-length");
        let mut body = buf[head_end..].to_vec();
        while body.len() < length {
            let read = socket.read(&mut chunk).expect("read the request body");
            assert!(read > 0, "the client closed before sending its body");
            body.extend_from_slice(&chunk[..read]);
        }
        let body_after_head = head_at.elapsed();
        let reply = "HTTP/1.1 200 OK\r\nContent-Length: 2\r\nConnection: close\r\n\r\nok";
        let _ = socket.write_all(reply.as_bytes());
        let _ = socket.flush();
        tx.send(body_after_head).expect("report the body timing");
    });
    (addr, rx)
}

/// A POST prepared request with a fixed-length body.
fn post(url: &str, body: SendSafeBody, host: String) -> PreparedRequest {
    let mut headers = SimpleHeaders::new();
    headers.insert(SimpleHeader::HOST, vec![host]);
    if matches!(body, SendSafeBody::Bytes(_)) {
        headers.insert(SimpleHeader::CONTENT_LENGTH, vec![BODY.len().to_string()]);
    }
    PreparedRequest {
        method: SimpleMethod::POST,
        url: Uri::parse(url).expect("request URI"),
        headers,
        body,
        extensions: Extensions::new(),
        body_readiness: None,
    }
}

/// A clock that is read normally once, then eleven seconds ahead.
///
/// The `100-continue` wait computes its deadline with the first read and
/// compares against it with the next. A request `Ctx` over this clock therefore
/// ends the wait at once, while the engine's real clock (the fallback when the
/// extension is dropped) waits the full ten seconds.
struct JumpingClock {
    inner: SimulatedClock,
    reads: AtomicUsize,
}

impl Clock for JumpingClock {
    fn now(&self) -> ClockInstant {
        let base = self.inner.now();
        if self.reads.fetch_add(1, Ordering::SeqCst) == 0 {
            base
        } else {
            base + Duration::from_secs(11)
        }
    }

    fn elapsed(&self) -> Duration {
        self.inner.elapsed()
    }

    fn wall_now(&self) -> std::time::SystemTime {
        self.inner.wall_now()
    }
}

/// The request `Ctx` extension reaches the exchange task, so the
/// `100-continue` wait runs on its clock.
#[valtron_test(timeout = 30_000)]
fn send_keeps_request_extensions_for_the_exchange_task() {
    let (addr, rx) = serve_without_continue();
    let host = format!("127.0.0.1:{}", addr.port());
    let url = format!("http://{host}/upload");

    let clock = Arc::new(JumpingClock {
        inner: SimulatedClock::new(Duration::ZERO),
        reads: AtomicUsize::new(0),
    });
    let mut request = post(&url, SendSafeBody::Bytes(BODY.to_vec()), host);
    request.extensions.insert(Ctx::fixed(clock));

    // The configured wait is ten seconds: only the request's clock can end it
    // early.
    let client = HttpClientBuilder::new()
        .max_redirects(0)
        .expect_continue_timeout(Duration::from_secs(10))
        .build();

    let response = client.send(request).expect("send the request");
    assert_eq!(response.get_status(), Status::OK);

    let body_after_head = rx
        .recv_timeout(Duration::from_secs(5))
        .expect("server report");
    assert!(
        body_after_head < Duration::from_secs(2),
        "the body arrived {body_after_head:?} after the head; the request's Ctx \
         did not reach the exchange task"
    );
}

/// Counts the writer's readiness checks and delegates to the pipe's readiness.
struct CountingReadiness {
    inner: BodyReadiness,
    checks: Arc<AtomicUsize>,
}

impl EventReadiness for CountingReadiness {
    fn is_ready(&self, dur: Option<Duration>) -> bool {
        self.checks.fetch_add(1, Ordering::SeqCst);
        self.inner.is_ready(dur)
    }

    fn clear(&self) {
        self.inner.clear();
    }

    fn deadline(&self) -> Option<foundation_compact::Instant> {
        self.inner.deadline()
    }

    fn register(&self, waker: &ReadinessWaker) -> bool {
        self.inner.register(waker)
    }
}

/// Serve one chunked POST until its terminating chunk, then reply `200` and
/// report the request bytes.
fn serve_chunked() -> (SocketAddr, mpsc::Receiver<String>) {
    let listener = TcpListener::bind("127.0.0.1:0").expect("bind");
    let addr = listener.local_addr().expect("local address");
    let (tx, rx) = mpsc::channel();
    thread::spawn(move || {
        let (mut socket, _) = listener.accept().expect("accept");
        let mut buf = Vec::new();
        let mut chunk = [0u8; 4096];
        loop {
            if find(&buf, b"0\r\n\r\n").is_some() {
                break;
            }
            let read = socket.read(&mut chunk).expect("read the request");
            if read == 0 {
                break;
            }
            buf.extend_from_slice(&chunk[..read]);
        }
        let reply = "HTTP/1.1 200 OK\r\nContent-Length: 2\r\nConnection: close\r\n\r\nok";
        let _ = socket.write_all(reply.as_bytes());
        let _ = socket.flush();
        tx.send(String::from_utf8_lossy(&buf).into_owned())
            .expect("report the request");
    });
    (addr, rx)
}

/// The pushable body's pipe readiness reaches the body writer, which parks on
/// it while the producer has nothing yet.
#[valtron_test(timeout = 30_000)]
fn send_keeps_body_readiness_for_the_body_writer() {
    let (addr, rx) = serve_chunked();
    let host = format!("127.0.0.1:{}", addr.port());
    let url = format!("http://{host}/stream");

    let (pushable, body, readiness) = pushable_request_body_with_readiness(4);
    let checks = Arc::new(AtomicUsize::new(0));
    let counted: BodyReadiness = Arc::new(CountingReadiness {
        inner: readiness,
        checks: checks.clone(),
    });

    let producer = thread::spawn(move || {
        thread::sleep(Duration::from_millis(300));
        pushable.try_push("hello").expect("push the body chunk");
        pushable.close();
    });

    let mut request = post(&url, body, host);
    request.body_readiness = Some(counted);

    // The first test owns the `100-continue` handshake; this one only needs the
    // body writer.
    let client = HttpClientBuilder::new()
        .max_redirects(0)
        .expect_continue(false)
        .build();
    let response = client.send(request).expect("send the request");
    producer.join().expect("body producer");
    assert_eq!(response.get_status(), Status::OK);

    let seen = rx
        .recv_timeout(Duration::from_secs(5))
        .expect("server report");
    assert!(
        seen.contains("hello"),
        "the pushed body did not reach the server: {seen:?}"
    );
    assert!(
        checks.load(Ordering::SeqCst) > 0,
        "the body writer never consulted the request's pipe readiness; \
         body_readiness was dropped on the send path"
    );
}
