# Gaps Analysis — Specification 36: Agentic API

**Date:** 2026-06-11  
**Scope:** Plan + 12 Design Decisions (01-12)

---

## Critical Gaps

### GAP-01: `Broadcaster<T>` is NOT thread-safe — breaks all pub/sub designs

**Affects:** Decision 02 (Message API), Decision 08 (Valtron Integration)

The plan and decisions repeatedly reference `Arc<Broadcaster<MessageEvent>>` for pub/sub broadcasting. However, the actual `Broadcaster<T>` in `/home/darkvoid/Boxxed/@dev/ewe_platform/backends/foundation_core/src/synca/mpp.rs` requires `&mut self` for both `broadcast()` and `subscribe()`:

```rust
pub fn broadcast(&mut self, event: T) { ... }
pub fn subscribe(&mut self) -> Receiver<T> { ... }
```

This makes it impossible to share across valtron tasks via `Arc` without wrapping in `Arc<Mutex<...>>`, which would serialize all broadcasts — defeating the purpose of a concurrent pub/sub system. The `Receiver<T>` side is `Clone` and works fine with `Arc`, but the sender side does not.

**Required:** Either wrap in interior mutability explicitly, create a `ThreadSafeBroadcaster` variant, or redesign the pub/sub mechanism. This blocks implementation of the Message API's pub/sub, EmbeddingProvider event subscription, and ObservationMemory trigger pipeline.

---

### GAP-02: `scru128` is NOT a dependency anywhere in the project

**Affects:** Decision 01 (Session Architecture), Decision 02 (Message API)

The entire ID scheme (SessionId, Message IDs, ordering, human-friendly name derivation) is built on scru128 IDs. However, `scru128` does not appear in any `Cargo.toml` in the project. It appears only as a hardcoded string in two unrelated type files. There is no `scru128` crate imported, no implementation, no ULID library.

**Required:** Add `scru128` or an equivalent ULID/UUIDv7 crate as a dependency, or implement the ID generation logic. The "deterministic name derivation" function `(name, timestamp, machine_id) → SessionId` also has no implementation spec.

---

### GAP-03: `DocumentStore` trait does NOT exist in `foundation_db`

**Affects:** Decision 12 (DB/Auth Integration)

Decision 12's architecture diagram and code examples reference `foundation_db::DocumentStore` with methods like `append()` and `scan()`. The actual `foundation_db` at `/home/darkvoid/Boxxed/@dev/ewe_platform/backends/foundation_db/src/core/storage_provider.rs` exposes:

- `KeyValueStore` (get, set, delete, exists, list_keys)
- `BlobStore` (put_blob, get_blob, delete_blob, blob_exists)
- `QueryStore` (query, execute, execute_batch)
- `RateLimiterStore`

There is NO `DocumentStore` trait, no `append()` method, and no `scan()` method. The message store design fundamentally relies on a storage primitive that does not exist.

**Required:** Either add a `DocumentStore` trait to `foundation_db`, or redesign the message persistence to use existing primitives (e.g., `KeyValueStore` with append-to-list semantics, or `QueryStore` with SQL inserts).

---

### GAP-04: `VectorStore` trait does NOT exist in `foundation_db`

**Affects:** Decision 07 (Vector Search), Decision 12 (DB/Auth Integration)

Decision 07 proposes a `VectorStore` trait to be added to `foundation_db`. Decision 12 assumes it already exists and shows the agentic API consuming it directly. However, no `VectorStore` trait exists in the current `foundation_db` codebase. The entire vector search backend infrastructure (in-memory, Turso, fjall) is unimplemented at the trait level.

**Required:** Decision 07 needs to be sequenced as a prerequisite — the `VectorStore` trait and its backends must be implemented in `foundation_db` before the agentic API can consume them.

---

### GAP-05: No `Notifier` type exists for queue wakeup

**Affects:** Decision 05 (Queue Architecture)

Decision 05 references `Arc<Notifier>` on both `PriorityQueue` and `FollowUpQueue` for waking the agent task when messages arrive:

```rust
pub struct PriorityQueue {
    queue: Arc<ConcurrentQueue<SteeringMessage>>,
    notifier: Arc<Notifier>, // wakes the agent task when messages arrive
}
```

However, `foundation_core::synca` contains only `Notifiable<T>` and `CanNotify<V>` traits — no concrete `Notifier` type. The decisions claim this matches "valtron's `EventReadiness` pattern" but there is no bridge between `Notifier` and `EventReadiness`.

**Required:** Either implement a `Notifier` type in `synca`, use `BoolSignal` (which exists and implements `EventReadiness`), or redesign the wakeup mechanism.

---

### GAP-06: `fff` (fast filesystem search) integration is entirely absent

**Affects:** Plan (mentioned twice), no decision covers it

The plan explicitly mentions `fff` integration for both the Messages API and Context API:

> "It also has a Vector search capability and also integrates `/home/darkvoid/Boxxed/@formulas/src.FileSystemAPIs/src.Search/fff` as well for fast grep/ripgrep to search files on a local filesystem."

> "Also integrates fff for ripgrep like search over files and file content."

None of the 12 decisions mention `fff`, file search, ripgrep integration, or full-text search. This is a planned feature with zero design coverage.

**Required:** A decision covering fff integration: how it plugs into Messages/Context, whether it runs as a valtron task, its API contract, and how results merge with vector search results.

---

### GAP-07: Session resume flow is underspecified

**Affects:** Decision 01 (Session Architecture), Plan

The plan mentions prepending "last 5-10 messages" on session resume, along with hydrating working memory, reflection, and observation. Decision 01 shows a resume flow diagram:

```
Session resumed (by SessionId)
├── Messages replayed from disk
├── WorkingMemory hydrated from latest snapshot
├── ObservationMemory loaded (or regenerated from observations past threshold)
├── ReflectionMemory loaded (or regenerated from reflections past threshold)
└── ToolCallManager restored (any incomplete tool calls are cancelled)
```

But critical details are missing:
- How many messages constitute "last N"? The plan says 5-10; no decision specifies.
- What is the exact context assembly on resume? Does it prepend working memory + reflection + last N messages + semantically recalled messages?
- How are "incomplete tool calls" identified and cancelled? What state is persisted to detect this?
- What happens to the write buffer on resume? Is it replayed from disk first, then new messages appended?
- Is there a resume token or version to detect stale resume attempts?

**Required:** A detailed session resume protocol decision covering context reconstruction order, message count, buffer state, and incomplete tool call handling.

---

### GAP-08: External tool registration and discovery is undefined

**Affects:** Decision 04 (ToolCallManager), Plan

The ToolCallManager handles execution but no decision covers:
- How are tools registered with the system?
- Where do tool definitions (JSON Schema for LLM function calling) come from?
- How are external tools (MCP servers, HTTP endpoints, CLI commands) discovered and loaded?
- What is the tool registry/trait that tool implementations must conform to?
- How are tool schemas converted to the LLM's function calling format?
- Is there a tool versioning strategy?

The plan mentions "Identifying the tool calls required to be made" but the mechanism for tool discovery is completely absent.

**Required:** A decision on tool registration, discovery, schema management, and the tool trait/registry interface.

---

### GAP-09: HTTP API surface is undefined

**Affects:** Decision 10 (Serialization Format), Decision 12 (DB/Auth Integration)

Decision 10 mentions transport layers:

```
Agent Loop → UI (via HTTP/WebSocket)
├── JSON for real-time streaming (text/event-stream)
└── Arrow for batch download (full session export)
```

Decision 12 shows an example auth flow for `POST /api/sessions/{session_id}/messages`. But there is no decision defining:
- The full set of HTTP endpoints
- Request/response formats per endpoint
- Whether the API is REST, gRPC, WebSocket, or Server-Sent Events
- How streaming responses work over HTTP
- Session creation, listing, deletion endpoints
- How external clients subscribe to message events

**Required:** A decision on the HTTP API surface, endpoint design, streaming protocol, and client integration pattern.

---

### GAP-10: Error handling conventions are not established

**Affects:** Decision 11 (Agentic Loop Architecture)

Decision 11 contains a basic error flow diagram but no unified error handling strategy:
- No `AgenticError` enum or type hierarchy is defined (only one variant `ToolNotAuthorized` appears in Decision 12)
- No retry policy is specified (Decision 11 mentions "retry with backoff" but no parameters)
- No distinction between retriable and non-retriable errors
- No error propagation convention between valtron tasks (do errors go through `Stream::Next(Err(...))` or a separate channel?)
- No decision on how tool call errors are surfaced to the LLM vs. to the user
- No circuit breaker or degradation strategy

**Required:** A decision on error type hierarchy, retry policies, error propagation across task boundaries, and error reporting conventions.

---

### GAP-11: Testing strategy is completely absent

**Affects:** All decisions

None of the 12 decisions mention:
- Unit testing strategy for individual components
- Integration testing for the full agentic loop
- How to mock LLM responses for deterministic tests
- How to test tool call execution with mock tools
- How to test loop detection with synthetic loop scenarios
- Performance/benchmarking strategy for vector search, message throughput
- WASM-specific testing

**Required:** A decision on testing strategy covering unit tests, integration tests, LLM mocking, and performance benchmarks.

---

## Design Inconsistencies

### INCONSISTENCY-01: Message API trait vs. direct foundation_db usage

**Decisions 02 vs 12**

Decision 02 defines a `MessageStore` trait:

```rust
pub trait MessageStore {
    fn recent(&self, n: usize) -> Vec<Message>;
    fn all(&self) -> impl Iterator<Item = Message>;
    fn semantic_search(&self, query: &str, limit: usize) -> Vec<Message>;
    fn append(&self, message: Message);
    fn flush(&self) -> Result<()>;
    fn subscribe(&self) -> Receiver<MessageEvent>;
}
```

Decision 12 explicitly states "no intermediate abstraction layer" and shows `MessageApi` using `DocumentStore` directly:

```rust
// DIRECT usage — no intermediate trait
pub struct MessageApi {
    doc_store: Arc<DocumentStore>,  // foundation_db type, directly used
}
```

These are contradictory. Decision 02 designs around a trait-based abstraction; Decision 12 rejects trait-based abstractions. Decision 12's `MessageApi` also lacks `semantic_search()`, `subscribe()`, `write_buffer`, `broadcaster`, and `vector_index` — all core to Decision 02's design.

**Resolution:** Pick one. If traits are preferred, Decision 12 needs updating. If direct usage is preferred, Decision 02's trait should be removed. The current state makes it impossible to know which architecture to implement.

---

### INCONSISTENCY-02: Loop Detector is both a background task AND an output processor

**Decisions 08 vs 11**

Decision 08 maps the Loop Detector as:
```
| Loop Detector | Background task — monitors agent output | sequenced — runs alongside agent loop |
```

Decision 11 lists it as an output processor:
```
| LoopDetector | Checks for repetition loops | Always |
```

Is the Loop Detector a separate valtron task running in parallel (Decision 08), or is it a processor that runs inline as part of the output processor workflow (Decision 11)? These are mutually exclusive execution models.

**Resolution:** Decide whether the loop detector runs as an independent task (parallel, non-blocking) or as an inline output processor (sequential, blocking). The parallel approach is better for responsiveness but requires inter-task communication. The inline approach is simpler but blocks the agent loop.

---

### INCONSISTENCY-03: Arrow/FlatBuffers decision is deferred, not resolved

**Decision 10**

Decision 10 states:
> "Start with JSON + Arrow. If Arrow's dependency tree causes issues in WASM environments, add FlatBuffers as a fallback for those environments only."

This is not a decision — it's a deferral. The actual implementation needs to know at compile time which serialization path to use. There is no:
- Feature flag design for Arrow vs FlatBuffers
- Conditional compilation strategy
- Migration path from Arrow to FlatBuffers
- Criteria for when the fallback is triggered

**Resolution:** Either commit to Arrow and accept WASM limitations, or design the feature flag + conditional compilation strategy now.

---

### INCONSISTENCY-04: Auth trait exists but foundation_auth types don't match

**Decision 12 vs actual foundation_auth**

Decision 12 defines an `AuthProvider` trait with methods like `authenticate()`, `authorize()`, `record_usage()`, `allowed_models()`, `allowed_tools()`. However, the actual `foundation_auth` crate exposes:

- `AuthManager` (not a trait)
- `AuthState`, `AuthStateMachine`
- `CredentialStore`, `AsyncCredentialStore`
- `JwtVerifier`, `OAuthManager`
- `Session`, `SessionManager`

There is no `AuthProvider` trait in foundation_auth. Decision 12 creates a new trait in `foundation_ai::agentic::auth` that is entirely disconnected from the existing `foundation_auth` types. The `AuthenticatedUser` type referenced in Decision 12 doesn't exist in foundation_auth (it has `Authenticated` enum).

**Resolution:** Either adapt the existing `foundation_auth` types into the agentic API, or explicitly define the bridge/adapter layer between Decision 12's `AuthProvider` and foundation_auth's actual API.

---

### INCONSISTENCY-05: Context assembly order contradicts Observation Memory role

**Decision 03 vs Decision 11**

Decision 03 states:
> "Observation memory is NOT directly included in context — it has been condensed into reflections."

Decision 11's Input Processor table includes:
> | ObservationInjector | Activates observations if threshold exceeded | When observation memory has content |

These contradict. Decision 03 says observations are never directly in context (only reflections). Decision 11 says observations are injected when they have content. If observations exist but haven't been reflected on yet, are they in context or not?

**Resolution:** Clarify the transitional state: when observations exist but reflections haven't been generated yet, are observations injected? Decision 03 should cover this gap.

---

## Missing Coverage

### MISSING-01: `fff` integration (fast filesystem search)

**Mentioned in:** Plan (lines 143, 144)  
**Covered by:** No decision

The plan mentions integrating `fff` (ripgrep-based fast filesystem search) into both Messages and Context APIs. None of the 12 decisions address:
- How fff plugs into the architecture
- Its API contract
- Whether it runs as a valtron task
- How fff results merge with vector search results
- When to use fff vs vector search

---

### MISSING-02: External tool registration and discovery

**Mentioned in:** Plan (line 145, ToolCallManager)  
**Covered by:** No decision

Decision 04 covers tool execution but not tool registration. Missing:
- Tool registry/trait
- Tool schema definition
- LLM function calling format conversion
- External tool loading (MCP, HTTP, CLI)
- Tool versioning

---

### MISSING-03: HTTP API surface definition

**Mentioned in:** Decision 10 (transport layer), Decision 12 (one example endpoint)  
**Covered by:** No dedicated decision

Missing:
- Complete endpoint list
- Request/response schemas
- Streaming protocol (SSE, WebSocket, etc.)
- Session lifecycle endpoints (create, list, delete)
- Rate limiting at the API layer

---

### MISSING-04: Error handling conventions

**Mentioned in:** Decision 11 (brief error flow diagram)  
**Covered by:** No dedicated decision

Missing:
- Error type hierarchy
- Retry policies and parameters
- Error propagation across task boundaries
- Circuit breaker / degradation strategy

---

### MISSING-05: Testing strategy

**Mentioned in:** None  
**Covered by:** No decision

Missing:
- Unit test patterns
- Integration test patterns
- LLM response mocking
- Deterministic test scenarios
- Performance benchmarks

---

### MISSING-06: Session resume message prepending detail

**Mentioned in:** Plan (line 162: "Get the last 5 messages")  
**Covered by:** No decision

The plan mentions prepending "last 5-10 messages" on resume. No decision specifies the exact count, the context assembly order on resume, or how the resume context differs from a fresh session context.

---

## Open Questions

### Q-01: Are token thresholds (30k/40k) configurable?

Decision 03 hardcodes Mastra-inspired thresholds (30k tokens for observation generation, 40k for reflection). These should be configurable per session, per model, or per deployment. Different models have different context windows.

### Q-02: What is the machine-ID embedding strategy for scru128?

Decision 01 mentions machine-ID embedding in SessionId but provides no mechanism for obtaining or generating the machine ID. Is it hostname-based, UUID-based, or environment-variable-driven?

### Q-03: How does the write buffer handle backpressure?

Decision 02 describes a `ConcurrentQueue<QueuedMessage>` write buffer but does not specify what happens when the queue is full. Does it block? Drop? Force-push? The flush task is a separate valtron task — what if it falls behind?

### Q-04: What happens on crash before message buffer flush?

Decision 02 guarantees "all buffered messages flushed to disk" on session end. But what about crashes? There is no write-ahead log, no periodic forced flush, and no crash recovery mechanism. Messages in the buffer at crash time are permanently lost.

### Q-05: How does the Loop Detector handle multi-turn subtle loops?

Decision 09's sliding window (default 5 messages) detects exact repetition, near-repetition, and tool call repetition. It does NOT detect semantic loops where the LLM takes different paths but achieves no progress (e.g., trying different tools that all fail in the same way, or re-exploring the same problem space with different wording). This is the most common loop type in practice.

### Q-06: What is the migration path from flat scan to IVF/HNSW indexing?

Decision 07 says "start with flat scan" and "add IVF when stores exceed 1,000 vectors." But there is no migration strategy: how is the index built? Does it block queries? Is it incremental? What happens to queries during index construction?

### Q-07: How are embedding dimension mismatches handled at runtime?

Decision 06 has a `DimensionRegistry` that panics on mismatch. In production, should it panic, or degrade gracefully? What if a model is reconfigured to use different dimensions?

### Q-08: What happens when multiple sessions share the same vector store?

Decision 07's `VectorMetadata` includes `session_id`, but Decision 12 says the agentic API uses foundation_db directly. If multiple sessions write to the same Turso database, how is vector search scoped to a single session? The `query()` method in Decision 07 doesn't accept a session filter.

---

## Edge Cases

### EDGE-01: Crash during tool call execution

If a tool call is in progress when the process crashes, Decision 01 says "incomplete tool calls are cancelled." But the tool may have had side effects (file writes, API calls). There is no idempotency guarantee or compensation mechanism.

### EDGE-02: Concurrent writes to the same session

What happens if two agent loops (or a user and an agent) write to the same session simultaneously? The `ConcurrentQueue` handles enqueue ordering, but the flush task drains sequentially. What about vector store writes from two concurrent embedding tasks?

### EDGE-03: Tool call with non-existent tool name

The LLM generates a tool call for a tool that isn't registered. Decision 04 shows `prepareToolCall() → validate` but does not define the validation logic or the error response to the LLM.

### EDGE-04: Embedding queue overflow

If messages arrive faster than embeddings can be generated (LLM generates many messages per second, embedding takes 10-100ms each), the `ConcurrentQueue<EmbeddingRequest>` grows unbounded. There is no backpressure mechanism, no queue size limit, and no drop policy.

### EDGE-05: Vector store exceeds flat scan threshold during operation

Decision 07 says flat scan for < 1,000 vectors, IVF for 1,000-100,000. But the transition from flat scan to IVF is not described. Does the system rebuild the index inline? Block queries? Run a background task?

### EDGE-06: Session with no messages

A session is created but the LLM produces no output (immediate error, or user cancels). What does the session store look like? Is an empty session valid? How is it listed?

### EDGE-07: LLM generates tool calls requiring user confirmation

Some tools may require explicit user approval before execution (destructive operations). The current design assumes all tool calls execute automatically. There is no "pending approval" state in the ToolCallManager.

### EDGE-08: Working memory grows unbounded

Decision 03 says Working Memory is "continuously updated — outdated facts are removed." But there is no mechanism described for detecting or removing outdated facts. Without this, working memory grows until it exceeds the ~500 token budget.

### EDGE-09: PriorityQueue and FollowUpQueue interaction

What happens if a message arrives in both queues simultaneously? Can the same message be in both? Is there deduplication? Decision 05 doesn't cover this.

### EDGE-10: Model change during persistent loop

Decision 09 says "try a different model" on persistent loops. But different models may have different tool calling formats, different context windows, and different token costs. There is no state migration plan when switching models mid-session.

---

## Recommendations

### REC-01: Define a `ThreadSafeBroadcaster` wrapper

Wrap the existing `Broadcaster<T>` in `Arc<Mutex<Broadcaster<T>>>` or create a new `ThreadSafeBroadcaster` with `&self` methods backed by interior mutability. This is needed for all pub/sub patterns in the design.

### REC-02: Add `DocumentStore` trait to `foundation_db` before agentic API work

The message store design requires append-and-scan semantics that don't map cleanly to existing `KeyValueStore` or `QueryStore`. A dedicated `DocumentStore` trait with `append()`, `scan()`, and `scan_all()` methods should be added to foundation_db.

### REC-03: Create a prerequisite decision for `VectorStore` implementation

Decision 07 should be split: Decision 07a for the trait definition and in-memory implementation (prerequisite for agentic API), Decision 07b for Turso/fjall backends (can be deferred).

### REC-04: Add a decision on error handling

Create Decision 13: Error Handling Conventions, covering error type hierarchy, retry policies, error propagation, and degradation strategies.

### REC-05: Add a decision on tool registration and discovery

Create Decision 14: Tool Registry and Discovery, covering tool trait, schema management, external tool loading, and LLM function calling format conversion.

### REC-06: Add a decision on the HTTP API surface

Create Decision 15: HTTP API Design, covering endpoints, streaming protocol, request/response formats, and client integration.

### REC-07: Define crash recovery for message write buffer

Add a write-ahead log or periodic forced flush mechanism to Decision 02. Consider a two-phase commit: append to WAL, then to buffer, then flush from buffer to permanent storage.

### REC-08: Add semantic loop detection to Decision 09

In addition to exact/fuzzy/tool-call repetition, add semantic similarity detection using embeddings. When the LLM's last N turns have high semantic similarity but no progress, trigger the loop redirect.

### REC-09: Define vector store scoping mechanism

Add a `session_id` filter parameter to `VectorStore::query()` to ensure cross-session isolation when multiple sessions share the same persistent vector store backend.

### REC-10: Add a testing strategy decision

Create Decision 16: Testing Strategy, covering unit tests, integration tests, LLM mocking via fixture replay, deterministic tool call testing, and performance benchmarks.
