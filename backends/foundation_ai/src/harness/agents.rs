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
//! The local GGUF constructors (0–3) need the `llamacpp` feature and
//! `candle_llama` the `candle` feature; the cloud ones are always available.
//!
//! 0. [`RouterPreset::hugging_face`] — any single `HuggingFace` GGUF repo, with
//!    [`RouterPreset::huggingface_cpu`] / [`RouterPreset::huggingface_gpu`]
//!    forcing the backend. Every local preset below is built on it.
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
//!
//! // One local model on the CPU, default config; the preset's model is the
//! // session's model, so there is no `with_model`.
//! let preset = RouterPreset::huggingface_cpu("unsloth/gemma-4-E2B-it-GGUF", None)?;
//! let agent = AgentSession::builder(preset).build()?;
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
use super::providers::{gguf_provider, Gemma4E2b, Gemma4_26b, Glm52, Qwen36};
use super::router::{RouterMix, RouterPreset};

/// `ModelId::Name` from a model id string, default quantization.
fn named(id: &str) -> ModelId {
    ModelId::Name(id.to_string(), None)
}

impl RouterPreset {
    // =======================================================================
    // Local GGUF models — `config` overrides the GGUF provider config (cache
    // dir, backend, GPU layers, quantization); `None` takes
    // `HuggingFaceGGUFConfig::default()` (CPU, `q4_k_m`, the `HuggingFace`
    // cache dir).
    // =======================================================================

    /// One `HuggingFace` GGUF model as the session's primary model.
    ///
    /// `model` is the repo id (`"unsloth/gemma-4-E2B-it-GGUF"`, or a family's
    /// `MODEL_ID` such as [`Gemma4E2b::MODEL_ID`]); it is both the model the
    /// GGUF provider downloads and the preset's
    /// [`primary_model`](RouterPreset::primary_model), so
    /// `AgentSession::builder(preset)` needs no `with_model`. `config` is used
    /// as given (`None` takes [`HuggingFaceGGUFConfig::default()`]); to force
    /// the backend use [`huggingface_cpu`](Self::huggingface_cpu) /
    /// [`huggingface_gpu`](Self::huggingface_gpu).
    ///
    /// Constructing the preset downloads nothing; the GGUF file is fetched into
    /// the cache dir on the first turn.
    ///
    /// # Errors
    /// Returns [`AgenticError::Provider`] if the GGUF provider cannot be
    /// constructed.
    #[cfg(all(feature = "llamacpp", not(target_family = "wasm")))]
    pub fn hugging_face(
        model: impl Into<ModelId>,
        config: Option<HuggingFaceGGUFConfig>,
    ) -> Result<Self, ErrorTrace<AgenticError>> {
        Self::hugging_face_mix((model.into(), config), None)
    }

    /// [`hugging_face`](Self::hugging_face) on the CPU: `config` (or the
    /// default) with [`HuggingFaceGGUFConfig::on_cpu`] applied — the
    /// `LLamaCPU` backend, no layers offloaded.
    ///
    /// # Errors
    /// Returns [`AgenticError::Provider`] if the GGUF provider cannot be
    /// constructed.
    #[cfg(all(feature = "llamacpp", not(target_family = "wasm")))]
    pub fn huggingface_cpu(
        model: impl Into<ModelId>,
        config: Option<HuggingFaceGGUFConfig>,
    ) -> Result<Self, ErrorTrace<AgenticError>> {
        Self::hugging_face(model, Some(config.unwrap_or_default().on_cpu()))
    }

    /// [`hugging_face`](Self::hugging_face) on the GPU: `config` (or the
    /// default) with [`HuggingFaceGGUFConfig::on_gpu`] applied — the platform
    /// GPU backend (`LLamaMetal` on Apple targets, `LLamaGPU` elsewhere) with
    /// every layer offloaded. Offload needs a build with a GPU feature
    /// (`metal`, `cuda`, `vulkan`); see `on_gpu`.
    ///
    /// # Errors
    /// Returns [`AgenticError::Provider`] if the GGUF provider cannot be
    /// constructed.
    #[cfg(all(feature = "llamacpp", not(target_family = "wasm")))]
    pub fn huggingface_gpu(
        model: impl Into<ModelId>,
        config: Option<HuggingFaceGGUFConfig>,
    ) -> Result<Self, ErrorTrace<AgenticError>> {
        Self::hugging_face(model, Some(config.unwrap_or_default().on_gpu()))
    }

    /// The shared base of every local preset: a GGUF primary model and an
    /// optional GGUF memory model, each `(repo id, config)`, each built through
    /// [`gguf_provider`].
    #[cfg(all(feature = "llamacpp", not(target_family = "wasm")))]
    fn hugging_face_mix(
        primary: (ModelId, Option<HuggingFaceGGUFConfig>),
        memory: Option<(ModelId, Option<HuggingFaceGGUFConfig>)>,
    ) -> Result<Self, ErrorTrace<AgenticError>> {
        let (primary_model, primary_config) = primary;
        let mut mix = RouterMix::new().primary(gguf_provider(primary_config)?, primary_model);
        if let Some((memory_model, memory_config)) = memory {
            mix = mix.memory(gguf_provider(memory_config)?, memory_model);
        }
        Ok(mix.build())
    }

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
        Self::hugging_face_mix(
            (named(Glm52::MODEL_ID), main),
            Some((named(Gemma4E2b::MODEL_ID), memory)),
        )
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
        Self::hugging_face_mix(
            (named(Qwen36::MODEL_ID), main),
            Some((named(Gemma4E2b::MODEL_ID), memory)),
        )
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
        Self::hugging_face_mix(
            (named(Gemma4_26b::MODEL_ID), main),
            Some((named(Gemma4E2b::MODEL_ID), memory)),
        )
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
