//! Harness module - provides pre-configured agent sessions for common model setups.
//!
//! This module provides convenience functions for creating [`AgentSession`](crate::agentic::session::AgentSession)
//! instances with sensible defaults for various models and configurations.
//!
//! # Quick Start
//!
//! ```ignore
//! use foundation_ai::harness::SessionHarness;
//!
//! // Create a session with a specific model
//! let session = SessionHarness::new()
//!     .with_model("claude-sonnet-4-6")
//!     .with_api_key("your-api-key")
//!     .build()
//!     .await?;
//!
//! // Or use a pre-configured preset
//! let session = SessionHarness::claude_opus("your-api-key").build().await?;
//! ```
//!
//! # Available Presets
//!
//! The harness provides several preset configurations for common models:
//!
//! - **Claude Models**: Claude Opus, Claude Sonnet, Claude Haiku
//! - **OpenAI Models**: GPT-4, GPT-4o, GPT-4o mini, o1, o3
//! - **Local Models**: GGUF models from HuggingFace for local inference
//!
//! Each preset configures the appropriate provider, model ID, and sensible defaults
//! for temperature, max tokens, and other parameters.

pub mod providers;
pub mod session;

pub use providers::{
    CloudPresets, Gemma4E2b, Gemma4E4b, Gemma4_26b, Glm52, Ornith10, Q3_K_M, Q4_K_M, Q5_K_M, Q8_0,
    Qwen36,
};
pub use session::SessionHarness;
