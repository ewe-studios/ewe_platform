# How-To: Working with the Memory System

How the agent's memory hierarchy works, how to configure triggers, and how
to customize memory generation.

---

## 1. The three memory tiers

| Tier | Content | Trigger | Lifespan |
|---|---|---|---|
| **Working** | Active facts, current task state | Immediate append | Until overwritten |
| **Observation** | Notable patterns, behaviors | 30K rolling tokens | Until reflection supersedes |
| **Reflection** | Synthesized insights | 40K observation tokens | Long-term |

## 2. Memory flow

```
Agent generates tokens
  → TokenLedger tracks rolling count
  → Rolling >= 30K → fire observation trigger
    → Memory model generates observation
    → Observation appended to memory
    → Rolling count resets
  → Observation tokens >= 40K → fire reflection trigger
    → Memory model generates reflection
    → Reflection replaces previous reflection
    → Observation count resets
```

## 3. Configuring memory triggers

```rust
let memory_config = MemoryConfig {
    observation_trigger_tokens: 30_000,  // Default
    reflection_trigger_tokens: 40_000,   // Default
    ..MemoryConfig::default()
};

let session = AgentSession::builder(SessionId::new(), router)
    .with_memory_config(memory_config)
    .build()?;
```

## 4. Memory generation models

The memory model generates observations and reflections:

```rust
let config = AgentConfig {
    memory_model: Some(ModelId::Name("claude-haiku-4-5".into(), None)),
    // Use a cheaper model for memory (it doesn't need to be smart)
    ..AgentConfig::default()
};
```

**Recommendations:**
- Use a **fast, cheap model** for memory generation (haiku, fast)
- Memory generation is background work — don't block the main response
- Memory models don't need tool access

## 5. Memory persistence

Memory is stored via `MemoryStore`:

```
memory:{session_id} → SessionMemory {
    working: Option<SessionRecord::WorkingMemory>,
    observation: Option<SessionRecord::Observation>,
    reflection: Option<SessionRecord::Reflection>,
}
```

**Default:** `KvMemoryStore` (in-memory HashMap for tests, KV storage for CF Workers)

**Custom storage:**

```rust
let memory_store = MyCustomMemoryStore::new(database);

let session = AgentSession::builder(SessionId::new(), router)
    .with_memory_store(memory_store)
    .build()?;
```

## 6. Inspecting memory state

```rust
// Access the memory hierarchy directly
let memory = session.memory();

// Check current token counts
let observation_tokens = memory.observation_memory_tokens();
let is_generating = memory.is_generating();

// Check triggers
let action = memory.check_triggers();
match action {
    MemoryAction::GenerateObservation => println!("Observation needed"),
    MemoryAction::GenerateReflection => println!("Reflection needed"),
    MemoryAction::None => println!("No memory action needed"),
}
```

## 7. Memory and context assembly

When the agent assembles context for generation, memory is included:

```
System prompt
  → Working memory (active facts)
  → Reflection memory (insights, if exists)
  → Observation memory (observations, if no reflection)
  → Recent messages (last N turns)
  → User message
```

This order ensures the model sees the most relevant context first.

## 8. Disabling memory

```rust
// Set thresholds to max to effectively disable
let memory_config = MemoryConfig {
    observation_trigger_tokens: usize::MAX,
    reflection_trigger_tokens: usize::MAX,
    ..MemoryConfig::default()
};
```

## 9. Memory and session resume

On resume, memory is loaded in Decision 01 order:

1. Load WorkingMemory (active facts)
2. Load Observation + Reflection (use reflection if exists, else observation)
3. Load recent 10 messages
4. Assemble context

The agent continues with full context from where it left off.
