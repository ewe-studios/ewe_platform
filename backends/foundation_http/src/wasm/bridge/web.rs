//! Web-standard wasm-bindgen bridge: `web_sys::Request` → `SimpleIncomingRequest` → dispatch → `web_sys::Response`.
//!
//! # Usage
//! ```js
//! import { WasmHttpApp } from 'foundation_http';
//! const app = new WasmHttpApp();
//! // configure routes in Rust...
//! export default {
//!   async fetch(req) { return await app.handleRequest(req); }
//! };
//! ```

use std::sync::Arc;

use foundation_core::wire::simple_http::{
    Proto, SendSafeBody, SimpleHeader, SimpleHeaders, SimpleIncomingRequest, SimpleMethod,
};
use wasm_bindgen::prelude::*;
use wasm_bindgen_futures::JsFuture;
use web_sys::{Request, Response, ResponseInit};

use crate::shared::app::HttpApp;
use crate::wasm::server::handle_request;

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

/// Build a `web_sys::Response` from raw HTTP response bytes.
fn response_from_bytes(bytes: Vec<u8>) -> Result<Response, JsError> {
    let mut status_code: u16 = 200;
    let mut header_entries: Vec<(String, String)> = Vec::new();

    if let Ok(response_str) = String::from_utf8(bytes.clone()) {
        let mut lines = response_str.split("\r\n");
        if let Some(status_line) = lines.next() {
            status_code = status_line
                .split_whitespace()
                .nth(1)
                .and_then(|s| s.parse().ok())
                .unwrap_or(200);
        }
        for line in lines {
            if line.is_empty() {
                break;
            }
            if let Some((key, val)) = line.split_once(": ") {
                header_entries.push((key.to_string(), val.to_string()));
            }
        }
    }

    let body = bytes
        .windows(4)
        .position(|w| w == b"\r\n\r\n")
        .map(|pos| &bytes[pos + 4..])
        .unwrap_or(&[]);

    let init = ResponseInit::new();
    init.set_status(status_code);

    let web_headers = web_sys::Headers::new().map_err(|e| {
        JsError::new(&format!("failed to create headers: {e:?}"))
    })?;
    for (k, v) in header_entries {
        let _ = web_headers.append(&k, &v);
    }
    init.set_headers(&web_headers);

    let mut body_owned = body.to_vec();
    Response::new_with_opt_u8_array_and_init(Some(&mut body_owned), &init).map_err(|e| {
        JsError::new(&format!("failed to create response: {e:?}"))
    })
}

/// JS-compatible wrapper around `HttpApp`.
#[wasm_bindgen]
pub struct WasmHttpApp {
    inner: Arc<HttpApp>,
}

#[wasm_bindgen]
impl WasmHttpApp {
    /// Create a new empty `WasmHttpApp`.
    #[wasm_bindgen(constructor)]
    pub fn new() -> Self {
        Self {
            inner: Arc::new(HttpApp::new()),
        }
    }

    /// Dispatch a `web_sys::Request` through the app and return a `web_sys::Response`.
    #[wasm_bindgen(js_name = handleRequest)]
    pub async fn handle_request(&self, req: Request) -> Result<Response, JsError> {
        let simple_req = request_from_web(&req).await?;
        let result = handle_request(&self.inner, simple_req)?;
        response_from_bytes(result)
    }
}

impl WasmHttpApp {
    /// Get the inner `HttpApp` reference for Rust-side route registration.
    pub fn app(&self) -> &HttpApp {
        &self.inner
    }
}

impl Default for WasmHttpApp {
    fn default() -> Self {
        Self::new()
    }
}
