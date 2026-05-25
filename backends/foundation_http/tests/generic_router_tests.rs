//! Tests for generic Router<S> — covers ServeWriter, RouteMethod<S>, and cross-type routing.

use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::Arc;

use foundation_http::shared::router::Router;
use foundation_http::shared::serve::{ConnectionResult, ServeWriter, ServeWriterFactory};
use foundation_http::shared::context::ContextBag;
use foundation_netio::simple_http::{
    Proto, SimpleIncomingRequest, SimpleMethod, SimpleHeaders,
};

// -----------------------------------------------------------------------
// Test ServeWriter handler

struct TestWriter {
    #[allow(dead_code)]
    id: usize,
    call_count: Arc<AtomicUsize>,
}

impl ServeWriterFactory for TestWriter {
    fn create(_bag: &ContextBag) -> Self {
        panic!("TestWriter must be created with new()");
    }
}

impl TestWriter {
    fn new(id: usize, call_count: Arc<AtomicUsize>) -> Self {
        Self { id, call_count }
    }
}

impl ServeWriter for TestWriter {
    fn serve_writer(
        &self,
        _bag: &ContextBag,
        _req: SimpleIncomingRequest,
        conn: &mut dyn std::io::Write,
    ) -> ConnectionResult {
        self.call_count.fetch_add(1, Ordering::SeqCst);
        let _ = conn.write_all(b"writer response");
        ConnectionResult::Keep
    }
}

fn make_writer(id: usize, count: Arc<AtomicUsize>) -> Arc<dyn ServeWriter> {
    Arc::new(TestWriter::new(id, count))
}

fn dummy_request() -> SimpleIncomingRequest {
    SimpleIncomingRequest::builder()
        .with_plain_url("/")
        .build()
        .unwrap()
}

// -----------------------------------------------------------------------
// Tests

#[test]
fn test_router_writer_dispatch() {
    let mut router: Router<Arc<dyn ServeWriter>> = Router::new();
    let count = Arc::new(AtomicUsize::new(0));

    router.add_route_writer(SimpleMethod::GET, "/api", make_writer(1, count.clone()));

    let handler = router.dispatch(&SimpleMethod::GET, "/api");
    assert!(handler.is_some());

    let mut buf = Vec::new();
    let result = handler.unwrap().serve_writer(&ContextBag::new(), dummy_request(), &mut buf);
    assert!(matches!(result, ConnectionResult::Keep));
    assert_eq!(count.load(Ordering::SeqCst), 1);
    assert_eq!(buf, b"writer response");
}

#[test]
fn test_router_writer_method_mismatch() {
    let mut router: Router<Arc<dyn ServeWriter>> = Router::new();
    let count = Arc::new(AtomicUsize::new(0));

    router.add_route_writer(SimpleMethod::POST, "/submit", make_writer(2, count.clone()));

    assert!(router.dispatch(&SimpleMethod::POST, "/submit").is_some());
    assert!(router.dispatch(&SimpleMethod::GET, "/submit").is_none());
}

#[test]
fn test_router_writer_any() {
    let mut router: Router<Arc<dyn ServeWriter>> = Router::new();
    let count = Arc::new(AtomicUsize::new(0));

    let handler = make_writer(3, count.clone());
    router.add_route_any_writer("/multi", &handler);

    assert!(router.dispatch(&SimpleMethod::GET, "/multi").is_some());
    assert!(router.dispatch(&SimpleMethod::POST, "/multi").is_some());
    assert!(router.dispatch(&SimpleMethod::DELETE, "/multi").is_some());
    assert!(router.dispatch(&SimpleMethod::PUT, "/multi").is_some());
}

#[test]
fn test_router_writer_param_route() {
    let mut router: Router<Arc<dyn ServeWriter>> = Router::new();
    let count = Arc::new(AtomicUsize::new(0));

    router.add_route_writer(SimpleMethod::GET, "/items/:id", make_writer(4, count.clone()));

    assert!(router.dispatch(&SimpleMethod::GET, "/items/42").is_some());
    assert!(router.dispatch(&SimpleMethod::GET, "/items/abc").is_some());
    assert!(router.dispatch(&SimpleMethod::GET, "/items").is_none());
}

#[test]
fn test_router_writer_wildcard() {
    let mut router: Router<Arc<dyn ServeWriter>> = Router::new();
    let count = Arc::new(AtomicUsize::new(0));

    router.add_route_writer(SimpleMethod::GET, "/*", make_writer(5, count.clone()));

    assert!(router.dispatch(&SimpleMethod::GET, "/anything/goes").is_some());
}

#[test]
fn test_router_multiple_writers_same_path() {
    let mut router: Router<Arc<dyn ServeWriter>> = Router::new();
    let get_count = Arc::new(AtomicUsize::new(0));
    let post_count = Arc::new(AtomicUsize::new(0));

    router.add_route_writer(SimpleMethod::GET, "/resource", make_writer(10, get_count.clone()));
    router.add_route_writer(SimpleMethod::POST, "/resource", make_writer(11, post_count.clone()));

    let handler = router.dispatch(&SimpleMethod::GET, "/resource").unwrap();
    let mut buf = Vec::new();
    let _ = handler.serve_writer(&ContextBag::new(), dummy_request(), &mut buf);
    assert_eq!(get_count.load(Ordering::SeqCst), 1);
    assert_eq!(post_count.load(Ordering::SeqCst), 0);

    let handler = router.dispatch(&SimpleMethod::POST, "/resource").unwrap();
    let mut buf = Vec::new();
    let _ = handler.serve_writer(&ContextBag::new(), dummy_request(), &mut buf);
    assert_eq!(post_count.load(Ordering::SeqCst), 1);
}
