//! Backend modules implement `ModelBackend` for different implementations.

#[cfg(all(feature = "llamacpp", not(target_family = "wasm")))]
pub mod huggingface_gguf_provider;
#[cfg(all(feature = "llamacpp", not(target_family = "wasm")))]
pub mod llamacpp;
#[cfg(all(feature = "llamacpp", not(target_family = "wasm")))]
pub mod llamacpp_helpers;

pub mod anthropic_messages_provider;
pub mod backend_utils;
pub mod openai_provider;
pub mod openai_responses_provider;

#[cfg(all(feature = "candle", not(target_family = "wasm")))]
pub mod candle;

#[cfg(all(feature = "candle", not(target_family = "wasm")))]
pub mod huggingface_candle_provider;
