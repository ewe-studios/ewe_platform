---
feature: "Message API — write-buffered append-only store with pub/sub + vector index"
description: "The session's authoritative append-only record log: Arc<MessageInner> over DocumentStore, a write buffer + flush valtron task, &self pub/sub for listeners, vector indexing via EmbeddingProvider+VectorStore, and recent/all/scan_from/semantic_search"
status: "pending"
priority: "high"
depends_on: ["01-message-model", "04-documentstore-trait-sql-memory", "12-vectorstore-trait-inmemory", "15-embedding-provider"]
estimated_effort: "large"
created: 2026-06-14
last_updated: 2026-06-14
author: "Main Agent"
tasks:
  completed: 0
  uncompleted: 13
  total: 13
  completion_percentage: 0%
---

# Feature 16: Message API

> **Review status (2026-06-14) — folded:**
> 1. **`append(&self, record) -> Scru128`** mints the scru128 **before enqueue** (so id-order ==
>    arrival-order) and **returns it** (callers need the id; `SessionRecord` has no id field). The id
>    becomes `doc_id` via F04 `append_with_id`. (OD-16-6.)
> 2. **The `&self` broadcaster is NET-NEW** — no `&self`-safe broadcaster exists in `synca`
>    (`Broadcaster` is `&mut self`). Build a small `Mutex<Vec<Sender<MessageEvent>>>` + `&self`
>    `subscribe`/`broadcast`, **elevated into `foundation_core::synca::mpp`**, reusing the existing
>    `Broadcaster::broadcast` fan-out/cleanup. `subscribe()` returns mpp `Receiver` (drop Decision 02's
>    `Arc<ConcurrentQueue>`). (OD-16-1 = build commitment.)
> 3. **Reads use F04's `scan_documents`/`scan_documents_from`** (`Document`-returning, carry `id` +
>    promoted columns) — NOT `scan`/`scan_all` (return `V`, no id). `StoredRecord.id` needs them.
> 4. **`semantic_search` hydration:** F12 `query` returns id+score only; F04 has no `get` — **add F04
>    `get_many(ids) -> Vec<Document>`** (or accept N `scan_from(id,1)`). (OD-16-10.)
> 5. **flush task type is `SpawnInfo`** (not `Entry`); the periodic trigger is `TaskStatus::Delayed(5s)`;
>    **`flush()` drains SYNCHRONOUSLY on the calling thread** for shutdown (no join primitive exists) —
>    the task only handles the ≥50/5s triggers. (OD-16-7.)
> 6. **Flush-task embedding must respect F15's OD-15-6** (non-blocking `embed`) — do NOT block the
>    executor. Add F15 to `depends_on`. Embedding needs a **`model_id` config field** on the Message
>    API (OD-16-8). **Embedding-failure policy:** durable write is kept, index is skipped/retried, a
>    `MessageEvent::Error` is emitted — never blocks durability (OD-16-9).
> 7. **Memory snapshots** (`SessionRecord::{WorkingMemory,Observation,Reflection}`) traverse the **same**
>    append→flush→index path with `record_type` set. **`foundation_ai::agentic` module** creation is
>    owned by F01/F02 (precondition). Crash-before-flush loss is accepted (WAL deferred) — stated.

> Implements Decision 02. The **authoritative, append-only audit log** of every session record
> (`SessionRecord` from F01). Never compacted. Backed by `DocumentStore` (F04), indexed in
> `VectorStore` (F12) via `EmbeddingProvider` (F15), with write buffering, a flush valtron task, and
> `&self` pub/sub. Resolves **CRIT-02** (Broadcaster `&mut self`).

## WHY: Problem Statement

Every interaction (user/assistant/tool-result + memory snapshots) must be durably recorded, replayable,
searchable, and observable in real time — without per-record disk latency stalling the agent loop.
Decision 02: write-buffered append-only log + pub/sub + semantic search, all `&self` and `Arc`-shared.

## WHAT: Solution

```rust
pub struct MessageApi { inner: Arc<MessageInner> }  // cheap clone across valtron tasks

struct MessageInner {
    session_id: SessionId,
    doc_store: Arc<dyn DocumentStore>,          // F04 — persistence (append/scan/scan_from)
    write_buffer: ConcurrentQueue<SessionRecord>, // in-memory buffer (interior mutability)
    vector_store: Arc<dyn VectorStore>,         // F12 — semantic index (namespace = session)
    embedder: Arc<dyn EmbeddingProvider>,       // F15
    subscribers: /* &self pub/sub (see CRIT-02 fix) */,
    flush_task: Entry,                          // valtron flush task handle
}

impl MessageApi {
    pub fn append(&self, record: SessionRecord);                         // buffered, returns immediately
    pub fn recent(&self, n: usize) -> Vec<StoredRecord>;                 // DocumentStore.scan
    pub fn all(&self) -> impl Iterator<Item = StoredRecord>;             // scan_all
    pub fn scan_from(&self, id: &Scru128, n: usize) -> Vec<StoredRecord>;// F04 temporal cursor
    pub fn semantic_search(&self, query: &str, k: usize) -> Vec<StoredRecord>; // embed + VectorStore.query(ns)
    pub fn flush(&self) -> Result<(), AgenticError>;                     // drain buffer → disk (+ index)
    pub fn subscribe(&self) -> Receiver<MessageEvent>;                   // pub/sub
}
```

### Write buffering + flush task (Decision 02)

- `append` enqueues into `write_buffer` (returns immediately — no disk latency in the loop).
- A **flush valtron task** drains on: buffer ≥ capacity (default 50), time interval (default 5s), or
  session end. On flush: `doc_store.append` each record (with promoted columns from F04 via
  `PromotableDocument`), then index its text in `vector_store` (embed via F15).
- **Flush-on-end is awaited** — no record lost before shutdown (Decision 01).
- Each record gets a scru128 id (caller-supplied via F04's `append_with_id`) → stable, time-ordered.

### `&self` pub/sub — CRIT-02 fix

`Broadcaster<T>` needs `&mut self` (verified), unusable behind `Arc`. Resolve with a **`&self`-safe**
mechanism: a `ConcurrentQueue<Sender>` registry or a small `ThreadSafeBroadcaster` (interior
`Mutex<Vec<Sender>>` + `&self subscribe/broadcast`). Events:

```rust
pub enum MessageEvent { Appended { id: Scru128, variant: &'static str }, Flushed { count: usize }, Error { msg: String } }
```

Listeners (the observation-memory trigger F19, embedding indexer, external UI) subscribe; `append`/
`flush` broadcast. (Decision 02; resolves the design's pub/sub dependency.)

### Semantic recall

- On flush, each text-bearing record is embedded (F15) and inserted into `VectorStore` (F12) with
  `namespace = session_id` (isolation) + `record_type` metadata.
- `semantic_search(query)` embeds the query, `vector_store.query(vec, k, Some(session))`, loads the
  matched records by id from `DocumentStore` (fast `scan_from`/id lookup).

### Serialization (Decision 10 — folds F17)

- Records persist as JSON (NDJSON-friendly, F04). Arrow columnar export is **F17** (this feature
  produces JSON; F17 adds the Arrow batch path + promoted columns). Reference F17.

## Architecture

```mermaid
graph TD
    L[Agent loop] -->|append record| WB[write_buffer ConcurrentQueue]
    WB --> FT[flush task: ≥50 / 5s / end]
    FT --> DS[(DocumentStore F04)]
    FT --> EMB[EmbeddingProvider F15] --> VS[(VectorStore F12 ns=session)]
    L -->|recent/scan_from/semantic_search| Q[reads]
    Q --> DS
    Q --> VS
    AP[append/flush] -->|broadcast| SUB[subscribers: F19 trigger, UI]
```

## Fundamentals Documentation (zero-to-expert) — REQUIRED

Author `fundamentals/` covering: append-only audit logs & event sourcing; write buffering &
batched-flush (latency vs durability, flush triggers, flush-on-shutdown); **`&self` pub/sub** patterns
(why `&mut self` broadcasters fail behind `Arc`, ConcurrentQueue-of-senders, `ThreadSafeBroadcaster`);
semantic recall pipelines (embed→index→query→load); `Arc<Inner>` interior-mutability sharing across
valtron tasks; scru128 time-ordering for replay. (Task — see list.)

## HOW: Implementation Steps

1. `MessageApi`/`MessageInner` (Arc, `&self`); wire DocumentStore/VectorStore/EmbeddingProvider.
2. `append` → buffer; flush valtron task (triggers + drain + persist + index); flush-on-end await.
3. `&self` pub/sub mechanism (CRIT-02) + `MessageEvent`.
4. `recent`/`all`/`scan_from` via DocumentStore; `semantic_search` via embed+VectorStore+load.
5. caller-supplied scru128 ids (F04 `append_with_id`); promoted columns via `PromotableDocument`.
6. Tests: append→flush durability; flush triggers; flush-on-end loses nothing; pub/sub delivery;
   semantic_search recall; scan_from cursor; concurrent append+read; wasm build.

## Open Decisions

- **OD-16-1 — pub/sub mechanism:** `ThreadSafeBroadcaster` (interior Mutex) vs ConcurrentQueue-of-receivers.
  Rec: a small `&self` broadcaster helper (possibly elevated into `foundation_core::synca`).
        ConcurrentQueueOfRecevers is not that better ?

- **OD-16-2 — backpressure:** write_buffer full (ConcurrentQueue push fails) → block? force-flush?
  Rec: force-flush synchronously when full (never drop records).
        - We can have an innerQueue which has a bounded window to cache these and if reached and no one is consuming then this means we have problems and need to panic and report fast, something is broken and wrong.

- **OD-16-3 — crash before flush:** records in the buffer are lost on crash (no WAL). Accept (the gaps
  Q-04), or add periodic forced flush / WAL? Rec: short flush interval + flush-on-end; WAL later.
        Why not WAL it, this even makes the backpressure easier to deal with and crash safe, cacache is there for our needs.

- **OD-16-4 — index which records:** only text-bearing conversation + memory records get embedded
  (skip pure tool-call args?). Rec: embed user/assistant text + observation/reflection; skip raw blobs.
      Good

- **OD-16-5 — StoredRecord shape:** `{ id: Scru128, record: SessionRecord, created_at }`. Confirm.

## Target Files

- `backends/foundation_ai/src/agentic/message_api.rs` (new)
- coordinates F01 (`SessionRecord`/`Scru128`), F04 (DocumentStore + `append_with_id` + columns), F12
  (VectorStore), F15 (EmbeddingProvider), F17 (Arrow), F19 (subscriber)

## Tests

```bash
cargo test -p foundation_ai -- agentic::message_api
cargo build -p foundation_ai --no-default-features --features agentic --target wasm32-unknown-unknown
```

## Verification

```bash
cargo build -p foundation_ai
cargo build -p foundation_ai --no-default-features --features agentic --target wasm32-unknown-unknown
cargo clippy -p foundation_ai -- -D warnings
cargo test  -p foundation_ai -- agentic::message_api
```

## Done When

- Append-only, write-buffered store over DocumentStore; flush task (triggers + flush-on-end loses
  nothing); `&self` pub/sub (CRIT-02 resolved); semantic_search via VectorStore; recent/all/scan_from.
- `Arc`-shared, `&self`, concurrent-safe; builds native + wasm.
- OD-16-1..5 resolved; fundamentals authored.
