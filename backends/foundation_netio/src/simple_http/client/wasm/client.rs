//! Fetch-based HTTP client for wasm32 (browser + CF Workers).
//!
//! WHY: AI providers and other HTTP consumers need to work on wasm32.
//! The native `SimpleHttpClient` uses TCP sockets — unavailable in
//! browsers and CF Workers. The `fetch()` Web API is the standard
//! HTTP primitive on these platforms.
//!
//! WHAT: `FetchHttpClient` implements the `HttpClient` trait using
//! the JS `fetch()` API. One implementation covers both browser
//! (`window.fetch`) and CF Workers (`global.fetch`) via runtime
//! detection of `ServiceWorkerGlobalScope`.
//!
//! HOW: Converts `PreparedRequest` → `web_sys::Request`, calls
//! `fetch()`, converts the `Response` back to `SimpleResponse`.
//! SSE responses use `WasmSseIterator` from the `stream` module.
//! Streaming request bodies are bridged to JS `ReadableStream` via
//! valtron's `iterator_to_readable_stream`.
//! Async methods are canonical; sync methods wrap via `valtron::run_future()`.

use std::sync::Arc;

use foundation_compact::SendWrapper;
use foundation_core::io::readers::Data;
use foundation_core::valtron::js_stream;
use foundation_core::valtron::run_future;
use wasm_bindgen::prelude::*;
use wasm_bindgen_futures::JsFuture;
use web_sys::{Request, RequestInit, Response};

use crate::simple_http::client::shared::http_client::{BoxedSseIterator, HttpClient};
use crate::simple_http::client::shared::request::PreparedRequest;
use crate::simple_http::shared::{
    HttpClientError, LineFeed, SendSafeBody, SimpleMethod, SimpleResponse, Status,
};

use super::headers::{simple_headers_to_web_sys, web_sys_headers_to_simple};
use super::stream::WasmSseIterator;

/// Fetch-based HTTP client for wasm32 (browser + Cloudflare Workers).
///
/// Stateless — each call builds a fresh `web_sys::Request` and calls
/// `fetch()`. Runtime detection selects `window.fetch` (browser) or
/// `ServiceWorkerGlobalScope.fetch` (CF Workers).
pub struct FetchHttpClient;

// SAFETY: wasm32 is single-threaded; FetchHttpClient has no state.
unsafe impl Send for FetchHttpClient {}
unsafe impl Sync for FetchHttpClient {}

#[async_trait::async_trait]
impl HttpClient for FetchHttpClient {
    async fn send_async(
        &self,
        req: PreparedRequest,
    ) -> Result<SimpleResponse<SendSafeBody>, HttpClientError> {
        SendWrapper::new(async move {
            let ws_req = build_web_request(req)?;
            let resp = do_fetch(&ws_req).await?;

            let status = Status::from(resp.status().to_string());
            let headers = web_sys_headers_to_simple(&resp.headers());

            let text_promise = resp
                .text()
                .map_err(|e| HttpClientError::Reason(format!("body read failed: {e:?}")))?;
            let body_js = JsFuture::from(text_promise)
                .await
                .map_err(|e| HttpClientError::Reason(format!("body await failed: {e:?}")))?;
            let body_text = body_js.as_string().unwrap_or_default();
            let body = if body_text.is_empty() {
                SendSafeBody::None
            } else {
                SendSafeBody::Text(body_text)
            };

            Ok(SimpleResponse::new(status, headers, body))
        })
        .await
    }

    async fn send_sse_async(
        &self,
        req: PreparedRequest,
    ) -> Result<BoxedSseIterator, HttpClientError> {
        SendWrapper::new(async move {
            let ws_req = build_web_request(req)?;
            let resp = do_fetch(&ws_req).await?;

            let status_code = resp.status();
            if !(200..=299).contains(&status_code) {
                return Err(HttpClientError::Reason(format!(
                    "SSE request failed with status {status_code}"
                )));
            }

            let body = resp
                .body()
                .ok_or_else(|| HttpClientError::Reason("SSE response has no body".into()))?;

            let iter = WasmSseIterator::new(body);
            Ok(Box::new(iter) as BoxedSseIterator)
        })
        .await
    }

    fn send(
        &self,
        req: PreparedRequest,
    ) -> Result<SimpleResponse<SendSafeBody>, HttpClientError> {
        let fut = FetchHttpClient.send_async(req);
        let results = run_future(fut)
            .map_err(|e| HttpClientError::Reason(format!("valtron executor error: {e}")))?;
        results
            .into_iter()
            .next()
            .unwrap_or(Err(HttpClientError::Reason(
                "run_future returned no result".into(),
            )))
    }

    fn send_sse(&self, req: PreparedRequest) -> Result<BoxedSseIterator, HttpClientError> {
        let fut = FetchHttpClient.send_sse_async(req);
        let results = run_future(fut)
            .map_err(|e| HttpClientError::Reason(format!("valtron executor error: {e}")))?;
        results
            .into_iter()
            .next()
            .unwrap_or(Err(HttpClientError::Reason(
                "run_future returned no result".into(),
            )))
    }
}

/// Convert a `PreparedRequest` into a `web_sys::Request` for `fetch()`.
///
/// Handles all `SendSafeBody` variants:
/// - `None` → no body
/// - `Text` → JS string
/// - `Bytes` → `Uint8Array`
/// - `Stream` / `ChunkedStream` / `LineFeedStream` / `SseStream` →
///   JS `ReadableStream` via valtron's `iterator_to_readable_stream`
fn build_web_request(req: PreparedRequest) -> Result<Request, HttpClientError> {
    let init = RequestInit::new();
    init.set_method(method_str(&req.method));

    let body_js = send_safe_body_to_js(req.body)?;
    if !body_js.is_undefined() {
        init.set_body(&body_js);
    }

    let ws_headers = simple_headers_to_web_sys(&req.headers)
        .map_err(|e| HttpClientError::Reason(format!("headers conversion failed: {e:?}")))?;
    init.set_headers(&ws_headers.into());

    let url_str = req.url.to_string();
    Request::new_with_str_and_init(&url_str, &init)
        .map_err(|e| HttpClientError::Reason(format!("request creation failed: {e:?}")))
}

/// Convert a `SendSafeBody` into a `JsValue` suitable for `RequestInit.body`.
///
/// Returns `JsValue::UNDEFINED` for `None` (no body set on the request).
fn send_safe_body_to_js(body: SendSafeBody) -> Result<JsValue, HttpClientError> {
    match body {
        SendSafeBody::None => Ok(JsValue::UNDEFINED),
        SendSafeBody::Text(text) => Ok(JsValue::from_str(&text)),
        SendSafeBody::Bytes(bytes) => {
            let array = js_sys::Uint8Array::from(bytes.as_slice());
            Ok(array.into())
        }
        SendSafeBody::Stream(opt_iter) => {
            let iter = opt_iter.ok_or_else(|| {
                HttpClientError::Reason("stream body already consumed".into())
            })?;
            let byte_iter = Box::new(iter.filter_map(|result| match result {
                Ok(Data::Bytes(bytes)) => Some(bytes),
                Ok(Data::Retry) | Err(_) => None,
            }));
            let stream = js_stream::iterator_to_readable_stream(byte_iter)
                .map_err(|e| HttpClientError::Reason(format!("ReadableStream creation failed: {e:?}")))?;
            Ok(stream.into())
        }
        SendSafeBody::ChunkedStream(opt_iter) => {
            let iter = opt_iter.ok_or_else(|| {
                HttpClientError::Reason("chunked stream body already consumed".into())
            })?;
            let byte_iter = Box::new(iter.filter_map(|result| match result {
                Ok(mut chunk) => {
                    let bytes = chunk.into_bytes();
                    if bytes.is_empty() { None } else { Some(bytes) }
                }
                Err(_) => None,
            }));
            let stream = js_stream::iterator_to_readable_stream(byte_iter)
                .map_err(|e| HttpClientError::Reason(format!("ReadableStream creation failed: {e:?}")))?;
            Ok(stream.into())
        }
        SendSafeBody::LineFeedStream(opt_iter) => {
            let iter = opt_iter.ok_or_else(|| {
                HttpClientError::Reason("line feed stream body already consumed".into())
            })?;
            let byte_iter = Box::new(iter.filter_map(|result| match result {
                Ok(LineFeed::Line(line)) => Some(format!("{line}\n").into_bytes()),
                Ok(LineFeed::END) | Ok(LineFeed::SKIP) | Err(_) => None,
            }));
            let stream = js_stream::iterator_to_readable_stream(byte_iter)
                .map_err(|e| HttpClientError::Reason(format!("ReadableStream creation failed: {e:?}")))?;
            Ok(stream.into())
        }
        SendSafeBody::SseStream(opt_iter) => {
            let iter = opt_iter.ok_or_else(|| {
                HttpClientError::Reason("SSE stream body already consumed".into())
            })?;
            let byte_iter = Box::new(iter.filter_map(|result| match result {
                Ok(parse_result) => {
                    use crate::event_source::Event;
                    let mut buf = String::new();
                    match &parse_result.event {
                        Event::Message { id, event_type, data, retry } => {
                            if let Some(id) = id {
                                buf.push_str(&format!("id: {id}\n"));
                            }
                            if let Some(et) = event_type {
                                buf.push_str(&format!("event: {et}\n"));
                            }
                            for line in data.lines() {
                                buf.push_str(&format!("data: {line}\n"));
                            }
                            if let Some(r) = retry {
                                buf.push_str(&format!("retry: {r}\n"));
                            }
                            buf.push('\n');
                        }
                        Event::Comment(text) => {
                            buf.push_str(&format!(": {text}\n\n"));
                        }
                        Event::Reconnect => {}
                    }
                    if buf.is_empty() { None } else { Some(buf.into_bytes()) }
                }
                Err(_) => None,
            }));
            let stream = js_stream::iterator_to_readable_stream(byte_iter)
                .map_err(|e| HttpClientError::Reason(format!("ReadableStream creation failed: {e:?}")))?;
            Ok(stream.into())
        }
    }
}

/// Call `fetch()`, detecting service worker (CF Workers) vs browser context.
async fn do_fetch(req: &Request) -> Result<Response, HttpClientError> {
    let promise = js_fetch(req);
    let js_value = JsFuture::from(promise)
        .await
        .map_err(|e| HttpClientError::Reason(format!("fetch failed: {e:?}")))?;
    js_value
        .dyn_into::<Response>()
        .map_err(|_| HttpClientError::Reason("fetch returned non-Response".into()))
}

/// Runtime detection: CF Workers have `ServiceWorkerGlobalScope`, browsers
/// have `window`. Call `fetch()` on whichever is available.
fn js_fetch(req: &Request) -> js_sys::Promise {
    let global = js_sys::global();

    if let Ok(true) =
        js_sys::Reflect::has(&global, &JsValue::from_str("ServiceWorkerGlobalScope"))
    {
        global
            .unchecked_into::<web_sys::ServiceWorkerGlobalScope>()
            .fetch_with_request(req)
    } else {
        web_sys::window()
            .expect("fetch: no window and no service worker global scope")
            .fetch_with_request(req)
    }
}

fn method_str(method: &SimpleMethod) -> &'static str {
    match method {
        SimpleMethod::GET => "GET",
        SimpleMethod::POST => "POST",
        SimpleMethod::PUT => "PUT",
        SimpleMethod::DELETE => "DELETE",
        SimpleMethod::PATCH => "PATCH",
        SimpleMethod::HEAD => "HEAD",
        SimpleMethod::OPTIONS => "OPTIONS",
        _ => "GET",
    }
}

/// Create a wasm `HttpClient` with default settings.
#[must_use]
pub fn default_http_client() -> Arc<dyn HttpClient> {
    Arc::new(FetchHttpClient)
}
