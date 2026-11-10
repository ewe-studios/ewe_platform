//! Async DNS resolution (feature 36) regression tests.
//!
//! WHY: `DnsResolver::resolve` is async now. A slow lookup parks its task on
//! readiness instead of holding a valtron worker, and the client bounds the
//! lookup with a configurable `dns_timeout` deadline.
//!
//! WHAT: these tests pin three behaviours: a mock resolver resolves through the
//! async path, a slow resolver does not hold a worker, and a stuck lookup fails
//! at the deadline.
//!
//! HOW: a `Ticker` task on the same pool is the canary — it can only tick while
//! a worker is free. A `SlowDnsResolver` parks until the test releases a result
//! through a [`Pipe`], so the tests are deterministic and never touch the real
//! network.

use std::io::{Read, Write};
use std::net::{SocketAddr, TcpListener};
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::Arc;
use std::time::{Duration, Instant};

use foundation_context::Ctx;
use foundation_core::url::Uri;
use foundation_core::valtron::{
    self, valtron_test, Pipe, PipeReceiver, PipeSender, Stream, TaskIterator, TaskStatus,
};
use foundation_netio::http::{ConnectionPool, HttpConnectionPool, HttpExchangeTask};
use foundation_netio::shared::client::request_task::{HttpExchange, HttpExchangePending};
use foundation_netio::shared::client::{
    async_trait, ClientConfig, DnsResolver, MockDnsResolver, PreparedRequest,
};
use foundation_netio::shared::http::{
    DnsError, Extensions, HttpClientError, SendSafeBody, SimpleHeader, SimpleHeaders, SimpleMethod,
    Status,
};

/// A plain GET with no body.
fn get(url: &str) -> PreparedRequest {
    let uri = Uri::parse(url).expect("uri");
    // HTTP/1.1 requires at least one header; `Host` is mandatory on requests.
    let mut headers = SimpleHeaders::new();
    headers.insert(
        SimpleHeader::HOST,
        vec![uri.host_str().unwrap_or_else(|| "localhost".to_string())],
    );
    PreparedRequest {
        method: SimpleMethod::GET,
        url: uri,
        headers,
        body: SendSafeBody::None,
        extensions: Extensions::new(),
        body_readiness: None,
    }
}

/// A valtron task that ticks every 10 ms and counts how many times it ran.
///
/// It needs a task worker to run. If DNS lookups held every worker in a blocking
/// resolve, it could not tick until they finished.
struct Ticker {
    ticks: Arc<AtomicUsize>,
}

impl TaskIterator for Ticker {
    type Ready = ();
    type Pending = ();

    fn next_status(&mut self, _ctx: Ctx) -> Option<TaskStatus<(), ()>> {
        self.ticks.fetch_add(1, Ordering::SeqCst);
        Some(TaskStatus::Delayed(Duration::from_millis(10)))
    }
}

/// Serve one HTTP/1.1 GET and reply `200 ok`.
fn serve_http_ok(mut stream: std::net::TcpStream) {
    let mut buf = [0u8; 4096];
    let mut seen = Vec::new();
    while !seen.windows(4).any(|w| w == b"\r\n\r\n") {
        let n = stream.read(&mut buf).expect("server read request head");
        assert!(n > 0, "client closed before sending its head");
        seen.extend_from_slice(&buf[..n]);
    }
    stream
        .write_all(b"HTTP/1.1 200 OK\r\nContent-Length: 2\r\nConnection: close\r\n\r\nok")
        .expect("write reply");
    stream.flush().expect("flush reply");
}

/// Drain an exchange that must fail, and return the boxed error.
fn drain_error(
    stream: impl Iterator<Item = Stream<HttpExchange, HttpExchangePending>>,
) -> Arc<dyn std::error::Error + Send + Sync + 'static> {
    for item in stream {
        match item {
            Stream::Next(HttpExchange::Failed(err)) => return err,
            Stream::Next(HttpExchange::Head { .. }) => panic!("expected a failure, got a head"),
            Stream::Next(HttpExchange::BodyChunk(_)) => panic!("expected a failure, got a body"),
            _ => {}
        }
    }
    panic!("exchange ended without a failure");
}

/// Drain an exchange that must succeed, and return (status, body).
fn drain_ok(
    stream: impl Iterator<Item = Stream<HttpExchange, HttpExchangePending>>,
) -> (Status, Vec<u8>) {
    let mut status = None;
    let mut body = Vec::new();
    for item in stream {
        match item {
            Stream::Next(HttpExchange::Head { status: s, .. }) => status = Some(s),
            Stream::Next(HttpExchange::BodyChunk(b)) => body.extend_from_slice(&b),
            Stream::Next(HttpExchange::Failed(err)) => panic!("exchange failed: {err}"),
            _ => {}
        }
    }
    (status.expect("a response head"), body)
}

fn http_error<'a>(err: &'a (dyn std::error::Error + Send + Sync + 'static)) -> &'a HttpClientError {
    err.downcast_ref::<HttpClientError>()
        .expect("the boxed error is an HttpClientError")
}

/// A resolver that parks until the test pushes a result through its pipe.
///
/// WHY: it models a slow resolver deterministically — no wall-clock sleep, no
/// real DNS. The pipe parks the resolving task and wakes it when the test sends.
struct SlowDnsResolver {
    rx: PipeReceiver<Result<Vec<SocketAddr>, DnsError>>,
}

#[async_trait]
impl DnsResolver for SlowDnsResolver {
    async fn resolve(&self, _host: &str, _port: u16) -> Result<Vec<SocketAddr>, DnsError> {
        match self.rx.receive().await {
            Some(result) => result,
            None => Err(DnsError::ResolutionFailed(
                "slow resolver closed before replying".to_string(),
            )),
        }
    }
}

fn slow_resolver() -> (
    SlowDnsResolver,
    PipeSender<Result<Vec<SocketAddr>, DnsError>>,
) {
    let (tx, rx) = Pipe::new();
    (SlowDnsResolver { rx }, tx)
}

// ─────────────────────────────────────────────────────────────────────────────
// Async resolution
// ─────────────────────────────────────────────────────────────────────────────

/// A mock resolver resolves through the async `Resolving` state and the request
/// completes against a local server.
#[valtron_test(seed = 31_001, threads = 4, timeout = 60_000)]
fn dns_mock_resolver_resolves_async() {
    let listener = TcpListener::bind("127.0.0.1:0").expect("bind loopback");
    let addr = listener.local_addr().expect("local addr");
    let server = std::thread::spawn(move || {
        let (socket, _) = listener.accept().expect("accept client");
        serve_http_ok(socket);
    });

    let resolver = MockDnsResolver::new().with_response("resolveme.test", vec![addr]);
    let pool = Arc::new(HttpConnectionPool::new(ConnectionPool::default(), resolver));

    let (status, body) = drain_ok(
        valtron::execute(
            HttpExchangeTask::new(
                get(&format!("http://resolveme.test:{}/", addr.port())),
                0,
                pool,
                ClientConfig::default(),
            ),
            None,
        )
        .expect("execute"),
    );

    server.join().expect("server thread");

    assert_eq!(status, Status::OK);
    assert_eq!(body, b"ok");
}

// ─────────────────────────────────────────────────────────────────────────────
// Slow lookup parks instead of holding a worker
// ─────────────────────────────────────────────────────────────────────────────

/// Four stuck lookups saturate the four task workers. A blocking lookup on each
/// leaves no worker for the ticker; parked lookups free their workers, so the
/// ticker keeps ticking while the lookups are pending.
#[valtron_test(seed = 31_002, threads = 4, timeout = 60_000)]
fn dns_slow_resolver_does_not_block_the_worker() {
    // A closed local port: once a lookup is released, the connect fails fast.
    let port = {
        let listener = TcpListener::bind("127.0.0.1:0").expect("bind loopback");
        listener.local_addr().expect("local addr").port()
    };
    let addr = SocketAddr::new("127.0.0.1".parse().expect("loopback"), port);

    let mut releases = Vec::new();
    let mut exchanges = Vec::new();
    for _ in 0..4 {
        let (resolver, tx) = slow_resolver();
        releases.push(tx);
        let pool = Arc::new(HttpConnectionPool::new(ConnectionPool::default(), resolver));
        // Generous DNS deadline: the test, not the deadline, releases the lookups.
        let config = ClientConfig::default().with_dns_timeout(Duration::from_secs(5));
        let exchange = valtron::execute(
            HttpExchangeTask::new(get("http://slow.test/"), 0, pool, config),
            None,
        )
        .expect("execute slow exchange");
        exchanges.push(exchange);
    }

    let ticks = Arc::new(AtomicUsize::new(0));
    let ticker = valtron::execute(
        Ticker {
            ticks: ticks.clone(),
        },
        None,
    )
    .expect("execute ticker");

    // While the lookups are pending, the ticker must keep ticking.
    let deadline = Instant::now() + Duration::from_secs(3);
    while ticks.load(Ordering::SeqCst) < 10 && Instant::now() < deadline {
        std::thread::sleep(Duration::from_millis(10));
    }
    let ticks_while_pending = ticks.load(Ordering::SeqCst);
    assert!(
        ticks_while_pending >= 10,
        "the ticker ran only {ticks_while_pending} times while DNS was pending: a worker was held"
    );

    // Release the lookups; each exchange then fails on the closed port.
    for tx in releases {
        tx.try_send(Ok(vec![addr])).expect("release DNS result");
    }

    for exchange in exchanges {
        let err = drain_error(exchange);
        let kind = http_error(err.as_ref());
        assert!(
            matches!(kind, HttpClientError::IoError(_)),
            "expected a connect IoError after the released lookup, got {kind:?}"
        );
    }

    drop(ticker);
}

// ─────────────────────────────────────────────────────────────────────────────
// DNS deadline
// ─────────────────────────────────────────────────────────────────────────────

/// A stuck lookup fails with `ConnectionTimeout` at the configured `dns_timeout`.
#[valtron_test(seed = 31_003, threads = 4, timeout = 60_000)]
fn dns_resolution_timeout_is_enforced() {
    let (resolver, _keep_open) = slow_resolver();
    let pool = Arc::new(HttpConnectionPool::new(ConnectionPool::default(), resolver));
    let config = ClientConfig::default().with_dns_timeout(Duration::from_millis(100));

    let started = Instant::now();
    let err = drain_error(
        valtron::execute(
            HttpExchangeTask::new(get("http://stuck.test/"), 0, pool, config),
            None,
        )
        .expect("execute"),
    );
    let elapsed = started.elapsed();

    let kind = http_error(err.as_ref());
    assert!(
        matches!(kind, HttpClientError::ConnectionTimeout(_)),
        "expected ConnectionTimeout for a stuck DNS lookup, got {kind:?}"
    );
    assert!(
        elapsed >= Duration::from_millis(90),
        "DNS deadline fired after only {elapsed:?}"
    );
    assert!(
        elapsed < Duration::from_secs(5),
        "DNS deadline took {elapsed:?}"
    );
}
