//! Core definition for what models entail

use std::collections::HashMap;
use std::path::PathBuf;

use derive_more::From;
use foundation_core::extensions::strings_ext::IntoString;
use foundation_core::valtron::StreamIterator;
use foundation_core::wire::simple_http::url::Uri;
use serde::{Deserialize, Serialize};

use crate::errors::GenerationResult;
use crate::errors::ModelProviderResult;
use crate::errors::ModelResult;

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
pub enum KnownModelProviders {
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
    Custom(String),
}

impl From<&'static str> for KnownModelProviders {
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

impl From<String> for KnownModelProviders {
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

    /// A model wih a specific alias generally not the full name
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
    pub cach_read: f64,
    pub cach_write: f64,
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
}

/// [`UsageCosting`] represents the overal costing in actual currency value.
#[derive(From, Serialize, Deserialize, Debug, Clone, PartialEq)]
pub struct UsageCosting {
    pub curreny: String,
    pub input: f64,
    pub output: f64,
    pub cache_read: f64,
    pub cache_write: f64,
    pub total_tokens: f64,
}

/// [`UsageReport`] represents the accumulated usage at the point in time of
/// generation and the overal costing of that usage.
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
pub struct TextOutput {
    pub content: String,
    pub signature: Option<String>,
}

#[derive(From, Serialize, Deserialize, Debug, Clone, PartialEq)]
pub struct ImageOutput {
    pub b64: String,
    pub mime_type: MimeType,
}

#[derive(From, Serialize, Deserialize, Debug, Clone, PartialEq)]
pub enum UserModelOutput {
    Text(TextOutput),
    Image(ImageOutput),
}

#[derive(From, Serialize, Deserialize, Debug, Clone, PartialEq)]
pub enum ModelOutput {
    Text(TextOutput),
    Image(ImageOutput),
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

pub enum Messages {
    UserMessage {
        role: String,
        content: UserModelOutput,
        signature: Option<String>,
    },
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

pub trait Model {
    /// [`spec`] returns model specification information for this target model.
    fn spec(&self) -> ModelSpec;

    /// [`costing`] returns model usage costing report.
    ///
    /// # Errors
    ///
    /// Returns a [`GenerationError`] if the underlying model fails to generate output.
    fn costing(&self) -> GenerationResult<UsageReport>;

    /// [`text`] calls the [`Model::generate`] method internally which
    /// should specifically take in a prompt and generate a text output.
    ///
    /// # Errors
    ///
    /// Returns a [`GenerationError`] if the underlying model fails to generate output.
    fn text(&self, prompt: String, specs: Option<ModelParams>) -> GenerationResult<String> {
        self.generate::<String>(prompt, specs)
    }

    /// [`stream_text]` provides a streaming version of the [`Model::text`] method which
    /// supports streaming text output.
    ///
    /// # Errors
    ///
    /// Returns a [`GenerationError`] if the underlying model fails to stream output.
    fn stream_text<T>(&self, prompt: String, specs: Option<ModelParams>) -> GenerationResult<T>
    where
        T: StreamIterator<String, ()>,
    {
        self.stream::<T, String, ()>(prompt, specs)
    }

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
    fn generate<T>(&self, prompt: String, specs: Option<ModelParams>) -> GenerationResult<T>;

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
    fn stream<T, D, P>(&self, prompt: String, specs: Option<ModelParams>) -> GenerationResult<T>
    where
        T: StreamIterator<D, P>;
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
