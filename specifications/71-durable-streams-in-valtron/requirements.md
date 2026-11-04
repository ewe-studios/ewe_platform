---
description: "Embed the Durable Streams contract — a resumable, offset-addressable, append-only log — directly into foundation_core::valtron as a DurableStreamStore trait, a DurableStreamReader/DurableStreamWriter pair, an idempotent producer, fork, and the HTTP/RPC surface, so every backend (netio, http, connectrpc, db, ai, foundation_log, deployment) spreads the capability from one primitive."
status: "planned"
priority: "high"
created: 2026-10-30
author: "Main Agent"
metadata:
  version: "1.0"
  estimated_effort: "large"
  tags: [valtron, durable-streams, stream, offset, resume, replay, idempotent-producer, fork, foundation-log, http, connectrpc, db, storage]
  stack_files: [rust.md]
  skills: [rust-clean-code, rust-valtron-usage]
  tools: []
has_features: true
has_fundamentals: true
builds_on: ""
related_specs:
  - "specifications/65-agent-rpc-serving-system"
  - "specifications/70-foundational-io-hardening"
  - "specifications/research/edge-data"
features:
  completed: 0
  uncompleted: 6
  total: 6
  completion_percentage: 0%
---

# Spec 71: Durable streams in valtron

_Written in ASD-STE100 Simplified Technical English (STE). Code and identifiers stay as they
are._

**Abbreviations in this file.** Hypertext Transfer Protocol (HTTP); Server-Sent Events (SSE);
Uniform Resource Locator (URL); identifier (ID); time to live (TTL); end of file (EOF);
JavaScript Object Notation (JSON); deterministic simulation testing (DST); application
programming interface (API); remote procedure call (RPC); Structured Query Language (SQL);
Durable Object (DO); Amazon Simple Storage Service (S3); pull request (PR); AeroSpace and Defence
Industries Association of Europe (ASD), the owner of the STE standard ASD-STE100. Durable Streams,
Caddy and Electric Cloud are product names. R2 and D1 are names of Cloudflare products, not
abbreviations. LMDB is a name, not an abbreviation.

## 1. Why this spec exists

The edge data research reviewed Durable Streams (source key `DS` in
[`durable-streams.md`](../research/edge-data/durable-streams.md)). It is an ordered, replayable,
resumable log over HTTP: a reader stores an opaque cursor and resumes from it, an idempotent
producer writes exactly once, a fork branches history without copying it, and an immutable prefix
makes the read path cacheable.

Today the platform has every piece of that scattered, and none of it reusable:

- `foundation_netio` and `foundation_http` stream SSE and bodies, but they carry no resume cursor.
  A dropped connection loses in-flight data.
- `foundation_log` (issue #155) plans an append-only log on R2 and S3, but its reader contract is
  not defined as a type.
- The agent-rpc turn stream (spec 65) needs resumption and replay, but there is no primitive to
  build it on.
- valtron already has `Stream::Wait` (park), `WakeableStream` (wake on readiness), `Stream::Spread`
  (a batch), `into_next_stream` / `into_future_stream` (async bridges), and `#[task]` (a state
  machine). These are exactly the pieces a durable stream needs. They are not named as one thing.

This spec names that thing and puts it in `foundation_core::valtron`, so every backend gets the
capability from one primitive instead of building it again.

## 2. The contract in valtron terms

A **durable stream** is an append-only, offset-addressable sequence of bytes with these properties:

1. **Append only.** Data written at a position never changes. New data appends at the tail.
2. **Opaque, ordered offsets.** A position is an `StreamOffset`. A reader never parses or builds
   one. It compares two offsets from one stream by byte order (lexicographic). Only the store
   builds offsets.
3. **A resume cursor.** A read returns a `StreamCursor`: the next offset, plus `up_to_date`
   ("more may come") and `closed` ("no more will ever come"). The reader stores the cursor and
   passes it back. This is the whole resumption mechanism.
4. **A refusal, never a gap.** A read at an offset that retention dropped is an error
   (`StoreError::Trimmed`), never a silent skip.
5. **Exactly-once writes.** A writer names itself with `(producer_id, epoch, seq)`. A stale epoch
   is fenced (`StoreError::Fenced`). A retried sequence is acknowledged without a duplicate.
6. **Fork without copy.** A fork shares the source prefix up to an offset and owns only its tail.
   Reads span the boundary transparently.

The map onto valtron:

| Durable Streams idea | valtron primitive today |
|---|---|
| Catch-up read (history) | `Stream::Next(D)` / `Stream::Spread(Vec<..>)` |
| Live tail (long-poll) | `Stream::Wait` + `WakeableStream` (wake on append) |
| Progress while pending | `Stream::Pending(P)` |
| Batch append / JSON array | `Stream::Spread` |
| Idempotent producer | a `#[task]` state machine (`producer_id` / `epoch` / `seq`) |
| Fork | a prefix-chain combinator (source prefix iterator + owned tail) |
| Async consumption | `into_next_stream()` / `into_future_stream()` (unchanged) |
| Storage backend | a `DurableStreamStore` trait injected through `Ctx` (rule 11) |

## 3. Design

### 3.1 Core types (feature 01)

All types are `no_std`-friendly. They live in `foundation_core::valtron::durable`.

```rust
// Opaque, lexicographically-ordered offset. Built only by the store.
#[derive(Clone, PartialEq, Eq, PartialOrd, Ord)]
pub struct StreamOffset(Box<[u8]>);   // Ord = byte-lexicographic, never parsed

// Where a read starts.
pub enum ReadFrom {
    Start,                       // beginning (the protocol's "-1")
    Offset(StreamOffset),        // resume at a stored cursor
    Tail,                        // skip history, watch only new data (the protocol's "now")
}

// The resume cursor a read returns.
pub struct StreamCursor {
    pub next: StreamOffset,      // the offset to pass on the next read
    pub up_to_date: bool,        // "more may come"
    pub closed: bool,            // "no more will ever come" (EOF)
}

// A bounded page the store returns for one read.
pub struct ReadPage {
    pub chunks: Vec<Chunk>,      // bounded; the limit is configuration, not a constant
    pub cursor: StreamCursor,
}

// A chunk of stream data. One message in JSON mode, raw bytes otherwise.
pub struct Chunk(Box<[u8]>);

// Stream metadata: content type, tail, TTL/expiry, closed.
pub struct StreamMeta {
    pub content_type: ContentType,
    pub tail: StreamOffset,
    pub closed: bool,
    pub expires: Option<Expiry>, // TTL (relative) or absolute, never both
}

// The acknowledgement an append returns. Two levels, both configuration.
pub enum AppendAck {
    Accepted { cursor: StreamCursor },  // durable at the tail
    Sealed { cursor: StreamCursor },    // durable in the sealed segment (object)
}
```

### 3.2 The store trait (feature 01)

Storage is injected, never global (CLAUDE.md rule 11). A backend implements
`DurableStreamStore`; the reader and writer are generic over it.

```rust
pub trait DurableStreamStore {
    fn create(&self, name: StreamName, meta: StreamMeta) -> Result<(), StoreError>;
    fn head(&self, name: &StreamName) -> Result<StreamMeta, StoreError>;
    fn append(&self, name: &StreamName, chunk: Chunk, ack: AckLevel,
              producer: Option<Producer>) -> Result<AppendAck, StoreError>;
    fn read(&self, name: &StreamName, from: ReadFrom, limit: usize)
        -> Result<ReadPage, StoreError>;
    fn close(&self, name: &StreamName) -> Result<(), StoreError>;
    fn delete(&self, name: &StreamName) -> Result<(), StoreError>;
    fn fork(&self, name: StreamName, from: &StreamName, at: Option<StreamOffset>,
            meta: StreamMeta) -> Result<(), StoreError>;
}

pub enum StoreError {
    NotFound,        // the stream does not exist
    Trimmed,         // the offset was dropped by retention (a refusal, never a gap)
    Fenced,          // a stale producer epoch wrote
    Conflict,        // a content-type or sequence mismatch
    Closed,          // an append to a closed stream
    Storage(Box<dyn core::error::Error>),  // the backend failed
}
```

The store is read from the context bundle: `ctx.get::<Env>()` returns the host's
`DurableStreamStore` (or a mock in tests). A missing store is a typed error, never a silent
fallback.

### 3.3 The reader (feature 02)

`DurableStreamReader<Store>` holds a store and a cursor. It implements `StreamIterator` with
`D = Chunk` and `P = ReaderProgress`, and `WakeableStream`.

- Catch-up: the reader pulls bounded pages and yields `Stream::Next(chunk)` for each chunk, or
  `Stream::Spread(chunks)` for one page. At the tail it sets `up_to_date` and, in live mode, parks
  with `Stream::Wait` (long-poll). `closed` yields the terminal `Stream::Next` for the last chunk
  then `None`.
- Live mode: the store registers a notifier on append. The reader's `wake` forwards to it, so the
  awaitable bridges (`into_next_stream`, `into_future_stream`) wake exactly when new data lands.
  A trim before the cursor surfaces `StoreError::Trimmed` as a terminal error, never a skip.
- `Stream::Pending(ReaderProgress)` carries the pending context (for example `Connecting`,
  `Trimming`), matching how `SseProgress` already works in `foundation_netio`.

The reader never blocks. Blocking stays at the boundary (`blocking_boundary!`), and only the
host's entry point uses it.

### 3.4 The writer and the idempotent producer (feature 03)

`DurableStreamWriter<Store>` appends and returns an `AppendAck`. The idempotent producer is a
`#[task]` state machine over `(producer_id, epoch, seq)`:

- A new epoch starts at `seq = 0`.
- The store keeps the last accepted `(stream, producer_id, epoch, seq)`.
- A retried sequence returns `AppendAck::Accepted` without a second write (deduplicated).
- A stale epoch returns `StoreError::Fenced`; the writer stops, it does not retry.
- A sequence gap returns `StoreError::Conflict`.

The store must commit the producer state and the append atomically where the backend can
(DO SQLite, a single LMDB transaction). Where it cannot (a file store), the failure mode is a
false `Conflict`, not a duplicate, and the writer recovers by incrementing its epoch. This matches
the Caddy file-store behaviour the Durable Streams repo documents.

### 3.5 Fork (feature 04)

`fork_at(offset)` builds a reader that chains the source's prefix with the fork's own tail, so the
reader sees one stream. The fork is independent: its own URL/name, TTL, closure and deletion.

- Offsets at or before the fork resolve against the source without copying; offsets after resolve
  against the fork's appends.
- Deleting a source with live forks soft-deletes it: reads through the forks keep working, the
  source name is blocked from re-creation, and when the last fork is deleted the source is
  garbage-collected, cascading up the chain.
- The reference count and soft-delete are store bookkeeping, not reader state.

### 3.6 Storage backends (feature 05)

`DurableStreamStore` gets five implementations. Each uses an existing seam:

| Backend | Seam | Target |
|---|---|---|
| In-memory | `BTreeMap` + a notifier | DST and tests (seeded, replayable) |
| File + LMDB | a native log file + LMDB metadata | native `foundation_log` tail |
| R2 / S3 segments | `foundation_db::BlobStore` + `foundation_log` key layout | the sealed tier |
| DO SQLite | `DoSqlStore` (`QueryStore`/`KeyValueStore`) | spec 65 T0 |
| D1 | `D1WasmStorage` | spec 65 T1 |

The store is the only place that builds offsets. The offset scheme is per-backend (a monotonic
sequence, a padded last-sequence, an epoch-prefixed key) and never leaks into the reader.

### 3.7 The HTTP and RPC surface (feature 06)

The durable contract maps onto the existing HTTP and RPC seams, so any `foundation_http` or
`foundation_connectrpc` service exposes a resumable stream:

| Protocol verb | valtron surface |
|---|---|
| `PUT` create (idempotent) | `store.create`; `200` on a match, `409` on a mismatch |
| `POST` append / close | `writer.append`; `Stream-Closed: true` closes atomically |
| `GET` read | `reader` → `Stream::Next` / `Stream::Spread` (catch-up) |
| `GET ?live=long-poll` | `reader` parks on `Stream::Wait`; 200 with data, 204 on timeout |
| `GET ?live=sse` | `reader` into `into_next_stream()`; `data` + `control` events |
| `HEAD` metadata | `store.head`; `Cache-Control: no-store` |
| `DELETE` | `store.delete` |

- The cursor headers (`Stream-Next-Offset`, `Stream-Up-To-Date`, `Stream-Closed`) map to
  `StreamCursor` fields.
- `Stream::Spread` maps to JSON-mode array flattening (one delivery, several messages).
- Catch-up responses are immutable and cacheable (`Cache-Control`, `ETag`, `If-None-Match`/`304`);
  the ETag varies with closure so a stale `304` never hides EOF.
- A `Stream-Cursor` on live responses enables CDN request collapsing; this is a deployment
  property, not new code.

## 4. Features

| # | Feature | Delivers | Depends on | Size |
|---|---|---|---|---|
| 01 | [Durable stream types](./features/01-durable-stream-types/feature.md) | `StreamOffset`, `StreamCursor`, `StreamMeta`, `ReadPage`, `AppendAck`, `StoreError`, the `DurableStreamStore` trait | — | M |
| 02 | [Resumable reader](./features/02-resumable-reader/feature.md) | `DurableStreamReader` (catch-up + live tail) as `StreamIterator` + `WakeableStream` | 01 | M |
| 03 | [Idempotent producer](./features/03-idempotent-producer/feature.md) | `DurableStreamWriter` + the `#[task]` producer state machine (epoch fencing, dedup) | 01 | M |
| 04 | [Fork and retention](./features/04-fork-and-retention/feature.md) | `fork_at` combinator, soft-delete refcount, trim-as-refusal | 02, 03 | M |
| 05 | [Storage backends](./features/05-storage-backends/feature.md) | In-memory, file+LMDB, R2/S3, DO SQLite, D1 implementations | 01 | L |
| 06 | [HTTP and RPC surface](./features/06-http-rpc-surface/feature.md) | The verbs, headers, JSON mode, live modes, caching on `foundation_http`/`foundation_connectrpc` | 02, 03 | L |

Order: 01 → (02, 03) → 04, and 05, 06 after 02 and 03.

## 5. Rules

These come from `CLAUDE.md` and `.agents/AGENTS.md`. Read both before you start.

1. **valtron always.** Never tokio, except valtron's own cross-runtime tests.
2. **Tests live in `tests/`**, registered in `tests/**/mod.rs`. No inline `#[cfg(test)]` modules
   (CLAUDE.md rule 7).
3. **Side effects are injected, never global** (rule 11). The store, the clock and the ID source
   come from the `Ctx` bundle. The DST tests use a simulated clock, a seeded RNG and an in-memory
   store, so any run replays from its seed.
4. **Limits are configuration** (rule 8). Chunk size, page limit, retention window, long-poll
   timeout, seal threshold and acknowledgement level are fields with documented defaults that name
   their source (nginx, Go `net/http`, Caddy, the Durable Streams conformance suite).
5. **Stream bodies, never buffer them.** A read is a `StreamIterator`, consumed chunk-by-chunk.
   The store never materialises a whole stream.
6. **Async all the way; valtron composes with it.** The store is `async` where it waits; the
   reader is a valtron `StreamIterator` that the awaitable bridges turn into a future or a
   `futures::Stream`.
7. **A refusal, never a gap.** `StoreError::Trimmed` is the only answer to a dropped offset.
8. **Every bug you notice goes in `docs/bug-backlog.md`** in the same change.

## 6. Verification protocol

```bash
export CARGO_PROFILE_DEV_DEBUG=0 CARGO_INCREMENTAL=0

# Per crate you change: fmt, build, clippy, tests, wasm where the crate has it.
python3 scripts/ci/ci.py run foundation_core --profile dev

# Whole workspace, before every push.
cargo check --workspace --all-targets --profile dev
```

- Each feature names its exact commands and its tests.
- A test that can hang has a time limit inside it (`#[valtron_test(timeout = ..)]`).
- Tests that involve threads, sockets or timing pass 20 runs in a row; report the count.
- The DST tests run from a seed and are replayable (rule 11).

## 7. Out of scope

- A public CDN product. The immutable-prefix and cursor properties are kept; a public cache is not
  a target.
- Ten client libraries. One Rust implementation in valtron.
- TanStack DB / StreamDB as a dependency. The reactive query engine is application glue.
- Authentication. The Durable Streams protocol leaves it out; our HTTP surface reuses the
  existing auth seams.
- A cross-stream total order. Offsets are per-stream.

## 8. Done for the whole spec

- [ ] `foundation_core::valtron::durable` exists with the core types and the `DurableStreamStore`
      trait.
- [ ] `DurableStreamReader` implements `StreamIterator` + `WakeableStream` and resumes from a
      cursor byte-exactly (no skip, no duplicate).
- [ ] The idempotent producer fences a stale epoch and deduplicates a retried sequence.
- [ ] `fork_at` spans the fork boundary transparently, and a soft-deleted source stays readable
      through its forks.
- [ ] Five storage backends pass the same DST scenario (in-memory, file+LMDB, R2/S3, DO SQLite,
      D1).
- [ ] The HTTP and RPC surface exposes the six verbs and the cursor headers, and an SSE tail parks
      on `Stream::Wait` instead of polling.
- [ ] No `#[cfg(test)]` module in the changed source files. No ignored `Result`s.
