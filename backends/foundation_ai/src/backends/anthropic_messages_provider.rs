//! Anthropic Messages API provider for Claude models.
//!
//! Implements the `/v1/messages` endpoint using Valtron `TaskIterator`/`StreamIterator`
//! patterns — no tokio, no async-trait.

use std::collections::HashMap;
use std::sync::Arc;
use std::thread;
use std::time::{Duration, SystemTime};

use foundation_auth::{AuthCredential, ConfidentialText};
use foundation_core::valtron::{execute, Stream, StreamIterator};
use foundation_core::wire::event_source::{Event, ReconnectingEventSourceTask};
use foundation_core::wire::simple_http::client::{
    DnsResolver, SimpleHttpClient, SystemDnsResolver,
};
use foundation_core::wire::simple_http::{SendSafeBody, SimpleHeader, SimpleHeaders};
use serde::{Deserialize, Serialize};

use crate::errors::{GenerationError, GenerationResult, ModelProviderErrors, ModelProviderResult};
use crate::types::{
    AuthProvider, Messages, Model, ModelId, ModelInteraction, ModelOutput,
    ModelParams, ModelProvider, ModelProviderDescriptor, ModelProviders, ModelSpec, ModelState,
    StopReason, TextContent, UsageCosting, UsageReport,
};

// ============================================================================
// Configuration
// ============================================================================

/// Configuration for the Anthropic Messages API provider.
#[derive(Debug)]
pub struct AnthropicConfig {
    pub base_url: String,
    pub api_version: String,
    pub timeout_secs: u64,
    pub max_retries: u32,
    pub proxy_url: Option<String>,
    pub streaming: bool,
    pub auth: Option<AuthCredential>,
}

impl Default for AnthropicConfig {
    fn default() -> Self {
        Self {
            base_url: String::from("https://api.anthropic.com"),
            api_version: String::from("2023-06-01"),
            timeout_secs: 120,
            max_retries: 3,
            proxy_url: None,
            streaming: true,
            auth: None,
        }
    }
}

impl AnthropicConfig {
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    #[must_use]
    pub fn with_base_url(mut self, url: impl Into<String>) -> Self {
        self.base_url = url.into();
        self
    }

    #[must_use]
    pub fn with_api_version(mut self, version: impl Into<String>) -> Self {
        self.api_version = version.into();
        self
    }

    #[must_use]
    pub fn with_timeout_secs(mut self, secs: u64) -> Self {
        self.timeout_secs = secs;
        self
    }

    #[must_use]
    pub fn with_max_retries(mut self, retries: u32) -> Self {
        self.max_retries = retries;
        self
    }

    #[must_use]
    pub fn with_proxy_url(mut self, url: impl Into<String>) -> Self {
        self.proxy_url = Some(url.into());
        self
    }

    #[must_use]
    pub fn with_streaming(mut self, enabled: bool) -> Self {
        self.streaming = enabled;
        self
    }

    #[must_use]
    pub fn with_auth(mut self, auth: AuthCredential) -> Self {
        self.auth = Some(auth);
        self
    }

    #[must_use]
    pub fn build_url(&self, endpoint: &str) -> String {
        format!(
            "{}/{}/{}",
            self.base_url.trim_end_matches('/'),
            self.api_version,
            endpoint.trim_start_matches('/')
        )
    }
}

impl Clone for AnthropicConfig {
    fn clone(&self) -> Self {
        Self {
            base_url: self.base_url.clone(),
            api_version: self.api_version.clone(),
            timeout_secs: self.timeout_secs,
            max_retries: self.max_retries,
            proxy_url: self.proxy_url.clone(),
            streaming: self.streaming,
            auth: None,
        }
    }
}

impl AuthProvider for AnthropicConfig {
    fn auth(&self) -> Option<&AuthCredential> {
        self.auth.as_ref()
    }
}

// ============================================================================
// Request Types
// ============================================================================

/// POST /v1/messages request body.
#[derive(Debug, Clone, Serialize)]
pub struct MessagesRequest {
    pub model: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub system: Option<AnthropicSystemContent>,
    pub messages: Vec<AnthropicMessage>,
    pub max_tokens: u32,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub temperature: Option<f64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub top_p: Option<f64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub top_k: Option<usize>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub stream: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub stop_sequences: Option<Vec<String>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub tools: Option<Vec<AnthropicTool>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub tool_choice: Option<AnthropicToolChoice>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub thinking: Option<AnthropicThinkingConfig>,
}

/// System prompt — can be a string or array of content blocks.
#[derive(Debug, Clone, Serialize)]
#[serde(untagged)]
pub enum AnthropicSystemContent {
    Text(String),
    Blocks(Vec<AnthropicSystemBlock>),
}

/// System content block for multimodal system prompts.
#[derive(Debug, Clone, Serialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum AnthropicSystemBlock {
    Text { text: String },
}

/// Role for Anthropic messages.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum AnthropicRole {
    User,
    Assistant,
}

/// A single message in the conversation.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AnthropicMessage {
    pub role: AnthropicRole,
    pub content: Vec<AnthropicContentBlock>,
}

/// Content blocks within a message.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum AnthropicContentBlock {
    Text {
        text: String,
    },
    Image {
        source: AnthropicImageSource,
    },
    ToolUse {
        id: String,
        name: String,
        input: serde_json::Value,
    },
    ToolResult {
        tool_use_id: String,
        content: String,
        #[serde(skip_serializing_if = "Option::is_none")]
        is_error: Option<bool>,
    },
    Thinking {
        thinking: String,
        signature: String,
    },
    RedactedThinking {
        data: String,
    },
}

/// Image source for Anthropic content blocks (base64 only).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AnthropicImageSource {
    #[serde(rename = "type")]
    pub source_type: String,
    pub media_type: String,
    pub data: String,
}

/// Tool definition at the request level.
#[derive(Debug, Clone, Serialize)]
pub struct AnthropicTool {
    pub name: String,
    pub description: String,
    pub input_schema: serde_json::Value,
}

/// Tool choice configuration.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum AnthropicToolChoice {
    Auto,
    Any,
    Tool { name: String },
}

/// Extended thinking configuration (Claude 3.7+).
#[derive(Debug, Clone, Serialize)]
pub struct AnthropicThinkingConfig {
    #[serde(rename = "type")]
    pub thinking_type: String,
    pub budget_tokens: u32,
}

// ============================================================================
// Response Types
// ============================================================================

/// POST /v1/messages response body.
#[derive(Debug, Clone, Deserialize)]
pub struct MessagesResponse {
    pub id: String,
    #[serde(rename = "type")]
    pub response_type: String,
    pub role: String,
    pub content: Vec<AnthropicContentBlock>,
    pub model: String,
    pub stop_reason: Option<String>,
    pub stop_sequence: Option<String>,
    pub usage: AnthropicUsage,
}

/// Usage information from Anthropic response.
#[derive(Debug, Clone, Deserialize)]
pub struct AnthropicUsage {
    pub input_tokens: u32,
    pub output_tokens: u32,
    #[serde(default)]
    pub cache_creation_input_tokens: u32,
    #[serde(default)]
    pub cache_read_input_tokens: u32,
}

// ============================================================================
// Streaming Event Types
// ============================================================================

#[derive(Debug, Clone, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum StreamEvent {
    MessageStart {
        message: MessagesResponse,
    },
    ContentBlockStart {
        index: usize,
        content_block: AnthropicContentBlock,
    },
    ContentBlockDelta {
        index: usize,
        delta: AnthropicDelta,
    },
    ContentBlockStop {
        index: usize,
    },
    MessageDelta {
        delta: MessageDeltaDelta,
        usage: AnthropicUsage,
    },
    MessageStop,
    Ping,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum AnthropicDelta {
    TextDelta { text: String },
    ThinkingDelta { thinking: String },
    InputJsonDelta { partial_json: String },
}

#[derive(Debug, Clone, Deserialize)]
pub struct MessageDeltaDelta {
    #[serde(default)]
    pub stop_reason: Option<String>,
    #[serde(default)]
    pub stop_sequence: Option<String>,
}

// ============================================================================
// Provider
// ============================================================================

/// Anthropic Messages API provider implementing [`ModelProvider`].
pub struct AnthropicMessagesProvider<R: DnsResolver = SystemDnsResolver> {
    config: AnthropicConfig,
    api_key: Option<ConfidentialText>,
    http_client: Option<SimpleHttpClient<R>>,
    resolver: Option<R>,
    models_cache: Arc<std::sync::Mutex<HashMap<String, crate::backends::openai_provider::OpenAIModelInfo>>>,
}

impl Default for AnthropicMessagesProvider<SystemDnsResolver> {
    fn default() -> Self {
        Self::new()
    }
}

impl AnthropicMessagesProvider<SystemDnsResolver> {
    #[must_use]
    pub fn new() -> Self {
        Self {
            config: AnthropicConfig::default(),
            api_key: None,
            http_client: None,
            resolver: Some(SystemDnsResolver),
            models_cache: Arc::new(std::sync::Mutex::new(HashMap::new())),
        }
    }

    #[must_use]
    pub fn with_config(config: AnthropicConfig) -> Self {
        Self {
            config,
            api_key: None,
            http_client: None,
            resolver: Some(SystemDnsResolver),
            models_cache: Arc::new(std::sync::Mutex::new(HashMap::new())),
        }
    }
}

impl<R: DnsResolver + 'static> AnthropicMessagesProvider<R> {
    #[must_use]
    pub fn with_resolver(resolver: R) -> Self {
        Self {
            config: AnthropicConfig::default(),
            api_key: None,
            http_client: Some(SimpleHttpClient::with_resolver(resolver.clone())),
            resolver: Some(resolver),
            models_cache: Arc::new(std::sync::Mutex::new(HashMap::new())),
        }
    }

    #[must_use]
    pub fn with_resolver_and_config(resolver: R, config: AnthropicConfig) -> Self {
        Self {
            config,
            api_key: None,
            http_client: Some(SimpleHttpClient::with_resolver(resolver.clone())),
            resolver: Some(resolver),
            models_cache: Arc::new(std::sync::Mutex::new(HashMap::new())),
        }
    }

    fn auth_headers(&self) -> Vec<(SimpleHeader, String)> {
        let mut headers = Vec::new();
        if let Some(key) = &self.api_key {
            headers.push((SimpleHeader::from("x-api-key".to_string()), key.get().clone()));
        }
        headers.push((
            SimpleHeader::from("anthropic-version".to_string()),
            self.config.api_version.clone(),
        ));
        headers.push((SimpleHeader::CONTENT_TYPE, String::from("application/json")));
        headers
    }

    fn build_url(&self, endpoint: &str) -> String {
        self.config.build_url(endpoint)
    }

    fn execute_request<T: for<'de> Deserialize<'de> + Send>(
        &self,
        url: &str,
        body: &str,
    ) -> GenerationResult<T> {
        let mut attempt = 0;
        let max_retries = self.config.max_retries;

        loop {
            let result = self.do_request::<T>(url, body)?;
            match result {
                Ok(value) => return Ok(value),
                Err((status, retry_after, msg)) => {
                    if attempt >= max_retries || !is_retryable_status(status) {
                        return Err(GenerationError::Backend(msg));
                    }
                    let delay = retry_after.unwrap_or_else(|| exponential_backoff(attempt));
                    attempt += 1;
                    thread::sleep(Duration::from_secs(delay));
                }
            }
        }
    }

    fn do_request<T: for<'de> Deserialize<'de> + Send>(
        &self,
        url: &str,
        body: &str,
    ) -> GenerationResult<Result<T, (u16, Option<u64>, String)>> {
        let Some(client) = &self.http_client else {
            return Err(GenerationError::Generic(
                "HTTP client not initialized".into(),
            ));
        };

        let mut builder = client
            .post(url)
            .map_err(|e| GenerationError::Backend(format!("Failed to create request: {e}")))?;
        for (k, v) in &self.auth_headers() {
            builder = builder.header(k.clone(), v.clone());
        }
        builder = builder.header(SimpleHeader::ACCEPT, String::from("application/json"));
        builder = builder.body_text(body.to_string());

        let request = client
            .request(builder)
            .map_err(|e| GenerationError::Backend(format!("Failed to build request: {e}")))?;

        let response = request
            .send()
            .map_err(|e| GenerationError::Backend(format!("Request failed: {e}")))?;

        let status_code: usize = response.get_status().into();
        let headers = response.get_headers_ref();
        let body_text = match response.get_body_ref() {
            SendSafeBody::Text(t) => t.clone(),
            SendSafeBody::Bytes(b) => String::from_utf8_lossy(b).to_string(),
            SendSafeBody::None
            | SendSafeBody::Stream(_)
            | SendSafeBody::ChunkedStream(_)
            | SendSafeBody::LineFeedStream(_) => String::new(),
        };

        if !(200..=299).contains(&status_code) {
            let retry_after = extract_retry_after(headers);
            let detail = parse_anthropic_error(&body_text).unwrap_or_else(|| body_text.clone());
            let msg = format_http_error(status_code, &detail);
            return Ok(Err((status_code as u16, retry_after, msg)));
        }

        serde_json::from_str(&body_text)
            .map(|v| Ok(v))
            .map_err(|e| GenerationError::Generic(format!("Parse error: {e}")))
    }
}

impl<R: DnsResolver + Default + 'static> ModelProvider for AnthropicMessagesProvider<R> {
    type Config = AnthropicConfig;
    type Model = AnthropicModel<R>;

    fn create(mut self, config: Option<Self::Config>) -> ModelProviderResult<Self> {
        if let Some(cfg) = config {
            if let Some(cred) = cfg.auth() {
                match &cred {
                    AuthCredential::SecretOnly(key) => {
                        self.api_key = Some(key.clone());
                    }
                    AuthCredential::ClientSecret {
                        client_secret, ..
                    } => {
                        self.api_key = Some(client_secret.clone());
                    }
                    AuthCredential::OAuth(cred) => {
                        self.api_key = Some(cred.access_token.clone());
                    }
                    AuthCredential::EmailAuth { .. }
                    | AuthCredential::UsernameAndPassword { .. } => {
                        return Err(ModelProviderErrors::NotFound(
                            "Anthropic provider requires SecretOnly, ClientSecret, or OAuth credentials"
                                .into(),
                        ));
                    }
                }
            }
            self.config = cfg;
        }

        let mut client = self.http_client.take().unwrap_or_default();
        if let Some(proxy) = &self.config.proxy_url {
            client = client
                .proxy(proxy)
                .map_err(|e| ModelProviderErrors::NotFound(format!("Invalid proxy URL: {e}")))?;
        }
        client = client.read_timeout(std::time::Duration::from_secs(self.config.timeout_secs));
        client = client.connect_timeout(std::time::Duration::from_secs(10));
        self.http_client = Some(client);

        Ok(self)
    }

    fn describe(&self) -> ModelProviderResult<ModelProviderDescriptor> {
        Ok(ModelProviderDescriptor {
            id: String::from("anthropic"),
            name: String::from("Anthropic"),
            reasoning: true,
            api: crate::types::ModelAPI::AnthropicMessages,
            provider: ModelProviders::ANTHROPIC,
            base_url: Some(self.config.base_url.clone()),
            inputs: crate::types::MessageType::TextAndImages,
            cost: crate::types::ModelUsageCosting {
                input: 0.0,
                output: 0.0,
                cache_read: 0.0,
                cache_write: 0.0,
            },
            context_window: 0,
            max_tokens: 0,
        })
    }

    fn get_model(&self, model_id: ModelId) -> ModelProviderResult<Self::Model> {
        let model_name = model_id_to_string(&model_id);

        let cache = self.models_cache.lock().expect("model cache poisoned");
        if let Some(info) = cache.get(&model_name) {
            return Ok(AnthropicModel {
                config: self.config.clone(),
                model_id: model_id.clone(),
                model_name: model_name.clone(),
                api_key: self.api_key.clone(),
                http_client: self.http_client.clone(),
                resolver: self.resolver.clone(),
                info: info.clone(),
            });
        }
        drop(cache);

        // Anthropic doesn't have a /v1/models endpoint like OpenAI,
        // so we create a best-effort model info.
        let info = crate::backends::openai_provider::OpenAIModelInfo {
            id: model_name.clone(),
            object: String::from("model"),
            owned_by: String::from("anthropic"),
            created: 0,
        };

        let mut cache = self.models_cache.lock().expect("model cache poisoned");
        cache.insert(model_name.clone(), info.clone());

        Ok(AnthropicModel {
            config: self.config.clone(),
            model_id,
            model_name,
            api_key: self.api_key.clone(),
            http_client: self.http_client.clone(),
            resolver: self.resolver.clone(),
            info,
        })
    }

    fn get_model_by_spec(&self, spec: ModelSpec) -> ModelProviderResult<Self::Model> {
        self.get_model(spec.id)
    }

    fn get_one(&self, model_id: ModelId) -> ModelProviderResult<ModelSpec> {
        Ok(ModelSpec {
            name: model_id_to_string(&model_id),
            id: model_id.clone(),
            devices: None,
            model_location: None,
            lora_location: None,
        })
    }

    fn get_all(&self, model_id: ModelId) -> ModelProviderResult<Vec<ModelSpec>> {
        // Anthropic doesn't expose a model listing API, return a single spec.
        Ok(vec![ModelSpec {
            name: model_id_to_string(&model_id),
            id: model_id,
            devices: None,
            model_location: None,
            lora_location: None,
        }])
    }
}

// ============================================================================
// Model
// ============================================================================

pub struct AnthropicModel<R: DnsResolver = SystemDnsResolver> {
    config: AnthropicConfig,
    model_id: ModelId,
    model_name: String,
    api_key: Option<ConfidentialText>,
    http_client: Option<SimpleHttpClient<R>>,
    resolver: Option<R>,
    #[allow(dead_code)]
    info: crate::backends::openai_provider::OpenAIModelInfo,
}

impl<R: DnsResolver + 'static> AnthropicModel<R> {
    fn build_url(&self, endpoint: &str) -> String {
        self.config.build_url(endpoint)
    }

    fn build_auth_headers(&self) -> Vec<(SimpleHeader, String)> {
        let mut headers = Vec::new();
        if let Some(key) = &self.api_key {
            headers.push((SimpleHeader::from("x-api-key".to_string()), key.get().clone()));
        }
        headers.push((
            SimpleHeader::from("anthropic-version".to_string()),
            self.config.api_version.clone(),
        ));
        headers.push((SimpleHeader::CONTENT_TYPE, String::from("application/json")));
        headers
    }

    fn execute_request<T: for<'de> Deserialize<'de> + Send>(
        &self,
        url: &str,
        body: &str,
    ) -> GenerationResult<T> {
        let mut attempt = 0;
        let max_retries = self.config.max_retries;

        loop {
            let result = self.do_request::<T>(url, body)?;
            match result {
                Ok(value) => return Ok(value),
                Err((status, retry_after, msg)) => {
                    if attempt >= max_retries || !is_retryable_status(status) {
                        return Err(GenerationError::Backend(msg));
                    }
                    let delay = retry_after.unwrap_or_else(|| exponential_backoff(attempt));
                    attempt += 1;
                    thread::sleep(Duration::from_secs(delay));
                }
            }
        }
    }

    fn do_request<T: for<'de> Deserialize<'de> + Send>(
        &self,
        url: &str,
        body: &str,
    ) -> GenerationResult<Result<T, (u16, Option<u64>, String)>> {
        let Some(client) = &self.http_client else {
            return Err(GenerationError::Generic(
                "HTTP client not initialized".into(),
            ));
        };

        let mut builder = client
            .post(url)
            .map_err(|e| GenerationError::Backend(format!("Failed to create request: {e}")))?;
        for (k, v) in &self.build_auth_headers() {
            builder = builder.header(k.clone(), v.clone());
        }
        builder = builder.header(SimpleHeader::ACCEPT, String::from("application/json"));
        builder = builder.body_text(body.to_string());

        let request = client
            .request(builder)
            .map_err(|e| GenerationError::Backend(format!("Failed to build request: {e}")))?;

        let response = request
            .send()
            .map_err(|e| GenerationError::Backend(format!("Request failed: {e}")))?;

        let status_code: usize = response.get_status().into();
        let body_text = match response.get_body_ref() {
            SendSafeBody::Text(t) => t.clone(),
            SendSafeBody::Bytes(b) => String::from_utf8_lossy(b).to_string(),
            SendSafeBody::None
            | SendSafeBody::Stream(_)
            | SendSafeBody::ChunkedStream(_)
            | SendSafeBody::LineFeedStream(_) => String::new(),
        };

        if !(200..=299).contains(&status_code) {
            let detail = parse_anthropic_error(&body_text).unwrap_or_else(|| body_text.clone());
            let msg = format_http_error(status_code, &detail);
            return Ok(Err((status_code as u16, None, msg)));
        }

        serde_json::from_str(&body_text)
            .map(|v| Ok(v))
            .map_err(|e| GenerationError::Generic(format!("Parse error: {e}")))
    }
}

impl<R: DnsResolver + 'static> Model for AnthropicModel<R> {
    fn spec(&self) -> ModelSpec {
        ModelSpec {
            name: self.model_name.clone(),
            id: self.model_id.clone(),
            devices: None,
            model_location: None,
            lora_location: None,
        }
    }

    fn costing(&self) -> GenerationResult<UsageReport> {
        Ok(empty_usage_report())
    }

    fn generate(
        &self,
        interaction: ModelInteraction,
        specs: Option<ModelParams>,
    ) -> GenerationResult<Vec<Messages>> {
        let params = specs.unwrap_or_default();
        let request = build_anthropic_request(&self.model_name, &interaction, &params, false);

        let body = serde_json::to_string(&request)
            .map_err(|e| GenerationError::Generic(format!("Failed to serialize request: {e}")))?;

        let url = self.build_url("messages");
        let response: MessagesResponse = self.execute_request(&url, &body)?;

        let message = parse_response(&response, &self.model_id)?;
        Ok(vec![message])
    }

    fn stream(
        &self,
        interaction: ModelInteraction,
        specs: Option<ModelParams>,
    ) -> GenerationResult<impl StreamIterator<D = Messages, P = ModelState>> {
        let params = specs.unwrap_or_default();
        let request = build_anthropic_request(&self.model_name, &interaction, &params, true);

        let body = serde_json::to_string(&request)
            .map_err(|e| GenerationError::Generic(format!("Failed to serialize request: {e}")))?;

        let url = self.build_url("messages");

        let resolver = self
            .resolver
            .as_ref()
            .ok_or_else(|| GenerationError::Generic("DNS resolver not initialized".into()))?
            .clone();

        let task = ReconnectingEventSourceTask::connect(resolver, &url)
            .map_err(|e| GenerationError::Backend(format!("Failed to create SSE task: {e}")))?
            .with_header(
                SimpleHeader::from("x-api-key".to_string()),
                self.api_key
                    .as_ref()
                    .map(|k| k.get().clone())
                    .unwrap_or_default(),
            )
            .with_header(
                SimpleHeader::from("anthropic-version".to_string()),
                self.config.api_version.clone(),
            )
            .with_header(SimpleHeader::ACCEPT, String::from("text/event-stream"))
            .with_header(SimpleHeader::CONTENT_TYPE, String::from("application/json"))
            .with_body(SendSafeBody::Text(body));

        let driven = execute(task, None)
            .map_err(|e| GenerationError::Backend(format!("Executor error: {e}")))?;

        Ok(AnthropicStream {
            inner: driven,
            model_id: self.model_id.clone(),
            accumulated_text: String::new(),
            accumulated_thinking: String::new(),
            tool_calls: Vec::new(),
            stop_reason: None,
            usage: None,
            done: false,
        })
    }
}

// ============================================================================
// Streaming Parser
// ============================================================================

struct AccumulatedToolCall {
    id: String,
    name: String,
    arguments: String,
}

struct AnthropicStream<R: DnsResolver + 'static> {
    inner: foundation_core::valtron::DrivenStreamIterator<ReconnectingEventSourceTask<R>>,
    model_id: ModelId,
    accumulated_text: String,
    accumulated_thinking: String,
    tool_calls: Vec<AccumulatedToolCall>,
    stop_reason: Option<String>,
    usage: Option<AnthropicUsage>,
    done: bool,
}

impl<R: DnsResolver + Send + 'static> Iterator for AnthropicStream<R> {
    type Item = Stream<Messages, ModelState>;

    fn next(&mut self) -> Option<Self::Item> {
        if self.done {
            return None;
        }

        loop {
            let item = self.inner.next()?;

            match item {
                Stream::Next(parse_result) => {
                    let Event::Message { data, event_type, .. } = &parse_result.event else {
                        continue;
                    };

                    // Anthropic uses named events; skip if no event name
                    if event_type.as_ref().map_or(true, |e| e.is_empty()) {
                        continue;
                    }

                    let event_name = event_type.as_ref().map(String::as_str).unwrap_or("");

                    match event_name {
                        "message_start" => {
                            let Ok(StreamEvent::MessageStart { message }) =
                                serde_json::from_str::<StreamEvent>(data)
                            else {
                                continue;
                            };
                            self.usage = Some(message.usage);
                            return Some(Stream::Ignore);
                        }
                        "content_block_start" => {
                            // Track content block type for accumulation
                            let Ok(StreamEvent::ContentBlockStart { content_block, .. }) =
                                serde_json::from_str::<StreamEvent>(data)
                            else {
                                continue;
                            };
                            if let AnthropicContentBlock::ToolUse { id, name, input } =
                                content_block
                            {
                                self.tool_calls.push(AccumulatedToolCall {
                                    id,
                                    name,
                                    arguments: serde_json::to_string(&input)
                                        .unwrap_or_default(),
                                });
                            }
                            return Some(Stream::Ignore);
                        }
                        "content_block_delta" => {
                            let Ok(StreamEvent::ContentBlockDelta { delta, .. }) =
                                serde_json::from_str::<StreamEvent>(data)
                            else {
                                continue;
                            };
                            match delta {
                                AnthropicDelta::TextDelta { text } => {
                                    self.accumulated_text.push_str(&text);
                                    return Some(Stream::Next(Messages::Assistant {
                                        model: self.model_id.clone(),
                                        timestamp: SystemTime::now(),
                                        usage: empty_usage_report(),
                                        content: ModelOutput::Text(TextContent {
                                            content: self.accumulated_text.clone(),
                                            signature: None,
                                        }),
                                        stop_reason: StopReason::Stop,
                                        provider: ModelProviders::ANTHROPIC,
                                        error_detail: None,
                                        signature: None,
                                        metadata: None,
                                    }));
                                }
                                AnthropicDelta::ThinkingDelta { thinking } => {
                                    self.accumulated_thinking.push_str(&thinking);
                                    return Some(Stream::Next(Messages::Assistant {
                                        model: self.model_id.clone(),
                                        timestamp: SystemTime::now(),
                                        usage: empty_usage_report(),
                                        content: ModelOutput::ThinkingContent {
                                            thinking: self.accumulated_thinking.clone(),
                                            signature: None,
                                        },
                                        stop_reason: StopReason::Stop,
                                        provider: ModelProviders::ANTHROPIC,
                                        error_detail: None,
                                        signature: None,
                                        metadata: None,
                                    }));
                                }
                                AnthropicDelta::InputJsonDelta { partial_json } => {
                                    // Append to last tool call arguments
                                    if let Some(tc) = self.tool_calls.last_mut() {
                                        tc.arguments.push_str(&partial_json);
                                    }
                                    return Some(Stream::Ignore);
                                }
                            }
                        }
                        "content_block_stop" => {
                            return Some(Stream::Ignore);
                        }
                        "message_delta" => {
                            let Ok(StreamEvent::MessageDelta { delta, usage }) =
                                serde_json::from_str::<StreamEvent>(data)
                            else {
                                continue;
                            };
                            if let Some(reason) = delta.stop_reason {
                                self.stop_reason = Some(reason);
                            }
                            self.usage = Some(usage);
                            return Some(Stream::Ignore);
                        }
                        "message_stop" => {
                            self.done = true;
                            return Some(Stream::Next(self.build_final_message()));
                        }
                        "ping" => {
                            return Some(Stream::Ignore);
                        }
                        _ => {
                            // Unknown event, ignore
                            return Some(Stream::Ignore);
                        }
                    }
                }
                Stream::Pending(_) => {
                    return Some(Stream::Pending(ModelState::GeneratingTokens(None)));
                }
                Stream::Delayed(d) => return Some(Stream::Delayed(d)),
                Stream::Init => return Some(Stream::Init),
                Stream::Ignore => {}
            }
        }
    }
}

impl<R: DnsResolver + 'static> AnthropicStream<R> {
    fn build_final_message(&self) -> Messages {
        #[allow(clippy::cast_precision_loss)]
        let usage_report = self
            .usage
            .as_ref()
            .map_or_else(empty_usage_report, |u| UsageReport {
                input: u.input_tokens as f64,
                output: u.output_tokens as f64,
                cache_read: u.cache_read_input_tokens as f64,
                cache_write: u.cache_creation_input_tokens as f64,
                total_tokens: (u.input_tokens + u.output_tokens) as f64,
                cost: UsageCosting {
                    currency: String::from("USD"),
                    input: 0.0,
                    output: 0.0,
                    cache_read: 0.0,
                    cache_write: 0.0,
                    total_tokens: (u.input_tokens + u.output_tokens) as f64,
                },
            });

        let stop_reason = map_stop_reason(&self.stop_reason);

        let content = if !self.tool_calls.is_empty() {
            let tc = &self.tool_calls[0];
            let arguments: Option<HashMap<String, crate::types::ArgType>> =
                serde_json::from_str(&tc.arguments)
                    .ok()
                    .map(|v: serde_json::Value| {
                        v.as_object()
                            .map(|obj| {
                                obj.iter()
                                    .map(|(k, v)| (k.clone(), json_value_to_arg_type(v)))
                                    .collect()
                            })
                            .unwrap_or_default()
                    });

            ModelOutput::ToolCall {
                id: tc.id.clone(),
                name: tc.name.clone(),
                arguments,
                signature: None,
            }
        } else if !self.accumulated_thinking.is_empty() {
            ModelOutput::ThinkingContent {
                thinking: self.accumulated_thinking.clone(),
                signature: None,
            }
        } else {
            ModelOutput::Text(TextContent {
                content: self.accumulated_text.clone(),
                signature: None,
            })
        };

        Messages::Assistant {
            model: self.model_id.clone(),
            timestamp: SystemTime::now(),
            usage: usage_report,
            content,
            stop_reason,
            provider: ModelProviders::ANTHROPIC,
            error_detail: None,
            signature: None,
            metadata: None,
        }
    }
}

// ============================================================================
// Helpers
// ============================================================================

fn build_anthropic_request(
    model_name: &str,
    interaction: &ModelInteraction,
    params: &ModelParams,
    streaming: bool,
) -> MessagesRequest {
    let system = interaction
        .system_prompt
        .as_ref()
        .map(|s| AnthropicSystemContent::Text(s.clone()));

    let messages: Vec<AnthropicMessage> = interaction
        .messages
        .iter()
        .filter_map(|msg| match msg {
            Messages::User { content, .. } => match content {
                crate::types::UserModelContent::Text(tc) => Some(AnthropicMessage {
                    role: AnthropicRole::User,
                    content: vec![AnthropicContentBlock::Text {
                        text: tc.content.clone(),
                    }],
                }),
                crate::types::UserModelContent::Image(img) => {
                    let mime_str = match img.mime_type {
                        crate::types::MimeType::ImagePng => "image/png",
                        crate::types::MimeType::ImageJpeg => "image/jpeg",
                        crate::types::MimeType::ImageGif => "image/gif",
                        crate::types::MimeType::ImageWebp => "image/webp",
                        _ => "image/png",
                    };
                    Some(AnthropicMessage {
                        role: AnthropicRole::User,
                        content: vec![AnthropicContentBlock::Image {
                            source: AnthropicImageSource {
                                source_type: String::from("base64"),
                                media_type: mime_str.to_string(),
                                data: img.b64.clone(),
                            },
                        }],
                    })
                }
            },
            Messages::Assistant { content, .. } => match content {
                ModelOutput::Text(tc) => Some(AnthropicMessage {
                    role: AnthropicRole::Assistant,
                    content: vec![AnthropicContentBlock::Text {
                        text: tc.content.clone(),
                    }],
                }),
                ModelOutput::ToolCall {
                    id,
                    name,
                    arguments,
                    ..
                } => {
                    let input = arguments
                        .as_ref()
                        .map(|args| serde_json::to_value(args).unwrap_or(serde_json::Value::Null))
                        .unwrap_or(serde_json::Value::Null);
                    Some(AnthropicMessage {
                        role: AnthropicRole::Assistant,
                        content: vec![AnthropicContentBlock::ToolUse {
                            id: id.clone(),
                            name: name.clone(),
                            input,
                        }],
                    })
                }
                ModelOutput::ThinkingContent { thinking, .. } => Some(AnthropicMessage {
                    role: AnthropicRole::Assistant,
                    content: vec![AnthropicContentBlock::Text {
                        text: thinking.clone(),
                    }],
                }),
                ModelOutput::Image(_) | ModelOutput::Embedding { .. } => None,
            },
            Messages::ToolResult {
                id, content, ..
            } => {
                let text = match content {
                    crate::types::UserModelContent::Text(tc) => tc.content.clone(),
                    crate::types::UserModelContent::Image(_) => String::from("[Image]"),
                };
                Some(AnthropicMessage {
                    role: AnthropicRole::User,
                    content: vec![AnthropicContentBlock::ToolResult {
                        tool_use_id: id.clone(),
                        content: text,
                        is_error: None,
                    }],
                })
            }
        })
        .collect();

    let tools = if interaction.tools.is_empty() {
        None
    } else {
        Some(
            interaction
                .tools
                .iter()
                .map(|tool| {
                    let mut properties = serde_json::Map::new();
                    if let Some(args) = &tool.arguments {
                        for (key, value) in args {
                            let schema_type = match value {
                                crate::types::ArgType::Float32(_)
                                | crate::types::ArgType::Float64(_) => "number",
                                crate::types::ArgType::Usize(_)
                                | crate::types::ArgType::U8(_)
                                | crate::types::ArgType::U16(_)
                                | crate::types::ArgType::U32(_)
                                | crate::types::ArgType::U64(_)
                                | crate::types::ArgType::Isize(_)
                                | crate::types::ArgType::I8(_)
                                | crate::types::ArgType::I16(_)
                                | crate::types::ArgType::I32(_)
                                | crate::types::ArgType::I64(_) => "integer",
                                _ => "string",
                            };
                            properties.insert(
                                key.clone(),
                                serde_json::json!({ "type": schema_type }),
                            );
                        }
                    }
                    AnthropicTool {
                        name: tool.name.clone(),
                        description: tool.description.clone(),
                        input_schema: serde_json::json!({
                            "type": "object",
                            "properties": properties,
                        }),
                    }
                })
                .collect(),
        )
    };

    let tool_choice = interaction.tool_choice.as_ref().map(|tc| match tc {
        crate::types::ToolChoice::Auto => AnthropicToolChoice::Auto,
        crate::types::ToolChoice::None => AnthropicToolChoice::Auto, // Anthropic doesn't have "none"
        crate::types::ToolChoice::Required => AnthropicToolChoice::Any,
        crate::types::ToolChoice::Function(f) => AnthropicToolChoice::Tool {
            name: f.function.name.clone(),
        },
    });

    // Anthropic requires max_tokens — use a sensible default
    #[allow(clippy::cast_possible_truncation)]
    let max_tokens = if params.max_tokens > 0 {
        params.max_tokens as u32
    } else {
        4096
    };

    MessagesRequest {
        model: model_name.to_string(),
        system,
        messages,
        max_tokens,
        temperature: if params.temperature > 0.0 {
            Some(params.temperature as f64)
        } else {
            None
        },
        top_p: if params.top_p > 0.0 && params.top_p < 1.0 {
            Some(params.top_p as f64)
        } else {
            None
        },
        top_k: if params.top_k > 0.0 {
            Some(params.top_k as usize)
        } else {
            None
        },
        stream: Some(streaming),
        stop_sequences: if params.stop_tokens.is_empty() {
            None
        } else {
            Some(params.stop_tokens.clone())
        },
        tools,
        tool_choice,
        thinking: None,
    }
}

fn parse_response(
    response: &MessagesResponse,
    model_id: &ModelId,
) -> GenerationResult<Messages> {
    let stop_reason = map_stop_reason(&response.stop_reason);

    #[allow(clippy::cast_precision_loss)]
    let usage_report = UsageReport {
        input: response.usage.input_tokens as f64,
        output: response.usage.output_tokens as f64,
        cache_read: response.usage.cache_read_input_tokens as f64,
        cache_write: response.usage.cache_creation_input_tokens as f64,
        total_tokens: (response.usage.input_tokens + response.usage.output_tokens) as f64,
        cost: UsageCosting {
            currency: String::from("USD"),
            input: 0.0,
            output: 0.0,
            cache_read: 0.0,
            cache_write: 0.0,
            total_tokens: (response.usage.input_tokens + response.usage.output_tokens) as f64,
        },
    };

    let content = extract_content(&response.content);

    Ok(Messages::Assistant {
        model: model_id.clone(),
        timestamp: SystemTime::now(),
        usage: usage_report,
        content,
        stop_reason,
        provider: ModelProviders::ANTHROPIC,
        error_detail: None,
        signature: None,
        metadata: None,
    })
}

fn extract_content(blocks: &[AnthropicContentBlock]) -> ModelOutput {
    for block in blocks {
        match block {
            AnthropicContentBlock::Text { text } => {
                return ModelOutput::Text(TextContent {
                    content: text.clone(),
                    signature: None,
                });
            }
            AnthropicContentBlock::ToolUse {
                id,
                name,
                input,
            } => {
                let arguments: Option<HashMap<String, crate::types::ArgType>> =
                    serde_json::from_value(input.clone())
                        .ok()
                        .map(|v: serde_json::Value| {
                            v.as_object()
                                .map(|obj| {
                                    obj.iter()
                                        .map(|(k, v)| (k.clone(), json_value_to_arg_type(v)))
                                        .collect()
                                })
                                .unwrap_or_default()
                        });

                return ModelOutput::ToolCall {
                    id: id.clone(),
                    name: name.clone(),
                    arguments,
                    signature: None,
                };
            }
            AnthropicContentBlock::Thinking {
                thinking,
                signature,
            } => {
                return ModelOutput::ThinkingContent {
                    thinking: thinking.clone(),
                    signature: Some(signature.clone()),
                };
            }
            AnthropicContentBlock::RedactedThinking { .. } => {
                return ModelOutput::ThinkingContent {
                    thinking: String::from("[redacted]"),
                    signature: None,
                };
            }
            AnthropicContentBlock::Image { .. } | AnthropicContentBlock::ToolResult { .. } => {
                // Skip non-assistant blocks
            }
        }
    }
    ModelOutput::Text(TextContent {
        content: String::new(),
        signature: None,
    })
}

fn map_stop_reason(reason: &Option<String>) -> StopReason {
    match reason.as_deref() {
        Some("end_turn") => StopReason::Stop,
        Some("stop_sequence") => StopReason::Stop,
        Some("max_tokens") => StopReason::Length,
        Some("tool_use") => StopReason::ToolUse,
        Some(other) => StopReason::Message(other.to_string()),
        None => StopReason::Stop,
    }
}

fn empty_usage_report() -> UsageReport {
    UsageReport {
        input: 0.0,
        output: 0.0,
        cache_read: 0.0,
        cache_write: 0.0,
        total_tokens: 0.0,
        cost: UsageCosting {
            currency: String::from("USD"),
            input: 0.0,
            output: 0.0,
            cache_read: 0.0,
            cache_write: 0.0,
            total_tokens: 0.0,
        },
    }
}

fn json_value_to_arg_type(v: &serde_json::Value) -> crate::types::ArgType {
    match v {
        serde_json::Value::String(s) => crate::types::ArgType::Text(s.clone()),
        serde_json::Value::Number(n) => {
            if let Some(i) = n.as_i64() {
                crate::types::ArgType::I64(i)
            } else if let Some(f) = n.as_f64() {
                crate::types::ArgType::Float64(f)
            } else {
                crate::types::ArgType::Text(n.to_string())
            }
        }
        other => crate::types::ArgType::JSON(other.to_string()),
    }
}

fn model_id_to_string(id: &ModelId) -> String {
    match id {
        ModelId::Name(name, _) => name.clone(),
        ModelId::Alias(alias, _) => alias.clone(),
        ModelId::Group(group, _) => group.clone(),
        ModelId::Architecture(arch, _) => arch.clone(),
    }
}

fn is_retryable_status(status: u16) -> bool {
    status == 429 || (500..=503).contains(&status)
}

fn exponential_backoff(attempt: u32) -> u64 {
    let base_secs: u64 = 1 << attempt.min(5);
    base_secs.min(30)
}

fn extract_retry_after(headers: &SimpleHeaders) -> Option<u64> {
    let header = SimpleHeader::from("Retry-After".to_string());
    headers
        .get(&header)
        .and_then(|values| values.first())
        .and_then(|v| v.parse::<u64>().ok())
}

fn parse_anthropic_error(body: &str) -> Option<String> {
    #[derive(Deserialize)]
    struct AnthropicErrorResponse {
        error: AnthropicErrorDetail,
    }
    #[derive(Deserialize)]
    struct AnthropicErrorDetail {
        #[serde(rename = "type")]
        error_type: Option<String>,
        message: String,
    }

    serde_json::from_str::<AnthropicErrorResponse>(body)
        .ok()
        .map(|e| {
            if let Some(error_type) = e.error.error_type {
                format!("[{error_type}] {}", e.error.message)
            } else {
                e.error.message
            }
        })
}

fn format_http_error(status_code: usize, detail: &str) -> String {
    match status_code {
        401 => format!("Authentication failed: {detail}"),
        403 => format!("Permission denied: {detail}"),
        404 => format!("Not found: {detail}"),
        429 => format!("Rate limit exceeded: {detail}"),
        500..=503 => format!("Server error (HTTP {status_code}): {detail}"),
        _ => format!("HTTP {status_code}: {detail}"),
    }
}

// ============================================================================
// Tests
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_anthropic_config_defaults() {
        let config = AnthropicConfig::default();
        assert_eq!(config.base_url, "https://api.anthropic.com");
        assert_eq!(config.api_version, "2023-06-01");
        assert_eq!(config.timeout_secs, 120);
        assert_eq!(config.max_retries, 3);
        assert!(config.proxy_url.is_none());
        assert!(config.streaming);
    }

    #[test]
    fn test_anthropic_config_builder() {
        let config = AnthropicConfig::new()
            .with_base_url("http://localhost:8080")
            .with_api_version("2024-01-01")
            .with_timeout_secs(60)
            .with_streaming(false);
        assert_eq!(config.base_url, "http://localhost:8080");
        assert_eq!(config.api_version, "2024-01-01");
        assert_eq!(config.timeout_secs, 60);
        assert!(!config.streaming);
    }

    #[test]
    fn test_build_url() {
        let config = AnthropicConfig::new()
            .with_base_url("http://localhost:8080")
            .with_api_version("v1");
        assert_eq!(
            config.build_url("messages"),
            "http://localhost:8080/v1/messages"
        );
    }

    #[test]
    fn test_messages_request_serialization() {
        let request = MessagesRequest {
            model: "claude-3-5-sonnet-20241022".into(),
            system: Some(AnthropicSystemContent::Text("Be helpful".into())),
            messages: vec![AnthropicMessage {
                role: AnthropicRole::User,
                content: vec![AnthropicContentBlock::Text {
                    text: "Hello".into(),
                }],
            }],
            max_tokens: 1024,
            temperature: Some(0.7),
            top_p: Some(0.9),
            top_k: None,
            stream: Some(false),
            stop_sequences: None,
            tools: None,
            tool_choice: None,
            thinking: None,
        };

        let json = serde_json::to_string(&request).unwrap();
        assert!(json.contains(r#""model":"claude-3-5-sonnet-20241022""#));
        assert!(json.contains(r#""max_tokens":1024"#));
        assert!(json.contains(r#""temperature":0.7"#));
        assert!(json.contains(r#""stream":false"#));
        assert!(json.contains(r#""text":"Hello""#));
    }

    #[test]
    fn test_system_content_blocks() {
        let request = MessagesRequest {
            model: "claude-3-5-sonnet-20241022".into(),
            system: Some(AnthropicSystemContent::Blocks(vec![
                AnthropicSystemBlock::Text {
                    text: "You are an expert".into(),
                },
            ])),
            messages: vec![],
            max_tokens: 1024,
            temperature: None,
            top_p: None,
            top_k: None,
            stream: Some(false),
            stop_sequences: None,
            tools: None,
            tool_choice: None,
            thinking: None,
        };

        let json = serde_json::to_string(&request).unwrap();
        assert!(json.contains(r#""type":"text""#));
        assert!(json.contains("You are an expert"));
    }

    #[test]
    fn test_message_content_block_serialization() {
        let msg = AnthropicMessage {
            role: AnthropicRole::User,
            content: vec![AnthropicContentBlock::Text {
                text: "Hello".into(),
            }],
        };
        let json = serde_json::to_string(&msg).unwrap();
        assert!(json.contains(r#""role":"user""#));
        assert!(json.contains(r#""type":"text""#));
        assert!(json.contains(r#""text":"Hello""#));
    }

    #[test]
    fn test_image_content_block() {
        let msg = AnthropicMessage {
            role: AnthropicRole::User,
            content: vec![AnthropicContentBlock::Image {
                source: AnthropicImageSource {
                    source_type: "base64".into(),
                    media_type: "image/png".into(),
                    data: "iVBORw0KGgo".into(),
                },
            }],
        };
        let json = serde_json::to_string(&msg).unwrap();
        assert!(json.contains(r#""type":"image""#));
        assert!(json.contains(r#""media_type":"image/png""#));
        assert!(json.contains("iVBORw0KGgo"));
    }

    #[test]
    fn test_tool_use_content_block() {
        let msg = AnthropicMessage {
            role: AnthropicRole::Assistant,
            content: vec![AnthropicContentBlock::ToolUse {
                id: "tool_123".into(),
                name: "get_weather".into(),
                input: serde_json::json!({"location": "Paris"}),
            }],
        };
        let json = serde_json::to_string(&msg).unwrap();
        assert!(json.contains(r#""type":"tool_use""#));
        assert!(json.contains(r#""name":"get_weather""#));
        assert!(json.contains("Paris"));
    }

    #[test]
    fn test_tool_result_content_block() {
        let msg = AnthropicMessage {
            role: AnthropicRole::User,
            content: vec![AnthropicContentBlock::ToolResult {
                tool_use_id: "tool_123".into(),
                content: "Sunny, 25°C".into(),
                is_error: None,
            }],
        };
        let json = serde_json::to_string(&msg).unwrap();
        assert!(json.contains(r#""type":"tool_result""#));
        assert!(json.contains(r#""tool_use_id":"tool_123""#));
        assert!(json.contains("Sunny"));
    }

    #[test]
    fn test_anthropic_tool_choice_serialization() {
        let tc = AnthropicToolChoice::Auto;
        let json = serde_json::to_string(&tc).unwrap();
        assert_eq!(json, r#"{"type":"auto"}"#);

        let tc = AnthropicToolChoice::Any;
        let json = serde_json::to_string(&tc).unwrap();
        assert_eq!(json, r#"{"type":"any"}"#);

        let tc = AnthropicToolChoice::Tool {
            name: "get_weather".into(),
        };
        let json = serde_json::to_string(&tc).unwrap();
        assert!(json.contains(r#""type":"tool""#));
        assert!(json.contains(r#""name":"get_weather""#));
    }

    #[test]
    fn test_anthropic_tool_definition() {
        let tool = AnthropicTool {
            name: "get_weather".into(),
            description: "Get weather for a location".into(),
            input_schema: serde_json::json!({
                "type": "object",
                "properties": {
                    "location": {"type": "string"}
                }
            }),
        };
        let json = serde_json::to_string(&tool).unwrap();
        assert!(json.contains(r#""name":"get_weather""#));
        assert!(json.contains("Get weather"));
        assert!(json.contains("location"));
    }

    #[test]
    fn test_messages_response_deserialization() {
        let json = r#"{
            "id": "msg_abc123",
            "type": "message",
            "role": "assistant",
            "content": [{"type": "text", "text": "Hello!"}],
            "model": "claude-3-5-sonnet-20241022",
            "stop_reason": "end_turn",
            "stop_sequence": null,
            "usage": {"input_tokens": 10, "output_tokens": 5, "cache_creation_input_tokens": 0, "cache_read_input_tokens": 0}
        }"#;

        let response: MessagesResponse = serde_json::from_str(json).unwrap();
        assert_eq!(response.id, "msg_abc123");
        assert_eq!(response.role, "assistant");
        assert_eq!(response.stop_reason.as_deref(), Some("end_turn"));
        assert_eq!(response.usage.input_tokens, 10);
        assert_eq!(response.usage.output_tokens, 5);
    }

    #[test]
    fn test_response_with_tool_use() {
        let json = r#"{
            "id": "msg_abc123",
            "type": "message",
            "role": "assistant",
            "content": [{"type": "tool_use", "id": "tool_1", "name": "get_weather", "input": {"location": "Paris"}}],
            "model": "claude-3-5-sonnet-20241022",
            "stop_reason": "tool_use",
            "stop_sequence": null,
            "usage": {"input_tokens": 50, "output_tokens": 20, "cache_creation_input_tokens": 0, "cache_read_input_tokens": 0}
        }"#;

        let response: MessagesResponse = serde_json::from_str(json).unwrap();
        assert_eq!(response.stop_reason.as_deref(), Some("tool_use"));
        match &response.content[0] {
            AnthropicContentBlock::ToolUse { id, name, input } => {
                assert_eq!(id, "tool_1");
                assert_eq!(name, "get_weather");
                assert!(input.get("location").is_some());
            }
            _ => panic!("Expected ToolUse content block"),
        }
    }

    #[test]
    fn test_stream_event_deserialization() {
        let json = r#"{
            "type": "message_start",
            "message": {
                "id": "msg_abc",
                "type": "message",
                "role": "assistant",
                "content": [],
                "model": "claude-3-5-sonnet-20241022",
                "stop_reason": null,
                "stop_sequence": null,
                "usage": {"input_tokens": 10, "output_tokens": 0, "cache_creation_input_tokens": 0, "cache_read_input_tokens": 0}
            }
        }"#;

        let event: StreamEvent = serde_json::from_str(json).unwrap();
        match event {
            StreamEvent::MessageStart { message } => {
                assert_eq!(message.id, "msg_abc");
            }
            _ => panic!("Expected MessageStart"),
        }
    }

    #[test]
    fn test_content_block_delta_deserialization() {
        let json = r#"{
            "type": "content_block_delta",
            "index": 0,
            "delta": {"type": "text_delta", "text": "Hello"}
        }"#;

        let event: StreamEvent = serde_json::from_str(json).unwrap();
        match event {
            StreamEvent::ContentBlockDelta { delta, .. } => match delta {
                AnthropicDelta::TextDelta { text } => {
                    assert_eq!(text, "Hello");
                }
                _ => panic!("Expected TextDelta"),
            },
            _ => panic!("Expected ContentBlockDelta"),
        }
    }

    #[test]
    fn test_message_delta_deserialization() {
        let json = r#"{
            "type": "message_delta",
            "delta": {"stop_reason": "end_turn"},
            "usage": {"input_tokens": 10, "output_tokens": 15, "cache_creation_input_tokens": 0, "cache_read_input_tokens": 0}
        }"#;

        let event: StreamEvent = serde_json::from_str(json).unwrap();
        match event {
            StreamEvent::MessageDelta { delta, usage } => {
                assert_eq!(delta.stop_reason.as_deref(), Some("end_turn"));
                assert_eq!(usage.output_tokens, 15);
            }
            _ => panic!("Expected MessageDelta"),
        }
    }

    #[test]
    fn test_parse_response_text() {
        let response = MessagesResponse {
            id: "msg_123".into(),
            response_type: "message".into(),
            role: "assistant".into(),
            content: vec![AnthropicContentBlock::Text {
                text: "Hello!".into(),
            }],
            model: "claude-3-5-sonnet-20241022".into(),
            stop_reason: Some("end_turn".into()),
            stop_sequence: None,
            usage: AnthropicUsage {
                input_tokens: 10,
                output_tokens: 5,
                cache_creation_input_tokens: 0,
                cache_read_input_tokens: 0,
            },
        };

        let model_id = ModelId::Name("claude-3-5-sonnet".into(), None);
        let msg = parse_response(&response, &model_id).unwrap();

        match &msg {
            Messages::Assistant {
                content,
                stop_reason,
                provider,
                ..
            } => {
                assert_eq!(*stop_reason, StopReason::Stop);
                assert_eq!(*provider, ModelProviders::ANTHROPIC);
                if let ModelOutput::Text(tc) = content {
                    assert_eq!(tc.content, "Hello!");
                } else {
                    panic!("Expected Text output");
                }
            }
            _ => panic!("Expected Assistant message"),
        }
    }

    #[test]
    fn test_parse_response_tool_use() {
        let response = MessagesResponse {
            id: "msg_123".into(),
            response_type: "message".into(),
            role: "assistant".into(),
            content: vec![AnthropicContentBlock::ToolUse {
                id: "tool_1".into(),
                name: "get_weather".into(),
                input: serde_json::json!({"location": "Paris"}),
            }],
            model: "claude-3-5-sonnet-20241022".into(),
            stop_reason: Some("tool_use".into()),
            stop_sequence: None,
            usage: AnthropicUsage {
                input_tokens: 50,
                output_tokens: 20,
                cache_creation_input_tokens: 0,
                cache_read_input_tokens: 0,
            },
        };

        let model_id = ModelId::Name("claude-3-5-sonnet".into(), None);
        let msg = parse_response(&response, &model_id).unwrap();

        match &msg {
            Messages::Assistant {
                content,
                stop_reason,
                ..
            } => {
                assert_eq!(*stop_reason, StopReason::ToolUse);
                if let ModelOutput::ToolCall { id, name, .. } = content {
                    assert_eq!(id, "tool_1");
                    assert_eq!(name, "get_weather");
                } else {
                    panic!("Expected ToolCall output");
                }
            }
            _ => panic!("Expected Assistant message"),
        }
    }

    #[test]
    fn test_stop_reason_mapping() {
        assert_eq!(map_stop_reason(&Some("end_turn".into())), StopReason::Stop);
        assert_eq!(
            map_stop_reason(&Some("stop_sequence".into())),
            StopReason::Stop
        );
        assert_eq!(
            map_stop_reason(&Some("max_tokens".into())),
            StopReason::Length
        );
        assert_eq!(
            map_stop_reason(&Some("tool_use".into())),
            StopReason::ToolUse
        );
        assert_eq!(
            map_stop_reason(&Some("unknown_reason".into())),
            StopReason::Message("unknown_reason".into())
        );
        assert_eq!(map_stop_reason(&None), StopReason::Stop);
    }

    #[test]
    fn test_build_anthropic_request_basic() {
        let interaction = ModelInteraction {
            system_prompt: Some("You are helpful".into()),
            messages: vec![Messages::User {
                role: "user".into(),
                content: crate::types::UserModelContent::Text(TextContent {
                    content: "Hello".into(),
                    signature: None,
                }),
                signature: None,
            }],
            tools: vec![],
            chat_template: None,
            tool_choice: None,
        };
        let params = ModelParams::default();

        let request = build_anthropic_request("claude-3-5-sonnet", &interaction, &params, false);

        assert_eq!(request.model, "claude-3-5-sonnet");
        assert!(request.system.is_some());
        assert_eq!(request.messages.len(), 1);
        assert!(matches!(request.messages[0].role, AnthropicRole::User));
        assert_eq!(request.max_tokens, 2048);
        assert_eq!(request.stream, Some(false));
    }

    #[test]
    fn test_build_anthropic_request_with_tools() {
        use crate::types::{ArgType, Tool};

        let interaction = ModelInteraction {
            system_prompt: None,
            messages: vec![],
            tools: vec![Tool {
                id: "tool_1".into(),
                name: "get_weather".into(),
                description: "Get weather info".into(),
                arguments: Some(HashMap::from([(
                    "location".into(),
                    ArgType::Text("Paris".into()),
                )])),
                returns: None,
            }],
            chat_template: None,
            tool_choice: None,
        };
        let params = ModelParams::default();

        let request = build_anthropic_request("claude-3-5-sonnet", &interaction, &params, false);

        assert!(request.tools.is_some());
        let tools = request.tools.unwrap();
        assert_eq!(tools.len(), 1);
        assert_eq!(tools[0].name, "get_weather");
        assert_eq!(tools[0].description, "Get weather info");
    }

    #[test]
    fn test_build_anthropic_request_image() {
        use crate::types::{ImageContent, MimeType, UserModelContent};

        let interaction = ModelInteraction {
            system_prompt: None,
            messages: vec![Messages::User {
                role: "user".into(),
                content: UserModelContent::Image(ImageContent {
                    b64: "base64data".into(),
                    mime_type: MimeType::ImagePng,
                }),
                signature: None,
            }],
            tools: vec![],
            chat_template: None,
            tool_choice: None,
        };
        let params = ModelParams::default();

        let request = build_anthropic_request("claude-3-5-sonnet", &interaction, &params, false);

        assert_eq!(request.messages.len(), 1);
        match &request.messages[0].content[0] {
            AnthropicContentBlock::Image { source } => {
                assert_eq!(source.media_type, "image/png");
                assert_eq!(source.data, "base64data");
            }
            _ => panic!("Expected Image content block"),
        }
    }

    #[test]
    fn test_build_anthropic_request_tool_result() {
        use crate::types::{Messages, UserModelContent};

        let interaction = ModelInteraction {
            system_prompt: None,
            messages: vec![
                Messages::Assistant {
                    model: ModelId::Name("test".into(), None),
                    timestamp: SystemTime::now(),
                    usage: empty_usage_report(),
                    content: ModelOutput::ToolCall {
                        id: "tool_1".into(),
                        name: "get_weather".into(),
                        arguments: None,
                        signature: None,
                    },
                    stop_reason: StopReason::ToolUse,
                    provider: ModelProviders::ANTHROPIC,
                    error_detail: None,
                    signature: None,
                    metadata: None,
                },
                Messages::ToolResult {
                    id: "tool_1".into(),
                    name: "get_weather".into(),
                    timestamp: SystemTime::now(),
                    details: None,
                    content: UserModelContent::Text(TextContent {
                        content: "Sunny".into(),
                        signature: None,
                    }),
                    error_detail: None,
                    signature: None,
                },
            ],
            tools: vec![],
            chat_template: None,
            tool_choice: None,
        };
        let params = ModelParams::default();

        let request = build_anthropic_request("claude-3-5-sonnet", &interaction, &params, false);

        assert_eq!(request.messages.len(), 2);
        // First message should be assistant with ToolUse
        assert!(matches!(
            request.messages[0].content[0],
            AnthropicContentBlock::ToolUse { .. }
        ));
        // Second message should be user with ToolResult
        assert!(matches!(
            request.messages[1].content[0],
            AnthropicContentBlock::ToolResult { .. }
        ));
    }

    #[test]
    fn test_anthropic_error_parsing() {
        let body = r#"{"error":{"type":"invalid_request_error","message":"Invalid API key"}}"#;
        let detail = parse_anthropic_error(body).expect("Should parse Anthropic error");
        assert!(detail.contains("invalid_request_error"));
        assert!(detail.contains("Invalid API key"));
    }

    #[test]
    fn test_anthropic_error_parsing_no_type() {
        let body = r#"{"error":{"message":"Something went wrong"}}"#;
        let detail = parse_anthropic_error(body).expect("Should parse Anthropic error");
        assert_eq!(detail, "Something went wrong");
    }

    #[test]
    fn test_anthropic_error_plain_text_fallback() {
        let detail = parse_anthropic_error("Internal Server Error");
        assert!(detail.is_none());
    }

    #[test]
    fn test_is_retryable_status() {
        assert!(is_retryable_status(429));
        assert!(is_retryable_status(500));
        assert!(is_retryable_status(502));
        assert!(is_retryable_status(503));
        assert!(!is_retryable_status(400));
        assert!(!is_retryable_status(401));
        assert!(!is_retryable_status(404));
        assert!(!is_retryable_status(200));
    }

    #[test]
    fn test_exponential_backoff() {
        assert_eq!(exponential_backoff(0), 1);
        assert_eq!(exponential_backoff(1), 2);
        assert_eq!(exponential_backoff(2), 4);
        assert_eq!(exponential_backoff(3), 8);
        assert_eq!(exponential_backoff(5), 30); // capped
        assert_eq!(exponential_backoff(10), 30); // capped
    }

    #[test]
    fn test_format_http_errors() {
        assert!(format_http_error(401, "bad").contains("Authentication"));
        assert!(format_http_error(403, "bad").contains("Permission"));
        assert!(format_http_error(429, "bad").contains("Rate limit"));
        assert!(format_http_error(500, "bad").contains("Server error"));
    }
}
