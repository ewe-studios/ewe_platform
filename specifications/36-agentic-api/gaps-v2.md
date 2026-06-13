# Gaps Analysis v2 -- Specification 36: Agentic API

**Date:** 2026-06-13  
**Scope:** Plan + 17 Design Decisions (01-17)  
**Reviewer:** Fresh reviewer, no prior context

---

## Critical Issues

### CRIT-01: `VectorStore` trait does not exist; `foundation_vectors` crate does not exist

**Affects:** Decision 07 (Vector Search), Decision 12 (DB/Auth), Decision 15 (Tool Registration)

Decision 07 proposes creating a `foundation_vectors` crate for vector algorithms AND adding a `VectorStore` trait to `foundation_db`. Neither exists. A grep of `foundation_db/src/` for `VectorStore` returns zero results. Decision 12 assumes `VectorStore` is a `foundation_db` primitive. Decision 15's `shed` tool relies on `vector_store.query()` in a "toolshed namespace."

This is a prerequisite dependency chain that must be implemented before any agentic API code can compile. The entire semantic recall, observation memory vector search, and tool discovery infrastructure is blocked.

**Required:** Implement `foundation_vectors` crate (flat scan, IVF, HNSW algorithms) AND add `VectorStore` trait + at least the in-memory backend to `foundation_db` before agentic API implementation begins.

---

### CRIT-02: `Broadcaster<T>` requires `&mut self` -- all pub/sub designs are broken

**Affects:** Decision 02 (Message API), Decision 08 (Valtron Integration)

The actual `Broadcaster<T>` at `foundation_core/src/synca/mpp.rs:400-437` has:

```rust
pub fn subscribe(&mut self) -> Receiver<T> { ... }
pub fn broadcast(&mut self, event: T) { ... }
```

Decision 02's architecture shows:
```rust
broadcaster: Arc<ConcurrentQueue<MessageEvent>>  // pub/sub for listeners
```

Decision 08's task mapping shows the EmbeddingProvider task, ObservationMemory task, and external listeners all "subscribing to Message API broadcaster" events. But you cannot call `subscribe()` or `broadcast()` on a type behind `Arc` without `Arc<Mutex<...>>` serialization, which defeats the purpose. Decision 02's rationale even says "Interior mutability (ConcurrentQueue, Broadcaster) avoids `&mut self` requirements" -- this is factually false for `Broadcaster`.

**Required:** Either wrap `Broadcaster` in `Arc<Mutex<Broadcaster<T>>>`, create a `ThreadSafeBroadcaster` with `&self` methods backed by interior mutability, or redesign pub/sub using the existing `ConcurrentQueue<Receiver<T>>` pattern.

---

### CRIT-03: Decision 16 references `GenerationError` variants that do not exist

**Affects:** Decision 16 (Error Handling)

Decision 16's error handling code references:
```rust
AgenticError::Generation(GenerationError::ContextOverflow) => { ... }
AgenticError::Generation(GenerationError::RateLimit) => { ... }
```

The actual `GenerationError` enum at `foundation_ai/src/errors/mod.rs:19-69` has NO `ContextOverflow` or `RateLimit` variants. It has: `Failed`, `LlamaCpp`, `Tokenization`, `TokenToString`, `Decode`, `Encode`, `Embeddings`, `ChatTemplate`, `ApplyChatTemplate`, `LlamaModelLoad`, `LlamaContextLoad`, `Candle` (feature-gated), `Tokenizer`, `Backend`, `Generic`.

The context overflow detection exists in `foundation_ai/src/types/mod.rs` as `Messages::is_context_overflow()` -- a method on the message type, NOT an error variant. Rate limiting is handled by `RateLimiterStore` in `foundation_db`, not by `GenerationError`.

**Required:** Either add `ContextOverflow` and `RateLimit` variants to `GenerationError`, or change Decision 16 to detect these conditions differently (e.g., `Messages::is_context_overflow()` for overflow, `RateLimiterStore` check for rate limits).

---

### CRIT-04: Decision 15 claims ToolShed has no `others` field -- it does

**Affects:** Decision 15 (Tool Registration)

Decision 15 states:
```
// No `others` field -- shed tool covers dynamic discovery
```

The actual `ToolShed` at `foundation_ai/src/types/mod.rs:1067-1077`:
```rust
pub struct ToolShed {
    pub shed: Tool,
    pub memory: Option<MemoryTool>,
    pub delegate: Option<DelegationTool>,
    pub read: Tool,
    pub edit: Tool,
    pub write: Tool,
    pub search: Tool,
    pub bash: Option<Tool>,
    pub others: Option<Vec<Tool>>,  // <-- EXISTS
}
```

This contradicts Decision 15's entire rationale: "Why no `others` field? -- `shed` tool covers dynamic discovery." The `others` field already exists and is part of the public API. Either Decision 15 is wrong, or the `others` field needs to be removed from `ToolShed`.

---

### CRIT-05: `Stream<D, P>` has no Error variant -- Decision 16's error propagation is incompatible

**Affects:** Decision 16 (Error Handling), Decision 11 (Agentic Loop)

The actual `Stream<D, P>` enum at `foundation_core/src/valtron/streams.rs:78` has variants: `Init`, `Ignore`, `Delayed`, `Pending(P)`, `Next(D)`, `Wait`, `Spread(...)`. There is NO error variant.

Decision 16 claims errors flow as `Stream::Next(Err(AgenticError))` through `Stream<AgentEvent, AgentProgress>`. But `AgentEvent` is an enum (not a `Result`), so `Stream<AgentEvent, AgentProgress>::Next` yields `AgentEvent`, not `Result<AgentEvent, AgenticError>`.

For Decision 16's pattern to work, the stream type must be `Stream<Result<AgentEvent, AgenticError>, AgentProgress>`, but no decision specifies this. Decision 11 defines the stream as `Stream<AgentEvent, AgentProgress>` with no `Result` wrapper.

**Required:** Decide whether the agent loop stream is `Stream<Result<AgentEvent, AgenticError>, AgentProgress>` (errors via Next) or add a dedicated `Error(E)` variant to the Stream enum, or use a separate error channel.

---

### CRIT-06: Decision 02 proposes `MessageRole` enum that does not exist

**Affects:** Decision 02 (Message API)

Decision 02 states the `role` field should be changed from `String` to a `MessageRole` enum:
```rust
pub enum MessageRole {
    User, Agent, System, Tool, Custom(String),
}
```

The actual `Messages::User` at `foundation_ai/src/types/mod.rs:893-898`:
```rust
User {
    role: String,              // <-- String, not MessageRole
    content: UserModelContent,
    signature: Option<String>,
},
```

`MessageRole` does not exist anywhere in `foundation_ai`. This is a breaking API change that would require updating all providers (Anthropic, OpenAI, Gemini, etc.) and the entire `Messages` enum serialization. Decision 02 presents this as if it's already done.

---

### CRIT-07: Decision 05 `CancelCode` enum has invalid Rust syntax

**Affects:** Decision 05 (Queue Architecture)

Decision 05 shows:
```rust
pub enum CancelCode: u32 {
    None = 0,
    PauseForPriority = 1,
    Abort = 2,
}
```

This is not valid Rust syntax. The correct form is `#[repr(u32)] pub enum CancelCode { ... }`. More importantly, this type does not exist in `foundation_core::synca` or anywhere in the project.

---

### CRIT-08: DocumentStore `scan()` returns `StorageItemStream`, not `Vec` -- Decision 12's code is wrong

**Affects:** Decision 12 (DB/Auth Integration)

Decision 12 shows:
```rust
pub fn recent(&self, n: usize) -> Result<Vec<Message>> {
    let docs = self.doc_store.scan(&key, n)?;  // Returns Vec<Message>
    ...
}
```

The actual `DocumentStore::scan` at `foundation_db/src/core/storage_provider.rs:547-551`:
```rust
fn scan<V: DeserializeOwned + Send + 'static>(
    &self, key: &str, limit: usize,
) -> StorageResult<StorageItemStream<'_, V>>;  // Returns Stream, not Vec
```

`StorageItemStream<'_, V>` is a `Box<dyn Iterator<Item = Stream<Result<T, StorageError>, ()>>>`. Decision 12's code treats it as returning a `Vec` directly. The caller must collect the stream first.

---

## Design Inconsistencies

### INCON-01: Message API trait (Decision 02) vs direct DocumentStore usage (Decision 12) -- still unresolved

Decision 02 defines a `MessageStore` trait with `recent()`, `all()`, `semantic_search()`, `append()`, `flush()`, `subscribe()`. Decision 12 explicitly states "no intermediate abstraction layer" and shows `MessageApi` using `DocumentStore` directly. Both are still "Proposed" with no resolution. The two decisions prescribe mutually exclusive architectures.

### INCON-02: Loop Detector execution model contradiction persists

Decision 08 maps Loop Detector as: `Background task -- monitors agent output | sequenced -- runs alongside agent loop`
Decision 11 lists it as: `LoopDetector | Checks for repetition loops | Always` (in the output processors table)

Is it a parallel background task or an inline output processor? The distinction matters for whether loop detection blocks the agent loop.

### INCON-03: Observation injection contradicts Decision 03

Decision 03: "Observation memory is NOT directly included in context -- it has been condensed into reflections."
Decision 11 input processor table: `ObservationInjector | Activates observations if threshold exceeded | When observation memory has content`

If observations exist but haven't been reflected on yet (the transitional state), are they in context? Decision 03 says no; Decision 11 says yes (when content exists).

### INCON-04: Decision 10's Arrow/FlatBuffers decision is still a deferral

Decision 10 says "Start with JSON + Arrow. If Arrow's dependency tree causes issues in WASM environments, add FlatBuffers as a fallback." This is not a decision -- it's a deferral with no feature flag design, conditional compilation strategy, or migration path. The decisions says "The feature flag and conditional compilation strategy is designed upfront" but provides zero design for it.

### INCON-05: Decision 13 says "ALL backends implemented. No partial delivery" -- unrealistic scope

Decision 13 requires ALL five backends (SQL, Memory, VFS, Cloudflare KV, Cloudflare D1) to be implemented at once for the DocumentStore feature. This is an unrealistic all-or-nothing constraint that will block all agentic API work until every backend is complete. Decision 07 makes the same demand for VectorStore backends.

---

## Missing Coverage

### MISS-01: No `MemoryFact`, `ObservationEntry`, `ReflectionEntry` types defined

Decision 02 adds `WorkingMemory`, `Observation`, and `Reflection` variants to the `Messages` enum using types `MemoryFact`, `ObservationEntry`, `ReflectionEntry`, and `ReflectionEntry`. None of these types exist in `foundation_ai::types`. The JSON structures in Decision 03 show what they should look like, but no Rust types are defined.

### MISS-02: `Scru128` as a Rust type is not imported

Decision 02 shows:
```rust
pub struct StoredMessage {
    pub id: Scru128,
    ...
}
```

`foundation_rng` exports `scru128::Id` and a `new_scru128_string()` function. There is no `Scru128` type alias or wrapper. Decision 01 also references `Scru128` as a type name. Either a type alias needs to be created, or the decisions should use `scru128::Id` directly.

### MISS-03: No `ToolCallRequest` / `ToolCallResult` types defined

Decisions 04, 11, and 16 reference `ToolCallRequest` and `ToolCallResult` types extensively. The actual `foundation_ai::types` has `ModelOutput::ToolCall` and `Messages::ToolResult`, but no standalone `ToolCallRequest` / `ToolCallResult` types for the ToolCallManager API. Either these need to be defined or the ToolCallManager should work directly with existing types.

### MISS-04: No `AgentConfig` type defined

Decision 16 references `AgentConfig` with `provider`, `primary_model`, `fallback_models`, `memory_model`. Decision 17 references `AgentSession::new(session_id, provider, config)`. No such types exist.

### MISS-05: Token counting mechanism is undefined

Decisions 03 and 11 use token thresholds (30k for observation generation, 40k for reflection generation) to trigger memory generation. But there is no decision on how tokens are counted. Is it based on `UsageReport` from LLM responses? Does it count tool call arguments? Memory snapshots? The token counting mechanism is referenced but never defined.

### MISS-06: "Shed meta-tool" implementation details missing

Decision 15 describes the `shed` meta-tool conceptually but provides no implementation details for:
- How the tool description vector store is populated (at session creation? lazily?)
- How the `shed` tool executes (is it a regular ToolCallManager execution or special?)
- What happens when `shed` returns multiple tool candidates -- does the agent pick one automatically?
- How are external tools (MCP, HTTP, CLI) converted to `Tool` and added to ToolShed?

### MISS-07: No migration path from Decision 01's `scru128`-based SessionId to existing auth Session types

`foundation_auth` has `Session` and `SessionManager` types. Decision 1 defines a new `SessionId` (scru128-based) for the agentic API. There is no discussion of how these two session concepts coexist, whether they share storage, or whether the agentic `SessionId` replaces the auth `Session`.

### MISS-08: No decision on how `ToolFormatter` implementations handle the `ToolShed` structure

Decision 15 mentions `ToolFormatter` converts `Tool` definitions to provider format. But `ToolShed` is a struct with named fields, not a flat `Vec<Tool>`. How does `ToolFormatter::format_tools(&[Tool])` receive tools from a `ToolShed`? Does someone flatten the ToolShed first? Is there a `ToolShed::into_tools()` method?

---

## Edge Cases

### EDGE-01: Crash during tool call execution with side effects

Decision 01 says "incomplete tool calls are cancelled" on session end. But if a tool call (e.g., `bash` command writing to disk) is mid-execution when the process crashes, there is no idempotency guarantee. The tool may have partially executed. No compensation mechanism exists.

### EDGE-02: Write buffer full -- what happens?

Decision 02's write buffer uses `ConcurrentQueue<QueuedMessage>`. What happens when the queue is full? `ConcurrentQueue::push()` returns `Err(PushError::Full)`. Does the agent block? Drop the message? Force-flush? No backpressure mechanism is defined.

### EDGE-03: Embedding queue overflow

If messages arrive faster than embeddings can be generated (embedding takes 10-100ms each), the `ConcurrentQueue<EmbeddingRequest>` grows unbounded. Decision 06 has no queue size limit or drop policy.

### EDGE-04: Semantic loop detection missing

Decision 09's sliding window detects exact repetition, fuzzy similarity (SimHash), and tool call repetition. It does NOT detect semantic loops where the LLM takes different paths but makes no progress (e.g., trying different tools that all fail the same way). This is the most common loop type in practice.

### EDGE-05: Multiple sessions sharing vector store -- no scoping

Decision 07's `VectorStore::query(vector, top_k)` has no session filter parameter. If multiple sessions write to the same persistent vector store (e.g., Turso), cross-session contamination occurs. Decision 12's namespace-based key scheme (`session:{id}:vectors`) doesn't help because `VectorStore` is a separate store from the document store.

### EDGE-06: Tool requiring user confirmation

Decision 04's ToolCallManager assumes all tool calls execute automatically. Some tools (destructive operations) may need explicit user approval. No "pending approval" state exists.

### EDGE-07: Working memory grows unbounded

Decision 03 says "outdated facts are removed" but provides no mechanism for detecting or removing outdated facts. Without this, working memory grows until it exceeds the ~500 token budget.

### EDGE-08: Model change mid-session requires state migration

Decision 09 says "try a different model" on persistent loops. Different models have different tool calling formats (native API vs. XML tags), different context windows, and different token costs. No state migration plan exists when switching models mid-session.

### EDGE-09: Vector store index transition blocks queries

Decision 07 says "start with flat scan, add IVF when stores exceed 1,000 vectors." The transition from flat scan to IVF is not described. Does index construction block queries? Is it incremental?

### EDGE-10: Concurrent writes to same session from different agents

What happens if two agent loops write to the same `SessionId` simultaneously? The `ConcurrentQueue` handles enqueue ordering, but vector store writes from two concurrent embedding tasks could produce duplicate embeddings.

---

## Incomplete Decisions

### INC-01: Decision 03 -- Configurable thresholds

Decision 03 hardcodes 30k/40k token thresholds for observation/reflection generation. These should be configurable per session, per model context window, or per deployment. Different models have vastly different context windows (4k for local models vs. 200k for API models).

### INC-02: Decision 06 -- `DimensionRegistry` panic vs graceful degradation

Decision 06 states the `DimensionRegistry` "panics or returns error on dimension mismatch." In production, should it panic? What if a model is reconfigured with different embedding dimensions at runtime?

### INC-03: Decision 07 -- All backends at once is impractical

Decision 07: "ALL backends listed above are implemented. No partial delivery." This includes Turso/D1 with native vector support, SQLite with sqlite-vec, Fjall with IVF, Cloudflare D1 with SQL cosine similarity, Cloudflare KV with fetch-all + client-side search. This is an enormous implementation scope for a single feature. The "no partial delivery" constraint will block progress.

### INC-04: Decision 09 -- SimHash/MinHash implementation undefined

Decision 09's fuzzy similarity detection uses "SimHash / MinHash comparison" but provides no implementation details. These are non-trivial algorithms requiring bit-vector manipulation and LSH tables. No crate dependency or implementation strategy is specified.

### INC-05: Decision 11 -- Input/Output processor interface is underspecified

Decision 11 shows an `InputProcessorWorkflow` with a `dyn InputProcessor` trait that has `.id()` and `.process(context)`. But the trait definition is incomplete: what does `process` return? How does a processor signal "skip this"? How does deduplication by ID work when processors have different runtime configurations?

### INC-06: Decision 14 -- WASM platform lacks fff entirely

Decision 14 correctly notes fff is native-only (heed, memmap2, notify, git2, rayon). But the fallback for WASM is "vector only" -- meaning the agent has NO filesystem search capability in WASM. For a coding agent that runs in CF Workers, this is a significant capability gap.

### INC-07: Decision 15 -- ToolShed structure doesn't match Decision 04's ToolCallManager

Decision 15 defines `ToolShed` with named fields (read, edit, write, search, bash). Decision 04's `ToolCallManager::execute()` uses a match on `call.name.as_str()`. These are disconnected -- there's no mechanism to look up a tool by name in the ToolShed. The ToolCallManager needs a `HashMap<String, Tool>` or similar lookup structure.

### INC-08: Decision 17 -- MockModelProvider uses regex but real providers don't

Decision 17's `MockModelProvider` uses `Regex` to match input patterns to scripted responses. But the actual LLM generation path in foundation_ai works with `ModelInteraction` (a struct with system_prompt, messages, tools_shed). The regex matching approach doesn't map cleanly to the actual input type.

---

## Recommendations

### REC-01: Resolve the MessageStore trait vs direct DocumentStore conflict

Pick one architecture and remove the other decision. Given that Decision 12 already exists with the "no intermediate abstraction" rationale, Decision 02's `MessageStore` trait should be converted to a concrete `MessageApi` struct that wraps `DocumentStore` and provides the higher-level operations (`semantic_search`, `subscribe`, etc.) directly.

### REC-02: Split Decision 07 into phases

Phase 1: `foundation_vectors` crate with flat scan + in-memory `VectorStore` backend. Phase 2: Persistent backends (Turso, SQLite, Fjall). Phase 3: Cloudflare backends. This allows agentic API development to proceed with in-memory vector search while persistent backends are built.

### REC-03: Add missing type definitions

Create `MemoryFact`, `ObservationEntry`, `ReflectionEntry`, `ToolCallRequest`, `ToolCallResult`, `AgentConfig`, and `AgentSession` types in a dedicated `foundation_ai::agentic::types` module.

### REC-04: Fix the error propagation type

Change the agent loop stream to `Stream<Result<AgentEvent, AgenticError>, AgentProgress>` or add a dedicated `Error(E)` variant to the Stream enum. Update Decision 11 and Decision 16 consistently.

### REC-05: Add `GenerationError::ContextOverflow` and `GenerationError::RateLimit`

Or alternatively, define detection mechanisms that don't require new error variants (e.g., `Messages::is_context_overflow()` for overflow, `RateLimiterStore` check for rate limits).

### REC-06: Define token counting mechanism

Add a decision on how session tokens are tracked. Likely candidate: accumulate `UsageReport::total_tokens` from each LLM response, with periodic recalculation for accuracy.

### REC-07: Add `session_id` parameter to `VectorStore::query()`

For cross-session isolation, the query method should accept an optional namespace filter: `fn query(&self, vector: &[f32], top_k: usize, namespace: Option<&str>) -> Result<Vec<VectorMatch>>`.

### REC-08: Create a Decision 18: Type Definitions

Consolidate all missing type definitions (`MemoryFact`, `ObservationEntry`, `ReflectionEntry`, `Scru128` alias, `ToolCallRequest`, `ToolCallResult`, `AgentConfig`, `AgentSession`) into a single decision.

### REC-09: Resolve ToolShed `others` field contradiction

Either remove `others: Option<Vec<Tool>>` from `ToolShed` (breaking change) or update Decision 15 to acknowledge the field exists and explain how `shed` meta-tool discovery complements the `others` list.

### REC-10: Add `#[repr(u32)]` to Decision 05's `CancelCode`

Fix the invalid Rust syntax. Also consider whether this type should live in `foundation_core::synca` or in a new agentic-specific module.
