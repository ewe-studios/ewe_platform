//! Non-blocking HTTP/1.1 client sockets: the client parks on readiness instead
//! of holding a valtron worker while it waits on the network.
//!
//! WHY: #76 found each H1 POST spending ~3s in the `Expect: 100-continue` wait.
//! The client blocked a task worker in a socket read. When the server needed a
//! worker on the same pool to send `100 Continue`, it starved until the read timed
//! out. These tests pin the fix:
//!
//! - while the client waits, other valtron tasks still get workers;
//! - `100 Continue` is honoured as soon as it arrives;
//! - a server that never sends `100` still gets the body when the timer fires;
//! - response heads and bodies split across many small writes parse correctly
//!   (no frame torn by a `WouldBlock`), over plain TCP and over rustls;
//! - a streaming request body whose producer is slow is still delivered;
//! - the same holds when an async caller drives the client through valtron's
//!   future adapters.
//!
//! HOW: a small std-thread HTTP/1.1 server (outside the valtron pool) records
//! when it saw the request head, when it sent `100 Continue`, and whether body
//! bytes had already arrived before it did.

use std::io::{Read, Write};
use std::net::{SocketAddr, TcpListener, TcpStream};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{mpsc, Arc};
use std::thread;
use std::time::{Duration, Instant};

use bytes::Bytes;
use foundation_core::url::Uri;
use foundation_core::valtron::{
    self, valtron_test, BoxedSendExecutionAction, Stream, StreamIteratorExt, TaskIterator,
    TaskStatus,
};
use foundation_netio::http::{HttpConnectionPool, HttpExchangeTask};
use foundation_netio::shared::client::request_task::HttpExchange;
use foundation_netio::shared::client::{ClientConfig, PreparedRequest, SystemDnsResolver};
use foundation_netio::shared::http::{
    pushable_request_body_with_depth, Extensions, SendSafeBody, SimpleHeader, SimpleHeaders,
    SimpleMethod, Status,
};
use futures_lite::StreamExt;

// ─────────────────────────────────────────────────────────────────────────────
// Test server
// ─────────────────────────────────────────────────────────────────────────────

/// When the server answers `Expect: 100-continue`.
#[derive(Clone)]
enum Continue {
    /// Right after reading the request head.
    Immediately,
    /// Once this flag is set (by a valtron task, in the worker-starvation test).
    When(Arc<AtomicBool>),
    /// Never: the client must fall back to its timer.
    Never,
}

/// How the server writes its response.
#[derive(Clone)]
enum Reply {
    /// `Content-Length` body, written in one go.
    Whole(&'static str),
    /// The whole response (head and body) written in small pieces with pauses,
    /// so the client sees header blocks and body frames split across reads.
    Trickle(Vec<&'static [u8]>),
}

/// What the server observed for one request.
#[derive(Debug)]
struct Seen {
    head: String,
    /// Body bytes were already waiting when the server was about to send `100`:
    /// the client gave up on the handshake instead of waiting for it.
    body_before_continue: bool,
    continue_sent: Option<Instant>,
    head_at: Instant,
    body_at: Instant,
    body: Vec<u8>,
}

fn find(hay: &[u8], needle: &[u8]) -> Option<usize> {
    hay.windows(needle.len()).position(|w| w == needle)
}

/// Read one HTTP/1.1 request (head + `Content-Length` or chunked body) and reply.
fn handle<S: Read + Write>(
    mut stream: S,
    peek_tcp: Option<&TcpStream>,
    cont: &Continue,
    reply: &Reply,
) -> Seen {
    let mut buf = Vec::new();
    let mut chunk = [0u8; 4096];
    let head_end = loop {
        if let Some(i) = find(&buf, b"\r\n\r\n") {
            break i + 4;
        }
        let n = stream.read(&mut chunk).expect("server read head");
        assert!(n > 0, "client closed before sending its head");
        buf.extend_from_slice(&chunk[..n]);
    };
    let head_at = Instant::now();
    let head = String::from_utf8_lossy(&buf[..head_end]).to_lowercase();
    let mut body: Vec<u8> = buf[head_end..].to_vec();

    let expects = head.contains("expect: 100-continue");
    let mut continue_sent = None;
    let mut body_before_continue = !body.is_empty();
    if expects {
        let send = match cont {
            Continue::Immediately => true,
            Continue::When(flag) => {
                let start = Instant::now();
                while !flag.load(Ordering::SeqCst) && start.elapsed() < Duration::from_secs(20) {
                    thread::sleep(Duration::from_millis(1));
                }
                true
            }
            Continue::Never => false,
        };
        if send {
            if let Some(tcp) = peek_tcp {
                tcp.set_nonblocking(true).expect("peek mode");
                let mut probe = [0u8; 1];
                body_before_continue |= matches!(tcp.peek(&mut probe), Ok(n) if n > 0);
                tcp.set_nonblocking(false).expect("restore mode");
            }
            stream
                .write_all(b"HTTP/1.1 100 Continue\r\n\r\n")
                .expect("write 100");
            stream.flush().expect("flush 100");
            continue_sent = Some(Instant::now());
        }
    }

    let length = head
        .lines()
        .find_map(|l| l.strip_prefix("content-length:"))
        .map(|v| v.trim().parse::<usize>().expect("content-length"));
    let chunked = head.contains("transfer-encoding: chunked");
    loop {
        let done = match length {
            Some(n) => body.len() >= n,
            None if chunked => body.ends_with(b"0\r\n\r\n"),
            None => true,
        };
        if done {
            break;
        }
        let n = stream.read(&mut chunk).expect("server read body");
        assert!(n > 0, "client closed mid-body");
        body.extend_from_slice(&chunk[..n]);
    }
    let body_at = Instant::now();

    match reply {
        Reply::Whole(text) => {
            let out = format!(
                "HTTP/1.1 200 OK\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{text}",
                text.len()
            );
            stream.write_all(out.as_bytes()).expect("write reply");
            stream.flush().expect("flush reply");
        }
        Reply::Trickle(pieces) => {
            for piece in pieces {
                stream.write_all(piece).expect("write piece");
                stream.flush().expect("flush piece");
                thread::sleep(Duration::from_millis(20));
            }
        }
    }

    Seen {
        head,
        body_before_continue,
        continue_sent,
        head_at,
        body_at,
        body,
    }
}

/// Serve `count` plain-TCP requests on loopback; observations arrive on the channel.
fn serve_tcp(count: usize, cont: Continue, reply: Reply) -> (SocketAddr, mpsc::Receiver<Seen>) {
    let listener = TcpListener::bind("127.0.0.1:0").expect("bind");
    let addr = listener.local_addr().expect("addr");
    let (tx, rx) = mpsc::channel();
    thread::spawn(move || {
        for _ in 0..count {
            let (socket, _) = listener.accept().expect("accept");
            let (tx, cont, reply) = (tx.clone(), cont.clone(), reply.clone());
            thread::spawn(move || {
                let peek = socket.try_clone().expect("clone for peek");
                let seen = handle(socket, Some(&peek), &cont, &reply);
                tx.send(seen).expect("report");
            });
        }
    });
    (addr, rx)
}

fn post(url: &str, body: SendSafeBody, length: Option<usize>) -> PreparedRequest {
    let mut headers = SimpleHeaders::new();
    headers.insert(SimpleHeader::HOST, vec!["127.0.0.1".to_string()]);
    if let Some(len) = length {
        headers.insert(SimpleHeader::CONTENT_LENGTH, vec![len.to_string()]);
    }
    PreparedRequest {
        method: SimpleMethod::POST,
        url: Uri::parse(url).expect("uri"),
        headers,
        body,
        extensions: Extensions::new(),
    }
}

fn exchange(
    request: PreparedRequest,
    pool: Arc<HttpConnectionPool<SystemDnsResolver>>,
    config: ClientConfig,
) -> HttpExchangeTask<SystemDnsResolver> {
    HttpExchangeTask::new(request, 0, pool, config)
}

/// Drain an exchange's output into (status, body).
fn collect(task: HttpExchangeTask<SystemDnsResolver>) -> (Status, Vec<u8>) {
    let mut status = None;
    let mut body = Vec::new();
    for item in valtron::execute(task, None).expect("execute") {
        match item {
            Stream::Next(HttpExchange::Head { status: s, .. }) => status = Some(s),
            Stream::Next(HttpExchange::BodyChunk(b)) => body.extend_from_slice(&b),
            Stream::Next(HttpExchange::Failed(e)) => panic!("exchange failed: {e}"),
            _ => {}
        }
    }
    (status.expect("a response head"), body)
}

/// A valtron task that, after a short delay, sets a flag and records when.
///
/// It needs a task worker to run. If clients held every worker in blocking
/// reads, it could not run until their reads timed out.
struct Releaser {
    flag: Arc<AtomicBool>,
    released_at: Arc<std::sync::Mutex<Option<Instant>>>,
    stage: u8,
}

impl TaskIterator for Releaser {
    type Ready = ();
    type Pending = ();
    type Spawner = BoxedSendExecutionAction;

    fn next_status(&mut self) -> Option<TaskStatus<(), (), BoxedSendExecutionAction>> {
        self.stage += 1;
        match self.stage {
            1 => Some(TaskStatus::Delayed(Duration::from_millis(50))),
            2 => {
                *self.released_at.lock().expect("lock") = Some(Instant::now());
                self.flag.store(true, Ordering::SeqCst);
                Some(TaskStatus::Ready(()))
            }
            _ => None,
        }
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// Tests
// ─────────────────────────────────────────────────────────────────────────────

/// More concurrent `Expect: 100-continue` POSTs than there are task workers.
/// The server sends `100` only once a separate valtron task tells it to.
///
/// With blocking sockets every worker sat in a read, the releasing task never
/// ran, and each client gave up on the handshake after its 3s timer. With
/// readiness parking the workers stay free. The release happens within
/// milliseconds, and every client waits for its `100` before sending the body.
#[test]
#[ntest::timeout(60_000)]
#[serial_test::serial]
fn waiting_clients_do_not_hold_workers() {
    // 4 threads = 2 task workers (+ background), fewer than the 4 clients.
    let _guard = valtron::initialize_pool(29_001, Some(4));
    const CLIENTS: usize = 4;

    let flag = Arc::new(AtomicBool::new(false));
    let (addr, seen) = serve_tcp(CLIENTS, Continue::When(flag.clone()), Reply::Whole("ok"));
    let url = format!("http://127.0.0.1:{}/upload", addr.port());
    let pool = Arc::new(HttpConnectionPool::default());
    let config = ClientConfig::default();
    assert_eq!(
        config.get_expect_continue_read_timeout(),
        Duration::from_secs(3)
    );

    let started = Instant::now();
    let clients: Vec<_> = (0..CLIENTS)
        .map(|_| {
            let task = exchange(
                post(&url, SendSafeBody::Bytes(b"hello".to_vec()), Some(5)),
                pool.clone(),
                config.clone(),
            );
            valtron::execute(task, None).expect("execute client")
        })
        .collect();

    // Let every client send its head and start waiting for `100`.
    thread::sleep(Duration::from_millis(200));
    let released_at = Arc::new(std::sync::Mutex::new(None));
    let scheduled = Instant::now();
    let releaser = valtron::execute(
        Releaser {
            flag,
            released_at: released_at.clone(),
            stage: 0,
        },
        None,
    )
    .expect("execute releaser");

    for client in clients {
        let mut status = None;
        let mut body = Vec::new();
        for item in client {
            match item {
                Stream::Next(HttpExchange::Head { status: s, .. }) => status = Some(s),
                Stream::Next(HttpExchange::BodyChunk(b)) => body.extend_from_slice(&b),
                Stream::Next(HttpExchange::Failed(e)) => panic!("exchange failed: {e}"),
                _ => {}
            }
        }
        assert_eq!(status, Some(Status::OK));
        assert_eq!(body, b"ok");
    }
    releaser.for_each(drop);
    let elapsed = started.elapsed();

    let released = released_at
        .lock()
        .expect("lock")
        .expect("the releasing task ran");
    assert!(
        released - scheduled < Duration::from_secs(1),
        "the releasing task waited {:?} for a worker: clients are holding workers",
        released - scheduled
    );
    for _ in 0..CLIENTS {
        let s = seen
            .recv_timeout(Duration::from_secs(5))
            .expect("server report");
        assert!(s.head.contains("expect: 100-continue"));
        assert!(
            !s.body_before_continue,
            "the client sent its body before `100 Continue`: it gave up on the handshake"
        );
        assert_eq!(s.body, b"hello");
        assert!(s.continue_sent.is_some_and(|c| s.body_at >= c));
    }
    assert!(
        elapsed < Duration::from_millis(2_500),
        "{CLIENTS} POSTs took {elapsed:?}; the 3s expect-continue timer must not be hit"
    );
}

/// A server that sends `100 Continue` at once: the body follows immediately,
/// nowhere near the 3s timer.
#[test]
#[ntest::timeout(60_000)]
#[serial_test::serial]
fn continue_from_server_sends_body_at_once() {
    let _guard = valtron::initialize_pool(29_002, Some(4));
    let (addr, seen) = serve_tcp(1, Continue::Immediately, Reply::Whole("done"));
    let url = format!("http://127.0.0.1:{}/upload", addr.port());

    let started = Instant::now();
    let (status, body) = collect(exchange(
        post(&url, SendSafeBody::Bytes(b"payload".to_vec()), Some(7)),
        Arc::new(HttpConnectionPool::default()),
        ClientConfig::default(),
    ));
    let elapsed = started.elapsed();

    assert_eq!(status, Status::OK);
    assert_eq!(body, b"done");
    let s = seen
        .recv_timeout(Duration::from_secs(5))
        .expect("server report");
    assert!(!s.body_before_continue, "body must wait for `100 Continue`");
    assert_eq!(s.body, b"payload");
    assert!(
        elapsed < Duration::from_secs(1),
        "POST took {elapsed:?} against a server that answered `100` at once"
    );
}

/// A server that never sends `100 Continue`: RFC 9110 §10.1.1 says the client
/// sends the body anyway after a wait. The wait is the configured timer, now a
/// readiness wait with a deadline instead of a blocking read timeout.
#[test]
#[ntest::timeout(60_000)]
#[serial_test::serial]
fn silent_server_gets_body_when_the_timer_fires() {
    let _guard = valtron::initialize_pool(29_003, Some(4));
    let (addr, seen) = serve_tcp(1, Continue::Never, Reply::Whole("late"));
    let url = format!("http://127.0.0.1:{}/upload", addr.port());
    let timer = Duration::from_millis(300);
    let config = ClientConfig::default().set_expect_continue_read_timeout(timer);

    let (status, body) = collect(exchange(
        post(&url, SendSafeBody::Bytes(b"anyway".to_vec()), Some(6)),
        Arc::new(HttpConnectionPool::default()),
        config,
    ));

    assert_eq!(status, Status::OK);
    assert_eq!(body, b"late");
    let s = seen
        .recv_timeout(Duration::from_secs(5))
        .expect("server report");
    assert!(s.continue_sent.is_none());
    assert_eq!(s.body, b"anyway");
    let waited = s.body_at - s.head_at;
    assert!(
        waited >= timer - Duration::from_millis(20) && waited < Duration::from_secs(2),
        "body arrived {waited:?} after the head; expected about the {timer:?} timer"
    );
}

/// The response head and a chunked body arrive in many small writes. Header
/// lines, chunk-size lines and chunk data are split across reads. The client
/// must not lose a header line or misframe a chunk on a `WouldBlock`.
#[test]
#[ntest::timeout(60_000)]
#[serial_test::serial]
fn trickled_chunked_response_parses_intact() {
    let _guard = valtron::initialize_pool(29_004, Some(4));
    let pieces: Vec<&'static [u8]> = vec![
        b"HTTP/1.1 200 OK\r\nX-One: ",
        b"1\r\nTransfer-Enc",
        b"oding: chunked\r\nConnection: close\r\n",
        b"\r\n5\r",
        b"\nhel",
        b"lo\r\n",
        b"6\r\n world\r\n0\r\n",
        b"\r\n",
    ];
    let (addr, seen) = serve_tcp(1, Continue::Immediately, Reply::Trickle(pieces));
    let url = format!("http://127.0.0.1:{}/chunks", addr.port());

    let task = exchange(
        post(&url, SendSafeBody::Bytes(b"x".to_vec()), Some(1)),
        Arc::new(HttpConnectionPool::default()),
        ClientConfig::default(),
    );
    let mut status = None;
    let mut headers = None;
    let mut body = Vec::new();
    for item in valtron::execute(task, None).expect("execute") {
        match item {
            Stream::Next(HttpExchange::Head {
                status: s,
                headers: h,
            }) => {
                status = Some(s);
                headers = Some(h);
            }
            Stream::Next(HttpExchange::BodyChunk(b)) => body.extend_from_slice(&b),
            Stream::Next(HttpExchange::Failed(e)) => panic!("exchange failed: {e}"),
            _ => {}
        }
    }
    assert_eq!(status, Some(Status::OK));
    let headers = headers.expect("headers");
    assert!(
        headers.iter().any(
            |(k, v)| k.to_string().eq_ignore_ascii_case("x-one") && v == &vec!["1".to_string()]
        ),
        "the header split across reads was lost: {headers:?}"
    );
    assert_eq!(String::from_utf8_lossy(&body), "hello world");
    seen.recv_timeout(Duration::from_secs(5))
        .expect("server report");
}

/// A streaming request body whose producer pushes late. The renderer sees "no
/// data yet" for a while. The client must back off without spinning, and must
/// not hold a worker, then deliver every chunk once the producer pushes.
#[test]
#[ntest::timeout(60_000)]
#[serial_test::serial]
fn slow_streaming_request_body_is_delivered() {
    let _guard = valtron::initialize_pool(29_005, Some(4));
    let (addr, seen) = serve_tcp(1, Continue::Immediately, Reply::Whole("got it"));
    let url = format!("http://127.0.0.1:{}/stream", addr.port());

    let (pushable, body) = pushable_request_body_with_depth(4);
    let producer = thread::spawn(move || {
        for part in ["first ", "second ", "third"] {
            thread::sleep(Duration::from_millis(60));
            pushable.try_push(Bytes::from(part)).expect("push");
        }
        pushable.close();
    });

    let (status, reply) = collect(exchange(
        post(&url, body, None),
        Arc::new(HttpConnectionPool::default()),
        ClientConfig::default(),
    ));
    producer.join().expect("producer");

    assert_eq!(status, Status::OK);
    assert_eq!(reply, b"got it");
    let s = seen
        .recv_timeout(Duration::from_secs(5))
        .expect("server report");
    assert!(s.head.contains("transfer-encoding: chunked"));
    let raw = String::from_utf8_lossy(&s.body);
    for part in ["first ", "second ", "third"] {
        assert!(raw.contains(part), "chunk {part:?} missing from {raw:?}");
    }
    assert!(raw.ends_with("0\r\n\r\n"));
}

/// An async caller drives the client through valtron's future adapters
/// (`execute` -> `into_next_stream`). Parking still happens inside the executor:
/// the `100 Continue` handshake completes promptly and the body streams back.
#[valtron_test(seed = 29_006, threads = 4, timeout = 60_000)]
async fn async_caller_through_future_adapters() {
    let (addr, seen) = serve_tcp(
        1,
        Continue::Immediately,
        Reply::Trickle(vec![
            b"HTTP/1.1 200 OK\r\nContent-Length: 11\r\n",
            b"Connection: close\r\n\r\nhello",
            b" world",
        ]),
    );
    let url = format!("http://127.0.0.1:{}/async", addr.port());

    let started = Instant::now();
    let task = exchange(
        post(&url, SendSafeBody::Bytes(b"async".to_vec()), Some(5)),
        Arc::new(HttpConnectionPool::default()),
        ClientConfig::default(),
    );
    let mut items = valtron::execute(task, None)
        .expect("execute")
        .into_next_stream();

    let mut status = None;
    let mut body = Vec::new();
    while let Some(item) = items.next().await {
        match item {
            HttpExchange::Head { status: s, .. } => status = Some(s),
            HttpExchange::BodyChunk(b) => body.extend_from_slice(&b),
            HttpExchange::Failed(e) => panic!("exchange failed: {e}"),
        }
    }

    assert_eq!(status, Some(Status::OK));
    assert_eq!(String::from_utf8_lossy(&body), "hello world");
    let s = seen
        .recv_timeout(Duration::from_secs(5))
        .expect("server report");
    assert!(!s.body_before_continue);
    assert!(
        started.elapsed() < Duration::from_secs(1),
        "async-driven POST took {:?}",
        started.elapsed()
    );
}

// ─────────────────────────────────────────────────────────────────────────────
// TLS
// ─────────────────────────────────────────────────────────────────────────────

#[cfg(feature = "ssl-rustls")]
mod tls {
    use super::*;
    use foundation_netio::netcap::connection::Connection;
    use foundation_netio::netcap::ssl::rustls::{RustlsAcceptor, RustlsConnector};
    use rustls::pki_types::pem::PemObject;
    use zeroize::Zeroizing;

    fn fixture(name: &str) -> Vec<u8> {
        let path = format!(
            "{}/tests/tcp_and_tls/fixtures/{name}",
            env!("CARGO_MANIFEST_DIR")
        );
        std::fs::read(path).expect("read TLS fixture")
    }

    /// Full exchange over rustls on a non-blocking socket. The handshake runs
    /// lazily on the first non-blocking write. The TLS session decides whether a
    /// stall waits for readable or writable. `100 Continue`, a trickled header
    /// block and a trickled chunked body all cross TLS records.
    #[test]
    #[ntest::timeout(60_000)]
    #[serial_test::serial]
    fn rustls_exchange_with_continue_and_trickled_body() {
        let _guard = valtron::initialize_pool(29_007, Some(4));
        let cert = fixture("cert.pem");
        let acceptor = RustlsAcceptor::from_pem(cert.clone(), Zeroizing::new(fixture("key.pem")))
            .expect("acceptor");

        let listener = TcpListener::bind("127.0.0.1:0").expect("bind");
        let addr = listener.local_addr().expect("addr");
        let (tx, seen) = mpsc::channel();
        thread::spawn(move || {
            let (socket, _) = listener.accept().expect("accept");
            let tls = acceptor
                .accept(Connection::Tcp(socket))
                .expect("tls accept");
            let reply = Reply::Trickle(vec![
                b"HTTP/1.1 200 OK\r\nTransfer-Encoding: chun",
                b"ked\r\nConnection: close\r\n\r\n3\r\nab",
                b"c\r\n0\r\n\r\n",
            ]);
            tx.send(handle(tls, None, &Continue::Immediately, &reply))
                .expect("report");
        });

        let mut roots = rustls::RootCertStore::empty();
        roots
            .add(rustls::pki_types::CertificateDer::from_pem_slice(&cert).expect("cert der"))
            .expect("trust fixture");
        let client_tls = rustls::ClientConfig::builder()
            .with_root_certificates(roots)
            .with_no_client_auth();
        let pool = Arc::new(
            HttpConnectionPool::<SystemDnsResolver>::default()
                .with_tls_connector(RustlsConnector::with_config(Arc::new(client_tls))),
        );

        let url = format!("https://localhost:{}/tls", addr.port());
        let started = Instant::now();
        let (status, body) = collect(exchange(
            post(&url, SendSafeBody::Bytes(b"secret".to_vec()), Some(6)),
            pool,
            ClientConfig::default(),
        ));

        assert_eq!(status, Status::OK);
        assert_eq!(body, b"abc");
        let s = seen
            .recv_timeout(Duration::from_secs(5))
            .expect("server report");
        assert_eq!(s.body, b"secret");
        assert!(s.continue_sent.is_some_and(|c| s.body_at >= c));
        assert!(
            started.elapsed() < Duration::from_secs(2),
            "TLS POST took {:?}",
            started.elapsed()
        );
    }
}
