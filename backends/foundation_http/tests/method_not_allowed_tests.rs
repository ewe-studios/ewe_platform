//! A known path with a wrong method answers 405, not 404 (issue #743).
//!
//! WHY: RFC 9110 section 15.5.6 says the server answers 405 when the target
//! resource exists but has no handler for the method. A client uses the `Allow`
//! header to retry with a supported method.
//! WHAT: a GET-only route, a POST to it, and a POST to a missing path.
//! HOW: a real HTTP/1.1 request over a loopback socket, and the raw reply.

#![cfg(feature = "multi")]

use std::io::{Read, Write};
use std::net::TcpStream;
use std::sync::Arc;
use std::time::Duration;

use foundation_core::io::ioutils::SharedByteBufferStream;
use foundation_core::valtron::initialize_pool;
use foundation_http::native::writer::send_text;
use foundation_http::{
    native::serve::{Serve, ServeFactory},
    native::server::{HttpServer, KeepAliveConfig, ServerConfig},
    shared::{app::HttpApp, context::Ctx, serve::ConnectionResult},
    ShutdownSignal,
};
use foundation_netio::netcap::RawStream;
use foundation_netio::shared::http::{SimpleIncomingRequest, SimpleMethod};
use serial_test::serial;

struct HelloHandler;

impl ServeFactory for HelloHandler {
    fn create(_ctx: &Ctx) -> Self {
        Self
    }
}

impl Serve for HelloHandler {
    fn serve(
        &self,
        _ctx: Ctx,
        _req: SimpleIncomingRequest,
        conn: SharedByteBufferStream<RawStream>,
    ) -> ConnectionResult {
        send_text(conn, 200, "hello", &foundation_core::valtron::Ctx::host())
    }
}

fn start(app: HttpApp<Arc<dyn Serve>>) -> std::net::SocketAddr {
    let shutdown = Arc::new(ShutdownSignal::new());
    let listener = std::net::TcpListener::bind("127.0.0.1:0").expect("bind");
    let addr = listener.local_addr().expect("addr");
    let config = ServerConfig::defaults()
        .with_keep_alive(KeepAliveConfig::defaults().with_idle_timeout(Duration::from_secs(3)));
    let server = HttpServer::with_config(app, &format!("127.0.0.1:{}", addr.port()), config);
    let signal = shutdown.clone();
    std::thread::spawn(move || server.serve_with_listener(&listener, &signal));
    addr
}

fn only_get_app() -> HttpApp<Arc<dyn Serve>> {
    let mut app = HttpApp::new_serve();
    app.router
        .add_route(SimpleMethod::GET, "/only-get", Arc::new(HelloHandler));
    app
}

/// Send `raw`, then read until the server closes or goes quiet.
fn exchange(addr: std::net::SocketAddr, raw: &str) -> String {
    let mut stream = TcpStream::connect(addr).expect("connect");
    stream
        .set_read_timeout(Some(Duration::from_secs(5)))
        .expect("read timeout");
    stream.write_all(raw.as_bytes()).expect("write");
    let mut out = Vec::new();
    let mut buf = [0u8; 4096];
    loop {
        match stream.read(&mut buf) {
            Ok(0) | Err(_) => break,
            Ok(n) => out.extend_from_slice(&buf[..n]),
        }
    }
    String::from_utf8_lossy(&out).into_owned()
}

#[test]
#[foundation_macros::timeout(60000)]
#[serial(http_test)]
fn a_wrong_method_on_a_known_path_answers_405_with_allow() {
    let _guard = initialize_pool(42, Some(5));
    let addr = start(only_get_app());

    let reply = exchange(
        addr,
        "POST /only-get HTTP/1.1\r\nHost: test\r\nContent-Length: 0\r\nConnection: close\r\n\r\n",
    );

    assert!(
        reply.starts_with("HTTP/1.1 405"),
        "expected a 405, got {reply:?}"
    );
    assert!(
        reply.to_ascii_lowercase().contains("\r\nallow: get\r\n"),
        "the 405 must name the allowed methods, got {reply:?}"
    );
    assert!(reply.contains("Method Not Allowed"), "body: {reply:?}");
}

#[test]
#[foundation_macros::timeout(60000)]
#[serial(http_test)]
fn a_missing_path_still_answers_404() {
    let _guard = initialize_pool(42, Some(5));
    let addr = start(only_get_app());

    let reply = exchange(
        addr,
        "POST /missing HTTP/1.1\r\nHost: test\r\nContent-Length: 0\r\nConnection: close\r\n\r\n",
    );

    assert!(
        reply.starts_with("HTTP/1.1 404"),
        "expected a 404, got {reply:?}"
    );
}
