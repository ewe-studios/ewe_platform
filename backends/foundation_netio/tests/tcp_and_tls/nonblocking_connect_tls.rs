//! Non-blocking TCP connect and TLS handshake (feature 04) regression tests.
//!
//! WHY: opening an HTTP connection used to block a valtron worker for the whole
//! TCP connect (`TcpStream::connect_timeout`). A dead or slow host held a worker,
//! and with only two task workers on the pool that starved every other task. The
//! TLS handshake also ran in place. Feature 04 makes the connect and the rustls
//! handshake park on socket readiness, bounded by the connect deadline.
//!
//! WHAT: these tests pin the non-blocking behaviour and the error kinds.
//!
//! HOW: a `Ticker` task on the same pool is the canary — it can only tick while
//! at least one worker is free. Two blackhole connects saturate the two task
//! workers, so a blocking connect starves the ticker and a parking one does not.
//! A mocked DNS resolver keeps the tests off the real network.

use std::io::{Read, Write};
use std::net::SocketAddr;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::Arc;
use std::time::{Duration, Instant};

use foundation_core::url::Uri;
use foundation_core::valtron::{self, valtron_test, Ctx, Stream, TaskIterator, TaskStatus};
use foundation_netio::http::{ConnectionPool, HttpConnectionPool, HttpExchangeTask};
use foundation_netio::shared::client::request_task::{HttpExchange, HttpExchangePending};
use foundation_netio::shared::client::{ClientConfig, MockDnsResolver, PreparedRequest};
use foundation_netio::shared::http::{
    Extensions, HttpClientError, SendSafeBody, SimpleHeader, SimpleHeaders, SimpleMethod, Status,
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
/// It needs a task worker to run. If clients held every worker in a blocking
/// connect, it could not tick until their connects timed out.
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

fn expect_timeout(err: &Arc<dyn std::error::Error + Send + Sync + 'static>) {
    let kind = http_error(err.as_ref());
    assert!(
        matches!(kind, HttpClientError::ConnectionTimeout(_)),
        "expected ConnectionTimeout, got {kind:?}"
    );
}

fn expect_refused(err: &Arc<dyn std::error::Error + Send + Sync + 'static>) {
    let kind = http_error(err.as_ref());
    match kind {
        HttpClientError::IoError(io) => assert_eq!(
            io.kind(),
            std::io::ErrorKind::ConnectionRefused,
            "expected ConnectionRefused, got {io:?}"
        ),
        other => panic!("expected IoError(ConnectionRefused), got {other:?}"),
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// Non-blocking TCP connect
// ─────────────────────────────────────────────────────────────────────────────

/// Connect to a non-routable address with a 2 s deadline, while a ticker on the
/// same pool needs a worker every 10 ms.
///
/// Two connects saturate the two task workers: a blocking connect on each leaves
/// no worker for the ticker. With readiness parking both connects free their
/// workers, so the ticker keeps ticking and each connect ends at the deadline.
#[valtron_test(seed = 30_001, threads = 4, timeout = 60_000)]
fn connect_to_blackhole_does_not_hold_a_worker() {
    // 10.255.255.1 is non-routable on most networks: the SYN is sent and dropped,
    // so the connect hangs until the client's own deadline. If a network rejects
    // it at once with `ENETUNREACH`, use a local listener with a full backlog
    // instead (see connection_tests::test_connection_timeout on Linux).
    let blackhole: SocketAddr = "10.255.255.1:9".parse().expect("blackhole addr");
    let resolver = MockDnsResolver::new().with_response("blackhole.test", vec![blackhole]);
    let pool = Arc::new(HttpConnectionPool::new(ConnectionPool::default(), resolver));
    let config = ClientConfig::default().with_connect_timeout(Duration::from_secs(2));

    let started = Instant::now();

    // Submit the two connects first so they take both workers, then the ticker.
    let conn_a = valtron::execute(
        HttpExchangeTask::new(
            get("http://blackhole.test:9/"),
            0,
            pool.clone(),
            config.clone(),
        ),
        None,
    )
    .expect("execute connect a");
    let conn_b = valtron::execute(
        HttpExchangeTask::new(
            get("http://blackhole.test:9/"),
            0,
            pool.clone(),
            config.clone(),
        ),
        None,
    )
    .expect("execute connect b");

    let ticks = Arc::new(AtomicUsize::new(0));
    let ticker = valtron::execute(Ticker { ticks: ticks.clone() }, None).expect("execute ticker");

    expect_timeout(&drain_error(conn_a));
    expect_timeout(&drain_error(conn_b));

    drop(ticker);
    let elapsed = started.elapsed();

    let tick_count = ticks.load(Ordering::SeqCst);
    assert!(
        tick_count >= 150,
        "the ticker ran only {tick_count} times in {elapsed:?}: a connect held a worker"
    );
    assert!(
        (Duration::from_millis(1500)..Duration::from_millis(3000)).contains(&elapsed),
        "connect ended after {elapsed:?}; expected the 2 s deadline"
    );
}

/// A closed local port refuses at once; the error keeps `ConnectionRefused`.
#[valtron_test(seed = 30_002, threads = 4, timeout = 60_000)]
fn connect_refused_reports_refused() {
    // Bind, learn the port, drop: the port is closed (nothing listens), so a
    // connect gets an immediate `ECONNREFUSED`.
    let port = {
        let listener = std::net::TcpListener::bind("127.0.0.1:0").expect("bind");
        listener.local_addr().expect("addr").port()
    };

    let resolver = MockDnsResolver::new().with_response(
        "refused.test",
        vec![SocketAddr::new("127.0.0.1".parse().expect("loopback"), port)],
    );
    let pool = Arc::new(HttpConnectionPool::new(ConnectionPool::default(), resolver));
    let config = ClientConfig::default();

    let stream = valtron::execute(
        HttpExchangeTask::new(
            get(&format!("http://refused.test:{port}/")),
            0,
            pool,
            config,
        ),
        None,
    )
    .expect("execute");

    expect_refused(&drain_error(stream));
}

// ─────────────────────────────────────────────────────────────────────────────
// Non-blocking TLS handshake
// ─────────────────────────────────────────────────────────────────────────────

#[cfg(feature = "ssl-rustls")]
mod tls {
    use super::*;
    use foundation_netio::netcap::connection::Connection;
    use foundation_netio::netcap::ssl::SSLAcceptor;
    use foundation_testing::http::{test_tls_connector, TestHttpsServer};
    use zeroize::Zeroizing;

    fn fixture(name: &str) -> Vec<u8> {
        let path = format!(
            "{}/tests/tcp_and_tls/fixtures/{name}",
            env!("CARGO_MANIFEST_DIR")
        );
        std::fs::read(path).expect("read TLS fixture")
    }

    /// Serve one HTTP/1.1 GET over an already-negotiated TLS stream.
    fn serve_http(mut stream: impl Read + Write) {
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

    /// A slow server stalls 500 ms before answering the ClientHello. The client
    /// must park (not hold a worker) and still finish the handshake.
    #[valtron_test(seed = 30_003, threads = 4, timeout = 60_000)]
    fn tls_handshake_waits_on_slow_server() {
        let acceptor = SSLAcceptor::from_pem(fixture("cert.pem"), Zeroizing::new(fixture("key.pem")))
            .expect("acceptor");

        let listener = std::net::TcpListener::bind("127.0.0.1:0").expect("bind");
        let addr = listener.local_addr().expect("addr");
        std::thread::spawn(move || {
            let (socket, _) = listener.accept().expect("accept");
            // Stall before answering the ClientHello so the client's handshake parks.
            std::thread::sleep(Duration::from_millis(500));
            let tls = acceptor.accept(Connection::Tcp(socket)).expect("tls accept");
            serve_http(tls);
        });

        // The client trusts the same self-signed fixture (foundation_testing's
        // embedded cert is byte-identical to tests/tcp_and_tls/fixtures/cert.pem).
        let resolver = MockDnsResolver::new().with_response(
            "localhost",
            vec![SocketAddr::new("127.0.0.1".parse().expect("loopback"), addr.port())],
        );
        let pool = Arc::new(
            HttpConnectionPool::new(ConnectionPool::default(), resolver)
                .with_tls_connector(test_tls_connector()),
        );

        let started = Instant::now();
        let ticks = Arc::new(AtomicUsize::new(0));
        let ticker = valtron::execute(Ticker { ticks: ticks.clone() }, None).expect("ticker");

        let (status, body) = drain_ok(valtron::execute(
            HttpExchangeTask::new(
                get(&format!("https://localhost:{}/slow", addr.port())),
                0,
                pool,
                ClientConfig::default(),
            ),
            None,
        )
        .expect("execute"));

        drop(ticker);
        let elapsed = started.elapsed();

        assert_eq!(status, Status::OK);
        assert_eq!(body, b"ok");
        assert!(
            elapsed >= Duration::from_millis(500),
            "handshake finished in {elapsed:?}; the server did not get to stall"
        );
        let tick_count = ticks.load(Ordering::SeqCst);
        assert!(
            tick_count >= 20,
            "the ticker ran only {tick_count} times during a 500 ms handshake"
        );
    }

    /// A self-signed server the client does not trust: the handshake fails with
    /// a TLS alert, surfaced as `TlsHandshakeFailed`.
    #[valtron_test(seed = 30_004, threads = 4, timeout = 60_000)]
    fn tls_handshake_bad_cert_reports_alert() {
        // `TestHttpsServer` presents a self-signed cert. The default connector
        // trusts only the webpki roots, so the client rejects it mid-handshake.
        let server = TestHttpsServer::start();
        let port = server.port();

        let resolver = MockDnsResolver::new().with_response(
            "localhost",
            vec![SocketAddr::new("127.0.0.1".parse().expect("loopback"), port)],
        );
        let pool = Arc::new(HttpConnectionPool::new(ConnectionPool::default(), resolver));

        let err = drain_error(
            valtron::execute(
                HttpExchangeTask::new(
                    get(&format!("https://localhost:{port}/")),
                    0,
                    pool,
                    ClientConfig::default(),
                ),
                None,
            )
            .expect("execute"),
        );

        let kind = http_error(err.as_ref());
        assert!(
            matches!(kind, HttpClientError::TlsHandshakeFailed(_)),
            "expected TlsHandshakeFailed, got {kind:?}"
        );
    }
}
