# Fundamentals 01 — The agentic loop

Zero-to-expert on how the AI agent works end-to-end. If you're adding a tool,
modifying the loop, or debugging agent behavior, start here.

---

## 1. The big picture

The agentic loop is a **streaming conversation** between the agent and a model
provider, interleaved with tool execution and memory management:

```
User message → AgentSession.start()
  └── Assemble context (system prompt + memories + recent messages)
  └── Stream generation from model provider
        ├── Text tokens → yield to user
        └── Tool calls → pause generation
              └── Execute tools → append results
              └── Resume generation with tool results
  └── Process output (memory triggers, loop detection)
  └── Check budget (token limit reached?)
  └── Continue or terminate
```

## 2. AgentSession

The public API for interacting with the agent:

```rust
let session = AgentSession::builder()
    .model(ModelId::Name("claude-sonnet-4-6".into(), None))
    .system_prompt("You are a coding assistant.")
    .memory_store(kv_store)
    .message_api(doc_store)
    .token_ledger(ledger)
    .build()?;

// Send a message and stream the response
let stream = session.send("Fix the bug in src/main.rs").await?;
// stream: AgentStream = Stream<SessionRecord, AgentProgress>
```

## 3. ContextProvider

Assembles the full prompt from multiple sources:

1. **System prompt** — fixed instructions from the builder
2. **Working memory** — active facts the agent should know (recent, high-confidence)
3. **Observation memory** — things the agent has noticed (medium-term)
4. **Reflection memory** — synthesized insights (long-term, sparse)
5. **Recent messages** — the last N conversation turns

Assembly order is deterministic: working → reflection → observation → recent
messages. This order matters because the model reads top-to-bottom and the most
recent content should be closest to the actual user message.

## 4. Memory hierarchy

Three tiers of memory, each with different lifespans and trigger thresholds:

| Tier | Content | Trigger | Lifespan |
|---|---|---|---|
| **Working** | Active facts, current task state | Immediate append | Until overwritten |
| **Observation** | Notable patterns, repeated behaviors | 30K rolling tokens | Until reflection supersedes |
| **Reflection** | Synthesized insights, condensed summaries | 40K observation tokens | Long-term (overwritten by newer) |

### Triggers

`MemoryHierarchy::check_triggers()` fires when:
- **Not generating** (don't interrupt active generation)
- **Rolling tokens ≥ observation threshold** → generate observation
- **Observation tokens ≥ reflection threshold** → generate reflection

Working memory takes priority over observation, observation over reflection.

### Persistence

Memory is stored via `KvMemoryStore` (foundation_db):
```
memory:{session_id} → SessionMemory { working, observation, reflection }
```

On CF Workers, this maps to KV storage. On native, it's a file or SQL table.

## 5. Tool execution

Tools are registered with `ToolCallManager` and executed via staged DAG:

```rust
let manager = ToolCallManager::new(session_id);
manager.register(Arc::new(ReadFileTool::new(vfs)));
manager.register(Arc::new(ShellTool::new(shell)));

let toolshed = manager.build_toolshed();
// toolshed.read, toolshed.shell, toolshed.search, ...
```

### Tool categories

- **read** — Read files, search code, inspect state (safe, fast)
- **write** — Edit files, create files (modifies state)
- **shell** — Run commands, scripts, processes (powerful, potentially dangerous)
- **search** — Search context, search files (read-only, semantic)
- **shed** — Tool discovery and metatool (dynamic)

### Execution stages

1. **Validate** — check arguments against schema
2. **Persist** — save tool call state before execution (crash recovery)
3. **Execute** — run the tool (async)
4. **Record** — save result to conversation history

### Dependency DAG

Tools can declare dependencies on other tools' results:

```rust
ToolCallRequest {
    id: "read-file-1",
    name: "read_file",
    arguments: {"path": "/src/main.rs"},
    depends_on: vec![],  // no dependencies
    execution_hint: ExecutionHint::default(),
}
```

The executor builds a DAG and runs independent tools in parallel.

## 6. Loop detection

Prevents the agent from getting stuck in repetitive behavior:

```rust
let detector = LoopDetector::new(LoopDetectorConfig::default());

for output in generation_stream {
    if let LoopDetection::ExactLoop { repetitions } = detector.check(&output) {
        // Agent is repeating itself → escalate
        match detector.escalate() {
            Escalation::Redirect => { restart with different prompt }
            Escalation::SwitchModelOrTemperature { delta } => { lower temperature }
            Escalation::Terminate => { end the session }
        }
    }
}
```

### Detection methods

- **Exact loop** — identical output repeated N times (simhash match = 1.0)
- **Tool call loop** — same tool call with same arguments repeated N times
- **Fuzzy loop** — near-identical output (simhash similarity ≥ threshold)

### Escalation ladder

1. **Redirect** — restart with a different system prompt (first response)
2. **Switch model or reduce temperature** — try a different model or lower
   creativity (repeated redirects)
3. **Terminate** — end the session (max redirects exceeded)

## 7. Token accounting

`TokenLedger` tracks token consumption across the session:

```rust
let ledger = TokenLedger::new();

// Record usage from a model response
ledger.record(&usage_report);

// Check budgets
if ledger.rolling() >= 30_000 {
    // Fire observation memory trigger
}
```

### Budget types

- **Rolling** — tokens consumed since last memory generation (resets on observe)
- **Session total** — all tokens consumed in the session (never resets)
- **Hard limit** — maximum tokens allowed (session terminates when reached)

### Cost calculation

Model costs are calculated via `calculate_cost()` with per-model pricing:

```rust
let pricing = ModelUsageCosting {
    input: 3.0,   // $3/M input tokens
    output: 15.0, // $15/M output tokens
    cache_read: 0.3,
    cache_write: 3.75,
};
let cost = calculate_cost(&usage_report, &pricing);
```

## 8. Error handling and retry

`AgenticError` classifies failures into actionable categories:

```rust
enum AgenticError {
    Generation(GenerationFailure), // Provider error, rate limit, network
    ToolCall { tool_name, reason }, // Tool execution failed
    ToolNotAuthorized { tool_name, user }, // User can't use this tool
    Budget { limit },              // Token budget exceeded
    LoopDetected(LoopDetection),   // Agent stuck in a loop
    Auth(AuthError),               // Authentication failed
    Routing(String),               // No provider for this model
    Unexpected(String),            // Unknown error
}
```

### Error policy

`ErrorPolicy` maps each error to an action:

| Error | Action |
|---|---|
| Context overflow | Retry with reduced context |
| Rate limit | Switch model |
| Tool call error | Continue (report to agent) |
| Budget exceeded | Terminate |
| Provider error | Terminate |

### Circuit breaker

`CircuitBreaker` provides automatic failover:

```rust
let mut breaker = CircuitBreaker::new(2, vec![fallback_model_1, fallback_model_2]);

match generation_result {
    Ok(output) => breaker.on_success(),
    Err(_) => {
        if let Some(fallback) = breaker.on_failure() {
            // Retry with fallback model
        }
    }
}
```

After N consecutive failures, switches to the next fallback model.
