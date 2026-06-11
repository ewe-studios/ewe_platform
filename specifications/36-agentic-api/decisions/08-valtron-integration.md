# Decision 08: Valtron Task Integration Strategy

**Status:** Proposed  
**Date:** 2026-06-11  
**Context:** Specification 36 — Agentic API for foundation_ai

## Problem

The agentic loop has multiple concurrent concerns (message handling, memory management, tool execution, embedding generation, queue processing). These need to run concurrently, communicate with each other, and integrate cleanly with valtron's progress-driven execution model.

## Decision

**"Yes, we valtron it all."** Every major component runs as a valtron task or is backed by one. Components communicate via `Arc`-shared state and valtron's execution primitives (broadcast, sequenced, lift).

### Task Mapping

| Component | Valtron Task Role | Communication | Execution Mode |
|-----------|------------------|---------------|----------------|
| **Agent Loop** | Primary task — orchestrates the entire session | Reads from all components via `Arc` | `execute()` — main loop |
| **Message Flush Task** | Background task — drains write buffer to disk | `ConcurrentQueue` from Message API | `broadcast` — runs in parallel |
| **ToolCallManager** | Background task — executes tool calls | `ConcurrentQueue` for requests/results | `broadcast` for parallel, `sequenced` for dependent |
| **Embedding Generation** | Background task — generates embeddings | `ConcurrentQueue` for requests, channel for responses | `broadcast` — independent requests |
| **Observation Generator** | Triggered task — generates observations when threshold exceeded | Subscribes to Message API broadcaster | `lift` — priority when triggered |
| **Reflection Generator** | Triggered task — generates reflections when observation threshold exceeded | Triggered by Observation Generator | `lift` — priority when triggered |
| **Loop Detector** | Background task — monitors agent output for repetition | Reads agent output stream | `sequenced` — runs alongside agent loop |

### Architecture Diagram

```
┌─────────────────────────────────────────────────────────────────┐
│                    Agent Loop (Primary Task)                     │
│                                                                  │
│  prompt("Fix the bug")                                           │
│  └─ runLoop()                                                    │
│     ├─ Read: MessageStore (Arc)                                 │
│     ├─ Read: ContextProvider (Arc)                              │
│     ├─ Write: MessageStore.append()                             │
│     ├─ Write: ToolCallManager.submit()                          │
│     ├─ Read: ToolCallManager.get_results()                      │
│     ├─ Check: PriorityQueue                                     │
│     ├─ Check: FollowUpQueue                                     │
│     └─ emit: Stream<AgentEvent> (progress states)               │
└──────────────────────────┬──────────────────────────────────────┘
                           │
          ┌────────────────┼────────────────┐
          ▼                ▼                ▼
┌─────────────────┐ ┌──────────────┐ ┌─────────────────┐
│ Message Flush   │ │ ToolCall     │ │ Embedding       │
│ Task (broadcast)│ │ Manager      │ │ Generation      │
│                 │ │ (broadcast)  │ │ (broadcast)     │
│ Drains queue →  │ │ Executes     │ │ Generates       │
│ persists to disk│ │ tool calls   │ │ embeddings      │
└─────────────────┘ └──────────────┘ └─────────────────┘
          │                │                │
          ▼                ▼                ▼
┌─────────────────┐ ┌──────────────┐ ┌─────────────────┐
│ Observation     │ │ Loop         │ │ Priority /      │
│ Generator (lift)│ │ Detector     │ │ FollowUp Queue  │
│                 │ │ (sequenced)  │ │                 │
│ Threshold check →│ │ Detects      │ │ User steering   │
│ generates obs   │ │ repetition   │ │ messages        │
└─────────────────┘ └──────────────┘ └─────────────────┘
```

### Why Valtron for Everything?

**Progress-driven execution matches agent loop semantics:**
- The agent loop is fundamentally an iterator — yield progress, check for interruption, continue
- Valtron's `TaskStatus::Pending`, `Init`, `Ready`, `Ignore` map directly to agent states:
  - `Pending` → "LLM is generating response" / "tool call in progress"
  - `Init` → "setting up context, loading memory"
  - `Ready` → "here's the LLM response" / "tool call result"
  - `Ignore` → "checking queues, nothing to report"

**Composability:**
- Valtron's `execute()`, `broadcast`, `sequenced`, `lift` primitives provide the exact concurrency patterns needed
- `broadcast` → parallel tool calls, parallel embedding requests
- `sequenced` → dependent tool calls, loop detector running alongside agent
- `lift` → priority tasks (observation/reflection generation, user interruption)

**Observability:**
- Every valtron task reports its state — the agent loop can observe progress
- `Stream::Pending` carries progress information (e.g., "3 of 5 tool calls complete")
- `Stream::Delayed` can communicate estimated time remaining

**WASM compatibility:**
- Valtron works in WASM environments (single-threaded executor)
- All components designed with `Arc`-shared state work in both multi-threaded and single-threaded modes

### Shared State Pattern

All components use `Arc<T>` for shared state, with interior mutability where needed:

```rust
pub struct AgentSession {
    pub session_id: SessionId,
    pub message_store: Arc<MessageInner>,
    pub context_provider: Arc<ContextProvider>,
    pub toolcall_manager: Arc<ToolCallManagerInner>,
    pub priority_queue: Arc<PriorityQueue>,
    pub followup_queue: Arc<FollowUpQueue>,
    pub embedding_provider: Arc<EmbeddingProviderInner>,
}
```

This pattern:
- Avoids `&mut self` — compatible with valtron's `&self` execution model
- Cheap to clone — `Arc::clone` is atomic increment, not deep copy
- Thread-safe — works in both multi-threaded and single-threaded valtron modes

### Agent Loop as TaskIterator

The agent loop itself is a `TaskIterator`:

```rust
pub struct AgentLoop {
    session: Arc<AgentSession>,
    state: AgentLoopState,
}

pub enum AgentLoopState {
    Initializing,
    Ready { messages: Vec<Message> },
    Generating { model: String },
    ToolCalls { calls: Vec<ToolCallRequest> },
    Executing { plan: ExecutionPlan },
    Complete { final_messages: Vec<Message> },
}

impl TaskIterator for AgentLoop {
    type Ready = AgentEvent;
    type Pending = AgentProgress;
    type Spawner = BoxedSendExecutionAction;

    fn next_status(&mut self) -> Option<TaskStatus<Self::Ready, Self::Pending, Self::Spawner>> {
        match &self.state {
            AgentLoopState::Initializing => {
                self.state = AgentLoopState::Ready { messages: self.load_context() };
                Some(TaskStatus::Init)
            }
            AgentLoopState::Ready { messages } => {
                // Stream LLM response
                Some(TaskStatus::Pending(AgentProgress::Generating {
                    model: "claude-sonnet-4-6".to_string(),
                    message_count: messages.len(),
                }))
            }
            AgentLoopState::ToolCalls { calls } => {
                // Submit tool calls to ToolCallManager
                self.session.toolcall_manager.submit(calls.clone());
                self.state = AgentLoopState::Executing {
                    plan: ExecutionPlan::Parallel(calls.clone()),
                };
                Some(TaskStatus::Pending(AgentProgress::Executing {
                    tool_calls: calls.len(),
                    completed: 0,
                }))
            }
            AgentLoopState::Executing { plan } => {
                // Check tool call results
                if self.session.toolcall_manager.is_complete() {
                    let results = self.session.toolcall_manager.get_results();
                    // Process results, continue loop or complete
                    Some(TaskStatus::Ready(AgentEvent::ToolResults(results)))
                } else {
                    Some(TaskStatus::Pending(AgentProgress::Executing {
                        tool_calls: plan.len(),
                        completed: self.session.toolcall_manager.completed_count(),
                    }))
                }
            }
            AgentLoopState::Complete { final_messages } => {
                Some(TaskStatus::Ready(AgentEvent::SessionEnd(
                    final_messages.clone(),
                )))
            }
        }
    }
}
```

This allows the agent loop to be:
- **Scheduled** via `execute()` — runs on the valtron pool
- **Observed** — progress is visible via `Stream::Pending(AgentProgress)`
- **Interrupted** — `map_circuit` can short-circuit on interruption signals
- **Composed** — multiple agent loops can run in parallel via `execute_collect_all`

### Event Streaming

The agent loop emits events via the valtron `Stream` protocol:

```rust
pub enum AgentEvent {
    SessionStart { session_id: SessionId },
    TurnStart { turn: usize },
    MessageStart { message_type: MessageType },
    MessageUpdate { content: String },
    MessageEnd { message_type: MessageType },
    ToolCallStart { tool_call: ToolCallRequest },
    ToolCallUpdate { progress: String },
    ToolCallEnd { result: ToolCallResult },
    ObservationGenerated { observation_count: usize },
    ReflectionGenerated { reflection_count: usize },
    SessionEnd { message_count: usize },
}

pub enum AgentProgress {
    Generating { model: String, message_count: usize },
    Executing { tool_calls: usize, completed: usize },
    Flushing { message_count: usize },
    Observing { token_count: usize },
    Reflecting { observation_count: usize },
}
```

These map directly to `Stream<AgentEvent, AgentProgress>`:
- `Stream::Next(AgentEvent)` — discrete events
- `Stream::Pending(AgentProgress)` — progress information
- `Stream::Ignore` — intermediate state (e.g., checking queues)

## Rationale

**Why not use async/await directly for the agent loop?**  
- Valtron's progress-driven model provides more granular observability
- `TaskStatus::Pending` carries progress information that `async` futures cannot
- Valtron works in WASM environments where `async` executors may not be available
- Composition is cleaner — valtron combinators shape the task before execution

**Why `Arc`-shared state instead of message passing?**  
- `Arc` is cheaper than cloning data between tasks
- Message passing adds latency — `Arc` access is memory-speed
- Interior mutability (ConcurrentQueue, Notifier) handles the communication needs
- `Arc` works in both multi-threaded and single-threaded valtron modes

**Why embed the agent loop as a TaskIterator?**  
- TaskIterator is the natural abstraction for progress-driven iteration
- The agent loop is fundamentally an iterator over LLM turns
- TaskStatus variants map to agent states (generating, executing, complete)
- Valtron's execution model handles scheduling, cancellation, and composition

## Alternatives Considered

### Pure async/await agent loop
- **Pros:** Familiar pattern, integrates with existing async ecosystems
- **Cons:** Progress reporting is limited (no intermediate states), harder to compose
- **Supported as a bridge:** The stream-to-future bridge (Decision 09) allows valtron streams to be `.await`ed in async contexts

### Actor model (channels between components)
- **Pros:** Clean isolation, no shared mutable state
- **Cons:** Higher latency, more allocations, harder to observe intermediate state
- **Rejected because:** `Arc` + interior mutability provides sufficient isolation with better performance

### Single-threaded synchronous agent loop
- **Pros:** Simplest, no concurrency concerns
- **Cons:** Blocks during LLM generation and tool execution, no interruption handling
- **Rejected because:** Agent loop must be responsive to user steering — requires concurrent execution
