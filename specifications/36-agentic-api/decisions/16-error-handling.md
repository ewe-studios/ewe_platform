# Decision 16: Error Handling Conventions

**Status:** Proposed  
**Date:** 2026-06-12  
**Context:** Specification 36 — Agentic API for foundation_ai

## Problem

The agentic loop spans multiple components (LLM, tools, memory, queues) that can fail in different ways. Errors need:
- A unified type that wraps all underlying errors
- A propagation mechanism through valtron streams
- Clear ownership of retry/resilience logic

## Decision

Errors are handled via a **unified `AgenticError` enum** propagated through valtron streams as `Stream::Next(Err(e))`. Retry and resilience logic is owned by model tasks internally — the agent loop does not manage retries.

### Error Type Hierarchy

```rust
#[derive(Debug, Display, Error)]
pub enum AgenticError {
    /// LLM generation failed (model error, context overflow, provider error)
    #[display("generation failed: {0}")]
    Generation(#[from] GenerationError),
    
    /// Tool call execution failed
    #[display("tool '{tool_name}' failed: {reason}")]
    ToolCall { tool_name: String, reason: String },
    
    /// Tool not authorized for this user/session
    #[display("tool '{tool_name}' not authorized")]
    ToolNotAuthorized { tool_name: String, user_id: UserId },
    
    /// Message store operation failed
    #[display("message store error: {0}")]
    MessageStore(#[from] StorageError),
    
    /// Memory operation failed
    #[display("memory error: {0}")]
    Memory(#[from] MemoryError),
    
    /// Vector store operation failed
    #[display("vector store error: {0}")]
    VectorStore(#[from] VectorStoreError),
    
    /// Embedding generation failed
    #[display("embedding error: {0}")]
    Embedding(#[from] EmbeddingError),
    
    /// Session operation failed
    #[display("session error: {0}")]
    Session(#[from] SessionError),
    
    /// Queue operation failed
    #[display("queue error: {0}")]
    Queue(#[from] QueueError),
    
    /// Loop detected — agent is repeating itself
    #[display("loop detected: {0}")]
    LoopDetected(LoopDetection),
    
    /// Authentication/authorization failed
    #[display("auth error: {0}")]
    Auth(#[from] AuthError),
    
    /// Unknown/unexpected error
    #[display("unexpected error: {0}")]
    Unexpected(String),
}
```

### Error Propagation

All errors flow through valtron streams as `Stream::Next(Err(AgenticError))`:

```rust
// Agent loop stream
Stream<AgentEvent, AgentProgress>
├── Stream::Next(AgentEvent::MessageEnd { ... })  // normal event
├── Stream::Next(AgentEvent::ToolCallEnd { ... }) // normal event
├── Stream::Next(Err(AgenticError::Generation(...))) // error — agent loop handles
├── Stream::Next(Err(AgenticError::ToolCall { ... })) // error — agent loop handles
└── Stream::Pending(AgentProgress::Generating { ... }) // progress
```

The agent loop receives errors via its valtron iterator:
```rust
for item in agent_loop {
    match item {
        Stream::Next(Ok(event)) => handle_event(event),
        Stream::Next(Err(error)) => handle_error(error),
        Stream::Pending(progress) => report_progress(progress),
        _ => {}
    }
}
```

### Error Ownership

**Model tasks own retry and resilience:**

```
foundation_ai model tasks (own retry logic internally)
├── LLM task
│   ├── Retries on rate limit (exponential backoff, max 3 retries)
│   ├── Retries on network failure (exponential backoff, max 3 retries)
│   ├── Context overflow detection → truncates or reports error
│   └── On permanent failure → returns Stream::Next(Err(AgenticError::Generation(...)))
│
├── Tool execution task
│   ├── Retries on tool timeout (configurable, max 2 retries)
│   ├── Tool panic → catches, returns error result to LLM
│   └── On permanent failure → returns Stream::Next(Err(AgenticError::ToolCall { ... }))
│
└── Embedding task
    ├── Retries on embedding model failure (max 2 retries)
    └── On permanent failure → returns error via channel
```

**Agent loop does NOT manage retries.** It receives the final error after the model task's internal retry logic is exhausted. The agent loop's job is:
1. Log the error
2. Decide whether to continue (e.g., skip the failed tool call and continue with others) or terminate
3. Surface the error to the user if appropriate

### Error Handling in Agent Loop

```rust
fn handle_error(&self, error: AgenticError) -> AgentAction {
    match error {
        // Retriable at agent level — try a different model or reduce context
        AgenticError::Generation(GenerationError::ContextOverflow) => {
            self.reduce_context_and_retry()
        }
        AgenticError::Generation(GenerationError::RateLimit) => {
            // Already retried by model task — switch to fallback model
            self.switch_to_fallback_model()
        }
        
        // Tool errors — report to LLM so it can adapt
        AgenticError::ToolCall { ref tool_name, ref reason } => {
            // Inject error message into conversation
            self.message_store.append(Messages::ToolResult {
                id: tool_name.clone(),
                name: tool_name.clone(),
                content: UserModelContent::Text(format!("Error: {reason}")),
                error_detail: Some(reason.clone()),
                ..
            });
            AgentAction::Continue  // LLM will see the error and adapt
        }
        
        // Auth errors — terminate, user needs to re-authenticate
        AgenticError::Auth(_) | AgenticError::ToolNotAuthorized { .. } => {
            AgentAction::Terminate(error)
        }
        
        // Loop detection — redirect with memory context
        AgenticError::LoopDetected(detection) => {
            self.inject_redirect_from_memory(detection)
        }
        
        // Unexpected errors — terminate with error
        AgenticError::Unexpected(_) => {
            AgentAction::Terminate(error)
        }
    }
}

enum AgentAction {
    Continue,          // Keep going — LLM will see error and adapt
    RetryWithReducedContext,  // Reduce context, retry LLM call
    SwitchModel,       // Try fallback model
    Terminate(AgenticError),  // End session with error
}
```

### Error Surfacing

**Tool errors ALWAYS go back to the model.** The LLM must know whether its tool call succeeded, failed, or produced an error — so it can adapt, retry with different arguments, or report to the user.

| Error Type | Surfaces To | Action |
|-----------|-------------|--------|
| LLM generation error | User (via stream) | Agent terminates or switches model |
| **Tool call error** | **LLM (via ToolResult message)** | **LLM sees error, adapts or retries with different args** |
| Auth error | User (via stream) | Agent terminates |
| Loop detection | Agent (internal redirect) | Agent redirects with memory context |
| Memory/vector error | Agent (logs, continues) | Agent continues without memory |

### ToolCallManager Retry Configuration

The ToolCallManager has built-in retry with exponential backoff for tool calls:

```rust
pub struct ToolRetryConfig {
    /// Maximum number of retry attempts (default: 3)
    pub max_retries: u32,
    /// Initial backoff duration (default: 1s)
    pub initial_backoff: Duration,
    /// Backoff multiplier (default: 2x)
    pub backoff_multiplier: f64,
    /// Maximum backoff duration (default: 30s)
    pub max_backoff: Duration,
    /// Retry only on these error types (default: timeouts, network errors)
    pub retry_on: Vec<ToolErrorKind>,
}

impl Default for ToolRetryConfig {
    fn default() -> Self {
        Self {
            max_retries: 3,
            initial_backoff: Duration::from_secs(1),
            backoff_multiplier: 2.0,
            max_backoff: Duration::from_secs(30),
            retry_on: vec![ToolErrorKind::Timeout, ToolErrorKind::Network],
        }
    }
}

impl ToolCallManager {
    /// Execute a tool call with automatic retry
    fn execute_with_retry(&self, call: ToolCallRequest) -> ToolCallResult {
        let config = self.get_retry_config(&call.name);
        let mut backoff = config.initial_backoff;
        
        for attempt in 0..=config.max_retries {
            match self.execute_tool(&call) {
                Ok(result) => return Ok(result),
                Err(e) => {
                    if attempt == config.max_retries || !config.retry_on.contains(&e.kind()) {
                        // All retries exhausted or non-retriable error
                        // Return error to LLM via ToolResult message
                        return self.return_error_to_llm(&call, e);
                    }
                    // Wait with exponential backoff
                    sleep(backoff);
                    backoff = min(backoff * config.backoff_multiplier, config.max_backoff);
                }
            }
        }
        unreachable!()
    }
}
```

Retry configuration can be customized per tool:
- **File operations** (read, write, edit): retry on I/O errors, 2 retries
- **Network tools** (HTTP, API calls): retry on timeouts/network errors, 3 retries
- **Shell commands**: no retry (side effects may be non-idempotent)
- **Search tools** (fff): retry on index errors, 1 retry

### Existing Error Types (Reused)

The agentic error wraps existing error types from the platform:

| Source | Error Type | Wrapped As |
|--------|-----------|------------|
| foundation_ai | `GenerationError` | `AgenticError::Generation` |
| foundation_ai | `ToolCallingError` | `AgenticError::ToolCall` |
| foundation_db | `StorageError` | `AgenticError::MessageStore` |
| foundation_auth | Auth errors | `AgenticError::Auth` |
| foundation_errstacks | `ErrorTrace<T>` | Converted to `String` in `Unexpected` |

## Rationale

**Why unified `AgenticError`?**
- Single error type for the entire agentic API — callers don't need to handle multiple error types
- `#[from]` conversions make it easy to wrap underlying errors
- Each variant carries the data relevant to that failure mode

**Why errors flow through `Stream::Next(Err(...))`?**
- Consistent with valtron's execution model — errors are just another stream item
- Callers handle errors in the same match block as normal events
- No separate error channel needed

**Why model tasks own retry logic?**
- Retry policies are model-specific (rate limits, timeouts, backoff)
- Agent loop shouldn't care about retry internals — it just sees the final result
- Keeps agent loop simple — flow control, not error recovery

**Why tool errors surface to LLM?**
- LLM can adapt — try a different tool, fix arguments, or report to user
- More resilient than terminating the session on first tool failure

## Alternatives Considered

### Separate error channel (not through stream)
- **Pros:** Errors are distinct from events
- **Cons:** More complex — two channels to monitor
- **Rejected because:** `Stream::Next(Err(...))` is sufficient and consistent

### Agent loop manages retries
- **Pros:** Agent has full control
- **Cons:** Agent loop becomes complex — needs to know about rate limits, backoff, etc.
- **Rejected because:** Model tasks are better positioned to manage their own resilience

### No unified error type (propagate raw errors)
- **Pros:** No wrapper overhead
- **Cons:** Callers need to handle many error types
- **Rejected because:** Unified type simplifies the API
