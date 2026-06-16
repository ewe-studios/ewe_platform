// Consolidated llama.cpp error types — isolated behind cfg(feature = "llamacpp").

use infrastructure_llama_cpp::{
    ApplyChatTemplateError, ChatTemplateError, DecodeError, EmbeddingsError, EncodeError,
    LlamaContextLoadError, LlamaCppError, LlamaModelLoadError, StringToTokenError,
    TokenToStringError,
};

/// Consolidated error type wrapping all `infrastructure_llama_cpp` errors.
#[derive(Debug)]
pub enum LlamaError {
    Cpp(LlamaCppError),
    Tokenization(StringToTokenError),
    TokenToString(TokenToStringError),
    Decode(DecodeError),
    Encode(EncodeError),
    Embeddings(EmbeddingsError),
    ChatTemplate(ChatTemplateError),
    ApplyChatTemplate(ApplyChatTemplateError),
    ModelLoad(LlamaModelLoadError),
    ContextLoad(LlamaContextLoadError),
}

impl core::fmt::Display for LlamaError {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        match self {
            Self::Cpp(e) => write!(f, "llama.cpp error: {e}"),
            Self::Tokenization(e) => write!(f, "tokenization failed: {e}"),
            Self::TokenToString(e) => write!(f, "token to string failed: {e}"),
            Self::Decode(e) => write!(f, "decode error: {e}"),
            Self::Encode(e) => write!(f, "encode error: {e}"),
            Self::Embeddings(e) => write!(f, "embeddings error: {e}"),
            Self::ChatTemplate(e) => write!(f, "chat template error: {e}"),
            Self::ApplyChatTemplate(e) => write!(f, "apply chat template error: {e}"),
            Self::ModelLoad(e) => write!(f, "model load error: {e}"),
            Self::ContextLoad(e) => write!(f, "context load error: {e}"),
        }
    }
}

impl std::error::Error for LlamaError {}

impl From<LlamaCppError> for LlamaError {
    fn from(e: LlamaCppError) -> Self { Self::Cpp(e) }
}
impl From<StringToTokenError> for LlamaError {
    fn from(e: StringToTokenError) -> Self { Self::Tokenization(e) }
}
impl From<TokenToStringError> for LlamaError {
    fn from(e: TokenToStringError) -> Self { Self::TokenToString(e) }
}
impl From<DecodeError> for LlamaError {
    fn from(e: DecodeError) -> Self { Self::Decode(e) }
}
impl From<EncodeError> for LlamaError {
    fn from(e: EncodeError) -> Self { Self::Encode(e) }
}
impl From<EmbeddingsError> for LlamaError {
    fn from(e: EmbeddingsError) -> Self { Self::Embeddings(e) }
}
impl From<ChatTemplateError> for LlamaError {
    fn from(e: ChatTemplateError) -> Self { Self::ChatTemplate(e) }
}
impl From<ApplyChatTemplateError> for LlamaError {
    fn from(e: ApplyChatTemplateError) -> Self { Self::ApplyChatTemplate(e) }
}
impl From<LlamaModelLoadError> for LlamaError {
    fn from(e: LlamaModelLoadError) -> Self { Self::ModelLoad(e) }
}
impl From<LlamaContextLoadError> for LlamaError {
    fn from(e: LlamaContextLoadError) -> Self { Self::ContextLoad(e) }
}

// Convenience: LlamaError → GenerationError
impl From<LlamaError> for super::GenerationError {
    fn from(e: LlamaError) -> Self { Self::Llama(e) }
}

// Convenience: LlamaError → ModelErrors
impl From<LlamaError> for super::ModelErrors {
    fn from(e: LlamaError) -> Self { Self::Llama(e) }
}

// Convenience: individual llama errors → GenerationError (through LlamaError)
impl From<LlamaCppError> for super::GenerationError {
    fn from(e: LlamaCppError) -> Self { Self::Llama(LlamaError::from(e)) }
}
impl From<StringToTokenError> for super::GenerationError {
    fn from(e: StringToTokenError) -> Self { Self::Llama(LlamaError::from(e)) }
}
impl From<TokenToStringError> for super::GenerationError {
    fn from(e: TokenToStringError) -> Self { Self::Llama(LlamaError::from(e)) }
}
impl From<DecodeError> for super::GenerationError {
    fn from(e: DecodeError) -> Self { Self::Llama(LlamaError::from(e)) }
}
impl From<EncodeError> for super::GenerationError {
    fn from(e: EncodeError) -> Self { Self::Llama(LlamaError::from(e)) }
}
impl From<EmbeddingsError> for super::GenerationError {
    fn from(e: EmbeddingsError) -> Self { Self::Llama(LlamaError::from(e)) }
}
impl From<ChatTemplateError> for super::GenerationError {
    fn from(e: ChatTemplateError) -> Self { Self::Llama(LlamaError::from(e)) }
}
impl From<ApplyChatTemplateError> for super::GenerationError {
    fn from(e: ApplyChatTemplateError) -> Self { Self::Llama(LlamaError::from(e)) }
}
impl From<LlamaModelLoadError> for super::GenerationError {
    fn from(e: LlamaModelLoadError) -> Self { Self::Llama(LlamaError::from(e)) }
}
impl From<LlamaContextLoadError> for super::GenerationError {
    fn from(e: LlamaContextLoadError) -> Self { Self::Llama(LlamaError::from(e)) }
}

// Convenience: individual llama errors → ModelErrors (through LlamaError)
impl From<LlamaModelLoadError> for super::ModelErrors {
    fn from(e: LlamaModelLoadError) -> Self { Self::Llama(LlamaError::from(e)) }
}
impl From<LlamaContextLoadError> for super::ModelErrors {
    fn from(e: LlamaContextLoadError) -> Self { Self::Llama(LlamaError::from(e)) }
}
impl From<EmbeddingsError> for super::ModelErrors {
    fn from(e: EmbeddingsError) -> Self { Self::Llama(LlamaError::from(e)) }
}
