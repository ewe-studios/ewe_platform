//! Agentic API layer (spec-36) — the orchestration substrate built on the
//! `foundation_ai` provider stack.
//!
//! Feature-by-feature this module grows the error taxonomy (F02), the stream
//! contract (F03), token accounting (F04), and the agentic loop (F19+).

pub mod access;
pub mod agent_loop;
pub mod context;
pub mod embedding;
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
pub mod toolshed;
pub mod turn;

// ---------------------------------------------------------------------------
// What an application needs: build a session, run turns, read results, give
// it tools, handle its errors.

pub use crate::harness::ToolPreset;
pub use access::{AllowAllAccess, SessionAccessProvider, TokenBudget};
pub use agent_loop::{AgentConfig, ModelSelection};
pub use context::ContextConfig;
pub use embedding::{CachedEmbeddingProvider, EmbeddingProvider};
pub use errors::{
    AgentAction, AgenticError, AuthError, ErrorPolicy, GenKind, GenerationFailure,
    LoopDetectedInfo, UserId,
};
pub use memory::{MemoryConfig, MemoryParseStrategy};
pub use memory_store::{KvMemoryStore, MemoryStore};
pub use progress::{AgentProgress, MemoryKind};
pub use session::{AgentSession, AgentSessionBuilder};
pub use token_ledger::TokenSnapshot;
pub use tool_impl::{FnTool, ToolArgs, ToolCallResult, ToolError, ToolImpl};
pub use toolshed::{
    tool_fn, ContextSearch, MemoryAccess, SessionParts, ToolConstructor, ToolShed, ToolShedError,
};
pub use turn::{Answer, Turn, TurnEvent, TurnEvents, TurnOutcome, TurnStream, TurnSummary};

/// Loop internals, for custom loops, extensions and tests.
///
/// Everything the session wires together — the loop state machine, context
/// assembly, the memory hierarchy, the tool-call manager, steering queues,
/// token accounting, loop detection, the built-in tool types and the
/// embedding/serialization helpers. Not covered by semver: these change with
/// the loop.
pub mod internals {
    pub use super::agent_loop::{AgentLoop, AgentLoopState};
    pub use super::context::{AgentContext, ContextProvider, KnowledgeHit, SearchMode};
    pub use super::embedding::{
        CacheStats, ColdCache, EmbeddingError, EmbeddingVector, NoopColdCache, SentenceChunker,
        TextChunker, WholeTextChunker,
    };
    pub use super::errors::CircuitBreaker;
    pub use super::loop_detection::{
        is_bare_number, is_vacuous_answer, question_expects_a_number, Escalation, LoopDetection,
        LoopDetector, LoopDetectorConfig, ToolCallSignature,
    };
    pub use super::memory::{MemoryAction, MemoryHierarchy};
    pub use super::memory_coordinator::MemoryCoordinator;
    pub use super::memory_store::{MemoryTier, SessionMemory};
    pub use super::message_api::{MessageApi, MessageEvent, Receiver as MessageReceiver};
    pub use super::progress::{lift_model_item, AgentStream};
    pub use super::serialization::{
        from_record_batch, to_record_batch, SerError, SessionRecordRow,
    };
    pub use super::steering::{CancelCode, SteeringQueues};
    pub use super::token_ledger::TokenLedger;
    pub use super::tool_impl::{
        FailMode, ToolCallManager, ToolCallRequest, ToolCallStage, ToolCallWorkflow, ToolErrorKind,
        ToolRetryConfig, WorkflowResult,
    };
    #[cfg(not(target_family = "wasm"))]
    pub use super::tools::search::{
        FileMatch, FileSearch, FileSearchKind, SearchContextTool, SearchFileTool, VfsSearchBackend,
    };
    pub use super::tools::shed::{
        ShedQuery, ShedResult, ToolDiscovery, ToolSummary, SHED_TOOL_NAME,
    };
    #[cfg(not(target_family = "wasm"))]
    pub use foundation_nativeapis::{VfsSearchKind, VfsSearchMatch, VfsSearcher};
}
