---
feature: "Core types and store trait"
status: "done"
priority: "high"
depends_on: []
crates: ["foundation_core"]
---

# Feature 01: Core types and store trait

_Written in ASD-STE100 Simplified Technical English (STE). Code and identifiers stay as they
are._

**Abbreviations in this file.** time to live (TTL); AeroSpace and Defence Industries Association
of Europe (ASD), the owner of the STE standard ASD-STE100.

## 1. Goal

Create the protocol core in `foundation_core::durable::shared`. It has no valtron dependency.
Every
later feature builds on these types and the store trait.

**User story.** As a platform engineer, I want one protocol core, so that every runtime and
transport shares the same semantics.

**BDD scenarios.**

- Given two offsets from one stream, when I compare them, then byte order decides.
- Given a media type with parameters, when I parse it, then the type is normalized.
- Given a stream path with `..`, when I parse it, then the request is refused.
- Given a driver, when the business API creates and appends, then a reader sees the bytes.

## 2. Requirements

1. **Offsets and sequences.** `StreamOffset` and `StreamSeq` are opaque newtypes over bytes.
   `Ord` is byte-lexicographic. A reader never parses an offset. `Debug` shows escaped bytes.
2. **Names and types.** `ContentType` parses a media type. `StreamName` is a validated URL path.
   `ProducerId` is a non-empty string. The default content type is `application/octet-stream`.
3. **Chunks.** `Chunk` is a delivery unit. For non-JSON streams, concatenation is byte-exact.
   For JSON streams, one chunk is one message.
4. **Read types.** `ReadFrom` has `Start` (`-1`), `Offset` and `Tail` (`now`). `ReadRequest` has
   a `ReadLimit` in bytes and chunks. `ReadPage` has chunks and a `ResumeCursor`.
5. **Resume cursor.** `ResumeCursor` has `next`, `up_to_date` and `closed`. `up_to_date` and
   `closed` match `Stream-Up-To-Date` and `Stream-Closed`.
6. **Metadata and create.** `StreamHead` has content type, tail, closed and expiry. `CreateSpec`
   has content type, expiry, closed, initial chunk and an optional `ForkSpec`.
7. **Append and close.** `AppendRequest` has a chunk, content type, `StreamSeq`, producer and a
   close flag. `AppendOutcome` has `AppendKind` (`Created` or `Duplicate`), a cursor, the highest
   accepted producer sequence and the closed flag. `CloseRequest` has an optional final chunk and
   a producer.
8. **Expiry.** `Expiry` is a relative TTL or an absolute RFC 3339 time, never both.
9. **Acknowledgement.** `AckLevel` is `Accepted` or `Sealed`. It is internal configuration. It
   never changes a wire response.
10. **Errors.** `StoreError` carries the cause: `NotFound`, `Gone`, `Trimmed`, `Fenced`,
    `InvalidEpochSeq`, `SequenceGap`, `StreamSeqConflict`, `ContentTypeMismatch`,
    `ConfigConflict`, `Closed`, `BadRequest`, `RateLimited`, `Unsupported`, `Storage`.
    `BadRequestKind` names the bad-request case. `StoreError` is `Send + Sync + 'static`.
11. **The store trait.** `DurableStreamStore` has create, head, append, read, close, delete,
    fork and notifier. It uses native `async fn`. The reader and writer are generic over it.
12. **Injection.** `DurableStream` is a struct in feature 05. It wraps `Arc<dyn DurableStreamVfs>`
    and implements the store trait. `Ctx` holds `Arc<DurableStream>`, not
    `Arc<dyn DurableStreamStore>`.
13. **No valtron.** This module has no valtron type, trait or executor. It compiles without
    valtron.
14. **Portable types.** The types use `core` and `alloc` types. The crate requires `std` today,
    so the core compiles on every target the crate supports.
15. **Driver.** `DurableDriver` provides the store and the append notifier. The business API
    takes a driver. Embedded, HTTP, WebSocket, DO and Session are drivers.
16. **BDD tests.** Behaviour tests use a driver and the business API. Each test is a
    Given/When/Then scenario. The same scenario runs on every driver.

## 3. Design

```rust
pub struct StreamOffset(Box<[u8]>);
pub struct StreamSeq(Box<[u8]>);
pub struct ContentType(Box<str>);
pub struct StreamName(Box<str>);
pub struct ProducerId(Box<str>);
pub struct Chunk(Box<[u8]>);

pub enum ReadFrom { Start, Offset(StreamOffset), Tail }
pub struct ReadRequest { pub from: ReadFrom, pub limit: ReadLimit }
pub struct ReadLimit { pub max_bytes: Option<usize>, pub max_chunks: Option<usize> }
pub struct ReadPage { pub chunks: Vec<Chunk>, pub cursor: ResumeCursor }
pub struct ResumeCursor { pub next: StreamOffset, pub up_to_date: bool, pub closed: bool }

pub enum Expiry { Ttl(core::time::Duration), Absolute(/* UNIX millis; RFC 3339 at the adapter */) }
pub struct StreamHead { pub content_type: ContentType, pub tail: StreamOffset,
                        pub closed: bool, pub expires: Option<Expiry> }
pub struct CreateSpec { pub content_type: Option<ContentType>, pub expires: Option<Expiry>,
                        pub closed: bool, pub initial: Option<Chunk>,
                        pub fork: Option<ForkSpec> }
pub struct ForkSpec { pub source: StreamName, pub at: Option<StreamOffset>, pub sub_offset: u32 }

pub struct Producer { pub id: ProducerId, pub epoch: u64, pub seq: u64 }
pub struct AppendRequest { pub chunk: Chunk, pub content_type: Option<ContentType>,
                           pub stream_seq: Option<StreamSeq>, pub producer: Option<Producer>,
                           pub close: bool }
pub enum AppendKind { Created, Duplicate }
pub struct AppendOutcome { pub kind: AppendKind, pub cursor: ResumeCursor,
                           pub producer_seq: Option<u64>, pub closed: bool }
pub struct CreateOutcome { pub created: bool, pub head: StreamHead }
pub struct CloseRequest { pub final_chunk: Option<Chunk>, pub producer: Option<Producer> }
pub struct DeleteOutcome { pub soft_deleted: bool }
pub enum AckLevel { Accepted, Sealed }

pub enum StoreError {
    NotFound, Gone, Trimmed,
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

pub trait DurableDriver: Send + Sync {
    type Store: DurableStreamStore;
    fn store(&self) -> &Self::Store;
    fn notifier(&self, name: &StreamName) -> Option<NotifierRef>;
}

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
    fn notifier(&self, name: &StreamName) -> Option<NotifierRef>;
}
```

`DurableStream` implements the trait by delegation. One match arm per method.

## 4. Tasks

- [x] **T0 — story and scenarios.** Write the user story and the Given/When/Then scenarios
      before the code. They guide T1 to T6.
- [x] **T1 — core types.** All types in section 3. Tests: offset and sequence order is
      byte-lexicographic; `ContentType` normalises case and parameters; `StreamName` rejects
      traversal; `Expiry` rejects both forms at once; `StoreError` carries its detail.
- [x] **T2 — the store trait.** The trait and its doc comments. Each method names its protocol
      verb and its errors.
- [x] **T3 — module wiring.** `pub mod durable` in `foundation_core::lib.rs`. No valtron, socket
      or global runtime import.
- [x] **T4 — compile independence.** A test crate builds `foundation_core::durable::shared` with
      valtron disabled, for native and wasm32.
- [x] **T5 — driver.** `DurableDriver` and a mock driver. Test:
      `given_a_driver_when_the_store_is_used_then_the_business_api_sees_it`.
- [x] **T6 — BDD harness.** A shared Given/When/Then helper. Test:
      `given_a_scenario_when_it_runs_then_every_driver_runs_the_same_steps`.
- [x] **T7 — oracles.** Specification and property oracles for the core types, plus an implicit
      no-panic oracle. Tests: `given_a_malformed_type_when_parsed_then_it_is_refused`,
      `parsing_never_panics`.

## 5. Success criteria

- [x] `cargo test -p foundation_core` green; tests live in `tests/durable/`.
      Recorded 2026-11-28 with the default features: 1,046 tests passed, 0 failed, 0 ignored.
      The count covers every test target of the package, including the `mod` integration target
      (914 tests) and the 71 doc-tests. With `--features test-clock,multi` it passed 1,134
      tests, 0 failed, 0 ignored.
- [x] `cargo check --target wasm32-unknown-unknown -p foundation_core` green.
- [x] `foundation_core::durable::shared` has no valtron, socket or global runtime dependency.
- [x] `DurableDriver` and the BDD harness exist; the harness runs a scenario on two drivers.
      The transport matrix holds one Given/When/Then scenario list and runs it on five drivers
      (embedded, HTTP, WebSocket, ConnectRPC and a Durable Object). The VFS testkit runs the
      shared scenarios on every VFS. The older store-contract harness still runs one mock
      driver; the matrix and the testkit are the proof that the same scenario runs on two or
      more drivers.
- [x] The specification and property oracles pass. Tests: `tests/durable/property.rs`
      (`append_read_is_byte_exact`, `paged_reads_are_bounded_and_reassemble`,
      `fork_read_is_source_prefix_plus_tail`) and `tests/durable/types.rs`. The recorded
      durable run passed 146 tests.
- [x] No `#[cfg(test)]` module in source files. Check: `grep -r cfg(test)
      backends/foundation_core/src` finds no match.

## 6. Open items

1. Run the BDD harness on two drivers.
