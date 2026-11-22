//! Shared helpers for the foundation_http wasm-bindgen cases (spec 88
//! feature 02).
//!
//! WHY: the cases live in `tests/wasm.rs` and run on the embedded Deno runtime,
//! in headless Chromium and inside workerd. Every case must build its own app,
//! build a real `web_sys::Request`, and read a real `web_sys::Response`.
//! WHAT: handler adapters, app builders, the request builder and the response
//! readers. It holds no case.
//! HOW: the wasm-bindgen bridge accepts `web_sys::Request`, so no host call is
//! stubbed (rule R1). A route is added to the plain `HttpApp` before
//! `WasmHttpApp::from_app`, because the wrapper shares the app.

use std::sync::Arc;

use foundation_core::io::readers::Data;
use foundation_db::wasm::bindgen::cf::D1Database;
use foundation_http::shared::app::HttpApp;
use foundation_http::shared::context::Ctx;
use foundation_http::shared::serve_web::WebServe;
use foundation_http::shared::web_conn::{WebConn, WebConnectionResult};
use foundation_http::wasm::bridge::cf::CfHttpApp;
use foundation_http::wasm::bridge::web::WasmHttpApp;
use foundation_http::wasm::cf_conn::{CfConn, CfConnectionResult};
use foundation_http::wasm::serve_cf::CfServe;
use foundation_http::{SendSafeBody, SimpleIncomingRequest};
use wasm_bindgen::JsCast;
use wasm_bindgen::JsValue;
use wasm_bindgen_futures::JsFuture;
use web_sys::{Request, RequestInit, Response};

/// The origin every case uses. It never leaves the case: the bridge reads the
/// URL only, it does not fetch it.
pub const ORIGIN: &str = "http://example.test";

/// The name of the host that runs the case (rule R3: a failure names its host).
///
/// A page has a `document`, Deno has a `Deno` global, and workerd has neither.
#[must_use]
pub fn host_name() -> &'static str {
    let global = js_sys::global();
    let has = |name: &str| {
        js_sys::Reflect::get(&global, &JsValue::from_str(name))
            .map(|value| !value.is_undefined())
            .unwrap_or(false)
    };
    if has("Deno") {
        "deno"
    } else if has("document") {
        "browser"
    } else {
        "workerd"
    }
}

// ── Request and response helpers ────────────────────────────────────

/// Build a real `web_sys::Request`.
///
/// `body` is `None` for a bodyless request. A body is a `Uint8Array`, so the
/// host sees a normal request body and the bridge streams it.
#[must_use]
pub fn request(method: &str, url: &str, headers: &[(&str, &str)], body: Option<&[u8]>) -> Request {
    let web_headers = web_sys::Headers::new().expect("create headers");
    for (name, value) in headers {
        web_headers
            .set(name, value)
            .expect("set a request header on built-in data");
    }

    let init = RequestInit::new();
    init.set_method(method);
    init.set_headers(&web_headers);
    if let Some(bytes) = body {
        let array: JsValue = js_sys::Uint8Array::from(bytes).into();
        init.set_body(&array);
    }
    Request::new_with_str_and_init(url, &init).expect("build the request")
}

/// The response body as text (case oracle: the handler's body round trip).
pub async fn body_text(response: &Response) -> String {
    JsFuture::from(response.text().expect("a body promise"))
        .await
        .expect("the body resolves")
        .as_string()
        .unwrap_or_default()
}

/// The first value of a response header, if any.
#[must_use]
pub fn header(response: &Response, name: &str) -> Option<String> {
    response.headers().get(name).ok().flatten()
}

/// Every value of a response header, in the order the response carries them.
///
/// A duplicate header is one entry with several values. The Fetch API exposes
/// `Set-Cookie` through `Headers.getSetCookie`; every other header joins its
/// values with `, ` in `Headers.get`. The helper reads the first form when the
/// host has it and falls back to the second.
#[must_use]
pub fn header_values(response: &Response, name: &str) -> Vec<String> {
    let headers = response.headers();
    if name.eq_ignore_ascii_case("set-cookie") {
        let getter = js_sys::Reflect::get(&headers, &JsValue::from_str("getSetCookie"))
            .ok()
            .and_then(|value| value.dyn_into::<js_sys::Function>().ok());
        if let Some(getter) = getter {
            if let Ok(result) = getter.call0(&headers) {
                let values: Vec<String> = js_sys::Array::from(&result)
                    .iter()
                    .filter_map(|item| item.as_string())
                    .collect();
                if !values.is_empty() {
                    return values;
                }
            }
        }
    }

    response
        .headers()
        .get(name)
        .ok()
        .flatten()
        .map(|value| {
            value
                .split(',')
                .map(|part| part.trim().to_string())
                .collect()
        })
        .unwrap_or_default()
}

// ── Handler adapters ────────────────────────────────────────────────

/// A `WebServe` handler from a synchronous closure.
pub struct FnHandler<F>(F);

impl<F> FnHandler<F>
where
    F: Fn(&Ctx, &SimpleIncomingRequest, &mut WebConn) + Send + Sync + 'static,
{
    /// Wrap `handler` as a shared `WebServe` handler.
    pub fn new(handler: F) -> Arc<dyn WebServe> {
        Arc::new(Self(handler))
    }
}

#[async_trait::async_trait(?Send)]
impl<F> WebServe for FnHandler<F>
where
    F: Fn(&Ctx, &SimpleIncomingRequest, &mut WebConn) + Send + Sync + 'static,
{
    async fn serve_web(
        &self,
        ctx: Ctx,
        req: SimpleIncomingRequest,
        conn: &mut WebConn,
    ) -> WebConnectionResult {
        (self.0)(&ctx, &req, conn);
        WebConnectionResult::Ok
    }
}

/// A `WebServe` handler that reads its request body as text.
///
/// It proves the bridge streams the body: the reader stops at the configured
/// bound and never holds the whole upload.
pub struct EchoBodyHandler;

/// The most host turns one body read may take before the case fails.
///
/// A streamed body yields `Data::Retry` until the host's reader task pushes a
/// chunk. The bound fails a case that never progresses instead of hanging the
/// suite (rule R4).
const BODY_READ_TURNS: usize = 1_000;

/// Yield one turn to the host event loop, so a spawned reader task can run.
async fn host_turn() {
    let promise = js_sys::Promise::new(&mut |resolve, _reject| {
        let global = js_sys::global();
        let set_timeout = js_sys::Reflect::get(&global, &JsValue::from_str("setTimeout"))
            .expect("the host has setTimeout")
            .dyn_into::<js_sys::Function>()
            .expect("setTimeout is a function");
        set_timeout
            .call2(&global, &resolve, &JsValue::from_f64(0.0))
            .expect("setTimeout queues the turn");
    });
    let _ = JsFuture::from(promise).await;
}

/// Read a request body as text.
///
/// A streamed body is consumed one chunk at a time. The read yields to the host
/// on every empty turn, so the bridge's reader task runs; a synchronous loop
/// that treats a wait as a result would spin the worker (root rule 6).
async fn read_body_text(body: SendSafeBody) -> String {
    match body {
        SendSafeBody::None => String::new(),
        SendSafeBody::Text(text) => text,
        SendSafeBody::Bytes(bytes) => String::from_utf8_lossy(&bytes).into_owned(),
        SendSafeBody::Stream(Some(mut iterator)) => {
            let mut bytes = Vec::new();
            let mut turns = 0;
            loop {
                match iterator.next() {
                    Some(Ok(Data::Bytes(chunk))) => bytes.extend_from_slice(&chunk),
                    Some(Ok(Data::Retry)) => {
                        turns += 1;
                        assert!(
                            turns < BODY_READ_TURNS,
                            "the request body made no progress after {BODY_READ_TURNS} host turns"
                        );
                        host_turn().await;
                    }
                    Some(Err(error)) => panic!("the request body failed: {error}"),
                    None => break,
                }
            }
            String::from_utf8(bytes).expect("the case sends a UTF-8 body")
        }
        SendSafeBody::Stream(None) => String::new(),
        _ => String::new(),
    }
}

#[async_trait::async_trait(?Send)]
impl WebServe for EchoBodyHandler {
    async fn serve_web(
        &self,
        _ctx: Ctx,
        req: SimpleIncomingRequest,
        conn: &mut WebConn,
    ) -> WebConnectionResult {
        let text = read_body_text(req.body.unwrap_or(SendSafeBody::None)).await;
        conn.set_status(200);
        conn.set_header("content-type", "text/plain");
        conn.set_header("x-body-len", &text.len().to_string());
        conn.set_body(format!("{}:{text}", text.len()).into_bytes());
        WebConnectionResult::Ok
    }
}

/// A `CfServe` handler from a synchronous closure.
pub struct CfFnHandler<F>(F);

impl<F> CfFnHandler<F>
where
    F: Fn(&Ctx, &SimpleIncomingRequest, &mut CfConn) + Send + Sync + 'static,
{
    /// Wrap `handler` as a shared `CfServe` handler.
    pub fn new(handler: F) -> Arc<dyn CfServe> {
        Arc::new(Self(handler))
    }
}

#[async_trait::async_trait(?Send)]
impl<F> CfServe for CfFnHandler<F>
where
    F: Fn(&Ctx, &SimpleIncomingRequest, &mut CfConn) + Send + Sync + 'static,
{
    async fn serve_cf(
        &self,
        ctx: Ctx,
        req: SimpleIncomingRequest,
        conn: &mut CfConn,
    ) -> CfConnectionResult {
        (self.0)(&ctx, &req, conn);
        CfConnectionResult::Ok
    }
}

// ── App builders ────────────────────────────────────────────────────

/// Build a `WasmHttpApp` from a fresh host-context app.
///
/// The closure adds routes and middleware before the wrap, because
/// `WasmHttpApp::app()` returns a shared reference.
pub fn web_app_with(build: impl FnOnce(&mut HttpApp<Arc<dyn WebServe>>)) -> WasmHttpApp {
    let mut app = HttpApp::<Arc<dyn WebServe>>::new_web();
    build(&mut app);
    WasmHttpApp::from_app(app)
}

/// Build a `WasmHttpApp` from an app with an explicit (deterministic) context.
pub fn web_app_with_ctx(
    ctx: Ctx,
    build: impl FnOnce(&mut HttpApp<Arc<dyn WebServe>>),
) -> WasmHttpApp {
    let mut app = HttpApp::<Arc<dyn WebServe>>::with_ctx(ctx);
    build(&mut app);
    WasmHttpApp::from_app(app)
}

/// Build a `CfHttpApp` from a fresh host-context app.
pub fn cf_app_with(build: impl FnOnce(&mut HttpApp<Arc<dyn CfServe>>)) -> CfHttpApp {
    let mut app = HttpApp::<Arc<dyn CfServe>>::new_cf();
    build(&mut app);
    CfHttpApp::from_app(app)
}

/// Read the D1 binding from a request context, or `None`.
///
/// The CF bridge stores `env.DB` as a [`D1Database`] in the forked request
/// context when the binding exists.
#[must_use]
pub fn request_db(ctx: &Ctx) -> Option<Arc<D1Database>> {
    ctx.get::<D1Database>()
}

/// The real worker `env` object, set by the testbed's `bindgen-wrangler`
/// worker as `globalThis.__CF_TEST_ENV__`.
///
/// `None` on every host that is not workerd under the testbed. The one case
/// that needs a real binding skips there and prints why (invariant I5).
#[must_use]
pub fn test_env() -> Option<JsValue> {
    js_sys::Reflect::get(&js_sys::global(), &JsValue::from_str("__CF_TEST_ENV__"))
        .ok()
        .filter(|value| !value.is_undefined())
}
