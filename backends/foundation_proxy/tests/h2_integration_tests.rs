//! End-to-end HTTP/2 proxy tests — H2 client → proxy → HTTP/1.1 backend.
//!
//! Starts a real HTTP/1.1 echo server (`foundation_http::HttpServer` — the same
//! stack the proxy forwards to, not a hand-rolled socket), the proxy with H2
//! enabled (`ServerApp::Both`), then uses `foundation_netio`'s `H2Client` to send
//! HTTP/2 requests through the proxy and verifies responses come back.

use std::net::{TcpListener, TcpStream};
use std::sync::Arc;
use std::thread;
use std::time::Duration;

use bytes::Bytes;
use foundation_core::io::ioutils::SharedByteBufferStream;
use foundation_core::synca::OnSignal;
use foundation_core::valtron::initialize_pool;
use foundation_http::native::server::{HttpServer, ServerConfig};
use foundation_http::shared::app::HttpApp;
use foundation_http::shared::context::ContextBag;
use foundation_http::shared::serve::{respond, ConnectionResult, Serve, ServeFactory};
use foundation_netio::http2::client::H2Client;
use foundation_netio::http2::connection::H2Request;
use foundation_netio::netcap::RawStream;
use foundation_netio::shared::client::body_reader::try_collect_bytes;
use foundation_netio::shared::http::SimpleIncomingRequest;
use foundation_proxy::config::ServiceConfig;
use foundation_proxy::{ProxyConfig, ProxyServer};

// ── HTTP/1.1 echo backend (real HttpServer) ─────────────────────────────

/// A `Serve` handler that echoes the request body back as `text/plain` — reads
/// the body through `foundation_netio`'s body reader, no hand-rolled framing.
struct BodyEchoHandler;

impl ServeFactory for BodyEchoHandler {
    fn create(_bag: &ContextBag) -> Self {
        Self
    }
}

impl Serve for BodyEchoHandler {
    fn serve(
        &self,
        _bag: Arc<ContextBag>,
        req: SimpleIncomingRequest,
        mut conn: SharedByteBufferStream<RawStream>,
    ) -> ConnectionResult {
        let body = req
            .body
            .and_then(|b| try_collect_bytes(b).ok())
            .unwrap_or_default();
        let text = String::from_utf8_lossy(&body);
        respond::text(&mut conn, 200, &text)
            .map_or(ConnectionResult::Close(None), |()| ConnectionResult::Keep)
    }
}

/// Owns the backend server's shutdown signal; stops the accept loop on drop.
struct EchoBackend {
    shutdown: Arc<OnSignal>,
}

impl Drop for EchoBackend {
    fn drop(&mut self) {
        self.shutdown.turn_on();
    }
}

/// Start a real `foundation_http` HTTP/1.1 echo server on an ephemeral port
/// (`:0`, so tests never collide). Requires an initialised valtron pool.
fn start_echo_backend() -> (u16, EchoBackend) {
    let listener = TcpListener::bind("127.0.0.1:0").expect("bind backend");
    let port = listener.local_addr().expect("backend addr").port();
    let bind_addr = format!("127.0.0.1:{port}");

    let mut app = HttpApp::new_serve();
    app.route_any::<BodyEchoHandler>("/*");
    let server = HttpServer::with_config(app, &bind_addr, ServerConfig::defaults());

    let shutdown = Arc::new(OnSignal::new());
    let thread_shutdown = Arc::clone(&shutdown);
    thread::spawn(move || {
        server.serve_with_listener(&listener, &thread_shutdown);
    });
    (port, EchoBackend { shutdown })
}

// ── Test helpers ───────────────────────────────────────────────────────

/// Build a proxy config whose front end binds an ephemeral port (`:0`); the
/// caller reads the real bound port from `ProxyServer::local_addr()`.
fn proxy_config(backend_port: u16) -> ProxyConfig {
    let svc = ServiceConfig::new("echo", "test.local")
        .backend(&format!("http://127.0.0.1:{backend_port}"));
    ProxyConfig::new("test.local", "127.0.0.1")
        .bind("127.0.0.1:0")
        .service(svc)
}

/// Open a TCP socket, perform H2 handshake, send a request, read the response.
fn h2_request(proxy_port: u16, host: &str, path: &str, body: Option<&[u8]>) -> (u32, Vec<u8>) {
    let stream = TcpStream::connect(format!("127.0.0.1:{proxy_port}")).expect("connect proxy");
    stream.set_read_timeout(Some(Duration::from_secs(10))).ok();

    let mut h2 = H2Client::new(stream);
    h2.connect().expect("h2 handshake");

    let host_b = Bytes::copy_from_slice(host.as_bytes());
    let path_b = Bytes::copy_from_slice(path.as_bytes());

    let headers = vec![
        (Bytes::from_static(b":method"), Bytes::from_static(b"POST")),
        (Bytes::from_static(b":path"), path_b.clone()),
        (Bytes::from_static(b":scheme"), Bytes::from_static(b"http")),
        (Bytes::from_static(b":authority"), host_b.clone()),
    ];

    let body_bytes = body.map(|b| Bytes::copy_from_slice(b));
    // The whole request (headers + full body) is sent in one shot, so the stream
    // is complete: END_STREAM must ride the last frame or the server waits forever
    // for a body continuation that never comes.
    let request = H2Request {
        method: Bytes::from_static(b"POST"),
        scheme: Bytes::from_static(b"http"),
        authority: host_b,
        path: path_b,
        headers,
        body: body_bytes.clone(),
        end_stream: true,
    };

    let stream_id = h2.send(request).expect("h2 send");
    h2.flush().ok();

    // Read response — recv_response returns (stream_id, response)
    loop {
        match h2.recv().expect("h2 recv") {
            Some((rid, response)) => {
                if rid == stream_id {
                    let resp_body = response.body.unwrap_or_default().to_vec();
                    let status = extract_status(&response.headers);
                    return (status, resp_body);
                }
            }
            None => {
                panic!("connection closed before response for stream {stream_id}");
            }
        }
    }
}

/// Extract HTTP status code from response pseudo-headers.
fn extract_status(headers: &[(Bytes, Bytes)]) -> u32 {
    headers
        .iter()
        .find(|(k, _)| k == &Bytes::from_static(b":status"))
        .map(|(_, v)| String::from_utf8_lossy(v).parse().unwrap_or(0))
        .unwrap_or(0)
}

// ── Tests ──────────────────────────────────────────────────────────────

#[test]
fn h2_echo_roundtrip() {
    let _guard = initialize_pool(42, Some(4));
    let (backend_port, _backend) = start_echo_backend();
    let proxy = ProxyServer::start(proxy_config(backend_port)).expect("start proxy");
    let proxy_port = proxy.local_addr().port();

    let (status, body) = h2_request(
        proxy_port,
        "test.local",
        "/api/echo",
        Some(b"h2-integration-test-payload"),
    );

    proxy.shutdown();
    assert_eq!(status, 200, "expected 200 OK, got HTTP {status}");
    assert!(
        String::from_utf8_lossy(&body).contains("h2-integration-test-payload"),
        "response should echo payload: {}",
        String::from_utf8_lossy(&body),
    );
}

#[test]
fn h2_404_on_unknown_host() {
    let _guard = initialize_pool(42, Some(4));
    let (backend_port, _backend) = start_echo_backend();
    let proxy = ProxyServer::start(proxy_config(backend_port)).expect("start proxy");
    let proxy_port = proxy.local_addr().port();

    let (status, _) = h2_request(proxy_port, "unknown.local", "/", None);
    proxy.shutdown();

    assert_eq!(status, 404, "unknown host should get 404, got {status}");
}
