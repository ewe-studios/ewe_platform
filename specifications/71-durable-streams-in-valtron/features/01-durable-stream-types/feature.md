---
feature: "Core types and store trait"
status: "planned"
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

Create the protocol core in `foundation_core::durable`. It has no valtron dependency. Every
later feature builds on these types and the store trait.

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
12. **Injection.** `DurableStream` is the host's concrete store enum. It implements the trait by
    delegation. `Ctx` holds `Arc<DurableStream>`, not `Arc<dyn DurableStreamStore>`.
13. **No valtron.** This module has no valtron type, trait or executor. It compiles without
    valtron.
14. **`no_std`-friendly.** The types use `alloc` and no `std`-only surface.

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

pub enum Expiry { Ttl(core::time::Duration), Absolute(/* RFC 3339 */) }
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

`DurableStream` implements the trait by delegation. One match arm per method.

## 4. Tasks

- [ ] **T1 — core types.** All types in section 3. Tests: offset and sequence order is
      byte-lexicographic; `ContentType` normalises case and parameters; `StreamName` rejects
      traversal; `Expiry` rejects both forms at once; `StoreError` carries its detail.
- [ ] **T2 — the store trait.** The trait and its doc comments. Each method names its protocol
      verb and its errors.
- [ ] **T3 — `DurableStream` enum.** The enum delegates to `FileStore`, `InMemoryStore` and
      `DoSqlStore`.
- [ ] **T4 — module wiring.** `pub mod durable` in `foundation_core::lib.rs`. No valtron import.
- [ ] **T5 — compile independence.** A test crate builds `foundation_core::durable` with valtron
      disabled.

## 5. Success criteria

- [ ] `cargo test -p foundation_core` green; tests live in `tests/durable/`.
- [ ] `cargo check --target wasm32-unknown-unknown -p foundation_core` green.
- [ ] `foundation_core::durable` has no valtron dependency.
- [ ] No `#[cfg(test)]` module in source files.
