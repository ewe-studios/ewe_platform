//! Core definition for what models entail

use std::path::PathBuf;

use foundation_core::valtron::StreamIterator;

use crate::errors::GenerationResult;
use crate::errors::ModelRegistryResult;
use crate::errors::ModelResult;

/// [`CallSpec`] defines the calling configuration for the model
/// which can be customized as needed for different use-case.
pub struct CallSpec {
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
pub struct ResponseSpec {
    pub call_spec: CallSpec,
    pub streaming: bool,
}

pub struct ModelExecutionSpec {
    // standard model properties
    pub context_length: usize,
    pub max_threads: usize,
    pub template: Option<String>,

    /// The [`ResponseSpec`] defines the properties controlling
    /// how we want this to overall output
    /// temperature, `top_k``top_p`, etc
    pub call_response_spec: ResponseSpec,
}

pub enum ModelRegistrySource {
    /// Http endpoint which contains the target model file.
    HTTP(String),

    /// Model repository name  where the model is located in hugging face.
    HuggingFace(String),
}

pub trait ModelRegistry {
    /// [`get_model`] returns a Model interaction type that allows you to
    /// perform completions/generations with a given underlying model.
    ///
    /// # Errors
    ///
    /// Returns a [`ModelRegistryResult`] or the [`ModelSpec`] for the model.
    ///
    fn get_model(
        &self,
        alias_name: String,
        source: ModelRegistrySource,
    ) -> ModelRegistryResult<ModelSpec>;
}

pub struct ModelSpec {
    pub name: String,

    /// The target device to use for this model execution.
    pub devices: Option<Vec<u8>>,

    /// Optional model registry name to be loaded from a target
    /// registry.
    pub registry_name: Option<ModelRegistrySource>,

    /// The optional path to the model file/directory according to the
    /// for which the backend will use.
    pub model_location: Option<PathBuf>,

    /// The optional path to the lora model files/directory for lora
    /// optimized inference with the main model file.
    pub lora_location: Option<PathBuf>,
}

pub trait Model {
    /// [`text`] calls the [`Model::generate`] method internally which
    /// should specifically take in a prompt and generate a text output.
    fn text(&self, prompt: String, specs: Option<CallSpec>) -> GenerationResult<String> {
        self.generate::<String>(prompt, specs)
    }

    /// [`stream_text]` provides a streaming version of the [`Model::text`] method which
    /// supports streaming text output.
    fn stream_text<T>(&self, prompt: String, specs: Option<CallSpec>) -> GenerationResult<T>
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
    fn generate<T>(&self, prompt: String, specs: Option<CallSpec>) -> GenerationResult<T>;

    /// [`stream`] will returns a stream iterator which will represent the
    /// results of the prompt from the underlying model.
    ///
    /// It purposely uses the [`crate::valtron::StreamIterator`] type
    /// which supports a more ergonomic usecase in async (computation is async)
    /// but provides a sync iterarator based API to receive result.
    fn stream<T, D, P>(&self, prompt: String, specs: Option<CallSpec>) -> GenerationResult<T>
    where
        T: StreamIterator<D, P>;
}

pub trait ModelBackend {
    /// [`get_model`] returns a Model interaction type that allows you to
    /// perform completions/generations with a given underlying model.
    fn get_model<T: Model>(&self, model_spec: ModelSpec) -> ModelResult<T>;
}
