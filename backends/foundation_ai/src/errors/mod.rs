//! Error definitions for the `foundation_ai` project.
//!
//! This module defines all error types for the foundation AI inference backend.
//! Provider-specific errors are consolidated into single gated wrapper variants
//! (e.g., `Llama(LlamaError)`) so the core enums stay provider-agnostic.

use foundation_core::extensions::result_ext::BoxedError;

#[cfg(all(feature = "llamacpp", not(target_family = "wasm")))]
pub mod llama;
#[cfg(all(feature = "llamacpp", not(target_family = "wasm")))]
pub use llama::LlamaError;

// ==================================
// Generation Errors
// ==================================

/// Errors that can occur during text generation, embedding extraction, or sampling.
#[derive(Debug)]
pub enum GenerationError {
    /// A boxed error for miscellaneous failures.
    Failed(BoxedError),

    /// Consolidated llama.cpp errors (all 10 kinds).
    #[cfg(all(feature = "llamacpp", not(target_family = "wasm")))]
    Llama(LlamaError),

    /// Candle tensor/compute errors.
    #[cfg(feature = "candle")]
    Candle(candle_core::Error),

    /// Tokenizer failures (from the `tokenizers` crate).
    Tokenizer(String),

    /// Backend execution failures (Valtron scheduling, etc.).
    Backend(String),

    /// Generic generation errors with a message.
    Generic(String),
}

impl From<BoxedError> for GenerationError {
    fn from(e: BoxedError) -> Self {
        GenerationError::Failed(e)
    }
}

impl From<String> for GenerationError {
    fn from(e: String) -> Self {
        GenerationError::Generic(e)
    }
}

#[cfg(feature = "candle")]
impl From<candle_core::Error> for GenerationError {
    fn from(e: candle_core::Error) -> Self {
        GenerationError::Candle(e)
    }
}

impl core::fmt::Display for GenerationError {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        match self {
            GenerationError::Failed(e) => write!(f, "Generation failed: {e}"),
            #[cfg(all(feature = "llamacpp", not(target_family = "wasm")))]
            GenerationError::Llama(e) => write!(f, "llama.cpp: {e}"),
            #[cfg(feature = "candle")]
            GenerationError::Candle(e) => write!(f, "Candle error: {e}"),
            GenerationError::Tokenizer(msg) => write!(f, "Tokenizer error: {msg}"),
            GenerationError::Backend(msg) => write!(f, "Backend error: {msg}"),
            GenerationError::Generic(msg) => write!(f, "Generation error: {msg}"),
        }
    }
}

impl std::error::Error for GenerationError {}

/// Result type alias for generation operations.
pub type GenerationResult<T> = std::result::Result<T, GenerationError>;

// ==================================
// Model Errors
// ==================================

/// Errors related to model loading, initialization, and specification.
#[derive(Debug)]
pub enum ModelErrors {
    /// Model was not found in the registry or cache.
    NotFound(String),

    /// Model failed to load from file or remote source.
    FailedLoading(BoxedError),

    /// Consolidated llama.cpp model/context/embedding errors.
    #[cfg(all(feature = "llamacpp", not(target_family = "wasm")))]
    Llama(LlamaError),

    /// Candle model loading/weight initialization failures.
    CandleModelLoad(String),

    /// Requested model architecture is not supported.
    UnsupportedArchitecture(String),
}

impl core::fmt::Display for ModelErrors {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        match self {
            ModelErrors::NotFound(name) => write!(f, "Model not found: {name}"),
            ModelErrors::FailedLoading(e) => write!(f, "Model failed to load: {e}"),
            #[cfg(all(feature = "llamacpp", not(target_family = "wasm")))]
            ModelErrors::Llama(e) => write!(f, "llama.cpp: {e}"),
            ModelErrors::CandleModelLoad(msg) => write!(f, "Candle model load error: {msg}"),
            ModelErrors::UnsupportedArchitecture(arch) => {
                write!(f, "Unsupported architecture: {arch}")
            }
        }
    }
}

impl std::error::Error for ModelErrors {}

/// Result type alias for model operations.
pub type ModelResult<T> = std::result::Result<T, ModelErrors>;

// ==================================
// Model Provider Errors
// ==================================

/// Errors from the model provider layer (backend initialization, model discovery).
#[derive(Debug)]
pub enum ModelProviderErrors {
    /// Provider or model was not found.
    NotFound(String),

    /// Provider failed to fetch model information.
    FailedFetching(BoxedError),

    /// Underlying model errors.
    ModelErrors(ModelErrors),
}

impl From<ModelErrors> for ModelProviderErrors {
    fn from(e: ModelErrors) -> Self {
        ModelProviderErrors::ModelErrors(e)
    }
}

impl core::fmt::Display for ModelProviderErrors {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        match self {
            ModelProviderErrors::NotFound(name) => write!(f, "Not found: {name}"),
            ModelProviderErrors::FailedFetching(e) => write!(f, "Fetch failed: {e}"),
            ModelProviderErrors::ModelErrors(e) => write!(f, "Model error: {e}"),
        }
    }
}

impl std::error::Error for ModelProviderErrors {}

/// Result type alias for model provider operations.
pub type ModelProviderResult<T> = std::result::Result<T, ModelProviderErrors>;

// ==================================
// Foundation AI Errors (Root Error Type)
// ==================================

/// A comprehensive type representing all errors supported by `foundation_ai`.
#[derive(Debug)]
pub enum FoundationAIErrors {
    /// Model-related errors.
    ModelErrors(ModelErrors),
    /// Generation-related errors.
    GenerationErrors(GenerationError),
    /// Model provider/registry errors.
    RegistryErrors(ModelProviderErrors),
}

impl From<ModelErrors> for FoundationAIErrors {
    fn from(e: ModelErrors) -> Self {
        FoundationAIErrors::ModelErrors(e)
    }
}

impl From<GenerationError> for FoundationAIErrors {
    fn from(e: GenerationError) -> Self {
        FoundationAIErrors::GenerationErrors(e)
    }
}

impl From<ModelProviderErrors> for FoundationAIErrors {
    fn from(e: ModelProviderErrors) -> Self {
        FoundationAIErrors::RegistryErrors(e)
    }
}

impl core::fmt::Display for FoundationAIErrors {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        match self {
            FoundationAIErrors::ModelErrors(e) => write!(f, "Model error: {e}"),
            FoundationAIErrors::GenerationErrors(e) => write!(f, "Generation error: {e}"),
            FoundationAIErrors::RegistryErrors(e) => write!(f, "Registry error: {e}"),
        }
    }
}

impl std::error::Error for FoundationAIErrors {}

/// Result type alias for foundation AI operations.
pub type FoundationAIResult<T> = std::result::Result<T, FoundationAIErrors>;
