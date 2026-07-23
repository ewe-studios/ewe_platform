//! Agent presets — one-call routers (and agent builders) for the common
//! model combinations.
//!
//! WHY: The whole point of the harness is "super easy to use": a caller who
//! just wants "GLM 5.2 for chat with a small Gemma for memory" should not have
//! to know about [`RouterMix`], routing rules, or which quantization a repo
//! ships. These functions encode sensible defaults while still returning the
//! customisable builder.
//!
//! WHAT: Two layers per combination —
//! * a `*_router()` function returning a [`RouterPreset`] (the router +
//!   model ids), for callers who want to mix further or inspect routing; and
//! * a `*_session()` function that takes the `SessionId` and returns a ready
//!   [`AgentSessionBuilder`] with the models already wired.
//!
//! Combinations:
//! 1. [`glm52_gemma_router`] / [`glm52_gemma_session`] — GLM 5.2 main +
//!    Gemma 4 E2B memory.
//! 2. [`qwen36_gemma_router`] / [`qwen36_gemma_session`] — Qwen 3.6 main +
//!    Gemma 4 E2B memory.
//! 3. [`gemma_router`] / [`gemma_session`] — Gemma 4 26B main +
//!    Gemma 4 E2B memory (Gemma for both roles, big + small).
//! 4. [`claude_router`] / [`claude_session`] — Claude Opus main +
//!    Claude Sonnet memory (Anthropic).
//! 5. [`openai_chat_router`] / [`openai_chat_session`] — GPT-4o main +
//!    GPT-4o-mini memory (Chat Completions API).
//! 6. [`openai_responses_router`] / [`openai_responses_session`] — GPT-4o main
//!    + GPT-4o-mini memory (Responses API).
//! 7. [`candle_llama_router`] / [`candle_llama_session`] — a single Candle
//!    (safetensors) model. Candle currently only implements the **Llama**
//!    architecture; other architectures error as unsupported.
//!
//! HOW: each `*_router()` builds the concrete providers (defaulting to
//! `Q4_K_M` for local GGUF), feeds them to [`RouterMix`], and returns the
//! preset. Each `*_session()` delegates to its router function and calls
//! [`RouterPreset::into_agent_builder`].

use foundation_db::traits::DocumentStore;

use crate::agentic::{AgentSessionBuilder, MemoryStore};
#[cfg(feature = "candle")]
use crate::backends::candle::CandleArchitecture;
#[cfg(feature = "candle")]
#[cfg(all(feature = "candle", not(target_family = "wasm")))]
use crate::backends::huggingface_candle_provider::{
    HuggingFaceCandleConfig, HuggingFaceCandleProvider,
};
#[cfg(all(feature = "llamacpp", not(target_family = "wasm")))]
use crate::backends::huggingface_gguf_provider::HuggingFaceGGUFConfig;
use crate::types::{ModelId, SessionId};

use super::providers::{
    CloudPresets, CLAUDE_OPUS, CLAUDE_SONNET, OPENAI_GPT4O, OPENAI_GPT4O_MINI,
};
#[cfg(all(feature = "llamacpp", not(target_family = "wasm")))]
use super::providers::{Gemma4E2b, Gemma4_26b, Glm52, Qwen36};
use super::router::{RouterMix, RouterPreset};

/// `ModelId::Name` from a model id string, default quantization.
fn named(id: &str) -> ModelId {
    ModelId::Name(id.to_string(), None)
}

// ===========================================================================
// 1. GLM 5.2 (main) + Gemma 4 E2B (memory)
// ===========================================================================

/// Router preset: GLM 5.2 as the main model, Gemma 4 E2B as the small memory
/// model. `main`/`memory` override the GGUF provider config (cache dir, GPU
/// layers, quantization); pass `None` for the `Q4_K_M` defaults.
///
/// # Errors
/// Returns an error string if either GGUF provider cannot be constructed.
#[cfg(all(feature = "llamacpp", not(target_family = "wasm")))]
pub fn glm52_gemma_router(
    main: Option<HuggingFaceGGUFConfig>,
    memory: Option<HuggingFaceGGUFConfig>,
) -> Result<RouterPreset, String> {
    let main_provider = Glm52::q4_k_m(main)?;
    let memory_provider = Gemma4E2b::q4_k_m(memory)?;
    Ok(RouterMix::new()
        .primary(main_provider, named(Glm52::MODEL_ID))
        .memory(memory_provider, named(Gemma4E2b::MODEL_ID))
        .build())
}

/// Agent builder for [`glm52_gemma_router`], wired to `session_id`.
///
/// # Errors
/// Returns an error string if either GGUF provider cannot be constructed.
#[cfg(all(feature = "llamacpp", not(target_family = "wasm")))]
pub fn glm52_gemma_session<D, M>(
    session_id: SessionId,
    main: Option<HuggingFaceGGUFConfig>,
    memory: Option<HuggingFaceGGUFConfig>,
) -> Result<AgentSessionBuilder<D, M>, String>
where
    D: DocumentStore + 'static,
    M: MemoryStore + 'static,
{
    Ok(glm52_gemma_router(main, memory)?.into_agent_builder(session_id))
}

// ===========================================================================
// 2. Qwen 3.6 (main) + Gemma 4 E2B (memory)
// ===========================================================================

/// Router preset: Qwen 3.6 35B-A3B as the main model, Gemma 4 E2B as the small
/// memory model.
///
/// # Errors
/// Returns an error string if either GGUF provider cannot be constructed.
#[cfg(all(feature = "llamacpp", not(target_family = "wasm")))]
pub fn qwen36_gemma_router(
    main: Option<HuggingFaceGGUFConfig>,
    memory: Option<HuggingFaceGGUFConfig>,
) -> Result<RouterPreset, String> {
    let main_provider = Qwen36::q4_k_m(main)?;
    let memory_provider = Gemma4E2b::q4_k_m(memory)?;
    Ok(RouterMix::new()
        .primary(main_provider, named(Qwen36::MODEL_ID))
        .memory(memory_provider, named(Gemma4E2b::MODEL_ID))
        .build())
}

/// Agent builder for [`qwen36_gemma_router`], wired to `session_id`.
///
/// # Errors
/// Returns an error string if either GGUF provider cannot be constructed.
#[cfg(all(feature = "llamacpp", not(target_family = "wasm")))]
pub fn qwen36_gemma_session<D, M>(
    session_id: SessionId,
    main: Option<HuggingFaceGGUFConfig>,
    memory: Option<HuggingFaceGGUFConfig>,
) -> Result<AgentSessionBuilder<D, M>, String>
where
    D: DocumentStore + 'static,
    M: MemoryStore + 'static,
{
    Ok(qwen36_gemma_router(main, memory)?.into_agent_builder(session_id))
}

// ===========================================================================
// 3. Gemma 4 26B (main) + Gemma 4 E2B (memory) — Gemma for both roles
// ===========================================================================

/// Router preset: Gemma 4 26B-A4B as the big main model, Gemma 4 E2B as the
/// small memory model — Gemma on both ends.
///
/// # Errors
/// Returns an error string if either GGUF provider cannot be constructed.
#[cfg(all(feature = "llamacpp", not(target_family = "wasm")))]
pub fn gemma_router(
    main: Option<HuggingFaceGGUFConfig>,
    memory: Option<HuggingFaceGGUFConfig>,
) -> Result<RouterPreset, String> {
    let main_provider = Gemma4_26b::q4_k_m(main)?;
    let memory_provider = Gemma4E2b::q4_k_m(memory)?;
    Ok(RouterMix::new()
        .primary(main_provider, named(Gemma4_26b::MODEL_ID))
        .memory(memory_provider, named(Gemma4E2b::MODEL_ID))
        .build())
}

/// Agent builder for [`gemma_router`], wired to `session_id`.
///
/// # Errors
/// Returns an error string if either GGUF provider cannot be constructed.
#[cfg(all(feature = "llamacpp", not(target_family = "wasm")))]
pub fn gemma_session<D, M>(
    session_id: SessionId,
    main: Option<HuggingFaceGGUFConfig>,
    memory: Option<HuggingFaceGGUFConfig>,
) -> Result<AgentSessionBuilder<D, M>, String>
where
    D: DocumentStore + 'static,
    M: MemoryStore + 'static,
{
    Ok(gemma_router(main, memory)?.into_agent_builder(session_id))
}

// ===========================================================================
// 4. Claude Opus (main) + Claude Sonnet (memory) — Anthropic
// ===========================================================================

/// Router preset: Claude Opus as the main model, Claude Sonnet as the memory
/// model, both via the Anthropic Messages provider.
///
/// # Errors
/// Returns an error string if a provider cannot be constructed.
pub fn claude_router(api_key: &str) -> Result<RouterPreset, String> {
    let main_provider = CloudPresets::claude_opus(api_key)?;
    let memory_provider = CloudPresets::claude_sonnet(api_key)?;
    Ok(RouterMix::new()
        .primary(main_provider, named(CLAUDE_OPUS))
        .memory(memory_provider, named(CLAUDE_SONNET))
        .build())
}

/// Agent builder for [`claude_router`], wired to `session_id`.
///
/// # Errors
/// Returns an error string if a provider cannot be constructed.
pub fn claude_session<D, M>(
    session_id: SessionId,
    api_key: &str,
) -> Result<AgentSessionBuilder<D, M>, String>
where
    D: DocumentStore + 'static,
    M: MemoryStore + 'static,
{
    Ok(claude_router(api_key)?.into_agent_builder(session_id))
}

// ===========================================================================
// 5. OpenAI Chat Completions — GPT-4o (main) + GPT-4o-mini (memory)
// ===========================================================================

/// Router preset: GPT-4o as the main model, GPT-4o-mini as the memory model,
/// both via the OpenAI **Chat Completions** provider.
///
/// # Errors
/// Returns an error string if a provider cannot be constructed.
pub fn openai_chat_router(api_key: &str) -> Result<RouterPreset, String> {
    let main_provider = CloudPresets::openai_gpt4o(api_key)?;
    let memory_provider = CloudPresets::openai_gpt4o(api_key)?;
    Ok(RouterMix::new()
        .primary(main_provider, named(OPENAI_GPT4O))
        .memory(memory_provider, named(OPENAI_GPT4O_MINI))
        .build())
}

/// Agent builder for [`openai_chat_router`], wired to `session_id`.
///
/// # Errors
/// Returns an error string if a provider cannot be constructed.
pub fn openai_chat_session<D, M>(
    session_id: SessionId,
    api_key: &str,
) -> Result<AgentSessionBuilder<D, M>, String>
where
    D: DocumentStore + 'static,
    M: MemoryStore + 'static,
{
    Ok(openai_chat_router(api_key)?.into_agent_builder(session_id))
}

// ===========================================================================
// 6. OpenAI Responses API — GPT-4o (main) + GPT-4o-mini (memory)
// ===========================================================================

/// Router preset: GPT-4o as the main model, GPT-4o-mini as the memory model,
/// both via the OpenAI **Responses** provider (`/v1/responses`).
///
/// # Errors
/// Returns an error string if a provider cannot be constructed.
pub fn openai_responses_router(api_key: &str) -> Result<RouterPreset, String> {
    let main_provider = CloudPresets::openai_responses(api_key)?;
    let memory_provider = CloudPresets::openai_responses(api_key)?;
    Ok(RouterMix::new()
        .primary(main_provider, named(OPENAI_GPT4O))
        .memory(memory_provider, named(OPENAI_GPT4O_MINI))
        .build())
}

/// Agent builder for [`openai_responses_router`], wired to `session_id`.
///
/// # Errors
/// Returns an error string if a provider cannot be constructed.
pub fn openai_responses_session<D, M>(
    session_id: SessionId,
    api_key: &str,
) -> Result<AgentSessionBuilder<D, M>, String>
where
    D: DocumentStore + 'static,
    M: MemoryStore + 'static,
{
    Ok(openai_responses_router(api_key)?.into_agent_builder(session_id))
}

// ===========================================================================
// 7. Candle (safetensors) — single Llama-architecture model
// ===========================================================================

/// Router preset: a single Candle-backed safetensors model as the main model.
///
/// Candle currently implements only the **Llama** architecture
/// ([`CandleArchitecture::Llama`]); other architectures load-fail as
/// unsupported. `repo_id` is a HuggingFace repo (e.g.
/// `"meta-llama/Llama-3.2-1B-Instruct"`). Pass a custom
/// [`HuggingFaceCandleConfig`] to set cache dir / dtype / context length, or
/// `None` for defaults (Llama architecture).
///
/// # Errors
/// Returns an error string if the Candle provider cannot be constructed.
#[cfg(feature = "candle")]
#[cfg(all(feature = "candle", not(target_family = "wasm")))]
pub fn candle_llama_router(
    repo_id: &str,
    config: Option<HuggingFaceCandleConfig>,
) -> Result<RouterPreset, String> {
    let config = config.unwrap_or_else(|| {
        HuggingFaceCandleConfig::builder()
            .architecture(CandleArchitecture::Llama)
            .build()
    });
    let provider = HuggingFaceCandleProvider::new(config)
        .map_err(|e| format!("Failed to create Candle provider: {e}"))?;
    Ok(RouterMix::new()
        .primary(provider, named(repo_id))
        .build())
}

/// Agent builder for [`candle_llama_router`], wired to `session_id`.
///
/// # Errors
/// Returns an error string if the Candle provider cannot be constructed.
#[cfg(feature = "candle")]
#[cfg(all(feature = "candle", not(target_family = "wasm")))]
pub fn candle_llama_session<D, M>(
    session_id: SessionId,
    repo_id: &str,
    config: Option<HuggingFaceCandleConfig>,
) -> Result<AgentSessionBuilder<D, M>, String>
where
    D: DocumentStore + 'static,
    M: MemoryStore + 'static,
{
    Ok(candle_llama_router(repo_id, config)?.into_agent_builder(session_id))
}
