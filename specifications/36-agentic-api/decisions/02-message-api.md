# Decision 02: Message API Design

**Status:** Proposed  
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

### Core Architecture

```
Message { inner: Arc<MessageInner> }
├── write_buffer: Arc<ConcurrentQueue<QueuedMessage>>  // in-memory buffer
├── flush_task: Arc<MessageTask>                        // valtron task that flushes to disk 
├── broadcaster: Arc<Broadcaster<MessageEvent>>         // pub/sub for listeners
├── vector_index: Arc<VectorStore>                      // semantic search
└── session_id: SessionId                               // owns this store
```

1. Having a valtron task to flush to disl is ok, should create a new wrapper for ConccurrentQueue that wraps it in Arc<T> and has a related type that takes a duration and a max items which a valtron tasks can use to report a TaskStatus::Depends(T) which will keep saying NO till either the duration elapsed (which we update after) or the total items in the queue is >= to max account which will wake up the task to flush to disk.

### Write Buffering Strategy

Messages are **never written directly to disk**. Instead:

1. **Incoming messages** are enqueued into a `ConcurrentQueue<QueuedMessage>` shared via `Arc`
2. **Flush triggers:**
   - Buffer reaches capacity threshold (e.g., 50 messages)
   - Time-based flush interval expires (e.g., every 5 seconds)
   - Agent session ends (guaranteed flush before shutdown)
3. **Flush task** is a dedicated valtron task that drains the queue and persists to disk
4. **Before agent stop**, the flush task is awaited — all messages guaranteed persisted

### Message Types (Append-Only Audit Trail)

Every interaction is a JSON-serializable record:

| message_type | Content | Used By |
|-------------|---------|---------|
| `user` | User text input | Agent loop, context assembly |
| `assistant` | LLM text response | Agent loop, context assembly |
| `tool_call` | Tool invocation request | ToolCallManager, agent loop |
| `tool_result` | Tool execution result | Agent loop, context assembly |
| `thinking` | LLM reasoning/thinking block | Agent loop (may be skipped in compacted views) |
| `steering` | User interrupt/redirect | PriorityQueue, agent loop |
| `observation` | ObservationMemory snapshot | Context memory (skipped by agent by default) |
| `reflection` | ReflectionMemory summary | Context memory (skipped by agent by default) |
| `working_memory` | WorkingMemory snapshot | Context memory (hydrated on session resume) |
| `system` | System prompt / instructions | Agent loop (first message) |
| `error` | Error/retry/failure record | Agent loop, debugging |

### Message Ordering

Messages use **scru128 IDs** (same as SessionId) for natural time ordering:

```rust
pub struct Message {
    pub id: Scru128,          // time-ordered unique ID
    pub session_id: SessionId,
    pub message_type: MessageType,
    pub role: MessageRole,    // user, assistant, system, tool
    pub content: MessageContent, // text, tool_calls, thinking, etc.
    pub metadata: MessageMeta,   // timestamps, token counts, etc.
    pub created_at: u128,        // scru128 timestamp (redundant but queryable)
}
```

### Pub/Sub Integration

The `Broadcaster` from `foundation_core::synca` allows any valtron task to subscribe to message events:

```rust
pub enum MessageEvent {
    Appended { message_id: Scru128, message_type: MessageType },
    Flushed { count: usize },
    Error { error: String },
}
```

This enables:
- **EmbeddingProvider task** subscribes to `Appended` events → generates embeddings for new messages
- **ObservationMemory task** subscribes to `Appended` events → tracks token accumulation
- **External listeners** (UI, monitoring) subscribe for real-time message streaming

### Semantic Recall

Messages are indexed in a **vector store** for semantic search:

- Each message's text content is embedded via the shared `EmbeddingProvider`
- Vector search returns relevant messages for context assembly
- Messages are retrieved by semantic similarity + recent message combination
- Vector index is flushed alongside the message write buffer

### Serialization

All messages are serializable to:
- **JSON** — human-readable, compatible with external tools
- **Arrow** — columnar format for batch processing, analytics, and efficient transport

### API Contract

```rust
pub trait MessageStore {
    /// Get the last N messages (for immediate context, no semantic search)
    fn recent(&self, n: usize) -> Vec<Message>;
    
    /// Get all messages from session start (full replay)
    fn all(&self) -> impl Iterator<Item = Message>;
    
    /// Semantic search — find messages relevant to query
    fn semantic_search(&self, query: &str, limit: usize) -> Vec<Message>;
    
    /// Append a message (buffered, not immediately persisted)
    fn append(&self, message: Message);
    
    /// Flush all buffered messages to disk (blocking)
    fn flush(&self) -> Result<()>;
    
    /// Subscribe to message events
    fn subscribe(&self) -> Receiver<MessageEvent>;
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
- Memory pressure is managed by the Context memory hierarchy (ObservationMemory → ReflectionMemory), not by compacting messages
- The message store is the source of truth — everything else derives from it

**Why scru128 IDs on messages?**  
- Natural ordering without `ORDER BY created_at` queries
- Time-embedded — listing messages is just ID sorting
- Collision-free — safe for parallel message generation

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
