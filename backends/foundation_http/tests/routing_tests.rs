//! Routing tests — RouteSegment tree matching, priority sorting, param extraction.

use std::sync::Arc;
use std::sync::atomic::{AtomicUsize, Ordering};

use foundation_core::io::ioutils::SharedByteBufferStream;
use foundation_core::netcap::RawStream;
use foundation_core::wire::simple_http::{SimpleIncomingRequest, SimpleMethod};

use foundation_http::context::ContextBag;
use foundation_http::router::Router;
use foundation_http::serve::{ConnectionResult, Serve, ServeFactory};

// -----------------------------------------------------------------------
// Test handler — records how many times it was called.

struct TestHandler {
    #[allow(dead_code)]
    id: usize,
    call_count: Arc<AtomicUsize>,
}

impl ServeFactory for TestHandler {
    fn create(_bag: &ContextBag) -> Self {
        panic!("TestHandler must be created with TestHandler::new()");
    }
}

impl TestHandler {
    fn new(id: usize, call_count: Arc<AtomicUsize>) -> Self {
        Self { id, call_count }
    }
}

impl Serve for TestHandler {
    fn serve(
        &self,
        _bag: Arc<ContextBag>,
        _req: SimpleIncomingRequest,
        _conn: SharedByteBufferStream<RawStream>,
    ) -> ConnectionResult {
        self.call_count.fetch_add(1, Ordering::SeqCst);
        ConnectionResult::Keep
    }
}

// -----------------------------------------------------------------------
// Helper: create a handler wrapped in ArcServe
fn make_handler(id: usize, count: Arc<AtomicUsize>) -> Arc<dyn Serve> {
    Arc::new(TestHandler::new(id, count))
}

// -----------------------------------------------------------------------
// Test fixtures
fn dummy_request() -> SimpleIncomingRequest {
    SimpleIncomingRequest::builder()
        .with_plain_url("/")
        .build()
        .unwrap_or_else(|_| {
            panic!("Failed to build dummy request")
        })
}

fn dummy_conn() -> SharedByteBufferStream<RawStream> {
    // Create a real TCP pair for testing
    let listener = std::net::TcpListener::bind("127.0.0.1:0").expect("Failed to bind");
    let addr = listener.local_addr().expect("Failed to get local addr");
    let _client = std::net::TcpStream::connect(addr).expect("Failed to connect");
    let (server, _client_addr) = listener.accept().expect("Failed to accept");
    let raw_stream = RawStream::from_tcp(server).expect("Failed to create RawStream");
    SharedByteBufferStream::rwrite(raw_stream)
}

// -----------------------------------------------------------------------
// Tests

#[test]
fn test_static_route_get() {
    let mut router = Router::new();
    let count = Arc::new(AtomicUsize::new(0));

    router.add_route(SimpleMethod::GET, "/hello", make_handler(1, count.clone()));

    let handler = router.dispatch(&SimpleMethod::GET, "/hello");
    assert!(handler.is_some());

    let result = handler.unwrap().serve(Arc::new(ContextBag::new()), dummy_request(), dummy_conn());
    assert!(matches!(result, ConnectionResult::Keep));
    assert_eq!(count.load(Ordering::SeqCst), 1);
}

#[test]
fn test_static_route_post() {
    let mut router = Router::new();
    let count = Arc::new(AtomicUsize::new(0));

    router.add_route(SimpleMethod::POST, "/users", make_handler(2, count.clone()));

    assert!(router.dispatch(&SimpleMethod::POST, "/users").is_some());
    assert!(router.dispatch(&SimpleMethod::GET, "/users").is_none());
    assert!(router.dispatch(&SimpleMethod::POST, "/other").is_none());
}

#[test]
fn test_param_route() {
    let mut router = Router::new();
    let count = Arc::new(AtomicUsize::new(0));

    router.add_route(SimpleMethod::GET, "/users/:id", make_handler(3, count.clone()));

    assert!(router.dispatch(&SimpleMethod::GET, "/users/123").is_some());
    assert!(router.dispatch(&SimpleMethod::GET, "/users/abc").is_some());
    assert!(router.dispatch(&SimpleMethod::GET, "/users").is_none());
}

#[test]
fn test_nested_routes() {
    let mut router = Router::new();
    let count = Arc::new(AtomicUsize::new(0));

    router.add_route(
        SimpleMethod::GET,
        "/api/v1/users",
        make_handler(4, count.clone()),
    );

    assert!(router.dispatch(&SimpleMethod::GET, "/api/v1/users").is_some());
    assert!(router.dispatch(&SimpleMethod::GET, "/api/v1/posts").is_none());
    assert!(router.dispatch(&SimpleMethod::GET, "/api/v2/users").is_none());
}

#[test]
fn test_wildcard_route() {
    let mut router = Router::new();
    let count = Arc::new(AtomicUsize::new(0));

    router.add_route(SimpleMethod::GET, "/*", make_handler(5, count.clone()));

    assert!(router.dispatch(&SimpleMethod::GET, "/anything").is_some());
    assert!(router.dispatch(&SimpleMethod::GET, "/a/b/c").is_some());
}

#[test]
fn test_static_precedence_over_param() {
    let mut router = Router::new();
    let static_count = Arc::new(AtomicUsize::new(0));
    let param_count = Arc::new(AtomicUsize::new(0));

    router.add_route(SimpleMethod::GET, "/users/profile", make_handler(10, static_count.clone()));
    router.add_route(SimpleMethod::GET, "/users/:action", make_handler(11, param_count.clone()));

    // Static route should win for exact match
    let handler = router.dispatch(&SimpleMethod::GET, "/users/profile").unwrap();
    let _ = handler.serve(Arc::new(ContextBag::new()), dummy_request(), dummy_conn());
    assert_eq!(static_count.load(Ordering::SeqCst), 1);
    assert_eq!(param_count.load(Ordering::SeqCst), 0);

    // Param route should match for non-static
    let handler = router.dispatch(&SimpleMethod::GET, "/users/edit").unwrap();
    let _ = handler.serve(Arc::new(ContextBag::new()), dummy_request(), dummy_conn());
    assert_eq!(param_count.load(Ordering::SeqCst), 1);
}

#[test]
fn test_multiple_methods_same_path() {
    let mut router = Router::new();
    let get_count = Arc::new(AtomicUsize::new(0));
    let post_count = Arc::new(AtomicUsize::new(0));

    router.add_route(SimpleMethod::GET, "/resource", make_handler(20, get_count.clone()));
    router.add_route(SimpleMethod::POST, "/resource", make_handler(21, post_count.clone()));

    let handler = router.dispatch(&SimpleMethod::GET, "/resource").unwrap();
    let _ = handler.serve(Arc::new(ContextBag::new()), dummy_request(), dummy_conn());
    assert_eq!(get_count.load(Ordering::SeqCst), 1);
    assert_eq!(post_count.load(Ordering::SeqCst), 0);

    let handler = router.dispatch(&SimpleMethod::POST, "/resource").unwrap();
    let _ = handler.serve(Arc::new(ContextBag::new()), dummy_request(), dummy_conn());
    assert_eq!(post_count.load(Ordering::SeqCst), 1);
}

#[test]
fn test_no_match_returns_none() {
    let mut router = Router::new();
    let count = Arc::new(AtomicUsize::new(0));

    router.add_route(SimpleMethod::GET, "/only", make_handler(30, count.clone()));

    assert!(router.dispatch(&SimpleMethod::DELETE, "/only").is_none());
    assert!(router.dispatch(&SimpleMethod::GET, "/nope").is_none());
}

#[test]
fn test_deep_nested_route() {
    let mut router = Router::new();
    let count = Arc::new(AtomicUsize::new(0));

    router.add_route(
        SimpleMethod::GET,
        "/a/b/c/d/e",
        make_handler(40, count.clone()),
    );

    assert!(router.dispatch(&SimpleMethod::GET, "/a/b/c/d/e").is_some());
    assert!(router.dispatch(&SimpleMethod::GET, "/a/b/c/d").is_none());
    assert!(router.dispatch(&SimpleMethod::GET, "/a/b/c/d/e/f").is_none());
}

#[test]
fn test_root_route() {
    let mut router = Router::new();
    let count = Arc::new(AtomicUsize::new(0));

    router.add_route(SimpleMethod::GET, "/", make_handler(50, count.clone()));

    assert!(router.dispatch(&SimpleMethod::GET, "/").is_some());
    assert!(router.dispatch(&SimpleMethod::GET, "/other").is_none());
}

#[test]
fn test_add_route_any() {
    let mut router = Router::new();
    let count = Arc::new(AtomicUsize::new(0));

    let handler = Arc::new(make_handler(60, count.clone()));
    router.add_route_any("/catch-all", &handler);

    assert!(router.dispatch(&SimpleMethod::GET, "/catch-all").is_some());
    assert!(router.dispatch(&SimpleMethod::POST, "/catch-all").is_some());
    assert!(router.dispatch(&SimpleMethod::DELETE, "/catch-all").is_some());
    assert!(router.dispatch(&SimpleMethod::PUT, "/catch-all").is_some());
}
