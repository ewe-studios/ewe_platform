//! Core definition for what models entail

use std::path::PathBuf;

use foundation_core::valtron::StreamIterator;
use foundation_core::wire::simple_http::url::Uri;
use serde::{Deserialize, Serialize};

use crate::errors::GenerationResult;
use crate::errors::ModelProviderResult;
use crate::errors::ModelResult;

#[derive(Serialize, Deserialize, Debug, Copy, Clone, PartialEq, PartialOrd)]
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

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, PartialOrd)]
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
#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, PartialOrd)]
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
#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, PartialOrd)]
pub struct ModelParams {
    pub max_tokens: usize,
    pub temperature: f32,
    pub top_p: f32,
    pub top_k: f32,
    pub repeat_penalty: f32,
    pub seed: Option<u32>,
    pub stop_tokens: Vec<String>,
}

/// [`ResponseSpec`] defines expectation for how a response specification
/// should be returned.
#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, PartialOrd)]
pub struct ResponseConfig {
    pub call_spec: ModelParams,
    pub streaming: bool,
}

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, PartialOrd)]
pub struct ModelConfig {
    // standard model properties
    pub context_length: usize,
    pub max_threads: usize,
    pub template: Option<String>,

    /// The [`ResponseConfig`] defines the properties controlling
    /// how we want this to overall output
    /// temperature, `top_k``top_p`, etc
    pub response_config: ResponseConfig,
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

#[derive(Serialize, Deserialize, Debug, Copy, Clone, PartialEq, PartialOrd)]
pub enum MessageType {
    Text,
    Images,
}

#[derive(Serialize, Deserialize, Debug, Copy, Clone, PartialEq, PartialOrd)]
pub struct ModelUsageCosting {
    pub input: f64,
    pub output: f64,
    pub cach_read: f64,
    pub cach_write: f64,
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
