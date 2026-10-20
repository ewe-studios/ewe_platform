//! Cloudflare Workers AI HTTP provider.
//!
//! Calls the Workers AI REST endpoint
//! (`POST /client/v4/accounts/{account_id}/ai/run/{model}`) over
//! `foundation_netio`'s `HttpClient` seam — the same cross-platform seam the
//! OpenAI/Anthropic providers use. Target-agnostic: compiles native and wasm,
//! with streaming served by `Model::stream()` (SSE, `BoxedSseIterator`).
//!
//! Workers AI text generation is an OpenAI-compatible chat API: it accepts a
//! `messages` array of `{role, content}` and, with `stream: true`, answers
//! `text/event-stream` where each `data:` line is `{"response": "<text so far>"}`
//! (the `response` field is cumulative). There is no `[DONE]` sentinel; the
//! stream ends when the connection closes.

use std::sync::Arc;

use foundation_compact::SystemTime;

use crate::backends::backend_utils::{empty_usage_report, model_id_to_string};
use crate::backends::openai_provider::OpenAIFormatter;
use crate::errors::{GenerationError, GenerationResult, ModelProviderErrors, ModelProviderResult};
use crate::types::base_types::{
    AuthProvider, Messages, MessageType, Model, ModelAPI, ModelId, ModelInteraction, ModelOutput,
    ModelParams, ModelProvider, ModelProviderDescriptor, ModelProviders, ModelSpec, ModelState,
    ModelStreamBox, ModelUsageCosting, StopReason, TextContent, ToolFormatter, UsageReport,
    UserModelContent,
};
use foundation_auth::{AuthCredential, ConfidentialText};
use foundation_core::url::Uri;
use foundation_core::valtron::{Stream, StreamSpread};
use foundation_netio::event_source::{Event, ParseResult};
use foundation_netio::shared::client::{
    body_reader::collect_strings_from_send_safe,
    http_client::{BoxedSseIterator, HttpClient, SseProgress},
    request::{Extensions, PreparedRequest},
};
use foundation_netio::shared::http::{SendSafeBody, SimpleHeader, SimpleHeaders, SimpleMethod};
use serde::{Deserialize, Serialize};

/// Cloudflare Workers AI REST API base URL.
pub const WORKERS_AI_BASE_URL: &str = "https://api.cloudflare.com";

/// `Authorization: Bearer <CLOUDFLARE_API_TOKEN>` — the Worker's API token,
/// injected through config, never hardcoded in the bundle.
const WORKERS_AI_PROVIDER_ID: &str = "workers-ai";

// ============================================================================
// Workers AI Configuration
// ============================================================================

/// Configuration for the Workers AI provider.
#[derive(Debug)]
pub struct WorkersAiConfig {
    pub account_id: String,
    pub base_url: String,
    pub timeout_secs: u64,
    pub streaming: bool,
    pub auth: Option<AuthCredential>,
}

impl Clone for WorkersAiConfig {
    fn clone(&self) -> Self {
        // `AuthCredential` is not `Clone`; config clones drop the credential —
        // `WorkersAiProvider::create` already extracted it into `api_token`.
        Self {
            account_id: self.account_id.clone(),
            base_url: self.base_url.clone(),
            timeout_secs: self.timeout_secs,
            streaming: self.streaming,
            auth: None,
        }
    }
}

impl Default for WorkersAiConfig {
    fn default() -> Self {
        Self {
            account_id: String::new(),
            base_url: String::from(WORKERS_AI_BASE_URL),
            timeout_secs: 30,
            streaming: true,
            auth: None,
        }
    }
}

impl WorkersAiConfig {
    /// A config authenticated with a Cloudflare API token
    /// (`AuthCredential::SecretOnly`).
    #[must_use]
    pub fn api_token(token: impl Into<String>) -> Self {
        Self::new().with_auth(AuthCredential::SecretOnly(ConfidentialText::new(
            token.into(),
        )))
    }

    /// [`api_token`](Self::api_token) plus the account id, from the
    /// `CLOUDFLARE_API_TOKEN` and `CLOUDFLARE_ACCOUNT_ID` environment variables.
    ///
    /// # Errors
    /// [`std::env::VarError`] when either variable is unset or not unicode.
    pub fn from_env() -> Result<Self, std::env::VarError> {
        Ok(Self::api_token(std::env::var("CLOUDFLARE_API_TOKEN")?)
            .with_account_id(std::env::var("CLOUDFLARE_ACCOUNT_ID")?))
    }

    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    #[must_use]
    pub fn with_account_id(mut self, account_id: impl Into<String>) -> Self {
        self.account_id = account_id.into();
        self
    }

    #[must_use]
    pub fn with_base_url(mut self, url: impl Into<String>) -> Self {
        self.base_url = url.into();
        self
    }

    #[must_use]
    pub fn with_timeout_secs(mut self, secs: u64) -> Self {
        self.timeout_secs = secs;
        self
    }

    #[must_use]
    pub fn with_streaming(mut self, streaming: bool) -> Self {
        self.streaming = streaming;
        self
    }

    #[must_use]
    pub fn with_auth(mut self, auth: AuthCredential) -> Self {
        self.auth = Some(auth);
        self
    }
}

impl AuthProvider for WorkersAiConfig {
    fn auth(&self) -> Option<&AuthCredential> {
        self.auth.as_ref()
    }
}

// ============================================================================
// Workers AI Provider
// ============================================================================

/// Cloudflare Workers AI provider implementing [`ModelProvider`].
pub struct WorkersAiProvider {
    config: WorkersAiConfig,
    api_token: Option<ConfidentialText>,
    http_client: Option<Arc<dyn HttpClient>>,
}

impl Default for WorkersAiProvider {
    fn default() -> Self {
        Self::new()
    }
}

impl WorkersAiProvider {
    #[must_use]
    pub fn new() -> Self {
        Self {
            config: WorkersAiConfig::default(),
            api_token: None,
            http_client: None,
        }
    }

    /// A provider authenticated with a Cloudflare API token.
    #[must_use]
    pub fn api_token(token: impl Into<String>) -> Self {
        Self::with_config(WorkersAiConfig::api_token(token))
    }

    #[must_use]
    pub fn with_config(config: WorkersAiConfig) -> Self {
        Self {
            config,
            api_token: None,
            http_client: None,
        }
    }

    #[must_use]
    pub fn with_http_client(client: Arc<dyn HttpClient>) -> Self {
        Self {
            config: WorkersAiConfig::default(),
            api_token: None,
            http_client: Some(client),
        }
    }

    #[must_use]
    pub fn with_http_client_and_config(
        client: Arc<dyn HttpClient>,
        config: WorkersAiConfig,
    ) -> Self {
        Self {
            config,
            api_token: None,
            http_client: Some(client),
        }
    }
}

impl ModelProvider for WorkersAiProvider {
    type Config = WorkersAiConfig;
    type Model = WorkersAiModel;

    fn create(mut self, config: Option<Self::Config>) -> ModelProviderResult<Self> {
        if let Some(cfg) = config {
            if let Some(cred) = cfg.auth() {
                match cred {
                    AuthCredential::SecretOnly(key) => {
                        self.api_token = Some(key.clone());
                    }
                    AuthCredential::ClientSecret {
                        client_id: _,
                        client_secret,
                    } => {
                        self.api_token = Some(client_secret.clone());
                    }
                    AuthCredential::OAuth(cred) => {
                        self.api_token = Some(cred.access_token.clone());
                    }
                    AuthCredential::EmailAuth { .. }
                    | AuthCredential::UsernameAndPassword { .. } => {
                        return Err(ModelProviderErrors::NotFound(
                            "Workers AI requires SecretOnly, ClientSecret, or OAuth credentials"
                                .into(),
                        ));
                    }
                }
            }
            self.config = cfg;
        }

        #[cfg(not(target_family = "wasm"))]
        if self.http_client.is_none() {
            self.http_client = Some(foundation_netio::http::default_http_client());
        }

        Ok(self)
    }

    fn describe(&self) -> ModelProviderResult<ModelProviderDescriptor> {
        Ok(ModelProviderDescriptor {
            id: WORKERS_AI_PROVIDER_ID,
            name: "Workers AI",
            reasoning: false,
            api: ModelAPI::Custom(WORKERS_AI_PROVIDER_ID.to_string()),
            provider: ModelProviders::Custom(WORKERS_AI_PROVIDER_ID.to_string()),
            base_url: None,
            inputs: MessageType::Text,
            cost: ModelUsageCosting::default(),
            context_window: 0,
            max_tokens: 0,
        })
    }

    fn get_model(&self, model_id: ModelId) -> ModelProviderResult<Self::Model> {
        let model_name = model_id_to_string(&model_id);
        Ok(WorkersAiModel {
            config: self.config.clone(),
            model_id,
            model_name,
            api_token: self.api_token.clone(),
            http_client: self.http_client.clone(),
            pricing: ModelUsageCosting::default(),
        })
    }

    fn get_model_by_spec(&self, spec: ModelSpec) -> ModelProviderResult<Self::Model> {
        self.get_model(spec.id)
    }

    fn get_one(&self, model_id: ModelId) -> ModelProviderResult<ModelSpec> {
        self.get_all(model_id.clone())?
            .into_iter()
            .next()
            .ok_or_else(|| ModelProviderErrors::NotFound(format!("No model matching {model_id:?}")))
    }

    fn get_all(&self, model_id: ModelId) -> ModelProviderResult<Vec<ModelSpec>> {
        // Workers AI model names are known strings (`@cf/meta/llama-3.1-8b-instruct`),
        // not a fetched catalogue. Synthesise a pass-through spec so `get_one`/
        // `serves` keep working without a round-trip.
        let name = model_id_to_string(&model_id);
        Ok(vec![ModelSpec {
            name,
            id: model_id,
            devices: None,
            model_location: None,
            lora_location: None,
        }])
    }
}

// ============================================================================
// Workers AI Model
// ============================================================================

/// A model handle for the Workers AI provider implementing [`Model`].
pub struct WorkersAiModel {
    config: WorkersAiConfig,
    model_id: ModelId,
    model_name: String,
    api_token: Option<ConfidentialText>,
    http_client: Option<Arc<dyn HttpClient>>,
    pricing: ModelUsageCosting,
}

impl WorkersAiModel {
    /// The REST endpoint for a single `ai/run` call.
    fn build_url(&self) -> String {
        format!(
            "{}/client/v4/accounts/{}/ai/run/{}",
            self.config.base_url.trim_end_matches('/'),
            self.config.account_id,
            self.model_name
        )
    }

    fn auth_headers(&self) -> SimpleHeaders {
        let mut headers = SimpleHeaders::new();
        if let Some(token) = &self.api_token {
            headers.insert(
                SimpleHeader::AUTHORIZATION,
                vec![format!("Bearer {}", token.get())],
            );
        }
        headers.insert(
            SimpleHeader::CONTENT_TYPE,
            vec![String::from("application/json")],
        );
        headers
    }

    fn build_prepared_request(&self, url: &str, body: &str) -> GenerationResult<PreparedRequest> {
        let uri =
            Uri::parse(url).map_err(|e| GenerationError::Backend(format!("Invalid URL: {e}")))?;
        let mut headers = self.auth_headers();
        headers.insert(SimpleHeader::ACCEPT, vec![String::from("application/json")]);
        Ok(PreparedRequest {
            method: SimpleMethod::POST,
            url: uri,
            headers,
            body: SendSafeBody::Text(body.to_string()),
            extensions: Extensions::default(),
        })
    }

    fn build_sse_request(&self, url: &str, body: &str) -> GenerationResult<PreparedRequest> {
        let uri =
            Uri::parse(url).map_err(|e| GenerationError::Backend(format!("Invalid URL: {e}")))?;
        let mut headers = self.auth_headers();
        headers.insert(
            SimpleHeader::ACCEPT,
            vec![String::from("text/event-stream")],
        );
        Ok(PreparedRequest {
            method: SimpleMethod::POST,
            url: uri,
            headers,
            body: SendSafeBody::Text(body.to_string()),
            extensions: Extensions::default(),
        })
    }

    /// Execute a single non-streaming request (no retry: wasm has no `thread`).
    fn do_request<T: for<'de> Deserialize<'de>>(&self, url: &str, body: &str) -> GenerationResult<T> {
        let client = self
            .http_client
            .as_ref()
            .ok_or_else(|| GenerationError::Generic("HTTP client not initialized".into()))?;

        let req = self.build_prepared_request(url, body)?;
        let response = client
            .send(req)
            .map_err(|e| GenerationError::Backend(format!("Request failed: {e}")))?;

        let (status, _headers, body) = response.into_parts();
        let status_code: usize = status.into();
        let body_text = collect_strings_from_send_safe(body)
            .map_err(|e| GenerationError::Generic(format!("Parse error: {e}")))?;

        if !(200..=299).contains(&status_code) {
            return Err(GenerationError::Backend(format!(
                "Workers AI request failed with status {status_code}: {body_text}"
            )));
        }

        serde_json::from_str(&body_text)
            .map_err(|e| GenerationError::Generic(format!("Parse error: {e}")))
    }
}

impl Model for WorkersAiModel {
    fn tool_formatter(&self) -> Box<dyn ToolFormatter> {
        // Workers AI tool calling is OpenAI-compatible.
        Box::new(OpenAIFormatter)
    }

    fn spec(&self) -> ModelSpec {
        ModelSpec {
            name: self.model_name.clone(),
            id: self.model_id.clone(),
            devices: None,
            model_location: None,
            lora_location: None,
        }
    }

    fn descriptor(&self) -> Option<ModelProviderDescriptor> {
        Some(ModelProviderDescriptor {
            id: WORKERS_AI_PROVIDER_ID,
            name: "Workers AI",
            reasoning: false,
            api: ModelAPI::Custom(WORKERS_AI_PROVIDER_ID.to_string()),
            provider: ModelProviders::Custom(WORKERS_AI_PROVIDER_ID.to_string()),
            base_url: None,
            inputs: MessageType::Text,
            cost: self.pricing,
            context_window: 0,
            max_tokens: 0,
        })
    }

    fn costing(&self) -> GenerationResult<UsageReport> {
        // Workers AI per-token pricing varies by model and is not modelled here.
        Ok(empty_usage_report())
    }

    fn generate(
        &self,
        interaction: ModelInteraction,
        specs: Option<ModelParams>,
    ) -> GenerationResult<Vec<Messages>> {
        let params = specs.unwrap_or_default();
        let request = build_run_request(&interaction, &params, false);
        let body = serde_json::to_string(&request)
            .map_err(|e| GenerationError::Generic(format!("Failed to serialize request: {e}")))?;

        let url = self.build_url();
        let response: WorkersAiRunResponse = self.do_request(&url, &body)?;
        let text = response.result.response.unwrap_or_default();
        Ok(vec![assistant_message(
            &self.model_id,
            text,
            StopReason::Stop,
            None,
        )])
    }

    fn stream(
        &self,
        interaction: ModelInteraction,
        specs: Option<ModelParams>,
    ) -> GenerationResult<ModelStreamBox> {
        let params = specs.unwrap_or_default();
        let request = build_run_request(&interaction, &params, true);
        let body = serde_json::to_string(&request)
            .map_err(|e| GenerationError::Generic(format!("Failed to serialize request: {e}")))?;

        let url = self.build_url();
        let client = self
            .http_client
            .as_ref()
            .ok_or_else(|| GenerationError::Generic("HTTP client not initialized".into()))?;

        let req = self.build_sse_request(&url, &body)?;
        let sse_iter = client
            .send_sse(req)
            .map_err(|e| GenerationError::Backend(format!("SSE request failed: {e}")))?;

        Ok(Box::new(WorkersAiStream {
            inner: sse_iter,
            model_id: self.model_id.clone(),
            accumulated_text: String::new(),
            done: false,
        }))
    }
}

// ============================================================================
// Streaming: SSE Parser
// ============================================================================

/// Streaming iterator that yields incremental `Messages` from a Workers AI SSE
/// stream.
///
/// Wraps a `BoxedSseIterator`. Each `Event::Message` carries a
/// `{"response": "<text so far>"}` chunk; `response` is cumulative, so it
/// replaces the accumulator and is yielded as `Stream::Next` (first token
/// arrives before the stream closes).
struct WorkersAiStream {
    inner: BoxedSseIterator,
    model_id: ModelId,
    accumulated_text: String,
    done: bool,
}

/// Readiness comes from the underlying SSE stream: a consumer parked on this
/// stream's `Wait` is woken when the next SSE event (or end of stream) arrives.
impl foundation_core::valtron::WakeableStream for WorkersAiStream {
    fn wake(
        &mut self,
        cb: foundation_core::valtron::WakeCallback,
    ) -> foundation_core::valtron::WakeResult {
        self.inner.wake(cb)
    }
}

impl Iterator for WorkersAiStream {
    type Item = Stream<Messages, ModelState>;

    fn next(&mut self) -> Option<Self::Item> {
        if self.done {
            return None;
        }

        let item = self.inner.next()?;

        match item {
            Stream::Next(ref parse_result) => Some(self.process_parse_result(parse_result)),
            Stream::Pending(SseProgress::Failed(msg)) => {
                Some(Stream::Pending(ModelState::Error(msg)))
            }
            Stream::Pending(_) => Some(Stream::Pending(ModelState::GeneratingTokens(None))),
            Stream::Delayed(d) => Some(Stream::Delayed(d)),
            Stream::Init => Some(Stream::Init),
            Stream::Ignore => Some(Stream::Ignore),
            Stream::Wait => Some(Stream::Wait),
            Stream::Spread(items) => {
                let mut mapped: Vec<StreamSpread<Messages, ModelState>> = Vec::new();
                for item in items {
                    match item {
                        StreamSpread::Done(ref parse_result) => {
                            match self.process_parse_result(parse_result) {
                                Stream::Next(msg) => mapped.push(StreamSpread::Done(msg)),
                                Stream::Pending(p) => mapped.push(StreamSpread::Pending(p)),
                                Stream::Delayed(_) => {
                                    self.done = true;
                                    mapped.push(StreamSpread::Done(assistant_message(
                                        &self.model_id,
                                        self.accumulated_text.clone(),
                                        StopReason::Stop,
                                        None,
                                    )));
                                }
                                Stream::Init
                                | Stream::Ignore
                                | Stream::Wait
                                | Stream::Spread(_) => {}
                            }
                        }
                        StreamSpread::Pending(SseProgress::Failed(msg)) => {
                            mapped.push(StreamSpread::Pending(ModelState::Error(msg)));
                        }
                        StreamSpread::Pending(_) => {
                            mapped.push(StreamSpread::Pending(ModelState::GeneratingTokens(None)));
                        }
                    }
                }
                if mapped.is_empty() {
                    Some(Stream::Ignore)
                } else {
                    Some(Stream::Spread(mapped))
                }
            }
        }
    }
}

impl WorkersAiStream {
    fn process_parse_result(&mut self, parse_result: &ParseResult) -> Stream<Messages, ModelState> {
        let Event::Message { data, .. } = &parse_result.event else {
            return Stream::Ignore;
        };

        // Workers AI streams have no `[DONE]` sentinel, but tolerate one anyway.
        if data.trim() == "[DONE]" {
            self.done = true;
            return Stream::Next(assistant_message(
                &self.model_id,
                self.accumulated_text.clone(),
                StopReason::Stop,
                None,
            ));
        }

        let chunk: WorkersAiStreamChunk = match serde_json::from_str(data) {
            Ok(chunk) => chunk,
            Err(_) => {
                tracing::warn!(data = %data, "Failed to parse Workers AI SSE chunk");
                return Stream::Next(assistant_message(
                    &self.model_id,
                    self.accumulated_text.clone(),
                    StopReason::Error,
                    Some(format!("Failed to parse SSE chunk: {data}")),
                ));
            }
        };

        match chunk.response {
            Some(text) => {
                self.accumulated_text = text;
                Stream::Next(assistant_message(
                    &self.model_id,
                    self.accumulated_text.clone(),
                    StopReason::Stop,
                    None,
                ))
            }
            None => Stream::Ignore,
        }
    }
}

// ============================================================================
// Workers AI API Request/Response Types
// ============================================================================

#[derive(Debug, Clone, Serialize)]
struct WorkersAiMessage {
    role: String,
    content: String,
}

#[derive(Debug, Clone, Serialize)]
struct WorkersAiRunRequest {
    messages: Vec<WorkersAiMessage>,
    #[serde(skip_serializing_if = "Option::is_none")]
    stream: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    max_tokens: Option<usize>,
}

#[derive(Debug, Deserialize)]
struct WorkersAiRunResponse {
    result: WorkersAiResult,
}

#[derive(Debug, Deserialize)]
struct WorkersAiResult {
    response: Option<String>,
}

/// One SSE `data:` payload — `response` is the cumulative text so far.
#[derive(Debug, Deserialize)]
struct WorkersAiStreamChunk {
    response: Option<String>,
}

// ============================================================================
// Helpers
// ============================================================================

/// Build a `{role, content}` chat request from a [`ModelInteraction`] (text only).
fn build_run_request(
    interaction: &ModelInteraction,
    params: &ModelParams,
    streaming: bool,
) -> WorkersAiRunRequest {
    let mut messages = Vec::new();

    let system_content = match (&interaction.system_prompt, &interaction.soul) {
        (Some(sys), Some(soul)) => Some(format!("{sys}\n\n{soul}")),
        (Some(sys), None) => Some(sys.clone()),
        (None, Some(soul)) => Some(soul.clone()),
        (None, None) => None,
    };
    if let Some(content) = system_content {
        if !content.is_empty() {
            messages.push(WorkersAiMessage {
                role: String::from("system"),
                content,
            });
        }
    }

    for msg in &interaction.messages {
        match msg {
            Messages::User {
                content: UserModelContent::Text(tc),
                ..
            } => messages.push(WorkersAiMessage {
                role: String::from("user"),
                content: tc.content.clone(),
            }),
            Messages::Assistant {
                content: ModelOutput::Text(tc),
                ..
            } => messages.push(WorkersAiMessage {
                role: String::from("assistant"),
                content: tc.content.clone(),
            }),
            // Images and tool results are not part of the text-only Workers AI
            // surface exercised here.
            _ => {}
        }
    }

    WorkersAiRunRequest {
        messages,
        stream: Some(streaming),
        max_tokens: if params.max_tokens > 0 {
            Some(params.max_tokens)
        } else {
            None
        },
    }
}

/// Build a `Messages::Assistant` text message for the Workers AI provider.
fn assistant_message(
    model_id: &ModelId,
    content: String,
    stop_reason: StopReason,
    error_detail: Option<String>,
) -> Messages {
    Messages::Assistant {
        id: foundation_compact::ids::new_scru128(),
        model: model_id.clone(),
        timestamp: SystemTime::now(),
        usage: empty_usage_report(),
        content: ModelOutput::Text(TextContent {
            content,
            signature: None,
        }),
        stop_reason,
        provider: ModelProviders::Custom(WORKERS_AI_PROVIDER_ID.to_string()),
        error_detail,
        signature: None,
        metadata: None,
    }
}
