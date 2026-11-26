---
description: "Embed the Durable Streams protocol as a transport-neutral, conformance-tested core in foundation_core::durable. Add an HTTP adapter, a ConnectRPC adapter, a Rust client and a conformance gate. Then wrap the core in foundation_core::valtron::durable with Stream::Wait, WakeableStream, Stream::Spread and #[task], so the core works with or without valtron."
status: "planned"
priority: "high"
created: 2026-10-30
author: "Main Agent"
metadata:
  version: "2.0"
  estimated_effort: "large"
  tags: [valtron, durable-streams, stream, offset, resume, replay, idempotent-producer, stream-seq, fork, foundation-log, http, connectrpc, client, conformance, db, storage]
  stack_files: [rust.md]
  skills: [rust-clean-code, rust-valtron-usage]
  tools: []
has_features: true
has_fundamentals: true
builds_on: ""
related_specs:
  - "specifications/83-agent-rpc-serving-system"
  - "specifications/completed/70-foundational-io-hardening"
  - "specifications/research/edge-data"
features:
  completed: 0
  uncompleted: 8
  total: 8
  completion_percentage: 0%
---

# Spec 71: Durable streams in valtron

_Written in ASD-STE100 Simplified Technical English (STE). Code and identifiers stay as they
are._

**Abbreviations in this file.** Hypertext Transfer Protocol (HTTP); Server-Sent Events (SSE);
Uniform Resource Locator (URL); identifier (ID); time to live (TTL); end of file (EOF);
JavaScript Object Notation (JSON); Entity Tag (ETag); Content Delivery Network (CDN);
deterministic simulation testing (DST); application programming interface (API); remote procedure
call (RPC); Structured Query Language (SQL); Durable Object (DO); Amazon Simple Storage Service
(S3); pull request (PR); AeroSpace and Defence Industries Association of Europe (ASD), the owner
of the STE standard ASD-STE100. Durable Streams, Caddy, Electric Cloud and ConnectRPC are product
names. R2 and D1 are names of Cloudflare products, not abbreviations. LMDB is a name, not an
abbreviation. RFC 3339 is an Internet date format.

**Glossary.**

| Term | Meaning |
|---|---|
| Core | The protocol implementation. It has no valtron dependency. |
| Adapter | A thin layer that maps the core to one transport or runtime. |
| Valtron layer | The optional wrapper that turns the core into a valtron stream. |
| Conformance | Passing the reference server suite and the reference client suite. |
| Resume cursor | The opaque server position that a reader stores and sends back. |
| Collapse cursor | The `Stream-Cursor` value for CDN request collapsing. It is not the resume cursor. |
| Producer triple | `Producer-Id`, `Producer-Epoch` and `Producer-Seq`. |
| `Stream-Seq` | An optional opaque writer sequence for application ordering. |
| Soft-delete | A deleted stream that stays readable through its forks. |

---

## 1. Why this spec exists

The edge data research reviewed Durable Streams (source key `DS` in
[`durable-streams.md`](../research/edge-data/durable-streams.md)). It is an ordered, replayable,
resumable log over HTTP. A reader stores an opaque cursor and resumes from it. An idempotent
producer writes exactly once. A fork branches history without copying it. An immutable prefix
makes the read path cacheable.

Today the platform has the pieces, but not one reusable contract:

- `foundation_netio` and `foundation_http` stream SSE and bodies, but they carry no resume cursor.
- `foundation_log` (issue #155) plans an append-only log on R2 and S3, but its reader contract is
  not a type.
- The agent-rpc turn stream (spec 83) needs resumption and replay, but it has no primitive.
- valtron has `Stream::Wait`, `WakeableStream`, `Stream::Spread`, `into_next_stream`,
  `into_future_stream` and `#[task]`. These are the pieces a durable stream needs. They are not
  named as one thing.

This spec names the contract. It puts the protocol core in `foundation_core::durable`. It puts
the optional valtron wrapper in `foundation_core::valtron::durable`.

**The core rule.** The protocol core must pass conformance without valtron. The valtron layer
wraps the core. It must not change protocol outcomes.

## 2. The contract

A **durable stream** is an append-only, offset-addressable sequence of bytes with these
properties:

1. **Append only.** Data written at a position never changes. New data appends at the tail.
2. **Opaque, ordered offsets.** A position is a `StreamOffset`. A reader never parses or builds
   one. It compares two offsets from one stream by byte order (lexicographic). Only the store
   builds offsets.
3. **A resume cursor.** A read returns a `ResumeCursor`: the next offset, plus `up_to_date`
   ("more may come") and `closed` ("no more will ever come"). The reader stores the cursor and
   passes it back.
4. **A refusal, never a gap.** A read at an offset that retention dropped returns
   `StoreError::Trimmed`. A read on a soft-deleted stream returns `StoreError::Gone`.
5. **Exactly-once writes.** A producer names itself with `(producer_id, epoch, seq)`. A stale
   epoch is fenced. A retried sequence is acknowledged without a duplicate.
6. **Writer ordering.** A writer may send `Stream-Seq`, an opaque value. The store compares it
   byte-lexicographically. A value at or below the last accepted value is a
   `StoreError::StreamSeqConflict`. The store updates the value only after a successful append.
7. **Fork without copy.** A fork shares the source prefix up to an offset and owns only its tail.
   Reads span the boundary transparently.
8. **Durable closure.** A closed stream accepts no appends. Its data stays readable. Closure is
   monotonic and observable as EOF.
9. **JSON mode.** An `application/json` stream preserves message boundaries. It flattens one
   array level on append. It returns an array on read.

## 3. Architecture

### 3.0 The layer model

```mermaid
flowchart TB
  V[Valtron layer: StreamIterator, Wait, WakeableStream, #[task]]
  A[Transport adapters: HTTP, ConnectRPC]
  C[Protocol core: DurableService]
  S[Store: DurableStreamStore]
  CL[Client core: foundation_netio::durable]
  CV[Valtron client layer]
  V --> C
  A --> C
  C --> S
  CV --> CL
  CL -. shared types .-> C
```

_Caption: The core serves transports and valtron, so it works without valtron._

The core is `foundation_core::durable`. The valtron wrapper is
`foundation_core::valtron::durable`. The client core is `foundation_netio::durable`.

Rules:

- The core has no valtron type, trait or executor. It compiles and tests without valtron.
- The core has no HTTP type, status code, header, ETag, SSE frame or CDN cursor.
- The valtron layer wraps the core. It adds waits, signals, readiness and spreads.
- Conformance tests the core and its transport adapters. It does not require the valtron layer.

### 3.1 Core types (feature 01)

All types are `no_std`-friendly. They live in `foundation_core::durable`.

```rust
// Opaque, lexicographically-ordered offset. Built only by the store.
#[derive(Clone, PartialEq, Eq, PartialOrd, Ord)]
pub struct StreamOffset(Box<[u8]>);

// Opaque writer sequence. The writer supplies it; the store compares it.
#[derive(Clone, PartialEq, Eq, PartialOrd, Ord)]
pub struct StreamSeq(Box<[u8]>);

// Parsed media type and validated stream path.
pub struct ContentType(Box<str>);
pub struct StreamName(Box<str>);
pub struct ProducerId(Box<str>);

// A chunk of stream data. In JSON mode it is one message; otherwise it is bytes.
pub struct Chunk(Box<[u8]>);

pub enum ReadFrom { Start, Offset(StreamOffset), Tail }

pub struct ResumeCursor {
    pub next: StreamOffset,
    pub up_to_date: bool,
    pub closed: bool,
}

pub struct ReadRequest { pub from: ReadFrom, pub limit: ReadLimit }
pub struct ReadLimit { pub max_bytes: Option<usize>, pub max_chunks: Option<usize> }
pub struct ReadPage { pub chunks: Vec<Chunk>, pub cursor: ResumeCursor }

pub enum Expiry {
    Ttl(core::time::Duration),               // sliding window
    Absolute(/* RFC 3339 timestamp */),
}

pub struct StreamHead {
    pub content_type: ContentType,
    pub tail: StreamOffset,
    pub closed: bool,
    pub expires: Option<Expiry>,
}

pub struct CreateSpec {
    pub content_type: Option<ContentType>,   // None = inherit or default
    pub expires: Option<Expiry>,
    pub closed: bool,
    pub initial: Option<Chunk>,
    pub fork: Option<ForkSpec>,
}

pub struct ForkSpec {
    pub source: StreamName,
    pub at: Option<StreamOffset>,            // None = source tail
    pub sub_offset: u32,                     // 0 = absent
}

pub struct Producer { pub id: ProducerId, pub epoch: u64, pub seq: u64 }

pub struct AppendRequest {
    pub chunk: Chunk,
    pub content_type: Option<ContentType>,   // None only for close-only
    pub stream_seq: Option<StreamSeq>,
    pub producer: Option<Producer>,
    pub close: bool,
}

pub enum AppendKind { Created, Duplicate }

pub struct AppendOutcome {
    pub kind: AppendKind,
    pub cursor: ResumeCursor,
    pub producer_seq: Option<u64>,           // highest accepted
    pub closed: bool,
}

pub struct CreateOutcome { pub created: bool, pub head: StreamHead }
pub struct CloseRequest { pub final_chunk: Option<Chunk>, pub producer: Option<Producer> }
pub struct DeleteOutcome { pub soft_deleted: bool }

pub enum AckLevel { Accepted, Sealed }       // internal config, never on the wire

pub enum StoreError {
    NotFound,
    Gone,
    Trimmed,
    Fenced { current_epoch: u64 },
    InvalidEpochSeq { seq: u64 },
    SequenceGap { expected: u64, received: u64 },
    StreamSeqConflict { last: Option<StreamSeq> },
    ContentTypeMismatch { expected: ContentType, got: ContentType },
    ConfigConflict,
    Closed { tail: StreamOffset },
    BadRequest(BadRequestKind),
    RateLimited { retry_after: Option<core::time::Duration> },
    Unsupported,
    Storage(Box<dyn core::error::Error + Send + Sync + 'static>),
}

pub enum BadRequestKind { Header, Body, Offset, Json, Producer, Fork }
```

Rules:

- `StreamOffset`, `StreamSeq` and `ResumeCursor` are the only position types. A reader never
  parses an offset.
- `ContentType` parses a media type. Compare the type and subtype case-insensitively. Ignore
  parameters. The default is `application/octet-stream`.
- `StreamName` is a validated URL path. It rejects `.` and `..` segments and control characters.
- `Chunk` is a delivery unit. For non-JSON streams, concatenation is byte-exact. For JSON
  streams, one chunk is one message.
- `AckLevel` is an internal durability choice. It never changes the wire response.

### 3.2 The store trait (feature 01)

Storage is injected, never global (CLAUDE.md rule 11). A backend implements
`DurableStreamStore`.

```rust
pub trait DurableStreamStore {
    async fn create(&self, name: StreamName, spec: CreateSpec)
        -> Result<CreateOutcome, StoreError>;
    async fn head(&self, name: &StreamName) -> Result<StreamHead, StoreError>;
    async fn append(&self, name: &StreamName, req: AppendRequest)
        -> Result<AppendOutcome, StoreError>;
    async fn read(&self, name: &StreamName, req: ReadRequest)
        -> Result<ReadPage, StoreError>;
    async fn close(&self, name: &StreamName, req: CloseRequest)
        -> Result<AppendOutcome, StoreError>;
    async fn delete(&self, name: &StreamName) -> Result<DeleteOutcome, StoreError>;
    async fn fork(&self, name: StreamName, spec: ForkSpec)
        -> Result<CreateOutcome, StoreError>;
    fn notifier(&self, name: &StreamName) -> Option<Notifier>;
}
```

The store owns:

- Offsets and the offset scheme.
- Producer state per `(stream, producer_id)`.
- `Stream-Seq` state per stream.
- Reference counts and soft-delete state.
- Last-access time for the sliding TTL.
- The append notifier.
- Backend atomicity.

The store does not own:

- HTTP status codes, headers, ETag, `Cache-Control`, CORS or the CDN cursor.
- SSE framing or JSON wire serialization. It may own JSON message boundaries.
- Valtron scheduling.

**Async and injection.** The trait uses native `async fn`. It is not object-safe. The reader and
writer are generic over `Store: DurableStreamStore`. `DurableStream` is the host's concrete store
enum. `Ctx` holds `Arc<DurableStream>`, not `Arc<dyn DurableStreamStore>`. Tests use a concrete
mock type. A host with several backends uses an enum that implements the trait.

```rust
pub enum DurableStream {
    File(FileStore),
    Memory(InMemoryStore),
    Do(DoSqlStore),
}

impl DurableStreamStore for DurableStream {
    async fn head(&self, name: &StreamName) -> Result<StreamHead, StoreError> {
        match self {
            Self::File(s) => s.head(name).await,
            Self::Memory(s) => s.head(name).await,
            Self::Do(s) => s.head(name).await,
        }
    }
    // One match arm per method.
}
```

`StoreError` is `Send + Sync + 'static`, so it can cross a valtron task boundary. A `!Send`
backend stays on one thread.

### 3.3 The protocol service

`DurableService<Store>` is the protocol core. It calls the store and applies the protocol
semantics. The transport adapters and the valtron layer call it.

Responsibilities:

- Create config matching, including closure status, TTL/expiry and fork details.
- Append content-type matching, JSON validation, one-level flatten and empty-array rules.
- Producer validation and idempotent acknowledgement.
- `Stream-Seq` ordering, after producer validation.
- Closure rules, including append-and-close and close-only idempotency.
- Read page limits and `up_to_date`/`closed` classification.
- Live wait: `wait_for_data(name, offset, timeout) -> WaitOutcome`.
- Fork creation, TTL inheritance and soft-delete.
- `StoreError` to protocol outcome mapping. It does not choose a transport status.

`WaitOutcome` is `Data`, `Timeout` or `Closed`. The HTTP adapter maps `Timeout` to `204` and
`Closed` to `204` plus `Stream-Closed`. The ConnectRPC adapter maps them to its own status.

### 3.4 Reader semantics (feature 02)

The core reader is a page reader. The valtron reader wraps it.

**State rules.**

| Position | `up_to_date` | `closed` | Action |
|---|---|---|---|
| Partial page of a closed stream | `false` | `false` | Continue catch-up. |
| Tail of an open stream | `true` | `false` | Wait in live mode. |
| Tail of a closed stream | `true` | `true` | End. Do not wait. |
| Data before closure | `true` or `false` | `false` | Continue, then read the tail. |

- `Stream-Closed` appears only when the response reaches the final offset at response time.
- A partial page of a closed stream omits `closed`.
- Closure can need one more read at the tail. That read returns an empty page with `closed`.
- `ReadFrom::Tail` is the protocol's `offset=now`. Catch-up returns an empty page with the tail
  offset and `up_to_date`. Long-poll waits for new data. SSE starts at the tail.
- A closed stream at the tail returns immediately. It never waits.
- A read of a future offset returns an empty page at the tail. The server may reject it.

**Valtron reader (optional).**

`DurableStreamReader<Store>` implements `StreamIterator` with `D = Chunk` and
`P = ReaderProgress`, and `WakeableStream`.

- A page yields `Stream::Next` per chunk, or `Stream::Spread` for one page.
- A pending core call yields `Stream::Pending(ReaderProgress)`.
- An open tail yields `Stream::Wait` in live mode. The notifier wakes it.
- A closed tail yields the final chunk then `None`.
- A trim yields a terminal error. It never skips.
- `ReaderProgress` is `Connecting`, `Trimming` or `Live`.
- The reader never blocks. Blocking stays at the boundary.

**Client reader.** The client core exposes the same state machine. Its valtron wrapper turns it
into a `StreamIterator`. The client also supports `LiveMode::Auto`: catch-up, then SSE, with
long-poll fallback.

### 3.5 Writer, producer and `Stream-Seq` (feature 03)

`DurableStreamWriter<Store>` appends and returns an `AppendOutcome`.

**Producer ladder.**

- All three producer headers are present or absent. Partial headers return `BadRequest`.
- `Producer-Id` is non-empty. `Producer-Epoch` and `Producer-Seq` are decimal integers at most
  `2^53 - 1`.
- Unknown producer: the first request must use `seq = 0`.
- `epoch < state.epoch` returns `Fenced { current_epoch }`.
- `epoch > state.epoch` with `seq != 0` returns `InvalidEpochSeq { seq }`.
- Same epoch, `seq <= last_seq` returns `AppendKind::Duplicate` with the highest accepted
  `Producer-Seq`.
- Same epoch, `seq == last_seq + 1` returns `AppendKind::Created`.
- `seq > last_seq + 1` returns `SequenceGap { expected, received }`.
- Validate and append under one lock per `(stream, producer_id)`.
- Commit producer state with the append where the backend can. Otherwise document the false
  `SequenceGap` recovery.
- Producer state TTL is 7 days in memory. Persistent stores retain it with the data.
- A failed append does not advance producer state. A gap can be filled after the earlier sequence
  arrives.
- A duplicate of the closing tuple returns `AppendKind::Duplicate` with `closed`.
- A fork does not inherit producer state.

**`Stream-Seq`.**

| Layer | Header | Scope | Comparison | Failure |
|---|---|---|---|---|
| Retry safety | `Producer-Id`, `Producer-Epoch`, `Producer-Seq` | Per stream and producer | Numeric, per epoch | `Fenced`, `InvalidEpochSeq`, `SequenceGap` |
| Writer ordering | `Stream-Seq` | Per stream | Byte-lexicographic string | `StreamSeqConflict` |

The store validates producer headers first. A retry with an older `Stream-Seq` is deduplicated,
not refused. The store updates `Stream-Seq` only after a successful append. A fork does not
inherit it.

**Valtron writer (optional).** `DurableStreamWriter` and the producer `#[task]` wrap the core.
The task states are `Idle`, `AwaitingAck { seq }`, `Fenced` and `Done`. `Fenced` is terminal.

### 3.6 Fork, TTL and retention (feature 04)

**Fork.**

- `Stream-Forked-From` is a path on the same server.
- `Stream-Fork-Offset` defaults to the source tail. Beyond the tail returns `BadRequest`.
  Malformed returns `BadRequest`.
- `Stream-Fork-Sub-Offset` is a non-negative integer. Default `0`. It counts JSON messages or
  binary bytes. Overshoot returns `BadRequest`. It requires `Stream-Forked-From`.
- Content type is inherited when omitted. A mismatch returns `ContentTypeMismatch`.
- A closed source can be forked. The fork starts open.
- A missing source returns `NotFound`. A soft-deleted source returns `Conflict` at the adapter.
- A target in use returns `ConfigConflict`.
- Fork creation is idempotent. Matching config returns `CreateOutcome { created: false }`.
  The match includes `Forked-From`, `Fork-Offset` and `Fork-Sub-Offset`.
- A fork does not inherit producer or `Stream-Seq` state.
- Reads stitch source and fork data without offset translation.
- Long-poll and SSE on a fork wait only for fork appends.
- Source appends after the fork are invisible to the fork.

The nine-row fork TTL table is in feature 04, section 2.1.

**Soft-delete.**

- `GET`, `HEAD`, `POST` and `DELETE` on a soft-deleted stream return `Gone` (`410`).
- `PUT` on its path returns `ConfigConflict` (`409`).
- A fork from it returns `Conflict` (`409`).
- Forks keep reading through it.
- The last fork delete cascades cleanup up the chain. Cleanup may be asynchronous.

**TTL and expiry.**

- `Stream-TTL` is a non-negative integer in decimal notation. It has no leading zeros, plus
  signs, decimal points or scientific notation.
- `Stream-Expires-At` is an RFC 3339 timestamp.
- Both headers on one request return `BadRequest`.
- TTL is a sliding window. Each read and write resets it. `HEAD` does not reset it.
- Live reads reset it when the server starts the request.
- A stream with active live readers does not expire.
- Expiry with forks soft-deletes the stream. Expiry without forks deletes it.

**Retention.** A store may drop old data. A read at a dropped offset returns `Trimmed`
(`410`). Retention is optional. The reference server has none.

### 3.7 Storage backends (feature 05)

The five backends and their seams are in feature 05. Each backend implements every store method.
Each stores producer, `Stream-Seq`, TTL and fork state. The offset scheme is private. Do not
materialise a whole stream. Reads are paged.

### 3.8 HTTP surface (feature 06)

The HTTP adapter maps `DurableService` to HTTP. It owns every wire rule.

**Verbs.**

| Verb | Request | Success | Main errors |
|---|---|---|---|
| `PUT` create | headers + optional body | `201` new, `200` idempotent | `400`, `404` fork source, `409` |
| `POST` append | body, `Content-Type`, optional headers | `204` plain, `200` producer | `400`, `404`, `409`, `410`, `413`, `429` |
| `POST` close | `Stream-Closed: true`, optional body | `204` | `400`, `404`, `409`, `410` |
| `GET` read | `offset`, optional `live`, `cursor` | `200`, `204` long-poll, `304` | `400`, `404`, `410`, `429` |
| `HEAD` | no body | `200` | `404`, `410`, `429` |
| `DELETE` | no body | `204` | `404`, `410` |
| Other | | `405` | |

**Headers.** Request: `Content-Type`, `If-None-Match`, `Accept-Encoding`, `Stream-Seq`,
`Stream-TTL`, `Stream-Expires-At`, `Stream-Closed`, `Producer-Id`, `Producer-Epoch`,
`Producer-Seq`, `Stream-Forked-From`, `Stream-Fork-Offset`, `Stream-Fork-Sub-Offset`.

Response: `Content-Type`, `Content-Encoding`, `Vary`, `ETag`, `Cache-Control`, `Location`,
`Stream-Next-Offset`, `Stream-Cursor`, `Stream-Up-To-Date`, `Stream-Closed`, `Stream-TTL`,
`Stream-Expires-At`, `Producer-Epoch`, `Producer-Seq`, `Producer-Expected-Seq`,
`Producer-Received-Seq`, `Stream-SSE-Data-Encoding`, `X-Content-Type-Options`,
`Cross-Origin-Resource-Policy`, plus CORS headers and `Access-Control-Expose-Headers`.

**Error precedence.** For append: closed, then content type, then producer, then `Stream-Seq`,
then JSON. For create: TTL/expiry, then fork headers, then store config. For read: not found,
then soft-deleted, then offset format.

**Read rules.**

- `offset` defaults to `-1`. `-1` is the start. `now` is the tail.
- `offset` must not be empty, repeated or malformed.
- Long-poll and SSE require `offset`.
- A catch-up read at the tail returns `200` with an empty body (`[]` for JSON).
- `Stream-Up-To-Date: true` only when the response has all data available at response time.
  Omit it on a capped partial page.
- `Stream-Closed: true` only when the stream is closed and the response reaches the final offset.
- `Stream-Next-Offset` is the next offset.
- Long-poll `204` includes `Stream-Next-Offset`, `Stream-Up-To-Date`, and `Stream-Cursor` when
  open. A closed stream at the tail returns immediately with `Stream-Closed`.
- A `304` has no body and the new ETag.
- `Stream-Seq` regression returns `409`.
- A fork does not inherit the value.

**JSON rules.** Validate JSON. Flatten one array level. One element is one message. Empty array
on `POST` is `400`. Empty array on `PUT` is valid. A read returns one JSON array. A fork read
across the boundary returns one array. `offset=now` returns `[]`.

**SSE rules.** `Content-Type: text/event-stream`. One `data` event per read batch. A `control`
event after every `data` event. Control fields are camelCase: `streamNextOffset`,
`streamCursor`, `upToDate`, `streamClosed`. `streamNextOffset` is always present. `streamCursor`
is present when open. `upToDate` marks a caught-up open stream. `streamClosed` is the final
event; then close the connection. Binary streams use base64 and
`Stream-SSE-Data-Encoding: base64`. Remove `\n` and `\r` before decoding. The server should
recycle connections about every 60 seconds. The client reconnects from the last
`streamNextOffset`, unless closed.

**Caching and cursor.** `ETag` is required on every `GET` except `offset=now`. The reference
format is `"<base64(path)>:<start>:<end>[:c]"`. `:c` changes the ETag on closure. A matching
`If-None-Match` returns `304`. Catch-up reads are cacheable with
`Cache-Control: public, max-age=60, stale-while-revalidate=300` (or `private`). `HEAD` returns
`no-store`. `offset=now` returns `no-store`. SSE returns `no-cache`. Clients order query
parameters lexicographically. `Stream-Cursor` is required on live responses when open and may be
omitted when closed. The cursor is an interval number: default interval 20 seconds, epoch
2024-10-09T00:00:00Z. A collision adds random jitter so the value strictly increases. Cursors
never go backwards.

**Security.** `X-Content-Type-Options: nosniff` on all responses. `Cross-Origin-Resource-Policy`
set. `Cache-Control: no-store` on `HEAD` and sensitive data. `429` for rate limits. Path
validation. Authentication at the existing seam. TLS in production. CORS exposes the stream
headers to browsers.

**Small wire rules.** `201` includes `Location`. `Content-Type` defaults to
`application/octet-stream`. `Stream-Closed` counts only the value `true` (case-insensitive).
Other values are ignored. `POST` with a body requires `Content-Type`. Close-only ignores it.
The standalone port is `4437`. Compression is optional; set `Vary: accept-encoding`.

### 3.9 ConnectRPC surface

The ConnectRPC adapter maps the same `DurableService` operations to RPC. It owns method names,
metadata, error codes and stream envelopes.

| Operation | ConnectRPC |
|---|---|
| Create | `Create` unary |
| Append | `Append` unary |
| Read | `Read` server stream |
| Live read | `Subscribe` server stream |
| Head | `Head` unary |
| Close | `Close` unary |
| Delete | `Delete` unary |
| Fork | `Fork` unary |

A live read is the same core state machine. The RPC stream carries `Chunk` values and a final
status that mirrors `ResumeCursor`. It does not use SSE frames, ETags or `Stream-Cursor`.

### 3.10 Client core (feature 07)

The client core is a plain async library. It has no valtron dependency. It uses the shared
types from `foundation_core::durable`. It lives in `foundation_netio::durable` and uses the
injected HTTP transport.

API:

- `DurableClient` with a base URL, headers, retry policy, clock and transport.
- `StreamHandle` with create, connect, append, head, delete, close, read and producer.
- `ReadBuilder` with offset, live mode, timeout, `max_chunks`, cursor and headers.
- `LiveMode` is `Off`, `Auto`, `LongPoll` and `Sse`.
- `Chunk` carries data, next offset, `up_to_date`, `closed`, cursor and status.
- `Producer` supports append, batch, flush, close, detach, epoch, auto-claim, batch sizes and
  in-flight limit.

Behaviour:

- Retry `500`, `502`, `503`, `504`, `429` and network errors with backoff. Honor `Retry-After`.
  Never retry `400` or `404`. Stop after a bound.
- Plain append does not retry. Producer append retries.
- Map errors to the conformance codes. A `404` message includes the path.
- Dynamic headers and parameters are evaluated per request.
- JSON parsing errors surface as `PARSE_ERROR`.
- SSE ignores unknown events. It strips one leading space. It preserves U+0085, U+2028 and
  U+2029. It decodes base64 when the header is present.
- A read without an offset starts at the beginning, even for live modes.

The client conformance adapter is a separate binary. It speaks JSON lines. It normalizes success
statuses: append, delete and close report `200` even when HTTP returns `204`. Duplicates report
`204` and `duplicate: true`.

### 3.11 Valtron layer

The valtron layer is optional. It wraps the core. It adds:

- `DurableStreamReader` as `StreamIterator` + `WakeableStream`.
- `Stream::Next`, `Stream::Spread`, `Stream::Pending` and `Stream::Wait`.
- The append notifier and `WakeRegistry`.
- `DurableStreamWriter` and the producer `#[task]`.
- `into_next_stream` and `into_future_stream` bridges.
- A client reader wrapper that drives the client core.

It must not change protocol outcomes. A conformance test must pass with the valtron layer
disabled.

## 4. Features

| # | Feature | Delivers | Depends on | Size |
|---|---|---|---|---|
| 01 | [Core types and store trait](./features/01-durable-stream-types/feature.md) | `StreamOffset`, `StreamSeq`, `ResumeCursor`, `StreamHead`, `CreateSpec`, `AppendRequest`, `AppendOutcome`, `StoreError`, the store trait and the `DurableStream` enum | — | M |
| 02 | [Reader](./features/02-resumable-reader/feature.md) | Core page reader and the optional valtron `DurableStreamReader` | 01 | M |
| 03 | [Writer, producer and `Stream-Seq`](./features/03-idempotent-producer/feature.md) | `DurableStreamWriter`, the producer task and `Stream-Seq` | 01 | M |
| 04 | [Fork, TTL and retention](./features/04-fork-and-retention/feature.md) | Fork, sub-offset, TTL table, soft-delete and trim | 02, 03 | M |
| 05 | [Storage backends](./features/05-storage-backends/feature.md) | In-memory, file+LMDB, R2/S3, DO SQLite, D1 | 01 | L |
| 06 | [HTTP and ConnectRPC adapters](./features/06-http-rpc-surface/feature.md) | The full wire surface on `foundation_http` and `foundation_connectrpc` | 02, 03 | L |
| 07 | [Client core](./features/07-durable-streams-client/feature.md) | The Rust client, the valtron client wrapper and the conformance adapter | 01 | L |
| 08 | [Conformance](./features/08-conformance/feature.md) | Server and client suites, cross-runs and CI gates | 02–07 | M |

Order: 01 → (02, 03) → 04, 05, 06 → 07 → 08. Run a conformance spike after 01 and 06.

## 5. Rules

1. **valtron only in the valtron layer.** The core uses native async. Never tokio.
2. **Tests live in `tests/`**, registered in `tests/**/mod.rs`. No inline `#[cfg(test)]`.
3. **Side effects are injected, never global.** The store, clock, RNG, ID source and transport
   come from `Ctx`.
4. **Limits are configuration.** Page bytes, page chunks, body bytes, long-poll timeout, SSE
   recycle, cursor interval and seal threshold have documented defaults.
5. **Stream bodies, never buffer them.** Reads are paged. The store never materialises a stream.
6. **Async all the way; valtron composes with it.** The core is async. The valtron layer drives
   it through tasks, waits and bridges.
7. **A refusal, never a gap.** `Trimmed` and `Gone` are the only answers to a lost position.
8. **Every bug goes in `docs/bug-backlog.md`.**
9. **Producer headers before `Stream-Seq`.** A retry is deduplicated even when its `Stream-Seq`
   is older.
10. **No transport or runtime in the core.** The core has no HTTP, SSE, ETag or valtron type.
11. **Conformance is a hard gate.** Both suites must pass. A skip needs a reason and owner
    approval.
12. **The core works without valtron.** Do not make the core depend on the valtron layer.
13. **Write in STE.** Keep sentences short.

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
- Tests with threads, sockets or timing pass 20 runs in a row; report the count.
- DST tests run from a seed and are replayable.

### 6.1 Conformance gate

```bash
npx @durable-streams/server-conformance-tests --run http://127.0.0.1:4437
npx @durable-streams/client-conformance-tests --run ./target/release/conformance-adapter
```

- The server suite runs against `HttpAdapter`, with the valtron layer disabled.
- The client suite runs with the Rust client adapter.
- Record the pass count and every skipped test with its reason.
- A failing or skipped test needs an owner decision before the feature closes.

## 7. Out of scope

- A public CDN product. Keep the cursor and immutable-prefix properties. Do not build a cache.
- Client libraries for other languages. One Rust client is in scope.
- TanStack DB / StreamDB as a dependency.
- Authentication. Reuse the existing auth seams.
- A cross-stream total order. Offsets are per stream.
- Subscriptions (`__ds`). The protocol defines them. The reference server implements them. Spec
  71 leaves them out. Keep the `__ds` prefix reserved for a later spec.

## 8. Done for the whole spec

- [ ] `foundation_core::durable` exists and has no valtron dependency.
- [ ] The core types and store trait are complete.
- [ ] The core reader classifies `up_to_date` and `closed` correctly.
- [ ] The producer fences a stale epoch, deduplicates a retry and rejects a gap.
- [ ] `Stream-Seq` is enforced byte-lexicographically, after producer deduplication.
- [ ] Fork, sub-offset, TTL and soft-delete follow the protocol.
- [ ] The HTTP adapter passes the server conformance suite.
- [ ] The client core passes the client conformance suite.
- [ ] The cross-runs pass.
- [ ] The valtron layer passes its behaviour tests.
- [ ] No `#[cfg(test)]` module in the changed source files. No ignored `Result`s.
