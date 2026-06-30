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

use crate::backends::anthropic_messages_provider::{AnthropicConfig, AnthropicMessagesProvider};
use crate::backends::huggingface_gguf_provider::{HuggingFaceGGUFConfig, HuggingFaceGGUFProvider};
use crate::backends::openai_provider::{OpenAIConfig, OpenAIProvider};
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
// Model presets with quantization methods
// ===========================================================================

/// GLM 5.2 - Zhipu AI's flagship model, excellent for long-horizon tasks.
pub struct Glm52;

impl Glm52 {
    pub const MODEL_ID: &str = "unsloth/GLM-5.2-GGUF";

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

/// Gemma 4 E2B - Smallest variant, suitable for resource-constrained environments.
pub struct Gemma4E2b;

impl Gemma4E2b {
    pub const MODEL_ID: &str = "ggml-org/gemma-4-E2B-it-GGUF";

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

    /// Create an OpenAI GPT-4 provider.
    pub fn openai_gpt4(api_key: &str) -> Result<OpenAIProvider, String> {
        let config = OpenAIConfig::new()
            .with_auth(AuthCredential::SecretOnly(ConfidentialText::new(api_key.to_string())));
        Ok(OpenAIProvider::with_config(config))
    }

    /// Create an OpenAI GPT-4o provider.
    pub fn openai_gpt4o(api_key: &str) -> Result<OpenAIProvider, String> {
        let config = OpenAIConfig::new()
            .with_auth(AuthCredential::SecretOnly(ConfidentialText::new(api_key.to_string())));
        Ok(OpenAIProvider::with_config(config))
    }
}
