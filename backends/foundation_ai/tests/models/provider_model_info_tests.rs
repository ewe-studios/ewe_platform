//! Model metadata lookups for the Anthropic and OpenAI Responses providers
//! (the shared `backends::model_info` path `OpenAIProvider` also uses):
//! - `fetch_model_info` is a non-blocking `GET models/{name}` whose result a
//!   model handed out earlier sees through `info()`;
//! - `get_model` never waits on the network: it returns placeholder metadata
//!   at once and the lookup runs in the background (it used to block natively,
//!   which hung whenever the server did not answer);
//! - Anthropic's default URLs are `/v1/...`, with `anthropic-version` as a
//!   header, and its `created_at` (RFC 3339) becomes Unix seconds.

use std::sync::{Arc, Mutex};

use async_trait::async_trait;
use foundation_ai::backends::anthropic_messages_provider::{
    AnthropicConfig, AnthropicMessagesProvider,
};
use foundation_ai::backends::model_info::rfc3339_to_unix;
use foundation_ai::backends::openai_responses_provider::ResponsesProvider;
use foundation_ai::types::{ModelId, ModelProvider};
use foundation_netio::shared::client::http_client::{
    BoxedSseFutureStream, BoxedSseIterator, HttpClient,
};
use foundation_netio::shared::client::request::PreparedRequest;
use foundation_netio::shared::client::HttpExchangeClientTask;
use foundation_netio::shared::http::{
    HttpClientError, SendSafeBody, SimpleHeader, SimpleHeaders, SimpleMethod, SimpleResponse,
    Status,
};

/// Answers both `send` and `send_async` with a fixed status and body and
/// records every request it saw.
struct RecordingClient {
    status: Status,
    body: &'static str,
    seen: Mutex<Vec<PreparedRequest>>,
}

impl RecordingClient {
    fn new(status: Status, body: &'static str) -> Arc<Self> {
        Arc::new(Self {
            status,
            body,
            seen: Mutex::new(Vec::new()),
        })
    }

    fn answer(&self, req: PreparedRequest) -> SimpleResponse<SendSafeBody> {
        self.seen.lock().unwrap().push(req);
        SimpleResponse::new(
            self.status.clone(),
            SimpleHeaders::new(),
            SendSafeBody::Text(self.body.to_string()),
        )
    }

    fn last(&self) -> (SimpleMethod, String, SimpleHeaders) {
        let seen = self.seen.lock().unwrap();
        let req = seen.last().expect("a request was sent");
        (req.method.clone(), req.url.to_string(), req.headers.clone())
    }
}

#[async_trait]
impl HttpClient for RecordingClient {
    async fn send_async(
        &self,
        req: PreparedRequest,
    ) -> Result<SimpleResponse<SendSafeBody>, HttpClientError> {
        Ok(self.answer(req))
    }

    async fn send_sse_async(
        &self,
        _req: PreparedRequest,
    ) -> Result<BoxedSseFutureStream, HttpClientError> {
        Err(HttpClientError::NotSupported)
    }

    fn send(&self, req: PreparedRequest) -> Result<SimpleResponse<SendSafeBody>, HttpClientError> {
        Ok(self.answer(req))
    }

    fn send_sse(&self, _req: PreparedRequest) -> Result<BoxedSseIterator, HttpClientError> {
        Err(HttpClientError::NotSupported)
    }

    fn open_exchange(&self, _req: PreparedRequest) -> HttpExchangeClientTask {
        unimplemented!("open_exchange is not exercised by the model-info tests")
    }
}

fn header(headers: &SimpleHeaders, name: &str) -> Option<String> {
    headers
        .get(&SimpleHeader::from(name.to_string()))
        .and_then(|v| v.first().cloned())
}

const CLAUDE: &str = "claude-test";
const CLAUDE_JSON: &str = r#"{"type":"model","id":"claude-test","display_name":"Claude Test","created_at":"2025-02-19T00:00:00Z"}"#;

fn anthropic(client: &Arc<RecordingClient>) -> AnthropicMessagesProvider {
    AnthropicMessagesProvider::with_http_client(Arc::clone(client) as Arc<dyn HttpClient>)
        .create(Some(AnthropicConfig::api_key("sk-test")))
        .expect("provider")
}

#[test]
fn anthropic_fetch_model_info_is_a_versioned_get() {
    let client = RecordingClient::new(Status::OK, CLAUDE_JSON);
    let provider = anthropic(&client);
    let info = futures_lite::future::block_on(provider.fetch_model_info(CLAUDE)).expect("lookup");
    assert_eq!(info.id, CLAUDE);
    assert_eq!(info.object, "model");
    assert_eq!(info.owned_by, "anthropic");
    assert_eq!(info.created, 1_739_923_200, "created_at parsed to Unix seconds");

    let (method, url, headers) = client.last();
    assert_eq!(method, SimpleMethod::GET);
    assert_eq!(url, format!("https://api.anthropic.com/v1/models/{CLAUDE}"));
    assert_eq!(header(&headers, "x-api-key").as_deref(), Some("sk-test"));
    assert_eq!(header(&headers, "anthropic-version").as_deref(), Some("2023-06-01"));
}

#[test]
fn anthropic_model_sees_a_later_lookup() {
    // The native `get_model` lookup fails (404), so the model starts with the
    // placeholder; a later successful fetch updates the same model.
    let failing = RecordingClient::new(Status::NotFound, r#"{"error":{"type":"not_found_error","message":"no such model"}}"#);
    let provider = anthropic(&failing);
    let model = provider
        .get_model(ModelId::Name(CLAUDE.to_string(), None))
        .expect("get_model never fails on a missing lookup");
    assert_eq!(model.info().created, 0, "placeholder");
    assert_eq!(model.info().owned_by, "anthropic");
    let err = futures_lite::future::block_on(provider.fetch_model_info(CLAUDE))
        .expect_err("404 is an error");
    assert!(err.to_string().contains("no such model"), "{err}");
    assert_eq!(model.info().created, 0, "a failed lookup keeps the placeholder");
}

#[test]
fn anthropic_urls_use_v1_and_follow_a_proxy_endpoint() {
    let config = AnthropicConfig::default();
    assert_eq!(config.build_url("messages"), "https://api.anthropic.com/v1/messages");
    assert_eq!(
        config.models_url("m").as_deref(),
        Some("https://api.anthropic.com/v1/models/m")
    );

    let proxied = AnthropicConfig::default()
        .with_base_url("http://localhost:8080")
        .with_messages_endpoint("/v1/messages");
    assert_eq!(proxied.build_url("messages"), "http://localhost:8080/v1/messages");
    assert_eq!(
        proxied.models_url("m").as_deref(),
        Some("http://localhost:8080/v1/models/m")
    );

    let odd = AnthropicConfig::default().with_messages_endpoint("/custom/chat");
    assert_eq!(odd.models_url("m"), None, "no models path to derive");
}

const GPT: &str = "gpt-test";
const GPT_JSON: &str = r#"{"id":"gpt-test","object":"model","owned_by":"acme","created":1700000000}"#;

fn responses(client: &Arc<RecordingClient>) -> ResponsesProvider {
    ResponsesProvider::with_http_client(Arc::clone(client) as Arc<dyn HttpClient>)
        .create(None)
        .expect("provider")
}

#[test]
fn responses_fetch_model_info_updates_earlier_models() {
    let failing = RecordingClient::new(Status::NotFound, r#"{"error":{"message":"nope"}}"#);
    let provider = responses(&failing);
    let model = provider
        .get_model(ModelId::Name(GPT.to_string(), None))
        .expect("model");
    assert_eq!(model.info().owned_by, "", "placeholder");
    assert!(futures_lite::future::block_on(provider.fetch_model_info(GPT)).is_err());
    assert_eq!(model.info().id, GPT);
}

#[test]
fn rfc3339_times_convert_to_unix_seconds() {
    assert_eq!(rfc3339_to_unix("1970-01-01T00:00:00Z"), Some(0));
    assert_eq!(rfc3339_to_unix("2025-02-19T00:00:00Z"), Some(1_739_923_200));
    assert_eq!(rfc3339_to_unix("2025-02-19T02:00:00+02:00"), Some(1_739_923_200));
    assert_eq!(rfc3339_to_unix("2025-02-18T19:00:00.123-05:00"), Some(1_739_923_200));
    assert_eq!(rfc3339_to_unix("2024-02-29T12:00:00Z"), Some(1_709_208_000), "leap day");
    for bad in ["", "2025-02-19", "2025-13-01T00:00:00Z", "2025-02-19T00:00:00", "1969-12-31T23:59:59Z"] {
        assert_eq!(rfc3339_to_unix(bad), None, "{bad:?}");
    }
}

/// A client whose requests never complete.
struct NeverAnswers;

#[async_trait]
impl HttpClient for NeverAnswers {
    async fn send_async(
        &self,
        _req: PreparedRequest,
    ) -> Result<SimpleResponse<SendSafeBody>, HttpClientError> {
        std::future::pending().await
    }

    async fn send_sse_async(
        &self,
        _req: PreparedRequest,
    ) -> Result<BoxedSseFutureStream, HttpClientError> {
        std::future::pending().await
    }

    fn send(&self, _req: PreparedRequest) -> Result<SimpleResponse<SendSafeBody>, HttpClientError> {
        loop {
            std::thread::park();
        }
    }

    fn send_sse(&self, _req: PreparedRequest) -> Result<BoxedSseIterator, HttpClientError> {
        loop {
            std::thread::park();
        }
    }

    fn open_exchange(&self, _req: PreparedRequest) -> HttpExchangeClientTask {
        unimplemented!("open_exchange is not exercised by the model-info tests")
    }
}

/// `get_model` returns placeholder metadata at once even when the models
/// endpoint never answers (each provider).
#[test]
fn get_model_never_waits_on_the_network() {
    let client: Arc<dyn HttpClient> = Arc::new(NeverAnswers);

    let model = AnthropicMessagesProvider::with_http_client(Arc::clone(&client))
        .create(Some(AnthropicConfig::api_key("sk-test")))
        .expect("provider")
        .get_model(ModelId::Name(CLAUDE.to_string(), None))
        .expect("model");
    assert_eq!(model.info().id, CLAUDE);

    let model = ResponsesProvider::with_http_client(Arc::clone(&client))
        .create(None)
        .expect("provider")
        .get_model(ModelId::Name(GPT.to_string(), None))
        .expect("model");
    assert_eq!(model.info().id, GPT);

    let model = foundation_ai::backends::openai_provider::OpenAIProvider::with_http_client(client)
        .get_model(ModelId::Name(GPT.to_string(), None))
        .expect("model");
    assert_eq!(model.info().id, GPT);
}
