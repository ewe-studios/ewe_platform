//! Tests for `HttpApp`<Arc<dyn CfServe>> — route registration and dispatch
//! through the generic router.
//!
//! WHY: `CfServe` stays in `wasm/` and needs `wasm-bindgen-http`, so this test
//! type-checks on wasm32 only. The browser `WebServe` path runs on the host in
//! `wasm_web_serve_tests`.
//!
//! Requires the wasm32 target with `--features "wasm-test wasm-bindgen-http"`.

#![cfg(all(
    target_family = "wasm",
    feature = "wasm-test",
    feature = "wasm-bindgen-http"
))]

use std::sync::Arc;

use foundation_http::shared::app::HttpApp;
use foundation_http::shared::context::Ctx;
use foundation_netio::shared::http::{SimpleIncomingRequest, SimpleMethod};

// -----------------------------------------------------------------------
// CfServe test handler — implements Factory properly (doesn't panic)

struct TestCfHandler;

impl foundation_http::wasm::serve_cf::CfServeFactory for TestCfHandler {
    fn create(_ctx: &Ctx) -> Self {
        TestCfHandler
    }
}

#[async_trait::async_trait(?Send)]
impl foundation_http::wasm::serve_cf::CfServe for TestCfHandler {
    async fn serve_cf(
        &self,
        _ctx: Ctx,
        _req: SimpleIncomingRequest,
        conn: &mut foundation_http::wasm::cf_conn::CfConn,
    ) -> foundation_http::wasm::cf_conn::CfConnectionResult {
        conn.set_status(200);
        conn.set_body(b"cf ok".to_vec());
        foundation_http::wasm::cf_conn::CfConnectionResult::Ok
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
    assert!(app
        .router()
        .dispatch(&SimpleMethod::GET, "/cf-all")
        .is_some());
    assert!(app
        .router()
        .dispatch(&SimpleMethod::POST, "/cf-all")
        .is_some());
    assert!(app
        .router()
        .dispatch(&SimpleMethod::DELETE, "/cf-all")
        .is_some());
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
    assert!(app
        .router()
        .dispatch(&SimpleMethod::GET, "/cf/users/123")
        .is_some());
    assert!(app
        .router()
        .dispatch(&SimpleMethod::GET, "/cf/users/abc")
        .is_some());
    assert!(app
        .router()
        .dispatch(&SimpleMethod::GET, "/cf/users")
        .is_none());
}

#[test]
fn test_http_app_cf_wildcard_route() {
    let mut app = HttpApp::<Arc<dyn foundation_http::wasm::serve_cf::CfServe>>::new_cf();
    app.route_cf::<TestCfHandler>(SimpleMethod::GET, "/cf/*");
    assert!(app
        .router()
        .dispatch(&SimpleMethod::GET, "/cf/anything")
        .is_some());
    assert!(app
        .router()
        .dispatch(&SimpleMethod::GET, "/cf/a/b/c")
        .is_some());
}

// -----------------------------------------------------------------------
// Test middleware

struct PassThroughMiddleware;
impl foundation_http::shared::middleware::RequestMiddleware for PassThroughMiddleware {
    fn handle(
        &self,
        _ctx: &Ctx,
        _req: &mut SimpleIncomingRequest,
    ) -> foundation_http::shared::middleware::MiddlewareResult {
        foundation_http::shared::middleware::MiddlewareResult::Continue
    }
}
