//! End-to-end HTTP/3 proxy test (F19) — H3 client → proxy (QUIC front end) →
//! HTTP/1.1 backend. A real QUIC client drives an H3 request; the proxy
//! terminates H3, forwards over HTTP/1.1 to a real `foundation_http` backend,
//! and streams the response back as H3 frames. No stubs.

#![cfg(feature = "quic")]

use std::io::Write as _;
use std::net::TcpListener;
use std::sync::Arc;
use std::time::{Duration, Instant};

use bytes::Bytes;
use foundation_core::io::ioutils::SharedByteBufferStream;
use foundation_core::synca::OnSignal;
use foundation_core::valtron::{initialize_pool, Stream};
use foundation_http::native::server::{HttpServer, ServerConfig};
use foundation_http::shared::app::HttpApp;
use foundation_http::shared::context::ContextBag;
use foundation_http::shared::serve::{respond, ConnectionResult, Serve, ServeFactory};
use foundation_netio::http3::{H3Connection, H3Request};
use foundation_netio::netcap::RawStream;
use foundation_netio::quic::{client_config_trusting_pem, QuicDriver, QuinnBidiStream};
use foundation_netio::shared::client::body_reader::try_collect_bytes;
use foundation_netio::shared::http::SimpleIncomingRequest;
use foundation_proxy::config::{ServiceConfig, SslConfig};
use foundation_proxy::{ProxyConfig, ProxyServer};

const CERT: &[u8] = include_bytes!("fixtures/quic_cert.pem");
const KEY: &[u8] = include_bytes!("fixtures/quic_key.pem");
const CA: &[u8] = include_bytes!("fixtures/quic_ca.pem");

fn ms(n: u64) -> Duration {
    Duration::from_millis(n)
}

// ── HTTP/1.1 echo backend (real HttpServer) ──────────────────────────────

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
        let body = req.body.and_then(|b| try_collect_bytes(b).ok()).unwrap_or_default();
        let text = String::from_utf8_lossy(&body);
        respond::text(&mut conn, 200, &text)
            .map_or(ConnectionResult::Close(None), |()| ConnectionResult::Keep)
    }
}

struct Backend {
    shutdown: Arc<OnSignal>,
}
impl Drop for Backend {
    fn drop(&mut self) {
        self.shutdown.turn_on();
    }
}

fn start_backend() -> (u16, Backend) {
    let listener = TcpListener::bind("127.0.0.1:0").expect("bind backend");
    let port = listener.local_addr().unwrap().port();
    let mut app = HttpApp::new_serve();
    app.route_any::<BodyEchoHandler>("/*");
    let server = HttpServer::with_config(app, &format!("127.0.0.1:{port}"), ServerConfig::defaults());
    let shutdown = Arc::new(OnSignal::new());
    let s = Arc::clone(&shutdown);
    std::thread::spawn(move || server.serve_with_listener(&listener, &s));
    (port, Backend { shutdown })
}

// ── H3 client helpers ────────────────────────────────────────────────────

fn drive<T>(
    mut poll: impl FnMut() -> Stream<Result<T, foundation_netio::http3::H3Error>, ()>,
    deadline: Instant,
) -> Option<T> {
    loop {
        match poll() {
            Stream::Next(Ok(v)) => return Some(v),
            Stream::Next(Err(_)) => return None,
            _ => {
                if Instant::now() >= deadline {
                    return None;
                }
                std::thread::sleep(ms(1));
            }
        }
    }
}

fn send_frame(request: &mut H3Request<QuinnBidiStream>, mut pending: Bytes, headers: bool, deadline: Instant) {
    loop {
        let step = if headers {
            request.poll_send_headers(&mut pending)
        } else {
            request.poll_send_data(&mut pending)
        };
        match step {
            Stream::Next(Ok(())) | Stream::Next(Err(_)) => return,
            _ => {
                if Instant::now() >= deadline {
                    return;
                }
                std::thread::sleep(ms(1));
            }
        }
    }
}

fn finish(request: &mut H3Request<QuinnBidiStream>, deadline: Instant) {
    loop {
        match request.poll_finish() {
            Stream::Next(Ok(())) | Stream::Next(Err(_)) => return,
            _ => {
                if Instant::now() >= deadline {
                    return;
                }
                std::thread::sleep(ms(1));
            }
        }
    }
}

fn read_body(request: &mut H3Request<QuinnBidiStream>, deadline: Instant) -> Vec<u8> {
    let mut body = Vec::new();
    loop {
        match request.poll_body() {
            Stream::Next(Ok(Some(chunk))) => body.extend_from_slice(&chunk),
            Stream::Next(Ok(None)) | Stream::Next(Err(_)) => return body,
            _ => {
                if Instant::now() >= deadline {
                    return body;
                }
                std::thread::sleep(ms(1));
            }
        }
    }
}

// ── Test ─────────────────────────────────────────────────────────────────

#[test]
fn h3_proxy_forwards_to_http1_backend() {
    let _guard = initialize_pool(91, Some(8));
    let (backend_port, _backend) = start_backend();

    // Write the TLS cert+key to disk for the Static provider.
    let dir = std::env::temp_dir().join(format!("ewe_h3_proxy_{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    let cert_path = dir.join("cert.pem");
    let key_path = dir.join("key.pem");
    std::fs::write(&cert_path, CERT).unwrap();
    std::fs::write(&key_path, KEY).unwrap();

    let svc = ServiceConfig::new("echo", "localhost")
        .backend(&format!("http://127.0.0.1:{backend_port}"));
    let config = ProxyConfig::new("localhost", "127.0.0.1")
        .bind("127.0.0.1:0")
        .ssl(SslConfig::static_cert(
            &cert_path.display().to_string(),
            &key_path.display().to_string(),
        ))
        .h3_bind("127.0.0.1:0")
        .service(svc);

    let proxy = ProxyServer::start(config).expect("start proxy with H3");
    let h3_addr = proxy.h3_local_addr().expect("H3 front end bound");
    std::thread::sleep(ms(300));

    // ── H3 client: connect over QUIC and drive one request through the proxy. ──
    let client_cfg = client_config_trusting_pem(CA).expect("client config");
    let (driver, conn) = QuicDriver::connect(h3_addr, client_cfg, "localhost").expect("connect");
    foundation_core::valtron::send(driver).expect("spawn client driver");

    let mut h3 = H3Connection::new(conn);
    let deadline = Instant::now() + Duration::from_secs(20);
    drive(|| h3.poll_setup(), deadline).expect("client h3 setup");

    let mut request = loop {
        match h3.poll_open_request() {
            Stream::Next(Ok(r)) => break r,
            Stream::Next(Err(e)) => panic!("open request: {e}"),
            _ => {
                assert!(Instant::now() < deadline, "never opened request stream");
                std::thread::sleep(ms(1));
            }
        }
    };

    let req_headers = H3Request::<QuinnBidiStream>::encode_headers(&[
        (b":method".as_ref(), b"POST".as_ref()),
        (b":scheme".as_ref(), b"https".as_ref()),
        (b":authority".as_ref(), b"localhost".as_ref()),
        (b":path".as_ref(), b"/api/echo".as_ref()),
    ]);
    send_frame(&mut request, req_headers, true, deadline);
    send_frame(
        &mut request,
        H3Request::<QuinnBidiStream>::encode_data(Bytes::from_static(b"h3-proxy-payload")),
        false,
        deadline,
    );
    finish(&mut request, deadline);

    let headers = drive(|| request.poll_headers(), deadline).expect("response headers");
    let status = headers
        .iter()
        .find(|(k, _)| k.as_ref() == b":status")
        .map(|(_, v)| String::from_utf8_lossy(v).to_string())
        .expect("status");
    assert_eq!(status, "200", "proxied H3 response status");

    let body = read_body(&mut request, deadline);
    assert_eq!(
        &body, b"h3-proxy-payload",
        "backend echoed the body through the H3 proxy: {:?}",
        String::from_utf8_lossy(&body)
    );

    proxy.shutdown();
    let _ = std::io::stdout().flush();
    let _ = std::fs::remove_dir_all(&dir);
}
