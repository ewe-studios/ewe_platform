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
use foundation_core::wire::simple_http::{SendSafeBody, SimpleHeader};
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
    /// Override the default `/{api_version}/messages` endpoint path.
    /// When set, this path is appended directly to `base_url` (with a `/` separator).
    /// Useful for proxies that expose a different path, e.g. `/v1/messages`.
    pub messages_endpoint: Option<String>,
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
            messages_endpoint: None,
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
    pub fn with_messages_endpoint(mut self, endpoint: impl Into<String>) -> Self {
        self.messages_endpoint = Some(endpoint.into());
        self
    }

    #[must_use]
    pub fn build_url(&self, endpoint: &str) -> String {
        let path = self
            .messages_endpoint
            .as_deref()
            .unwrap_or(endpoint)
            .trim_start_matches('/');
        if self.messages_endpoint.is_some() {
            // Custom endpoint: append directly to base_url.
            format!(
                "{}/{}",
                self.base_url.trim_end_matches('/'),
                path
            )
        } else {
            // Default: /{api_version}/{endpoint}
            format!(
                "{}/{}/{}",
                self.base_url.trim_end_matches('/'),
                self.api_version,
                path
            )
        }
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
            messages_endpoint: self.messages_endpoint.clone(),
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
        Ok(message)
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
            final_messages: Vec::new(),
            final_message_index: 0,
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
    final_messages: Vec<Messages>,
    final_message_index: usize,
}

impl<R: DnsResolver + Send + 'static> Iterator for AnthropicStream<R> {
    type Item = Stream<Messages, ModelState>;

    fn next(&mut self) -> Option<Self::Item> {
        // Drain buffered final messages first.
        if self.final_message_index < self.final_messages.len() {
            let msg = self.final_messages[self.final_message_index].clone();
            self.final_message_index += 1;
            if self.final_message_index >= self.final_messages.len() {
                self.done = true;
            }
            return Some(Stream::Next(msg));
        }

        if self.done {
            return None;
        }

        let Some(item) = self.inner.next() else {
            // Inner stream closed.
            self.done = true;
            return None;
        };

        match item {
            Stream::Next(parse_result) => {
                let Event::Message { data, event_type, .. } = &parse_result.event else {
                    return Some(Stream::Ignore);
                };

                // Anthropic uses named events; skip if no event name
                let event_name = event_type.as_ref().map(String::as_str).unwrap_or("");
                if event_name.is_empty() {
                    return Some(Stream::Ignore);
                }

                match event_name {
                    "message_start" => {
                        let Ok(StreamEvent::MessageStart { message }) =
                            serde_json::from_str::<StreamEvent>(data)
                        else {
                            return Some(Stream::Ignore);
                        };
                        self.usage = Some(message.usage);
                        Some(Stream::Ignore)
                    }
                    "content_block_start" => {
                        let Ok(StreamEvent::ContentBlockStart { content_block, .. }) =
                            serde_json::from_str::<StreamEvent>(data)
                        else {
                            return Some(Stream::Ignore);
                        };
                        if let AnthropicContentBlock::ToolUse { id, name, .. } = content_block {
                            self.tool_calls.push(AccumulatedToolCall {
                                id,
                                name,
                                arguments: String::new(),
                            });
                        }
                        Some(Stream::Ignore)
                    }
                    "content_block_delta" => {
                        let Ok(StreamEvent::ContentBlockDelta { delta, .. }) =
                            serde_json::from_str::<StreamEvent>(data)
                        else {
                            return Some(Stream::Ignore);
                        };
                        match delta {
                            AnthropicDelta::TextDelta { text } => {
                                self.accumulated_text.push_str(&text);
                                Some(Stream::Next(Messages::Assistant {
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
                                }))
                            }
                            AnthropicDelta::ThinkingDelta { thinking } => {
                                self.accumulated_thinking.push_str(&thinking);
                                Some(Stream::Next(Messages::Assistant {
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
                                }))
                            }
                            AnthropicDelta::InputJsonDelta { partial_json } => {
                                if let Some(tc) = self.tool_calls.last_mut() {
                                    tc.arguments.push_str(&partial_json);
                                }
                                Some(Stream::Ignore)
                            }
                        }
                    }
                    "content_block_stop" => Some(Stream::Ignore),
                    "message_delta" => {
                        let Ok(StreamEvent::MessageDelta { delta, usage }) =
                            serde_json::from_str::<StreamEvent>(data)
                        else {
                            return Some(Stream::Ignore);
                        };
                        if let Some(reason) = delta.stop_reason {
                            self.stop_reason = Some(reason);
                        }
                        self.usage = Some(usage);
                        Some(Stream::Ignore)
                    }
                    "message_stop" => {
                        self.final_messages = self.build_final_messages();
                        self.final_message_index = 0;
                        Some(Stream::Ignore)
                    }
                    "ping" | _ => Some(Stream::Ignore),
                }
            }
            Stream::Pending(_) => {
                Some(Stream::Pending(ModelState::GeneratingTokens(None)))
            }
            Stream::Delayed(d) => Some(Stream::Delayed(d)),
            Stream::Init => Some(Stream::Init),
            Stream::Ignore => Some(Stream::Ignore),
        }
    }
}

impl<R: DnsResolver + 'static> AnthropicStream<R> {
    fn build_final_messages(&self) -> Vec<Messages> {
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

        let mut messages = Vec::new();

        // Emit thinking as a separate message if accumulated.
        if !self.accumulated_thinking.is_empty() {
            messages.push(Messages::Assistant {
                model: self.model_id.clone(),
                timestamp: SystemTime::now(),
                usage: usage_report.clone(),
                content: ModelOutput::ThinkingContent {
                    thinking: self.accumulated_thinking.clone(),
                    signature: None,
                },
                stop_reason: stop_reason.clone(),
                provider: ModelProviders::ANTHROPIC,
                error_detail: None,
                signature: None,
                metadata: None,
            });
        }

        // Emit text as a separate message if accumulated.
        if !self.accumulated_text.is_empty() {
            messages.push(Messages::Assistant {
                model: self.model_id.clone(),
                timestamp: SystemTime::now(),
                usage: usage_report.clone(),
                content: ModelOutput::Text(TextContent {
                    content: self.accumulated_text.clone(),
                    signature: None,
                }),
                stop_reason: stop_reason.clone(),
                provider: ModelProviders::ANTHROPIC,
                error_detail: None,
                signature: None,
                metadata: None,
            });
        }

        // Emit each tool call as a separate message.
        for tc in &self.tool_calls {
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

            messages.push(Messages::Assistant {
                model: self.model_id.clone(),
                timestamp: SystemTime::now(),
                usage: usage_report.clone(),
                content: ModelOutput::ToolCall {
                    id: tc.id.clone(),
                    name: tc.name.clone(),
                    arguments,
                    signature: None,
                },
                stop_reason: stop_reason.clone(),
                provider: ModelProviders::ANTHROPIC,
                error_detail: None,
                signature: None,
                metadata: None,
            });
        }

        // If nothing accumulated, return a single empty text message.
        if messages.is_empty() {
            messages.push(Messages::Assistant {
                model: self.model_id.clone(),
                timestamp: SystemTime::now(),
                usage: usage_report,
                content: ModelOutput::Text(TextContent {
                    content: String::new(),
                    signature: None,
                }),
                stop_reason,
                provider: ModelProviders::ANTHROPIC,
                error_detail: None,
                signature: None,
                metadata: None,
            });
        }

        messages
    }
}

// ============================================================================
// Helpers
// ============================================================================

pub fn build_anthropic_request(
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

pub fn parse_response(
    response: &MessagesResponse,
    model_id: &ModelId,
) -> GenerationResult<Vec<Messages>> {
    let stop_reason = map_stop_reason(&response.stop_reason);

    #[allow(clippy::cast_precision_loss)]
    let usage_total = (response.usage.input_tokens + response.usage.output_tokens) as f64;

    let mut messages = Vec::new();

    for block in &response.content {
        let msg = match block {
            AnthropicContentBlock::Text { text } => Messages::Assistant {
                model: model_id.clone(),
                timestamp: SystemTime::now(),
                usage: UsageReport {
                    input: response.usage.input_tokens as f64,
                    output: response.usage.output_tokens as f64,
                    cache_read: response.usage.cache_read_input_tokens as f64,
                    cache_write: response.usage.cache_creation_input_tokens as f64,
                    total_tokens: usage_total,
                    cost: UsageCosting {
                        currency: String::from("USD"),
                        input: 0.0,
                        output: 0.0,
                        cache_read: 0.0,
                        cache_write: 0.0,
                        total_tokens: 0.0,
                    },
                },
                content: ModelOutput::Text(TextContent {
                    content: text.clone(),
                    signature: None,
                }),
                stop_reason: stop_reason.clone(),
                provider: ModelProviders::ANTHROPIC,
                error_detail: None,
                signature: None,
                metadata: None,
            },
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

                Messages::Assistant {
                    model: model_id.clone(),
                    timestamp: SystemTime::now(),
                    usage: UsageReport {
                        input: response.usage.input_tokens as f64,
                        output: response.usage.output_tokens as f64,
                        cache_read: response.usage.cache_read_input_tokens as f64,
                        cache_write: response.usage.cache_creation_input_tokens as f64,
                        total_tokens: usage_total,
                        cost: UsageCosting {
                            currency: String::from("USD"),
                            input: 0.0,
                            output: 0.0,
                            cache_read: 0.0,
                            cache_write: 0.0,
                            total_tokens: 0.0,
                        },
                    },
                    content: ModelOutput::ToolCall {
                        id: id.clone(),
                        name: name.clone(),
                        arguments,
                        signature: None,
                    },
                    stop_reason: stop_reason.clone(),
                    provider: ModelProviders::ANTHROPIC,
                    error_detail: None,
                    signature: None,
                    metadata: None,
                }
            }
            AnthropicContentBlock::Thinking {
                thinking,
                signature,
            } => Messages::Assistant {
                model: model_id.clone(),
                timestamp: SystemTime::now(),
                usage: UsageReport {
                    input: response.usage.input_tokens as f64,
                    output: response.usage.output_tokens as f64,
                    cache_read: response.usage.cache_read_input_tokens as f64,
                    cache_write: response.usage.cache_creation_input_tokens as f64,
                    total_tokens: usage_total,
                    cost: UsageCosting {
                        currency: String::from("USD"),
                        input: 0.0,
                        output: 0.0,
                        cache_read: 0.0,
                        cache_write: 0.0,
                        total_tokens: 0.0,
                    },
                },
                content: ModelOutput::ThinkingContent {
                    thinking: thinking.clone(),
                    signature: Some(signature.clone()),
                },
                stop_reason: stop_reason.clone(),
                provider: ModelProviders::ANTHROPIC,
                error_detail: None,
                signature: None,
                metadata: None,
            },
            AnthropicContentBlock::RedactedThinking { .. } => Messages::Assistant {
                model: model_id.clone(),
                timestamp: SystemTime::now(),
                usage: UsageReport {
                    input: response.usage.input_tokens as f64,
                    output: response.usage.output_tokens as f64,
                    cache_read: response.usage.cache_read_input_tokens as f64,
                    cache_write: response.usage.cache_creation_input_tokens as f64,
                    total_tokens: usage_total,
                    cost: UsageCosting {
                        currency: String::from("USD"),
                        input: 0.0,
                        output: 0.0,
                        cache_read: 0.0,
                        cache_write: 0.0,
                        total_tokens: 0.0,
                    },
                },
                content: ModelOutput::ThinkingContent {
                    thinking: String::from("[redacted]"),
                    signature: None,
                },
                stop_reason: stop_reason.clone(),
                provider: ModelProviders::ANTHROPIC,
                error_detail: None,
                signature: None,
                metadata: None,
            },
            AnthropicContentBlock::Image { .. } | AnthropicContentBlock::ToolResult { .. } => {
                // Non-assistant blocks, skip
                continue;
            }
        };
        messages.push(msg);
    }

    // If response had no parseable content blocks, return empty text.
    if messages.is_empty() {
        messages.push(Messages::Assistant {
            model: model_id.clone(),
            timestamp: SystemTime::now(),
            usage: UsageReport {
                input: response.usage.input_tokens as f64,
                output: response.usage.output_tokens as f64,
                cache_read: response.usage.cache_read_input_tokens as f64,
                cache_write: response.usage.cache_creation_input_tokens as f64,
                total_tokens: usage_total,
                cost: UsageCosting {
                    currency: String::from("USD"),
                    input: 0.0,
                    output: 0.0,
                    cache_read: 0.0,
                    cache_write: 0.0,
                    total_tokens: 0.0,
                },
            },
            content: ModelOutput::Text(TextContent {
                content: String::new(),
                signature: None,
            }),
            stop_reason: stop_reason.clone(),
            provider: ModelProviders::ANTHROPIC,
            error_detail: None,
            signature: None,
            metadata: None,
        });
    }

    Ok(messages)
}

pub fn map_stop_reason(reason: &Option<String>) -> StopReason {
    match reason.as_deref() {
        Some("end_turn") => StopReason::Stop,
        Some("stop_sequence") => StopReason::Stop,
        Some("max_tokens") => StopReason::Length,
        Some("tool_use") => StopReason::ToolUse,
        Some(other) => StopReason::Message(other.to_string()),
        None => StopReason::Stop,
    }
}

pub fn empty_usage_report() -> UsageReport {
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

pub fn is_retryable_status(status: u16) -> bool {
    status == 429 || (500..=503).contains(&status)
}

pub fn exponential_backoff(attempt: u32) -> u64 {
    let base_secs: u64 = 1 << attempt.min(5);
    base_secs.min(30)
}


pub fn parse_anthropic_error(body: &str) -> Option<String> {
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

pub fn format_http_error(status_code: usize, detail: &str) -> String {
    match status_code {
        401 => format!("Authentication failed: {detail}"),
        403 => format!("Permission denied: {detail}"),
        404 => format!("Not found: {detail}"),
        429 => format!("Rate limit exceeded: {detail}"),
        500..=503 => format!("Server error (HTTP {status_code}): {detail}"),
        _ => format!("HTTP {status_code}: {detail}"),
    }
}
