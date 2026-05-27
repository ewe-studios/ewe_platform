//! Web-standard wasm-bindgen bridge: `web_sys::Request` → `SimpleIncomingRequest` → dispatch → `web_sys::Response`.

use std::sync::Arc;

use foundation_netio::simple_http::shared::{
    Proto, SendSafeBody, SimpleHeader, SimpleHeaders, SimpleIncomingRequest, SimpleMethod,
};
use wasm_bindgen::prelude::*;
use wasm_bindgen_futures::JsFuture;
use web_sys::{Request, Response};

use crate::shared::app::HttpApp;
use crate::wasm::dispatch::HttpAppWebDispatch;
use crate::wasm::serve_web::WebServe;

/// Convert a `web_sys::Request` to a `SimpleIncomingRequest`.
async fn request_from_web(req: &Request) -> Result<SimpleIncomingRequest, JsError> {
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

/// JS-compatible wrapper around `HttpApp`.
#[wasm_bindgen]
pub struct WasmHttpApp {
    inner: Arc<HttpApp<Arc<dyn WebServe>>>,
}

#[wasm_bindgen]
impl WasmHttpApp {
    /// Create a new empty `WasmHttpApp`.
    #[wasm_bindgen(constructor)]
    pub fn new() -> Self {
        Self {
            inner: Arc::new(HttpApp::new_web()),
        }
    }

    /// Dispatch a `web_sys::Request` through the app and return a `web_sys::Response`.
    #[wasm_bindgen(js_name = handleRequest)]
    pub async fn handle_request(&self, req: Request) -> Result<Response, JsError> {
        let simple_req = request_from_web(&req).await?;
        let bag = Arc::new(crate::shared::context::ContextBag::new());
        Ok(self.inner.dispatch_web(bag, simple_req).map_err(|e| {
            JsError::new(&format!("dispatch failed: {e:?}"))
        })?)
    }
}

impl WasmHttpApp {
    /// Get the inner `HttpApp` reference for Rust-side route registration.
    pub fn app(&self) -> &HttpApp<Arc<dyn WebServe>> {
        &self.inner
    }
}

impl Default for WasmHttpApp {
    fn default() -> Self {
        Self::new()
    }
}
