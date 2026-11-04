---
feature: "Durable stream types and the store trait"
status: "planned"
priority: "high"
depends_on: []
crates: ["foundation_core"]
---

# Feature 01: Durable stream types

_Written in ASD-STE100 Simplified Technical English (STE). Code and identifiers stay as they
are._

**Abbreviations in this file.** time to live (TTL); AeroSpace and Defence Industries Association
of Europe (ASD), the owner of the STE standard ASD-STE100.

## 1. Goal

Create the core types and the `DurableStreamStore` trait in
`foundation_core::valtron::durable`. Nothing runs yet. Every later feature builds on these types.

## 2. Requirements

1. **`StreamOffset` is opaque and ordered.** A newtype over bytes. `Ord` is byte-lexicographic.
   It is built only by a store (`pub(crate)` constructor or a `from_bytes` sealed to the store).
   A reader and a writer never parse it. `Debug` shows it as escaped bytes, not a parsed value.
2. **`ReadFrom` has three forms.** `Start` (the protocol's `"-1"`), `Offset(StreamOffset)` (resume
   at a stored cursor) and `Tail` (the protocol's `"now"`).
3. **`StreamCursor` carries the resume state.** `next: StreamOffset`, `up_to_date: bool`,
   `closed: bool`. `up_to_date` and `closed` match the protocol's `Stream-Up-To-Date` and
   `Stream-Closed`.
4. **`ReadPage` is bounded.** `chunks: Vec<Chunk>` and `cursor: StreamCursor`. The page limit is a
   parameter, never a constant (CLAUDE.md rule 8).
5. **`Chunk` and `StreamMeta`.** A chunk is bytes. Metadata is content type, tail, closed, and an
   optional expiry. `Expiry` is either a relative TTL or an absolute deadline, never both.
6. **`AppendAck` has two levels.** `Accepted` (durable at the tail) and `Sealed` (durable in the
   sealed object). Both carry a `StreamCursor`.
7. **`StoreError` names the refusal cases.** `NotFound`, `Trimmed`, `Fenced`, `Conflict`, `Closed`,
   `Storage(Box<dyn core::error::Error>)`.
8. **`DurableStreamStore` is the trait** in `requirements.md` section 3.2. It is object-safe where
   possible, but `Chunk` and `StreamOffset` are owned, so the trait may stay generic. The methods
   match the six protocol operations: `create`, `head`, `append`, `read`, `close`, `delete`, and
   `fork`.
9. **`no_std`-friendly.** The types use `alloc` (`Box`, `Vec`) and no `std`-only surface. The
   module is gated behind the `alloc` support valtron already has.

## 3. Design

The module is `foundation_core::valtron::durable`, re-exported from `valtron::mod.rs` so users
reach everything from one path: `use foundation_core::valtron::durable::{DurableStreamStore,
StreamOffset, StreamCursor}`.

```rust
pub struct StreamOffset(Box<[u8]>);
impl Ord for StreamOffset { /* byte-lexicographic */ }

pub enum ReadFrom { Start, Offset(StreamOffset), Tail }

pub struct StreamCursor { pub next: StreamOffset, pub up_to_date: bool, pub closed: bool }

pub struct ReadPage { pub chunks: Vec<Chunk>, pub cursor: StreamCursor }

pub struct Chunk(Box<[u8]>);

pub enum Expiry { Ttl(core::time::Duration), Absolute(/* RFC 3339 timestamp */) }

pub struct StreamMeta { pub content_type: ContentType, pub tail: StreamOffset,
                        pub closed: bool, pub expires: Option<Expiry> }

pub enum AppendAck { Accepted { cursor: StreamCursor }, Sealed { cursor: StreamCursor } }

pub enum StoreError { NotFound, Trimmed, Fenced, Conflict, Closed,
                      Storage(Box<dyn core::error::Error>) }
```

## 4. Tasks

- [ ] **T1 — core types.** `StreamOffset`, `ReadFrom`, `StreamCursor`, `ReadPage`, `Chunk`,
      `Expiry`, `StreamMeta`, `AppendAck`, `StoreError`. Unit tests in
      `tests/valtron/durable/types.rs`: `StreamOffset` orders lexicographically and never parses;
      `Expiry` rejects both forms at once.
- [ ] **T2 — the trait.** `DurableStreamStore` with the seven methods. A doc comment on each
      method names its protocol verb and its error.
- [ ] **T3 — module wiring.** `pub mod durable` in `valtron::mod.rs`, re-exported in `lib.rs`.

## 5. Success criteria

- [ ] `cargo test -p foundation_core` green; the new types are tested from `tests/`, not inline.
- [ ] `cargo check --target wasm32-unknown-unknown -p foundation_core` green (the types are
      `no_std`-friendly).
- [ ] No `#[cfg(test)]` module in source files.
