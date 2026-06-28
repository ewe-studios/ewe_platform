# How-To: Customizing the Agent Loop

How to modify the agent's behavior: iteration limits, tool strategies,
memory triggers, and circuit breaker behavior.

---

## 1. The AgentLoop lifecycle

The agent loop runs in iterations:

```
User message
  → Assemble context (system prompt + memories + recent messages)
  → Generate from model
    → Text response → yield to user
    → Tool calls → execute → loop back with results
  → Check memory triggers (observation/reflection)
  → Check loop detection
  → Check budget
  → Continue or terminate
```

Two iteration levels:
- **Inner iteration** — tool call rounds within a single generation
- **Outer iteration** — full conversation rounds (user → agent → user)

## 2. Configuring iteration limits

```rust
let config = AgentConfig {
    max_inner_iterations: 10,  // Default: 10 tool call rounds
    max_outer_iterations: 5,   // Default: 5 conversation rounds
    ..AgentConfig::default()
};

let session = AgentSession::builder(SessionId::new(), router)
    .with_config(config)
    .build()?;
```

**When to change:**
- **Increase inner** for complex tool chains (e.g., read → analyze → write)
- **Decrease inner** for fast responses (agent stops after N tool calls)
- **Increase outer** for long conversations
- **Decrease outer** for single-turn interactions

## 3. Circuit breaker behavior

The circuit breaker switches to fallback models after repeated failures:

```rust
let config = AgentConfig {
    circuit_breaker_threshold: 3,  // Failures before fallback (default: 3)
    fallback_models: vec![
        ModelId::Name("gpt-4".into(), None),
        ModelId::Name("local-llama".into(), None),
    ],
    ..AgentConfig::default()
};
```

**How it works:**
1. Primary model fails → increment failure counter
2. Counter reaches threshold → switch to first fallback
3. Fallback also fails → switch to next fallback
4. All fallbacks exhausted → terminate with error

**When to change:**
- **Lower threshold** (2) for production systems (fail fast, switch quickly)
- **Higher threshold** (5) for development (give primary more chances)

## 4. Context pressure management

The agent monitors context window usage and can compress or truncate:

```rust
let config = AgentConfig {
    context_pressure_threshold: 0.8,  // Warn at 80% context usage
    preflight_compression_threshold: 0.9,  // Compress at 90%
    ..AgentConfig::default()
};
```

**What happens at each threshold:**
- **Warning (context_pressure_threshold)** — log a warning, continue normally
- **Compression (preflight_compression_threshold)** — compress recent messages
  before next generation (summarize verbose turns)
- **Hard limit** — terminate with `AgenticError::ContextOverflow`

## 5. Memory trigger customization

Control when the agent generates observations and reflections:

```rust
let memory_config = MemoryConfig {
    observation_trigger_tokens: 30_000,  // Generate observation after 30K tokens
    reflection_trigger_tokens: 40_000,   // Generate reflection after 40K tokens
    ..MemoryConfig::default()
};

let session = AgentSession::builder(SessionId::new(), router)
    .with_memory_config(memory_config)
    .build()?;
```

**When to change:**
- **Lower thresholds** (10K/15K) for memory-intensive agents (research, analysis)
- **Higher thresholds** (50K/80K) for token-efficient agents (simple Q&A)
- **Disable memory** — set thresholds to `usize::MAX`

## 6. Model parameter tuning

```rust
let config = AgentConfig {
    model_params: ModelParams {
        temperature: 0.7,       // 0.0 (deterministic) to 1.0 (creative)
        top_p: 0.95,           // Nucleus sampling (0.0 to 1.0)
        max_tokens: 4096,      // Max tokens per response
        stop_sequences: vec![], // Stop when any sequence appears
        ..ModelParams::default()
    },
    ..AgentConfig::default()
};
```

**When to change:**
- **Low temperature** (0.1-0.3) for code generation, factual responses
- **High temperature** (0.7-1.0) for creative writing, brainstorming
- **Low top_p** (0.7) for focused, safe responses
- **High top_p** (0.95) for diverse, varied responses

## 7. Custom tool strategies

### Tool selection per model

```rust
// Some models handle tools better than others
let toolshed = if model.is_tool_capable() {
    build_full_toolshed()
} else {
    ToolShed::default()  // No tools for this model
};
```

### Tool timeout configuration

```rust
// Tools can be configured with timeouts
let tool_config = ToolConfig {
    timeout: Duration::from_secs(30),  // 30 second max per tool
    max_retries: 3,
    ..ToolConfig::default()
};
```

## 8. Error handling customization

The agent's error policy maps errors to actions:

| Error | Default Action | Customization |
|---|---|---|
| Context overflow | Retry with reduced context | Increase context window |
| Rate limit | Switch model | Retry with backoff |
| Tool error | Continue (report to agent) | Retry tool, abort session |
| Budget exceeded | Terminate | Alert user, continue |
| Provider error | Terminate | Switch provider, retry |

To customize, implement a custom `ErrorPolicy`:

```rust
struct MyErrorPolicy;

impl ErrorPolicy for MyErrorPolicy {
    fn classify(&self, error: &AgenticError) -> AgentAction {
        match error {
            AgenticError::Generation(f) if f.kind == GenKind::RateLimit => {
                AgentAction::RetryWithBackoff(Duration::from_secs(5))
            }
            // ... custom logic
            _ => AgentAction::default_classify(error),
        }
    }
}
```
