//! Agentic API layer (spec-36) — the orchestration substrate built on the
//! `foundation_ai` provider stack.
//!
//! Feature-by-feature this module grows the error taxonomy (F02), the stream
//! contract (F03), token accounting (F04), and the agentic loop (F19+).

pub mod access;
pub mod agent_loop;
pub mod context;
pub mod errors;
pub mod loop_detection;
pub mod memory;
pub mod memory_coordinator;
pub mod memory_store;
pub mod message_api;
pub mod progress;
pub mod serialization;
pub mod session;
pub mod steering;
#[cfg(any(test, feature = "testing"))]
pub mod testing;
pub mod token_ledger;
pub mod tool_impl;
pub mod tools;

pub use access::{AllowAllAccess, SessionAccessProvider, TokenBudget};
pub use agent_loop::{AgentConfig, AgentLoop, AgentLoopState};
pub use context::{AgentContext, ContextConfig, ContextProvider, KnowledgeHit, SearchMode};
pub use errors::{
    AgentAction, AgenticError, AuthError, CircuitBreaker, ErrorPolicy, GenKind, GenerationFailure,
    LoopDetection, UserId,
};
pub use loop_detection::{
    Escalation, LoopDetection as InlineLoopDetection, LoopDetector, LoopDetectorConfig,
    ToolCallSignature,
};
pub use memory::{MemoryAction, MemoryConfig, MemoryHierarchy, MemoryParseStrategy};
pub use memory_coordinator::MemoryCoordinator;
pub use memory_store::{KvMemoryStore, MemoryStore, MemoryTier, SessionMemory};
pub use message_api::{MessageApi, MessageEvent, Receiver as MessageReceiver};
pub use progress::{lift_model_item, AgentProgress, AgentStream, MemoryKind};
pub use serialization::{from_record_batch, to_record_batch, SerError, SessionRecordRow};
pub use session::{AgentSession, AgentSessionBuilder};
pub use steering::{CancelCode, SteeringQueues};
pub use token_ledger::{TokenLedger, TokenSnapshot};
pub use tool_impl::{
    FailMode, ToolCallManager, ToolCallRequest, ToolCallResult, ToolCallStage, ToolCallWorkflow,
    ToolDefinition, ToolError, ToolErrorKind, ToolImpl, ToolRetryConfig, WorkflowResult,
};
pub use tools::search::{
    FileMatch, FileSearch, FileSearchKind, SearchContextTool, SearchFileTool, VfsSearchBackend,
};
pub use foundation_nativeapis::{VfsSearchKind, VfsSearchMatch, VfsSearcher};
