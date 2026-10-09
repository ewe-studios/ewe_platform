//! Provider presets - provides pre-configured model presets for various models.
//!
//! Each model is represented by a zero-sized unit struct with methods to create
//! providers with different quantizations. Pass `None` to use default config,
//! or provide your own `HuggingFaceGGUFConfig` for full control.
//!
//! # Example
//!
//! ```ignore
//! use foundation_ai::harness::providers::Glm52;
//!
//! // Create with default config (Q4_K_M quantization)
//! let provider = Glm52::q4_k_m(None)?;
//!
//! // Create with custom config
//! let config = HuggingFaceGGUFConfig::builder()
//!     .cache_dir("/path/to/cache")
//!     .n_gpu_layers(35)
//!     .build();
//! let provider = Glm52::q4_k_m(Some(config))?;
//! ```

use std::path::PathBuf;

use crate::backends::anthropic_messages_provider::{AnthropicConfig, AnthropicMessagesProvider};
use crate::backends::huggingface_gguf_provider::{HuggingFaceGGUFConfig, HuggingFaceGGUFProvider};
use crate::backends::llamacpp::SpeculativeConfig;
use crate::backends::openai_provider::{OpenAIConfig, OpenAIProvider};
use crate::backends::openai_responses_provider::{ResponsesConfig, ResponsesProvider};
use foundation_auth::{AuthCredential, ConfidentialText};

// ===========================================================================
// Quantization presets
// ===========================================================================

pub const Q3_K_M: &str = "Q3_K_M";
pub const Q4_K_M: &str = "Q4_K_M";
pub const Q5_K_M: &str = "Q5_K_M";
pub const Q8_0: &str = "Q8_0";

// ===========================================================================
// Helper function to create a provider
// ===========================================================================

fn create_provider(
    quantization: &str,
    config: Option<HuggingFaceGGUFConfig>,
) -> Result<HuggingFaceGGUFProvider, String> {
    let config = config.unwrap_or_else(|| {
        HuggingFaceGGUFConfig::builder()
            .default_quantization(quantization)
            .build()
    });
    HuggingFaceGGUFProvider::new(config)
        .map_err(|e| format!("Failed to create provider: {e}"))
}

// ===========================================================================
// MTP / speculative decoding opt-in (spec-51)
// ===========================================================================

/// Enable Multi-Token Prediction (MTP) speculative decoding on a GGUF config.
///
/// Starts from `base` (or the default config) and sets the speculative field.
/// `n_max` is the max draft tokens proposed per target step; `mtp_model` is an
/// optional separate MTP head GGUF (`None` uses the main model's embedded head).
///
/// MTP is **opt-in and capability-gated**: it only takes effect on models that
/// actually ship an MTP head (see each preset's `SUPPORTS_MTP` — currently
/// GLM 5.2, Qwen 3.6, and the Gemma 4 family). Enabling it on a model without
/// an MTP head fails at model load rather than silently doing nothing.
///
/// Pass the result as the *main-model* config to a combo router, e.g.
/// `glm52_gemma_router(Some(with_mtp(None, 4, None)), None)`.
#[must_use]
pub fn with_mtp(
    base: Option<HuggingFaceGGUFConfig>,
    n_max: u32,
    mtp_model: Option<PathBuf>,
) -> HuggingFaceGGUFConfig {
    let mut config = base.unwrap_or_default();
    config.llama_config.speculative = Some(SpeculativeConfig::mtp(mtp_model, n_max));
    config
}

// ===========================================================================
// Model presets with quantization methods
// ===========================================================================

/// GLM 5.2 - Zhipu AI's flagship model, excellent for long-horizon tasks.
pub struct Glm52;

impl Glm52 {
    pub const MODEL_ID: &str = "unsloth/GLM-5.2-GGUF";
    /// GLM 5.2 ships an MTP head — MTP speculative decoding is supported.
    pub const SUPPORTS_MTP: bool = true;

    pub fn q3_k_m(config: Option<HuggingFaceGGUFConfig>) -> Result<HuggingFaceGGUFProvider, String> {
        create_provider(Q3_K_M, config)
    }

    pub fn q4_k_m(config: Option<HuggingFaceGGUFConfig>) -> Result<HuggingFaceGGUFProvider, String> {
        create_provider(Q4_K_M, config)
    }

    pub fn q5_k_m(config: Option<HuggingFaceGGUFConfig>) -> Result<HuggingFaceGGUFProvider, String> {
        create_provider(Q5_K_M, config)
    }

    pub fn q8_0(config: Option<HuggingFaceGGUFConfig>) -> Result<HuggingFaceGGUFProvider, String> {
        create_provider(Q8_0, config)
    }
}

/// Qwen 3.6 35B-A3B - Alibaba's MoE model with excellent reasoning.
pub struct Qwen36;

impl Qwen36 {
    pub const MODEL_ID: &str = "unsloth/Qwen3.6-35B-A3B-GGUF";
    /// Qwen 3.6 ships an MTP head — MTP speculative decoding is supported.
    pub const SUPPORTS_MTP: bool = true;

    pub fn q3_k_m(config: Option<HuggingFaceGGUFConfig>) -> Result<HuggingFaceGGUFProvider, String> {
        create_provider(Q3_K_M, config)
    }

    pub fn q4_k_m(config: Option<HuggingFaceGGUFConfig>) -> Result<HuggingFaceGGUFProvider, String> {
        create_provider(Q4_K_M, config)
    }

    pub fn q5_k_m(config: Option<HuggingFaceGGUFConfig>) -> Result<HuggingFaceGGUFProvider, String> {
        create_provider(Q5_K_M, config)
    }

    pub fn q8_0(config: Option<HuggingFaceGGUFConfig>) -> Result<HuggingFaceGGUFProvider, String> {
        create_provider(Q8_0, config)
    }
}

/// Ornith 1.0 35B - Fine-tuned model with excellent instruction following.
pub struct Ornith10;

impl Ornith10 {
    pub const MODEL_ID: &str = "LordNeel/Ornith-1.0-35B-GGUF-llamacpp-tp1";
    /// Ornith 1.0 does not ship an MTP head.
    pub const SUPPORTS_MTP: bool = false;

    pub fn q3_k_m(config: Option<HuggingFaceGGUFConfig>) -> Result<HuggingFaceGGUFProvider, String> {
        create_provider(Q3_K_M, config)
    }

    pub fn q4_k_m(config: Option<HuggingFaceGGUFConfig>) -> Result<HuggingFaceGGUFProvider, String> {
        create_provider(Q4_K_M, config)
    }

    pub fn q5_k_m(config: Option<HuggingFaceGGUFConfig>) -> Result<HuggingFaceGGUFProvider, String> {
        create_provider(Q5_K_M, config)
    }

    pub fn q8_0(config: Option<HuggingFaceGGUFConfig>) -> Result<HuggingFaceGGUFProvider, String> {
        create_provider(Q8_0, config)
    }
}

/// Gemma 4 E4B - Google's small expert model, efficient for memory tasks.
pub struct Gemma4E4b;

impl Gemma4E4b {
    pub const MODEL_ID: &str = "unsloth/gemma-4-E4B-it-GGUF";
    /// Gemma 4 ships an MTP head — MTP speculative decoding is supported.
    pub const SUPPORTS_MTP: bool = true;

    pub fn q3_k_m(config: Option<HuggingFaceGGUFConfig>) -> Result<HuggingFaceGGUFProvider, String> {
        create_provider(Q3_K_M, config)
    }

    pub fn q4_k_m(config: Option<HuggingFaceGGUFConfig>) -> Result<HuggingFaceGGUFProvider, String> {
        create_provider(Q4_K_M, config)
    }

    pub fn q5_k_m(config: Option<HuggingFaceGGUFConfig>) -> Result<HuggingFaceGGUFProvider, String> {
        create_provider(Q5_K_M, config)
    }

    pub fn q8_0(config: Option<HuggingFaceGGUFConfig>) -> Result<HuggingFaceGGUFProvider, String> {
        create_provider(Q8_0, config)
    }
}

/// Gemma 4 26B-A4B - Google's MoE model.
pub struct Gemma4_26b;

impl Gemma4_26b {
    pub const MODEL_ID: &str = "unsloth/gemma-4-26B-A4B-it-GGUF";
    /// Gemma 4 ships an MTP head — MTP speculative decoding is supported.
    pub const SUPPORTS_MTP: bool = true;

    pub fn q3_k_m(config: Option<HuggingFaceGGUFConfig>) -> Result<HuggingFaceGGUFProvider, String> {
        create_provider(Q3_K_M, config)
    }

    pub fn q4_k_m(config: Option<HuggingFaceGGUFConfig>) -> Result<HuggingFaceGGUFProvider, String> {
        create_provider(Q4_K_M, config)
    }

    pub fn q5_k_m(config: Option<HuggingFaceGGUFConfig>) -> Result<HuggingFaceGGUFProvider, String> {
        create_provider(Q5_K_M, config)
    }

    pub fn q8_0(config: Option<HuggingFaceGGUFConfig>) -> Result<HuggingFaceGGUFProvider, String> {
        create_provider(Q8_0, config)
    }
}

/// Gemma 4 E2B - Smallest variant, suitable for resource-constrained
/// environments and as a fast, cheap memory/reflection model.
///
/// NOTE: This points at `unsloth/gemma-4-E2B-it-GGUF` rather than the
/// `ggml-org` mirror because the unsloth repo ships the full K-quant ladder
/// (`Q3_K_M`/`Q4_K_M`/`Q5_K_M`/`Q8_0`); the `ggml-org` mirror only carries
/// `Q8_0` and `bf16`, so the mid-range quant methods below would fail to
/// resolve a file there.
pub struct Gemma4E2b;

impl Gemma4E2b {
    pub const MODEL_ID: &str = "unsloth/gemma-4-E2B-it-GGUF";
    /// Gemma 4 ships an MTP head — MTP speculative decoding is supported.
    pub const SUPPORTS_MTP: bool = true;

    pub fn q3_k_m(config: Option<HuggingFaceGGUFConfig>) -> Result<HuggingFaceGGUFProvider, String> {
        create_provider(Q3_K_M, config)
    }

    pub fn q4_k_m(config: Option<HuggingFaceGGUFConfig>) -> Result<HuggingFaceGGUFProvider, String> {
        create_provider(Q4_K_M, config)
    }

    pub fn q5_k_m(config: Option<HuggingFaceGGUFConfig>) -> Result<HuggingFaceGGUFProvider, String> {
        create_provider(Q5_K_M, config)
    }

    pub fn q8_0(config: Option<HuggingFaceGGUFConfig>) -> Result<HuggingFaceGGUFProvider, String> {
        create_provider(Q8_0, config)
    }
}

// ===========================================================================
// Cloud provider presets
// ===========================================================================

// ===========================================================================
// Cloud model-id presets
// ===========================================================================
//
// Cloud providers serve a model by the id string passed at generation time
// (`ModelId::Name`), not by anything baked into the provider. These constants
// give callers the canonical ids so a router rule and the agent's
// `primary_model`/`memory_model` line up.

/// Claude Opus 4.8 — strongest reasoning, the default "main" cloud model.
pub const CLAUDE_OPUS: &str = "claude-opus-4-8";
/// Claude Sonnet 4.6 — fast + cheap, a good cloud memory/reflection model.
pub const CLAUDE_SONNET: &str = "claude-sonnet-4-6";
/// OpenAI GPT-4o — the default "main" OpenAI model.
pub const OPENAI_GPT4O: &str = "gpt-4o";
/// OpenAI GPT-4o-mini — fast + cheap, a good OpenAI memory/reflection model.
pub const OPENAI_GPT4O_MINI: &str = "gpt-4o-mini";

/// Cloud provider presets - pre-configured providers for cloud models.
pub struct CloudPresets;

impl CloudPresets {
    /// Create an Anthropic Claude Opus provider.
    pub fn claude_opus(api_key: &str) -> Result<AnthropicMessagesProvider, String> {
        let config = AnthropicConfig::new()
            .with_auth(AuthCredential::SecretOnly(ConfidentialText::new(api_key.to_string())));
        Ok(AnthropicMessagesProvider::with_config(config))
    }

    /// Create an Anthropic Claude Sonnet provider.
    pub fn claude_sonnet(api_key: &str) -> Result<AnthropicMessagesProvider, String> {
        let config = AnthropicConfig::new()
            .with_auth(AuthCredential::SecretOnly(ConfidentialText::new(api_key.to_string())));
        Ok(AnthropicMessagesProvider::with_config(config))
    }

    /// Create an OpenAI GPT-4 provider (Chat Completions API).
    pub fn openai_gpt4(api_key: &str) -> Result<OpenAIProvider, String> {
        let config = OpenAIConfig::new()
            .with_auth(AuthCredential::SecretOnly(ConfidentialText::new(api_key.to_string())));
        Ok(OpenAIProvider::with_config(config))
    }

    /// Create an OpenAI GPT-4o provider (Chat Completions API).
    pub fn openai_gpt4o(api_key: &str) -> Result<OpenAIProvider, String> {
        let config = OpenAIConfig::new()
            .with_auth(AuthCredential::SecretOnly(ConfidentialText::new(api_key.to_string())));
        Ok(OpenAIProvider::with_config(config))
    }

    /// Create an OpenAI provider that speaks the **Responses API** (rather than
    /// Chat Completions). Use this for the newer `/v1/responses` surface.
    pub fn openai_responses(api_key: &str) -> Result<ResponsesProvider, String> {
        let config = ResponsesConfig::new()
            .with_auth(AuthCredential::SecretOnly(ConfidentialText::new(api_key.to_string())));
        Ok(ResponsesProvider::with_config(config))
    }
}
