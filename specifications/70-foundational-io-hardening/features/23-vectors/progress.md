# Feature 23 progress

_Written in ASD-STE100 Simplified Technical English._

## Landed

- **E1.** `FjallVectorStore` runs its two blocking operations on a valtron
  offload pool. The pool uses `BackgroundJobRegistry` and a `Pipe`, the same
  seam as `NativeFs` (feature 20 E4). A namespace index rebuild and `flush`
  now await a worker; `flush` is `async`. Small reads and writes stay inline.
  New test `index_rebuild_does_not_hold_worker` shows a ticker keeps ticking
  during the first search on a reopened store with 100 000 vectors.
- **E2.** `build.rs` compiles the vendored `vendor/sqlite-vec.c` into the
  sqlite-vec loadable extension for the current target when the `sqlite-vec`
  feature is on. It sets `VEC0_SO` to the built path. The prebuilt
  `vendor/vec0.so` is deleted; `sqlite_vec_store.rs` now embeds the built
  extension. The vendored `sqlite3ext.h` is the standard SQLite 3.45.1 header
  (the one `libsql-ffi` 0.5.0 forks). Tested with the `sqlite-vec` feature on.
- **E3.** `SqliteVecVectorStore::open` migrates old-shape `vec0` tables (spec
  36 F29: no text primary key, no `metadata` column). The migration copies the
  rows into a temporary table, swaps, and recovers a stopped swap on the next
  open. A wrong dimension or a duplicate old id fails with the new typed
  `VectorStoreError::SchemaMismatch` naming the table. New tests
  `old_vec0_table_is_migrated` and
  `old_vec0_table_with_wrong_dimension_is_refused`.
- **E4.** An opt-in live test `live_service_deleted_ids_and_vector` checks the
  real `TurboPuffer` service when `TURBOPUFFER_API_KEY` is set; it skips with a
  message when the key is not set. It proves `deleted_ids` lists only ids that
  existed and `include_attributes: ["vector"]` returns the vector. It uses a
  namespace with a unique suffix and deletes the namespace at the end.
- **E5.** `TurboPufferVectorStore` retries HTTP 429 and 503 with exponential
  backoff and full jitter, and honours `Retry-After`. The attempt count, base
  delay and cap live on the new `TurboPufferConfig` (defaults 5 attempts,
  200 ms, 10 s; the delays follow the AWS SDK standard retry mode). The wait
  uses the valtron timer (`sleep_async`), never a thread sleep. New tests
  `turbopuffer_retries_429_then_succeeds`,
  `turbopuffer_gives_up_after_max_attempts` and
  `turbopuffer_honours_retry_after` run against the mock; the retry tests pass
  a 20-run flake check.

## In progress

- None.

## Next

- E7: `hybrid_search` text lookup.
- E8: `SqlVectorStore` scan-cost docs and threshold warning.
- E6 (last): `sql-store` on wasm32 + CI wasm run.
