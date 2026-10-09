//! Agent presets — one-call routers for the common model combinations.
//!
//! WHY: The whole point of the harness is "super easy to use": a caller who
//! just wants "GLM 5.2 for chat with a small Gemma for memory" should not have
//! to know about [`RouterMix`], routing rules, or which quantization a repo
//! ships. These constructors encode sensible defaults while still returning a
//! value the caller can inspect and customise.
//!
//! WHAT: one constructor on [`RouterPreset`] per combination, each returning
//! the router plus the model ids that drive the agent:
//! The local GGUF combinations (1–3) need the `llamacpp` feature and
//! `candle_llama` the `candle` feature; the cloud ones are always available.
//!
//! 1. [`RouterPreset::glm52_gemma`] — GLM 5.2 main + Gemma 4 E2B memory.
//! 2. [`RouterPreset::qwen36_gemma`] — Qwen 3.6 main + Gemma 4 E2B memory.
//! 3. [`RouterPreset::gemma`] — Gemma 4 26B main + Gemma 4 E2B memory
//!    (Gemma for both roles, big + small).
//! 4. [`RouterPreset::claude`] — Claude Opus main + Claude Sonnet memory
//!    (Anthropic).
//! 5. [`RouterPreset::openai_chat`] — GPT-4o main + GPT-4o-mini memory (Chat
//!    Completions API).
//! 6. [`RouterPreset::openai_responses`] — GPT-4o main + GPT-4o-mini memory
//!    (Responses API).
//! 7. [`RouterPreset::candle_llama`] — a single Candle (safetensors) model.
//!    Candle currently only implements the **Llama** architecture; other
//!    architectures error as unsupported.
//!
//! HOW: each constructor builds the concrete providers (defaulting to
//! `Q4_K_M` for local GGUF), feeds them to [`RouterMix`], and returns the
//! preset. [`RouterPreset::into_agent_builder`] bridges to an
//! `AgentSessionBuilder` with the models already wired:
//!
//! ```ignore
//! let agent = RouterPreset::claude(&key)?
//!     .into_agent_builder()
//!     .with_session_id(session_id)
//!     .build()?;
//! ```

use foundation_errstacks::ErrorTrace;

use crate::agentic::AgenticError;
#[cfg(all(feature = "candle", not(target_family = "wasm")))]
use crate::backends::candle::CandleArchitecture;
#[cfg(all(feature = "candle", not(target_family = "wasm")))]
use crate::backends::huggingface_candle_provider::{
    HuggingFaceCandleConfig, HuggingFaceCandleProvider,
};
#[cfg(all(feature = "llamacpp", not(target_family = "wasm")))]
use crate::backends::huggingface_gguf_provider::HuggingFaceGGUFConfig;
use crate::types::ModelId;

use super::providers::{CloudPresets, CLAUDE_OPUS, CLAUDE_SONNET, OPENAI_GPT4O, OPENAI_GPT4O_MINI};
#[cfg(all(feature = "llamacpp", not(target_family = "wasm")))]
use super::providers::{Gemma4E2b, Gemma4_26b, Glm52, Qwen36};
use super::router::{RouterMix, RouterPreset};

/// `ModelId::Name` from a model id string, default quantization.
fn named(id: &str) -> ModelId {
    ModelId::Name(id.to_string(), None)
}

impl RouterPreset {
    // =======================================================================
    // Local GGUF combinations — `main` / `memory` override the GGUF provider
    // config (cache dir, GPU layers, quantization); `None` takes the `Q4_K_M`
    // defaults.
    // =======================================================================

    /// GLM 5.2 as the main model, Gemma 4 E2B as the small memory model.
    ///
    /// # Errors
    /// Returns [`AgenticError::Provider`] if either GGUF provider cannot be
    /// constructed.
    #[cfg(all(feature = "llamacpp", not(target_family = "wasm")))]
    pub fn glm52_gemma(
        main: Option<HuggingFaceGGUFConfig>,
        memory: Option<HuggingFaceGGUFConfig>,
    ) -> Result<Self, ErrorTrace<AgenticError>> {
        let main_provider = Glm52::q4_k_m(main)?;
        let memory_provider = Gemma4E2b::q4_k_m(memory)?;
        Ok(RouterMix::new()
            .primary(main_provider, named(Glm52::MODEL_ID))
            .memory(memory_provider, named(Gemma4E2b::MODEL_ID))
            .build())
    }

    /// Qwen 3.6 35B-A3B as the main model, Gemma 4 E2B as the small memory
    /// model.
    ///
    /// # Errors
    /// Returns [`AgenticError::Provider`] if either GGUF provider cannot be
    /// constructed.
    #[cfg(all(feature = "llamacpp", not(target_family = "wasm")))]
    pub fn qwen36_gemma(
        main: Option<HuggingFaceGGUFConfig>,
        memory: Option<HuggingFaceGGUFConfig>,
    ) -> Result<Self, ErrorTrace<AgenticError>> {
        let main_provider = Qwen36::q4_k_m(main)?;
        let memory_provider = Gemma4E2b::q4_k_m(memory)?;
        Ok(RouterMix::new()
            .primary(main_provider, named(Qwen36::MODEL_ID))
            .memory(memory_provider, named(Gemma4E2b::MODEL_ID))
            .build())
    }

    /// Gemma 4 26B-A4B as the big main model, Gemma 4 E2B as the small memory
    /// model — Gemma on both ends.
    ///
    /// # Errors
    /// Returns [`AgenticError::Provider`] if either GGUF provider cannot be
    /// constructed.
    #[cfg(all(feature = "llamacpp", not(target_family = "wasm")))]
    pub fn gemma(
        main: Option<HuggingFaceGGUFConfig>,
        memory: Option<HuggingFaceGGUFConfig>,
    ) -> Result<Self, ErrorTrace<AgenticError>> {
        let main_provider = Gemma4_26b::q4_k_m(main)?;
        let memory_provider = Gemma4E2b::q4_k_m(memory)?;
        Ok(RouterMix::new()
            .primary(main_provider, named(Gemma4_26b::MODEL_ID))
            .memory(memory_provider, named(Gemma4E2b::MODEL_ID))
            .build())
    }

    // =======================================================================
    // Cloud combinations — each takes the provider's API key.
    // =======================================================================

    /// Claude Opus as the main model, Claude Sonnet as the memory model, both
    /// via the Anthropic Messages provider.
    ///
    /// # Errors
    /// Returns [`AgenticError::Provider`] if a provider cannot be constructed.
    pub fn claude(api_key: &str) -> Result<Self, ErrorTrace<AgenticError>> {
        let main_provider = CloudPresets::claude_opus(api_key)?;
        let memory_provider = CloudPresets::claude_sonnet(api_key)?;
        Ok(RouterMix::new()
            .primary(main_provider, named(CLAUDE_OPUS))
            .memory(memory_provider, named(CLAUDE_SONNET))
            .build())
    }

    /// GPT-4o as the main model, GPT-4o-mini as the memory model, both via the
    /// `OpenAI` **Chat Completions** provider.
    ///
    /// # Errors
    /// Returns [`AgenticError::Provider`] if a provider cannot be constructed.
    pub fn openai_chat(api_key: &str) -> Result<Self, ErrorTrace<AgenticError>> {
        let main_provider = CloudPresets::openai_gpt4o(api_key)?;
        let memory_provider = CloudPresets::openai_gpt4o(api_key)?;
        Ok(RouterMix::new()
            .primary(main_provider, named(OPENAI_GPT4O))
            .memory(memory_provider, named(OPENAI_GPT4O_MINI))
            .build())
    }

    /// GPT-4o as the main model, GPT-4o-mini as the memory model, both via the
    /// `OpenAI` **Responses** provider (`/v1/responses`).
    ///
    /// # Errors
    /// Returns [`AgenticError::Provider`] if a provider cannot be constructed.
    pub fn openai_responses(api_key: &str) -> Result<Self, ErrorTrace<AgenticError>> {
        let main_provider = CloudPresets::openai_responses(api_key)?;
        let memory_provider = CloudPresets::openai_responses(api_key)?;
        Ok(RouterMix::new()
            .primary(main_provider, named(OPENAI_GPT4O))
            .memory(memory_provider, named(OPENAI_GPT4O_MINI))
            .build())
    }

    // =======================================================================
    // Candle (safetensors) — single Llama-architecture model
    // =======================================================================

    /// A single Candle-backed safetensors model as the main model.
    ///
    /// Candle currently implements only the **Llama** architecture
    /// ([`CandleArchitecture::Llama`]); other architectures load-fail as
    /// unsupported. `repo_id` is a `HuggingFace` repo (e.g.
    /// `"meta-llama/Llama-3.2-1B-Instruct"`). Pass a custom
    /// [`HuggingFaceCandleConfig`] to set cache dir / dtype / context length,
    /// or `None` for defaults (Llama architecture).
    ///
    /// # Errors
    /// Returns [`AgenticError::Provider`] if the Candle provider cannot be
    /// constructed.
    #[cfg(all(feature = "candle", not(target_family = "wasm")))]
    pub fn candle_llama(
        repo_id: &str,
        config: Option<HuggingFaceCandleConfig>,
    ) -> Result<Self, ErrorTrace<AgenticError>> {
        let config = config.unwrap_or_else(|| {
            HuggingFaceCandleConfig::builder()
                .architecture(CandleArchitecture::Llama)
                .build()
        });
        let provider = HuggingFaceCandleProvider::new(config).map_err(|e| {
            ErrorTrace::new(AgenticError::Provider(format!(
                "failed to create Candle provider for {repo_id}: {e}"
            )))
        })?;
        Ok(RouterMix::new().primary(provider, named(repo_id)).build())
    }
}
