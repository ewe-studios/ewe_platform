//! Agentic API layer (spec-36) — the orchestration substrate built on the
//! `foundation_ai` provider stack.
//!
//! Feature-by-feature this module grows the error taxonomy (F02), the stream
//! contract (F03), token accounting (F04), and the agentic loop (F19+).

pub mod access;
pub mod errors;
pub mod loop_detection;
pub mod memory_coordinator;
pub mod memory_store;
pub mod message_api;
pub mod progress;
pub mod serialization;
pub mod steering;
pub mod token_ledger;
pub mod tool_impl;

pub use access::{AllowAllAccess, SessionAccessProvider, TokenBudget};
pub use loop_detection::{
    Escalation, LoopDetector, LoopDetectorConfig, ToolCallSignature,
    LoopDetection as InlineLoopDetection,
};
pub use errors::{
    AgentAction, AgenticError, AuthError, CircuitBreaker, ErrorPolicy, GenKind, GenerationFailure,
    LoopDetection, UserId,
};
pub use memory_coordinator::MemoryCoordinator;
pub use memory_store::{KvMemoryStore, MemoryStore, MemoryTier, SessionMemory};
pub use message_api::{MessageApi, MessageEvent, Receiver as MessageReceiver};
pub use progress::{lift_model_item, AgentProgress, AgentStream, MemoryKind};
pub use serialization::{from_record_batch, to_record_batch, SerError, SessionRecordRow};
pub use token_ledger::{TokenLedger, TokenSnapshot};
pub use steering::{CancelCode, SteeringQueues};
pub use tool_impl::{
    FailMode, ToolCallManager, ToolCallRequest, ToolCallResult, ToolCallStage, ToolCallWorkflow,
    ToolDefinition, ToolError, ToolErrorKind, ToolImpl, ToolRetryConfig, WorkflowResult,
};
