//! Agentic API layer (spec-36) — the orchestration substrate built on the
//! `foundation_ai` provider stack.
//!
//! Feature-by-feature this module grows the error taxonomy (F02), the stream
//! contract (F03), token accounting (F04), and the agentic loop (F19+).

pub mod errors;
pub mod progress;
pub mod token_ledger;

pub use errors::{
    AgentAction, AgenticError, AuthError, CircuitBreaker, ErrorPolicy, GenKind, GenerationFailure,
    LoopDetection, UserId,
};
pub use progress::{lift_model_item, AgentProgress, AgentStream, MemoryKind};
pub use token_ledger::{TokenLedger, TokenSnapshot};
