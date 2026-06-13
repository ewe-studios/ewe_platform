# Decision 11: Agentic Loop Architecture

**Status:** Proposed  
**Date:** 2026-06-11  
**Context:** Specification 36 — Agentic API for foundation_ai

## Problem

The agentic loop receives a **ModelProvider** (not just a Model), giving it access to:
- All available models for fallback on error (circuit breaker)
- A smaller model for memory generation (observations, reflections)
- Model switching based on cost/performance requirements It must support:
- Streaming LLM responses
- Tool call extraction, execution, and result injection
- User interruption (immediate and deferred)
- Memory generation triggers
- Loop detection
- Progress emission for UI and monitoring

## Decision

The agentic loop follows a **nested inner/outer loop** pattern, synthesized from Pi (inner/outer loop with steering) and Mastra (input/output processors with memory management). The loop runs as a valtron `TaskIterator` and emits progress via `Stream` states.

### Loop Structure

```
agent.prompt("Fix the bug")
└─ runWithLifecycle()
   └─ runAgentLoop(prompts, context, config, emit, streamFn)
      └─ runLoop()
         │
         ├─ emit(AgentEvent::SessionStart)
         ├─ emit(AgentEvent::TurnStart)
         │
         ├─ OUTER LOOP (follow-up continuation)
         │   │
         │   ├─ emit(MessageStart/End) for each prompt
         │   │
         │   ├─ Check FollowUpQueue → if messages, add as pending, continue outer loop
         │   │
         │   ├─ INNER LOOP (tool calls + steering)
         │   │   │
         │   │   ├─ Drain PriorityQueue → inject at FRONT of message list
         │   │   │   └── If PriorityQueue had messages:
         │   │   │       └── ToolCallManager.cancel() → abort in-progress tool calls
         │   │   │
         │   │   ├─ transformContext(messages) → assemble context from memory
         │   │   │   ├── Working Memory (always present)
         │   │   │   ├── Reflection Memory (summarized observations)
         │   │   │   ├── Recent raw messages (last N)
         │   │   │   └── Semantically recalled messages (vector search)
         │   │   │
         │   │   ├─ convertToLlm(messages) → format for LLM API
         │   │   │
         │   │   ├─ streamAssistantResponse()
         │   │   │   ├── streamSimple(model, context)
         │   │   │   └── Emit: message_start → message_update* → message_end
         │   │   │
         │   │   ├─ If error/aborted → emit(TurnEnd, AgentEnd), return
         │   │   │
         │   │   ├─ Extract tool calls from LLM response
         │   │   │
         │   │   ├─ If tool calls:
         │   │   │   │
         │   │   │   ├─ ToolCallManager.submit(tool_calls)
         │   │   │   │
         │   │   │   ├─ executeToolCalls() → parallel/sequential/batched
         │   │   │   │   ├── prepareToolCall() → validate, beforeToolCall
         │   │   │   │   ├── execute() → run tool with abort signal
         │   │   │   │   └── finalizeExecutedToolCall() → afterToolCall
         │   │   │   │
         │   │   │   ├─ Emit: tool_execution_start → tool_execution_update* → tool_execution_end
         │   │   │   │
         │   │   │   ├─ message_start/end for each tool result
         │   │   │   │
         │   │   │   └── Check PriorityQueue → if steering, cancel and return to outer loop
         │   │   │
         │   │   └─ Check steering queue → repeat inner loop if messages
         │   │
         │   ├─ Output Processors (Mastra-inspired)
         │   │   ├── MessageHistory: save new messages
         │   │   ├── SemanticRecall: create embeddings for new messages
         │   │   ├── ObservationalMemory: check observation/reflection trigger
         │   │   │   └── If recent interactions > 30k tokens → generate observations
         │   │   │   └── If observation memory > 40k tokens → generate reflections
         │   │   └── WorkingMemory: update if new facts detected
         │   │
         │   └─ Check FollowUpQueue → if messages, continue outer loop
         │       └── Move to pending, repeat outer loop
         │
         └─ emit(AgentEvent::SessionEnd, newMessages)
```

### Input Processors (Mastra-Inspired)

Before each LLM call, input processors transform the context:

| Processor | Function | Trigger |
|-----------|----------|---------|
| **WorkingMemoryInjector** | Injects working memory as system message | Always |
| **MessageHistoryLoader** | Loads last N messages from store | Always |
| **SemanticRecall** | Queries vector store for relevant older messages | When recent messages insufficient |
| **ReflectionInjector** | Injects reflection memory as context | When observations have been reflected |
| **ObservationInjector** | Activates observations if threshold exceeded | When observation memory has content |

Processors are deduplicated by processor ID and executed in priority order:

```rust
pub struct InputProcessorWorkflow {
    processors: Vec<Box<dyn InputProcessor>>,
}

impl InputProcessorWorkflow {
    pub fn execute(&self, context: &mut AgentContext) -> Result<()> {
        // Deduplicate by processor ID
        let mut seen = HashSet::new();
        let processors = self.processors.iter()
            .filter(|p| seen.insert(p.id()))
            .collect::<Vec<_>>();
        
        // Execute in priority order
        for processor in processors {
            processor.process(context)?;
        }
        Ok(())
    }
}
```

### Output Processors (Mastra-Inspired)

After each LLM response, output processors handle side effects:

| Processor | Function | Trigger |
|-----------|----------|---------|
| **MessageSaver** | Saves new messages to store | Always |
| **EmbeddingGenerator** | Creates embeddings for new messages | Always (cached) |
| **ObservationTrigger** | Checks if observation generation needed | When token threshold exceeded |
| **ReflectionTrigger** | Checks if reflection generation needed | When observation threshold exceeded |
| **WorkingMemoryUpdater** | Updates working memory with new facts | When new facts detected |
| **LoopDetector** | Checks for repetition loops | Always |

### Stream States (Valtron Progress)

The agent loop emits progress via `Stream<Result<AgentEvent, AgenticError>, AgentProgress>`:

```rust
// Stream::Next(value) — discrete events
pub enum AgentEvent {
    SessionStart { session_id: SessionId },
    TurnStart { turn: usize },
    MessageStart { message_variant: &'static str },
    MessageUpdate { content: String },    // streaming token chunks
    MessageEnd { message_variant: &'static str, full_content: String },
    ToolCallStart { tool_name: String, arguments: String },
    ToolCallUpdate { progress: String },
    ToolCallEnd { tool_name: String, result: String },
    ObservationGenerated { count: usize },
    ReflectionGenerated { count: usize },
    LoopDetected { detection: LoopDetection },
    SessionEnd { message_count: usize },
}

// Stream::Pending(progress) — progress information
pub enum AgentProgress {
    Initializing { step: String },
    Generating { model: String, tokens_so_far: usize },
    ExecutingTools { total: usize, completed: usize },
    ProcessingMemory { step: String },
    FlushingMessages { count: usize },
}
```

### Valtron Integration

The agent loop is a `TaskIterator` that maps to valtron states:

```
TaskStatus::Init              → Session initializing, loading context
TaskStatus::Pending(Progress) → LLM generating, tool executing, memory processing
TaskStatus::Ready(Event)      → Discrete event (message end, tool result, etc.)
TaskStatus::Ignore            → Checking queues, no event to emit
TaskStatus::Spawn(action)     → Spawning sub-tasks (tool execution, embedding generation)
```

This allows the agent loop to be:
- **Scheduled** via `execute()` on the valtron pool
- **Observed** — progress visible via `Stream::Pending`
- **Interrupted** — `map_circuit` can short-circuit on steering
- **Composed** — multiple agent loops in parallel via `execute_collect_all`

### Error Handling

```
Error during LLM call
├── If rate limit → retry with backoff, emit ToolCallUpdate
├── If model error → try fallback model (if configured)
├── If aborted (steering) → emit AgentEvent::SessionEnd with partial results
└── If unrecoverable → emit AgentEvent::SessionEnd with error details

Error during tool execution
├── If tool panicked → emit ToolCallEnd with error result, continue loop
├── If tool timeout → emit ToolCallEnd with timeout error, continue loop
└── If all tool calls fail → emit AgentEvent::SessionEnd with error details
```

## Rationale

**Why nested inner/outer loop?**  
- Inner loop handles tool call execution — the "work" of each turn
- Outer loop handles follow-up continuation — "what's next?"
- This separation allows PriorityQueue to interrupt the inner loop while FollowUpQueue waits for the outer loop boundary

**Why input/output processors (Mastra pattern)?**  
- Decouples context transformation from the core loop
- Processors are composable — add/remove without changing the loop
- Deduplication prevents redundant processing
- Easy to test — each processor is an isolated unit

**Why valtron TaskIterator for the agent loop?**  
- Progress-driven execution maps naturally to agent states
- Stream::Pending carries progress information (tokens generated, tools executing)
- Combinators (`map_circuit`, `filter_ready`) shape the loop behavior
- Works in both multi-threaded and WASM environments

## Alternatives Considered

### Single flat loop (no inner/outer)
- **Pros:** Simpler
- **Cons:** No distinction between "stop now" and "continue after" — can't serve both PriorityQueue and FollowUpQueue semantics
- **Rejected because:** Two distinct user steering patterns require two loop boundaries

### Async/await agent loop
- **Pros:** Familiar pattern
- **Cons:** No intermediate progress states, harder to compose
- **Supported via bridge:** The stream-to-future bridge allows `.await` in async contexts

### State machine (explicit states, transitions)
- **Pros:** Explicit, easy to reason about
- **Cons:** Verbose, hard to extend, doesn't compose well
- **Rejected because:** TaskIterator provides the same state machine semantics with better composition
