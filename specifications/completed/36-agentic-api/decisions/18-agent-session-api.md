# Decision 18: Agent Session API

**Status:** Accepted  
**Date:** 2026-06-13  
**Context:** Specification 36 — Agentic API for foundation_ai

## Problem

The agentic API has many components (Message API, Context API, ToolCallManager, queues, embeddings, vector store) that need to be wired together. Users shouldn't need to initialize each component manually.

## Decision

A high-level `AgentSession` constructor handles all initialization and provides a clean entry point. Inspired by Pi's `createAgentSession()` but designed in the Rust/ewe_platform style.

> **AMENDED (2026-06-15, F20):** The builder requires a **`ToolShed` + a `ProviderRouter`** (not `tools(vec![...])`). The ToolShed is the explicit struct with named fields (`read`/`edit`/`write`/`search`/`search_files`/`shell`/`shed`/`memory`/`delegate`); it's built by `ToolShed::default()` or custom-wired., why did you even come up with a tools(vec![]) or will you have a tool registry which will transform these into a ToolShed? Anyway it makes no sense, there was intent in making the ToolShed explicit with the fields it supplies.

### API Design

```rust
// Minimal: defaults with in-memory storage
let session = AgentSession::builder()
    .provider(my_model_provider)
    .build()?;

// Custom: override specific options
let session = AgentSession::builder()
    .provider(my_model_provider)
    .model("claude-sonnet-4-6")
    .tools(vec!["read", "edit", "write", "search", "bash"]) // this does not even match our ToolShed approach, fix this.
    .session_store(DocumentStore::sqlite(&conn)?)
    .vector_store(InMemoryVectorStore::new(768))
    .working_memory(WorkingMemory::default())
    .build()?;

// Resume existing session
let session = AgentSession::resume(session_id, provider, config)?;
```

### Builder Pattern

```rust
pub struct AgentSessionBuilder {
    provider: Arc<dyn ModelProvider>,
    model: Option<ModelId>,
    fallback_models: Vec<ModelId>,
    memory_model: Option<ModelId>,
    session_id: Option<SessionId>,
    tools: Vec<String>,
    session_store: Option<Arc<dyn DocumentStore>>,
    vector_store: Option<Arc<dyn VectorStore>>,
    config: AgentConfig,
}

impl AgentSessionBuilder {
    pub fn provider(mut self, provider: Arc<dyn ModelProvider>) -> Self { ... }
    pub fn model(mut self, model_id: impl Into<ModelId>) -> Self { ... }
    pub fn fallback_models(mut self, models: Vec<ModelId>) -> Self { ... }
    pub fn memory_model(mut self, model_id: impl Into<ModelId>) -> Self { ... }
    pub fn session_id(mut self, session_id: SessionId) -> Self { ... }
    pub fn tools(mut self, tools: Vec<String>) -> Self { ... }
    pub fn session_store(mut self, store: Arc<dyn DocumentStore>) -> Self { ... }
    pub fn vector_store(mut self, store: Arc<dyn VectorStore>) -> Self { ... }
    pub fn build(self) -> Result<AgentSession> { ... }
}
```

### What the Builder Initializes

```
AgentSession::builder().provider(...).build()
├── SessionId generated (scru128)
├── Message API initialized (write-buffered, pub/sub)
├── Context API initialized (WorkingMemory, ObservationMemory, ReflectionMemory)
├── ToolCallManager initialized (tools registered, workflow DAG ready)
├── PriorityQueue + FollowUpQueue initialized (empty)
├── EmbeddingProvider initialized (LRU cache, vector store)
├── LoopDetector initialized (sliding window)
└── All components wired via Arc-shared state
```

### Interaction API

> **AMENDED (2026-06-15, F03/F20):** the snippet below is SUPERSEDED. `AgentEvent` is dead (D08/D11);
> the stream is **pure `D = SessionRecord`** with thin `AgentProgress` on `Pending`. Errors are
> **`SessionRecord::FailedAction` records** (not a `Result` in `D`); synchronous methods return
> `Result<_, ErrorTrace<AgenticError>>`. The real shape:
> ```rust
> for item in session.run_turn_stream("Write a test")? {   // StreamIterator<D=SessionRecord, P=AgentProgress>
>     match item {
>         Stream::Next(SessionRecord::Conversation { message }) => render(message),
>         Stream::Next(SessionRecord::FailedAction { error, trace }) => log_error(error, trace),
>         Stream::Pending(AgentProgress::Generating { .. })          => show_status(),
>         _ => {}
>     }
> }
> ```

```rust
// Single turn
let response = session.run_turn("Fix the auth bug").await?;

// Streaming
let stream = session.run_turn_stream("Write a test for the login flow").await?;
for event in stream {
    match event {
        Stream::Next(Ok(AgentEvent::MessageUpdate { content })) => print!("{}", content),
        Stream::Next(Ok(AgentEvent::ToolCallStart { tool_name, .. })) => println!("\nCalling {tool_name}"),
        Stream::Next(Err(e)) => eprintln!("Error: {e}"),
        _ => {}
    }
}

// Session lifecycle
session.end().await?;  // flushes everything, persists state

// Resume
let session = AgentSession::resume(session_id, provider, config)?;
```

### Default Configuration

```rust
impl AgentSessionBuilder {
    /// Create a builder with sensible defaults
    pub fn new(provider: Arc<dyn ModelProvider>) -> Self {
        Self {
            provider,
            model: None,  // uses provider's default model
            fallback_models: vec![],
            memory_model: None,  // uses primary model if not set
            session_id: None,    // generates new scru128 ID
            tools: vec!["read", "edit", "write", "search", "bash"],
            session_store: None,  // uses in-memory store
            vector_store: None,   // uses in-memory vector store (768 dim)
            config: AgentConfig {
                observation_threshold: 30_000,   // 30k tokens
                reflection_threshold: 40_000,    // 40k tokens
                loop_window_size: 5,
                loop_similarity_threshold: 0.9,
                loop_max_redirects: 3,
                circuit_breaker_threshold: 3,
                message_buffer_capacity: 50,
                message_flush_interval: Duration::from_secs(5),
                embedding_cache_size: 1000,
            },
        }
    }
}
```

### Valtron Integration

The AgentSession runs as a valtron task internally:

```rust
impl AgentSession {
    /// Run the agent loop as a valtron task
    pub fn run_as_task(&self) -> Result<impl StreamIterator<D = Result<AgentEvent, AgenticError>, P = AgentProgress>> {
        let agent_loop = AgentLoop::new(self.inner.clone());
        execute(agent_loop, None)
    }
}
```

## Rationale

**Why a builder pattern?**
- Rust convention for complex construction
- Clear defaults, explicit overrides
- Type-safe configuration

**Why separate `run_turn` and `run_turn_stream`?**
- Some callers want the final result, others want real-time events
- Consistent with foundation_ai's `generate` vs `stream` pattern

**Why not a trait-based session interface?**
- Single implementation is sufficient — no need for trait abstraction
- Builder handles all configuration options
- Trait would add complexity without benefit

## Alternatives Considered

### Manual component initialization
- **Pros:** Full control over each component
- **Cons:** Error-prone, verbose, easy to misconfigure
- **Rejected because:** Users should get a working session with minimal code

### Function-based API (`create_agent_session()`)
- **Pros:** Simpler for basic use cases
- **Cons:** Less discoverable, harder to extend
- **Rejected because:** Builder pattern is more idiomatic Rust
