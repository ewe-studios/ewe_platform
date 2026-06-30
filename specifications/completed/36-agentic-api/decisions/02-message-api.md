# Decision 02: Message API Design

**Status:** Accepted  
**Date:** 2026-06-11  
**Context:** Specification 36 — Agentic API for foundation_ai

## Problem

The Message API is the authoritative record of every interaction within a session. It must support:
- Storing every message type (user, assistant, tool call, tool result, thinking, steering)
- Fast retrieval of recent messages (last N)
- Full replay from session start
- Semantic search over message content
- Pub/sub notification to listeners (other valtron tasks)
- Write buffering to reduce disk I/O pressure
- Serialization to JSON and Arrow formats

## Decision

The Message API is an `Arc<MessageInner>`-backed store with write buffering, pub/sub broadcasting, and pluggable persistence. Messages are **never compacted** — the store is an append-only audit trail.

### Foundation_ai Message Types

All message content uses foundation_ai's existing types from `foundation_ai::types`, with `role` changed from `String` to a typed enum:

```rust
/// Source of a message — distinguishes human, agent, system, and tool origins.
pub enum MessageRole {
    User,           // human user input
    Agent,          // another LLM agent providing guidance/steering
    System,         // system prompt/instructions
    Tool,           // tool result context
    Custom(String), // extensibility for unknown/custom roles
}

pub enum Messages {
    User {
        role: MessageRole,           // enum, not String
        content: UserModelContent,   // Text(TextContent) | Image(ImageContent)
        signature: Option<String>,
    },
    Assistant {
        model: ModelId,
        timestamp: SystemTime,
        usage: UsageReport,
        content: ModelOutput,        // Text | Image | ThinkingContent | ToolCall | Embedding
        stop_reason: StopReason,
        provider: ModelProviders,
        error_detail: Option<String>,
        signature: Option<String>,
        metadata: Option<Vec<GenerationMetadata>>,
    },
    ToolResult {
        id: String,
        name: String,
        timestamp: SystemTime,
        details: Option<String>,
        content: UserModelContent,
        error_detail: Option<String>,
        signature: Option<String>,
    },
}
```

This means:
- **Human user input** → `Messages::User { role: MessageRole::User, ... }`
- **Agent-to-agent steering** → `Messages::User { role: MessageRole::Agent, ... }`
- **System instructions** → `Messages::User { role: MessageRole::System, ... }`
- **LLM response** → `Messages::Assistant { ... }`
- **Tool output** → `Messages::ToolResult { ... }`

### Agentic-Extended Message Types

The agentic API adds these variants to the `Messages` enum for internal session management:

```rust
// Agentic additions to Messages enum — for internal session management.
// Steering and System messages use Messages::User with MessageRole::Agent/System.
pub enum Messages {
    // ... existing variants (User, Assistant, ToolResult) ...
    
    WorkingMemory {
        facts: Vec<MemoryFact>,
        version: u64,
        timestamp: SystemTime,
    },
    Observation {
        observations: Vec<ObservationEntry>,
        token_count: u64,
        timestamp: SystemTime,
    },
    Reflection {
        reflections: Vec<ReflectionEntry>,
        generated_at: SystemTime,
        observation_token_count_before: u64,
        reflection_token_count_after: u64,
    },
}
```

### Core Architecture

```
Message { inner: Arc<MessageInner> }
├── write_buffer: Arc<ConcurrentQueue<QueuedMessage>>  // in-memory buffer
├── flush_task: Arc<MessageTask>                        // valtron task that flushes to disk
├── broadcaster: Arc<ConcurrentQueue<MessageEvent>>     // pub/sub for listeners (Decision 01 pattern)
├── vector_index: Arc<dyn VectorStore>                  // semantic search
└── session_id: SessionId                               // owns this store
```

### Write Buffering Strategy

Messages are **never written directly to disk**. Instead:

1. **Incoming messages** are enqueued into a `ConcurrentQueue<QueuedMessage>` shared via `Arc`
2. **Flush triggers:**
   - Buffer reaches capacity threshold (e.g., 50 messages)
   - Time-based flush interval expires (e.g., every 5 seconds)
   - Agent session ends (guaranteed flush before shutdown)
3. **Flush task** is a dedicated valtron task that drains the queue and persists to disk
4. **Before agent stop**, the flush task is awaited — all messages guaranteed persisted

### Message Ordering

Messages use **scru128 IDs** (from `foundation_rng`) for natural time ordering:

```rust
pub struct StoredMessage {
    pub id: Scru128,              // time-ordered unique ID
    pub session_id: SessionId,
    pub message: Messages,         // foundation_ai::types::Messages
    pub created_at: u128,          // scru128 timestamp (redundant but queryable)
}
```

### Pub/Sub Integration

The broadcaster uses `Arc<ConcurrentQueue<MessageEvent>>` (Decision 01 pattern):

```rust
pub enum MessageEvent {
    Appended { message_id: Scru128, message_variant: &'static str },
    Flushed { count: usize },
    Error { error: String },
}
```

This enables:
- **EmbeddingProvider task** subscribes to `Appended` events → generates embeddings for new messages
- **ObservationMemory task** subscribes to `Appended` events → tracks token accumulation *TODO*: why is ObservationMemory any business with token accumulation, its job is to record observations
- **External listeners** (UI, monitoring) subscribe for real-time message streaming

### Semantic Recall

Messages are indexed in a **vector store** for semantic search:

- Each message's text content is embedded via the shared `EmbeddingProvider`
- Vector search returns relevant messages for context assembly
- Messages are retrieved by semantic similarity + recent message combination
- Vector index is flushed alongside the message write buffer

### Serialization

All messages are serializable to:
- **JSON** — via `serde` on foundation_ai's `Messages` enum (already `Serialize + Deserialize`)
- **Arrow** — columnar format for batch processing, analytics, and efficient transport

### API Contract

```rust
pub trait MessageStore {
    /// Get the last N messages (for immediate context, no semantic search)
    fn recent(&self, n: usize) -> Vec<StoredMessage>;
    
    /// Get all messages from session start (full replay)
    fn all(&self) -> impl Iterator<Item = StoredMessage>;
    
    /// Semantic search — find messages relevant to query
    fn semantic_search(&self, query: &str, limit: usize) -> Vec<StoredMessage>;
    
    /// Append a message (buffered, not immediately persisted)
    fn append(&self, message: Messages);
    
    /// Flush all buffered messages to disk (blocking)
    fn flush(&self) -> Result<()>;
    
    /// Subscribe to message events
    fn subscribe(&self) -> Arc<ConcurrentQueue<MessageEvent>>;
}
```

## Rationale

**Why `Arc<MessageInner>` with interior mutability?**  
- Multiple valtron tasks need concurrent access to the same store
- `Arc` allows cheap cloning across task boundaries
- Interior mutability (ConcurrentQueue, Broadcaster) avoids `&mut self` requirements
- Methods can be `&self` — compatible with valtron's execution model

**Why write buffering instead of immediate writes?**  
- LLM interactions produce many messages per turn (tool calls, results, assistant text)
- Immediate disk writes create I/O pressure and latency
- Buffering batches writes, reducing disk operations by 10-50x
- Flush-on-exit guarantees no data loss

**Why append-only (no compaction)?**  
- Compaction destroys audit trail — impossible to replay or debug
- Memory pressure is managed by the Context memory hierarchy (ObservationMemory → ReflectionMemory), not by compacting messages.
- The message store is the source of truth — everything else derives from it

**Why scru128 IDs on messages?**  
- Natural ordering without `ORDER BY created_at` queries
- Time-embedded — listing messages is just ID sorting
- Collision-free — safe for parallel message generation

> **RESOLVED (2026-06-15, F01 + F04 + F08):** `Messages::User.role` changed from `String` to **`MessageRole` enum** (F01). Memory records use a **`SessionRecord` wrapper enum** (`Conversation { message: Messages }` struct variant + WorkingMemory/Observation/Reflection), NOT inline `Messages` variants — keeps `Messages` provider-pure (F01). Token tracking is in the **`TokenLedger`** (F04), not ObservationMemory. The `&self` pub/sub is a **`&self`-safe broadcaster in `synca`** (F08).

## Alternatives Considered

### Immediate disk writes (no buffering)
- **Pros:** Simpler, no flush-on-exit required
- **Cons:** High I/O latency, especially with many tool calls per turn
- **Rejected because:** Latency compounds with tool call frequency — unacceptable for agent loops

### SQLite-backed store
- **Pros:** ACID guarantees, query flexibility
- **Cons:** Adds database dependency, not WASM-compatible, overkill for append-only log
- **Deferred to:** Persistence layer decision — the trait interface allows SQLite as one backend, NDJSON as another

### Compaction (removing old messages)
- **Pros:** Reduces storage size
- **Cons:** Destroys audit trail, breaks replayability
- **Rejected because:** Context reduction is handled by memory hierarchy, not message deletion
