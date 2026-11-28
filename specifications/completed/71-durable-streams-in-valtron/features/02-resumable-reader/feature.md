---
feature: "Reader"
status: "done"
priority: "high"
depends_on: ["01-durable-stream-types"]
crates: ["foundation_core"]
---

# Feature 02: Reader

_Written in ASD-STE100 Simplified Technical English (STE). Code and identifiers stay as they
are._

**Abbreviations in this file.** end of file (EOF); AeroSpace and Defence Industries Association
of Europe (ASD), the owner of the STE standard ASD-STE100.

## 1. Goal

Add the core page reader. Then add the required valtron reader. The core reader must be correct
without valtron. The valtron reader adds `StreamIterator`, `Stream::Wait`, `Stream::Spread` and
`WakeableStream`. The valtron layer is required for platform use.

## 2. Requirements

1. **Core page reader.** `read(name, ReadRequest)` returns a bounded `ReadPage`. It does not
   parse offsets.
2. **Byte-exact resumption.** Reading page by page through the cursor rebuilds the same bytes as
   one full read. No skip. No duplicate.
3. **State rules.** The reader sets `up_to_date` only when the page has all data at response
   time. It sets `closed` only when the stream is closed and the page reaches the final offset.
   A partial page of a closed stream has `up_to_date = false` and `closed = false`.
4. **Closure discovery.** Closure can need one more read at the tail. That read returns an empty
   page with `closed = true`.
5. **`offset=now`.** `ReadFrom::Tail` returns an empty page at the tail with `up_to_date = true`.
   In live mode it waits for new data. A closed stream returns immediately with `closed = true`.
6. **Trim refusal.** `StoreError::Trimmed` is terminal. The reader never resets to `Start`.
7. **No blocking.** The core reader never blocks.
8. **Valtron reader.** `DurableStreamReader<Store>` implements `StreamIterator` with
   `D = Chunk` and `P = ReaderProgress`, and `WakeableStream`.
9. **Valtron states.** A page yields `Stream::Next` per chunk or `Stream::Spread` for one page.
   A pending call yields `Stream::Pending`. An open tail yields `Stream::Wait` in live mode. A
   closed tail yields the final chunk then `None`.
10. **Wake.** `wake` registers a callback on the store notifier. A live reader parks on `Wait`
    and wakes exactly when data lands. `NotSupported` degrades to self-wake, never a lost wake.
11. **Bridges.** `into_next_stream` and `into_future_stream` park on `Wait` and resume on wake.
12. **Limits.** Page bytes, page chunks and live timeout are configuration.

## 3. Design

Paths follow the repository layout:

| Item | Path |
|---|---|
| Core reader | `foundation_core/src/durable/shared/` |
| Valtron reader | `foundation_core/src/durable/shared/valtron/` |

```rust
pub struct DurableStreamReader {
    inner: DrivenTaskIterator<read_step>,
}

impl Iterator for DurableStreamReader {
    type Item = Stream<Chunk, ReaderProgress>;
    fn next(&mut self) -> Option<Self::Item> { self.inner.next().map(Stream::from) }
}

impl WakeableStream for DurableStreamReader {}

pub enum ReaderProgress { Connecting, Trimming, Live }

#[task]
async fn read_step(ctx: Ctx, state: ReaderState)
    -> Option<(ReaderState, TaskStatus<Chunk, ReaderProgress>)> { /* read, spread, end */ }
```

The core reader is `DurableStream::read`. The valtron reader wraps it in a `#[task]` and must
not change the core state rules. The current slice covers catch-up. The live tail and wake
forwarding land before the feature closes.

## 4. Tasks

- [x] **T1 — core catch-up.** Page reads and cursor threading. Test:
      `reads_resume_byte_exactly_across_page_boundaries`.
- [x] **T2 — core state rules.** `up_to_date`, `closed` and partial pages. Tests:
      `partial_page_omits_up_to_date_and_closed`, `closed_tail_reports_both`.
- [x] **T3 — closure discovery.** Read the tail after closure. Test:
      `closure_discovered_by_next_read`.
- [x] **T4 — `offset=now`.** Catch-up, live and closed cases. Tests: `now_catchup_empty`,
      `now_live_waits`, `now_closed_returns_immediately`.
- [x] **T5 — trim refusal.** Test: `trimmed_offset_is_a_refusal_not_a_gap`.
- [x] **T6 — valtron tail.** `Stream::Wait`, wake on append and end on close. Tests:
      `live_tail_parks_then_wakes_on_append`, `closed_stream_ends_without_parking`.
- [x] **T7 — bridges.** `into_next_stream` and `into_future_stream`. Test with a hand-rolled
      waker.
- [x] **T8 — DST harness.** Seeded append and read replay. Test:
      `seeded_reader_replay_is_byte_exact`.
- [x] **T9 — valtron independence.** The core reader tests run with valtron disabled.
- [x] **T10 — oracles.** Model, metamorphic and property oracles. Tests:
      `model_matches_driver`, `read_from_offset_is_suffix`, `append_read_is_byte_exact`.

## 5. Success criteria

- [x] `cargo test -p foundation_core` green; tests live in `tests/durable/`.
- [x] The core reader tests pass without valtron.
- [x] Byte-exact resumption, state rules and trim refusal pass 20 runs in a row.
- [x] The valtron reader tests pass with valtron enabled.
