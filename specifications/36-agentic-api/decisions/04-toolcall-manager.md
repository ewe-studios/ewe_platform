# Decision 04: ToolCallManager Design

**Status:** Proposed  
**Date:** 2026-06-11  
**Context:** Specification 36 — Agentic API for foundation_ai

## Problem

Tool calls in an agentic loop need coordinated execution: some can run in parallel, some must run sequentially (dependent on prior results), and some should be batched. Tool call results must be persisted to the Message API before being fed back to the LLM, ensuring the audit trail is complete. Tool calls may be interrupted by user steering.

## Decision

The **ToolCallManager** is a dedicated component that owns tool call execution, scheduling, and result delivery. It runs as a valtron task and communicates with the agent loop via `Arc`-shared state.

### Core Responsibilities

1. **Receive tool call messages** from the agent loop
2. **Classify dependencies** — which calls can run in parallel, which must be sequential
3. **Execute** — run tool calls according to their dependency graph
4. **Persist results** — save tool call requests and results to the Message API before returning
5. **Return results** — deliver persisted results to the agent loop for LLM re-processing
6. **Handle interruption** — cancel pending/in-progress tool calls on user steering

### Architecture

```
ToolCallManager { inner: Arc<ToolCallManagerInner> }
├── execution_queue: Arc<ConcurrentQueue<ToolCallRequest>>
├── results_store: Arc<ConcurrentQueue<ToolCallResult>>
├── cancellation_signal: Arc<AtomicBool>
├── message_store: Arc<MessageInner>         // for persistence
├── session_id: SessionId
└── task_entry: Option<Entry>                // valtron task entry
```

### Execution Modes

| Mode | Description | Use Case |
|------|-------------|----------|
| **Sequential** | Execute one tool call, wait for result, execute next | Dependent tool calls (output of A is input of B) |
| **Parallel** | Execute all tool calls concurrently, collect all results | Independent tool calls (no data dependencies) |
| **Batch** | Group tool calls, execute groups sequentially, parallel within groups | Mixed dependencies (some independent, some dependent) |

### Dependency Analysis

When the LLM returns multiple tool calls, the ToolCallManager analyzes dependencies:

```rust
pub enum ExecutionPlan {
    /// All tool calls are independent — run in parallel
    Parallel(Vec<ToolCallRequest>),
    
    /// All tool calls must run sequentially
    Sequential(Vec<ToolCallRequest>),
    
    /// Grouped execution: groups run sequentially, members run in parallel
    Batched(Vec<ToolCallGroup>),
}

pub struct ToolCallGroup {
    pub group_id: usize,
    pub calls: Vec<ToolCallRequest>,
    pub depends_on: Vec<usize>, // group IDs this group depends on
}
```

**Dependency detection strategy:**  
- **Default: Parallel** — tool calls with no explicit dependencies are assumed independent
- **Sequential markers** — tool calls can declare `depends_on: [tool_call_id]` to force ordering
- **Heuristic detection** (future) — analyze tool call arguments for references to prior tool call outputs

### Persistence Guarantee

**Critical invariant:** Tool call results are persisted to the Message API **before** being delivered to the agent loop.

```
LLM produces tool calls
├── ToolCallManager receives tool call requests
│   ├── Persist each tool call request to Message API (message_type: tool_call)
│   └── Classify into execution plan
├── Execute tool calls (parallel/sequential/batched)
│   ├── On each completion:
│   │   ├── Persist tool call result to Message API (message_type: tool_result)
│   │   └── Emit via Broadcaster (real-time UI updates)
│   └── On interruption (PriorityQueue has messages):
│       ├── Cancel in-progress tool calls (check cancellation_signal)
│       ├── Persist cancellation records
│       └── Return partial results
└── Return all persisted results to agent loop
```

This ensures:
- **No lost results** — if the agent crashes, tool call results are already persisted
- **Full audit trail** — every tool call and result is in the message store
- **Replayability** — session replay re-executes tool calls from the message store

### Interruption Handling

The ToolCallManager checks for interruption signals:

1. **PriorityQueue** — when a user sends a steering message, it goes to the PriorityQueue
2. **ToolCallManager** checks the PriorityQueue before starting a new execution group
3. **If PriorityQueue has messages:**
   - Cancel any in-progress tool calls (via `cancellation_signal`)
   - Persist partial results
   - Return control to the agent loop so it can process the steering message

```rust
impl ToolCallManager {
    /// Check if there's pending steering — if so, cancel and return early
    pub fn check_interruption(&self, priority_queue: &PriorityQueue) -> bool {
        if !priority_queue.is_empty() {
            self.cancellation_signal.store(true, Ordering::SeqCst);
            true
        } else {
            false
        }
    }
}
```

### Valtron Task Integration

The ToolCallManager runs as a valtron task:

```
Agent loop valtron task
├── Produces tool call messages
│   └── Enqueues to ToolCallManager's ConcurrentQueue
├── Checks ToolCallManager's result queue
│   └── When results available: deliver to agent loop, feed back to LLM
└── Meanwhile, ToolCallManager valtron task:
    ├── Dequeues tool calls
    ├── Executes (parallel/sequential/batched via valtron broadcast/sequenced)
    ├── Persists to Message API
    └── Enqueues results
```

**Why a separate valtron task?**  
- **Non-blocking** — agent loop doesn't block waiting for tool execution
- **Composable** — tool calls can use valtron's `broadcast` for parallel execution
- **Observable** — tool call progress is visible via valtron's progress states
- **Interruptible** — cancellation_signal is checked between execution groups

### API Contract

```rust
pub trait ToolCallExecution {
    /// Submit tool calls for execution
    fn submit(&self, calls: Vec<ToolCallRequest>);
    
    /// Get execution results (blocks until all complete or interrupted)
    fn get_results(&self) -> Vec<ToolCallResult>;
    
    /// Check if execution is complete
    fn is_complete(&self) -> bool;
    
    /// Cancel in-progress tool calls
    fn cancel(&self);
    
    /// Get execution plan (for debugging/observability)
    fn execution_plan(&self) -> ExecutionPlan;
}
```

## Rationale

**Why persist before delivering results?**  
- Ensures the message store is always the source of truth
- If the agent crashes between execution and delivery, results are not lost
- Enables session replay — tool call results are in the message store, not in ephemeral memory

**Why a separate valtron task instead of inline execution?**  
- Valtron's execution model is progress-driven — tool execution fits naturally
- Parallel tool calls can use valtron's `broadcast` for true concurrency
- Cancellation is clean — valtron tasks can be interrupted between execution steps
- Observability — tool call progress is visible via valtron's state machine

**Why not use external orchestration (Celery, Temporal)?**  
- Adds infrastructure complexity
- Tool calls are typically fast (sub-second to few seconds)
- Valtron already provides the execution model we need
- WASM-compatibility requirement rules out many external orchestrators

## Alternatives Considered

### Inline execution (no ToolCallManager)
- **Pros:** Simpler, no queue management
- **Cons:** Blocks agent loop, no parallelism, no interruption handling
- **Rejected because:** Agent loop must remain responsive to steering messages

### External task queue (Celery, Temporal, etc.)
- **Pros:** Distributed execution, retry logic, monitoring
- **Cons:** Infrastructure dependency, latency, not WASM-compatible
- **Rejected because:** Overkill for single-process tool call execution; valtron provides sufficient orchestration

### Always sequential execution
- **Pros:** Simple, no dependency analysis
- **Cons:** Slow when tool calls are independent
- **Rejected because:** Many tool calls are independent (e.g., read multiple files, query multiple APIs) — parallelism is essential for performance
