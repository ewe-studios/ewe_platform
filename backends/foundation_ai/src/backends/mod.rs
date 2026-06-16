//! Backend modules implement `ModelBackend` for different implementations.

#[cfg(feature = "llamacpp")]
pub mod huggingface_gguf_provider;
#[cfg(feature = "llamacpp")]
pub mod llamacpp;
#[cfg(feature = "llamacpp")]
pub mod llamacpp_helpers;

pub mod openai_provider;
pub mod openai_responses_provider;
pub mod anthropic_messages_provider;

#[cfg(feature = "candle")]
pub mod candle;

#[cfg(feature = "candle")]
pub mod huggingface_candle_provider;
