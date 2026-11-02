# Feature 23 progress

_Written in ASD-STE100 Simplified Technical English._

## Landed

- **E1.** `FjallVectorStore` runs its two blocking operations on a valtron
  offload pool. The pool uses `BackgroundJobRegistry` and a `Pipe`, the same
  seam as `NativeFs` (feature 20 E4). A namespace index rebuild and `flush`
  now await a worker; `flush` is `async`. Small reads and writes stay inline.
  New test `index_rebuild_does_not_hold_worker` shows a ticker keeps ticking
  during the first search on a reopened store with 100 000 vectors.

## In progress

- None.

## Next

- E2: build sqlite-vec from source in `build.rs`; delete `vendor/vec0.so`.
- E3: migrate old `vec0` tables.
- E4: opt-in live TurboPuffer test.
- E5: TurboPuffer 429/503 retry.
- E7: `hybrid_search` text lookup.
- E8: `SqlVectorStore` scan-cost docs and threshold warning.
- E6 (last): `sql-store` on wasm32 + CI wasm run.
