//! Deferred SSE transport-error mapping (feature 06 T4).
//!
//! `HttpClient::send_sse` is lazy on wasm, so a fetch failure is surfaced in-band
//! as `Stream::Pending(SseProgress::Failed(msg))` rather than on the `Result`.
//! These tests drive the public `Model::stream` path with a mock client that
//! yields exactly that item and assert the model layer maps it to
//! `ModelState::Error(msg)` — for both `OpenAIStream` and `WorkersAiStream` — with
//! no network.

use std::sync::Arc;

use async_trait::async_trait;
use foundation_ai::backends::openai_provider::{OpenAIConfig, OpenAIProvider};
use foundation_ai::backends::workers_ai::{WorkersAiConfig, WorkersAiProvider};
use foundation_ai::types::{
    MessageRole, Messages, Model, ModelId, ModelInteraction, ModelProvider, ModelState,
    ModelStreamBox, TextContent, ToolDeclarations, UserModelContent,
};
use foundation_core::valtron::{valtron_test, Stream};
use foundation_netio::shared::client::http_client::{
    BoxedSseFutureStream, BoxedSseIterator, HttpClient, SseProgress,
};
use foundation_netio::shared::client::request::PreparedRequest;
use foundation_netio::shared::client::HttpExchangeClientTask;
use foundation_netio::shared::http::{HttpClientError, SendSafeBody, SimpleResponse};

/// A mock `HttpClient` whose `send_sse` yields a single
/// `Stream::Pending(SseProgress::Failed(..))` before ending.
struct FailedSseClient;

#[async_trait]
impl HttpClient for FailedSseClient {
    async fn send_async(
        &self,
        _req: PreparedRequest,
    ) -> Result<SimpleResponse<SendSafeBody>, HttpClientError> {
        Err(HttpClientError::NotSupported)
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
        Ok(Box::new(std::iter::once(Stream::Pending(
            SseProgress::Failed("boom".to_string()),
        ))))
    }

    fn open_exchange(&self, _req: PreparedRequest) -> HttpExchangeClientTask {
        unimplemented!("open_exchange is not exercised by the error-mapping tests")
    }
}

fn interaction(prompt: &str) -> ModelInteraction {
    ModelInteraction {
        system_prompt: None,
        soul: None,
        tools_shed: ToolDeclarations::default(),
        messages: vec![Messages::User {
            id: foundation_compact::ids::new_scru128(),
            role: MessageRole::User,
            content: UserModelContent::Text(TextContent {
                content: prompt.to_string(),
                signature: None,
            }),
            signature: None,
        }],
        chat_template: None,
        tool_choice: None,
    }
}

/// The shared assertion: the first stream item is the mapped error, and the
/// stream then terminates.
fn assert_maps_failed(mut stream: ModelStreamBox) {
    match stream.next().expect("first stream item") {
        Stream::Pending(ModelState::Error(msg)) => assert_eq!(msg, "boom"),
        other => panic!("expected Pending(ModelState::Error), got {other:?}"),
    }
    assert!(stream.next().is_none(), "Failed should terminate the stream");
}

#[valtron_test]
fn openai_stream_maps_failed_to_model_error() {
    let client: Arc<dyn HttpClient> = Arc::new(FailedSseClient);
    let config = OpenAIConfig::new().with_base_url("http://localhost");
    let provider = OpenAIProvider::with_http_client_and_config(client, config.clone())
        .create(Some(config))
        .unwrap();
    let model = provider
        .get_model(ModelId::Name("gpt-4".into(), None))
        .unwrap();

    assert_maps_failed(model.stream(interaction("Hi"), None).unwrap());
}

#[valtron_test]
fn workers_ai_stream_maps_failed_to_model_error() {
    let client: Arc<dyn HttpClient> = Arc::new(FailedSseClient);
    let config = WorkersAiConfig::new()
        .with_base_url("http://localhost")
        .with_account_id("test-account");
    let provider = WorkersAiProvider::with_http_client_and_config(client, config.clone())
        .create(Some(config))
        .unwrap();
    let model = provider
        .get_model(ModelId::Name("@cf/meta/llama-3-8b-instruct".into(), None))
        .unwrap();

    assert_maps_failed(model.stream(interaction("Hi"), None).unwrap());
}
