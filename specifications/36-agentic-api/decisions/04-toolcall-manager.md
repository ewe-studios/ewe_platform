# Decision 04: ToolCallManager Design

**Status:** Accepted  
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

### Dependency Analysis — Workflow-Based DAG Execution

When the LLM returns multiple tool calls, the ToolCallManager builds a **workflow** — a flat list of stages that execute sequentially, where each stage is either parallel or sequential tool calls.

```rust
pub struct ToolCallWorkflow {
    pub stages: Vec<ToolCallStage>,
}

pub enum ToolCallStage {
    /// Run these tool calls in parallel
    Parallel {
        calls: Vec<ToolCallRequest>,
        fail_mode: FailMode,  // continue on failure vs cancel all
    },
    /// Run these tool calls sequentially, each feeds off the previous result
    Sequential {
        calls: Vec<ToolCallRequest>,
        fail_fast: bool,  // if one fails, stop the rest
    },
}

pub enum FailMode {
    /// Continue running all tool calls, collect all results (default)
    CollectAll,
    /// If any fails, cancel the remaining calls in this stage
    CancelOnFailure,
}
```

The LLM declares dependencies via fields in the tool call schema. These fields **must be added** to `foundation_ai::types::ModelOutput::ToolCall`:

```rust
// In foundation_ai::types::ModelOutput
ToolCall {
    id: String,
    name: String,
    arguments: Option<HashMap<String, ArgType>>,
    signature: Option<String>,
    // NEW: dependency fields for DAG execution
    depends_on: Vec<String>,        // tool call IDs this depends on
    execution_hint: ExecutionHint,  // parallel, sequential, or unspecified
}

pub enum ExecutionHint {
    Unspecified,   // ToolCallManager decides (default)
    Parallel,      // Run in parallel with other independent calls
    Sequential,    // Run after depends_on calls complete
}
```

The schema is included in the tool definition sent to the LLM, so the LLM **knows** it can declare dependencies:

```json
{
  "name": "read_file",
  "description": "Read file content",
  "parameters": {
    "type": "object",
    "properties": {
      "path": { "type": "string" },
      "depends_on": {
        "type": "array",
        "items": { "type": "string" },
        "description": "Tool call IDs this depends on"
      },
      "execution_hint": {
        "type": "string",
        "enum": ["parallel", "sequential", "unspecified"],
        "default": "unspecified"
      }
    }
  }
}
```

Without these fields in the schema, the LLM cannot declare dependencies — it has no way to know the feature exists.

The ToolCallManager groups tool calls into stages based on dependency depth:
1. Tool calls with no dependencies → Stage 0 (parallel)
2. Tool calls depending on Stage 0 → Stage 1 (sequential or parallel based on their dependencies)
3. And so on...

**Execution example:**

```
LLM returns 5 tool calls:
├── call_1: read_file("auth.rs")          — no dependencies
├── call_2: read_file("middleware.rs")    — no dependencies
├── call_3: search_content("auth_check")  — no dependencies
├── call_4: analyze_results(call_1, call_2, call_3)  — depends on [call_1, call_2, call_3]
└── call_5: write_report(call_4)          — depends on [call_4]

ToolCallManager builds workflow:
├── Stage 0 (Parallel): [call_1, call_2, call_3]
│   └── All run concurrently, collect all results
├── Stage 1 (Sequential): [call_4]
│   └── Runs after Stage 0 completes, feeds off results
└── Stage 2 (Sequential): [call_5]
    └── Runs after Stage 1 completes

Execution:
├── Stage 0 → 3 files read in parallel
├── Stage 1 → analysis (waits for all reads)
└── Stage 2 → report (waits for analysis)
```

**Sequential stage with multiple calls:**

```
LLM returns 3 dependent calls:
├── call_1: query_database("SELECT users")     — no dependencies
├── call_2: process_data(call_1.result)        — depends on [call_1]
└── call_3: save_results(call_2.result)        — depends on [call_2]

ToolCallManager builds workflow:
├── Stage 0 (Sequential, fail_fast=true): [call_1, call_2, call_3]
│   └── call_1 runs, result passed to call_2, result passed to call_3
│   └── If any fails, remaining calls are skipped
```

**Execution logic:**

```rust
impl ToolCallManager {
    fn execute_workflow(&self, workflow: ToolCallWorkflow) -> Vec<ToolCallResult> {
        let mut results = HashMap::new();
        
        for stage in workflow.stages {
            match stage {
                ToolCallStage::Parallel { calls, fail_mode } => {
                    let stage_results = self.execute_parallel(calls, fail_mode);
                    
                    // Check if any failed and fail_mode is CancelOnFailure
                    if fail_mode == FailMode::CancelOnFailure 
                        && stage_results.iter().any(|r| r.is_err()) {
                        break;  // cancel remaining stages
                    }
                    results.extend(stage_results);
                }
                ToolCallStage::Sequential { calls, fail_fast } => {
                    for call in calls {
                        let result = self.execute(call);
                        results.insert(call.id.clone(), result.clone());
                        
                        if fail_fast && result.is_err() {
                            break;  // stop sequential chain
                        }
                    }
                }
            }
        }
        results
    }
}
```

**Dependency detection strategy:**  
- **Default: Parallel** — tool calls with no explicit dependencies are assumed independent
- **Explicit dependencies** — tool calls declare `depends_on: [tool_call_id]` to force ordering
- **Stage grouping** — ToolCallManager groups calls into stages by dependency depth (topological sort)
- **LLM hints** — LLM can use `execution_hint` (Parallel, Sequential) to guide grouping
- **Fail mode** — Parallel stages default to `CollectAll` (continue on failure), can be set to `CancelOnFailure`
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

> **RESOLVED (2026-06-15, F01 + F23):** `ModelOutput::ToolCall` gains **`depends_on: Vec<String>`** + **`execution_hint: ExecutionHint`** with `#[serde(default)]` (F01). DAG staging is implemented in F23.

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
