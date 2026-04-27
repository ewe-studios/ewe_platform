//! Backend modules implement `ModelBackend` for different implementations.

pub mod huggingface_gguf_provider;
pub mod llamacpp;
pub mod llamacpp_helpers;
pub mod openai_provider;
pub mod openai_responses_provider;
pub mod anthropic_messages_provider;

#[cfg(feature = "candle")]
pub mod candle;

#[cfg(feature = "candle")]
pub mod huggingface_candle_provider;
