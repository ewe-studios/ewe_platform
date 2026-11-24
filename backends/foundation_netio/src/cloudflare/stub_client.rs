//! A [`NetClient`](crate::network_client::NetClient) over a Cloudflare Durable
//! Object stub.
//!
//! WHY: a Worker calls a Durable Object through a stub. The call must use the
//! same `HttpClient` abstraction as every other call, so `H1Transport`,
//! `DynNetClient` and the response streams work unchanged.
//!
//! WHAT: [`StubHttpClient`] sends each request through one `worker::Stub`. It
//! implements `HttpClient` and `WebSocketConnector`.
//!
//! HOW: the request becomes a JS `Request`; `Stub::fetch_with_request` sends
//! it; the response body becomes a `WasmByteStream`. It lives behind the
//! optional `cloudflare` feature, so a browser build never sees `worker`.
//!
//! The forwarded identity headers stay with the caller: this type adds no
//! header. A host adds its own header on the request it builds.

use std::sync::Arc;

use foundation_compact::SendWrapper;
use wasm_bindgen::JsValue;

use crate::shared::client::http_client::{BoxedSseFutureStream, BoxedSseIterator, HttpClient};
use crate::shared::client::request::PreparedRequest;
use crate::shared::client::request_task::HttpExchangeClientTask;
use crate::shared::client::ClientConfig;
use crate::shared::http::{HttpClientError, SendSafeBody, SimpleResponse};
use crate::wasm::client::client::{build_web_request, response_from_web};
use crate::wasm::client::tasks::WasmHttpExchangeTask;
use crate::websocket::shared::client::{MessageDelivery, WebSocketClient, WebSocketConnectConfig};
use crate::websocket::shared::connector::{WebSocketConnector, WsExchangeTask};
use crate::websocket::shared::error::WebSocketError;

/// A `NetClient` that sends each request through one Durable Object stub.
#[derive(Clone)]
pub struct StubHttpClient {
    stub: Arc<worker::Stub>,
    /// The body read-ahead bound and byte limit, from the client config.
    body_queue_chunks: usize,
    max_body_size: Option<u64>,
}

impl StubHttpClient {
    /// The stub for the Durable Object with this name.
    ///
    /// # Errors
    ///
    /// The error that `worker::Env::durable_object` gives, as a `JsValue`.
    pub fn new(env: &worker::Env, binding: &str, name: &str) -> Result<Self, JsValue> {
        Self::new_with_config(env, binding, name, &ClientConfig::default())
    }

    /// The stub for the Durable Object with this name, with `config` limits.
    ///
    /// # Errors
    ///
    /// The error that `worker::Env::durable_object` gives, as a `JsValue`.
    pub fn new_with_config(
        env: &worker::Env,
        binding: &str,
        name: &str,
        config: &ClientConfig,
    ) -> Result<Self, JsValue> {
        let namespace = env
            .durable_object(binding)
            .map_err(|e| JsValue::from_str(&e.to_string()))?;
        let id = namespace
            .id_from_name(name)
            .map_err(|e| JsValue::from_str(&e.to_string()))?;
        let stub = id
            .get_stub()
            .map_err(|e| JsValue::from_str(&e.to_string()))?;
        Ok(Self::from_stub_with_config(stub, config))
    }

    /// Wrap a stub that the caller already resolved, such as one from
    /// `DurableObjectId::get_stub()`.
    #[must_use]
    pub fn from_stub(stub: worker::Stub) -> Self {
        Self::from_stub_with_config(stub, &ClientConfig::default())
    }

    /// Wrap a resolved stub with `config` limits.
    #[must_use]
    pub fn from_stub_with_config(stub: worker::Stub, config: &ClientConfig) -> Self {
        Self {
            stub: Arc::new(stub),
            body_queue_chunks: config.body_queue_chunks,
            max_body_size: config.max_body_size,
        }
    }

    /// The address of the `fetch` call in the stub's host runtime.
    fn send(
        &self,
        req: PreparedRequest,
    ) -> impl std::future::Future<Output = Result<SimpleResponse<SendSafeBody>, HttpClientError>>
    {
        let stub = Arc::clone(&self.stub);
        let body_queue_chunks = self.body_queue_chunks;
        let max_body_size = crate::wasm::client::stream::usize_limit(self.max_body_size);
        async move {
            let ws_req = build_web_request(req, 0)?;
            let worker_req: worker::Request = ws_req.into();
            let response = stub
                .fetch_with_request(worker_req)
                .await
                .map_err(|e| HttpClientError::Reason(format!("DO stub fetch failed: {e}")))?;
            let response: web_sys::Response = response.into();
            response_from_web(&response, body_queue_chunks, max_body_size)
        }
    }
}

#[async_trait::async_trait]
impl HttpClient for StubHttpClient {
    async fn send_async(
        &self,
        req: PreparedRequest,
    ) -> Result<SimpleResponse<SendSafeBody>, HttpClientError> {
        SendWrapper::new(self.send(req)).await
    }

    async fn send_sse_async(
        &self,
        _req: PreparedRequest,
    ) -> Result<BoxedSseFutureStream, HttpClientError> {
        Err(HttpClientError::Reason(
            "server-sent events over a Durable Object stub are unsupported".into(),
        ))
    }

    fn send(&self, _req: PreparedRequest) -> Result<SimpleResponse<SendSafeBody>, HttpClientError> {
        Err(HttpClientError::Reason(
            "synchronous send over a Durable Object stub is unsupported; use send_async".into(),
        ))
    }

    fn send_sse(&self, _req: PreparedRequest) -> Result<BoxedSseIterator, HttpClientError> {
        Err(HttpClientError::Reason(
            "server-sent events over a Durable Object stub are unsupported".into(),
        ))
    }

    fn open_exchange(&self, req: PreparedRequest) -> HttpExchangeClientTask {
        Box::new(WasmHttpExchangeTask::new(Arc::new(self.clone()), req))
    }
}

impl WebSocketConnector for StubHttpClient {
    fn open_websocket(
        &self,
        _url: &str,
        _config: WebSocketConnectConfig,
    ) -> Result<WebSocketClient, WebSocketError> {
        Err(WebSocketError::ProtocolError(
            "a Durable Object stub has no WebSocket here".into(),
        ))
    }

    fn open_websocket_task(
        &self,
        _url: &str,
        _config: WebSocketConnectConfig,
    ) -> Result<(WsExchangeTask, MessageDelivery), WebSocketError> {
        Err(WebSocketError::ProtocolError(
            "a Durable Object stub has no WebSocket here".into(),
        ))
    }
}
