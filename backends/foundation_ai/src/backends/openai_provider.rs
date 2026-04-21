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
use foundation_core::valtron::{execute, Stream, StreamIterator};
use foundation_core::wire::event_source::{Event, ReconnectingEventSourceTask};
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
    resolver: Option<R>,
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
            resolver: Some(SystemDnsResolver),
            models_cache: Arc::new(std::sync::Mutex::new(HashMap::new())),
        }
    }

    #[must_use]
    pub fn with_config(config: OpenAIConfig) -> Self {
        Self {
            config,
            api_key: None,
            http_client: None,
            resolver: Some(SystemDnsResolver),
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
            http_client: Some(SimpleHttpClient::with_resolver(resolver.clone())),
            resolver: Some(resolver),
            models_cache: Arc::new(std::sync::Mutex::new(HashMap::new())),
        }
    }

    /// Creates an `OpenAIProvider` with a custom DNS resolver and config.
    #[must_use]
    pub fn with_resolver_and_config(resolver: R, config: OpenAIConfig) -> Self {
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
            return Err(map_http_error(status_code, &body_text));
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
                resolver: self.resolver.clone(),
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
            resolver: self.resolver.clone(),
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
    resolver: Option<R>,
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
            return Err(map_http_error(status_code, &body_text));
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

        let resolver = self
            .resolver
            .as_ref()
            .ok_or_else(|| GenerationError::Generic("DNS resolver not initialized".into()))?
            .clone();

        let task = ReconnectingEventSourceTask::connect(resolver, &url)
            .map_err(|e| GenerationError::Backend(format!("Failed to create SSE task: {e}")))?
            .with_header(
                SimpleHeader::AUTHORIZATION,
                format!(
                    "Bearer {}",
                    self.api_key
                        .as_ref()
                        .map(ConfidentialText::get)
                        .unwrap_or_default()
                ),
            )
            .with_header(SimpleHeader::ACCEPT, String::from("text/event-stream"))
            .with_header(SimpleHeader::CONTENT_TYPE, String::from("application/json"))
            .with_body(SendSafeBody::Text(body));

        let driven = execute(task, None)
            .map_err(|e| GenerationError::Backend(format!("Executor error: {e}")))?;

        Ok(OpenAIStream {
            inner: driven,
            model_id: self.model_id.clone(),
            accumulated_text: String::new(),
            tool_calls: Vec::new(),
            finish_reason: None,
            usage: None,
            done: false,
        })
    }
}

// ============================================================================
// Streaming: SSE Parser
// ============================================================================

/// Streaming iterator that yields incremental `Messages` from an `OpenAI` SSE stream.
///
/// Wraps a `ReconnectingEventSourceTask` driven iterator. For each `Event::Message`
/// containing a `ChatCompletionChunk`, yields incremental text as `Stream::Next`.
/// On stream completion (`[DONE]`), yields the final accumulated message.
struct OpenAIStream<R: DnsResolver + 'static> {
    inner: foundation_core::valtron::DrivenStreamIterator<ReconnectingEventSourceTask<R>>,
    model_id: ModelId,
    accumulated_text: String,
    tool_calls: Vec<AccumulatedToolCall>,
    finish_reason: Option<String>,
    usage: Option<OpenAIUsage>,
    done: bool,
}

struct AccumulatedToolCall {
    id: String,
    name: String,
    arguments: String,
}

impl<R: DnsResolver + Send + 'static> Iterator for OpenAIStream<R> {
    type Item = Stream<Messages, ModelState>;

    fn next(&mut self) -> Option<Self::Item> {
        if self.done {
            return None;
        }

        loop {
            let item = self.inner.next()?;

            match item {
                Stream::Next(parse_result) => {
                    let Event::Message { data, .. } = &parse_result.event else {
                        continue;
                    };

                    if data.trim() == "[DONE]" {
                        self.done = true;
                        return Some(Stream::Next(self.build_final_message()));
                    }

                    let Ok(chunk) = serde_json::from_str::<ChatCompletionChunk>(data) else {
                        continue;
                    };

                    if let Some(u) = chunk.usage {
                        self.usage = Some(u);
                    }

                    let mut text_yielded = false;
                    for choice in &chunk.choices {
                        if let Some(ref delta) = choice.delta {
                            if let Some(ref content) = delta.content {
                                if !content.is_empty() {
                                    self.accumulated_text.push_str(content);
                                    text_yielded = true;
                                }
                            }
                            if let Some(ref tool_calls) = delta.tool_calls {
                                self.accumulate_tool_calls(tool_calls);
                            }
                        }
                        if let Some(ref reason) = choice.finish_reason {
                            if reason != "null" {
                                self.finish_reason = Some(reason.clone());
                            }
                        }
                    }

                    if text_yielded {
                        return Some(Stream::Next(Messages::Assistant {
                            model: self.model_id.clone(),
                            timestamp: SystemTime::now(),
                            usage: empty_usage_report(),
                            content: ModelOutput::Text(TextContent {
                                content: self.accumulated_text.clone(),
                                signature: None,
                            }),
                            stop_reason: StopReason::Stop,
                            provider: ModelProviders::OPENAI,
                            error_detail: None,
                            signature: None,
                        }));
                    }
                }
                Stream::Pending(_) | Stream::Delayed(_) | Stream::Init | Stream::Ignore => {}
            }
        }
    }
}

impl<R: DnsResolver + 'static> OpenAIStream<R> {
    fn accumulate_tool_calls(&mut self, deltas: &[OpenAIToolCallDelta]) {
        for delta in deltas {
            let idx = delta.index as usize;

            // Grow the vec if needed
            while self.tool_calls.len() <= idx {
                self.tool_calls.push(AccumulatedToolCall {
                    id: String::new(),
                    name: String::new(),
                    arguments: String::new(),
                });
            }

            let tc = &mut self.tool_calls[idx];
            if let Some(ref id) = delta.id {
                tc.id.clone_from(id);
            }
            if let Some(ref func) = delta.function {
                if let Some(ref name) = func.name {
                    tc.name.clone_from(name);
                }
                if let Some(ref args) = func.arguments {
                    tc.arguments.push_str(args);
                }
            }
        }
    }

    fn build_final_message(&self) -> Messages {
        let stop_reason = match self.finish_reason.as_deref() {
            Some("stop") | None => StopReason::Stop,
            Some("length") => StopReason::Length,
            Some("tool_calls") => StopReason::ToolUse,
            Some(_) => StopReason::Error,
        };

        #[allow(clippy::cast_precision_loss)]
        let usage_report = self
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

        let content = if self.tool_calls.is_empty() {
            ModelOutput::Text(TextContent {
                content: self.accumulated_text.clone(),
                signature: None,
            })
        } else {
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
        };

        Messages::Assistant {
            model: self.model_id.clone(),
            timestamp: SystemTime::now(),
            usage: usage_report,
            content,
            stop_reason,
            provider: ModelProviders::OPENAI,
            error_detail: None,
            signature: None,
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
    pub tool_calls: Option<Vec<OpenAIToolCallDelta>>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OpenAIToolCallDelta {
    pub index: u32,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub id: Option<String>,
    #[serde(rename = "type", skip_serializing_if = "Option::is_none")]
    pub tool_type: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub function: Option<OpenAIFunctionCallDelta>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OpenAIFunctionCallDelta {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub name: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub arguments: Option<String>,
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
struct OpenAIErrorResponse {
    error: OpenAIErrorDetail,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct OpenAIErrorDetail {
    message: String,
    #[serde(rename = "type")]
    error_type: Option<String>,
    code: Option<String>,
}

fn map_http_error(status_code: usize, body_text: &str) -> GenerationError {
    let detail = serde_json::from_str::<OpenAIErrorResponse>(body_text)
        .map_or_else(|_| body_text.to_string(), |e| e.error.message);

    match status_code {
        401 => GenerationError::Backend(format!("Authentication failed: {detail}")),
        403 => GenerationError::Backend(format!("Permission denied: {detail}")),
        404 => GenerationError::Backend(format!("Not found: {detail}")),
        429 => GenerationError::Backend(format!("Rate limit exceeded: {detail}")),
        500..=503 => {
            GenerationError::Backend(format!("Server error (HTTP {status_code}): {detail}"))
        }
        _ => GenerationError::Backend(format!("HTTP {status_code}: {detail}")),
    }
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

    let output = if let Some(tool_calls) = &message.tool_calls {
        if let Some(tc) = tool_calls.first() {
            let arguments: Option<HashMap<String, crate::types::ArgType>> =
                serde_json::from_str(&tc.function.arguments)
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
                name: tc.function.name.clone(),
                arguments,
                signature: None,
            }
        } else {
            ModelOutput::Text(TextContent {
                content,
                signature: None,
            })
        }
    } else {
        ModelOutput::Text(TextContent {
            content,
            signature: None,
        })
    };

    Ok(Messages::Assistant {
        model: model_id.clone(),
        timestamp: SystemTime::now(),
        usage: usage_report,
        content: output,
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
    fn test_parse_sse_chunks() {
        let chunks_raw = vec![
            r#"{"id":"chatcmpl-123","object":"chat.completion.chunk","created":1234567890,"model":"gpt-4","choices":[{"index":0,"delta":{"role":"assistant","content":"Hello"},"finish_reason":null}]}"#,
            r#"{"id":"chatcmpl-123","object":"chat.completion.chunk","created":1234567890,"model":"gpt-4","choices":[{"index":0,"delta":{"content":" world"},"finish_reason":null}]}"#,
            r#"{"id":"chatcmpl-123","object":"chat.completion.chunk","created":1234567890,"model":"gpt-4","choices":[{"index":0,"delta":{},"finish_reason":"stop"}]}"#,
        ];

        let mut accumulated_text = String::new();
        let mut finish_reason: Option<String> = None;

        for raw in &chunks_raw {
            let chunk: ChatCompletionChunk = serde_json::from_str(raw).unwrap();
            for choice in &chunk.choices {
                if let Some(ref delta) = choice.delta {
                    if let Some(ref content) = delta.content {
                        accumulated_text.push_str(content);
                    }
                }
                if choice.finish_reason.is_some() {
                    finish_reason = choice.finish_reason.clone();
                }
            }
        }

        assert_eq!(accumulated_text, "Hello world");
        assert_eq!(finish_reason.as_deref(), Some("stop"));

        let stop_reason = match finish_reason.as_deref() {
            Some("stop") | None => StopReason::Stop,
            Some("length") => StopReason::Length,
            Some("tool_calls") => StopReason::ToolUse,
            Some(_) => StopReason::Error,
        };
        assert_eq!(stop_reason, StopReason::Stop);
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

    #[test]
    fn test_tool_call_delta_deserialization() {
        let chunk_json = r#"{
            "id":"chatcmpl-123","object":"chat.completion.chunk","created":1234567890,"model":"gpt-4",
            "choices":[{
                "index":0,
                "delta":{"tool_calls":[{"index":0,"id":"call_abc","type":"function","function":{"name":"get_weather","arguments":""}}]},
                "finish_reason":null
            }]
        }"#;

        let chunk: ChatCompletionChunk = serde_json::from_str(chunk_json).unwrap();
        let delta = chunk.choices[0].delta.as_ref().unwrap();
        let tc = &delta.tool_calls.as_ref().unwrap()[0];
        assert_eq!(tc.index, 0);
        assert_eq!(tc.id.as_deref(), Some("call_abc"));
        assert_eq!(
            tc.function.as_ref().unwrap().name.as_deref(),
            Some("get_weather")
        );
    }

    #[test]
    fn test_tool_call_delta_continuation() {
        let continuation = r#"{
            "id":"chatcmpl-123","object":"chat.completion.chunk","created":1234567890,"model":"gpt-4",
            "choices":[{
                "index":0,
                "delta":{"tool_calls":[{"index":0,"function":{"arguments":"{\"loc"}}]},
                "finish_reason":null
            }]
        }"#;

        let chunk: ChatCompletionChunk = serde_json::from_str(continuation).unwrap();
        let delta = chunk.choices[0].delta.as_ref().unwrap();
        let tc = &delta.tool_calls.as_ref().unwrap()[0];
        assert_eq!(tc.index, 0);
        assert!(tc.id.is_none());
        assert_eq!(
            tc.function.as_ref().unwrap().arguments.as_deref(),
            Some("{\"loc")
        );
    }

    #[test]
    fn test_tool_call_accumulation() {
        let chunks = vec![
            r#"{"id":"c1","object":"chat.completion.chunk","created":0,"model":"gpt-4","choices":[{"index":0,"delta":{"tool_calls":[{"index":0,"id":"call_1","type":"function","function":{"name":"get_weather","arguments":""}}]},"finish_reason":null}]}"#,
            r#"{"id":"c1","object":"chat.completion.chunk","created":0,"model":"gpt-4","choices":[{"index":0,"delta":{"tool_calls":[{"index":0,"function":{"arguments":"{\"location\""}}]},"finish_reason":null}]}"#,
            r#"{"id":"c1","object":"chat.completion.chunk","created":0,"model":"gpt-4","choices":[{"index":0,"delta":{"tool_calls":[{"index":0,"function":{"arguments":": \"Paris\"}"}}]},"finish_reason":null}]}"#,
        ];

        let mut tool_calls: Vec<AccumulatedToolCall> = Vec::new();

        for raw in &chunks {
            let chunk: ChatCompletionChunk = serde_json::from_str(raw).unwrap();
            for choice in &chunk.choices {
                if let Some(ref delta) = choice.delta {
                    if let Some(ref tcs) = delta.tool_calls {
                        for tc_delta in tcs {
                            let idx = tc_delta.index as usize;
                            while tool_calls.len() <= idx {
                                tool_calls.push(AccumulatedToolCall {
                                    id: String::new(),
                                    name: String::new(),
                                    arguments: String::new(),
                                });
                            }
                            let tc = &mut tool_calls[idx];
                            if let Some(ref id) = tc_delta.id {
                                tc.id.clone_from(id);
                            }
                            if let Some(ref func) = tc_delta.function {
                                if let Some(ref name) = func.name {
                                    tc.name.clone_from(name);
                                }
                                if let Some(ref args) = func.arguments {
                                    tc.arguments.push_str(args);
                                }
                            }
                        }
                    }
                }
            }
        }

        assert_eq!(tool_calls.len(), 1);
        assert_eq!(tool_calls[0].id, "call_1");
        assert_eq!(tool_calls[0].name, "get_weather");
        assert_eq!(tool_calls[0].arguments, r#"{"location": "Paris"}"#);
    }

    #[test]
    fn test_json_value_to_arg_type() {
        let text = json_value_to_arg_type(&serde_json::json!("hello"));
        assert!(matches!(text, crate::types::ArgType::Text(s) if s == "hello"));

        let int = json_value_to_arg_type(&serde_json::json!(42));
        assert!(matches!(int, crate::types::ArgType::I64(42)));

        let float = json_value_to_arg_type(&serde_json::json!(3.14));
        assert!(
            matches!(float, crate::types::ArgType::Float64(f) if (f - 3.14).abs() < f64::EPSILON)
        );

        let obj = json_value_to_arg_type(&serde_json::json!({"nested": true}));
        assert!(matches!(obj, crate::types::ArgType::JSON(_)));
    }

    #[test]
    fn test_parse_chat_response_tool_calls() {
        let response = ChatCompletionResponse {
            id: "chatcmpl-123".into(),
            object: "chat.completion".into(),
            created: 0,
            model: "gpt-4".into(),
            choices: vec![OpenAIChoice {
                index: 0,
                message: Some(OpenAIMessage {
                    role: "assistant".into(),
                    content: None,
                    tool_calls: Some(vec![OpenAIToolCall {
                        id: "call_abc".into(),
                        tool_type: "function".into(),
                        function: OpenAIFunctionCall {
                            name: "get_weather".into(),
                            arguments: r#"{"location":"Paris"}"#.into(),
                        },
                    }]),
                    tool_call_id: None,
                }),
                finish_reason: Some("tool_calls".into()),
            }],
            usage: Some(OpenAIUsage {
                prompt_tokens: 10,
                completion_tokens: 5,
                total_tokens: 15,
            }),
        };

        let model_id = ModelId::Name("gpt-4".into(), None);
        let result = parse_chat_response(&response, &model_id).unwrap();

        if let Messages::Assistant {
            content,
            stop_reason,
            ..
        } = &result
        {
            assert_eq!(*stop_reason, StopReason::ToolUse);
            if let ModelOutput::ToolCall {
                id,
                name,
                arguments,
                ..
            } = content
            {
                assert_eq!(id, "call_abc");
                assert_eq!(name, "get_weather");
                let args = arguments.as_ref().expect("Should have arguments");
                assert!(args.contains_key("location"));
            } else {
                panic!("Expected ToolCall output");
            }
        } else {
            panic!("Expected Assistant message");
        }
    }

    #[test]
    fn test_parse_chat_response_text() {
        let response = ChatCompletionResponse {
            id: "chatcmpl-456".into(),
            object: "chat.completion".into(),
            created: 0,
            model: "gpt-4".into(),
            choices: vec![OpenAIChoice {
                index: 0,
                message: Some(OpenAIMessage {
                    role: "assistant".into(),
                    content: Some("Hello!".into()),
                    tool_calls: None,
                    tool_call_id: None,
                }),
                finish_reason: Some("stop".into()),
            }],
            usage: Some(OpenAIUsage {
                prompt_tokens: 5,
                completion_tokens: 2,
                total_tokens: 7,
            }),
        };

        let model_id = ModelId::Name("gpt-4".into(), None);
        let result = parse_chat_response(&response, &model_id).unwrap();

        if let Messages::Assistant {
            content,
            stop_reason,
            usage,
            ..
        } = &result
        {
            assert_eq!(*stop_reason, StopReason::Stop);
            if let ModelOutput::Text(tc) = content {
                assert_eq!(tc.content, "Hello!");
            } else {
                panic!("Expected Text output");
            }
            assert!((usage.total_tokens - 7.0).abs() < f64::EPSILON);
        } else {
            panic!("Expected Assistant message");
        }
    }

    #[test]
    fn test_map_http_error_parses_openai_format() {
        let body = r#"{"error":{"message":"Invalid API key","type":"invalid_request_error","code":"invalid_api_key"}}"#;
        let err = map_http_error(401, body);
        let msg = err.to_string();
        assert!(msg.contains("Authentication failed"), "got: {msg}");
        assert!(msg.contains("Invalid API key"), "got: {msg}");
    }

    #[test]
    fn test_map_http_error_rate_limit() {
        let body =
            r#"{"error":{"message":"Rate limit reached","type":"rate_limit_error","code":null}}"#;
        let err = map_http_error(429, body);
        let msg = err.to_string();
        assert!(msg.contains("Rate limit exceeded"), "got: {msg}");
    }

    #[test]
    fn test_map_http_error_plain_text_fallback() {
        let err = map_http_error(500, "Internal Server Error");
        let msg = err.to_string();
        assert!(msg.contains("Server error"), "got: {msg}");
        assert!(msg.contains("Internal Server Error"), "got: {msg}");
    }
}
