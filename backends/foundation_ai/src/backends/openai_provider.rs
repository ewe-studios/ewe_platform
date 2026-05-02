//! OpenAI-compatible HTTP provider for connecting to `OpenAI`, llama.cpp server,
//! vLLM, `Ollama`, `OpenRouter`, and any `OpenAI`-compatible endpoint.
//!
//! Uses `foundation_core::simple_http` for HTTP I/O with Valtron `TaskIterator`/`StreamIterator`
//! patterns — no tokio, no async-trait.

use std::cell::RefCell;
use std::collections::HashMap;
use std::rc::Rc;
use std::sync::Arc;
use std::thread;
use std::time::{Duration, SystemTime};

use derive_more::From;
use foundation_auth::{AuthCredential, ConfidentialText};
use foundation_core::valtron::{execute, Stream, StreamIterator};
use foundation_core::wire::event_source::{Event, ReconnectingEventSourceTask, ReconnectingProgress};
use foundation_core::wire::simple_http::client::{
    DnsResolver, SimpleHttpClient, SystemDnsResolver,
};
use foundation_core::wire::simple_http::{SendSafeBody, SimpleHeader, SimpleHeaders};
use foundation_errstacks::ErrorTrace;
use serde::{Deserialize, Serialize};

use crate::costing::{calculate_cost, CostAccumulator};
use crate::errors::{GenerationError, GenerationResult, ModelProviderErrors, ModelProviderResult};
use crate::types::{
    AuthProvider, ExtractResult, Messages, Model, ModelId, ModelInteraction, ModelOutput,
    ModelParams, ModelProvider, ModelProviderDescriptor, ModelProviders, ModelSpec, ModelState,
    ModelUsageCosting, StopReason, TextContent, Tool, ToolCallingError, ToolFormatter, ToolShed,
    CostStatus, UsageCosting, UsageReport,
};

// ============================================================================
// OpenAI Configuration
// ============================================================================

/// Configuration for the `OpenAI` provider.
#[derive(Debug)]
pub struct OpenAIConfig {
    pub base_url: String,
    pub api_version: String,
    pub timeout_secs: u64,
    pub max_retries: u32,
    pub proxy_url: Option<String>,
    pub streaming: bool,
    pub auth: Option<AuthCredential>,
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
            auth: None,
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

    #[must_use]
    pub fn with_auth(mut self, auth: AuthCredential) -> Self {
        self.auth = Some(auth);
        self
    }

    /// Build a full URL for an API endpoint.
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

impl Clone for OpenAIConfig {
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

impl crate::types::AuthProvider for OpenAIConfig {
    fn auth(&self) -> Option<&AuthCredential> {
        self.auth.as_ref()
    }
}

impl<R: DnsResolver> OpenAIProvider<R> {
    fn build_url(&self, endpoint: &str) -> String {
        self.config.build_url(endpoint)
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

    /// Execute a non-streaming HTTP request with retry on 429/5xx errors.
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

    /// Perform a single HTTP request attempt and parse the JSON response.
    /// Returns `Err((status, retry_after, message))` on HTTP errors.
    #[allow(clippy::type_complexity, clippy::cast_possible_truncation)]
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
            let detail = parse_openai_error(&body_text).unwrap_or_else(|| body_text.clone());
            let msg = format_http_error(status_code, &detail);
            return Ok(Err((status_code as u16, retry_after, msg)));
        }

        serde_json::from_str(&body_text)
            .map(|v| Ok(v))
            .map_err(|e| GenerationError::Generic(format!("Parse error: {e}")))
    }
}

impl<R: DnsResolver + Default + 'static> ModelProvider for OpenAIProvider<R> {
    type Config = OpenAIConfig;
    type Model = OpenAIModel<OpenAIFormatter, R>;

    fn create(mut self, config: Option<Self::Config>) -> ModelProviderResult<Self> {
        if let Some(cfg) = config {
            if let Some(cred) = cfg.auth() {
                match &cred {
                    AuthCredential::SecretOnly(key) => {
                        self.api_key = Some(key.clone());
                    }
                    AuthCredential::ClientSecret {
                        client_id: _,
                        client_secret,
                    } => {
                        self.api_key = Some(client_secret.clone());
                    }
                    AuthCredential::OAuth(cred) => {
                        self.api_key = Some(cred.access_token.clone());
                    }
                    AuthCredential::EmailAuth { .. }
                    | AuthCredential::UsernameAndPassword { .. } => {
                        return Err(ModelProviderErrors::NotFound(
                            "OpenAI provider requires SecretOnly, ClientSecret, or OAuth credentials"
                                .into(),
                        ));
                    }
                }
            }
            self.config = cfg;
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
            id: "openai",
            name: "OpenAI",
            reasoning: false,
            api: crate::types::ModelAPI::OpenAICompletions,
            provider: ModelProviders::OPENAI,
            base_url: None,
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
                _formatter: std::marker::PhantomData,
                pricing: self.describe().ok().map(|d| d.cost).unwrap_or_default(),
                cumulative_cost: Rc::new(RefCell::new(CostAccumulator::new())),
            });
        }
        drop(cache);

        let url = self.build_url(&format!("models/{model_name}"));
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
            _formatter: std::marker::PhantomData,
            pricing: self.describe().ok().map(|d| d.cost).unwrap_or_default(),
            cumulative_cost: Rc::new(RefCell::new(CostAccumulator::new())),
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
        let url = self.build_url("models");
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
///
/// The `F` type parameter allows customizing the tool formatter. When used
/// natively with `OpenAI` it defaults to `OpenAIFormatter`; when used as a
/// proxy to other endpoints the caller can supply a different formatter.
pub struct OpenAIModel<F: ToolFormatter = OpenAIFormatter, R: DnsResolver = SystemDnsResolver> {
    config: OpenAIConfig,
    model_id: ModelId,
    model_name: String,
    api_key: Option<ConfidentialText>,
    http_client: Option<SimpleHttpClient<R>>,
    resolver: Option<R>,
    /// Cached model metadata from the provider (used in model identity).
    #[allow(dead_code)]
    info: OpenAIModelInfo,
    _formatter: std::marker::PhantomData<F>,
    pricing: ModelUsageCosting,
    cumulative_cost: Rc<RefCell<CostAccumulator>>,
}

impl<F: ToolFormatter, R: DnsResolver + 'static> OpenAIModel<F, R> {
    fn build_url(&self, endpoint: &str) -> String {
        self.config.build_url(endpoint)
    }

    fn build_auth_headers(&self) -> Vec<(SimpleHeader, String)> {
        let mut headers = Vec::new();
        if let Some(key) = &self.api_key {
            headers.push((SimpleHeader::AUTHORIZATION, format!("Bearer {}", key.get())));
        }
        headers.push((SimpleHeader::CONTENT_TYPE, String::from("application/json")));
        headers
    }

    /// Execute a non-streaming HTTP request with retry on 429/5xx errors.
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

    /// Perform a single HTTP request attempt and parse the JSON response.
    #[allow(clippy::type_complexity, clippy::cast_possible_truncation)]
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
            let detail = parse_openai_error(&body_text).unwrap_or_else(|| body_text.clone());
            let msg = format_http_error(status_code, &detail);
            return Ok(Err((status_code as u16, retry_after, msg)));
        }

        serde_json::from_str(&body_text)
            .map(|v| Ok(v))
            .map_err(|e| GenerationError::Generic(format!("Parse error: {e}")))
    }

    /// Generate embeddings via `/v1/embeddings` endpoint.
    fn generate_embeddings(&self, interaction: &ModelInteraction) -> GenerationResult<Vec<Messages>> {
        let text = interaction
            .messages
            .iter()
            .filter_map(|msg| {
                if let Messages::User {
                    content: crate::types::UserModelContent::Text(tc),
                    ..
                } = msg
                {
                    Some(tc.content.clone())
                } else {
                    None
                }
            })
            .collect::<Vec<_>>();

        let input = if text.len() == 1 {
            EmbeddingInput::String(text.into_iter().next().unwrap())
        } else {
            EmbeddingInput::Strings(text)
        };

        let request = EmbeddingRequest {
            model: self.model_name.clone(),
            input,
            encoding_format: Some("float".to_string()),
        };

        let body = serde_json::to_string(&request)
            .map_err(|e| GenerationError::Generic(format!("Failed to serialize request: {e}")))?;

        let url = self.build_url("embeddings");
        let response: EmbeddingResponse = self.execute_request(&url, &body)?;

        let data = response
            .data
            .first()
            .ok_or_else(|| GenerationError::Generic("No embeddings in response".into()))?;

        #[allow(clippy::cast_precision_loss)]
        let emb_usage = UsageReport {
            input: response
                .usage
                .as_ref()
                .map_or(0.0, |u| u.prompt_tokens as f64),
            output: 0.0,
            cache_read: 0.0,
            cache_write: 0.0,
            total_tokens: response
                .usage
                .as_ref()
                .map_or(0.0, |u| u.total_tokens as f64),
            cost: UsageCosting {
                currency: "USD".to_string(),
                input: 0.0,
                output: 0.0,
                cache_read: 0.0,
                cache_write: 0.0,
                total_tokens: 0.0,
                status: CostStatus::Actual,
            },
        };
        let emb_cost = calculate_cost(&self.pricing, &emb_usage, CostStatus::Actual);
        let emb_usage = UsageReport { cost: emb_cost, ..emb_usage };
        self.cumulative_cost.borrow_mut().add(&emb_usage.cost);
        Ok(vec![Messages::Assistant {
            model: self.model_id.clone(),
            timestamp: SystemTime::now(),
            usage: emb_usage,
            content: ModelOutput::Embedding {
                dimensions: data.embedding.len(),
                values: data.embedding.clone(),
            },
            stop_reason: StopReason::Stop,
            provider: ModelProviders::OPENAI,
            error_detail: None,
            signature: None,
            metadata: None,
        }])
    }
}

// ============================================================================
// OpenAI Tool Formatter
// ============================================================================

/// Formatter for `OpenAI`'s native tool calling format.
///
/// Tool defs: `{type: "function", function: {name, description, parameters}}`
/// Tool calls: `{id: "...", type: "function", function: {name, arguments: "..."}}`
/// Tool results: `{role: "tool", tool_call_id: "...", content: "..."}`
#[derive(Default, Clone, Copy)]
pub struct OpenAIFormatter;

impl ToolFormatter for OpenAIFormatter {
    fn format_tools(
        &self,
        tools: &[Tool],
    ) -> Result<serde_json::Value, ErrorTrace<ToolCallingError>> {
        Ok(serde_json::Value::Array(
            tools
                .iter()
                .map(|tool| {
                    // Use the Args schema if present, otherwise default to empty object
                    let parameters = tool.arguments.as_ref().map_or_else(|| serde_json::json!({
                            "type": "object",
                            "properties": {},
                        }), |a| a.schema.clone());
                    serde_json::json!({
                        "type": "function",
                        "function": {
                            "name": &tool.name,
                            "description": tool.description,
                            "parameters": parameters,
                        },
                    })
                })
                .collect(),
        ))
    }

    fn tool_calling_instructions(&self) -> Option<String> {
        None
    }

    fn extract_tool_calls(
        &self,
        response: &str,
    ) -> Result<ExtractResult, ErrorTrace<ToolCallingError>> {
        let parsed: serde_json::Value =
            serde_json::from_str(response).map_err(|e| {
                ErrorTrace::new(ToolCallingError::Extract { reason: e.to_string() })
                    .attach("source=openai_response")
            })?;

        let mut calls = Vec::new();
        let mut remaining_text = None;

        if let Some(choice) = parsed.get("choices").and_then(|v| v.as_array()).and_then(|a| a.first()) {
            if let Some(message) = choice.get("message") {
                // Check for tool_calls
                if let Some(tool_calls) = message.get("tool_calls").and_then(|v| v.as_array()) {
                    for tc in tool_calls {
                        let id = tc.get("id").and_then(|v| v.as_str()).unwrap_or("").to_string();
                        let function = tc.get("function").and_then(|v| v.as_object());
                        let name = function.and_then(|f| f.get("name")).and_then(|v| v.as_str()).unwrap_or("").to_string();
                        let args_str = function.and_then(|f| f.get("arguments")).and_then(|v| v.as_str()).unwrap_or("{}");
                        let arguments: Option<HashMap<String, crate::types::ArgType>> =
                            serde_json::from_str(args_str).ok();
                        calls.push(ModelOutput::ToolCall {
                            id,
                            name,
                            arguments,
                            signature: None,
                        });
                    }
                }
                // Get text content for remaining_text
                if let Some(content) = message.get("content").and_then(|v| v.as_str()) {
                    if !content.is_empty() {
                        remaining_text = Some(content.to_string());
                    }
                }
            }
        }

        let has_tool_calls = !calls.is_empty();
        Ok(ExtractResult { calls, remaining_text, has_tool_calls })
    }

    fn format_tool_response(
        &self,
        result: &Messages,
    ) -> Result<serde_json::Value, ErrorTrace<ToolCallingError>> {
        let Messages::ToolResult {
            id, content, ..
        } = result
        else {
            return Err(ErrorTrace::new(ToolCallingError::Response {
                tool_name: String::new(),
                reason: "expected Messages::ToolResult".to_string(),
            }).attach("source=openai_formatter"));
        };

        let content_str = match content {
            crate::types::UserModelContent::Text(t) => t.content.clone(),
            crate::types::UserModelContent::Image(_) => "[image]".to_string(),
        };

        Ok(serde_json::json!({
            "role": "tool",
            "tool_call_id": id,
            "content": content_str,
        }))
    }
}

impl<F: ToolFormatter, R: DnsResolver + 'static> Model for OpenAIModel<F, R> {
    type Formatter = F;
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
            id: "openai",
            name: "OpenAI",
            reasoning: false,
            api: crate::types::ModelAPI::OpenAICompletions,
            provider: ModelProviders::OPENAI,
            base_url: None,
            inputs: crate::types::MessageType::TextAndImages,
            cost: self.pricing,
            context_window: 0,
            max_tokens: 0,
        })
    }

    fn costing(&self) -> GenerationResult<UsageReport> {
        let cost = self.cumulative_cost.borrow().result();
        Ok(UsageReport {
            input: 0.0,
            output: 0.0,
            cache_read: 0.0,
            cache_write: 0.0,
            total_tokens: cost.total_tokens,
            cost,
        })
    }

    fn generate(
        &self,
        interaction: ModelInteraction,
        specs: Option<ModelParams>,
    ) -> GenerationResult<Vec<Messages>> {
        let params = specs.unwrap_or_default();

        // Check if this is an embedding request (model returns ModelOutput::Embedding)
        if is_embedding_request(&interaction.messages) {
            return self.generate_embeddings(&interaction);
        }

        let request = build_chat_request(&self.model_name, &interaction, &params, false);

        let body = serde_json::to_string(&request)
            .map_err(|e| GenerationError::Generic(format!("Failed to serialize request: {e}")))?;

        let url = self.build_url("chat/completions");
        let response: ChatCompletionResponse = self.execute_request(&url, &body)?;

        let (message, report) = parse_chat_response(&response, &self.model_id, &self.pricing)?;
        self.cumulative_cost.borrow_mut().add(&report.cost);
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

        let url = self.build_url("chat/completions");

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
            pricing: self.pricing,
            cumulative_cost: Rc::clone(&self.cumulative_cost),
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
    pricing: ModelUsageCosting,
    cumulative_cost: Rc<RefCell<CostAccumulator>>,
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

        let item = self.inner.next()?;

        match item {
            Stream::Next(parse_result) => {
                let Event::Message { data, .. } = &parse_result.event else {
                    return Some(Stream::Ignore);
                };

                if data.trim() == "[DONE]" {
                    self.done = true;
                    let (msg, report) = self.build_final_message();
                    self.cumulative_cost.borrow_mut().add(&report.cost);
                    return Some(Stream::Next(msg));
                }

                let Ok(chunk) = serde_json::from_str::<ChatCompletionChunk>(data) else {
                    tracing::warn!(data = %data, "Failed to parse SSE chunk JSON");
                    return Some(Stream::Next(Messages::Assistant {
                        model: self.model_id.clone(),
                        timestamp: SystemTime::now(),
                        usage: empty_usage_report(),
                        content: ModelOutput::Text(TextContent {
                            content: self.accumulated_text.clone(),
                            signature: None,
                        }),
                        stop_reason: StopReason::Error,
                        provider: ModelProviders::OPENAI,
                        error_detail: Some(format!("Failed to parse SSE chunk: {data}")),
                        signature: None,
                        metadata: None,
                    }));
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
                    Some(Stream::Next(Messages::Assistant {
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
                        metadata: None,
                    }))
                } else {
                    Some(Stream::Ignore)
                }
            }
            Stream::Pending(p) => Some(Stream::Pending(match p {
                ReconnectingProgress::Connecting | ReconnectingProgress::Reading => {
                    ModelState::GeneratingTokens(None)
                }
                ReconnectingProgress::Reconnecting => ModelState::GeneratingTokens(None),
            })),
            Stream::Delayed(d) => Some(Stream::Delayed(d)),
            Stream::Init => Some(Stream::Init),
            Stream::Ignore => Some(Stream::Ignore),
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

    fn build_final_message(&self) -> (Messages, UsageReport) {
        let stop_reason = match self.finish_reason.as_deref() {
            Some("stop") | None => StopReason::Stop,
            Some("length") => StopReason::Length,
            Some("tool_calls") => StopReason::ToolUse,
            Some(reason) => StopReason::Message(reason.to_string()),
        };

        let usage_report = self
            .usage
            .as_ref().map_or_else(empty_usage_report, |u| {
                #[allow(clippy::cast_precision_loss)]
                let usage = UsageReport {
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
                        status: CostStatus::Actual,
                    },
                };
                let costing = calculate_cost(&self.pricing, &usage, CostStatus::Actual);
                UsageReport { cost: costing, ..usage }
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

        let msg = Messages::Assistant {
            model: self.model_id.clone(),
            timestamp: SystemTime::now(),
            usage: usage_report.clone(),
            content,
            stop_reason,
            provider: ModelProviders::OPENAI,
            error_detail: None,
            signature: None,
            metadata: None,
        };
        (msg, usage_report)
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
    pub tool_choice: Option<OpenAIToolChoice>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub n: Option<u32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub seed: Option<u32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub frequency_penalty: Option<f32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub presence_penalty: Option<f32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub logit_bias: Option<HashMap<String, i32>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub response_format: Option<OpenAIResponseFormat>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub logprobs: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub top_logprobs: Option<usize>,
}

/// Wire format for `OpenAI` `response_format` field.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum OpenAIResponseFormat {
    Text,
    JsonObject,
    JsonSchema {
        #[serde(skip_serializing_if = "Option::is_none")]
        json_schema: Option<OpenAIJsonSchema>,
    },
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OpenAIJsonSchema {
    pub name: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,
    pub schema: serde_json::Value,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub strict: Option<bool>,
}

/// Wire format for `OpenAI` `tool_choice` field.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(untagged)]
pub enum OpenAIToolChoice {
    /// `"auto"` or `"none"` or `"required"`
    Simple(String),
    /// `{ "type": "function", "function": { "name": "..." } }`
    Function {
        #[serde(rename = "type")]
        type_: String,
        function: OpenAIToolChoiceFunction,
    },
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OpenAIToolChoiceFunction {
    pub name: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OpenAIMessage {
    pub role: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub content: Option<OpenAIMessageContent>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub tool_calls: Option<Vec<OpenAIToolCall>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub tool_call_id: Option<String>,
    /// Refusal text when model declines due to safety/policy.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub refusal: Option<String>,
}

/// Content for an `OpenAI` message — either simple text or multimodal parts.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(untagged)]
pub enum OpenAIMessageContent {
    /// Simple text content (shorthand form).
    Text(String),
    /// Array of content parts for multimodal messages.
    Parts(Vec<OpenAIContentPart>),
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum OpenAIContentPart {
    Text { text: String },
    ImageUrl { image_url: OpenAIImageUrlObject },
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct OpenAIImageUrlObject {
    /// Data URL (base64) or HTTPS URL.
    pub url: String,
    /// Detail level: "low", "high", or "auto".
    #[serde(skip_serializing_if = "Option::is_none")]
    pub detail: Option<String>,
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
    #[serde(skip_serializing_if = "Option::is_none")]
    pub system_fingerprint: Option<String>,
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
    #[serde(skip_serializing_if = "Option::is_none")]
    pub logprobs: Option<OpenAILogProbs>,
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

/// Wire format — `OpenAI`'s logprobs in the response.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OpenAILogProbs {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub content: Option<Vec<OpenAIContentLogProb>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub refusal: Option<Vec<OpenAIRefusalLogProb>>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OpenAIContentLogProb {
    pub token: String,
    pub logprob: f32,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub bytes: Option<Vec<u8>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub top_logprobs: Option<Vec<OpenAITopLogProb>>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OpenAITopLogProb {
    pub token: String,
    pub logprob: f32,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub bytes: Option<Vec<u8>>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OpenAIRefusalLogProb {
    pub token: String,
    pub logprob: f32,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub bytes: Option<Vec<u8>>,
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

// ============================================================================
// Retry & Error Helpers
// ============================================================================

/// Check if the user is requesting embeddings by looking for a marker message.
fn is_embedding_request(messages: &[Messages]) -> bool {
    messages.iter().any(|msg| {
        if let Messages::Assistant { content, .. } = msg {
            matches!(content, ModelOutput::Embedding { .. })
        } else {
            false
        }
    })
}

fn is_retryable_status(status: u16) -> bool {
    status == 429 || (500..=503).contains(&status)
}

fn exponential_backoff(attempt: u32) -> u64 {
    let base_secs: u64 = 1 << attempt.min(5); // 1, 2, 4, 8, 16, 32
    base_secs.min(30) // cap at 30s
}

fn extract_retry_after(headers: &SimpleHeaders) -> Option<u64> {
    let header = SimpleHeader::from("Retry-After".to_string());
    headers
        .get(&header)
        .and_then(|values| values.first())
        .and_then(|v| v.parse::<u64>().ok())
}

fn parse_openai_error(body: &str) -> Option<String> {
    serde_json::from_str::<OpenAIErrorResponse>(body)
        .ok()
        .map(|e| e.error.message)
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

/// Flatten a `ToolShed` into a flat Vec<Tool> for provider APIs.
#[must_use]
pub fn flatten_tools(shed: &ToolShed) -> Vec<Tool> {
    let mut tools = vec![
        shed.shed.clone(),
        shed.read.clone(),
        shed.edit.clone(),
        shed.write.clone(),
        shed.search.clone(),
    ];
    if let Some(mem) = &shed.memory {
        tools.push(mem.add.clone());
        tools.push(mem.replace.clone());
        tools.push(mem.remove.clone());
    }
    if let Some(delegate) = &shed.delegate {
        tools.push(delegate.start.clone());
        tools.push(delegate.check.clone());
        tools.push(delegate.get.clone());
    }
    if let Some(bash) = &shed.bash {
        tools.push(bash.clone());
    }
    if let Some(others) = &shed.others {
        tools.extend(others.iter().cloned());
    }
    tools
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
            status: CostStatus::Actual,
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
#[allow(clippy::cast_possible_truncation, clippy::cast_sign_loss)]
fn build_chat_request(
    model_name: &str,
    interaction: &ModelInteraction,
    params: &ModelParams,
    streaming: bool,
) -> ChatCompletionRequest {
    let mut messages = Vec::new();

    // System: combine system_prompt + soul
    let system_content = match (&interaction.system_prompt, &interaction.soul) {
        (Some(sys), Some(soul)) => Some(format!("{sys}\n\n{soul}")),
        (Some(sys), None) => Some(sys.clone()),
        (None, Some(soul)) => Some(soul.clone()),
        (None, None) => None,
    };
    if let Some(content) = system_content {
        messages.push(OpenAIMessage {
            role: String::from("system"),
            content: Some(OpenAIMessageContent::Text(content)),
            tool_calls: None,
            tool_call_id: None,
            refusal: None,
        });
    }

    for msg in &interaction.messages {
        match msg {
            Messages::User { content, .. } => {
                let msg_content = match content {
                    crate::types::UserModelContent::Text(tc) => {
                        OpenAIMessageContent::Text(tc.content.clone())
                    }
                    crate::types::UserModelContent::Image(img) => {
                        let mime_str = match img.mime_type {
                            #[allow(clippy::match_same_arms)]
                            crate::types::MimeType::ImagePng => "image/png",
                            crate::types::MimeType::ImageJpeg => "image/jpeg",
                            crate::types::MimeType::ImageGif => "image/gif",
                            crate::types::MimeType::ImageWebp => "image/webp",
                            _ => "image/png",
                        };
                        let data_url = format!("data:{};base64,{}", mime_str, img.b64);
                        OpenAIMessageContent::Parts(vec![OpenAIContentPart::ImageUrl {
                            image_url: OpenAIImageUrlObject {
                                url: data_url,
                                detail: Some(String::from("auto")),
                            },
                        }])
                    }
                };
                messages.push(OpenAIMessage {
                    role: String::from("user"),
                    content: Some(msg_content),
                    tool_calls: None,
                    tool_call_id: None,
                    refusal: None,
                });
            }
            Messages::Assistant { content, .. } => match content {
                ModelOutput::Text(tc) => {
                    messages.push(OpenAIMessage {
                        role: String::from("assistant"),
                        content: Some(OpenAIMessageContent::Text(tc.content.clone())),
                        tool_calls: None,
                        tool_call_id: None,
                        refusal: None,
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
                        refusal: None,
                    });
                }
                ModelOutput::ThinkingContent { thinking, .. } => {
                    messages.push(OpenAIMessage {
                        role: String::from("assistant"),
                        content: Some(OpenAIMessageContent::Text(thinking.clone())),
                        tool_calls: None,
                        tool_call_id: None,
                        refusal: None,
                    });
                }
                ModelOutput::Image(img) => {
                    #[allow(clippy::match_same_arms)]
                    let mime_str = match img.mime_type {
                        crate::types::MimeType::ImagePng => "image/png",
                        crate::types::MimeType::ImageJpeg => "image/jpeg",
                        crate::types::MimeType::ImageGif => "image/gif",
                        crate::types::MimeType::ImageWebp => "image/webp",
                        _ => "image/png",
                    };
                    let data_url = format!("data:{};base64,{}", mime_str, img.b64);
                    messages.push(OpenAIMessage {
                        role: String::from("assistant"),
                        content: Some(OpenAIMessageContent::Parts(vec![
                            OpenAIContentPart::ImageUrl {
                                image_url: OpenAIImageUrlObject {
                                    url: data_url,
                                    detail: Some(String::from("auto")),
                                },
                            },
                        ])),
                        tool_calls: None,
                        tool_call_id: None,
                        refusal: None,
                    });
                }
                ModelOutput::Embedding { .. } => {}
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
                    content: Some(OpenAIMessageContent::Text(format!("[{name}] {text}"))),
                    tool_calls: None,
                    tool_call_id: Some(id.clone()),
                    refusal: None,
                });
            }
        }
    }

    let tools = interaction.tools_shed.as_ref().map(|shed| {
        flatten_tools(shed)
            .iter()
            .map(|tool| OpenAITool {
                tool_type: String::from("function"),
                function: OpenAIFunction {
                    name: tool.name.clone(),
                    description: Some(tool.description.clone()),
                    parameters: tool.arguments.as_ref()
                        .map(|a| a.schema.clone()),
                },
            })
            .collect::<Vec<_>>()
    }).filter(|t: &Vec<OpenAITool>| !t.is_empty());

    let response_format = params.output_format.as_ref().map(|fmt| match fmt {
        crate::types::OutputFormat::Text => OpenAIResponseFormat::Text,
        crate::types::OutputFormat::JsonObject => OpenAIResponseFormat::JsonObject,
        crate::types::OutputFormat::JsonSchema(js) => OpenAIResponseFormat::JsonSchema {
            json_schema: Some(OpenAIJsonSchema {
                name: js.name.clone(),
                description: js.description.clone(),
                schema: js.schema.clone(),
                strict: js.strict,
            }),
        },
    });

    let tool_choice = interaction.tool_choice.as_ref().map(|tc| match tc {
        crate::types::ToolChoice::Auto => OpenAIToolChoice::Simple(String::from("auto")),
        crate::types::ToolChoice::None => OpenAIToolChoice::Simple(String::from("none")),
        crate::types::ToolChoice::Required => OpenAIToolChoice::Simple(String::from("required")),
        crate::types::ToolChoice::Function(f) => OpenAIToolChoice::Function {
            type_: f.tool_type.clone(),
            function: OpenAIToolChoiceFunction {
                name: f.function.name.clone(),
            },
        },
    });

    let logit_bias = params.logit_bias.as_ref().map(|bias| {
        bias.iter()
            .map(|(k, v)| (k.clone(), (*v).round() as i32))
            .collect()
    });

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
        tool_choice,
        n: Some(1),
        seed: params.seed,
        frequency_penalty: params.frequency_penalty,
        presence_penalty: params.presence_penalty,
        logit_bias,
        response_format,
        logprobs: None,
        top_logprobs: None,
    }
}

fn parse_chat_response(
    response: &ChatCompletionResponse,
    model_id: &ModelId,
    pricing: &ModelUsageCosting,
) -> GenerationResult<(Messages, UsageReport)> {
    let choice = response
        .choices
        .first()
        .ok_or_else(|| GenerationError::Generic("No choices in response".into()))?;

    let message = choice
        .message
        .as_ref()
        .ok_or_else(|| GenerationError::Generic("No message in response choice".into()))?;

    let content = match &message.content {
        Some(OpenAIMessageContent::Text(text)) => text.clone(),
        Some(OpenAIMessageContent::Parts(parts)) => parts
            .iter()
            .filter_map(|p| match p {
                OpenAIContentPart::Text { text } => Some(text.clone()),
                OpenAIContentPart::ImageUrl { .. } => None,
            })
            .collect::<Vec<_>>()
            .join("\n"),
        None => String::new(),
    };

    let stop_reason = match choice.finish_reason.as_deref() {
        Some("stop") | None => StopReason::Stop,
        Some("length") => StopReason::Length,
        Some("tool_calls") => StopReason::ToolUse,
        Some("content_filter") => StopReason::Error,
        Some(other) => StopReason::Message(other.to_string()),
    };

    let usage_report = response
        .usage
        .as_ref().map_or_else(empty_usage_report, |u| {
            #[allow(clippy::cast_precision_loss)]
            let usage = UsageReport {
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
                    status: CostStatus::Actual,
                },
            };
            let costing = calculate_cost(pricing, &usage, CostStatus::Actual);
            UsageReport { cost: costing, ..usage }
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

    Ok((
        Messages::Assistant {
            model: model_id.clone(),
            timestamp: SystemTime::now(),
            usage: usage_report.clone(),
            content: output,
            stop_reason,
            provider: ModelProviders::OPENAI,
            error_detail: None,
            signature: None,
            metadata: build_metadata(choice.logprobs.as_ref(), response.system_fingerprint.as_ref(), message.refusal.as_ref()),
        },
        usage_report,
    ))
}

fn build_metadata(
    logprobs: Option<&OpenAILogProbs>,
    system_fingerprint: Option<&String>,
    refusal: Option<&String>,
) -> Option<Vec<crate::types::GenerationMetadata>> {
    let mut metadata = Vec::new();

    if let Some(lp) = logprobs {
        let content: Vec<crate::types::ContentLogProb> = lp
            .content
            .as_ref()
            .map(|items| {
                items
                    .iter()
                    .map(|p| crate::types::ContentLogProb {
                        token: p.token.clone(),
                        logprob: p.logprob,
                        bytes: p.bytes.clone(),
                        top_logprobs: p.top_logprobs.as_ref().map(|tops| {
                            tops.iter()
                                .map(|t| crate::types::TopLogProbEntry {
                                    token: t.token.clone(),
                                    logprob: t.logprob,
                                    bytes: t.bytes.clone(),
                                })
                                .collect()
                        }),
                    })
                    .collect()
            })
            .unwrap_or_default();

        let refusal_probs: Option<Vec<crate::types::RefusalLogProb>> =
            lp.refusal.as_ref().map(|items| {
                items
                    .iter()
                    .map(|p| crate::types::RefusalLogProb {
                        token: p.token.clone(),
                        logprob: p.logprob,
                        bytes: p.bytes.clone(),
                    })
                    .collect()
            });

        if !content.is_empty() || refusal_probs.is_some() {
            metadata.push(crate::types::GenerationMetadata::LogProbs {
                content,
                refusal: refusal_probs,
            });
        }
    }

    if let Some(fp) = system_fingerprint {
        metadata.push(crate::types::GenerationMetadata::SystemFingerprint(
            fp.clone(),
        ));
    }

    if let Some(reason) = refusal {
        metadata.push(crate::types::GenerationMetadata::RefusalReason(
            reason.clone(),
        ));
    }

    if metadata.is_empty() {
        None
    } else {
        Some(metadata)
    }
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
                    content: Some(OpenAIMessageContent::Text("You are helpful".into())),
                    tool_calls: None,
                    tool_call_id: None,
                    refusal: None,
                },
                OpenAIMessage {
                    role: "user".into(),
                    content: Some(OpenAIMessageContent::Text("Hello".into())),
                    tool_calls: None,
                    tool_call_id: None,
                    refusal: None,
                },
            ],
            temperature: Some(0.7),
            top_p: Some(0.9),
            max_tokens: Some(100),
            stop: None,
            stream: Some(false),
            tools: None,
            tool_choice: None,
            n: Some(1),
            seed: None,
            frequency_penalty: None,
            presence_penalty: None,
            logit_bias: None,
            response_format: None,
            logprobs: None,
            top_logprobs: None,
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
            Some(OpenAIMessageContent::Text("Hello!".into()))
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

        let float = json_value_to_arg_type(&serde_json::json!(3.64));
        assert!(
            matches!(float, crate::types::ArgType::Float64(f) if (f - 3.64).abs() < f64::EPSILON)
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
                    refusal: None,
                }),
                finish_reason: Some("tool_calls".into()),
                logprobs: None,
            }],
            usage: Some(OpenAIUsage {
                prompt_tokens: 10,
                completion_tokens: 5,
                total_tokens: 15,
            }),
            system_fingerprint: None,
        };

        let model_id = ModelId::Name("gpt-4".into(), None);
        let (msg, _report) = parse_chat_response(&response, &model_id, &ModelUsageCosting::default()).unwrap();

        if let Messages::Assistant {
            content,
            stop_reason,
            ..
        } = &msg
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
                    content: Some(OpenAIMessageContent::Text("Hello!".into())),
                    tool_calls: None,
                    tool_call_id: None,
                    refusal: None,
                }),
                finish_reason: Some("stop".into()),
                logprobs: None,
            }],
            usage: Some(OpenAIUsage {
                prompt_tokens: 5,
                completion_tokens: 2,
                total_tokens: 7,
            }),
            system_fingerprint: None,
        };

        let model_id = ModelId::Name("gpt-4".into(), None);
        let (msg, _report) = parse_chat_response(&response, &model_id, &ModelUsageCosting::default()).unwrap();

        if let Messages::Assistant {
            content,
            stop_reason,
            usage,
            metadata: _,
            ..
        } = &msg
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
    fn test_parse_openai_error_format() {
        let body = r#"{"error":{"message":"Invalid API key","type":"invalid_request_error","code":"invalid_api_key"}}"#;
        let detail = parse_openai_error(body).expect("Should parse OpenAI error");
        assert_eq!(detail, "Invalid API key");
    }

    #[test]
    fn test_parse_openai_error_plain_text_fallback() {
        let detail = parse_openai_error("Internal Server Error");
        assert!(detail.is_none());
    }

    #[test]
    fn test_format_http_error_auth() {
        let msg = format_http_error(401, "Invalid API key");
        assert!(msg.contains("Authentication failed"), "got: {msg}");
        assert!(msg.contains("Invalid API key"), "got: {msg}");
    }

    #[test]
    fn test_format_http_error_rate_limit() {
        let msg = format_http_error(429, "Rate limit reached");
        assert!(msg.contains("Rate limit exceeded"), "got: {msg}");
    }

    #[test]
    fn test_format_http_error_server_error() {
        let msg = format_http_error(500, "Internal Server Error");
        assert!(msg.contains("Server error"), "got: {msg}");
        assert!(msg.contains("Internal Server Error"), "got: {msg}");
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
    fn test_response_format_serialization() {
        let fmt = OpenAIResponseFormat::Text;
        let json = serde_json::to_string(&fmt).unwrap();
        assert_eq!(json, r#"{"type":"text"}"#);

        let fmt = OpenAIResponseFormat::JsonObject;
        let json = serde_json::to_string(&fmt).unwrap();
        assert_eq!(json, r#"{"type":"json_object"}"#);

        let fmt = OpenAIResponseFormat::JsonSchema {
            json_schema: Some(OpenAIJsonSchema {
                name: "test_schema".into(),
                description: Some("A test schema".into()),
                schema: serde_json::json!({"type": "object", "properties": {"name": {"type": "string"}}}),
                strict: Some(true),
            }),
        };
        let json = serde_json::to_string(&fmt).unwrap();
        assert!(json.contains(r#""type":"json_schema""#));
        assert!(json.contains(r#""name":"test_schema""#));
        assert!(json.contains(r#""strict":true"#));
    }

    #[test]
    fn test_tool_choice_serialization() {
        let tc = OpenAIToolChoice::Simple("auto".into());
        let json = serde_json::to_string(&tc).unwrap();
        assert_eq!(json, r#""auto""#);

        let tc = OpenAIToolChoice::Simple("none".into());
        let json = serde_json::to_string(&tc).unwrap();
        assert_eq!(json, r#""none""#);

        let tc = OpenAIToolChoice::Simple("required".into());
        let json = serde_json::to_string(&tc).unwrap();
        assert_eq!(json, r#""required""#);

        let tc = OpenAIToolChoice::Function {
            type_: "function".into(),
            function: OpenAIToolChoiceFunction {
                name: "get_weather".into(),
            },
        };
        let json = serde_json::to_string(&tc).unwrap();
        assert!(json.contains(r#""type":"function""#));
        assert!(json.contains(r#""name":"get_weather""#));
    }

    #[test]
    fn test_build_chat_request_with_new_fields() {
        use crate::types::{OutputFormat, ToolChoice, ToolChoiceFunction, ToolFunctionRef};

        let mut interaction = ModelInteraction {
            system_prompt: Some("You are helpful".into()),
            soul: Some("Be concise and technical".into()),
            messages: vec![crate::types::Messages::User {
                role: "user".into(),
                content: crate::types::UserModelContent::Text(TextContent {
                    content: "Hello".into(),
                    signature: None,
                }),
                signature: None,
            }],
            tools_shed: None,
            chat_template: None,
            tool_choice: Some(ToolChoice::Auto),
        };
        let mut params = ModelParams::default();
        params.output_format = Some(OutputFormat::JsonObject);
        params.frequency_penalty = Some(0.5);
        params.presence_penalty = Some(0.3);
        let mut logit_bias = std::collections::HashMap::new();
        logit_bias.insert("50256".to_string(), -100.0);
        params.logit_bias = Some(logit_bias);

        let request = build_chat_request("gpt-4", &interaction, &params, false);

        assert!(request.response_format.is_some());
        let fmt = request.response_format.unwrap();
        assert!(serde_json::to_string(&fmt).unwrap().contains("json_object"));

        assert_eq!(request.frequency_penalty, Some(0.5));
        assert_eq!(request.presence_penalty, Some(0.3));
        assert!(request.logit_bias.is_some());
        assert!(request.tool_choice.is_some());

        // Now test forced function
        interaction.tool_choice = Some(ToolChoice::Function(ToolChoiceFunction {
            tool_type: "function".into(),
            function: ToolFunctionRef {
                name: "get_weather".into(),
            },
        }));
        let request2 = build_chat_request("gpt-4", &interaction, &params, false);
        let tc = request2.tool_choice.unwrap();
        let json = serde_json::to_string(&tc).unwrap();
        assert!(json.contains("get_weather"));
    }

    #[test]
    fn test_multimodal_message_serialization() {
        let msg = OpenAIMessage {
            role: "user".into(),
            content: Some(OpenAIMessageContent::Parts(vec![
                OpenAIContentPart::Text {
                    text: "What is this?".into(),
                },
                OpenAIContentPart::ImageUrl {
                    image_url: OpenAIImageUrlObject {
                        url: "data:image/png;base64,iVBORw0KGgo".into(),
                        detail: Some("auto".into()),
                    },
                },
            ])),
            tool_calls: None,
            tool_call_id: None,
            refusal: None,
        };

        let json = serde_json::to_string(&msg).unwrap();
        assert!(json.contains(r#""type":"text""#));
        assert!(json.contains(r#""type":"image_url""#));
        assert!(json.contains("data:image/png;base64,"));
        assert!(json.contains(r#""detail":"auto""#));
    }

    #[test]
    fn test_text_only_message_serialization() {
        let msg = OpenAIMessage {
            role: "user".into(),
            content: Some(OpenAIMessageContent::Text("Hello".into())),
            tool_calls: None,
            tool_call_id: None,
            refusal: None,
        };

        let json = serde_json::to_string(&msg).unwrap();
        // Text variant serializes as simple string, not object
        assert!(json.contains(r#""content":"Hello""#));
    }
}
