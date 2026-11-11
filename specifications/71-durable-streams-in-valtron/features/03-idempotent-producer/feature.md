---
feature: "Idempotent producer"
status: "planned"
priority: "high"
depends_on: ["01-durable-stream-types"]
crates: ["foundation_core"]
---

# Feature 03: Idempotent producer

_Written in ASD-STE100 Simplified Technical English (STE). Code and identifiers stay as they
are._

**Abbreviations in this file.** identifier (ID); AeroSpace and Defence Industries Association of
Europe (ASD), the owner of the STE standard ASD-STE100.

## 1. Goal

`DurableStreamWriter<Store>` appends to a stream, and an idempotent producer gives exactly-once
writes with epoch fencing and deduplicated acknowledgement.

## 2. Requirements

1. **The producer triple.** `Producer { id, epoch, seq }`. All three come together or not at all
   (a plain append has no producer).
2. **A `#[task]` state machine.** The producer transition maps
   `(Ctx, ProducerState)` to the next state plus a `TaskStatus`. States: `Idle`,
   `AwaitingAck { seq }`, `Fenced`, `Done`.
3. **Epoch fencing.** A stale epoch returns `StoreError::Fenced`. The producer enters `Fenced` and
   stops; it does not retry.
4. **Deduplication.** A retried `(id, epoch, seq)` returns `AppendAck::Accepted` without a second
   write. The store keeps the last accepted `(stream, id, epoch, seq)`.
5. **Sequence rules.** A new epoch starts at `seq = 0`. A sequence gap returns
   `StoreError::Conflict`.
6. **Atomicity where possible.** The store commits the producer state and the append in one
   transaction where the backend can (DO SQLite, one LMDB transaction). Where it cannot, the
   failure mode is a false `Conflict`, not a duplicate, and the writer recovers by incrementing
   its epoch. The doc comment states this per backend.
7. **Acknowledgement levels.** `AckLevel` is a field: `Accepted` or `Sealed`. It is configuration,
   not a constant.

## 3. Design

```rust
pub struct DurableStreamWriter<Store> { store: Store, name: StreamName }

impl<Store: DurableStreamStore> DurableStreamWriter<Store> {
    pub fn append(&self, chunk: Chunk, ack: AckLevel) -> Result<AppendAck, StoreError>;
    pub fn append_as(&self, producer: Producer, chunk: Chunk, ack: AckLevel)
        -> Result<AppendAck, StoreError>;
    pub fn close(&self) -> Result<(), StoreError>;
}

pub struct Producer { pub id: ProducerId, pub epoch: u64, pub seq: u64 }

#[task]
fn producer_step(ctx: Ctx, state: ProducerState)
    -> Option<(ProducerState, TaskStatus<AppendAck, ProducerProgress>)> {
    match state {
        ProducerState::Idle => Some((ProducerState::AwaitingAck { seq: 0 }, TaskStatus::Ignore)),
        ProducerState::AwaitingAck { seq } => { /* append; Fenced / Conflict / ack */ }
        ProducerState::Fenced => None,
        ProducerState::Done => None,
    }
}
```

The producer reads the store from the context bundle (`ctx.get::<Env>()`), so tests swap a mock
store. The `Fenced` state is terminal: a `StoreError::Fenced` maps to `TaskStatus::Ready(Err(..))`
then `Done`.

## 4. Tasks

- [ ] **T1 — writer.** `DurableStreamWriter::append` and `close`. Test: an append returns a cursor
      and a closed stream rejects an append with `StoreError::Closed`.
- [ ] **T2 — the `#[task]` producer.** The state machine, epoch fencing, dedup. Tests:
      `retried_seq_is_deduplicated`, `stale_epoch_is_fenced`, `new_epoch_starts_at_zero`,
      `sequence_gap_is_conflict`.
- [ ] **T3 — atomicity note per backend.** Document where the store commits producer state with
      the append, and where the false-`Conflict` recovery applies. No code beyond the doc.
- [ ] **T4 — DST harness.** A seeded run replays a producer crash-and-restart and proves no
      duplicate chunk lands in the stream.

## 5. Success criteria

- [ ] `cargo test -p foundation_core` green; producer tests live in `tests/`, not inline.
- [ ] Fencing and dedup are tested and pass 20 runs in a row.
- [ ] `cargo check --target wasm32-unknown-unknown -p foundation_core` green.
