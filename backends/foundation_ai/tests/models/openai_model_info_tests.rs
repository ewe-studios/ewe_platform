//! `OpenAIProvider::fetch_model_info`: the non-blocking `GET models/{name}`
//! lookup. On wasm `get_model` hands out a placeholder and runs this lookup in
//! the background; these tests pin the pieces that path relies on, with a mock
//! `HttpClient` and no network:
//! - the async lookup parses the vendor's model object,
//! - a model handed out *before* the lookup completes reports the result
//!   through `OpenAIModel::info` (shared cache),
//! - a non-2xx answer is an error and leaves the cached entry alone.

use std::sync::Arc;

use async_trait::async_trait;
use foundation_ai::backends::openai_provider::OpenAIProvider;
use foundation_ai::types::{ModelId, ModelProvider};
use foundation_netio::shared::client::http_client::{
    BoxedSseFutureStream, BoxedSseIterator, HttpClient,
};
use foundation_netio::shared::client::request::PreparedRequest;
use foundation_netio::shared::client::HttpExchangeClientTask;
use foundation_netio::shared::http::{
    HttpClientError, SendSafeBody, SimpleHeaders, SimpleResponse, Status,
};
use foundation_netio::websocket::shared::client::{
    MessageDelivery, WebSocketClient, WebSocketConnectConfig,
};
use foundation_netio::websocket::shared::connector::{WebSocketConnector, WsExchangeTask};
use foundation_netio::websocket::shared::error::WebSocketError;

const MODEL: &str = "gpt-test";

/// Answers `send_async` with a fixed status and body; the sync `send` fails, so
/// the native `get_model` falls back to placeholder metadata.
struct ModelInfoClient {
    status: Status,
    body: &'static str,
}

#[async_trait]
impl HttpClient for ModelInfoClient {
    async fn send_async(
        &self,
        req: PreparedRequest,
    ) -> Result<SimpleResponse<SendSafeBody>, HttpClientError> {
        assert!(
            req.url.to_string().ends_with(&format!("models/{MODEL}")),
            "unexpected lookup URL: {}",
            req.url
        );
        Ok(SimpleResponse::new(
            self.status.clone(),
            SimpleHeaders::new(),
            SendSafeBody::Text(self.body.to_string()),
        ))
    }

    async fn send_sse_async(
        &self,
        _req: PreparedRequest,
    ) -> Result<BoxedSseFutureStream, HttpClientError> {
        Err(HttpClientError::NotSupported)
    }

    fn send(&self, _req: PreparedRequest) -> Result<SimpleResponse<SendSafeBody>, HttpClientError> {
        Err(HttpClientError::NotSupported)
    }

    fn send_sse(&self, _req: PreparedRequest) -> Result<BoxedSseIterator, HttpClientError> {
        Err(HttpClientError::NotSupported)
    }

    fn open_exchange(&self, _req: PreparedRequest) -> HttpExchangeClientTask {
        unimplemented!("open_exchange is not exercised by the model-info tests")
    }
}
/// The tests send no WebSocket traffic; the impl only makes the fake a
/// `DynNetClient`.
impl WebSocketConnector for ModelInfoClient {
    fn open_websocket(
        &self,
        _url: &str,
        _config: WebSocketConnectConfig,
    ) -> Result<WebSocketClient, WebSocketError> {
        Err(WebSocketError::ProtocolError("not used by these tests".into()))
    }

    fn open_websocket_task(
        &self,
        _url: &str,
        _config: WebSocketConnectConfig,
    ) -> Result<(WsExchangeTask, MessageDelivery), WebSocketError> {
        Err(WebSocketError::ProtocolError("not used by these tests".into()))
    }
}

fn provider(status: Status, body: &'static str) -> OpenAIProvider {
    OpenAIProvider::with_http_client(Arc::new(ModelInfoClient { status, body }))
}

const MODEL_JSON: &str =
    r#"{"id":"gpt-test","object":"model","owned_by":"acme","created":1700000000}"#;

#[test]
fn fetch_model_info_parses_the_vendor_model() {
    let provider = provider(Status::OK, MODEL_JSON);
    let info =
        futures_lite::future::block_on(provider.fetch_model_info(MODEL)).expect("lookup succeeds");
    assert_eq!(info.id, "gpt-test");
    assert_eq!(info.object, "model");
    assert_eq!(info.owned_by, "acme");
    assert_eq!(info.created, 1_700_000_000);
}

#[test]
fn model_handed_out_before_the_lookup_reports_its_result() {
    let provider = provider(Status::OK, MODEL_JSON);
    let model = provider
        .get_model(ModelId::Name(MODEL.to_string(), None))
        .expect("get_model never fails on a missing lookup");
    assert_eq!(model.info().owned_by, "", "placeholder before the lookup");

    futures_lite::future::block_on(provider.fetch_model_info(MODEL)).expect("lookup succeeds");
    assert_eq!(
        model.info().owned_by,
        "acme",
        "the same model sees the lookup result"
    );
}

#[test]
fn non_2xx_lookup_is_an_error_and_keeps_the_placeholder() {
    let provider = provider(Status::NotFound, r#"{"error":{"message":"no such model"}}"#);
    let model = provider
        .get_model(ModelId::Name(MODEL.to_string(), None))
        .expect("get_model never fails on a missing lookup");

    let err = futures_lite::future::block_on(provider.fetch_model_info(MODEL))
        .expect_err("404 is an error");
    assert!(
        err.to_string().contains("no such model"),
        "error carries the vendor message: {err}"
    );
    assert_eq!(model.info().id, MODEL);
    assert_eq!(model.info().owned_by, "");
}
