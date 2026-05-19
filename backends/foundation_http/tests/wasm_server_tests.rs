//! Tests for wasm server dispatch — middleware chain and short-circuit behavior.

use foundation_http::wasm::server::run_middleware;
use foundation_http::shared::app::HttpApp;
use foundation_http::shared::context::ContextBag;
use foundation_http::shared::middleware::{MiddlewareResult, RequestMiddleware};
use foundation_core::wire::simple_http::{
    Proto, SendSafeBody, SimpleHeaders, SimpleHeader, SimpleIncomingRequest, SimpleMethod,
    SimpleOutgoingResponse, Status,
};
use std::sync::Arc;

/// Middleware that always continues — should not modify the request.
struct PassThroughMiddleware;
impl RequestMiddleware for PassThroughMiddleware {
    fn handle(&self, _ctx: &Arc<ContextBag>, _req: &mut SimpleIncomingRequest) -> MiddlewareResult {
        MiddlewareResult::Continue
    }
}

/// Middleware that short-circuits with 401.
struct RejectMiddleware;
impl RequestMiddleware for RejectMiddleware {
    fn handle(&self, _ctx: &Arc<ContextBag>, _req: &mut SimpleIncomingRequest) -> MiddlewareResult {
        MiddlewareResult::Response(SimpleOutgoingResponse {
            proto: Proto::HTTP11,
            status: Status::Unauthorized,
            headers: SimpleHeaders::new(),
            body: Some(SendSafeBody::Text("Unauthorized".into())),
        })
    }
}

/// Middleware that adds a custom header to the request.
struct AddHeaderMiddleware;
impl RequestMiddleware for AddHeaderMiddleware {
    fn handle(&self, _ctx: &Arc<ContextBag>, req: &mut SimpleIncomingRequest) -> MiddlewareResult {
        req.headers
            .entry(SimpleHeader::from("X-Middleware".to_string()))
            .or_default()
            .push("passed".to_string());
        MiddlewareResult::Continue
    }
}

fn make_request() -> SimpleIncomingRequest {
    SimpleIncomingRequest::builder()
        .with_parsed_url("http://example.com/test")
        .with_method(SimpleMethod::from("GET".to_string()))
        .with_proto(Proto::HTTP11)
        .with_headers(SimpleHeaders::new())
        .build()
        .expect("valid request")
}

#[test]
fn test_run_middleware_no_chain_returns_none() {
    let app = HttpApp::new();
    let bag = Arc::new(ContextBag::new());
    let req = make_request();

    let (returned_req, short_circuit) = run_middleware(&app, bag, req);
    assert!(short_circuit.is_none());
    assert_eq!(returned_req.method, SimpleMethod::from("GET".to_string()));
}

#[test]
fn test_run_middleware_pass_through() {
    let mut app = HttpApp::new();
    app.middleware(PassThroughMiddleware);
    let bag = Arc::new(ContextBag::new());
    let req = make_request();

    let (returned_req, short_circuit) = run_middleware(&app, bag, req);
    assert!(short_circuit.is_none());
    assert_eq!(returned_req.method, SimpleMethod::from("GET".to_string()));
}

#[test]
fn test_run_middleware_short_circuit_401() {
    let mut app = HttpApp::new();
    app.middleware(RejectMiddleware);
    let bag = Arc::new(ContextBag::new());
    let req = make_request();

    let (_returned_req, short_circuit) = run_middleware(&app, bag, req);
    let response = short_circuit.expect("should have short-circuited");
    assert_eq!(response.status, 401);
    assert_eq!(response.body, Some(b"Unauthorized".to_vec()));
    assert!(response.headers.is_empty());
}

#[test]
fn test_run_middleware_adds_header() {
    let mut app = HttpApp::new();
    app.middleware(AddHeaderMiddleware);
    let bag = Arc::new(ContextBag::new());
    let req = make_request();

    let (returned_req, short_circuit) = run_middleware(&app, bag, req);
    assert!(short_circuit.is_none());
    let header_key = SimpleHeader::from("X-Middleware".to_string());
    let values = returned_req.headers.get(&header_key).expect("header should exist");
    assert_eq!(values[0], "passed");
}

#[test]
fn test_run_middleware_multiple_continue() {
    let mut app = HttpApp::new();
    app.middleware(PassThroughMiddleware);
    app.middleware(AddHeaderMiddleware);
    app.middleware(PassThroughMiddleware);
    let bag = Arc::new(ContextBag::new());
    let req = make_request();

    let (returned_req, short_circuit) = run_middleware(&app, bag, req);
    assert!(short_circuit.is_none());
    let header_key = SimpleHeader::from("X-Middleware".to_string());
    assert!(returned_req.headers.get(&header_key).is_some());
}

#[test]
fn test_run_middleware_first_blocks_skips_rest() {
    let mut app = HttpApp::new();
    app.middleware(RejectMiddleware);
    app.middleware(AddHeaderMiddleware);
    let bag = Arc::new(ContextBag::new());
    let req = make_request();

    let (_returned_req, short_circuit) = run_middleware(&app, bag, req);
    let response = short_circuit.expect("should have short-circuited");
    assert_eq!(response.status, 401);
    assert!(response.headers.iter().all(|(k, _)| k != "X-Middleware"));
}

#[test]
fn test_middleware_short_circuit_forbidden_text() {
    struct TextMiddleware;
    impl RequestMiddleware for TextMiddleware {
        fn handle(&self, _ctx: &Arc<ContextBag>, _req: &mut SimpleIncomingRequest) -> MiddlewareResult {
            MiddlewareResult::Response(SimpleOutgoingResponse {
                proto: Proto::HTTP11,
                status: Status::Forbidden,
                headers: SimpleHeaders::new(),
                body: Some(SendSafeBody::Text("Access denied".into())),
            })
        }
    }

    let mut app = HttpApp::new();
    app.middleware(TextMiddleware);
    let bag = Arc::new(ContextBag::new());
    let req = make_request();

    let (_, short_circuit) = run_middleware(&app, bag, req);
    let response = short_circuit.unwrap();
    assert_eq!(response.status, 403);
    assert_eq!(response.body, Some(b"Access denied".to_vec()));
}

#[test]
fn test_middleware_short_circuit_bytes_body() {
    struct BinaryMiddleware;
    impl RequestMiddleware for BinaryMiddleware {
        fn handle(&self, _ctx: &Arc<ContextBag>, _req: &mut SimpleIncomingRequest) -> MiddlewareResult {
            MiddlewareResult::Response(SimpleOutgoingResponse {
                proto: Proto::HTTP11,
                status: Status::BadRequest,
                headers: SimpleHeaders::new(),
                body: Some(SendSafeBody::Bytes(vec![0x01, 0x02, 0x03])),
            })
        }
    }

    let mut app = HttpApp::new();
    app.middleware(BinaryMiddleware);
    let bag = Arc::new(ContextBag::new());
    let req = make_request();

    let (_, short_circuit) = run_middleware(&app, bag, req);
    let response = short_circuit.unwrap();
    assert_eq!(response.status, 400);
    assert_eq!(response.body, Some(vec![0x01, 0x02, 0x03]));
}

#[test]
fn test_middleware_short_circuit_with_headers() {
    struct HeaderMiddleware;
    impl RequestMiddleware for HeaderMiddleware {
        fn handle(&self, _ctx: &Arc<ContextBag>, _req: &mut SimpleIncomingRequest) -> MiddlewareResult {
            let mut headers = SimpleHeaders::new();
            headers
                .entry(SimpleHeader::from("WWW-Authenticate".to_string()))
                .or_default()
                .push("Bearer realm=\"test\"".to_string());
            MiddlewareResult::Response(SimpleOutgoingResponse {
                proto: Proto::HTTP11,
                status: Status::Unauthorized,
                headers,
                body: Some(SendSafeBody::Text("Unauthorized".into())),
            })
        }
    }

    let mut app = HttpApp::new();
    app.middleware(HeaderMiddleware);
    let bag = Arc::new(ContextBag::new());
    let req = make_request();

    let (_, short_circuit) = run_middleware(&app, bag, req);
    let response = short_circuit.unwrap();
    assert_eq!(response.status, 401);
    assert_eq!(response.headers.len(), 1);
    assert_eq!(response.headers[0], ("WWW-AUTHENTICATE".to_string(), "Bearer realm=\"test\"".to_string()));
}
