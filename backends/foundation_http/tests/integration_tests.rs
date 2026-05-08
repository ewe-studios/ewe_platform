//! HttpApp integration tests — middleware, config, and serve pipeline.

use std::sync::Arc;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::time::Duration;

use foundation_core::synca::OnSignal;
use foundation_core::valtron::BackgroundJobRegistry;
use foundation_core::wire::simple_http::{
    SimpleIncomingRequest, SimpleMethod, SimpleUrl,
};

use foundation_http::{
    ContextBag, HttpApp, HttpServer, MiddlewareResult, RequestMiddleware,
    ServerConfig,
    SimpleOutgoingResponse, SendSafeBody, Status,
};

// -----------------------------------------------------------------------
// Counter middleware — counts requests passing through.

struct CountingMiddleware {
    count: Arc<AtomicUsize>,
}

impl RequestMiddleware for CountingMiddleware {
    fn handle(
        &self,
        _ctx: &Arc<ContextBag>,
        _req: &mut SimpleIncomingRequest,
    ) -> MiddlewareResult {
        self.count.fetch_add(1, Ordering::SeqCst);
        MiddlewareResult::Continue
    }
}

// -----------------------------------------------------------------------
// Blocking middleware — short-circuits with 403.

struct BlockAllMiddleware;

impl RequestMiddleware for BlockAllMiddleware {
    fn handle(
        &self,
        _ctx: &Arc<ContextBag>,
        _req: &mut SimpleIncomingRequest,
    ) -> MiddlewareResult {
        let response = SimpleOutgoingResponse::builder()
            .with_status(Status::Forbidden)
            .with_body(SendSafeBody::Text("blocked".into()))
            .build()
            .expect("valid response");
        MiddlewareResult::Response(response)
    }
}

// -----------------------------------------------------------------------
// Helper: create a SimpleUrl for tests.

fn test_url(path: &str) -> SimpleUrl {
    SimpleUrl::url_only(path)
}

// -----------------------------------------------------------------------
// Helper: create a minimal request for middleware testing.

fn test_request(path: &str) -> SimpleIncomingRequest {
    SimpleIncomingRequest::builder()
        .with_method(SimpleMethod::GET)
        .with_url(test_url(path))
        .build()
        .expect("valid request")
}

// -----------------------------------------------------------------------
// Tests

#[test]
fn test_http_app_builder() {
    let app = HttpApp::new();
    app.context().store("test_config".to_string());

    let config = app.context().get::<String>();
    assert!(config.is_some());
    assert_eq!(*config.unwrap(), "test_config");
}

#[test]
fn test_server_config_defaults() {
    let config = ServerConfig::defaults();
    assert_eq!(config.would_block_sleep, Duration::from_millis(10));
    assert_eq!(config.accept_error_sleep, Duration::from_millis(100));
    assert!(config.keep_alive_timeout.is_none());
    assert_eq!(config.max_body_bytes, 10 * 1024 * 1024);
}

#[test]
fn test_server_config_builder() {
    let config = ServerConfig::defaults()
        .with_would_block_sleep(Duration::from_millis(50))
        .with_accept_error_sleep(Duration::from_millis(200))
        .with_keep_alive_timeout(Duration::from_secs(30))
        .with_max_body_bytes(1024);

    assert_eq!(config.would_block_sleep, Duration::from_millis(50));
    assert_eq!(config.accept_error_sleep, Duration::from_millis(200));
    assert_eq!(config.keep_alive_timeout, Some(Duration::from_secs(30)));
    assert_eq!(config.max_body_bytes, 1024);
}

#[test]
fn test_middleware_continues() {
    let count = Arc::new(AtomicUsize::new(0));
    let mw = CountingMiddleware { count: count.clone() };
    let mut req = test_request("/echo");
    let ctx = Arc::new(ContextBag::new());

    match mw.handle(&ctx, &mut req) {
        MiddlewareResult::Continue => {}
        MiddlewareResult::Response(_) => panic!("middleware should not short-circuit"),
    }

    assert_eq!(count.load(Ordering::SeqCst), 1);
}

#[test]
fn test_middleware_short_circuit() {
    let mw = BlockAllMiddleware;
    let mut req = test_request("/echo");
    let ctx = Arc::new(ContextBag::new());

    let result = mw.handle(&ctx, &mut req);
    assert!(matches!(result, MiddlewareResult::Response(_)));
}

#[test]
fn test_multiple_middleware_chain() {
    let count1 = Arc::new(AtomicUsize::new(0));
    let count2 = Arc::new(AtomicUsize::new(0));

    let mw1 = CountingMiddleware { count: count1.clone() };
    let mw2 = CountingMiddleware { count: count2.clone() };

    let ctx = Arc::new(ContextBag::new());
    let mut req = test_request("/any");

    match mw1.handle(&ctx, &mut req) {
        MiddlewareResult::Continue => {}
        MiddlewareResult::Response(_) => panic!("first middleware short-circuited"),
    }
    match mw2.handle(&ctx, &mut req) {
        MiddlewareResult::Continue => {}
        MiddlewareResult::Response(_) => panic!("second middleware short-circuited"),
    }

    assert_eq!(count1.load(Ordering::SeqCst), 1);
    assert_eq!(count2.load(Ordering::SeqCst), 1);
}

#[test]
fn test_middleware_chain_with_block() {
    let count = Arc::new(AtomicUsize::new(0));
    let mw1 = CountingMiddleware { count: count.clone() };
    let mw2 = BlockAllMiddleware;

    let ctx = Arc::new(ContextBag::new());
    let mut req = test_request("/any");

    match mw1.handle(&ctx, &mut req) {
        MiddlewareResult::Continue => {}
        MiddlewareResult::Response(_) => panic!("counting middleware short-circuited"),
    }
    match mw2.handle(&ctx, &mut req) {
        MiddlewareResult::Continue => panic!("block middleware did not short-circuit"),
        MiddlewareResult::Response(_) => {}
    }

    assert_eq!(count.load(Ordering::SeqCst), 1);
}

#[test]
fn test_http_server_creation() {
    let shutdown = Arc::new(OnSignal::new());
    let bg = BackgroundJobRegistry::new(2, shutdown.clone(), Duration::from_millis(1));

    let app = HttpApp::new();
    let _server = HttpServer::new(app, bg, "127.0.0.1:0");
}

#[test]
fn test_http_server_with_config_creation() {
    let shutdown = Arc::new(OnSignal::new());
    let bg = BackgroundJobRegistry::new(2, shutdown.clone(), Duration::from_millis(1));

    let config = ServerConfig::defaults()
        .with_would_block_sleep(Duration::from_millis(5))
        .with_keep_alive_timeout(Duration::from_secs(10));

    let app = HttpApp::new();
    let _server = HttpServer::with_config(app, bg, "127.0.0.1:0", config);
}

#[test]
fn test_context_store_multiple_types() {
    let app = HttpApp::new();
    app.context().store(42u32);
    app.context().store("hello".to_string());
    app.context().store(true);

    assert_eq!(*app.context().get::<u32>().unwrap(), 42);
    assert_eq!(*app.context().get::<String>().unwrap(), "hello");
    assert_eq!(*app.context().get::<bool>().unwrap(), true);
}
