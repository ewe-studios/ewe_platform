//! The wasm exchange uses the client it was called on.
//!
//! WHY: `open_exchange` used to build a fresh default fetch client, so a caller
//! that held a `NetClient` of its own was silently bypassed (issue #559, spec 85
//! rules 3 and 4).
//!
//! WHAT: a recording `HttpClient` whose `open_exchange` passes itself to the
//! exchange task. A request driven through that task must reach the recording
//! client.
//!
//! HOW: drive `next_status` directly and yield to the JS event loop between
//! polls. No network call happens.

use std::sync::{Arc, Mutex};

use foundation_context::Ctx;
use foundation_core::url::Uri;
use foundation_core::valtron::{TaskIterator, TaskStatus};
use foundation_netio::shared::client::http_client::{BoxedSseFutureStream, BoxedSseIterator};
use foundation_netio::shared::client::request_task::{
    HttpExchange, HttpExchangeClientTask,
};
use foundation_netio::shared::client::{HttpClient, PreparedRequest};
use foundation_netio::shared::http::{
    Extensions, HttpClientError, SendSafeBody, SimpleHeaders, SimpleMethod, SimpleResponse, Status,
};
use foundation_netio::wasm::client::tasks::WasmHttpExchangeTask;
use foundation_testbed::bindgen::wasm_bindgen::JsValue;
use foundation_testbed::bindgen::{js_sys, wasm_bindgen_futures};
use wasm_bindgen_test::wasm_bindgen_test;

async fn yield_now() {
    let promise = js_sys::Promise::resolve(&JsValue::UNDEFINED);
    let _ = wasm_bindgen_futures::JsFuture::from(promise).await;
}

/// A client that records each request URL it sends.
#[derive(Clone, Default)]
struct RecordingClient {
    seen: Arc<Mutex<Vec<String>>>,
}

impl RecordingClient {
    fn urls(&self) -> Vec<String> {
        self.seen.lock().expect("lock").clone()
    }
}

#[async_trait::async_trait]
impl HttpClient for RecordingClient {
    async fn send_async(
        &self,
        req: PreparedRequest,
    ) -> Result<SimpleResponse<SendSafeBody>, HttpClientError> {
        self.seen
            .lock()
            .expect("lock")
            .push(req.url.to_string());
        Ok(SimpleResponse::new(
            Status::OK,
            SimpleHeaders::new(),
            SendSafeBody::Bytes(b"ok".to_vec()),
        ))
    }

    async fn send_sse_async(
        &self,
        _req: PreparedRequest,
    ) -> Result<BoxedSseFutureStream, HttpClientError> {
        Err(HttpClientError::Unsupported)
    }

    fn send(&self, _req: PreparedRequest) -> Result<SimpleResponse<SendSafeBody>, HttpClientError> {
        Err(HttpClientError::Unsupported)
    }

    fn send_sse(&self, _req: PreparedRequest) -> Result<BoxedSseIterator, HttpClientError> {
        Err(HttpClientError::Unsupported)
    }

    fn open_exchange(&self, req: PreparedRequest) -> HttpExchangeClientTask {
        Box::new(WasmHttpExchangeTask::new(Arc::new(self.clone()), req))
    }
}

#[wasm_bindgen_test]
async fn custom_client_is_used() {
    let client = RecordingClient::default();
    let url = "http://recording.invalid/call";
    let request = PreparedRequest {
        method: SimpleMethod::POST,
        url: Uri::parse(url).expect("uri"),
        headers: SimpleHeaders::new(),
        body: SendSafeBody::Bytes(b"frame".to_vec()),
        extensions: Extensions::new(),
        body_readiness: None,
    };

    let mut task = client.open_exchange(request);
    let mut saw_head = false;
    for _ in 0..200 {
        match task.next_status(Ctx::host()) {
            Some(TaskStatus::Ready(HttpExchange::Head { .. })) => {
                saw_head = true;
                break;
            }
            Some(TaskStatus::Ready(HttpExchange::Failed(error))) => {
                panic!("exchange failed: {error}")
            }
            Some(_) | None => yield_now().await,
        }
    }

    assert!(saw_head, "the exchange produced a response head");
    assert_eq!(
        client.urls(),
        vec![url.to_string()],
        "the exchange sent the request through the recording client"
    );
}
