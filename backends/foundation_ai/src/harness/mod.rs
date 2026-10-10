//! Harness module - provides pre-configured model providers for easy router creation.
//!
//! This module provides zero-sized unit structs for various models, each with
//! methods to create providers with different quantizations. Use these with
//! [`crate::types::ProviderRouter`] to create pre-configured routers.
//!
//! # Examples
//!
//! The easiest path — a preset for a model combination, bridged into a
//! fully-wired agent builder:
//!
//! ```ignore
//! use foundation_ai::harness::RouterPreset;
//!
//! // GLM 5.2 for chat + a small Gemma 4 for memory, ready to customise.
//! let agent = RouterPreset::glm52_gemma(None, None)?
//!     .into_agent_builder()
//!     .with_system_prompt("You are a helpful assistant.")
//!     .build()?;
//! ```
//!
//! Or inspect the preset, or mix your own:
//!
//! ```ignore
//! use foundation_ai::harness::{RouterMix, RouterPreset, providers::Glm52, providers::Gemma4E2b};
//! use foundation_ai::types::ModelId;
//!
//! // A ready-made mix...
//! let preset = RouterPreset::claude(api_key)?;
//! let builder = preset
//!     .into_agent_builder()
//!     .with_session_id(session_id)
//!     .with_doc_store(my_doc_store)
//!     .with_memory_store(my_memory_store);
//!
//! // ...or a fully custom mix:
//! let preset = RouterMix::new()
//!     .primary(Glm52::q4_k_m(None)?, ModelId::Name(Glm52::MODEL_ID.into(), None))
//!     .memory(Gemma4E2b::q4_k_m(None)?, ModelId::Name(Gemma4E2b::MODEL_ID.into(), None))
//!     .build();
//! ```
//!
//! Low-level: build a single-provider router by hand:
//!
//! ```ignore
//! use foundation_ai::harness::CloudPresets;
//! use foundation_ai::types::{ProviderRouter, RoutableProviderBox};
//!
//! let provider = CloudPresets::claude_sonnet(api_key)?;
//! let routable = RoutableProviderBox::new(provider);
//! let router = ProviderRouter::single(Box::new(routable));
//! ```

pub mod agents;
pub mod providers;
pub mod router;
pub mod tools;

#[cfg(all(feature = "llamacpp", not(target_family = "wasm")))]
pub use providers::{with_mtp, Gemma4E2b, Gemma4E4b, Gemma4_26b, Glm52, Ornith10, Qwen36};
pub use providers::{
    CloudPresets, CLAUDE_OPUS, CLAUDE_SONNET, OPENAI_GPT4O, OPENAI_GPT4O_MINI, Q3_K_M, Q4_K_M,
    Q5_K_M, Q8_0,
};

pub use router::{RouterMix, RouterPreset};
pub use tools::ToolPreset;
