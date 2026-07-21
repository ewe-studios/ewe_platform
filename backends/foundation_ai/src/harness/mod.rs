//! Harness module - provides pre-configured model providers for easy router creation.
//!
//! This module provides zero-sized unit structs for various models, each with
//! methods to create providers with different quantizations. Use these with
//! [`crate::types::ProviderRouter`] to create pre-configured routers.
//!
//! # Examples
//!
//! The easiest path — a fully-wired agent builder for a model combination,
//! given a session id:
//!
//! ```ignore
//! use foundation_ai::harness;
//!
//! // GLM 5.2 for chat + a small Gemma 4 for memory, ready to customise.
//! let agent = harness::glm52_gemma_session::<MyDocStore, MyMemStore>(
//!         session_id, None, None,
//!     )?
//!     .with_system_prompt("You are a helpful assistant.")
//!     .build()?;
//! ```
//!
//! Or take the router preset and mix/inspect it yourself:
//!
//! ```ignore
//! use foundation_ai::harness::{self, RouterMix, providers::Glm52, providers::Gemma4E2b};
//! use foundation_ai::types::ModelId;
//!
//! // A ready-made mix...
//! let preset = harness::claude_router(api_key)?;
//! let builder = preset.into_agent_builder::<MyDocStore, MyMemStore>(session_id);
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

pub use providers::{
    with_mtp, CloudPresets, Gemma4E2b, Gemma4E4b, Gemma4_26b, Glm52, Ornith10, CLAUDE_OPUS,
    CLAUDE_SONNET, OPENAI_GPT4O, OPENAI_GPT4O_MINI, Q3_K_M, Q4_K_M, Q5_K_M, Q8_0, Qwen36,
};

pub use router::{RouterMix, RouterPreset};
pub use tools::ToolPreset;

#[cfg(feature = "candle")]
pub use agents::{candle_llama_router, candle_llama_session};
pub use agents::{
    claude_router, claude_session, gemma_router, gemma_session, glm52_gemma_router,
    glm52_gemma_session, openai_chat_router, openai_chat_session, openai_responses_router,
    openai_responses_session, qwen36_gemma_router, qwen36_gemma_session,
};
