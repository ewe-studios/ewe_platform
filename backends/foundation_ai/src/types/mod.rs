//! Core definition for what models entail

use std::collections::HashMap;
use std::path::PathBuf;
use std::time::SystemTime;

use derive_more::{Display, Error, From};
use foundation_auth::AuthCredential;
use foundation_core::extensions::strings_ext::IntoString;
use foundation_core::valtron::StreamIterator;
use foundation_core::wire::simple_http::url::Uri;
use foundation_errstacks::ErrorTrace;
use lazy_regex::regex;
use regex::Regex;
use serde::{Deserialize, Serialize};

use crate::errors::GenerationResult;
use crate::errors::ModelProviderResult;

#[derive(From, Serialize, Deserialize, Debug, Copy, Clone, PartialEq, PartialOrd)]
pub struct DeviceId(u16);

impl DeviceId {
    /// Create a new `DeviceId` from a raw u16 value.
    #[must_use]
    pub fn new(id: u16) -> Self {
        DeviceId(id)
    }

    /// Retrieve the underlying device id value.
    #[must_use]
    pub fn get_id(&self) -> u16 {
        self.0
    }
}

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, PartialOrd, Default)]
pub enum CacheRetention {
    #[default]
    None,
    Short,
    Long,
    Custom(String),
}

impl From<String> for CacheRetention {
    fn from(value: String) -> Self {
        match value.as_str() {
            "none" => Self::None,
            "short" => Self::Short,
            "long" => Self::Long,
            _ => Self::Custom(value),
        }
    }
}

impl From<&'static str> for CacheRetention {
    fn from(value: &'static str) -> Self {
        match value {
            "none" => Self::None,
            "short" => Self::Short,
            "long" => Self::Long,
            _ => Self::Custom(value.to_string()),
        }
    }
}

#[derive(From, Serialize, Deserialize, Debug, Copy, Clone, PartialEq, PartialOrd)]
pub struct ThinkingBudget {
    pub minimal: f64,
    pub medium: f64,
    pub low: f64,
    pub high: f64,
}

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, PartialOrd, Default)]
pub enum ThinkingLevels {
    Minimal,
    Low,
    #[default]
    Medium,
    High,
    Custom(String),
}

impl From<String> for ThinkingLevels {
    fn from(value: String) -> Self {
        match value.as_str() {
            "low" => Self::Low,
            "high" => Self::High,
            "medium" => Self::Medium,
            "minimal" => Self::Minimal,
            _ => Self::Custom(value),
        }
    }
}

impl From<&'static str> for ThinkingLevels {
    fn from(value: &'static str) -> Self {
        match value {
            "low" => Self::Low,
            "high" => Self::High,
            "medium" => Self::Medium,
            "minimal" => Self::Minimal,
            _ => Self::Custom(value.to_string()),
        }
    }
}

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, PartialOrd)]
pub enum ModelProviders {
    AMAZONBEDROCK,
    ANTHROPIC,
    GOOGLE,
    GOOGLEGEMINICLI,
    GOOGLEANTIGRAVITY,
    GOOGLEVERTEX,
    OPENAI,
    OPENAIRESPONSES,
    AZUREOPENAIRESPONSES,
    OPENAICODEX,
    GITHUBCOPILOT,
    XAI,
    GROQ,
    CEREBRAS,
    OPENROUTER,
    VERCELAIGATEWAY,
    ZAI,
    MISTRAL,
    MINIMAX,
    MINIMAXCN,
    HUGGINGFACE,
    OPENCODE,
    KIMICODING,
    LLAMACPP,
    Custom(String),
}

impl From<&'static str> for ModelProviders {
    fn from(value: &'static str) -> Self {
        match value {
            "amazon-bedrock" => Self::AMAZONBEDROCK,
            "anthropic" => Self::ANTHROPIC,
            "google" => Self::GOOGLE,
            "google-gemini-cli" => Self::GOOGLEGEMINICLI,
            "google-antigravity" => Self::GOOGLEANTIGRAVITY,
            "google-vertex" => Self::GOOGLEVERTEX,
            "openai" => Self::OPENAI,
            "azure-openai-responses" => Self::AZUREOPENAIRESPONSES,
            "openai-codex" => Self::OPENAICODEX,
            "github-copilot" => Self::GITHUBCOPILOT,
            "xai" => Self::XAI,
            "groq" => Self::GROQ,
            "cerebras" => Self::CEREBRAS,
            "openrouter" => Self::OPENROUTER,
            "vercel-ai-gateway" => Self::VERCELAIGATEWAY,
            "zai" => Self::ZAI,
            "mistral" => Self::MISTRAL,
            "minimax" => Self::MINIMAX,
            "minimax-cn" => Self::MINIMAXCN,
            "huggingface" => Self::HUGGINGFACE,
            "opencode" => Self::OPENCODE,
            "kimi-coding" => Self::KIMICODING,
            _ => Self::Custom(value.to_string()),
        }
    }
}

impl From<String> for ModelProviders {
    fn from(value: String) -> Self {
        match value.as_str() {
            "amazon-bedrock" => Self::AMAZONBEDROCK,
            "anthropic" => Self::ANTHROPIC,
            "google" => Self::GOOGLE,
            "google-gemini-cli" => Self::GOOGLEGEMINICLI,
            "google-antigravity" => Self::GOOGLEANTIGRAVITY,
            "google-vertex" => Self::GOOGLEVERTEX,
            "openai" => Self::OPENAI,
            "azure-openai-responses" => Self::AZUREOPENAIRESPONSES,
            "openai-codex" => Self::OPENAICODEX,
            "github-copilot" => Self::GITHUBCOPILOT,
            "xai" => Self::XAI,
            "groq" => Self::GROQ,
            "cerebras" => Self::CEREBRAS,
            "openrouter" => Self::OPENROUTER,
            "vercel-ai-gateway" => Self::VERCELAIGATEWAY,
            "zai" => Self::ZAI,
            "mistral" => Self::MISTRAL,
            "minimax" => Self::MINIMAX,
            "minimax-cn" => Self::MINIMAXCN,
            "huggingface" => Self::HUGGINGFACE,
            "opencode" => Self::OPENCODE,
            "kimi-coding" => Self::KIMICODING,
            _ => Self::Custom(value),
        }
    }
}

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, PartialOrd)]
pub enum ModelAPI {
    OpenAICompletions,
    OpenAIResponses,
    AzureOpenaiResponses,
    OpenaiCodexResponses,
    AnthropicMessages,
    BedrockConverseStream,
    GoogleGenerativeAi,
    GoogleGeminiCli,
    GoogleVertex,
    Custom(String),
}

impl From<String> for ModelAPI {
    fn from(value: String) -> Self {
        match value.as_str() {
            "openai-completions" => Self::OpenAICompletions,
            "openai-responses" => Self::OpenAIResponses,
            "azure-openai-responses" => Self::AzureOpenaiResponses,
            "openai-codex-responses" => Self::OpenaiCodexResponses,
            "anthropic-messages" => Self::AnthropicMessages,
            "bedrock-converse-stream" => Self::BedrockConverseStream,
            "google-generative-ai" => Self::GoogleGenerativeAi,
            "google-gemini-cli" => Self::GoogleGeminiCli,
            "google-vertex" => Self::GoogleVertex,
            _ => Self::Custom(value),
        }
    }
}

impl From<&'static str> for ModelAPI {
    fn from(value: &'static str) -> Self {
        match value {
            "openai-completions" => Self::OpenAICompletions,
            "openai-responses" => Self::OpenAIResponses,
            "azure-openai-responses" => Self::AzureOpenaiResponses,
            "openai-codex-responses" => Self::OpenaiCodexResponses,
            "anthropic-messages" => Self::AnthropicMessages,
            "bedrock-converse-stream" => Self::BedrockConverseStream,
            "google-generative-ai" => Self::GoogleGenerativeAi,
            "google-gemini-cli" => Self::GoogleGeminiCli,
            "google-vertex" => Self::GoogleVertex,
            _ => Self::Custom(value.into_string()),
        }
    }
}

#[derive(Serialize, Deserialize, Debug, Copy, Clone, PartialEq, PartialOrd)]
pub enum MessageType {
    Text,
    TextAndImages,
}

#[derive(Serialize, Debug, Clone, PartialEq, PartialOrd)]
pub struct ModelProviderDescriptor {
    pub id: &'static str,
    pub name: &'static str,
    pub reasoning: bool,
    pub api: ModelAPI,
    pub provider: ModelProviders,
    pub base_url: Option<&'static str>,
    pub inputs: MessageType,
    pub cost: ModelUsageCosting,
    pub context_window: u32,
    pub max_tokens: u32,
}

#[allow(non_camel_case_types)]
#[derive(From, Serialize, Deserialize, Debug, Clone, PartialEq, PartialOrd)]
pub enum Quantization {
    None,
    Default,
    F16,
    Q2K,
    Q2_KS,
    Q2_KM,
    Q2_KL,
    Q3_KS,
    Q3_KM,
    Q4_0,
    Q4_1,
    IQ_4Nl,
    IQ_4Xs,
    Q4_KM,
    Q4_KS,
    Q5_KS,
    Q5_KM,
    Q5_KL,
    Q6_K,
    Q6_KM,
    Q6_KS,
    Q6_KL,
    Q8_0,
    Q8_1,
    Ud_IQ_1M,
    UD_IQ_1S,
    UD_IQ_2M,
    UD_IQ_2Xxs,
    UD_IQ_3Xxs,
    UD_Q_2KXl,
    UD_Q_3KXl,
    UD_Q_4KXl,
    UD_Q_5KXl,
    UD_Q_6KXl,
    UD_Q_8KXl,
    Custom(String),
}

impl Quantization {
    /// Convert to GGUF filename format (the exact string used in GGUF filenames).
    ///
    /// # Examples
    ///
    /// ```
    /// # use foundation_ai::types::Quantization;
    /// assert_eq!(Quantization::Q2K.to_filename_format(), "Q2_K");
    /// assert_eq!(Quantization::Q4_KM.to_filename_format(), "Q4_K_M");
    /// assert_eq!(Quantization::F16.to_filename_format(), "F16");
    /// ```
    #[must_use]
    pub fn to_filename_format(&self) -> String {
        match self {
            Quantization::None | Quantization::Default => String::new(),
            Quantization::F16 => "F16".to_string(),
            Quantization::Q2K => "Q2_K".to_string(),
            Quantization::Q2_KS => "Q2_KS".to_string(),
            Quantization::Q2_KM => "Q2_KM".to_string(),
            Quantization::Q2_KL => "Q2_KL".to_string(),
            Quantization::Q3_KS => "Q3_KS".to_string(),
            Quantization::Q3_KM => "Q3_KM".to_string(),
            Quantization::Q4_0 => "Q4_0".to_string(),
            Quantization::Q4_1 => "Q4_1".to_string(),
            Quantization::IQ_4Nl => "IQ4_NL".to_string(),
            Quantization::IQ_4Xs => "IQ4_XS".to_string(),
            Quantization::Q4_KM => "Q4_K_M".to_string(),
            Quantization::Q4_KS => "Q4_KS".to_string(),
            Quantization::Q5_KS => "Q5_KS".to_string(),
            Quantization::Q5_KM => "Q5_K_M".to_string(),
            Quantization::Q5_KL => "Q5_KL".to_string(),
            Quantization::Q6_K => "Q6_K".to_string(),
            Quantization::Q6_KM => "Q6_K_M".to_string(),
            Quantization::Q6_KS => "Q6_KS".to_string(),
            Quantization::Q6_KL => "Q6_KL".to_string(),
            Quantization::Q8_0 => "Q8_0".to_string(),
            Quantization::Q8_1 => "Q8_1".to_string(),
            Quantization::Ud_IQ_1M => "IQ1_M".to_string(),
            Quantization::UD_IQ_1S => "IQ1_S".to_string(),
            Quantization::UD_IQ_2M => "IQ2_M".to_string(),
            Quantization::UD_IQ_2Xxs => "IQ2_XXS".to_string(),
            Quantization::UD_IQ_3Xxs => "IQ3_XXS".to_string(),
            Quantization::UD_Q_2KXl => "Q2_K_XL".to_string(),
            Quantization::UD_Q_3KXl => "Q3_K_XL".to_string(),
            Quantization::UD_Q_4KXl => "Q4_K_XL".to_string(),
            Quantization::UD_Q_5KXl => "Q5_K_XL".to_string(),
            Quantization::UD_Q_6KXl => "Q6_K_XL".to_string(),
            Quantization::UD_Q_8KXl => "Q8_K_XL".to_string(),
            Quantization::Custom(s) => s.clone(),
        }
    }
}

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, PartialOrd)]
pub enum ModelId {
    /// Specifically named model.
    Name(String, Option<Quantization>),

    /// A model with a specific alias generally not the full name
    /// and optional quantization.
    Alias(String, Option<Quantization>),

    /// A model based on its group and targeting a specific quantization.
    Group(String, Option<Quantization>),

    /// A model based on its architecture and targeting a specific quantization.
    Architecture(String, Option<Quantization>),
}

/// [`CallSpec`] defines the calling configuration for the model
/// which can be customized as needed for different use-case.
#[derive(From, Serialize, Deserialize, Debug, Clone, PartialEq)]
pub struct ModelParams {
    pub max_tokens: usize,
    pub temperature: f32,
    pub top_p: f32,
    pub top_k: f32,
    pub repeat_penalty: f32,
    pub seed: Option<u32>,
    pub stop_tokens: Vec<String>,
    pub thinking_level: ThinkingLevels,
    pub cache_retention: CacheRetention,
    pub thinking_budget: Option<ThinkingBudget>,
    /// Constrain the model's output format (text, JSON, schema).
    pub output_format: Option<OutputFormat>,
    /// Penalize new tokens based on their frequency in the text so far
    /// (-2.0 to 2.0, provider-specific; `OpenAI` only).
    pub frequency_penalty: Option<f32>,
    /// Penalize new tokens based on whether they appear in the text so far
    /// (-2.0 to 2.0, provider-specific; `OpenAI` only).
    pub presence_penalty: Option<f32>,
    /// Modify likelihood of specified tokens (-2.0 to 2.0, provider-specific).
    pub logit_bias: Option<HashMap<String, f32>>,
}

impl Default for ModelParams {
    fn default() -> Self {
        Self {
            max_tokens: 2048,
            temperature: 0.7,
            top_p: 0.9,
            top_k: 40.0,
            repeat_penalty: 1.1,
            seed: None,
            stop_tokens: Vec::new(),
            thinking_level: ThinkingLevels::default(),
            cache_retention: CacheRetention::default(),
            thinking_budget: None,
            output_format: None,
            frequency_penalty: None,
            presence_penalty: None,
            logit_bias: None,
        }
    }
}

#[derive(From, Serialize, Deserialize, Debug, Clone, PartialEq)]
pub struct ModelConfig {
    // standard model properties
    pub context_length: usize,
    pub max_threads: usize,
    pub template: Option<String>,

    /// The [`ResponseConfig`] defines the properties controlling
    /// how we want this to overall output
    /// temperature, `top_k``top_p`, etc
    pub params: ModelParams,
    pub streaming: bool,
}

pub enum ModelSource {
    /// Http endpoint which contains the target model file.
    HTTP(Uri),

    /// Model repository name  where the model is located in hugging face.
    HuggingFace(String),

    /// [`LocalFile`] points to a local source file where the model is located.
    LocalFile(PathBuf),

    /// [`LocalDirectory`] points to a local source directory where the model is located.
    LocalDirectory(PathBuf),
}

#[derive(From, Serialize, Deserialize, Debug, Copy, Clone, PartialEq, PartialOrd, Default)]
pub struct ModelUsageCosting {
    pub input: f64,
    pub output: f64,
    pub cache_read: f64,
    pub cache_write: f64,
}

#[derive(From, Serialize, Deserialize, Debug, Clone, PartialEq, PartialOrd)]
pub enum MimeType {
    TextPlain,
    TextHtml,
    TextMarkdown,
    TextXml,
    TextCss,
    ApplicationJson,
    ApplicationXml,
    ApplicationOctetStream,
    ApplicationPdf,
    ImagePng,
    ImageJpeg,
    ImageGif,
    ImageWebp,
    ImageSvgXml,
    ImageBmp,
    AudioMp3,
    AudioWav,
    AudioOgg,
    AudioMpeg,
    VideoMp4,
    VideoWebm,
    VideoOgg,

    #[from(ignore)]
    Custom(String),
}

impl From<&'static str> for MimeType {
    fn from(value: &'static str) -> Self {
        match value {
            "text/plain" => Self::TextPlain,
            "text/html" => Self::TextHtml,
            "text/markdown" => Self::TextMarkdown,
            "text/xml" => Self::TextXml,
            "text/css" => Self::TextCss,
            "application/json" => Self::ApplicationJson,
            "application/xml" => Self::ApplicationXml,
            "application/octet-stream" => Self::ApplicationOctetStream,
            "application/pdf" => Self::ApplicationPdf,
            "image/png" => Self::ImagePng,
            "image/jpeg" => Self::ImageJpeg,
            "image/gif" => Self::ImageGif,
            "image/webp" => Self::ImageWebp,
            "image/svg+xml" => Self::ImageSvgXml,
            "image/bmp" => Self::ImageBmp,
            "audio/mp3" => Self::AudioMp3,
            "audio/wav" => Self::AudioWav,
            "audio/ogg" => Self::AudioOgg,
            "audio/mpeg" => Self::AudioMpeg,
            "video/mp4" => Self::VideoMp4,
            "video/webm" => Self::VideoWebm,
            "video/ogg" => Self::VideoOgg,
            _ => Self::Custom(value.to_string()),
        }
    }
}

impl From<String> for MimeType {
    fn from(value: String) -> Self {
        match value.as_str() {
            "text/plain" => Self::TextPlain,
            "text/html" => Self::TextHtml,
            "text/markdown" => Self::TextMarkdown,
            "text/xml" => Self::TextXml,
            "text/css" => Self::TextCss,
            "application/json" => Self::ApplicationJson,
            "application/xml" => Self::ApplicationXml,
            "application/octet-stream" => Self::ApplicationOctetStream,
            "application/pdf" => Self::ApplicationPdf,
            "image/png" => Self::ImagePng,
            "image/jpeg" => Self::ImageJpeg,
            "image/gif" => Self::ImageGif,
            "image/webp" => Self::ImageWebp,
            "image/svg+xml" => Self::ImageSvgXml,
            "image/bmp" => Self::ImageBmp,
            "audio/mp3" => Self::AudioMp3,
            "audio/wav" => Self::AudioWav,
            "audio/ogg" => Self::AudioOgg,
            "audio/mpeg" => Self::AudioMpeg,
            "video/mp4" => Self::VideoMp4,
            "video/webm" => Self::VideoWebm,
            "video/ogg" => Self::VideoOgg,
            _ => Self::Custom(value),
        }
    }
}

#[derive(From, Serialize, Deserialize, Debug, Clone, PartialEq)]
pub enum StopReason {
    Stop,
    Length,
    ToolUse,
    Error,
    Aborted,
    /// Custom or unknown finish reason — preserves provider-specific values.
    #[from(ignore)]
    Message(String),
}

#[allow(clippy::match_same_arms)]
impl From<String> for StopReason {
    fn from(value: String) -> Self {
        match value.to_lowercase().as_str() {
            "length" => Self::Length,
            "tooluse" | "tool_calls" => Self::ToolUse,
            "error" => Self::Error,
            "aborted" => Self::Aborted,
            "stop" => Self::Stop,
            _ => Self::Message(value),
        }
    }
}

#[allow(clippy::match_same_arms)]
impl From<&'static str> for StopReason {
    fn from(value: &'static str) -> Self {
        match value.to_lowercase().as_str() {
            "stop" => Self::Stop,
            "length" => Self::Length,
            "tooluse" | "tool_calls" => Self::ToolUse,
            "error" => Self::Error,
            "aborted" => Self::Aborted,
            _ => Self::Message(value.to_string()),
        }
    }
}

#[derive(From, Serialize, Deserialize, Debug, Clone, PartialEq)]
pub enum ArgType {
    Text(String),
    Float32(f32),
    Float64(f64),
    Usize(usize),
    U8(u8),
    U16(u16),
    U32(u32),
    U64(u64),
    U128(u128),
    Isize(isize),
    I8(i8),
    I16(i16),
    I32(i32),
    I64(i64),
    I128(i128),
    Duration(std::time::Duration),

    // Custom types
    #[from(ignore)]
    JSON(String),
    /// Nested structured object with named sub-fields.
    #[from(ignore)]
    JSONMap(std::collections::HashMap<String, ArgType>),
}

/// A tool argument/return definition — stores a JSON Schema document and a
/// pre-built `ValidationOptions` so that callers can extract the schema for
/// API requests and compile a `Validator` for runtime validation.
///
/// WHY: Previously `Args` was an externally-tagged enum mapping `ArgType` →
/// `"type"` per provider. This loses expressiveness (min, max, format, etc.)
/// and can't produce real validators. Now `Args` holds the full JSON Schema
/// and its compiled `ValidationOptions` together.
///
/// HOW: Build with the `scheme` builder from `foundation_jsonschema`, then
/// store the result:
/// ```ignore
/// let opts = scheme::object()
///     .required("query", scheme::string().min_len(1))
///     .build(); // → ValidationOptions
/// let args = Args::new(opts);
/// ```
pub struct Args {
    /// The JSON Schema document for this argument/return.
    pub schema: serde_json::Value,
    /// Pre-built `ValidationOptions` with schema embedded.
    /// Call `.compile()` to get a `Validator`.
    pub validator: foundation_jsonschema::ValidationOptions,
}

impl std::fmt::Debug for Args {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Args")
            .field("schema", &self.schema)
            .finish_non_exhaustive()
    }
}

impl Args {
    /// Create an `Args` from a `ValidationOptions` (produced by a scheme builder).
    ///
    /// The schema is extracted from the `ValidationOptions` via `.clone_schema()`.
    #[must_use]
    pub fn new(opts: foundation_jsonschema::ValidationOptions) -> Self {
        let schema = opts.clone_schema();
        Self {
            schema,
            validator: opts,
        }
    }

    /// Create an `Args` from a raw JSON Schema value.
    ///
    /// Wraps the value in a fresh `ValidationOptions` with the schema embedded.
    #[must_use]
    pub fn from_value(value: serde_json::Value) -> Self {
        Self {
            validator: foundation_jsonschema::ValidationOptions::with_schema(value.clone()),
            schema: value,
        }
    }
}

impl Clone for Args {
    fn clone(&self) -> Self {
        Self {
            schema: self.schema.clone(),
            validator: foundation_jsonschema::ValidationOptions::with_schema(self.schema.clone()),
        }
    }
}

impl PartialEq for Args {
    fn eq(&self, other: &Self) -> bool {
        self.schema == other.schema
    }
}

impl Serialize for Args {
    fn serialize<S: serde::Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        self.schema.serialize(serializer)
    }
}

impl<'de> Deserialize<'de> for Args {
    fn deserialize<D: serde::Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        let value = serde_json::Value::deserialize(deserializer)?;
        Ok(Self::from_value(value))
    }
}

/// Status of a cost calculation.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum CostStatus {
    /// Calculated from pre-call token estimation.
    Estimated,
    /// Calculated from actual API response usage.
    Actual,
    /// Pricing unavailable for used tokens.
    Unknown,
}

/// [`UsageCosting`] represents the overall costing in actual currency value.
#[derive(From, Serialize, Deserialize, Debug, Clone, PartialEq)]
pub struct UsageCosting {
    pub currency: String,
    pub input: f64,
    pub output: f64,
    pub cache_read: f64,
    pub cache_write: f64,
    pub total_tokens: f64,
    /// Status of this cost calculation.
    pub status: CostStatus,
}

impl UsageCosting {
    /// Computed total USD cost.
    #[must_use]
    pub fn total(&self) -> f64 {
        self.input + self.output + self.cache_read + self.cache_write
    }

    /// Zeroed cost with the given status.
    #[must_use]
    pub fn zero(status: CostStatus) -> Self {
        Self {
            currency: String::from("USD"),
            input: 0.0,
            output: 0.0,
            cache_read: 0.0,
            cache_write: 0.0,
            total_tokens: 0.0,
            status,
        }
    }
}

/// [`UsageReport`] represents the accumulated usage at the point in time of
/// generation and the overall costing of that usage.
#[derive(From, Serialize, Deserialize, Debug, Clone, PartialEq)]
pub struct UsageReport {
    pub input: f64,
    pub output: f64,
    pub cache_read: f64,
    pub cache_write: f64,
    pub total_tokens: f64,
    pub cost: UsageCosting,
}

#[derive(From, Serialize, Deserialize, Debug, Clone, PartialEq)]
pub struct TextContent {
    pub content: String,
    pub signature: Option<String>,
}

#[derive(From, Serialize, Deserialize, Debug, Clone, PartialEq)]
pub struct ImageContent {
    pub b64: String,
    pub mime_type: MimeType,
}

#[derive(From, Serialize, Deserialize, Debug, Clone, PartialEq)]
pub enum UserModelContent {
    Text(TextContent),
    Image(ImageContent),
}

/// Constrain the model's output format.
///
/// Used by `ModelParams::output_format` to request structured output
/// from providers that support it (e.g., `OpenAI` `response_format`).
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Default)]
pub enum OutputFormat {
    /// Plain text output (default).
    #[default]
    Text,
    /// Force JSON output. Model responds with valid JSON.
    JsonObject,
    /// Schema-constrained JSON output.
    JsonSchema(JsonSchema),
}

/// JSON schema definition for `OutputFormat::JsonSchema`.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct JsonSchema {
    /// Name of the schema (for identification).
    pub name: String,
    /// Description of what the schema represents.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,
    /// The JSON schema definition.
    pub schema: serde_json::Value,
    /// Whether to enforce strict schema validation.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub strict: Option<bool>,
}

/// Per-token log probability and alternative tokens.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct ContentLogProb {
    pub token: String,
    pub logprob: f32,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub bytes: Option<Vec<u8>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub top_logprobs: Option<Vec<TopLogProbEntry>>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct TopLogProbEntry {
    pub token: String,
    pub logprob: f32,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub bytes: Option<Vec<u8>>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct RefusalLogProb {
    pub token: String,
    pub logprob: f32,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub bytes: Option<Vec<u8>>,
}

/// Provider-specific metadata attached to a generation result.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum GenerationMetadata {
    /// Log probabilities for each generated token (`OpenAI`).
    LogProbs {
        /// Per-token log probability and alternative tokens.
        content: Vec<ContentLogProb>,
        /// Log probs for refusal tokens, if any.
        #[serde(skip_serializing_if = "Option::is_none")]
        refusal: Option<Vec<RefusalLogProb>>,
    },
    /// System fingerprint for reproducibility (`OpenAI`).
    SystemFingerprint(String),
    /// Timing information for generation (local backends).
    Timing {
        /// Total generation time in milliseconds.
        total_ms: u64,
        /// Time to first token in milliseconds.
        time_to_first_ms: Option<u64>,
        /// Tokens per second.
        tokens_per_sec: Option<f64>,
    },
    /// Model refusal reason for safety/policy violations.
    RefusalReason(String),
}

#[derive(From, Serialize, Deserialize, Debug, Clone, PartialEq)]
pub enum ModelOutput {
    Text(TextContent),
    Image(ImageContent),
    ThinkingContent {
        thinking: String,
        signature: Option<String>,
    },
    ToolCall {
        id: String,
        name: String,
        arguments: Option<HashMap<String, ArgType>>,
        signature: Option<String>,
    },
    /// Embedding output for RAG pipelines and semantic search.
    /// Contains the embedding dimensions and the float values.
    Embedding {
        dimensions: usize,
        values: Vec<f32>,
    },
}

#[derive(From, Serialize, Deserialize, Debug, Clone, PartialEq)]
pub struct ToolParam {
    pub value: ArgType,
    pub name: String,
    pub description: String,
}

#[allow(clippy::large_enum_variant)]
#[derive(From, Serialize, Deserialize, Debug, Clone, PartialEq)]
pub enum Messages {
    User {
        role: String,
        content: UserModelContent,
        signature: Option<String>,
    },
    Assistant {
        model: ModelId,
        timestamp: SystemTime,
        usage: UsageReport,
        content: ModelOutput,
        stop_reason: StopReason,
        provider: ModelProviders,
        error_detail: Option<String>,
        signature: Option<String>,
        /// Provider-specific metadata (logprobs, system fingerprint, etc.).
        metadata: Option<Vec<GenerationMetadata>>,
    },
    ToolResult {
        id: String,
        name: String,
        timestamp: SystemTime,
        details: Option<String>,
        content: UserModelContent,
        error_detail: Option<String>,
        signature: Option<String>,
    },
}

/// Regex patterns to detect context overflow errors from different providers.
///
/// These patterns match error messages returned when the input exceeds
/// the model's context window.
///
/// Provider-specific patterns (with example error messages):
///
/// - Anthropic: "prompt is too long: 213462 tokens > 200000 maximum"
/// - `OpenAI`: "Your input exceeds the context window of this model"
/// - Google: "The input token count (1196265) exceeds the maximum number of tokens allowed (1048575)"
/// - xAI: "This model's maximum prompt length is 131072 but the request contains 537812 tokens"
/// - Groq: "Please reduce the length of the messages or completion"
/// - `OpenRouter`: "This endpoint's maximum context length is X tokens. However, you requested about Y tokens"
/// - `llama.cpp`: "the request exceeds the available context size, try increasing it"
/// - LM Studio: "tokens to keep from the initial prompt is greater than the context length"
/// - GitHub Copilot: "prompt token count of X exceeds the limit of Y"
/// - `MiniMax`: "invalid params, context window exceeds limit"
/// - Kimi For Coding: "Your request exceeded model token limit: X (requested: Y)"
/// - Cerebras: Returns "400/413 status code (no body)" - handled separately below
/// - Mistral: Returns "400/413 status code (no body)" - handled separately below
/// - z.ai: Does NOT error, accepts overflow silently - handled via usage.input > contextWindow
/// - Ollama: Silently truncates input - not detectable via error message
const OVERFLOW_PATTERNS: &[&str] = &[
    r"(?i)prompt is too long",                     // Anthropic
    r"(?i)input is too long for requested model",  // Amazon Bedrock
    r"(?i)exceeds the context window",             // OpenAI (Completions & Responses API)
    r"(?i)input token count.*exceeds the maximum", // Google (Gemini)
    r"(?i)maximum prompt length is \d+",           // xAI (Grok)
    r"(?i)reduce the length of the messages",      // Groq
    r"(?i)maximum context length is \d+ tokens",   // OpenRouter (all backends)
    r"(?i)exceeds the limit of \d+",               // GitHub Copilot
    r"(?i)exceeds the available context size",     // llama.cpp server
    r"(?i)greater than the context length",        // LM Studio
    r"(?i)context window exceeds limit",           // MiniMax
    r"(?i)exceeded model token limit",             // Kimi For Coding
    r"(?i)context[_ ]length[_ ]exceeded",          // Generic fallback
    r"(?i)too many tokens",                        // Generic fallback
    r"(?i)token limit exceeded",                   // Generic fallback
];

static OVERFLOW_SILENT_PATTERN: std::sync::LazyLock<Regex> = std::sync::LazyLock::new(|| {
    regex::Regex::new(r"(?i)^4(00|13)\s*(status code)?\s*\(no body\)").unwrap()
});

impl Messages {
    pub fn is_context_overflow(&self, context_window: u64) -> bool {
        match self {
            Messages::Assistant {
                stop_reason,
                error_detail,
                usage,
                ..
            } => {
                // Case 1: Check error message patterns
                if *stop_reason == StopReason::Error {
                    if let Some(error_msg) = error_detail {
                        // Check known patterns
                        for pattern in OVERFLOW_PATTERNS {
                            if let Ok(re) = regex::Regex::new(pattern) {
                                if re.is_match(error_msg) {
                                    return true;
                                }
                            }
                        }

                        // Cerebras and Mistral return 400/413 with no body for context overflow
                        // Note: 429 is rate limiting (requests/tokens per time), NOT context overflow
                        if OVERFLOW_SILENT_PATTERN.is_match(error_msg) {
                            return true;
                        }
                    }
                }

                // Case 2: Silent overflow (z.ai style) - successful but usage exceeds context
                if *stop_reason == StopReason::Stop {
                    #[allow(clippy::cast_possible_truncation, clippy::cast_sign_loss)]
                    let input_tokens = (usage.input + usage.cache_read).max(0.0) as u64;
                    if input_tokens > context_window {
                        return true;
                    }
                }

                false
            }
            _ => false,
        }
    }
}

#[derive(From, Serialize, Deserialize, Debug, Clone, PartialEq)]
pub struct Tool {
    pub id: String,
    pub name: String,
    pub description: String,
    pub arguments: Option<Args>,
    pub returns: Option<Args>,
}

/// Strategy for tool selection in model interactions.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(untagged)]
pub enum ToolChoice {
    /// Let the model decide whether to use tools.
    Auto,
    /// Force the model to not use any tools.
    None,
    /// Force the model to use at least one tool.
    Required,
    /// Force the model to use a specific function.
    Function(ToolChoiceFunction),
}

/// Reference to a specific function in a forced tool choice.
///
/// Note: Serde supports `#[serde(serialize_with, deserialize_with)]` to render
/// a plain `String` as `{"name": "..."}` and back, which would eliminate the
/// struct. However, that requires custom serialization functions (~10 lines)
/// that are more code and less readable than the struct itself.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct ToolChoiceFunction {
    #[serde(rename = "type")]
    pub tool_type: String,
    pub function: ToolFunctionRef,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct ToolFunctionRef {
    pub name: String,
}

#[derive(From, Serialize, Deserialize, Debug, Clone, PartialEq)]
pub struct MemoryTool {
    pub add: Tool,
    pub replace: Tool,
    pub remove: Tool,
}

#[derive(From, Serialize, Deserialize, Debug, Clone, PartialEq)]
pub struct DelegationTool {
    pub start: Tool,
    pub check: Tool,
    pub get: Tool,
}

#[derive(From, Serialize, Deserialize, Debug, Clone, PartialEq)]
pub struct ToolShed {
    pub shed: Tool,
    pub memory: Option<MemoryTool>,
    pub delegate: Option<DelegationTool>,
    pub read: Tool,
    pub edit: Tool,
    pub write: Tool,
    pub search: Tool,
    pub bash: Option<Tool>,
    pub others: Option<Vec<Tool>>,
}

#[derive(From, Serialize, Deserialize, Debug, Clone, PartialEq)]
pub struct ModelInteraction {
    pub system_prompt: Option<String>,
    pub soul: Option<String>,
    pub tools_shed: Option<ToolShed>,
    pub messages: Vec<Messages>,
    pub chat_template: Option<String>,
    /// Strategy for tool selection. When `None`, the provider's default
    /// behavior is used (typically auto).
    pub tool_choice: Option<ToolChoice>,
}

#[derive(From, Serialize, Deserialize, Debug, Clone, PartialEq)]
pub struct ModelSpec {
    pub name: String,

    /// The id representing the giving model.
    pub id: ModelId,

    /// The target device to use for this model execution.
    pub devices: Option<Vec<DeviceId>>,

    /// The optional path to the model file/directory according to the
    /// for which the backend will use.
    pub model_location: Option<PathBuf>,

    /// The optional path to the lora model files/directory for lora
    /// optimized inference with the main model file.
    pub lora_location: Option<PathBuf>,
}

#[derive(From, Serialize, Deserialize, Debug, Clone, PartialEq)]
pub enum ModelState {
    GeneratingEmbeddings,
    GeneratingTokens(Option<UsageReport>),
    Finished,
    Error(String),
}

/// Error type for tool calling formatting operations.
///
/// Enum-first design: each variant carries the fields relevant to that
/// failure mode. Used with `foundation_errstacks` for context-aware
/// error traces.
#[derive(Debug, Display, Error)]
pub enum ToolCallingError {
    /// Failed to parse a tool call from provider output.
    #[display("failed to extract tool calls: {reason}")]
    Extract { reason: String },
    /// Failed to format a tool definition for a provider.
    #[display("failed to format tool '{tool_name}': {reason}")]
    Format { tool_name: String, reason: String },
    /// Failed to format a tool result for round-trip.
    #[display("failed to format result for '{tool_name}': {reason}")]
    Response { tool_name: String, reason: String },
}

/// Stateless formatter for tool calling format conversion.
///
/// Each provider implements this to translate between our internal tool
/// representation and the format expected by its API.
pub trait ToolFormatter: Default + Send + Sync {
    /// Convert internal `Tool[]` definitions to provider-specific tool schema.
    ///
    /// # Errors
    ///
    /// Returns an error if the tool schema cannot be serialized to the
    /// provider's expected format.
    fn format_tools(
        &self,
        tools: &[Tool],
    ) -> Result<serde_json::Value, ErrorTrace<ToolCallingError>>;

    /// System prompt instructions for tool calling format.
    ///
    /// Returns `Some(instructions)` when the model needs guidance on how to
    /// format tool calls (text-based models, no tool-aware template).
    /// Returns `None` for API providers since their native tool format handles it.
    fn tool_calling_instructions(&self) -> Option<String>;

    /// Extract tool calls from provider response text.
    ///
    /// # Errors
    ///
    /// Returns an error if the response contains malformed tool call data.
    fn extract_tool_calls(
        &self,
        response: &str,
    ) -> Result<ExtractResult, ErrorTrace<ToolCallingError>>;

    /// Format a tool execution result into provider message structure.
    ///
    /// # Errors
    ///
    /// Returns an error if the tool result cannot be serialized to the
    /// provider's expected format.
    fn format_tool_response(
        &self,
        result: &Messages,
    ) -> Result<serde_json::Value, ErrorTrace<ToolCallingError>>;
}

/// Output of `extract_tool_calls()` — extracted tool calls plus remaining text.
#[derive(Debug)]
pub struct ExtractResult {
    /// Extracted tool calls in unified `ModelOutput::ToolCall` format.
    pub calls: Vec<ModelOutput>,
    /// Non-tool-call portions of the response.
    pub remaining_text: Option<String>,
    /// Whether the model intends to use tools.
    pub has_tool_calls: bool,
}

/// XML tag used by text-based models for tool calling.
const TOOL_CALL_OPEN: &str = "<ToolCall>";
const TOOL_CALL_CLOSE: &str = "</ToolCall>";

/// Convert a raw JSON value to `ArgType`.
/// Needed because `ArgType` is externally tagged and can't be deserialized
/// from plain JSON values.
fn json_value_to_arg_type(v: &serde_json::Value) -> ArgType {
    match v {
        serde_json::Value::String(s) => ArgType::Text(s.clone()),
        serde_json::Value::Number(n) => {
            if let Some(i) = n.as_i64() {
                ArgType::I64(i)
            } else if let Some(f) = n.as_f64() {
                ArgType::Float64(f)
            } else {
                ArgType::Text(v.to_string())
            }
        }
        serde_json::Value::Bool(b) => ArgType::Text(if *b { "true" } else { "false" }.to_string()),
        serde_json::Value::Null => ArgType::Text(String::new()),
        serde_json::Value::Array(arr) => ArgType::Text(
            arr.iter()
                .map(std::string::ToString::to_string)
                .collect::<Vec<_>>()
                .join(", "),
        ),
        serde_json::Value::Object(map) => {
            let nested: HashMap<String, ArgType> = map
                .iter()
                .map(|(k, v)| (k.clone(), json_value_to_arg_type(v)))
                .collect();
            ArgType::JSONMap(nested)
        }
    }
}

/// Shared formatter for text-based models (llama.cpp, Candle).
///
/// Produces system prompt instructions and parses `<ToolCall>...</ToolCall>`
/// XML-wrapped JSON from model output.
#[derive(Default, Clone, Copy)]
pub struct TextBasedFormatter;

impl ToolFormatter for TextBasedFormatter {
    fn format_tools(
        &self,
        tools: &[Tool],
    ) -> Result<serde_json::Value, ErrorTrace<ToolCallingError>> {
        Ok(serde_json::Value::Array(
            tools
                .iter()
                .map(|tool| {
                    // Use the Args schema if present, otherwise default to empty object
                    let params = tool
                        .arguments
                        .as_ref().map_or_else(|| {
                            serde_json::json!({
                                "type": "object",
                                "properties": {},
                            })
                        }, |a| a.schema.clone());
                    serde_json::json!({
                        "name": &tool.name,
                        "description": tool.description,
                        "parameters": params,
                    })
                })
                .collect(),
        ))
    }

    fn tool_calling_instructions(&self) -> Option<String> {
        Some(format!(
            "To use a tool, wrap your call in {TOOL_CALL_OPEN} tags with valid JSON inside:\n\
            {TOOL_CALL_OPEN}{{\"name\":\"tool_name\",\"arguments\":{{\"param\":\"value\"}}}}{TOOL_CALL_CLOSE}\n\n\
            Only output tool calls when necessary. Do not invent tool calls."
        ))
    }

    fn extract_tool_calls(
        &self,
        response: &str,
    ) -> Result<ExtractResult, ErrorTrace<ToolCallingError>> {
        let pattern = regex!(r"<ToolCall>\s*(.*?)\s*</ToolCall>");
        let mut calls = Vec::new();
        let mut remaining_parts = Vec::new();
        let mut last_end = 0;

        for cap in pattern.captures_iter(response) {
            let full = cap.get(0).unwrap();
            let json_str = cap.get(1).unwrap().as_str();

            // Collect text before this match as remaining
            if full.start() > last_end {
                remaining_parts.push(&response[last_end..full.start()]);
            }
            last_end = full.end();

            // Try to parse the JSON as a generic value, then convert to ArgType map
            let parsed: Result<serde_json::Value, _> = serde_json::from_str(json_str);
            match parsed {
                Ok(value) => {
                    if let Some(obj) = value.as_object() {
                        let arguments: HashMap<String, ArgType> = obj
                            .iter()
                            .map(|(k, v)| (k.clone(), json_value_to_arg_type(v)))
                            .collect();
                        let name = arguments
                            .get("name")
                            .and_then(|v| match v {
                                ArgType::Text(s) => Some(s.clone()),
                                _ => None,
                            })
                            .unwrap_or_default();
                        if !name.is_empty() {
                            calls.push(ModelOutput::ToolCall {
                                id: format!("tool_{}", calls.len()),
                                name,
                                arguments: Some(arguments),
                                signature: None,
                            });
                        }
                    } else {
                        remaining_parts.push(full.as_str());
                    }
                }
                Err(_) => {
                    // Malformed JSON — keep as text
                    remaining_parts.push(full.as_str());
                }
            }
        }

        // Remaining text after last match
        if last_end < response.len() {
            remaining_parts.push(&response[last_end..]);
        }

        let has_tool_calls = !calls.is_empty();
        let remaining_text = if remaining_parts.is_empty() {
            None
        } else {
            let trimmed: Vec<&str> = remaining_parts
                .iter()
                .map(|s| s.trim())
                .filter(|s| !s.is_empty())
                .collect();
            if trimmed.is_empty() {
                None
            } else {
                Some(trimmed.join("\n"))
            }
        };

        Ok(ExtractResult {
            calls,
            remaining_text,
            has_tool_calls,
        })
    }

    fn format_tool_response(
        &self,
        result: &Messages,
    ) -> Result<serde_json::Value, ErrorTrace<ToolCallingError>> {
        let Messages::ToolResult {
            id: _,
            name,
            content,
            error_detail,
            ..
        } = result
        else {
            return Err(ErrorTrace::new(ToolCallingError::Response {
                tool_name: String::new(),
                reason: "expected Messages::ToolResult".to_string(),
            })
            .attach("source=text_based_formatter"));
        };

        let content_str = match content {
            crate::types::UserModelContent::Text(t) => t.content.clone(),
            crate::types::UserModelContent::Image(_) => "[image]".to_string(),
        };

        let error_note = if error_detail.is_some() {
            " (error)"
        } else {
            ""
        };
        Ok(serde_json::json!({
            "role": "tool",
            "name": name,
            "content": format!("[{name}] {content_str}{error_note}"),
        }))
    }
}

pub trait Model {
    /// The tool formatter type used by this model.
    type Formatter: ToolFormatter;
    /// [`spec`] returns model specification information for this target model.
    fn spec(&self) -> ModelSpec;

    /// Returns the `ModelProviderDescriptor` for this model instance,
    /// which contains pricing (`ModelUsageCosting`), context window, etc.
    /// Returns `None` for dynamically loaded or local models without
    /// descriptor metadata.
    fn descriptor(&self) -> Option<ModelProviderDescriptor>;

    /// [`costing`] returns model usage costing report.
    ///
    /// # Errors
    ///
    /// Returns a [`GenerationError`] if the underlying model fails to generate output.
    fn costing(&self) -> GenerationResult<UsageReport>;

    /// [`generate`] runs the actual inference within the model outputting
    /// the relevant type of output desired by the specified type.
    ///
    /// It should be expected whatever internal value is returned should
    /// support [`Into<T>`] or whatever conversation mechanism to transform
    /// into the desired output.
    ///
    /// # Errors
    ///
    /// Returns a [`GenerationError`] if inference fails.
    fn generate(
        &self,
        interaction: ModelInteraction,
        specs: Option<ModelParams>,
    ) -> GenerationResult<Vec<Messages>>;

    /// [`stream`] will returns a stream iterator which will represent the
    /// results of the prompt from the underlying model.
    ///
    /// It purposely uses the [`crate::valtron::StreamIterator`] type
    /// which supports a more ergonomic usecase in async (computations are async)
    /// but provides a sync iterator based API to receive result.
    ///
    /// # Errors
    ///
    /// Returns a [`GenerationError`] if streaming fails.
    fn stream(
        &self,
        interaction: ModelInteraction,
        specs: Option<ModelParams>,
    ) -> GenerationResult<impl StreamIterator<D = Messages, P = ModelState>>;
}

/// Trait for config types that can provide authentication credentials
/// to a [`ModelProvider`].
///
/// Implement this on provider config types to expose auth through `create()`.
///
/// # Example
///
/// ```text
/// impl AuthProvider for MyProviderConfig {
///     fn auth(&self) -> Option<&AuthCredential> {
///         self.auth.as_ref()
///     }
/// }
/// ```
pub trait AuthProvider {
    fn auth(&self) -> Option<&AuthCredential>;
}

pub trait ModelProvider {
    type Config: AuthProvider;
    type Model: Model;

    /// [`create`] will consume self and the credentials, perform the necessary
    /// operation to properly authenticate provider to ensure provider is fully
    /// ready to service and perform operations.
    ///
    /// This means if the provider requires credentials to function and none of these
    /// get called then all functions should fail when called from this provider.
    ///
    /// It seems reasonable that the provider should handle the refresh and re-authentication/
    /// re-authorization necessary after the initial call by user.
    ///
    /// # Errors
    ///
    /// Returns a [`ModelProviderErrors`] if the provider fails to initialize or authenticate.
    fn create(self, config: Option<Self::Config>) -> ModelProviderResult<Self>
    where
        Self: Sized;

    /// Returns a descriptor for this model provider.
    ///
    /// # Errors
    ///
    /// Returns a [`ModelProviderErrors`] if the provider descriptor cannot be generated.
    fn describe(&self) -> ModelProviderResult<ModelProviderDescriptor>;

    /// [`get_model_by_spec`] returns a Model interaction type that allows you to
    /// perform completions/generations with a given underlying model.
    ///
    /// # Errors
    ///
    /// Returns a [`ModelError`] if the model cannot be loaded or initialized.
    fn get_model(&self, model_id: ModelId) -> ModelProviderResult<Self::Model>;

    /// [`get_model_by_spec`] returns a Model interaction type that allows you to
    /// perform completions/generations with a given underlying model.
    ///
    /// # Errors
    ///
    /// Returns a [`ModelError`] if the model cannot be loaded or initialized.
    fn get_model_by_spec(&self, model_spec: ModelSpec) -> ModelProviderResult<Self::Model>;

    /// [`get_model`] returns a Model interaction type that allows you to
    /// perform completions/generations with a given underlying model.
    ///
    /// # Errors
    ///
    /// Returns a [`ModelRegistryResult`] or the [`ModelSpec`] for the model.
    ///
    fn get_one(&self, model_id: ModelId) -> ModelProviderResult<ModelSpec>;

    /// [`get_all`] returns all models matching the provided model id
    /// from the target source.
    ///
    /// # Errors
    ///
    /// Returns a [`ModelProviderErrors`] if the model list cannot be fetched.
    ///
    fn get_all(&self, model_id: ModelId) -> ModelProviderResult<Vec<ModelSpec>>;
}

// ==================================
// `llama.cpp` Specific Types
// ==================================

/// `ChatMessage` provides an ergonomic way to construct chat messages
/// for use with chat templates and model interactions.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct ChatMessage {
    /// The role of the message sender (e.g., "user", "assistant", "system").
    pub role: String,
    /// The content of the message.
    pub content: String,
}

impl ChatMessage {
    /// Create a new `ChatMessage` with the given role and content.
    #[must_use]
    pub fn new(role: impl Into<String>, content: impl Into<String>) -> Self {
        Self {
            role: role.into(),
            content: content.into(),
        }
    }

    /// Create a user message.
    #[must_use]
    pub fn user(content: impl Into<String>) -> Self {
        Self::new("user", content)
    }

    /// Create an assistant message.
    #[must_use]
    pub fn assistant(content: impl Into<String>) -> Self {
        Self::new("assistant", content)
    }

    /// Create a system message.
    #[must_use]
    pub fn system(content: impl Into<String>) -> Self {
        Self::new("system", content)
    }
}

/// [`KVCacheType`] defines the precision/format of the KV cache.
/// Lower precision types reduce memory usage but may affect quality.
#[derive(Debug, Clone, Copy, Default, Serialize, Deserialize, PartialEq)]
pub enum KVCacheType {
    /// Full 32-bit floating point (highest precision, most memory).
    F32,
    /// 16-bit floating point (good balance).
    #[default]
    F16,
    /// 8-bit quantized (less memory, slight quality loss).
    Q8_0,
    /// 5-bit quantized (even less memory).
    Q5_0,
}

impl KVCacheType {
    /// Returns the number of bytes per element for this cache type.
    #[must_use]
    pub const fn bytes_per_element(&self) -> usize {
        match self {
            KVCacheType::F32 => 4,
            KVCacheType::F16 => 2,
            KVCacheType::Q8_0 | KVCacheType::Q5_0 => 1,
        }
    }
}

/// [`SplitMode`] defines how to split model layers across multiple GPUs.
#[derive(Debug, Clone, Copy, Default, Serialize, Deserialize, PartialEq)]
pub enum SplitMode {
    /// No splitting - all layers on a single GPU.
    None,
    /// Split by layer - alternate layers on different GPUs.
    #[default]
    Layer,
    /// Split by row - split individual layers across GPUs.
    Row,
}

/// `LlamaConfig` contains `llama.cpp`-specific configuration options
/// for hardware acceleration and memory management.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct LlamaConfig {
    /// Number of layers to offload to GPU.
    /// If 0, all layers run on CPU.
    pub n_gpu_layers: u32,

    /// Which GPU to use as the main GPU (for multi-GPU systems).
    pub main_gpu: u32,

    /// How to split layers across GPUs.
    pub split_mode: SplitMode,

    /// Type of KV cache to use (affects memory/quality tradeoff).
    pub kv_cache_type: KVCacheType,

    /// Use memory mapping (mmap) for model loading.
    pub use_mmap: bool,

    /// Lock model in physical memory (prevents swapping).
    pub use_mlock: bool,
}

impl Default for LlamaConfig {
    fn default() -> Self {
        Self {
            n_gpu_layers: 0, // CPU-only by default
            main_gpu: 0,     // First GPU
            split_mode: SplitMode::Layer,
            kv_cache_type: KVCacheType::F16,
            use_mmap: true,   // Enable mmap by default
            use_mlock: false, // Don't mlock by default
        }
    }
}

impl LlamaConfig {
    /// Create a new `LlamaConfig` with default values.
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Set the number of GPU layers to offload.
    #[must_use]
    pub fn with_n_gpu_layers(mut self, n: u32) -> Self {
        self.n_gpu_layers = n;
        self
    }

    /// Set the main GPU index.
    #[must_use]
    pub fn with_main_gpu(mut self, gpu_index: u32) -> Self {
        self.main_gpu = gpu_index;
        self
    }

    /// Set the split mode for multi-GPU.
    #[must_use]
    pub fn with_split_mode(mut self, mode: SplitMode) -> Self {
        self.split_mode = mode;
        self
    }

    /// Set the KV cache type.
    #[must_use]
    pub fn with_kv_cache_type(mut self, cache_type: KVCacheType) -> Self {
        self.kv_cache_type = cache_type;
        self
    }

    /// Enable or disable memory mapping.
    #[must_use]
    pub fn with_mmap(mut self, enabled: bool) -> Self {
        self.use_mmap = enabled;
        self
    }

    /// Enable or disable memory locking.
    #[must_use]
    pub fn with_mlock(mut self, enabled: bool) -> Self {
        self.use_mlock = enabled;
        self
    }
}
