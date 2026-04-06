//! Error definitions for the `foundation_ai` project.
//!
//! This module defines all error types for the foundation AI inference backend.
//! Errors use `derive_more::From` for automatic conversions and manual `Display` implementations.

use foundation_core::extensions::result_ext::BoxedError;
use infrastructure_llama_cpp::{
    ChatTemplateError, DecodeError, EmbeddingsError, EncodeError,
    LlamaContextLoadError, LlamaCppError, LlamaModelLoadError, StringToTokenError,
    ApplyChatTemplateError,
};

// ==================================
// Generation Errors
// ==================================

/// Errors that can occur during text generation, embedding extraction, or sampling.
#[derive(Debug)]
pub enum GenerationError {
    /// A boxed error for miscellaneous failures.
    Failed(BoxedError),

    /// llama.cpp operational errors from the infrastructure layer.
    LlamaCpp(LlamaCppError),

    /// Tokenization failures when converting text to tokens.
    Tokenization(StringToTokenError),

    /// Decode errors when processing tokens through the model.
    Decode(DecodeError),

    /// Encode errors during embedding extraction.
    Encode(EncodeError),

    /// Chat template application failures.
    ChatTemplate(ChatTemplateError),

    /// Chat template application failures (alternative error type).
    ApplyChatTemplate(ApplyChatTemplateError),

    /// llama.cpp model load errors.
    LlamaModelLoad(LlamaModelLoadError),

    /// llama.cpp context load errors.
    LlamaContextLoad(LlamaContextLoadError),

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

impl From<LlamaCppError> for GenerationError {
    fn from(e: LlamaCppError) -> Self {
        GenerationError::LlamaCpp(e)
    }
}

impl From<StringToTokenError> for GenerationError {
    fn from(e: StringToTokenError) -> Self {
        GenerationError::Tokenization(e)
    }
}

impl From<DecodeError> for GenerationError {
    fn from(e: DecodeError) -> Self {
        GenerationError::Decode(e)
    }
}

impl From<EncodeError> for GenerationError {
    fn from(e: EncodeError) -> Self {
        GenerationError::Encode(e)
    }
}

impl From<ChatTemplateError> for GenerationError {
    fn from(e: ChatTemplateError) -> Self {
        GenerationError::ChatTemplate(e)
    }
}

impl From<ApplyChatTemplateError> for GenerationError {
    fn from(e: ApplyChatTemplateError) -> Self {
        GenerationError::ApplyChatTemplate(e)
    }
}

impl From<LlamaModelLoadError> for GenerationError {
    fn from(e: LlamaModelLoadError) -> Self {
        GenerationError::LlamaModelLoad(e)
    }
}

impl From<LlamaContextLoadError> for GenerationError {
    fn from(e: LlamaContextLoadError) -> Self {
        GenerationError::LlamaContextLoad(e)
    }
}

impl core::fmt::Display for GenerationError {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        match self {
            GenerationError::Failed(e) => write!(f, "Generation failed: {e}"),
            GenerationError::LlamaCpp(e) => write!(f, "llama.cpp error: {e}"),
            GenerationError::Tokenization(e) => write!(f, "Tokenization failed: {e}"),
            GenerationError::Decode(e) => write!(f, "Decode error: {e}"),
            GenerationError::Encode(e) => write!(f, "Encode error: {e}"),
            GenerationError::ChatTemplate(e) => write!(f, "Chat template error: {e}"),
            GenerationError::ApplyChatTemplate(e) => write!(f, "Apply chat template error: {e}"),
            GenerationError::LlamaModelLoad(e) => write!(f, "llama.cpp model load error: {e}"),
            GenerationError::LlamaContextLoad(e) => write!(f, "llama.cpp context load error: {e}"),
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

    /// llama.cpp model loading failures.
    LlamaModelLoad(LlamaModelLoadError),

    /// llama.cpp context initialization failures.
    LlamaContextLoad(LlamaContextLoadError),

    /// Embedding extraction errors.
    Embeddings(EmbeddingsError),
}

impl From<LlamaModelLoadError> for ModelErrors {
    fn from(e: LlamaModelLoadError) -> Self {
        ModelErrors::LlamaModelLoad(e)
    }
}

impl From<LlamaContextLoadError> for ModelErrors {
    fn from(e: LlamaContextLoadError) -> Self {
        ModelErrors::LlamaContextLoad(e)
    }
}

impl From<EmbeddingsError> for ModelErrors {
    fn from(e: EmbeddingsError) -> Self {
        ModelErrors::Embeddings(e)
    }
}

impl core::fmt::Display for ModelErrors {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        match self {
            ModelErrors::NotFound(name) => write!(f, "Model not found: {name}"),
            ModelErrors::FailedLoading(e) => write!(f, "Model failed to load: {e}"),
            ModelErrors::LlamaModelLoad(e) => write!(f, "llama.cpp model load error: {e}"),
            ModelErrors::LlamaContextLoad(e) => write!(f, "llama.cpp context load error: {e}"),
            ModelErrors::Embeddings(e) => write!(f, "Embedding error: {e}"),
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

/// A comprehensive type representing all errors supported by foundation_ai.
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
