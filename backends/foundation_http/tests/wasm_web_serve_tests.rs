//! Tests for `HttpApp`<Arc<dyn WebServe>> — route registration and dispatch
//! through the generic router.
//!
//! WHY: `WebServe` stays in `shared/` because it needs no native I/O. After the
//! native `Serve` and `ServerApp` types moved to `native/`, this test proves the
//! browser path still builds and dispatches. It runs on the host, and the same
//! dispatch code compiles for wasm32.

use std::sync::Arc;

use foundation_http::shared::app::HttpApp;
use foundation_http::shared::context::Ctx;
use foundation_http::shared::middleware::{MiddlewareResult, RequestMiddleware};
use foundation_http::shared::serve::{ConnectionResult, ServeWriter, ServeWriterFactory};
use foundation_http::shared::serve_web::{WebServe, WebServeFactory};
use foundation_http::shared::web_conn::{WebConn, WebConnectionResult};
use foundation_netio::shared::http::{
    Proto, SendSafeBody, SimpleHeaders, SimpleIncomingRequest, SimpleMethod,
    SimpleOutgoingResponse, Status,
};

// -----------------------------------------------------------------------
// WebServe test handler

struct TestWebHandler;

impl WebServeFactory for TestWebHandler {
    fn create(_ctx: &Ctx) -> Self {
        TestWebHandler
    }
}

#[async_trait::async_trait(?Send)]
impl WebServe for TestWebHandler {
    async fn serve_web(
        &self,
        _ctx: Ctx,
        _req: SimpleIncomingRequest,
        conn: &mut WebConn,
    ) -> WebConnectionResult {
        conn.set_status(200);
        conn.set_body(b"web ok".to_vec());
        WebConnectionResult::Ok
    }
}

// -----------------------------------------------------------------------
// Test middleware

struct PassThroughMiddleware;
impl RequestMiddleware for PassThroughMiddleware {
    fn handle(&self, _ctx: &Ctx, _req: &mut SimpleIncomingRequest) -> MiddlewareResult {
        MiddlewareResult::Continue
    }
}

struct RejectMiddleware;
impl RequestMiddleware for RejectMiddleware {
    fn handle(&self, _ctx: &Ctx, _req: &mut SimpleIncomingRequest) -> MiddlewareResult {
        MiddlewareResult::Response(SimpleOutgoingResponse {
            proto: Proto::HTTP11,
            status: Status::Unauthorized,
            headers: SimpleHeaders::new(),
            body: Some(SendSafeBody::Text("Unauthorized".into())),
            trailers: SimpleHeaders::new(),
        })
    }
}

// -----------------------------------------------------------------------
// ServeWriter test handler (for the cross-type test)

struct TestWriterHandler;

impl ServeWriterFactory for TestWriterHandler {
    fn create(_ctx: &Ctx) -> Self {
        TestWriterHandler
    }
}

impl ServeWriter for TestWriterHandler {
    fn serve_writer(
        &self,
        _ctx: &Ctx,
        _req: SimpleIncomingRequest,
        _conn: &mut dyn std::io::Write,
    ) -> ConnectionResult {
        ConnectionResult::Keep
    }
}

// -----------------------------------------------------------------------
// WebServe tests

#[test]
fn test_http_app_web_route_registration() {
    let mut app = HttpApp::<Arc<dyn WebServe>>::new_web();
    app.route_web::<TestWebHandler>(SimpleMethod::GET, "/web");
    assert!(app.router().dispatch(&SimpleMethod::GET, "/web").is_some());
    assert!(app.router().dispatch(&SimpleMethod::POST, "/web").is_none());
}

#[test]
fn test_http_app_web_route_any() {
    let mut app = HttpApp::<Arc<dyn WebServe>>::new_web();
    app.route_any_web::<TestWebHandler>("/web-all");
    assert!(app
        .router()
        .dispatch(&SimpleMethod::GET, "/web-all")
        .is_some());
    assert!(app
        .router()
        .dispatch(&SimpleMethod::POST, "/web-all")
        .is_some());
    assert!(app
        .router()
        .dispatch(&SimpleMethod::PUT, "/web-all")
        .is_some());
}

#[test]
fn test_http_app_web_middleware_pass_through() {
    let mut app = HttpApp::<Arc<dyn WebServe>>::new_web();
    app.middleware(PassThroughMiddleware);
    assert_eq!(app.middleware_chain().len(), 1);
}

#[test]
fn test_http_app_web_middleware_short_circuit() {
    let mut app = HttpApp::<Arc<dyn WebServe>>::new_web();
    app.middleware(RejectMiddleware);
    app.route_web::<TestWebHandler>(SimpleMethod::GET, "/web");
    assert_eq!(app.middleware_chain().len(), 1);
}

#[test]
fn test_http_app_web_param_route() {
    let mut app = HttpApp::<Arc<dyn WebServe>>::new_web();
    app.route_web::<TestWebHandler>(SimpleMethod::GET, "/web/items/:id");
    assert!(app
        .router()
        .dispatch(&SimpleMethod::GET, "/web/items/42")
        .is_some());
    assert!(app
        .router()
        .dispatch(&SimpleMethod::GET, "/web/items")
        .is_none());
}

#[test]
fn test_http_app_web_wildcard_route() {
    let mut app = HttpApp::<Arc<dyn WebServe>>::new_web();
    app.route_web::<TestWebHandler>(SimpleMethod::GET, "/web/*");
    assert!(app
        .router()
        .dispatch(&SimpleMethod::GET, "/web/anything")
        .is_some());
    assert!(app
        .router()
        .dispatch(&SimpleMethod::GET, "/web/a/b/c")
        .is_some());
}

// -----------------------------------------------------------------------
// Cross-type test — verify ServeWriter coexists separately

#[test]
fn test_serve_writer_app_route_registration() {
    let mut app = HttpApp::<Arc<dyn ServeWriter>>::new_writer();
    app.route_writer::<TestWriterHandler>(SimpleMethod::GET, "/writer");
    assert!(app
        .router()
        .dispatch(&SimpleMethod::GET, "/writer")
        .is_some());
    assert!(app
        .router()
        .dispatch(&SimpleMethod::POST, "/writer")
        .is_none());
}
