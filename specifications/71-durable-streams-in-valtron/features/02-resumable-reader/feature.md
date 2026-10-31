---
feature: "Resumable reader"
status: "planned"
priority: "high"
depends_on: ["01-durable-stream-types"]
crates: ["foundation_core"]
---

# Feature 02: Resumable reader

_Written in ASD-STE100 Simplified Technical English (STE). Code and identifiers stay as they
are._

**Abbreviations in this file.** AeroSpace and Defence Industries Association of Europe (ASD), the
owner of the STE standard ASD-STE100.

## 1. Goal

`DurableStreamReader<Store>` turns a `DurableStreamStore` into a valtron `StreamIterator` that
resumes byte-exactly from a cursor, catches up on history, and parks on `Stream::Wait` for live
data. It implements `WakeableStream` so the awaitable bridges wake on append.

## 2. Requirements

1. **Catch-up.** `read(ReadFrom)` pulls bounded pages. Each chunk yields `Stream::Next(Chunk)`; a
   page yields `Stream::Spread(chunks)` when the store hands back more than one chunk at once.
2. **Byte-exact resumption.** Reading in chunks through the cursor must rebuild the same bytes as
   one full read: no skip, no duplicate. This is a test, not a claim.
3. **The tail.** When `up_to_date` is set and the reader is in live mode, the next pull yields
   `Stream::Wait`. `closed` yields the final chunk then `None` (the terminal signal).
4. **A refusal, never a gap.** `StoreError::Trimmed` surfaces as a terminal error state, never a
   silent skip. The reader does not reset to `Start` on its own.
5. **Wake.** `WakeableStream::wake` forwards to a notifier the store provides. A live reader parks
   on `Wait` and is woken exactly when new data lands. On `NotSupported` it degrades to self-wake,
   never a lost wake.
6. **Pending context.** `Stream::Pending(ReaderProgress)` carries the pending state (`Connecting`,
   `Trimming`), matching `SseProgress` in `foundation_netio`.
7. **No blocking.** The reader never blocks the calling thread. Blocking stays at the boundary.
8. **Limits are configuration** (rule 8). The page limit, the live-mode timeout and the wake
   policy are fields with documented defaults.

## 3. Design

`DurableStreamReader<Store>` stores the store handle, the stream name, the cursor, the live-mode
flag and the page limit. It implements:

```rust
impl<Store: DurableStreamStore> StreamIterator for DurableStreamReader<Store> {
    type D = Chunk;
    type P = ReaderProgress;
}

impl<Store: DurableStreamStore> WakeableStream for DurableStreamReader<Store> {
    fn wake(&mut self, cb: WakeCallback) -> WakeResult { /* forward to the store notifier */ }
}

pub enum ReaderProgress { Connecting, Trimming, Live }
```

The store exposes a notifier for append, so the reader's `wake` registers a callback that the
store fires on the next append. This is the same `WakeRegistry` pattern the existing
`NotifyQueueStreamIterator` and the wasm `LazySseIterator` use.

## 4. Tasks

- [ ] **T1 — catch-up reader.** Pull pages, yield `Next`/`Spread`, thread the cursor. Test:
      `reads_resume_byte_exactly_across_page_boundaries`.
- [ ] **T2 — live tail.** `Stream::Wait` at `up_to_date`; wake on append; `closed` ends the
      stream. Test: `live_tail_parks_then_wakes_on_append`.
- [ ] **T3 — trim refusal.** A read at a dropped offset surfaces `Trimmed` as a terminal error.
      Test: `trimmed_offset_is_a_refusal_not_a_gap`.
- [ ] **T4 — bridges.** `into_next_stream()` and `into_future_stream()` over the reader park on
      `Wait` and resume on wake. Test with a hand-rolled waker (see spec 65 feature 06's parking
      tests).
- [ ] **T5 — DST harness.** An in-memory store with a simulated clock; the reader replays a seeded
      append/read scenario. This lands in `tests/valtron/durable/`.

## 5. Success criteria

- [ ] `cargo test -p foundation_core` green; the reader tests live in `tests/`, not inline.
- [ ] Byte-exact resumption and trim refusal are tested and pass 20 runs in a row.
- [ ] `cargo check --target wasm32-unknown-unknown -p foundation_core` green.
