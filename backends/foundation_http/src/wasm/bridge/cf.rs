//! Cloudflare Workers wasm-bindgen bridge: `Request` + `env` → dispatch → `Response`.

use std::sync::Arc;

use foundation_netio::simple_http::{
    Proto, SendSafeBody, SimpleHeader, SimpleHeaders, SimpleIncomingRequest, SimpleMethod,
};
use js_sys;
use wasm_bindgen::prelude::*;
use wasm_bindgen_futures::JsFuture;
use web_sys::{Request, Response};

use crate::shared::app::HttpApp;
use crate::shared::context::ContextBag;
use crate::wasm::dispatch::HttpAppCfDispatch;
use crate::wasm::serve_cf::CfServe;
use foundation_db::wasm::bindgen::cf::D1Database;

/// Convert a `Request` to a `SimpleIncomingRequest`.
async fn request_from_cf(req: &Request) -> Result<SimpleIncomingRequest, JsError> {
    let method = SimpleMethod::from(req.method());
    let url = req.url();

    let headers = req.headers();
    let entries = js_sys::Array::from(&headers.entries());
    let mut simple_headers = SimpleHeaders::new();
    for entry in entries.iter() {
        let pair = js_sys::Array::from(&entry);
        if let (Some(key), Some(val)) = (pair.get(0).as_string(), pair.get(1).as_string()) {
            let header = SimpleHeader::from(key.clone());
            simple_headers.entry(header).or_default().push(val);
        }
    }

    let text_promise = req.text().map_err(|e| {
        JsError::new(&format!("failed to get request text: {e:?}"))
    })?;
    let body_js = JsFuture::from(text_promise).await.map_err(|e| {
        JsError::new(&format!("failed to read request body: {e:?}"))
    })?;
    let body_text = body_js.as_string().unwrap_or_default();
    let body = if body_text.is_empty() {
        None
    } else {
        Some(SendSafeBody::Text(body_text))
    };

    let simple_req = SimpleIncomingRequest::builder()
        .with_parsed_url(url)
        .with_method(method)
        .with_proto(Proto::HTTP11)
        .with_headers(simple_headers)
        .with_some_body(body)
        .build()
        .map_err(|e| JsError::new(&format!("failed to build request: {e}")))?;

    Ok(simple_req)
}

/// Extract typed bindings from the CF env object and store them in the context bag.
fn extract_cf_bindings(env: &JsValue, bag: &ContextBag) {
    if let Ok(db) = D1Database::from_env(env, "DB") {
        bag.store(db);
    }
}

/// Cloudflare Workers `CfHttpApp` wrapper.
#[wasm_bindgen]
pub struct CfHttpApp {
    inner: Arc<HttpApp<Arc<dyn CfServe>>>,
}

#[wasm_bindgen]
impl CfHttpApp {
    /// Create a new empty `CfHttpApp`.
    #[wasm_bindgen(constructor)]
    pub fn new() -> Self {
        Self {
            inner: Arc::new(HttpApp::new_cf()),
        }
    }

    /// Dispatch a `Request` with CF `env` bindings through the app.
    #[wasm_bindgen(js_name = handleRequest)]
    pub async fn handle_request(
        &self,
        req: Request,
        env: JsValue,
    ) -> Result<Response, JsError> {
        let simple_req = request_from_cf(&req).await?;
        let bag = Arc::new(ContextBag::new());
        extract_cf_bindings(&env, &bag);
        Ok(self.inner.dispatch_cf(bag, simple_req).map_err(|e| {
            JsError::new(&format!("dispatch failed: {e:?}"))
        })?)
    }
}

impl CfHttpApp {
    /// Create a `CfHttpApp` from an existing `HttpApp<Arc<dyn CfServe>>`.
    pub fn from_app(app: HttpApp<Arc<dyn CfServe>>) -> Self {
        Self {
            inner: Arc::new(app),
        }
    }

    /// Dispatch a `SimpleIncomingRequest` through the app with the given context bag.
    pub fn dispatch(
        &self,
        bag: Arc<ContextBag>,
        req: SimpleIncomingRequest,
    ) -> Result<Response, foundation_errstacks::ErrorTrace<crate::shared::serve::ServeError>> {
        self.inner.dispatch_cf(bag, req)
    }

    /// Get the inner `HttpApp` reference for Rust-side route registration.
    pub fn app(&self) -> &HttpApp<Arc<dyn CfServe>> {
        &self.inner
    }
}

impl Default for CfHttpApp {
    fn default() -> Self {
        Self::new()
    }
}
