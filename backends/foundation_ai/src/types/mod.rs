//! Core definition for what models entail

use std::collections::HashMap;
use std::path::PathBuf;
use std::time::SystemTime;

use derive_more::From;
use foundation_core::extensions::strings_ext::IntoString;
use foundation_core::valtron::StreamIterator;
use foundation_core::wire::simple_http::url::Uri;
use lazy_regex::{lazy_regex, Lazy, Regex};
use serde::{Deserialize, Serialize};

use crate::errors::GenerationResult;
use crate::errors::ModelProviderResult;
use crate::errors::ModelResult;

/// Regex patterns to detect context overflow errors from different providers.
///
/// These patterns match error messages returned when the input exceeds
/// the model's context window.
///
/// Provider-specific patterns (with example error messages):
///
/// - Anthropic: "prompt is too long: 213462 tokens > 200000 maximum"
/// - OpenAI: "Your input exceeds the context window of this model"
/// - Google: "The input token count (1196265) exceeds the maximum number of tokens allowed (1048575)"
/// - xAI: "This model's maximum prompt length is 131072 but the request contains 537812 tokens"
/// - Groq: "Please reduce the length of the messages or completion"
/// - OpenRouter: "This endpoint's maximum context length is X tokens. However, you requested about Y tokens"
/// - llama.cpp: "the request exceeds the available context size, try increasing it"
/// - LM Studio: "tokens to keep from the initial prompt is greater than the context length"
/// - GitHub Copilot: "prompt token count of X exceeds the limit of Y"
/// - MiniMax: "invalid params, context window exceeds limit"
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

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, PartialOrd)]
pub enum CacheRetention {
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

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, PartialOrd)]
pub enum ThinkingLevels {
    Minimal,
    Low,
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

#[derive(From, Serialize, Deserialize, Debug, Clone, PartialEq, PartialOrd)]
pub struct ModelProviderDescriptor {
    pub id: String,
    pub name: String,
    pub api: ModelAPI,
    pub provider: String,
    pub base_url: String,
    pub reasoning: bool,
    pub inputs: [MessageType; 2],
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
#[derive(From, Serialize, Deserialize, Debug, Clone, PartialEq, PartialOrd)]
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
}

#[derive(From, Serialize, Deserialize, Debug, Clone, PartialEq, PartialOrd)]
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

#[derive(From, Serialize, Deserialize, Debug, Copy, Clone, PartialEq, PartialOrd)]
pub struct ModelUsageCosting {
    pub input: f64,
    pub output: f64,
    pub cache_read: f64,
    pub cache_write: f64,
}

#[derive(Serialize, Deserialize, Debug, Copy, Clone, PartialEq, PartialOrd)]
pub enum MessageType {
    Text,
    Images,
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
}

#[allow(clippy::match_same_arms)]
impl From<String> for StopReason {
    fn from(value: String) -> Self {
        match value.to_lowercase().as_str() {
            "length" => Self::Length,
            "tooluse" => Self::ToolUse,
            "error" => Self::Error,
            "aborted" => Self::Aborted,
            "stop" => Self::Stop,
            _ => Self::Stop,
        }
    }
}

#[allow(clippy::match_same_arms)]
impl From<&'static str> for StopReason {
    fn from(value: &'static str) -> Self {
        match value.to_lowercase().as_str() {
            "stop" => Self::Stop,
            "length" => Self::Length,
            "tooluse" => Self::ToolUse,
            "error" => Self::Error,
            "aborted" => Self::Aborted,
            _ => Self::Stop,
        }
    }
}

#[derive(From, Serialize, Deserialize, Debug, Clone, PartialEq, PartialOrd)]
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
}

#[derive(From, Serialize, Deserialize, Debug, Clone, PartialEq)]
pub struct ToolParam {
    value: ArgType,
    name: String,
    description: String,
}

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

static OVERFLOW_SILENT_PATTERN: Lazy<Regex> =
    lazy_regex!(r"(?i)^4(00|13)\s*(status code)?\s*\(no body\)");

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
                    let input_tokens = usage.input + usage.cache_read;
                    if input_tokens > context_window as f64 {
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
    id: String,
    name: String,
    description: String,
    arguments: Option<HashMap<String, ArgType>>,
}

#[derive(From, Serialize, Deserialize, Debug, Clone, PartialEq)]
pub struct ModelInteraction {
    system_prompt: Option<String>,
    messages: Vec<Messages>,
    tools: Vec<Tool>,
}

pub trait ModelProvider {
    /// [`get_model`] returns a Model interaction type that allows you to
    /// perform completions/generations with a given underlying model.
    ///
    /// # Errors
    ///
    /// Returns a [`ModelRegistryResult`] or the [`ModelSpec`] for the model.
    ///
    fn get_one(&self, model_id: ModelId) -> ModelProviderResult<ModelSpec>;

    /// [`get_all`] returns all Model type match the provided modeil id and.
    /// from the target source.
    ///
    /// # Errors
    ///
    /// Returns a [`ModelRegistryResult`] or the [`ModelSpec`] for the model.
    ///
    fn get_all(&self, model_id: ModelId) -> ModelProviderResult<ModelSpec>;
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
}

pub trait Model {
    /// [`spec`] returns model specification information for this target model.
    fn spec(&self) -> ModelSpec;

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
    /// which supports a more ergonomic usecase in async (computation is async)
    /// but provides a sync iterarator based API to receive result.
    ///
    /// # Errors
    ///
    /// Returns a [`GenerationError`] if streaming fails.
    fn stream<T>(
        &self,
        interaction: ModelInteraction,
        specs: Option<ModelParams>,
    ) -> GenerationResult<T>
    where
        T: StreamIterator<Messages, ModelState>;
}

pub trait ModelBackend {
    /// [`get_model`] returns a Model interaction type that allows you to
    /// perform completions/generations with a given underlying model.
    ///
    /// # Errors
    ///
    /// Returns a [`ModelError`] if the model cannot be loaded or initialized.
    fn get_model<T: Model>(&self, model_spec: ModelSpec) -> ModelResult<T>;
}
