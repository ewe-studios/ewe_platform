//! Core definition for what models entail

pub mod agentic;
pub mod base_types;
pub mod routable_provider;

// Re-export base types for backward compatibility
pub use base_types::*;

pub use routable_provider::{
    PreloadedProvider, ProviderRouter, ProviderRouterBuilder, RoutableProvider, RoutableProviderBox,
    RouterError, RoutingRule,
};

pub use agentic::{
    AgenticError, MemoryFact, ObservationEntry, ObservationKind, ReflectionEntry, SessionId,
    SessionRecord, TimeRange, TokenSnapshot,
};
