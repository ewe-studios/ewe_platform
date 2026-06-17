//! Agentic API layer (spec-36) — the orchestration substrate built on the
//! `foundation_ai` provider stack.
//!
//! Feature-by-feature this module grows the error taxonomy (F02), the stream
//! contract (F03), token accounting (F04), and the agentic loop (F19+).

pub mod errors;

pub use errors::{
    AgentAction, AgenticError, AuthError, CircuitBreaker, ErrorPolicy, GenKind, GenerationFailure,
    LoopDetection, UserId,
};
