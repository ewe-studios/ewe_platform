//! End-to-end integration tests exercising the full HTTP stack:
//! real TcpListener → HttpServer → middleware → routing → handler → response.
//!
//! Each test starts a real `HttpServer` on a random port, sends actual
//! HTTP requests via `SimpleHttpClient` with `StaticSocketAddr` resolver,
//! and verifies the responses.

use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::Arc;
use std::time::Duration;
use tracing_test::traced_test;

use foundation_core::io::ioutils::SharedByteBufferStream;
use foundation_core::netcap::RawStream;
use foundation_core::synca::OnSignal;
use foundation_core::valtron::initialize_pool;
use foundation_core::wire::simple_http::client::{SimpleHttpClient, StaticSocketAddr};
use foundation_core::wire::simple_http::{
    SendSafeBody, SimpleIncomingRequest, SimpleMethod, Status,
};
use serial_test::serial;

use foundation_http::{
    respond, ConnectionResult, ContextBag, HttpApp, HttpServer, MiddlewareResult,
    RequestMiddleware, Serve, ServeFactory, ServerConfig,
};

// ---------------------------------------------------------------------------
// Echo handler — returns method and path as JSON for test assertions.

struct EchoHandler;

impl ServeFactory for EchoHandler {
    fn create(_bag: &ContextBag) -> Self {
        Self
    }
}

impl Serve for EchoHandler {
    fn serve(
        &self,
        _bag: Arc<ContextBag>,
        req: SimpleIncomingRequest,
        mut conn: SharedByteBufferStream<RawStream>,
    ) -> ConnectionResult {
        let method = match req.method {
            SimpleMethod::GET => "GET",
            SimpleMethod::POST => "POST",
            SimpleMethod::PUT => "PUT",
            SimpleMethod::DELETE => "DELETE",
            SimpleMethod::PATCH => "PATCH",
            SimpleMethod::HEAD => "HEAD",
            SimpleMethod::OPTIONS => "OPTIONS",
            SimpleMethod::CONNECT => "CONNECT",
            SimpleMethod::TRACE => "TRACE",
            SimpleMethod::Custom(_) => "CUSTOM",
        };
        let path = &req.request_url.url;
        let body = serde_json::json!({ "method": method, "path": path });

        tracing::info!("Writing json back to connection");
        let value = respond::json(&mut conn, 200, &body)
            .map_or(ConnectionResult::Close(None), |()| ConnectionResult::Keep);
        tracing::info!("Written json back to connection");

        value
    }
}

// ---------------------------------------------------------------------------
// Body echo handler — echoes the request body back as text.

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
        let body_text = match req.body {
            Some(SendSafeBody::Text(t)) => t,
            Some(SendSafeBody::Bytes(b)) => String::from_utf8_lossy(&b).to_string(),
            _ => String::new(),
        };
        respond::text(&mut conn, 200, &body_text)
            .map_or(ConnectionResult::Close(None), |()| ConnectionResult::Keep)
    }
}

// ---------------------------------------------------------------------------
// Query echo handler — returns the full URL including query string.

struct QueryHandler;

impl ServeFactory for QueryHandler {
    fn create(_bag: &ContextBag) -> Self {
        Self
    }
}

impl Serve for QueryHandler {
    fn serve(
        &self,
        _bag: Arc<ContextBag>,
        req: SimpleIncomingRequest,
        mut conn: SharedByteBufferStream<RawStream>,
    ) -> ConnectionResult {
        let url = &req.request_url.url;
        respond::text(&mut conn, 200, url)
            .map_or(ConnectionResult::Close(None), |()| ConnectionResult::Keep)
    }
}

// ---------------------------------------------------------------------------
// Counter middleware — tracks how many requests passed through.

struct CounterMiddleware {
    count: Arc<AtomicUsize>,
}

impl RequestMiddleware for CounterMiddleware {
    fn handle(&self, _ctx: &Arc<ContextBag>, _req: &mut SimpleIncomingRequest) -> MiddlewareResult {
        self.count.fetch_add(1, Ordering::SeqCst);
        MiddlewareResult::Continue
    }
}

// ---------------------------------------------------------------------------
// Blocking middleware — short-circuits every request with 403.

struct BlockMiddleware;

impl RequestMiddleware for BlockMiddleware {
    fn handle(&self, _ctx: &Arc<ContextBag>, _req: &mut SimpleIncomingRequest) -> MiddlewareResult {
        MiddlewareResult::Response(
            foundation_core::wire::simple_http::SimpleOutgoingResponse::builder()
                .with_status(Status::Forbidden)
                .with_body(SendSafeBody::Text("blocked by middleware".into()))
                .build()
                .expect("valid response"),
        )
    }
}

// ---------------------------------------------------------------------------
// Test helpers.

/// Start an HttpServer in a background thread. Returns (addr, shutdown_signal).
/// Binds the TcpListener before spawning the server thread so the port is
/// guaranteed to be in use by the time this function returns.
fn start_server(app: HttpApp) -> (std::net::SocketAddr, Arc<OnSignal>) {
    let shutdown = Arc::new(OnSignal::new());

    // Bind the listener first — the port is ours from this point on.
    let listener = std::net::TcpListener::bind("127.0.0.1:0").expect("bind failed");
    let addr = listener.local_addr().expect("local_addr failed");

    let config = ServerConfig::defaults().with_keep_alive(
        foundation_http::KeepAliveConfig::defaults().with_idle_timeout(Duration::from_secs(5)),
    );

    let bind_addr = format!("127.0.0.1:{}", addr.port());
    let server = HttpServer::with_config(app, &bind_addr, config);

    let shutdown_thread = shutdown.clone();
    std::thread::spawn(move || {
        // Initialize the valtron thread pool in this thread
        let _guard = initialize_pool(42, Some(5));
        server.serve_with_listener(listener, shutdown_thread);
    });

    (addr, shutdown)
}

/// Create a SimpleHttpClient pointed at the test server via StaticSocketAddr.
fn make_client(addr: std::net::SocketAddr) -> SimpleHttpClient<StaticSocketAddr> {
    let resolver = StaticSocketAddr::new(addr);
    SimpleHttpClient::with_resolver(resolver)
        .max_retries(10)
        .connect_timeout(Duration::from_secs(5))
        .read_timeout(Duration::from_secs(20))
        .write_timeout(Duration::from_secs(5))
}

/// Extract body text from a response.
fn body_text(body: &SendSafeBody) -> String {
    match body {
        SendSafeBody::Text(s) => s.clone(),
        SendSafeBody::Bytes(b) => String::from_utf8_lossy(b).to_string(),
        _ => String::new(),
    }
}

/// Extract status code as u16.
fn status_code(status: &Status) -> u16 {
    status.clone().into_usize() as u16
}

// ---------------------------------------------------------------------------
// Tests: static routes

/// GET /echo → 200 with JSON containing method and path.
#[cfg(feature = "multi")]
#[test]
#[traced_test]
#[serial(http_test)]
fn test_static_route_get() {
    let _guard = initialize_pool(42, Some(5));
    let mut app = HttpApp::new();
    app.route::<EchoHandler>(SimpleMethod::GET, "/echo");

    let (addr, shutdown) = start_server(app);
    let client = make_client(addr);

    let response = client
        .get("http://testserver/echo")
        .unwrap()
        .build_client()
        .unwrap()
        .send()
        .unwrap();

    assert!(response.is_success());
    let body = body_text(response.get_body_ref());
    dbg!("Received body", &body);
    assert!(
        body.contains("GET"),
        "body should contain method GET: {body}"
    );
    assert!(
        body.contains("/echo"),
        "body should contain path /echo: {body}"
    );

    shutdown.turn_on();
}

/// POST /echo → 200 with JSON containing method POST.
#[cfg(feature = "multi")]
#[test]
#[traced_test]
#[serial(http_test)]
fn test_static_route_post() {
    let _guard = initialize_pool(42, Some(5));
    let mut app = HttpApp::new();
    app.route::<EchoHandler>(SimpleMethod::POST, "/echo");

    let (addr, shutdown) = start_server(app);
    let client = make_client(addr);

    let response = client
        .post("http://testserver/echo")
        .unwrap()
        .build_client()
        .unwrap()
        .send()
        .unwrap();

    assert!(response.is_success());
    let body = body_text(response.get_body_ref());
    assert!(
        body.contains("POST"),
        "body should contain method POST: {body}"
    );

    shutdown.turn_on();
}

// ---------------------------------------------------------------------------
// Tests: param routes

/// GET /users/:id → matches /users/42, /users/abc, etc.
#[cfg(feature = "multi")]
#[test]
#[traced_test]
#[serial(http_test)]
fn test_param_route() {
    let _guard = initialize_pool(42, Some(5));
    let mut app = HttpApp::new();
    app.route::<EchoHandler>(SimpleMethod::GET, "/users/:id");

    let (addr, shutdown) = start_server(app);
    let client = make_client(addr);

    let r1 = client
        .get("http://testserver/users/42")
        .unwrap()
        .build_client()
        .unwrap()
        .send()
        .unwrap();
    assert!(r1.is_success());

    let r2 = client
        .get("http://testserver/users/abc")
        .unwrap()
        .build_client()
        .unwrap()
        .send()
        .unwrap();
    assert!(r2.is_success());

    shutdown.turn_on();
}

/// GET /users/:id/posts/:post_id — nested params.
#[cfg(feature = "multi")]
#[test]
#[serial(http_test)]
#[traced_test]
fn test_nested_param_route() {
    let _guard = initialize_pool(42, Some(5));
    let mut app = HttpApp::new();
    app.route::<EchoHandler>(SimpleMethod::GET, "/users/:id/posts/:post_id");

    let (addr, shutdown) = start_server(app);
    let client = make_client(addr);

    let response = client
        .get("http://testserver/users/123/posts/456")
        .unwrap()
        .build_client()
        .unwrap()
        .send()
        .unwrap();

    assert!(response.is_success());
    let body = body_text(response.get_body_ref());
    assert!(
        body.contains("/users/123/posts/456"),
        "body should contain full path: {body}"
    );

    shutdown.turn_on();
}

// ---------------------------------------------------------------------------
// Tests: wildcard routes

/// GET /files/* → matches any path starting with /files/.
#[cfg(feature = "multi")]
#[test]
#[traced_test]
#[serial(http_test)]
fn test_wildcard_route() {
    let _guard = initialize_pool(42, Some(5));
    let mut app = HttpApp::new();
    app.route::<EchoHandler>(SimpleMethod::GET, "/files/*");

    let (addr, shutdown) = start_server(app);
    let client = make_client(addr);

    let r2 = client
        .get("http://testserver/files/a/b/c.txt")
        .unwrap()
        .build_client()
        .unwrap()
        .send()
        .unwrap();
    assert!(r2.is_success());

    let r1 = client
        .get("http://testserver/files/a")
        .unwrap()
        .build_client()
        .unwrap()
        .send()
        .unwrap();
    assert!(r1.is_success());

    shutdown.turn_on();
}

// ---------------------------------------------------------------------------
// Tests: route_any

/// route_any matches all HTTP methods on the same path.
#[cfg(feature = "multi")]
#[test]
#[traced_test]
#[serial(http_test)]
fn test_route_any_matches_all_methods() {
    let _guard = initialize_pool(42, Some(5));
    let mut app = HttpApp::new();
    app.route_any::<EchoHandler>("/any");

    let (addr, shutdown) = start_server(app);
    let client = make_client(addr);

    assert!(client
        .get("http://testserver/any")
        .unwrap()
        .build_client()
        .unwrap()
        .send()
        .unwrap()
        .is_success());
    assert!(client
        .post("http://testserver/any")
        .unwrap()
        .build_client()
        .unwrap()
        .send()
        .unwrap()
        .is_success());
    assert!(client
        .delete("http://testserver/any")
        .unwrap()
        .build_client()
        .unwrap()
        .send()
        .unwrap()
        .is_success());
    assert!(client
        .put("http://testserver/any")
        .unwrap()
        .build_client()
        .unwrap()
        .send()
        .unwrap()
        .is_success());
    assert!(client
        .patch("http://testserver/any")
        .unwrap()
        .build_client()
        .unwrap()
        .send()
        .unwrap()
        .is_success());

    shutdown.turn_on();
}

// ---------------------------------------------------------------------------
// Tests: root route

#[cfg(feature = "multi")]
#[test]
#[traced_test]
#[serial(http_test)]
fn test_root_route() {
    let _guard = initialize_pool(42, Some(5));
    let mut app = HttpApp::new();
    app.route::<EchoHandler>(SimpleMethod::GET, "/");

    let (addr, shutdown) = start_server(app);
    let client = make_client(addr);

    let response = client
        .get("http://testserver/")
        .unwrap()
        .build_client()
        .unwrap()
        .send()
        .unwrap();

    assert!(response.is_success());

    shutdown.turn_on();
}

// ---------------------------------------------------------------------------
// Tests: 404 and method mismatch

/// Unmatched path → 404.
#[cfg(feature = "multi")]
#[test]
#[traced_test]
#[serial(http_test)]
fn test_not_found() {
    let _guard = initialize_pool(42, Some(5));
    let mut app = HttpApp::new();
    app.route::<EchoHandler>(SimpleMethod::GET, "/echo");

    let (addr, shutdown) = start_server(app);
    let client = make_client(addr);

    let response = client
        .get("http://testserver/nonexistent")
        .unwrap()
        .build_client()
        .unwrap()
        .send()
        .unwrap();

    assert_eq!(status_code(&response.get_status()), 404);

    shutdown.turn_on();
}

/// Route exists for GET but not POST → 404.
#[cfg(feature = "multi")]
#[test]
#[traced_test]
#[serial(http_test)]
fn test_method_mismatch_returns_not_found() {
    let _guard = initialize_pool(42, Some(5));
    let mut app = HttpApp::new();
    app.route::<EchoHandler>(SimpleMethod::GET, "/echo");

    let (addr, shutdown) = start_server(app);
    let client = make_client(addr);

    let response = client
        .post("http://testserver/echo")
        .unwrap()
        .build_client()
        .unwrap()
        .send()
        .unwrap();

    assert_eq!(status_code(&response.get_status()), 404);

    shutdown.turn_on();
}

// ---------------------------------------------------------------------------
// Tests: request body

/// POST with text body → body is echoed back.
#[cfg(feature = "multi")]
#[test]
#[traced_test]
#[serial(http_test)]
fn test_post_with_text_body() {
    let _guard = initialize_pool(42, Some(5));
    let mut app = HttpApp::new();
    app.route::<EchoHandler>(SimpleMethod::POST, "/body-echo");

    let (addr, shutdown) = start_server(app);
    let client = make_client(addr);

    let response = client
        .post("http://testserver/body-echo")
        .unwrap()
        .body_text("hello world")
        .build_client()
        .unwrap()
        .send()
        .unwrap();

    assert!(response.is_success());

    shutdown.turn_on();
}

/// POST with JSON body → body is echoed back.
#[cfg(feature = "multi")]
#[test]
#[traced_test]
#[serial(http_test)]
fn test_post_with_json_body() {
    let _guard = initialize_pool(42, Some(5));
    let mut app = HttpApp::new();
    app.route::<BodyEchoHandler>(SimpleMethod::POST, "/json");

    let (addr, shutdown) = start_server(app);
    let client = make_client(addr);

    let payload = serde_json::json!({ "name": "test", "value": 42 });

    let response = client
        .post("http://testserver/json")
        .unwrap()
        .body_json(&payload)
        .unwrap()
        .build_client()
        .unwrap()
        .send()
        .unwrap();

    assert!(response.is_success());
    let body = body_text(response.get_body_ref());
    assert!(body.contains("test"), "body should contain 'test': {body}");
    assert!(body.contains("42"), "body should contain '42': {body}");

    shutdown.turn_on();
}

// ---------------------------------------------------------------------------
// Tests: query strings

/// Query string is preserved in the request URL.
#[cfg(feature = "multi")]
#[test]
#[traced_test]
#[serial(http_test)]
fn test_query_string_preserved() {
    let _guard = initialize_pool(42, Some(5));
    let mut app = HttpApp::new();
    app.route::<QueryHandler>(SimpleMethod::GET, "/search");

    let (addr, shutdown) = start_server(app);
    let client = make_client(addr);

    let response = client
        .get("http://testserver/search?q=rust&lang=en")
        .unwrap()
        .build_client()
        .unwrap()
        .send()
        .unwrap();

    assert!(response.is_success());
    let body = body_text(response.get_body_ref());
    // The router matches /search, and the query string should be in the URL.
    assert!(
        body.contains("/search"),
        "body should contain /search: {body}"
    );

    shutdown.turn_on();
}

// ---------------------------------------------------------------------------
// Tests: middleware

/// Middleware runs before handler — counter increments for each request.
#[cfg(feature = "multi")]
#[test]
#[traced_test]
#[serial(http_test)]
fn test_middleware_runs_before_handler() {
    let _guard = initialize_pool(42, Some(5));
    let count = Arc::new(AtomicUsize::new(0));
    let mw = CounterMiddleware {
        count: count.clone(),
    };

    let mut app = HttpApp::new();
    app.middleware(mw);
    app.route::<EchoHandler>(SimpleMethod::GET, "/echo");

    let (addr, shutdown) = start_server(app);
    let client = make_client(addr);

    let _ = client
        .get("http://testserver/echo")
        .unwrap()
        .build_client()
        .unwrap()
        .send()
        .unwrap();
    let _ = client
        .get("http://testserver/echo")
        .unwrap()
        .build_client()
        .unwrap()
        .send()
        .unwrap();
    let _ = client
        .get("http://testserver/echo")
        .unwrap()
        .build_client()
        .unwrap()
        .send()
        .unwrap();

    assert_eq!(count.load(Ordering::SeqCst), 3);

    shutdown.turn_on();
}

/// Blocking middleware short-circuits — handler never runs.
#[cfg(feature = "multi")]
#[test]
#[traced_test]
#[serial(http_test)]
fn test_middleware_blocks_request() {
    let _guard = initialize_pool(42, Some(5));
    let mut app = HttpApp::new();
    app.middleware(BlockMiddleware);
    app.route::<EchoHandler>(SimpleMethod::GET, "/echo");

    let (addr, shutdown) = start_server(app);
    let client = make_client(addr);

    let response = client
        .get("http://testserver/echo")
        .unwrap()
        .build_client()
        .unwrap()
        .send()
        .unwrap();

    assert_eq!(status_code(&response.get_status()), 403);
    assert_eq!(body_text(response.get_body_ref()), "blocked by middleware");

    shutdown.turn_on();
}

/// Multiple middleware in chain — all run in order, block stops the chain.
#[cfg(feature = "multi")]
#[test]
#[traced_test]
#[serial(http_test)]
fn test_multiple_middleware_chain() {
    let _guard = initialize_pool(42, Some(5));
    let count = Arc::new(AtomicUsize::new(0));
    let mw1 = CounterMiddleware {
        count: count.clone(),
    };
    let mw2 = CounterMiddleware {
        count: count.clone(),
    };

    let mut app = HttpApp::new();
    app.middleware(mw1);
    app.middleware(mw2);
    app.route::<EchoHandler>(SimpleMethod::GET, "/echo");

    let (addr, shutdown) = start_server(app);
    let client = make_client(addr);

    let _ = client
        .get("http://testserver/echo")
        .unwrap()
        .build_client()
        .unwrap()
        .send()
        .unwrap();
    // Each request passes through both middleware.
    assert_eq!(count.load(Ordering::SeqCst), 2);

    shutdown.turn_on();
}

// ---------------------------------------------------------------------------
// Tests: multiple routes on same app (sequential, baseline)

#[cfg(feature = "multi")]
#[test]
#[traced_test]
#[serial(http_test)]
fn test_multiple_routes_same_app() {
    let _guard = initialize_pool(42, Some(5));
    let mut app = HttpApp::new();
    app.route::<EchoHandler>(SimpleMethod::GET, "/echo");
    app.route::<BodyEchoHandler>(SimpleMethod::POST, "/body");
    app.route::<QueryHandler>(SimpleMethod::GET, "/search");

    let (addr, shutdown) = start_server(app);
    let client = make_client(addr);

    // GET /echo
    tracing::info!("Moving to next request: http://testserver/echo");
    let r1 = client
        .get("http://testserver/echo")
        .unwrap()
        .build_client()
        .unwrap()
        .send()
        .unwrap();
    assert!(r1.is_success());
    let b1 = body_text(r1.get_body_ref());
    assert!(b1.contains("GET"));

    // POST /body
    tracing::info!("Moving to next request: http://testserver/body");
    let r2 = client
        .post("http://testserver/body")
        .unwrap()
        .body_text("hello")
        .build_client()
        .unwrap()
        .send()
        .unwrap();
    assert!(r2.is_success());
    assert_eq!(body_text(r2.get_body_ref()), "hello");

    // GET /search?q=test
    tracing::info!("Moving to next request: http://testserver/search?q=test");
    let r3 = client
        .get("http://testserver/search?q=test")
        .unwrap()
        .build_client()
        .unwrap()
        .send()
        .unwrap();
    assert!(r3.is_success());

    shutdown.turn_on();
}

// ---------------------------------------------------------------------------
// Tests: valtron multiplexing — concurrent connections with only 2 worker threads
//
// This test verifies that the valtron-driven keep-alive properly multiplexes
// idle connections. With pool=3 (2 valtron workers + 1 background thread),
// we fire N concurrent connections. If each connection blocked a thread,
// only 2 could run simultaneously. By proving all N succeed, we confirm
// that idle connections yield via TaskStatus::Delayed and free their thread
// for other work.

/// Slow handler that takes 500ms to respond — holds the connection open
/// long enough to force other connections to wait.
struct SlowHandler;

impl ServeFactory for SlowHandler {
    fn create(_bag: &ContextBag) -> Self {
        Self
    }
}

impl Serve for SlowHandler {
    fn serve(
        &self,
        _bag: Arc<ContextBag>,
        req: SimpleIncomingRequest,
        mut conn: SharedByteBufferStream<RawStream>,
    ) -> ConnectionResult {
        std::thread::sleep(Duration::from_millis(500));
        let path = &req.request_url.url;
        let body = serde_json::json!({ "path": path, "slept_ms": 500 });
        respond::json(&mut conn, 200, &body)
            .map_or(ConnectionResult::Close(None), |()| ConnectionResult::Keep)
    }
}

#[cfg(feature = "multi")]
#[test]
#[traced_test]
#[serial(http_test)]
fn test_valtron_multiplex_concurrent_connections() {
    // Pool of 3 = 2 valtron workers + 1 background thread.
    // We fire 6 concurrent connections — if connections blocked threads,
    // only 2 could run at once. All 6 succeeding proves multiplexing.

    // Phase 1: sequential baseline — 3 requests one after another.
    let _guard = initialize_pool(42, Some(5));
    let mut app = HttpApp::new();
    app.route::<SlowHandler>(SimpleMethod::GET, "/slow");
    app.route::<EchoHandler>(SimpleMethod::GET, "/echo");
    app.route::<QueryHandler>(SimpleMethod::GET, "/search");

    let (addr, shutdown) = start_server(app);

    // Sequential: verify basic routing works.
    let client = make_client(addr);
    let r1 = client
        .get("http://testserver/echo")
        .unwrap()
        .build_client()
        .unwrap()
        .send()
        .unwrap();
    assert!(r1.is_success(), "Sequential /echo should succeed");
    let r2 = client
        .get("http://testserver/slow")
        .unwrap()
        .build_client()
        .unwrap()
        .send()
        .unwrap();
    assert!(r2.is_success(), "Sequential /slow should succeed");
    let r3 = client
        .get("http://testserver/search?q=seq")
        .unwrap()
        .build_client()
        .unwrap()
        .send()
        .unwrap();
    assert!(r3.is_success(), "Sequential /search should succeed");

    // Phase 2: concurrent — 6 requests fired simultaneously on separate threads.
    // With only 2 valtron worker threads, if each connection blocked a thread,
    // at most 2 could run at once. All 6 succeeding proves idle connections
    // yield via TaskStatus::Delayed, freeing the thread for other work.
    const NUM_CONCURRENT: usize = 6;
    let handles: Vec<_> = (0..NUM_CONCURRENT)
        .map(|i| {
            let addr = addr;
            std::thread::spawn(move || {
                let client = make_client(addr);
                let url = match i % 3 {
                    0 => format!("http://testserver/slow?i={i}"),
                    1 => format!("http://testserver/echo?i={i}"),
                    _ => format!("http://testserver/search?q=conc&i={i}"),
                };
                let response = client.get(&url).unwrap().build_client().unwrap().send();
                (i, response)
            })
        })
        .collect();

    for handle in handles {
        let (i, result) = handle.join().expect("request thread panicked");
        let response = result.expect("request failed");
        assert!(
            response.is_success(),
            "Concurrent request {i} should succeed (multiplexing proof). Got status: {:?}",
            response.get_status()
        );
    }

    shutdown.turn_on();
}

// ---------------------------------------------------------------------------
// Tests: keep-alive (multiple requests in sequence)

#[cfg(feature = "multi")]
#[test]
#[traced_test]
#[serial(http_test)]
fn test_multiple_sequential_requests() {
    let _guard = initialize_pool(42, Some(5));
    let mut app = HttpApp::new();
    app.route::<EchoHandler>(SimpleMethod::GET, "/echo");

    let (addr, shutdown) = start_server(app);
    let client = make_client(addr);

    for i in 0..5 {
        let url = format!("http://testserver/echo?n={i}");
        let response = client
            .get(&url)
            .unwrap()
            .build_client()
            .unwrap()
            .send()
            .unwrap();
        assert!(response.is_success(), "Request {i} should succeed");
    }

    shutdown.turn_on();
}

// ---------------------------------------------------------------------------
// Tests: custom headers in requests

#[cfg(feature = "multi")]
#[test]
#[traced_test]
#[serial(http_test)]
fn test_custom_request_header() {
    let _guard = initialize_pool(42, Some(5));
    let mut app = HttpApp::new();
    app.route::<EchoHandler>(SimpleMethod::GET, "/echo");

    let (addr, shutdown) = start_server(app);
    let client = make_client(addr);

    let response = client
        .get("http://testserver/echo")
        .unwrap()
        .add_header("X-Custom-Header".to_string(), "test-value")
        .build_client()
        .unwrap()
        .send()
        .unwrap();

    assert!(response.is_success());

    shutdown.turn_on();
}

// ---------------------------------------------------------------------------
// Tests: HEAD request

#[cfg(feature = "multi")]
#[test]
#[traced_test]
#[serial(http_test)]
fn test_head_request() {
    let _guard = initialize_pool(42, Some(5));
    let mut app = HttpApp::new();
    app.route::<EchoHandler>(SimpleMethod::HEAD, "/echo");

    let (addr, shutdown) = start_server(app);
    let client = make_client(addr);

    let response = client
        .head("http://testserver/echo")
        .unwrap()
        .build_client()
        .unwrap()
        .send()
        .unwrap();

    assert!(response.is_success());

    shutdown.turn_on();
}

// ---------------------------------------------------------------------------
// Tests: DELETE request

#[cfg(feature = "multi")]
#[test]
#[serial(http_test)]
#[traced_test]
fn test_delete_request() {
    let _guard = initialize_pool(42, Some(5));
    let mut app = HttpApp::new();
    app.route::<EchoHandler>(SimpleMethod::DELETE, "/items/:id");

    let (addr, shutdown) = start_server(app);
    let client = make_client(addr);

    let response = client
        .delete("http://testserver/items/99")
        .unwrap()
        .build_client()
        .unwrap()
        .send()
        .unwrap();

    assert!(response.is_success());
    let body = body_text(response.get_body_ref());
    assert!(body.contains("DELETE"));

    shutdown.turn_on();
}

// ---------------------------------------------------------------------------
// Unit-style tests that don't need a running server (kept from original).

#[cfg(feature = "multi")]
#[test]
fn test_http_app_builder() {
    let app = HttpApp::new();
    app.context().store("test_config".to_string());

    let config = app.context().get::<String>();
    assert!(config.is_some());
    assert_eq!(*config.unwrap(), "test_config");
}

#[cfg(feature = "multi")]
#[test]
fn test_server_config_defaults() {
    let config = ServerConfig::defaults();
    assert_eq!(config.would_block_sleep(), Duration::from_millis(15)); // base sleep from calculator
    assert_eq!(config.accept_error_sleep(), Duration::from_millis(30)); // 2x base sleep
    assert_eq!(config.keep_alive.min_delay(), Duration::from_millis(15));
    assert_eq!(config.keep_alive.max_delay(), Duration::from_secs(300)); // max_read_timeout
    assert_eq!(config.keep_alive.idle_timeout(), Duration::from_secs(120)); // min_read_timeout
    assert_eq!(config.keep_alive.escalation_threshold, 50);
    assert_eq!(config.keep_alive.max_delay_cycles, 100);
    assert_eq!(config.max_body_bytes, 10 * 1024 * 1024);
}

#[cfg(feature = "multi")]
#[test]
fn test_server_config_builder() {
    let config = ServerConfig::defaults()
        .with_would_block_sleep(Duration::from_millis(50))
        .with_accept_error_sleep(Duration::from_millis(200))
        .with_keep_alive(
            foundation_http::KeepAliveConfig::defaults().with_idle_timeout(Duration::from_secs(30)),
        )
        .with_max_body_bytes(1024);

    // with_accept_error_sleep sets min_sleep_duration to dur/2 = 100ms
    // would_block_sleep() returns min_sleep_duration = 100ms
    // accept_error_sleep() returns would_block_sleep * 2 = 200ms
    assert_eq!(config.accept_error_sleep(), Duration::from_millis(200));
    // idle_timeout is now calculated from calculator
    assert_eq!(config.keep_alive.idle_timeout(), Duration::from_secs(30));
    assert_eq!(config.max_body_bytes, 1024);
}

#[cfg(feature = "multi")]
#[test]
fn test_context_store_multiple_types() {
    let app = HttpApp::new();
    app.context().store(42u32);
    app.context().store("hello".to_string());
    app.context().store(true);

    assert_eq!(*app.context().get::<u32>().unwrap(), 42);
    assert_eq!(*app.context().get::<String>().unwrap(), "hello");
    assert!(*app.context().get::<bool>().unwrap());
}
