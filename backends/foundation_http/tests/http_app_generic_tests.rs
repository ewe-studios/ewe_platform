//! Tests for HttpApp<Arc<dyn CfServe>> and HttpApp<Arc<dyn WebServe>> — route registration
//! and dispatch through the generic router.

use std::sync::Arc;

use foundation_http::shared::app::HttpApp;
use foundation_http::shared::context::ContextBag;
use foundation_http::shared::middleware::{MiddlewareResult, RequestMiddleware};
use foundation_http::shared::serve::{ServeWriter, ServeWriterFactory, ConnectionResult};
use foundation_netio::simple_http::shared::{
    Proto, SendSafeBody, SimpleHeaders, SimpleIncomingRequest, SimpleMethod,
    SimpleOutgoingResponse, Status,
};

// -----------------------------------------------------------------------
// CfServe test handler — implements Factory properly (doesn't panic)

struct TestCfHandler;

impl foundation_http::wasm::serve_cf::CfServeFactory for TestCfHandler {
    fn create(_bag: &ContextBag) -> Self {
        TestCfHandler
    }
}

impl foundation_http::wasm::serve_cf::CfServe for TestCfHandler {
    async fn serve_cf(
        &self,
        _bag: Arc<ContextBag>,
        _req: SimpleIncomingRequest,
        conn: &mut foundation_http::wasm::cf_conn::CfConn,
    ) -> foundation_http::wasm::cf_conn::CfConnectionResult {
        conn.set_status(200);
        conn.set_body(b"cf ok".to_vec());
        foundation_http::wasm::cf_conn::CfConnectionResult::Ok
    }
}

// -----------------------------------------------------------------------
// WebServe test handler — implements Factory properly

struct TestWebHandler;

impl foundation_http::wasm::serve_web::WebServeFactory for TestWebHandler {
    fn create(_bag: &ContextBag) -> Self {
        TestWebHandler
    }
}

impl foundation_http::wasm::serve_web::WebServe for TestWebHandler {
    async fn serve_web(
        &self,
        _bag: Arc<ContextBag>,
        _req: SimpleIncomingRequest,
        conn: &mut foundation_http::wasm::web_conn::WebConn,
    ) -> foundation_http::wasm::web_conn::WebConnectionResult {
        conn.set_status(200);
        conn.set_body(b"web ok".to_vec());
        foundation_http::wasm::web_conn::WebConnectionResult::Ok
    }
}

// -----------------------------------------------------------------------
// Test middleware

struct PassThroughMiddleware;
impl RequestMiddleware for PassThroughMiddleware {
    fn handle(&self, _ctx: &Arc<ContextBag>, _req: &mut SimpleIncomingRequest) -> MiddlewareResult {
        MiddlewareResult::Continue
    }
}

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

// -----------------------------------------------------------------------
// ServeWriter test handler (for cross-type test)

struct TestWriterHandler;

impl ServeWriterFactory for TestWriterHandler {
    fn create(_bag: &ContextBag) -> Self {
        TestWriterHandler
    }
}

impl ServeWriter for TestWriterHandler {
    fn serve_writer(
        &self,
        _bag: &ContextBag,
        _req: SimpleIncomingRequest,
        _conn: &mut dyn std::io::Write,
    ) -> ConnectionResult {
        ConnectionResult::Keep
    }
}

// -----------------------------------------------------------------------
// CfServe tests

#[test]
fn test_http_app_cf_route_registration() {
    let mut app = HttpApp::<Arc<dyn foundation_http::wasm::serve_cf::CfServe>>::new_cf();
    app.route_cf::<TestCfHandler>(SimpleMethod::GET, "/cf");
    assert!(app.router().dispatch(&SimpleMethod::GET, "/cf").is_some());
    assert!(app.router().dispatch(&SimpleMethod::POST, "/cf").is_none());
}

#[test]
fn test_http_app_cf_route_any() {
    let mut app = HttpApp::<Arc<dyn foundation_http::wasm::serve_cf::CfServe>>::new_cf();
    app.route_any_cf::<TestCfHandler>("/cf-all");
    assert!(app.router().dispatch(&SimpleMethod::GET, "/cf-all").is_some());
    assert!(app.router().dispatch(&SimpleMethod::POST, "/cf-all").is_some());
    assert!(app.router().dispatch(&SimpleMethod::DELETE, "/cf-all").is_some());
}

#[test]
fn test_http_app_cf_middleware_pass_through() {
    let mut app = HttpApp::<Arc<dyn foundation_http::wasm::serve_cf::CfServe>>::new_cf();
    app.middleware(PassThroughMiddleware);
    assert_eq!(app.middleware_chain().len(), 1);
}

#[test]
fn test_http_app_cf_param_route() {
    let mut app = HttpApp::<Arc<dyn foundation_http::wasm::serve_cf::CfServe>>::new_cf();
    app.route_cf::<TestCfHandler>(SimpleMethod::GET, "/cf/users/:id");
    assert!(app.router().dispatch(&SimpleMethod::GET, "/cf/users/123").is_some());
    assert!(app.router().dispatch(&SimpleMethod::GET, "/cf/users/abc").is_some());
    assert!(app.router().dispatch(&SimpleMethod::GET, "/cf/users").is_none());
}

#[test]
fn test_http_app_cf_wildcard_route() {
    let mut app = HttpApp::<Arc<dyn foundation_http::wasm::serve_cf::CfServe>>::new_cf();
    app.route_cf::<TestCfHandler>(SimpleMethod::GET, "/cf/*");
    assert!(app.router().dispatch(&SimpleMethod::GET, "/cf/anything").is_some());
    assert!(app.router().dispatch(&SimpleMethod::GET, "/cf/a/b/c").is_some());
}

// -----------------------------------------------------------------------
// WebServe tests

#[test]
fn test_http_app_web_route_registration() {
    let mut app = HttpApp::<Arc<dyn foundation_http::wasm::serve_web::WebServe>>::new_web();
    app.route_web::<TestWebHandler>(SimpleMethod::GET, "/web");
    assert!(app.router().dispatch(&SimpleMethod::GET, "/web").is_some());
    assert!(app.router().dispatch(&SimpleMethod::POST, "/web").is_none());
}

#[test]
fn test_http_app_web_route_any() {
    let mut app = HttpApp::<Arc<dyn foundation_http::wasm::serve_web::WebServe>>::new_web();
    app.route_any_web::<TestWebHandler>("/web-all");
    assert!(app.router().dispatch(&SimpleMethod::GET, "/web-all").is_some());
    assert!(app.router().dispatch(&SimpleMethod::POST, "/web-all").is_some());
    assert!(app.router().dispatch(&SimpleMethod::PUT, "/web-all").is_some());
}

#[test]
fn test_http_app_web_middleware_pass_through() {
    let mut app = HttpApp::<Arc<dyn foundation_http::wasm::serve_web::WebServe>>::new_web();
    app.middleware(PassThroughMiddleware);
    assert_eq!(app.middleware_chain().len(), 1);
}

#[test]
fn test_http_app_web_middleware_short_circuit() {
    let mut app = HttpApp::<Arc<dyn foundation_http::wasm::serve_web::WebServe>>::new_web();
    app.middleware(RejectMiddleware);
    app.route_web::<TestWebHandler>(SimpleMethod::GET, "/web");
    assert_eq!(app.middleware_chain().len(), 1);
}

#[test]
fn test_http_app_web_param_route() {
    let mut app = HttpApp::<Arc<dyn foundation_http::wasm::serve_web::WebServe>>::new_web();
    app.route_web::<TestWebHandler>(SimpleMethod::GET, "/web/items/:id");
    assert!(app.router().dispatch(&SimpleMethod::GET, "/web/items/42").is_some());
    assert!(app.router().dispatch(&SimpleMethod::GET, "/web/items").is_none());
}

#[test]
fn test_http_app_web_wildcard_route() {
    let mut app = HttpApp::<Arc<dyn foundation_http::wasm::serve_web::WebServe>>::new_web();
    app.route_web::<TestWebHandler>(SimpleMethod::GET, "/web/*");
    assert!(app.router().dispatch(&SimpleMethod::GET, "/web/anything").is_some());
    assert!(app.router().dispatch(&SimpleMethod::GET, "/web/a/b/c").is_some());
}

// -----------------------------------------------------------------------
// Cross-type test — verify ServeWriter coexists separately

#[test]
fn test_serve_writer_app_route_registration() {
    let mut app = HttpApp::<Arc<dyn ServeWriter>>::new_writer();
    app.route_writer::<TestWriterHandler>(SimpleMethod::GET, "/writer");
    assert!(app.router().dispatch(&SimpleMethod::GET, "/writer").is_some());
    assert!(app.router().dispatch(&SimpleMethod::POST, "/writer").is_none());
}
