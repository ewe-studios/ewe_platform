//! Harness module - provides pre-configured model providers for easy router creation.
//!
//! This module provides zero-sized unit structs for various models, each with
//! methods to create providers with different quantizations. Use these with
//! [`crate::types::ProviderRouter`] to create pre-configured routers.
//!
//! # Example
//!
//! ```ignore
//! use foundation_ai::harness::{Glm52, CloudPresets};
//! use foundation_ai::types::{ProviderRouter, RoutableProviderBox};
//!
//! // Create a router with a local GGUF model
//! let provider = Glm52::q4_k_m(None)?;
//! let routable = RoutableProviderBox::new(provider);
//! let router = ProviderRouter::single(Box::new(routable));
//!
//! // Or use cloud providers
//! let provider = CloudPresets::claude_sonnet(api_key)?;
//! let routable = RoutableProviderBox::new(provider);
//! let router = ProviderRouter::single(Box::new(routable));
//! ```

pub mod providers;

pub use providers::{
    CloudPresets, Gemma4E2b, Gemma4E4b, Gemma4_26b, Glm52, Ornith10, Q3_K_M, Q4_K_M, Q5_K_M, Q8_0,
    Qwen36,
};
