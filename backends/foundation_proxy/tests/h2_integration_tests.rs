//! End-to-end HTTP/2 proxy tests — H2 client → proxy → HTTP/1.1 backend.
//!
//! Starts a real HTTP/1.1 echo server (`foundation_http::HttpServer` — the same
//! stack the proxy forwards to, not a hand-rolled socket), the proxy with H2
//! enabled (`ServerApp::Both`), then uses `foundation_netio`'s `H2Client` to send
//! HTTP/2 requests through the proxy and verifies responses come back.

use std::io;
use std::net::{TcpListener, TcpStream};
use std::sync::{Arc, Mutex};
use std::thread;
use std::time::{Duration, Instant};

use bytes::Bytes;
use foundation_core::io::ioutils::SharedByteBufferStream;
use foundation_core::synca::OnSignal;
use foundation_core::url::Uri;
use foundation_core::valtron::{initialize_pool, valtron_test, Pipe, PipeReceiver, PipeSender, BlockingFutureExt};
use foundation_http::native::server::{HttpServer, ServerConfig};
use foundation_http::native::serve::{BoxFuture, H2Serve};
use foundation_http::shared::app::{HttpApp, ServerApp};
use foundation_http::shared::context::ContextBag;
use foundation_http::shared::serve::{respond, ConnectionResult, Serve, ServeFactory};
use foundation_netio::http2::client::H2Client;
use foundation_netio::http2::connection::H2Request;
use foundation_netio::http2::types::{H2Frame, H2IncomingFrame, SimpleIncomingRequestHeader};
use foundation_netio::netcap::RawStream;
use foundation_netio::shared::client::body_reader::try_collect_bytes;
use foundation_netio::shared::http::{
    Proto, RequestDescriptor, SimpleHeader, SimpleHeaders, SimpleIncomingRequest, SimpleMethod,
    SimpleUrl, Status,
};
use foundation_connectrpc::{H2Transport, Transport};
use futures_util::StreamExt;
use foundation_proxy::config::ServiceConfig;
use foundation_proxy::h2_proxy::h2_status;
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

// ── HTTP/2 gRPC backend (real HttpServer, H2-only) ───────────────────────

/// The gRPC method path both the test client and the H2 backend register.
const GRPC_PATH: &str = "/grpc.Echo/Unary";

/// The request shape the H2 backend observed, for no-downgrade assertions.
#[derive(Default)]
struct Observed {
    scheme: String,
    authority: String,
    path: String,
    method: String,
    content_type: Option<String>,
    te: Option<String>,
}

/// An `H2Serve` backend that records the request head and echoes the body back
/// verbatim. With `send_trailers`, it ends the response with a `grpc-status: 0`
/// trailing HEADERS frame instead of a plain `END_STREAM` DATA frame.
struct H2EchoHandler {
    observed: Arc<Mutex<Observed>>,
    send_trailers: bool,
}

impl H2Serve for H2EchoHandler {
    fn serve_h2(
        &self,
        _bag: Arc<ContextBag>,
        header: SimpleIncomingRequestHeader,
        body: PipeReceiver<H2IncomingFrame>,
        tx: PipeSender<H2Frame>,
    ) -> BoxFuture<'static, io::Result<()>> {
        let observed = Arc::clone(&self.observed);
        let send_trailers = self.send_trailers;
        Box::pin(async move {
            // Record the request head before it is consumed.
            {
                let mut rec = observed.lock().unwrap();
                rec.scheme = header.scheme.clone();
                rec.authority = header.authority.clone();
                rec.path = header.path.clone();
                rec.method = header.method.to_string();
                rec.content_type = header
                    .headers
                    .get(&SimpleHeader::CONTENT_TYPE)
                    .and_then(|v| v.first())
                    .cloned();
                rec.te = header
                    .headers
                    .get(&SimpleHeader::TE)
                    .and_then(|v| v.first())
                    .cloned();
            }

            // Drain the request body so it can be echoed.
            let mut echoed = Vec::new();
            while let Some(frame) = body.receive().await {
                match frame {
                    H2IncomingFrame::Data(bytes) => echoed.extend_from_slice(&bytes),
                    H2IncomingFrame::Reset(_) => break,
                }
            }

            let _ = tx
                .send(H2Frame::Headers {
                    status: 200,
                    headers: vec![(
                        Bytes::from_static(b"content-type"),
                        Bytes::from_static(b"application/octet-stream"),
                    )],
                    end_stream: false,
                })
                .await;

            if !echoed.is_empty() {
                let _ = tx
                    .send(H2Frame::Data {
                        payload: Bytes::from(echoed),
                        end_stream: false,
                    })
                    .await;
            }

            if send_trailers {
                let _ = tx
                    .send(H2Frame::Trailers {
                        headers: vec![(
                            Bytes::from_static(b"grpc-status"),
                            Bytes::from_static(b"0"),
                        )],
                    })
                    .await;
            } else {
                let _ = tx
                    .send(H2Frame::Data {
                        payload: Bytes::new(),
                        end_stream: true,
                    })
                    .await;
            }
            Ok(())
        })
    }
}

/// Start a real `foundation_http` H2-only echo backend on an ephemeral port.
fn start_h2_backend(send_trailers: bool) -> (u16, Arc<Mutex<Observed>>, Arc<OnSignal>) {
    let observed = Arc::new(Mutex::new(Observed::default()));
    let handler: Arc<dyn H2Serve> = Arc::new(H2EchoHandler {
        observed: Arc::clone(&observed),
        send_trailers,
    });

    let mut app = HttpApp::new_h2_serve();
    app.route_any_h2(GRPC_PATH, handler);

    let listener = TcpListener::bind("127.0.0.1:0").expect("bind h2 backend");
    let port = listener.local_addr().expect("h2 backend addr").port();
    let addr = format!("127.0.0.1:{port}");

    let shutdown = Arc::new(OnSignal::new());
    let server_shutdown = Arc::clone(&shutdown);
    let server = HttpServer::from_app(ServerApp::http2(app), &addr);
    thread::spawn(move || {
        server.serve_with_listener(&listener, &server_shutdown);
    });

    (port, observed, shutdown)
}

/// An H2 backend that streams a response in two DATA frames, holding the stream
/// open on a release signal between them — proving the proxy relays frames as
/// they arrive rather than buffering the whole body before emitting the first byte.
struct StreamingHandler {
    release: PipeReceiver<()>,
}

impl H2Serve for StreamingHandler {
    fn serve_h2(
        &self,
        _bag: Arc<ContextBag>,
        _header: SimpleIncomingRequestHeader,
        body: PipeReceiver<H2IncomingFrame>,
        tx: PipeSender<H2Frame>,
    ) -> BoxFuture<'static, io::Result<()>> {
        let mut release = self.release.clone();
        Box::pin(async move {
            // Drain the unary request body before responding.
            while let Some(frame) = body.receive().await {
                match frame {
                    H2IncomingFrame::Data(_) => {}
                    H2IncomingFrame::Reset(_) => break,
                }
            }

            let _ = tx
                .send(H2Frame::Headers {
                    status: 200,
                    headers: vec![(
                        Bytes::from_static(b"content-type"),
                        Bytes::from_static(b"application/octet-stream"),
                    )],
                    end_stream: false,
                })
                .await;
            let _ = tx
                .send(H2Frame::Data {
                    payload: Bytes::from_static(b"first"),
                    end_stream: false,
                })
                .await;

            // Hold the stream open: a buffering proxy would withhold `first`
            // until the stream ends. The test reads `first`, then releases this
            // signal so the backend can finish — so `first` is observable while
            // the stream is still open, without any time-based sleep (which is
            // racy under valtron's shared worker pool).
            let _ = release.receive().await;

            let _ = tx
                .send(H2Frame::Data {
                    payload: Bytes::from_static(b"second"),
                    end_stream: false,
                })
                .await;
            let _ = tx
                .send(H2Frame::Trailers {
                    headers: vec![(
                        Bytes::from_static(b"grpc-status"),
                        Bytes::from_static(b"0"),
                    )],
                })
                .await;
            Ok(())
        })
    }
}

/// Start an H2 backend that streams a two-frame response, holding the second
/// frame until the returned `release` sender fires. Returns the backend port,
/// its shutdown signal, and the release sender.
fn start_streaming_backend() -> (u16, Arc<OnSignal>, PipeSender<()>) {
    let (release_tx, release_rx) = Pipe::<()>::new();
    let handler: Arc<dyn H2Serve> = Arc::new(StreamingHandler { release: release_rx });
    let mut app = HttpApp::new_h2_serve();
    app.route_any_h2(GRPC_PATH, handler);

    let listener = TcpListener::bind("127.0.0.1:0").expect("bind streaming h2 backend");
    let port = listener.local_addr().expect("streaming h2 backend addr").port();
    let addr = format!("127.0.0.1:{port}");

    let shutdown = Arc::new(OnSignal::new());
    let server_shutdown = Arc::clone(&shutdown);
    let server = HttpServer::from_app(ServerApp::http2(app), &addr);
    thread::spawn(move || {
        server.serve_with_listener(&listener, &server_shutdown);
    });

    (port, shutdown, release_tx)
}

/// Build a proxy config whose service routes host `127.0.0.1` to an H2 backend.
/// (The connectrpc `H2Transport` client dials and sets `:authority` to the same
/// host, so the service host must be the loopback address, not a virtual name.)
fn grpc_proxy_config(backend_port: u16) -> ProxyConfig {
    let svc = ServiceConfig::new("grpc", "127.0.0.1")
        .backend(&format!("http://127.0.0.1:{backend_port}"));
    ProxyConfig::new("127.0.0.1", "127.0.0.1")
        .bind("127.0.0.1:0")
        .service(svc)
}

/// Send a gRPC request through the proxy with connectrpc's `H2Transport` — the
/// only H2 client that surfaces trailers — returning `(status, body, trailers)`.
async fn h2_grpc_request(
    proxy_port: u16,
    path: &str,
    payload: &[u8],
) -> (Status, Vec<u8>, SimpleHeaders) {
    let url = format!("http://127.0.0.1:{proxy_port}{path}");
    let uri = Uri::parse(&url).expect("valid uri");

    let mut headers = SimpleHeaders::new();
    headers.insert(
        SimpleHeader::CONTENT_TYPE,
        vec!["application/grpc+json".to_string()],
    );
    headers.insert(
        SimpleHeader::from("te".to_string()),
        vec!["trailers".to_string()],
    );

    let mut stream = H2Transport::new()
        .open(RequestDescriptor {
            proto: Proto::HTTP20,
            request_url: SimpleUrl::url_only(&url),
            request_uri: uri,
            headers,
            method: SimpleMethod::POST,
        })
        .expect("open h2 stream");

    stream
        .send_body
        .try_send(Bytes::from(payload.to_vec()))
        .expect("send request body");
    stream.send_body.close();

    let (status, _resp_headers) = stream
        .head
        .next()
        .await
        .expect("response head")
        .expect("head ok");

    let mut body = Vec::new();
    while let Some(chunk) = stream.recv_body.next().await {
        body.extend_from_slice(&chunk.expect("body chunk"));
    }

    let trailers = stream.trailers.receive().await.unwrap_or_default();

    (status, body, trailers)
}

// ── Tests ──────────────────────────────────────────────────────────────

#[test]
fn h2_echo_roundtrip() {
    let _guard = initialize_pool(42, Some(4));
    let (backend_port, _backend) = start_echo_backend();
    let proxy = ProxyServer::start(proxy_config(backend_port)).block().expect("start proxy");
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
    let proxy = ProxyServer::start(proxy_config(backend_port)).block().expect("start proxy");
    let proxy_port = proxy.local_addr().port();

    let (status, _) = h2_request(proxy_port, "unknown.local", "/", None);
    proxy.shutdown();

    assert_eq!(status, 404, "unknown host should get 404, got {status}");
}

/// WHY: gRPC requires HTTP/2 end-to-end. The proxy must forward `application/grpc`
/// requests to the backend over HTTP/2 without downgrading to HTTP/1.1.
///
/// WHAT: An H2-only backend (which would reject an HTTP/1.1 preface) echoes the
/// request body. A 200 round-trip therefore proves the H2 leg was used, and the
/// recorded pseudo-headers prove the request reached the backend intact.
#[valtron_test]
async fn h2_grpc_forward_no_downgrade() {
    let (backend_port, observed, backend_shutdown) = start_h2_backend(false);
    let proxy = ProxyServer::start(grpc_proxy_config(backend_port)).await.expect("start proxy");
    let proxy_port = proxy.local_addr().port();

    let payload = b"h2-grpc-no-downgrade-payload".to_vec();
    let (status, body, _trailers) = h2_grpc_request(proxy_port, GRPC_PATH, &payload).await;

    proxy.shutdown_async().await;
    backend_shutdown.turn_on();

    assert_eq!(status, Status::OK, "gRPC through the H2 backend must be 200 OK");
    assert_eq!(body, payload, "request body must round-trip through the H2 backend");

    let rec = observed.lock().unwrap();
    assert_eq!(
        rec.authority,
        format!("127.0.0.1:{backend_port}"),
        "upstream :authority must be the backend's own host:port"
    );
    assert_eq!(rec.scheme, "http", "upstream :scheme must be http (h2c)");
    assert_eq!(
        rec.content_type.as_deref(),
        Some("application/grpc+json"),
        "content-type must survive the proxy"
    );
    assert_eq!(
        rec.te.as_deref(),
        Some("trailers"),
        "te: trailers must survive the proxy"
    );
}

/// WHY: gRPC status rides trailing HEADERS (`grpc-status`), never the HTTP status
/// line. The proxy must relay the backend's trailers to the client as an
/// `H2Frame::Trailers`, or a gRPC client sees a stream end with no status.
///
/// WHAT: The H2 backend sends `grpc-status: 0` trailers; the client asserts it
/// observes them through `H2Transport`'s trailer pipe.
#[valtron_test]
async fn h2_grpc_trailers_relayed() {
    let (backend_port, _observed, backend_shutdown) = start_h2_backend(true);
    let proxy = ProxyServer::start(grpc_proxy_config(backend_port)).await.expect("start proxy");
    let proxy_port = proxy.local_addr().port();

    let payload = b"h2-grpc-trailers-payload".to_vec();
    let (status, _body, trailers) = h2_grpc_request(proxy_port, GRPC_PATH, &payload).await;

    proxy.shutdown_async().await;
    backend_shutdown.turn_on();

    assert_eq!(status, Status::OK, "gRPC response must be 200 OK");

    let grpc_status = trailers
        .get(&SimpleHeader::GRPC_STATUS)
        .and_then(|v| v.first())
        .map(|s| s.as_str());
    assert_eq!(
        grpc_status,
        Some("0"),
        "grpc-status trailer must be relayed as 0"
    );
}

/// WHY: a proxy must stream response bodies — a server-streaming RPC's first
/// frame must reach the client before the stream completes. Buffering the whole
/// body would stall every frame until the backend finishes.
///
/// WHAT: an H2 backend sends a head + first DATA frame, then holds the stream
/// open on a release signal before sending a second DATA frame + trailers. The
/// client asserts the first frame arrives while the stream is still open (well
/// before the release), i.e. the proxy is relaying frames as they arrive rather
/// than buffering the stream.
#[valtron_test]
async fn h2_grpc_streams_without_buffering() {
    let (backend_port, backend_shutdown, release_tx) = start_streaming_backend();
    let proxy = ProxyServer::start(grpc_proxy_config(backend_port)).await.expect("start proxy");
    let proxy_port = proxy.local_addr().port();

    let url = format!("http://127.0.0.1:{proxy_port}{GRPC_PATH}");
    let uri = Uri::parse(&url).expect("valid uri");
    let mut headers = SimpleHeaders::new();
    headers.insert(
        SimpleHeader::CONTENT_TYPE,
        vec!["application/grpc+json".to_string()],
    );
    headers.insert(
        SimpleHeader::from("te".to_string()),
        vec!["trailers".to_string()],
    );

    let mut stream = H2Transport::new()
        .open(RequestDescriptor {
            proto: Proto::HTTP20,
            request_url: SimpleUrl::url_only(&url),
            request_uri: uri,
            headers,
            method: SimpleMethod::POST,
        })
        .expect("open h2 stream");

    stream
        .send_body
        .try_send(Bytes::from_static(b"x"))
        .expect("send request body");
    stream.send_body.close();

    let started = Instant::now();
    let (_status, _resp_headers) = stream
        .head
        .next()
        .await
        .expect("response head")
        .expect("head ok");

    // The first frame must arrive while the backend is still mid-stream —
    // before the release signal fires, so the stream is demonstrably open.
    let first = stream
        .recv_body
        .next()
        .await
        .expect("first body frame")
        .expect("first frame ok");
    let elapsed = started.elapsed();
    assert!(!first.is_empty(), "first frame must be non-empty");
    assert!(
        elapsed < Duration::from_millis(1000),
        "first frame must stream before the stream completes (elapsed {elapsed:?})"
    );

    // Now let the backend finish the stream.
    release_tx
        .send(())
        .await
        .expect("release streaming backend");

    let mut rest = Vec::new();
    while let Some(chunk) = stream.recv_body.next().await {
        rest.extend_from_slice(&chunk.expect("body chunk"));
    }

    let trailers = stream.trailers.receive().await.unwrap_or_default();
    let grpc_status = trailers
        .get(&SimpleHeader::GRPC_STATUS)
        .and_then(|v| v.first())
        .map(|s| s.as_str());

    proxy.shutdown_async().await;
    backend_shutdown.turn_on();

    let mut full = Vec::from(first.as_ref());
    full.extend_from_slice(&rest);
    assert_eq!(full, b"firstsecond", "streamed frames must arrive in order");
    assert_eq!(grpc_status, Some("0"), "grpc-status trailer relayed after the stream");
}

// ── h2_status (moved out of src/h2_proxy.rs per the tests/ rule) ──────────

#[test]
fn h2_status_404() {
    match h2_status(404, true) {
        H2Frame::Headers { status, end_stream, .. } => {
            assert_eq!(status, 404);
            assert!(end_stream);
        }
        _ => panic!("expected Headers frame"),
    }
}

#[test]
fn h2_status_503() {
    match h2_status(503, false) {
        H2Frame::Headers { status, end_stream, .. } => {
            assert_eq!(status, 503);
            assert!(!end_stream);
        }
        _ => panic!("expected Headers frame"),
    }
}
