//! `OpenAI` Responses API provider for reasoning models (o1, o3, o1-pro, etc.).
//!
//! Implements the `/v1/responses` endpoint using Valtron `TaskIterator`/`StreamIterator`
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
    AuthProvider, GenerationMetadata, Messages, Model, ModelId, ModelInteraction, ModelOutput,
    ModelParams, ModelProvider, ModelProviderDescriptor, ModelProviders, ModelSpec, ModelState,
    StopReason, TextContent, CostStatus, ToolShed, UsageCosting, UsageReport,
};

// ============================================================================
// Configuration (reuses OpenAIConfig pattern)
// ============================================================================

/// Configuration for the Responses API provider.
#[derive(Debug)]
pub struct ResponsesConfig {
    pub base_url: String,
    pub api_version: String,
    pub timeout_secs: u64,
    pub max_retries: u32,
    pub proxy_url: Option<String>,
    pub streaming: bool,
    pub auth: Option<AuthCredential>,
}

impl Default for ResponsesConfig {
    fn default() -> Self {
        Self {
            base_url: String::from("https://api.openai.com"),
            api_version: String::from("v1"),
            timeout_secs: 120,
            max_retries: 3,
            proxy_url: None,
            streaming: true,
            auth: None,
        }
    }
}

impl ResponsesConfig {
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

impl Clone for ResponsesConfig {
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

impl AuthProvider for ResponsesConfig {
    fn auth(&self) -> Option<&AuthCredential> {
        self.auth.as_ref()
    }
}

// ============================================================================
// Request Types
// ============================================================================

#[derive(Debug, Clone, Serialize)]
pub struct ResponseRequest {
    pub model: String,
    pub input: ResponseInput,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub instructions: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub tools: Option<Vec<ResponseTool>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub tool_choice: Option<ResponseToolChoice>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub max_output_tokens: Option<usize>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub temperature: Option<f32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub top_p: Option<f32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub stream: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub truncate: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub previous_response_id: Option<String>,
}

/// Tool definition for the Responses API.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ResponseTool {
    #[serde(rename = "type")]
    pub tool_type: String,
    pub function: ResponseFunction,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ResponseFunction {
    pub name: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub parameters: Option<serde_json::Value>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub strict: Option<bool>,
}

/// Tool choice for the Responses API.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(untagged)]
pub enum ResponseToolChoice {
    Simple(String),
    Function {
        #[serde(rename = "type")]
        r#type: String,
        function: ResponseToolChoiceFunction,
    },
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ResponseToolChoiceFunction {
    pub name: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(untagged)]
pub enum ResponseInput {
    Text(String),
    Items(Vec<ResponseInputItem>),
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum ResponseInputItem {
    Message {
        role: String,
        content: ResponseInputContent,
    },
    FunctionCallOutput {
        call_id: String,
        output: String,
    },
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(untagged)]
pub enum ResponseInputContent {
    Text(String),
    Parts(Vec<ResponseInputContentPart>),
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum ResponseInputContentPart {
    InputText { text: String },
    InputImage { image_url: String },
}

// ============================================================================
// Response Types
// ============================================================================

#[derive(Debug, Clone, Deserialize)]
pub struct Response {
    pub id: String,
    pub object: String,
    pub created_at: u64,
    pub model: String,
    pub output: Vec<ResponseOutputItem>,
    pub status: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error: Option<ResponseError>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub usage: Option<ResponseUsage>,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum ResponseOutputItem {
    Message {
        id: String,
        status: String,
        role: String,
        content: Vec<ResponseOutputContent>,
    },
    FunctionCall {
        id: String,
        call_id: String,
        name: String,
        arguments: String,
        status: String,
    },
    Reasoning {
        id: String,
        content: String,
    },
}

#[derive(Debug, Clone, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum ResponseOutputContent {
    OutputText { text: String },
}

#[derive(Debug, Clone, Deserialize)]
pub struct ResponseUsage {
    pub input_tokens: usize,
    pub output_tokens: usize,
    pub total_tokens: usize,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub reasoning_tokens: Option<usize>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct ResponseError {
    pub code: String,
    pub message: String,
}

// ============================================================================
// Streaming Event Types
// ============================================================================

#[derive(Debug, Clone, Deserialize)]
#[serde(tag = "type")]
pub enum ResponseEvent {
    #[serde(rename = "response.created")]
    ResponseCreated { response: Response },
    #[serde(rename = "response.in_progress")]
    ResponseInProgress { response: Response },
    #[serde(rename = "response.output_item.added")]
    ResponseOutputItemAdded {
        response_id: String,
        item: ResponseOutputItem,
    },
    #[serde(rename = "response.output_item.done")]
    ResponseOutputItemDone {
        response_id: String,
        item: ResponseOutputItem,
    },
    #[serde(rename = "response.output_text.delta")]
    ResponseOutputTextDelta {
        item_id: String,
        delta: String,
    },
    #[serde(rename = "response.output_text.done")]
    ResponseOutputTextDone {
        item_id: String,
        text: String,
    },
    #[serde(rename = "response.completed")]
    ResponseCompleted { response: Response },
    #[serde(rename = "response.failed")]
    ResponseFailed { response: Response },
}

// ============================================================================
// Provider
// ============================================================================

/// `OpenAI` Responses API provider implementing [`ModelProvider`].
pub struct ResponsesProvider<R: DnsResolver = SystemDnsResolver> {
    config: ResponsesConfig,
    api_key: Option<ConfidentialText>,
    http_client: Option<SimpleHttpClient<R>>,
    resolver: Option<R>,
    models_cache: Arc<std::sync::Mutex<HashMap<String, crate::backends::openai_provider::OpenAIModelInfo>>>,
}

impl Default for ResponsesProvider<SystemDnsResolver> {
    fn default() -> Self {
        Self::new()
    }
}

impl ResponsesProvider<SystemDnsResolver> {
    #[must_use]
    pub fn new() -> Self {
        Self {
            config: ResponsesConfig::default(),
            api_key: None,
            http_client: None,
            resolver: Some(SystemDnsResolver),
            models_cache: Arc::new(std::sync::Mutex::new(HashMap::new())),
        }
    }

    #[must_use]
    pub fn with_config(config: ResponsesConfig) -> Self {
        Self {
            config,
            api_key: None,
            http_client: None,
            resolver: Some(SystemDnsResolver),
            models_cache: Arc::new(std::sync::Mutex::new(HashMap::new())),
        }
    }
}

impl<R: DnsResolver + 'static> ResponsesProvider<R> {
    #[must_use]
    pub fn with_resolver(resolver: R) -> Self {
        Self {
            config: ResponsesConfig::default(),
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
            let msg = format!("HTTP {status_code}: {body_text}");
            return Ok(Err((status_code as u16, retry_after, msg)));
        }

        serde_json::from_str(&body_text)
            .map(|v| Ok(v))
            .map_err(|e| GenerationError::Generic(format!("Parse error: {e}")))
    }
}

impl<R: DnsResolver + Default + 'static> ModelProvider for ResponsesProvider<R> {
    type Config = ResponsesConfig;
    type Model = ResponsesModel<R>;

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
                            "Responses provider requires SecretOnly, ClientSecret, or OAuth credentials"
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
            id: "openai-responses",
            name: "OpenAI Responses",
            reasoning: true,
            api: crate::types::ModelAPI::OpenAIResponses,
            provider: ModelProviders::OPENAIRESPONSES,
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
            return Ok(ResponsesModel {
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

        let url = self.build_url(&format!("models/{model_name}"));
        let result: Result<crate::backends::openai_provider::OpenAIModelResponse, _> =
            self.execute_request(&url, "");

        let info = match result {
            Ok(resp) => crate::backends::openai_provider::OpenAIModelInfo {
                id: resp.id,
                object: resp.object,
                owned_by: resp.owned_by.unwrap_or_default(),
                created: resp.created.unwrap_or(0),
            },
            Err(_) => crate::backends::openai_provider::OpenAIModelInfo {
                id: model_name.clone(),
                object: String::from("model"),
                owned_by: String::new(),
                created: 0,
            },
        };

        let mut cache = self.models_cache.lock().expect("model cache poisoned");
        cache.insert(model_name.clone(), info.clone());

        Ok(ResponsesModel {
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
        use crate::backends::openai_provider::OpenAIListResponse;

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
// Model
// ============================================================================

pub struct ResponsesModel<R: DnsResolver = SystemDnsResolver> {
    config: ResponsesConfig,
    model_id: ModelId,
    model_name: String,
    api_key: Option<ConfidentialText>,
    http_client: Option<SimpleHttpClient<R>>,
    resolver: Option<R>,
    #[allow(dead_code)]
    info: crate::backends::openai_provider::OpenAIModelInfo,
}

impl<R: DnsResolver + 'static> ResponsesModel<R> {
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

    fn build_request(
        &self,
        interaction: &ModelInteraction,
        params: &ModelParams,
        streaming: bool,
    ) -> ResponseRequest {
        let input = build_response_input(interaction);

        // Instructions: combine system_prompt + soul
        let instructions = match (&interaction.system_prompt, &interaction.soul) {
            (Some(sys), Some(soul)) => Some(format!("{sys}\n\n{soul}")),
            (Some(sys), None) => Some(sys.clone()),
            (None, Some(soul)) => Some(soul.clone()),
            (None, None) => None,
        };

        // Tools: flatten ToolShed into ResponseTool array
        let tools = interaction.tools_shed.as_ref().map(|shed| {
            flatten_tools(shed)
                .iter()
                .map(|tool| ResponseTool {
                    tool_type: String::from("function"),
                    function: ResponseFunction {
                        name: tool.name.clone(),
                        description: Some(tool.description.clone()),
                        parameters: tool.arguments.as_ref()
                            .map(|a| a.schema.clone()),
                        strict: None,
                    },
                })
                .collect::<Vec<_>>()
        }).filter(|t: &Vec<ResponseTool>| !t.is_empty());

        // Tool choice
        let tool_choice = interaction.tool_choice.as_ref().map(convert_tool_choice);

        ResponseRequest {
            model: self.model_name.clone(),
            input,
            instructions,
            tools,
            tool_choice,
            max_output_tokens: if params.max_tokens > 0 {
                Some(params.max_tokens)
            } else {
                None
            },
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
            stream: Some(streaming),
            truncate: None,
            previous_response_id: None,
        }
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
            let msg = format!("HTTP {status_code}: {body_text}");
            return Ok(Err((status_code as u16, None, msg)));
        }

        serde_json::from_str(&body_text)
            .map(|v| Ok(v))
            .map_err(|e| GenerationError::Generic(format!("Parse error: {e}")))
    }
}

impl<R: DnsResolver + 'static> Model for ResponsesModel<R> {
    type Formatter = crate::types::TextBasedFormatter;
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
        None
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
        let request = self.build_request(&interaction, &params, false);

        let body = serde_json::to_string(&request)
            .map_err(|e| GenerationError::Generic(format!("Failed to serialize request: {e}")))?;

        let url = self.build_url("responses");
        let response: Response = self.execute_request(&url, &body)?;

        let message = parse_response(&response, &self.model_id);
        Ok(vec![message])
    }

    fn stream(
        &self,
        interaction: ModelInteraction,
        specs: Option<ModelParams>,
    ) -> GenerationResult<impl StreamIterator<D = Messages, P = ModelState>> {
        let params = specs.unwrap_or_default();
        let request = self.build_request(&interaction, &params, true);

        let body = serde_json::to_string(&request)
            .map_err(|e| GenerationError::Generic(format!("Failed to serialize request: {e}")))?;

        let url = self.build_url("responses");

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

        Ok(ResponsesStream {
            inner: driven,
            model_id: self.model_id.clone(),
            accumulated_text: String::new(),
            response: None,
            done: false,
        })
    }
}

// ============================================================================
// Streaming Parser
// ============================================================================

struct ResponsesStream<R: DnsResolver + 'static> {
    inner: foundation_core::valtron::DrivenStreamIterator<ReconnectingEventSourceTask<R>>,
    model_id: ModelId,
    accumulated_text: String,
    response: Option<Response>,
    done: bool,
}

impl<R: DnsResolver + Send + 'static> Iterator for ResponsesStream<R> {
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

                    let Ok(event) = serde_json::from_str::<ResponseEvent>(data) else {
                        tracing::warn!(data = %data, "Failed to parse SSE chunk JSON in Responses API");
                        return Some(Stream::Next(Messages::Assistant {
                            model: self.model_id.clone(),
                            timestamp: SystemTime::now(),
                            usage: empty_usage_report(),
                            content: ModelOutput::Text(TextContent {
                                content: self.accumulated_text.clone(),
                                signature: None,
                            }),
                            stop_reason: StopReason::Error,
                            provider: ModelProviders::OPENAIRESPONSES,
                            error_detail: Some(format!("Failed to parse SSE chunk: {data}")),
                            signature: None,
                            metadata: None,
                        }));
                    };

                    match event {
                        ResponseEvent::ResponseOutputTextDelta { delta, .. } => {
                            self.accumulated_text.push_str(&delta);
                            return Some(Stream::Next(Messages::Assistant {
                                model: self.model_id.clone(),
                                timestamp: SystemTime::now(),
                                usage: empty_usage_report(),
                                content: ModelOutput::Text(TextContent {
                                    content: self.accumulated_text.clone(),
                                    signature: None,
                                }),
                                stop_reason: StopReason::Stop,
                                provider: ModelProviders::OPENAIRESPONSES,
                                error_detail: None,
                                signature: None,
                                metadata: None,
                            }));
                        }
                        ResponseEvent::ResponseCompleted { response }
                        | ResponseEvent::ResponseFailed { response } => {
                            self.response = Some(response);
                            self.done = true;
                            return Some(Stream::Next(self.build_final_message()));
                        }
                        _ => {}
                    }
                }
                Stream::Pending(_) | Stream::Delayed(_) | Stream::Init | Stream::Ignore => {}
            }
        }
    }
}

impl<R: DnsResolver + 'static> ResponsesStream<R> {
    fn build_final_message(&self) -> Messages {
        let Some(response) = &self.response else {
            return Messages::Assistant {
                model: self.model_id.clone(),
                timestamp: SystemTime::now(),
                usage: empty_usage_report(),
                content: ModelOutput::Text(TextContent {
                    content: self.accumulated_text.clone(),
                    signature: None,
                }),
                stop_reason: StopReason::Error,
                provider: ModelProviders::OPENAIRESPONSES,
                error_detail: Some("No response received".into()),
                signature: None,
                metadata: None,
            };
        };

        #[allow(clippy::cast_precision_loss)]
        let usage = response
            .usage
            .as_ref()
            .map_or_else(empty_usage_report, |u| UsageReport {
                input: u.input_tokens as f64,
                output: u.output_tokens as f64,
                cache_read: u.reasoning_tokens.map_or(0.0, |r| r as f64),
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
            });

        let stop_reason = match response.status.as_str() {
            "completed" | "in_progress" => StopReason::Stop,
            "failed" => StopReason::Error,
            other => StopReason::Message(other.to_string()),
        };

        let content = extract_output(&response.output);
        let metadata = build_response_metadata(&response.output);

        Messages::Assistant {
            model: self.model_id.clone(),
            timestamp: SystemTime::now(),
            usage,
            content,
            stop_reason,
            provider: ModelProviders::OPENAIRESPONSES,
            error_detail: response.error.as_ref().map(|e| e.message.clone()),
            signature: None,
            metadata,
        }
    }
}

// ============================================================================
// Helpers
// ============================================================================

fn build_response_input(interaction: &ModelInteraction) -> ResponseInput {
    let items: Vec<ResponseInputItem> = interaction
        .messages
        .iter()
        .filter_map(|msg| match msg {
            Messages::User { content, .. } => match content {
                crate::types::UserModelContent::Text(tc) => Some(ResponseInputItem::Message {
                    role: String::from("user"),
                    content: ResponseInputContent::Text(tc.content.clone()),
                }),
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
                    Some(ResponseInputItem::Message {
                        role: String::from("user"),
                        content: ResponseInputContent::Parts(vec![
                            ResponseInputContentPart::InputImage {
                                image_url: data_url,
                            },
                        ]),
                    })
                }
            },
            Messages::Assistant { content, .. } => match content {
                ModelOutput::Text(tc) => Some(ResponseInputItem::Message {
                    role: String::from("assistant"),
                    content: ResponseInputContent::Text(tc.content.clone()),
                }),
                ModelOutput::ThinkingContent { thinking, .. } => Some(ResponseInputItem::Message {
                    role: String::from("assistant"),
                    content: ResponseInputContent::Text(thinking.clone()),
                }),
                ModelOutput::ToolCall {
                    name, arguments, ..
                } => Some(ResponseInputItem::FunctionCallOutput {
                    call_id: name.clone(),
                    output: arguments
                        .as_ref()
                        .map(|a| serde_json::to_string(a).unwrap_or_default())
                        .unwrap_or_default(),
                }),
                _ => None,
            },
            Messages::ToolResult {
                id, name, content, ..
            } => {
                let text = match content {
                    crate::types::UserModelContent::Text(tc) => tc.content.clone(),
                    crate::types::UserModelContent::Image(_) => String::from("[Image]"),
                };
                Some(ResponseInputItem::FunctionCallOutput {
                    call_id: id.clone(),
                    output: format!("[{name}] {text}"),
                })
            }
        })
        .collect();

    if items.is_empty() {
        ResponseInput::Text(String::new())
    } else {
        ResponseInput::Items(items)
    }
}

fn extract_output(output: &[ResponseOutputItem]) -> ModelOutput {
    for item in output {
        match item {
            ResponseOutputItem::Message { content, .. } => {
                let text: String = content
                    .iter()
                    .map(|c| {
                        let ResponseOutputContent::OutputText { text } = c;
                        text.clone()
                    })
                    .collect::<Vec<_>>()
                    .join("\n");
                if !text.is_empty() {
                    return ModelOutput::Text(TextContent {
                        content: text,
                        signature: None,
                    });
                }
            }
            ResponseOutputItem::FunctionCall {
                id,
                name,
                arguments,
                ..
            } => {
                let args: Option<HashMap<String, crate::types::ArgType>> =
                    serde_json::from_str(arguments)
                        .ok()
                        .map(|v: serde_json::Value| {
                            v.as_object()
                                .map(|obj| {
                                    obj.iter()
                                        .map(|(k, v)| {
                                            (k.clone(), json_value_to_arg_type(v))
                                        })
                                        .collect()
                                })
                                .unwrap_or_default()
                        });
                return ModelOutput::ToolCall {
                    id: id.clone(),
                    name: name.clone(),
                    arguments: args,
                    signature: None,
                };
            }
            ResponseOutputItem::Reasoning { content, .. } => {
                return ModelOutput::ThinkingContent {
                    thinking: content.clone(),
                    signature: None,
                };
            }
        }
    }
    ModelOutput::Text(TextContent {
        content: String::new(),
        signature: None,
    })
}

fn build_response_metadata(
    output: &[ResponseOutputItem],
) -> Option<Vec<GenerationMetadata>> {
    let has_reasoning = output
        .iter()
        .any(|item| matches!(item, ResponseOutputItem::Reasoning { .. }));

    if has_reasoning {
        Some(vec![GenerationMetadata::Timing {
            total_ms: 0,
            time_to_first_ms: None,
            tokens_per_sec: None,
        }])
    } else {
        None
    }
}

fn parse_response(response: &Response, model_id: &ModelId) -> Messages {
    #[allow(clippy::cast_precision_loss)]
    let usage = response
        .usage
        .as_ref()
        .map_or_else(empty_usage_report, |u| UsageReport {
            input: u.input_tokens as f64,
            output: u.output_tokens as f64,
            cache_read: u.reasoning_tokens.map_or(0.0, |r| r as f64),
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
        });

    let stop_reason = match response.status.as_str() {
        "completed" | "in_progress" => StopReason::Stop,
        "failed" => StopReason::Error,
        other => StopReason::Message(other.to_string()),
    };

    let content = extract_output(&response.output);
    let metadata = build_response_metadata(&response.output);

    Messages::Assistant {
        model: model_id.clone(),
        timestamp: SystemTime::now(),
        usage,
        content,
        stop_reason,
        provider: ModelProviders::OPENAIRESPONSES,
        error_detail: response.error.as_ref().map(|e| e.message.clone()),
        signature: None,
        metadata,
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

// ============================================================================
// Helpers
// ============================================================================

/// Flatten a `ToolShed` into a Vec<Tool> for formatting.
#[must_use]
pub fn flatten_tools(shed: &ToolShed) -> Vec<crate::types::Tool> {
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

fn convert_tool_choice(choice: &crate::types::ToolChoice) -> ResponseToolChoice {
    match choice {
        crate::types::ToolChoice::Auto => ResponseToolChoice::Simple(String::from("auto")),
        crate::types::ToolChoice::None => ResponseToolChoice::Simple(String::from("none")),
        crate::types::ToolChoice::Required => ResponseToolChoice::Simple(String::from("required")),
        crate::types::ToolChoice::Function(f) => ResponseToolChoice::Function {
            r#type: String::from("function"),
            function: ResponseToolChoiceFunction {
                name: f.function.name.clone(),
            },
        },
    }
}

// ============================================================================
// Tests
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_response_request_serialization() {
        let request = ResponseRequest {
            model: "o1".into(),
            input: ResponseInput::Text("Hello".into()),
            instructions: Some("Be helpful".into()),
            tools: None,
            tool_choice: None,
            max_output_tokens: Some(100),
            temperature: Some(1.0),
            top_p: None,
            stream: Some(false),
            truncate: None,
            previous_response_id: None,
        };

        let json = serde_json::to_string(&request).unwrap();
        assert!(json.contains(r#""model":"o1""#));
        assert!(json.contains(r#""input":"Hello""#));
        assert!(json.contains(r#""instructions":"Be helpful""#));
        assert!(json.contains(r#""max_output_tokens":100"#));
        assert!(json.contains(r#""stream":false"#));
    }

    #[test]
    fn test_response_input_items() {
        let input = ResponseInput::Items(vec![ResponseInputItem::Message {
            role: "user".into(),
            content: ResponseInputContent::Text("Hello".into()),
        }]);

        let json = serde_json::to_string(&input).unwrap();
        assert!(json.contains(r#""type":"message""#));
        assert!(json.contains(r#""role":"user""#));
    }

    #[test]
    fn test_response_deserialization() {
        let json = r#"{
            "id": "resp_abc123",
            "object": "response",
            "created_at": 1234567890,
            "model": "o1",
            "output": [{
                "type": "message",
                "id": "msg_001",
                "status": "completed",
                "role": "assistant",
                "content": [{"type": "output_text", "text": "Hello!"}]
            }],
            "status": "completed",
            "usage": {"input_tokens": 10, "output_tokens": 5, "total_tokens": 15}
        }"#;

        let response: Response = serde_json::from_str(json).unwrap();
        assert_eq!(response.id, "resp_abc123");
        assert_eq!(response.status, "completed");
        assert_eq!(response.output.len(), 1);
        assert_eq!(response.usage.as_ref().unwrap().total_tokens, 15);
    }

    #[test]
    fn test_response_output_item_reasoning() {
        let json = r#"{
            "type": "reasoning",
            "id": "rs_001",
            "content": "Let me think about this..."
        }"#;

        let item: ResponseOutputItem = serde_json::from_str(json).unwrap();
        match item {
            ResponseOutputItem::Reasoning { content, .. } => {
                assert_eq!(content, "Let me think about this...");
            }
            _ => panic!("Expected Reasoning variant"),
        }
    }

    #[test]
    fn test_response_output_item_function_call() {
        let json = r#"{
            "type": "function_call",
            "id": "fc_001",
            "call_id": "call_123",
            "name": "get_weather",
            "arguments": "{\"location\": \"Paris\"}",
            "status": "completed"
        }"#;

        let item: ResponseOutputItem = serde_json::from_str(json).unwrap();
        match item {
            ResponseOutputItem::FunctionCall {
                name, arguments, ..
            } => {
                assert_eq!(name, "get_weather");
                assert!(arguments.contains("Paris"));
            }
            _ => panic!("Expected FunctionCall variant"),
        }
    }

    #[test]
    fn test_response_event_deserialization() {
        let json = r#"{
            "type": "response.output_text.delta",
            "item_id": "msg_001",
            "delta": "Hello"
        }"#;

        let event: ResponseEvent = serde_json::from_str(json).unwrap();
        match event {
            ResponseEvent::ResponseOutputTextDelta { delta, .. } => {
                assert_eq!(delta, "Hello");
            }
            _ => panic!("Expected ResponseOutputTextDelta variant"),
        }
    }

    #[test]
    fn test_response_completed_event() {
        let json = r#"{
            "type": "response.completed",
            "response": {
                "id": "resp_abc",
                "object": "response",
                "created_at": 0,
                "model": "o1",
                "output": [],
                "status": "completed"
            }
        }"#;

        let event: ResponseEvent = serde_json::from_str(json).unwrap();
        match event {
            ResponseEvent::ResponseCompleted { response } => {
                assert_eq!(response.id, "resp_abc");
            }
            _ => panic!("Expected ResponseCompleted variant"),
        }
    }

    #[test]
    fn test_build_response_input_from_messages() {
        let interaction = ModelInteraction {
            system_prompt: Some("Be helpful".into()),
            soul: None,
            messages: vec![Messages::User {
                role: "user".into(),
                content: crate::types::UserModelContent::Text(TextContent {
                    content: "Hello".into(),
                    signature: None,
                }),
                signature: None,
            }],
            tools_shed: None,
            chat_template: None,
            tool_choice: None,
        };

        let input = build_response_input(&interaction);
        match input {
            ResponseInput::Items(items) => {
                assert_eq!(items.len(), 1);
                match &items[0] {
                    ResponseInputItem::Message { role, content } => {
                        assert_eq!(role, "user");
                        assert!(matches!(content, ResponseInputContent::Text(_)));
                    }
                    _ => panic!("Expected Message item"),
                }
            }
            _ => panic!("Expected Items input"),
        }
    }

    #[test]
    fn test_configs_defaults() {
        let config = ResponsesConfig::default();
        assert_eq!(config.base_url, "https://api.openai.com");
        assert_eq!(config.api_version, "v1");
        assert_eq!(config.timeout_secs, 120);
        assert!(config.streaming);
    }
}
