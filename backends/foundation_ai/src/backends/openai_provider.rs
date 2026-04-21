//! OpenAI-compatible HTTP provider for connecting to `OpenAI`, llama.cpp server,
//! vLLM, `Ollama`, `OpenRouter`, and any `OpenAI`-compatible endpoint.
//!
//! Uses `foundation_core::simple_http` for HTTP I/O with Valtron `TaskIterator`/`StreamIterator`
//! patterns — no tokio, no async-trait.

use std::collections::HashMap;
use std::sync::Arc;
use std::time::SystemTime;

use derive_more::From;
use foundation_auth::{AuthCredential, ConfidentialText};
use foundation_core::valtron::{Stream, StreamIterator};
use foundation_core::wire::simple_http::client::{
    DnsResolver, SimpleHttpClient, SystemDnsResolver,
};
use foundation_core::wire::simple_http::{SendSafeBody, SimpleHeader};
use serde::{Deserialize, Serialize};

use crate::errors::{GenerationError, GenerationResult, ModelProviderErrors, ModelProviderResult};
use crate::types::{
    Messages, Model, ModelId, ModelInteraction, ModelOutput, ModelParams, ModelProvider,
    ModelProviderDescriptor, ModelProviders, ModelSpec, ModelState, StopReason, TextContent,
    UsageCosting, UsageReport,
};

// ============================================================================
// OpenAI Configuration
// ============================================================================

/// Configuration for the `OpenAI` provider.
#[derive(Debug, Clone)]
pub struct OpenAIConfig {
    pub base_url: String,
    pub api_version: String,
    pub timeout_secs: u64,
    pub max_retries: u32,
    pub proxy_url: Option<String>,
    pub streaming: bool,
}

impl Default for OpenAIConfig {
    fn default() -> Self {
        Self {
            base_url: String::from("https://api.openai.com"),
            api_version: String::from("v1"),
            timeout_secs: 30,
            max_retries: 3,
            proxy_url: None,
            streaming: true,
        }
    }
}

impl OpenAIConfig {
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

    fn build_url(&self, endpoint: &str) -> String {
        format!(
            "{}/{}/{}",
            self.base_url.trim_end_matches('/'),
            self.api_version,
            endpoint.trim_start_matches('/')
        )
    }
}

// ============================================================================
// OpenAI Provider
// ============================================================================

/// OpenAI-compatible HTTP provider implementing [`ModelProvider`].
pub struct OpenAIProvider<R: DnsResolver = SystemDnsResolver> {
    config: OpenAIConfig,
    api_key: Option<ConfidentialText>,
    http_client: Option<SimpleHttpClient<R>>,
    models_cache: Arc<std::sync::Mutex<HashMap<String, OpenAIModelInfo>>>,
}

impl Default for OpenAIProvider<SystemDnsResolver> {
    fn default() -> Self {
        Self::new()
    }
}

impl OpenAIProvider<SystemDnsResolver> {
    #[must_use]
    pub fn new() -> Self {
        Self {
            config: OpenAIConfig::default(),
            api_key: None,
            http_client: None,
            models_cache: Arc::new(std::sync::Mutex::new(HashMap::new())),
        }
    }

    #[must_use]
    pub fn with_config(config: OpenAIConfig) -> Self {
        Self {
            config,
            api_key: None,
            http_client: None,
            models_cache: Arc::new(std::sync::Mutex::new(HashMap::new())),
        }
    }
}

impl<R: DnsResolver + 'static> OpenAIProvider<R> {
    /// Creates an `OpenAIProvider` with a custom DNS resolver.
    #[must_use]
    pub fn with_resolver(resolver: R) -> Self {
        Self {
            config: OpenAIConfig::default(),
            api_key: None,
            http_client: Some(SimpleHttpClient::with_resolver(resolver)),
            models_cache: Arc::new(std::sync::Mutex::new(HashMap::new())),
        }
    }

    /// Creates an `OpenAIProvider` with a custom DNS resolver and config.
    #[must_use]
    pub fn with_resolver_and_config(resolver: R, config: OpenAIConfig) -> Self {
        Self {
            config,
            api_key: None,
            http_client: Some(SimpleHttpClient::with_resolver(resolver)),
            models_cache: Arc::new(std::sync::Mutex::new(HashMap::new())),
        }
    }

    fn auth_headers(&self) -> Vec<(SimpleHeader, String)> {
        let mut headers = Vec::new();
        if let Some(key) = &self.api_key {
            headers.push((SimpleHeader::AUTHORIZATION, format!("Bearer {}", key.get())));
        }
        headers.push((SimpleHeader::CONTENT_TYPE, String::from("application/json")));
        headers
    }

    /// Execute a non-streaming HTTP request and parse the JSON response.
    fn execute_request<T: for<'de> Deserialize<'de> + Send>(
        &self,
        url: &str,
        body: &str,
    ) -> GenerationResult<T> {
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
        let body_text = match response.get_body_ref() {
            SendSafeBody::Text(t) => t.clone(),
            SendSafeBody::Bytes(b) => String::from_utf8_lossy(b).to_string(),
            SendSafeBody::None
            | SendSafeBody::Stream(_)
            | SendSafeBody::ChunkedStream(_)
            | SendSafeBody::LineFeedStream(_) => String::new(),
        };

        if !(200..=299).contains(&status_code) {
            return Err(GenerationError::Backend(format!(
                "HTTP {status_code}: {body_text}"
            )));
        }

        serde_json::from_str(&body_text)
            .map_err(|e| GenerationError::Generic(format!("Parse error: {e}")))
    }
}

impl<R: DnsResolver + Default + 'static> ModelProvider for OpenAIProvider<R> {
    type Config = OpenAIConfig;
    type Model = OpenAIModel<R>;

    fn create(
        mut self,
        config: Option<Self::Config>,
        credential: Option<AuthCredential>,
    ) -> ModelProviderResult<Self> {
        if let Some(cfg) = config {
            self.config = cfg;
        }

        match &credential {
            Some(AuthCredential::SecretOnly(key)) => {
                self.api_key = Some(key.clone());
            }
            Some(AuthCredential::ClientSecret {
                client_id: _,
                client_secret,
            }) => {
                self.api_key = Some(client_secret.clone());
            }
            Some(AuthCredential::OAuth(cred)) => {
                self.api_key = Some(cred.access_token.clone());
            }
            Some(AuthCredential::EmailAuth { .. } | AuthCredential::UsernameAndPassword { .. }) => {
                return Err(ModelProviderErrors::NotFound(
                    "OpenAI provider requires SecretOnly, ClientSecret, or OAuth credentials"
                        .into(),
                ));
            }
            None => {}
        }

        // Apply proxy and timeout configuration to the HTTP client.
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
            id: String::from("openai"),
            name: String::from("OpenAI"),
            reasoning: false,
            api: crate::types::ModelAPI::OpenAICompletions,
            provider: ModelProviders::OPENAI,
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
            return Ok(OpenAIModel {
                config: self.config.clone(),
                model_id: model_id.clone(),
                model_name: model_name.clone(),
                api_key: self.api_key.clone(),
                http_client: self.http_client.clone(),
                info: info.clone(),
            });
        }
        drop(cache);

        let url = self.config.build_url(&format!("models/{model_name}"));
        let result: Result<OpenAIModelResponse, _> = self.execute_request(&url, "");

        let info = match result {
            Ok(resp) => OpenAIModelInfo {
                id: resp.id,
                object: resp.object,
                owned_by: resp.owned_by.unwrap_or_default(),
                created: resp.created.unwrap_or(0),
            },
            Err(_) => OpenAIModelInfo {
                id: model_name.clone(),
                object: String::from("model"),
                owned_by: String::new(),
                created: 0,
            },
        };

        let mut cache = self.models_cache.lock().expect("model cache poisoned");
        cache.insert(model_name.clone(), info.clone());

        Ok(OpenAIModel {
            config: self.config.clone(),
            model_id,
            model_name,
            api_key: self.api_key.clone(),
            http_client: self.http_client.clone(),
            info,
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
        let url = self.config.build_url("models");
        let response: OpenAIListResponse = self
            .execute_request(&url, "")
            .map_err(|e| ModelProviderErrors::NotFound(e.to_string()))?;

        let filter_pattern = match &model_id {
            ModelId::Name(name, _) => name.to_lowercase(),
            ModelId::Alias(alias, _) => alias.to_lowercase(),
            ModelId::Group(group, _) => group.to_lowercase(),
            ModelId::Architecture(arch, _) => arch.to_lowercase(),
        };

        let specs: Vec<ModelSpec> = response
            .data
            .into_iter()
            .filter(|m| m.id.to_lowercase().contains(&filter_pattern))
            .map(|m| ModelSpec {
                name: m.id.clone(),
                id: ModelId::Name(m.id.clone(), None),
                devices: None,
                model_location: None,
                lora_location: None,
            })
            .collect();

        Ok(specs)
    }
}

// ============================================================================
// OpenAI Model
// ============================================================================

/// A model handle for the `OpenAI` provider implementing [`Model`].
pub struct OpenAIModel<R: DnsResolver = SystemDnsResolver> {
    config: OpenAIConfig,
    model_id: ModelId,
    model_name: String,
    api_key: Option<ConfidentialText>,
    http_client: Option<SimpleHttpClient<R>>,
    /// Cached model metadata from the provider.
    #[allow(dead_code)]
    info: OpenAIModelInfo,
}

impl<R: DnsResolver + 'static> OpenAIModel<R> {
    fn build_auth_headers(&self) -> Vec<(SimpleHeader, String)> {
        let mut headers = Vec::new();
        if let Some(key) = &self.api_key {
            headers.push((SimpleHeader::AUTHORIZATION, format!("Bearer {}", key.get())));
        }
        headers.push((SimpleHeader::CONTENT_TYPE, String::from("application/json")));
        headers
    }

    /// Execute a non-streaming HTTP request and parse the JSON response.
    fn execute_request<T: for<'de> Deserialize<'de> + Send>(
        &self,
        url: &str,
        body: &str,
    ) -> GenerationResult<T> {
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
            return Err(GenerationError::Backend(format!(
                "HTTP {status_code}: {body_text}"
            )));
        }

        serde_json::from_str(&body_text)
            .map_err(|e| GenerationError::Generic(format!("Parse error: {e}")))
    }
}

impl<R: DnsResolver + 'static> Model for OpenAIModel<R> {
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
        let request = build_chat_request(&self.model_name, &interaction, &params, false);

        let body = serde_json::to_string(&request)
            .map_err(|e| GenerationError::Generic(format!("Failed to serialize request: {e}")))?;

        let url = self.config.build_url("chat/completions");
        let response: ChatCompletionResponse = self.execute_request(&url, &body)?;

        let message = parse_chat_response(&response, &self.model_id)?;
        Ok(vec![message])
    }

    fn stream(
        &self,
        interaction: ModelInteraction,
        specs: Option<ModelParams>,
    ) -> GenerationResult<impl StreamIterator<D = Messages, P = ModelState>> {
        let params = specs.unwrap_or_default();
        let request = build_chat_request(&self.model_name, &interaction, &params, true);

        let body = serde_json::to_string(&request)
            .map_err(|e| GenerationError::Generic(format!("Failed to serialize request: {e}")))?;

        let url = self.config.build_url("chat/completions");
        let Some(client) = &self.http_client else {
            return Err(GenerationError::Generic(
                "HTTP client not initialized".into(),
            ));
        };

        let mut builder = client
            .post(&url)
            .map_err(|e| GenerationError::Backend(format!("Failed to create request: {e}")))?;
        for (k, v) in &self.build_auth_headers() {
            builder = builder.header(k.clone(), v.clone());
        }
        builder = builder.header(SimpleHeader::ACCEPT, String::from("text/event-stream"));
        builder = builder.body_text(body);

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
            return Err(GenerationError::Backend(format!(
                "HTTP {status_code}: {body_text}"
            )));
        }

        let model_id = self.model_id.clone();
        let messages = parse_sse_stream(&body_text, &model_id);

        Ok(SingleMessageStream {
            inner: messages
                .into_iter()
                .map(Ok)
                .collect::<Vec<Result<Messages, OpenAIError>>>()
                .into_iter(),
            done: false,
        })
    }
}

// ============================================================================
// Streaming: SSE Parser
// ============================================================================

/// Parse SSE events from a raw response body string.
fn parse_sse_stream(body: &str, model_id: &ModelId) -> Vec<Messages> {
    let mut accumulated_text = String::new();
    let mut usage: Option<OpenAIUsage> = None;
    let mut finish_reason: Option<String> = None;

    for line in body.lines() {
        let trimmed = line.trim();
        if trimmed.is_empty() || trimmed.starts_with(':') {
            continue;
        }

        if let Some(data) = trimmed.strip_prefix("data: ") {
            if data == "[DONE]" {
                break;
            }

            if let Ok(chunk) = serde_json::from_str::<ChatCompletionChunk>(data) {
                if let Some(u) = chunk.usage {
                    usage = Some(u);
                }

                for choice in &chunk.choices {
                    if let Some(ref delta) = choice.delta {
                        if let Some(ref content) = delta.content {
                            accumulated_text.push_str(content);
                        }
                    }
                    if let Some(ref reason) = choice.finish_reason {
                        if reason != "null" {
                            finish_reason = Some(reason.clone());
                        }
                    }
                }
            }
        }
    }

    let stop_reason = if accumulated_text.is_empty() {
        StopReason::Error
    } else {
        match finish_reason.as_deref() {
            Some("stop") | None => StopReason::Stop,
            Some("length") => StopReason::Length,
            Some("tool_calls") => StopReason::ToolUse,
            Some(_) => StopReason::Error,
        }
    };

    #[allow(clippy::cast_precision_loss)]
    let usage_report = usage.map_or_else(empty_usage_report, |u| UsageReport {
        input: u.prompt_tokens as f64,
        output: u.completion_tokens as f64,
        cache_read: 0.0,
        cache_write: 0.0,
        total_tokens: u.total_tokens as f64,
        cost: UsageCosting {
            currency: String::from("USD"),
            input: 0.0,
            output: 0.0,
            cache_read: 0.0,
            cache_write: 0.0,
            total_tokens: u.total_tokens as f64,
        },
    });

    vec![Messages::Assistant {
        model: model_id.clone(),
        timestamp: SystemTime::now(),
        usage: usage_report,
        content: ModelOutput::Text(TextContent {
            content: accumulated_text,
            signature: None,
        }),
        stop_reason,
        provider: ModelProviders::OPENAI,
        error_detail: None,
        signature: None,
    }]
}

/// Iterator that yields a single message from the accumulated SSE response.
struct SingleMessageStream {
    inner: std::vec::IntoIter<Result<Messages, OpenAIError>>,
    done: bool,
}

impl Iterator for SingleMessageStream {
    type Item = Stream<Messages, ModelState>;

    fn next(&mut self) -> Option<Self::Item> {
        if self.done {
            return None;
        }

        let result = self.inner.next();
        match result {
            Some(Ok(msg)) => {
                self.done = true;
                Some(Stream::Next(msg))
            }
            Some(Err(e)) => {
                self.done = true;
                Some(Stream::Next(Messages::Assistant {
                    model: ModelId::Name(String::from("openai"), None),
                    timestamp: SystemTime::now(),
                    usage: empty_usage_report(),
                    content: ModelOutput::Text(TextContent {
                        content: format!("OpenAI streaming error: {e}"),
                        signature: None,
                    }),
                    stop_reason: StopReason::Error,
                    provider: ModelProviders::OPENAI,
                    error_detail: Some(e.to_string()),
                    signature: None,
                }))
            }
            None => {
                self.done = true;
                Some(Stream::Init)
            }
        }
    }
}

// ============================================================================
// OpenAI API Request/Response Types
// ============================================================================

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ChatCompletionRequest {
    pub model: String,
    pub messages: Vec<OpenAIMessage>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub temperature: Option<f32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub top_p: Option<f32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub max_tokens: Option<usize>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub stop: Option<Vec<String>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub stream: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub tools: Option<Vec<OpenAITool>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub n: Option<u32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub seed: Option<u32>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OpenAIMessage {
    pub role: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub content: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub tool_calls: Option<Vec<OpenAIToolCall>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub tool_call_id: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OpenAITool {
    #[serde(rename = "type")]
    pub tool_type: String,
    pub function: OpenAIFunction,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OpenAIFunction {
    pub name: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub parameters: Option<serde_json::Value>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OpenAIToolCall {
    pub id: String,
    #[serde(rename = "type")]
    pub tool_type: String,
    pub function: OpenAIFunctionCall,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OpenAIFunctionCall {
    pub name: String,
    pub arguments: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ChatCompletionResponse {
    pub id: String,
    pub object: String,
    pub created: u64,
    pub model: String,
    pub choices: Vec<OpenAIChoice>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub usage: Option<OpenAIUsage>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ChatCompletionChunk {
    pub id: String,
    pub object: String,
    pub created: u64,
    pub model: String,
    pub choices: Vec<OpenAIChunkChoice>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub usage: Option<OpenAIUsage>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OpenAIChoice {
    pub index: u32,
    pub message: Option<OpenAIMessage>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub finish_reason: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OpenAIChunkChoice {
    pub index: u32,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub delta: Option<OpenAIDelta>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub finish_reason: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OpenAIDelta {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub role: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub content: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub tool_calls: Option<Vec<OpenAIToolCall>>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OpenAIUsage {
    #[serde(rename = "prompt_tokens")]
    pub prompt_tokens: u64,
    #[serde(rename = "completion_tokens")]
    pub completion_tokens: u64,
    #[serde(rename = "total_tokens")]
    pub total_tokens: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OpenAIModelResponse {
    pub id: String,
    pub object: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub created: Option<u64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub owned_by: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OpenAIListResponse {
    pub object: String,
    pub data: Vec<OpenAIModelResponse>,
}

#[derive(Debug, Clone)]
pub struct OpenAIModelInfo {
    pub id: String,
    pub object: String,
    pub owned_by: String,
    pub created: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EmbeddingRequest {
    pub model: String,
    pub input: EmbeddingInput,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub encoding_format: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(untagged)]
pub enum EmbeddingInput {
    String(String),
    Strings(Vec<String>),
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EmbeddingResponse {
    pub object: String,
    pub data: Vec<EmbeddingData>,
    pub model: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub usage: Option<OpenAIUsage>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EmbeddingData {
    pub index: u32,
    pub object: String,
    pub embedding: Vec<f32>,
}

// ============================================================================
// Error Types
// ============================================================================

/// OpenAI-specific errors.
#[derive(From, Debug)]
pub enum OpenAIError {
    #[from(ignore)]
    Http(String),
    HttpStatus {
        code: u16,
        body: String,
    },
    #[from(ignore)]
    Parse(String),
    #[from(ignore)]
    Valtron(String),
    NoResult,
    RateLimit {
        retry_after: Option<u64>,
    },
}

impl core::fmt::Display for OpenAIError {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        match self {
            OpenAIError::Http(e) => write!(f, "OpenAI HTTP error: {e}"),
            OpenAIError::HttpStatus { code, body } => write!(f, "OpenAI HTTP {code}: {body}"),
            OpenAIError::Parse(e) => write!(f, "OpenAI parse error: {e}"),
            OpenAIError::Valtron(e) => write!(f, "OpenAI Valtron error: {e}"),
            OpenAIError::NoResult => write!(f, "No result from OpenAI API"),
            OpenAIError::RateLimit { retry_after } => {
                if let Some(secs) = retry_after {
                    write!(f, "OpenAI rate limited, retry after {secs}s")
                } else {
                    write!(f, "OpenAI rate limited")
                }
            }
        }
    }
}

impl std::error::Error for OpenAIError {}

// ============================================================================
// Helper Functions
// ============================================================================

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

fn model_id_to_string(id: &ModelId) -> String {
    match id {
        ModelId::Name(name, _) => name.clone(),
        ModelId::Alias(alias, _) => alias.clone(),
        ModelId::Group(group, _) => group.clone(),
        ModelId::Architecture(arch, _) => arch.clone(),
    }
}

#[allow(clippy::too_many_lines)]
fn build_chat_request(
    model_name: &str,
    interaction: &ModelInteraction,
    params: &ModelParams,
    streaming: bool,
) -> ChatCompletionRequest {
    let mut messages = Vec::new();

    if let Some(ref system) = interaction.system_prompt {
        messages.push(OpenAIMessage {
            role: String::from("system"),
            content: Some(system.clone()),
            tool_calls: None,
            tool_call_id: None,
        });
    }

    for msg in &interaction.messages {
        match msg {
            Messages::User { content, .. } => {
                let text = match content {
                    crate::types::UserModelContent::Text(tc) => tc.content.clone(),
                    crate::types::UserModelContent::Image(_) => String::from("[Image]"),
                };
                messages.push(OpenAIMessage {
                    role: String::from("user"),
                    content: Some(text),
                    tool_calls: None,
                    tool_call_id: None,
                });
            }
            Messages::Assistant { content, .. } => match content {
                ModelOutput::Text(tc) => {
                    messages.push(OpenAIMessage {
                        role: String::from("assistant"),
                        content: Some(tc.content.clone()),
                        tool_calls: None,
                        tool_call_id: None,
                    });
                }
                ModelOutput::ToolCall {
                    id,
                    name,
                    arguments,
                    ..
                } => {
                    let tool_calls = vec![OpenAIToolCall {
                        id: id.clone(),
                        tool_type: String::from("function"),
                        function: OpenAIFunctionCall {
                            name: name.clone(),
                            arguments: arguments
                                .as_ref()
                                .map(|args| serde_json::to_string(args).unwrap_or_default())
                                .unwrap_or_default(),
                        },
                    }];
                    messages.push(OpenAIMessage {
                        role: String::from("assistant"),
                        content: None,
                        tool_calls: Some(tool_calls),
                        tool_call_id: None,
                    });
                }
                ModelOutput::ThinkingContent { thinking, .. } => {
                    messages.push(OpenAIMessage {
                        role: String::from("assistant"),
                        content: Some(thinking.clone()),
                        tool_calls: None,
                        tool_call_id: None,
                    });
                }
                ModelOutput::Image(_) | ModelOutput::Embedding { .. } => {}
            },
            Messages::ToolResult {
                id, name, content, ..
            } => {
                let text = match content {
                    crate::types::UserModelContent::Text(tc) => tc.content.clone(),
                    crate::types::UserModelContent::Image(_) => String::from("[Image]"),
                };
                messages.push(OpenAIMessage {
                    role: String::from("tool"),
                    content: Some(format!("[{name}] {text}")),
                    tool_calls: None,
                    tool_call_id: Some(id.clone()),
                });
            }
        }
    }

    let tools = if interaction.tools.is_empty() {
        None
    } else {
        Some(
            interaction
                .tools
                .iter()
                .map(|tool| OpenAITool {
                    tool_type: String::from("function"),
                    function: OpenAIFunction {
                        name: tool.name.clone(),
                        description: Some(tool.description.clone()),
                        parameters: tool.arguments.as_ref().map(|args| {
                            let mut properties = serde_json::Map::new();
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
                            serde_json::json!({
                                "type": "object",
                                "properties": properties,
                            })
                        }),
                    },
                })
                .collect(),
        )
    };

    #[allow(clippy::cast_possible_truncation)]
    ChatCompletionRequest {
        model: model_name.to_string(),
        messages,
        temperature: if params.temperature > 0.0 {
            Some(params.temperature)
        } else {
            None
        },
        top_p: if params.top_p > 0.0 && params.top_p < 1.0 {
            Some(params.top_p)
        } else {
            None
        },
        max_tokens: if params.max_tokens > 0 {
            Some(params.max_tokens)
        } else {
            None
        },
        stop: if params.stop_tokens.is_empty() {
            None
        } else {
            Some(params.stop_tokens.clone())
        },
        stream: Some(streaming),
        tools,
        n: Some(1),
        seed: params.seed,
    }
}

fn parse_chat_response(
    response: &ChatCompletionResponse,
    model_id: &ModelId,
) -> GenerationResult<Messages> {
    let choice = response
        .choices
        .first()
        .ok_or_else(|| GenerationError::Generic("No choices in response".into()))?;

    let message = choice
        .message
        .as_ref()
        .ok_or_else(|| GenerationError::Generic("No message in response choice".into()))?;

    let content = message.content.clone().unwrap_or_default();

    let stop_reason = match choice.finish_reason.as_deref() {
        Some("stop") | None => StopReason::Stop,
        Some("length") => StopReason::Length,
        Some("tool_calls") => StopReason::ToolUse,
        Some("content_filter") => StopReason::Error,
        Some(other) => {
            tracing::warn!("Unknown finish reason: {other}");
            StopReason::Stop
        }
    };

    #[allow(clippy::cast_precision_loss)]
    let usage_report = response
        .usage
        .as_ref()
        .map_or_else(empty_usage_report, |u| UsageReport {
            input: u.prompt_tokens as f64,
            output: u.completion_tokens as f64,
            cache_read: 0.0,
            cache_write: 0.0,
            total_tokens: u.total_tokens as f64,
            cost: UsageCosting {
                currency: String::from("USD"),
                input: 0.0,
                output: 0.0,
                cache_read: 0.0,
                cache_write: 0.0,
                total_tokens: u.total_tokens as f64,
            },
        });

    Ok(Messages::Assistant {
        model: model_id.clone(),
        timestamp: SystemTime::now(),
        usage: usage_report,
        content: ModelOutput::Text(TextContent {
            content,
            signature: None,
        }),
        stop_reason,
        provider: ModelProviders::OPENAI,
        error_detail: None,
        signature: None,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_openai_config_defaults() {
        let config = OpenAIConfig::default();
        assert_eq!(config.base_url, "https://api.openai.com");
        assert_eq!(config.api_version, "v1");
        assert_eq!(config.timeout_secs, 30);
        assert_eq!(config.max_retries, 3);
        assert!(config.proxy_url.is_none());
        assert!(config.streaming);
    }

    #[test]
    fn test_openai_config_builder() {
        let config = OpenAIConfig::new()
            .with_base_url("http://localhost:8080")
            .with_timeout_secs(60)
            .with_streaming(false);
        assert_eq!(config.base_url, "http://localhost:8080");
        assert_eq!(config.timeout_secs, 60);
        assert!(!config.streaming);
    }

    #[test]
    fn test_build_url() {
        let config = OpenAIConfig::new()
            .with_base_url("http://localhost:8080")
            .with_api_version("v1");
        assert_eq!(
            config.build_url("chat/completions"),
            "http://localhost:8080/v1/chat/completions"
        );
    }

    #[test]
    fn test_model_id_to_string() {
        assert_eq!(
            model_id_to_string(&ModelId::Name("gpt-4".into(), None)),
            "gpt-4"
        );
        assert_eq!(model_id_to_string(&ModelId::Alias("4".into(), None)), "4");
    }

    #[test]
    fn test_parse_sse_stream() {
        let body = r#"data: {"id":"chatcmpl-123","object":"chat.completion.chunk","created":1234567890,"model":"gpt-4","choices":[{"index":0,"delta":{"role":"assistant","content":"Hello"},"finish_reason":null}]}

data: {"id":"chatcmpl-123","object":"chat.completion.chunk","created":1234567890,"model":"gpt-4","choices":[{"index":0,"delta":{"content":" world"},"finish_reason":null}]}

data: {"id":"chatcmpl-123","object":"chat.completion.chunk","created":1234567890,"model":"gpt-4","choices":[{"index":0,"delta":{},"finish_reason":"stop"}]}

data: [DONE]
"#;
        let model_id = ModelId::Name("gpt-4".into(), None);
        let messages = parse_sse_stream(body, &model_id);
        assert_eq!(messages.len(), 1);

        if let Messages::Assistant {
            content,
            stop_reason,
            ..
        } = &messages[0]
        {
            if let ModelOutput::Text(tc) = content {
                assert_eq!(tc.content, "Hello world");
            } else {
                panic!("Expected Text output");
            }
            assert_eq!(*stop_reason, StopReason::Stop);
        } else {
            panic!("Expected Assistant message");
        }
    }

    #[test]
    fn test_chat_completion_request_serialization() {
        let request = ChatCompletionRequest {
            model: "gpt-4".into(),
            messages: vec![
                OpenAIMessage {
                    role: "system".into(),
                    content: Some("You are helpful".into()),
                    tool_calls: None,
                    tool_call_id: None,
                },
                OpenAIMessage {
                    role: "user".into(),
                    content: Some("Hello".into()),
                    tool_calls: None,
                    tool_call_id: None,
                },
            ],
            temperature: Some(0.7),
            top_p: Some(0.9),
            max_tokens: Some(100),
            stop: None,
            stream: Some(false),
            tools: None,
            n: Some(1),
            seed: None,
        };

        let json = serde_json::to_string(&request).unwrap();
        assert!(json.contains("\"model\":\"gpt-4\""));
        assert!(json.contains("\"temperature\":0.7"));
        assert!(json.contains("\"stream\":false"));
    }

    #[test]
    fn test_chat_response_deserialization() {
        let json = r#"{
            "id": "chatcmpl-123",
            "object": "chat.completion",
            "created": 1234567890,
            "model": "gpt-4",
            "choices": [{
                "index": 0,
                "message": { "role": "assistant", "content": "Hello!" },
                "finish_reason": "stop"
            }],
            "usage": { "prompt_tokens": 10, "completion_tokens": 5, "total_tokens": 15 }
        }"#;

        let response: ChatCompletionResponse = serde_json::from_str(json).unwrap();
        assert_eq!(response.id, "chatcmpl-123");
        assert_eq!(
            response.choices[0].message.as_ref().unwrap().content,
            Some("Hello!".into())
        );
        assert!(response.usage.is_some());
        assert_eq!(response.usage.as_ref().unwrap().total_tokens, 15);
    }
}
