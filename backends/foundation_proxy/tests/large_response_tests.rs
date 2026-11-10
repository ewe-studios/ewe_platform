//! Spec 70 feature 63 — a proxy relays a large response byte for byte.
//!
//! WHY: a test backend that answers from a non-blocking socket must write
//! through the async response writer. The synchronous `respond_*` helpers write
//! with `write_all`; after a partial write they fail and the response is
//! truncated. The proxy then relays a short body under a full `Content-Length`.
//!
//! WHAT: a real `foundation_http` server returns 4 MiB from `send_bytes`; the
//! proxy forwards it to a slow client; every byte must arrive, in order.
//!
//! NOTE: the backend handler first used the synchronous helper, and this test
//! failed on it (the client received part of the body and the connection
//! closed). The handler now uses `send_bytes`, which cannot truncate.

use std::io::{Read, Write};
use std::net::{TcpListener, TcpStream};
use std::sync::Arc;
use std::time::Duration;

use foundation_core::io::ioutils::SharedByteBufferStream;
use foundation_core::valtron::{valtron_test, BlockingFutureExt};
use foundation_http::native::serve::{Serve, ServeFactory};
use foundation_http::native::server::{HttpServer, ServerConfig};
use foundation_http::native::writer::send_bytes;
use foundation_http::shared::app::HttpApp;
use foundation_http::shared::context::Ctx;
use foundation_http::shared::serve::ConnectionResult;
use foundation_http::ShutdownSignal;
use foundation_netio::netcap::RawStream;
use foundation_netio::shared::http::SimpleIncomingRequest;
use foundation_proxy::config::ServiceConfig;
use foundation_proxy::{ProxyConfig, ProxyServer};

const BODY_LEN: usize = 4 * 1024 * 1024;

/// A body no shifted or repeated copy can match: every 8-byte word is its own
/// index.
fn pattern(len: usize) -> Vec<u8> {
    let mut out = Vec::with_capacity(len);
    let mut i: u64 = 0;
    while out.len() < len {
        out.extend_from_slice(&i.wrapping_mul(0x9E37_79B9_7F4A_7C15).to_le_bytes());
        i += 1;
    }
    out.truncate(len);
    out
}

/// A slice-backed body that `send_bytes` can take without copying it again.
struct Body(Arc<Vec<u8>>);

impl AsRef<[u8]> for Body {
    fn as_ref(&self) -> &[u8] {
        self.0.as_slice()
    }
}

struct BigBodyHandler {
    body: Arc<Vec<u8>>,
}

impl ServeFactory for BigBodyHandler {
    fn create(ctx: &Ctx) -> Self {
        Self {
            body: ctx.get::<Vec<u8>>().expect("big body in the context"),
        }
    }
}

impl Serve for BigBodyHandler {
    fn serve(
        &self,
        _ctx: Ctx,
        _req: SimpleIncomingRequest,
        conn: SharedByteBufferStream<RawStream>,
    ) -> ConnectionResult {
        send_bytes(
            conn,
            200,
            "application/octet-stream",
            Body(Arc::clone(&self.body)),
        )
    }
}

/// Start a real HTTP/1.1 backend that answers every path with `body`.
fn start_backend(body: Vec<u8>) -> (u16, Arc<ShutdownSignal>) {
    let listener = TcpListener::bind("127.0.0.1:0").expect("bind backend");
    let port = listener.local_addr().expect("backend addr").port();

    let mut app = HttpApp::new_serve();
    app.context().store(body);
    app.route_any::<BigBodyHandler>("/*");
    let server =
        HttpServer::with_config(app, &format!("127.0.0.1:{port}"), ServerConfig::defaults());

    let shutdown = Arc::new(ShutdownSignal::new());
    let thread_shutdown = Arc::clone(&shutdown);
    std::thread::spawn(move || server.serve_with_listener(&listener, &thread_shutdown));
    (port, shutdown)
}

/// Send one request, read the head, then read `Content-Length` body bytes in
/// small chunks with a pause, so the proxy's socket fills and it meets
/// `WouldBlock`.
fn slow_get(addr: std::net::SocketAddr) -> Vec<u8> {
    let mut client = TcpStream::connect(addr).expect("connect proxy");
    client
        .set_read_timeout(Some(Duration::from_secs(30)))
        .expect("read timeout");
    client
        .write_all(b"GET /big HTTP/1.1\r\nHost: app.local\r\nConnection: close\r\n\r\n")
        .expect("write request");

    let mut acc = Vec::new();
    let mut buf = vec![0u8; 16 * 1024];
    let split = loop {
        if let Some(pos) = acc.windows(4).position(|w| w == b"\r\n\r\n") {
            break pos;
        }
        let n = client.read(&mut buf).expect("read head");
        assert!(n > 0, "closed before the response head ended");
        acc.extend_from_slice(&buf[..n]);
    };
    let head = String::from_utf8_lossy(&acc[..split]).to_string();
    assert!(head.starts_with("HTTP/1.1 200"), "head: {head:?}");
    let want: usize = head
        .lines()
        .find_map(|line| {
            let (name, value) = line.split_once(':')?;
            name.eq_ignore_ascii_case("content-length")
                .then(|| value.trim().parse().expect("content-length"))
        })
        .expect("response carries Content-Length");

    let mut body = acc[split + 4..].to_vec();
    while body.len() < want {
        let n = client.read(&mut buf).expect("read body");
        assert!(n > 0, "closed after {} of {want} bytes", body.len());
        body.extend_from_slice(&buf[..n]);
        std::thread::sleep(Duration::from_millis(1));
    }
    assert_eq!(body.len(), want, "more bytes than Content-Length");
    body
}

/// A 4 MiB backend response relayed by the proxy arrives byte for byte.
#[valtron_test(seed = 631, threads = 4, timeout = 120_000)]
fn proxy_large_response_is_byte_exact() {
    let data = pattern(BODY_LEN);
    let (backend_port, backend_shutdown) = start_backend(data.clone());

    let service =
        ServiceConfig::new("app", "app.local").backend(&format!("http://127.0.0.1:{backend_port}"));
    let proxy = ProxyServer::start(
        ProxyConfig::new("app.local", "127.0.0.1")
            .bind("127.0.0.1:0")
            .service(service),
        foundation_core::valtron::Ctx::host(),
    )
    .block()
    .expect("start proxy");

    // The proxy and backend listeners are already bound; the client connects
    // through the proxy and reads the relayed body slowly.
    let addr = proxy.local_addr();
    let received = slow_get(addr);

    assert_eq!(
        received.len(),
        data.len(),
        "the proxy must relay the whole 4 MiB body"
    );
    assert!(
        received == data,
        "the relayed body differs from the backend's body"
    );

    proxy.shutdown();
    backend_shutdown.turn_on();
}
