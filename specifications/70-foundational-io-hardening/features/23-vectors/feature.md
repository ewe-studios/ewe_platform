---
feature: "foundation_vectors: fjall off the worker, sqlite-vec per platform and schema migration, TurboPuffer live test and 429 retry, sql-store on wasm, reranker texts, documented scan cost"
status: "pending"
priority: "medium"
depends_on: ["01-valtron-parking", "17-db"]
branch: "fix/io-23-vectors"
crates: ["foundation_vectors", "foundation_db"]
backlog: "docs/bug-backlog.md §17 (8 entries)"
---

# Feature 23: `foundation_vectors`

_Written in ASD-STE100 Simplified Technical English (STE). Code and identifiers stay as they
are._

**Abbreviations in this file.** Structured Query Language (SQL); Hypertext Transfer Protocol
(HTTP); application programming interface (API); WebAssembly (wasm); continuous integration (CI);
pull request (PR); AeroSpace and Defence Industries Association of Europe (ASD), the owner of
the STE standard ASD-STE100.

Use the shared protocol in [`requirements.md`](../../requirements.md) sections 3 and 4. One
commit per entry.

## T0. Check that every entry is still open (table in the PR body)

## E1. `FjallVectorStore` runs fjall inline; `flush` is sync

- **Where:** `src/fjall_store.rs`. The first use of a namespace scans all its vectors to rebuild
  the index; `flush` calls `fsync`. Both hold a worker.
- **Fix:** run the index rebuild and `flush` on valtron's blocking-work seam (feature 01 or 20 E4's
  offload pool; use the one that exists, do not add a second); `flush` becomes `async`. Small
  reads and writes stay inline (no `fsync`); say so in the doc comment.
- **Test:** `fjall_index_rebuild_does_not_hold_worker` (100 000 vectors on a 2-worker pool; a
  ticker keeps ticking during the first search).

## E2. `SqliteVecVectorStore` ships only a linux-x86_64 `vec0.so`

- **Fix:** build sqlite-vec from source in `build.rs` (the `sqlite-vec` crate or its C source with
  `cc`), so every target `cc` supports works; delete `vendor/vec0.so`. If building from source is
  not possible, vendor one file per supported target and fail on others with a clear typed error.
- **Verify:** `cargo test -p foundation_vectors --features sqlite-vec` passes here; the build
  for `aarch64-unknown-linux-gnu` compiles (`cargo check --target …`, if the target is installed).

## E3. Old `vec0` tables (before spec 50 F05 T1) fail

- **Fix:** on `open`, detect the old schema (integer primary key, no `metadata` column) and
  migrate (copy rows into the new table, then swap), or fail with a typed `SchemaMismatch` error
  naming the table. Prefer the migration.
- **Test:** `old_vec0_table_is_migrated` (create the old shape, open, search finds the rows).

## E4. `TurboPufferVectorStore` is tested only against a mock

- **Fix:** an opt-in live test behind `TURBOPUFFER_API_KEY` (skipped with a message when unset):
  `deleted_ids` lists only ids that existed; `include_attributes: ["vector"]` returns the vector.
  The test uses a namespace with a random suffix and deletes it at the end.
- **Verify:** the test runs and passes once with a key (say who ran it, in the PR); it skips
  cleanly without one. Fix the store if the live service differs from the mock.

## E5. `TurboPufferVectorStore` does not retry HTTP 429

- **Fix:** retry 429 (and 503) with exponential backoff and jitter; honour `Retry-After`;
  attempts, base delay and cap on a `TurboPufferConfig` (rule 8; defaults 5 attempts, 200 ms,
  10 s; say where they come from). Use the valtron timer, not a thread sleep.
- **Tests:** `turbopuffer_retries_429_then_succeeds`; `turbopuffer_gives_up_after_max_attempts`;
  `turbopuffer_honours_retry_after` (mock server).

## E6. The `sql-store` feature does not build for wasm32

- **Fix:** take `foundation_db` without default features on wasm32 (target-specific dependency),
  then add `sql-store` to the crate's `wasm-runs` in `ci/packages.toml`.
- **Verify:** `cargo check --target wasm32-unknown-unknown -p foundation_vectors --features sql-store`
  passes; `python3 scripts/ci/ci.py run foundation_vectors` includes the wasm run.

## E7. `hybrid_search` gives the reranker no texts

- **Where:** `src/fusion.rs` calls `Reranker::rerank(query, fused, &[])`.
- **Fix:** `hybrid_search` takes a text source (`&dyn TextLookup`, id → text, async, may return
  none); it fetches texts for the fused candidates only and passes them in. Update callers.
- **Test:** `hybrid_search_passes_texts_to_reranker` (a recording reranker sees one text per
  candidate, in order).

## E8. SQL-backed stores scan a whole namespace per search

- **Fix:** this is expected for the portable fallback. Document the O(n·d) cost on
  `SqlVectorStore` and in the crate docs, with the advice to use `LibsqlVectorStore` or a vector
  backend for large namespaces. Add a `tracing` warning once per namespace when the count passes
  a configured threshold (default 100 000).
- **Test:** `sql_store_warns_above_scan_threshold` (threshold 10, 11 vectors, one warning).

## Verification (run exactly)

```bash
export CARGO_PROFILE_DEV_DEBUG=0 CARGO_INCREMENTAL=0
python3 scripts/ci/ci.py run foundation_vectors --profile dev
cargo test -p foundation_vectors --profile dev --all-features
cargo check --target wasm32-unknown-unknown -p foundation_vectors --features sql-store
cargo check --workspace --all-targets --profile dev
```

## Done when

- [ ] T0 table; every open entry fixed; backlog entries deleted.
- [ ] New tests fail on the base code where they can; 20-run flake check on E1 and E5.
- [ ] CI green, including the new wasm run.
