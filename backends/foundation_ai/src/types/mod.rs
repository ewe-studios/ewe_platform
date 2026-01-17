//! Core definition for what models entail

use std::path::Path;

use foundation_core::valtron::StreamIterator;

use crate::errors::GenerationResult;
use crate::errors::ModelResult;

pub struct ModelExecutionSpec {
    // standard model properties
    pub context_length: usize,
    pub max_threads: usize,
    pub template: Option<String>,

    // properties controlling how we want this to overall output
    // temperature, top_k, top_p,etc
    pub max_tokens: usize,
    pub temperature: f32,
    pub top_p: f32,
    pub top_k: f32,
    pub repeat_penalty: f32,
    pub seed: Option<u32>,
    pub streaming: bool,
    pub stop_tokens: Vec<String>,
}

pub struct ModelSpec {
    pub name: String,
    pub model_directory: Option<Path>,
    pub lora_directory: Option<Path>,
}

pub trait Model {
    fn generate(&self, prompt: String) -> GenerationResult<String>;
    fn stream(&self, prompt: String) -> GenerationResult<StreamIterator<String, ()>>;
}

pub trait Backend {
    fn get_model(&self, model_spec: ModelSpec) -> ModelResult<Model>;
}
