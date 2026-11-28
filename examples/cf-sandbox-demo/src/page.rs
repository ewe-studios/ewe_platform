//! The page of the demo, served from `foundation_http::wasm`.

use std::sync::{Arc, OnceLock};

use foundation_context::Ctx;
use foundation_http::shared::app::HttpApp;
use foundation_http::wasm::bridge::cf::CfHttpApp;
use foundation_http::wasm::cf_conn::{CfConn, CfConnectionResult};
use foundation_http::wasm::serve_cf::{CfServe, CfServeFactory};
use foundation_netio::shared::http::SimpleIncomingRequest;
use worker::{Env, Request, Response};

/// The one page of the demo.
const INDEX_HTML: &str = include_str!("../static/index.html");

static APP: OnceLock<CfHttpApp> = OnceLock::new();

/// The page response for one request.
///
/// # Errors
/// A `worker` error when the page app cannot build the response.
pub async fn serve(request: Request, env: Env, ctx: Ctx) -> worker::Result<Response> {
    let app = APP.get_or_init(|| build(ctx));
    let page = app
        .handle_request(
            request.inner().clone(),
            worker::wasm_bindgen::JsValue::from(env),
        )
        .await
        .map_err(|error| worker::Error::RustError(format!("the page: {error:?}")))?;
    Ok(Response::from(page))
}

/// The `foundation_http` app with the page route.
fn build(ctx: Ctx) -> CfHttpApp {
    let mut app = HttpApp::<Arc<dyn CfServe>>::with_ctx(ctx);
    app.route_any_cf::<Page>("/");
    CfHttpApp::from_app(app)
}

/// The static page.
struct Page;

impl CfServeFactory for Page {
    fn create(_ctx: &Ctx) -> Self {
        Self
    }
}

#[async_trait::async_trait(?Send)]
impl CfServe for Page {
    async fn serve_cf(
        &self,
        _ctx: Ctx,
        _request: SimpleIncomingRequest,
        conn: &mut CfConn,
    ) -> CfConnectionResult {
        conn.set_status(200);
        conn.set_header("content-type", "text/html; charset=utf-8");
        conn.set_body(INDEX_HTML.as_bytes().to_vec());
        CfConnectionResult::Ok
    }
}
