//! A response the server writes without reading the request body (a 404, a
//! middleware short-circuit) must not leave that body on a kept-alive
//! connection: the next "request" would be parsed out of the old body. Here the
//! body is itself a request line for a real route, so the old behaviour
//! answered it — request smuggling.

#![cfg(feature = "multi")]

use std::io::{Read, Write};
use std::net::TcpStream;
use std::sync::Arc;
use std::time::Duration;

use foundation_core::io::ioutils::SharedByteBufferStream;
use foundation_http::ShutdownSignal;
use foundation_core::valtron::initialize_pool;
use foundation_netio::netcap::RawStream;
use foundation_netio::shared::http::{
    SendSafeBody, SimpleIncomingRequest, SimpleMethod, SimpleOutgoingResponse, Status,
};
use serial_test::serial;

use foundation_http::{
    native::server::{HttpServer, KeepAliveConfig, ServerConfig},
    shared::{
        app::HttpApp,
        context::ContextBag,
        middleware::{MiddlewareResult, RequestMiddleware},
        serve::{respond, ConnectionResult, Serve, ServeFactory},
    },
};

struct SecretHandler;

impl ServeFactory for SecretHandler {
    fn create(_bag: &ContextBag) -> Self {
        Self
    }
}

impl Serve for SecretHandler {
    fn serve(
        &self,
        _bag: Arc<ContextBag>,
        _req: SimpleIncomingRequest,
        mut conn: SharedByteBufferStream<RawStream>,
    ) -> ConnectionResult {
        respond::text(&mut conn, 200, "smuggled-secret")
            .map_or(ConnectionResult::Close(None), |()| ConnectionResult::Keep)
    }
}

/// Answers every POST itself, without reading its body.
struct RejectPosts;

impl RequestMiddleware for RejectPosts {
    fn handle(&self, _ctx: &Arc<ContextBag>, req: &mut SimpleIncomingRequest) -> MiddlewareResult {
        if req.method != SimpleMethod::POST {
            return MiddlewareResult::Continue;
        }
        MiddlewareResult::Response(
            SimpleOutgoingResponse::builder()
                .with_status(Status::Forbidden)
                .with_body(SendSafeBody::Text("no posts".into()))
                .build()
                .expect("valid response"),
        )
    }
}

fn start(app: HttpApp<Arc<dyn Serve>>) -> (std::net::SocketAddr, Arc<ShutdownSignal>) {
    let shutdown = Arc::new(ShutdownSignal::new());
    let listener = std::net::TcpListener::bind("127.0.0.1:0").expect("bind");
    let addr = listener.local_addr().expect("addr");
    let config = ServerConfig::defaults()
        .with_keep_alive(KeepAliveConfig::defaults().with_idle_timeout(Duration::from_secs(3)));
    let server = HttpServer::with_config(app, &format!("127.0.0.1:{}", addr.port()), config);
    let signal = shutdown.clone();
    std::thread::spawn(move || server.serve_with_listener(&listener, &signal));
    (addr, shutdown)
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

fn post_with_smuggled_get(path: &str) -> String {
    let inner = "GET /secret HTTP/1.1\r\nHost: test\r\n\r\n";
    format!(
        "POST {path} HTTP/1.1\r\nHost: test\r\nContent-Length: {}\r\n\r\n{inner}",
        inner.len()
    )
}

#[test]
#[foundation_macros::timeout(60000)]
#[serial(http_test)]
fn not_found_with_an_unread_body_does_not_parse_the_body_as_a_request() {
    let _guard = initialize_pool(42, Some(5));
    let mut app = HttpApp::new_serve();
    app.route::<SecretHandler>(SimpleMethod::GET, "/secret");
    let (addr, shutdown) = start(app);

    let reply = exchange(addr, &post_with_smuggled_get("/missing"));
    shutdown.turn_on();

    assert!(reply.contains("404"), "expected a 404, got {reply:?}");
    assert!(
        !reply.contains("smuggled-secret"),
        "the unread body was served as a request: {reply:?}"
    );
}

#[test]
#[foundation_macros::timeout(60000)]
#[serial(http_test)]
fn middleware_response_with_an_unread_body_does_not_parse_the_body_as_a_request() {
    let _guard = initialize_pool(42, Some(5));
    let mut app = HttpApp::new_serve();
    app.middleware(RejectPosts);
    app.route::<SecretHandler>(SimpleMethod::GET, "/secret");
    let (addr, shutdown) = start(app);

    let reply = exchange(addr, &post_with_smuggled_get("/secret"));
    shutdown.turn_on();

    assert!(reply.contains("403"), "expected a 403, got {reply:?}");
    assert!(
        !reply.contains("smuggled-secret"),
        "the unread body was served as a request: {reply:?}"
    );
}

/// Without a body there is nothing left over: the connection stays usable.
#[test]
#[foundation_macros::timeout(60000)]
#[serial(http_test)]
fn not_found_without_a_body_keeps_the_connection_alive() {
    let _guard = initialize_pool(42, Some(5));
    let mut app = HttpApp::new_serve();
    app.route::<SecretHandler>(SimpleMethod::GET, "/secret");
    let (addr, shutdown) = start(app);

    let reply = exchange(
        addr,
        "GET /missing HTTP/1.1\r\nHost: test\r\n\r\nGET /secret HTTP/1.1\r\nHost: test\r\n\r\n",
    );
    shutdown.turn_on();

    assert!(reply.contains("404"), "expected a 404 first, got {reply:?}");
    assert!(
        reply.contains("smuggled-secret"),
        "the second, real request should be answered: {reply:?}"
    );
}
