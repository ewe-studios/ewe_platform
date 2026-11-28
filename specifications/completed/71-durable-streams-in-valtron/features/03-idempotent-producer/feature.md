---
feature: "Writer, producer and Stream-Seq"
status: "done"
priority: "high"
depends_on: ["01-durable-stream-types"]
crates: ["foundation_core"]
---

# Feature 03: Writer, producer and Stream-Seq

_Written in ASD-STE100 Simplified Technical English (STE). Code and identifiers stay as they
are._

**Abbreviations in this file.** identifier (ID); AeroSpace and Defence Industries Association of
Europe (ASD), the owner of the STE standard ASD-STE100.

## 1. Goal

Add the core writer, the idempotent producer and `Stream-Seq`. Then add the required valtron
writer and producer `#[task]`.

## 2. Requirements

1. **Writer.** `DurableStreamWriter<Store>` has `append`, `append_as` and `close`. It returns
   `AppendOutcome`.
2. **Producer triple.** `Producer { id, epoch, seq }`. All three headers come together or not at
   all. Partial headers return `BadRequest`.
3. **Header validation.** `Producer-Id` is non-empty. Epoch and sequence are decimal integers at
   most `2^53 - 1`.
4. **Epoch fencing.** A stale epoch returns `Fenced { current_epoch }`. The writer stops. It does
   not retry.
5. **Epoch change.** A higher epoch must start at `seq = 0`. Otherwise `InvalidEpochSeq`.
6. **Deduplication.** Same epoch and `seq <= last_seq` returns `AppendKind::Duplicate` with the
   highest accepted `Producer-Seq`. A retry writes nothing.
7. **Next sequence.** Same epoch and `seq == last_seq + 1` returns `AppendKind::Created`.
8. **Gap.** `seq > last_seq + 1` returns `SequenceGap { expected, received }`.
9. **Serialization.** Validate and append atomically. Use a per-`(stream, producer_id)` lock or a
   versioned compare-and-swap. A stale version retries.
10. **Atomicity.** Commit producer state with the append where the backend can. The VFS
    `append_with_meta` commits the writer state and the bytes together. Otherwise the failure
    mode is a false `SequenceGap`, not a duplicate. A failed append does not advance producer
    state. A gap can be filled after the earlier sequence arrives.
11. **State lifetime.** Producer state TTL is 7 days in memory. Persistent stores retain it with
    the data. A fork does not inherit producer state.
12. **Close.** A duplicate of the closing tuple returns `Duplicate` with `closed`. A close-only
    request is idempotent. An append to a closed stream returns `Closed { tail }`.
13. **`Stream-Seq`.** An append may carry `StreamSeq`. Compare it byte-lexicographically with the
    last accepted value. A value at or below the last accepted value returns `StreamSeqConflict`.
    Update the value only after a successful append. The scope is per stream. A fork does not
    inherit it.
14. **Producer first.** Validate the producer triple before `Stream-Seq`. A retry with an older
    `Stream-Seq` is deduplicated, not refused.
15. **Acknowledgement.** `AckLevel` is `Accepted` or `Sealed`. It is configuration, not a wire
    value.
16. **Valtron writer.** `DurableStreamWriter` and the producer `#[task]` wrap the core. The task
    states are `Idle`, `AwaitingAck { seq }`, `Fenced` and `Done`. `Fenced` is terminal. The task
    must not change the core outcomes.

## 3. Design

Paths follow the repository layout:

| Item | Path |
|---|---|
| Core writer and producer | `foundation_core/src/durable/shared/` |
| Valtron writer and task | `foundation_core/src/durable/shared/valtron/` |

```rust
pub struct DurableStreamWriter<Store> { service: DurableService<Store>, name: StreamName }

impl<Store: DurableStreamStore> DurableStreamWriter<Store> {
    pub async fn append(&self, chunk: Chunk, ack: AckLevel, stream_seq: Option<StreamSeq>)
        -> Result<AppendOutcome, StoreError>;
    pub async fn append_as(&self, producer: Producer, chunk: Chunk, ack: AckLevel,
                           stream_seq: Option<StreamSeq>) -> Result<AppendOutcome, StoreError>;
    pub async fn close(&self, req: CloseRequest) -> Result<AppendOutcome, StoreError>;
}

#[task]
fn producer_step(ctx: Ctx, state: ProducerState)
    -> Option<(ProducerState, TaskStatus<AppendOutcome, ProducerProgress>)> {
    match state {
        ProducerState::Idle => Some((ProducerState::AwaitingAck { seq: 0 }, TaskStatus::Ignore)),
        ProducerState::AwaitingAck { seq } => { /* append; Fenced / SequenceGap / ack */ }
        ProducerState::Fenced => None,
        ProducerState::Done => None,
    }
}
```

## 4. Tasks

- [x] **T1 — core writer.** `append`, `append_as` and `close`. Test: append returns a cursor and
      a closed stream rejects an append with `Closed { tail }`.
- [x] **T2 — producer ladder.** Header validation, epoch fencing, epoch change, dedup, next
      sequence and gap. Tests: `retried_seq_is_deduplicated`, `stale_epoch_is_fenced`,
      `new_epoch_requires_seq_zero`, `sequence_gap_is_typed`.
- [x] **T3 — serialization and atomicity.** Versioned append and commit order. Tests:
      `concurrent_batches_are_ordered`, `failed_append_does_not_advance_state`,
      `gap_can_be_filled`.
- [x] **T4 — `Stream-Seq`.** Lexicographic comparison, per-stream state and producer-first order.
      Tests: `stream_seq_regression_is_conflict`, `stream_seq_skips_are_accepted`,
      `producer_retry_precedes_stream_seq`, `fork_does_not_inherit_stream_seq`.
- [x] **T5 — valtron producer.** The `#[task]` state machine. Test:
      `producer_task_deduplicates_and_fences`.
- [x] **T6 — DST harness.** A seeded crash and restart proves no duplicate chunk. Test:
      `seeded_producer_restart_has_no_duplicate`.
- [x] **T7 — core independence.** Producer tests run with valtron disabled.
- [x] **T8 — oracles.** State model, differential and property oracles. Tests:
      `producer_model_matches`, `drivers_agree`, `stream_seq_is_monotonic`.

## 5. Success criteria

- [x] `cargo test -p foundation_core` green; tests live in `tests/durable/`.
- [x] The core writer and producer tests pass without valtron.
- [x] Fencing, dedup and `Stream-Seq` pass 20 runs in a row.
- [x] The valtron producer task tests pass with valtron enabled.
