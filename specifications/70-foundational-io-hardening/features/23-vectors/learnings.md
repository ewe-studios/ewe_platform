# Feature 23 learnings

_Written in ASD-STE100 Simplified Technical English._

## E1

- **The existing seam is `BackgroundJobRegistry` + `Pipe`.** It needs only
  `foundation_core/std`, not the wasm-breaking `multi` feature. `NativeFs` in
  `foundation_nativeapis` (feature 20 E4) is the reference implementation.
- **A `std::sync::RwLockWriteGuard` held across an `.await` makes the future
  not `Send`.** The first `mutate_index` version reused one `mut guard` before
  and after the rebuild await. The compiler kept the guard slot alive across
  the await. The fix: scope the first lock to a block and return early on the
  present path, then await the rebuild with no lock held.
- **`BackgroundJobError` has no `Display`.** Format it with `{err:?}`.
- **The `vec0` test data must not be the zero vector.** A zero vector is
  rejected (`ZeroVector`). The first E1 test used `[i % 1000, i % 997,
  i % 991]`, which is `[0, 0, 0]` at `i = 0`.

## E2

- **sqlite-vec is a loadable extension, not a library.** It resolves every
  `sqlite3_*` call through the `sqlite3_api` pointer from `sqlite3ext.h`. So
  `build.rs` can compile `sqlite-vec.c` with a plain C compiler and no link
  against sqlite3: `-fPIC -shared` (macOS: `-dynamiclib -undefined
  dynamic_lookup`), one compile-and-link step.
- **`cc::Build::new().get_compiler().to_command()` gives the right C compiler
  for the current target**, including cross-compilation. Using `cc::Build`
  directly would add a link step against the system sqlite3; the raw command
  avoids it.
- **`libsql-ffi` 0.5.0 ships a fork of `sqlite3ext.h`** with
  `libsql_api_routines`. It does not compile sqlite-vec. The fix: vendor the
  standard SQLite 3.45.1 `sqlite3ext.h` (3.45.1 matches the SQLite bundled
  with `libsql-ffi` 0.5.0).
- **The `sqlite-vec.h` header is generated from a template** (`sqlite-vec.h.tmpl`)
  with `VERSION=0.1.9` substituted. Vendoring the generated header is simpler
  than running the generator in `build.rs`.
- **`include_bytes!(env!("VEC0_SO"))` replaces `include_bytes!("../vendor/vec0.so")`.**
  The path comes from `OUT_DIR`, so it works for every target without a
  relative path to a vendored binary.

## E3

- **A virtual table's shadow tables share its `vec0_` name prefix.** The
  `sqlite_master` scan `name GLOB 'vec0_*'` also returns `vec0_old_chunks`,
  `vec0_old_info`, `vec0_old_rowids` and the rest. Filter on the statement:
  only `sql` that contains `USING vec0` is the virtual table itself.
- **`DROP TABLE` on a vec0 virtual table fails with `SQL logic error` while a
  `sqlite_master` cursor is open.** The migration runs DDL, so the first pass
  must collect the table names and close the cursor before the DDL. Reproduced
  in a throwaway test: the same DDL succeeds with the cursor closed.
- **vec0 has no `xRename`** (`sqlite-vec` sets it to 0), so `ALTER TABLE ...
  RENAME TO` cannot swap tables. The migration copies rows into a temporary
  table, drops the old table, recreates it, and copies back.
- **The old shape allowed duplicate `external_id` rows.** The new text primary
  key refuses them, so the migration counts duplicates first and reports a
  typed `SchemaMismatch` instead of an opaque SQL constraint failure.

## E4

- **The live test skips, not fails, when the key is not set.** The pattern
  `let Ok(key) = std::env::var("TURBOPUFFER_API_KEY") else { eprintln!(...);
  return; }` keeps `cargo test` green in CI with no key. The message prints
  with `--nocapture`.
- **`PreparedRequestBuilder::delete` + `bearer_token` deletes the namespace.**
  The `VectorStore` trait has no namespace delete, so the cleanup sends a raw
  `DELETE /v2/namespaces/{ns}` through a fresh `DynNetClient`.

## E5

- **`VectorStore: Send + Sync`, so a `RefCell<fastrand::Rng>` cannot live in
  the store.** The existing `ExponentialBackoffDecider` in `foundation_core`
  holds a `RefCell`, which is `Send` but not `Sync`. The retry delay is
  computed in `post` from `foundation_compact::entropy::u32()` instead, a
  stateless platform entropy call (rule 11) that keeps the store `Sync`.
- **`foundation_core::valtron::sleep_async` is the cross-runtime sleep.** On
  native it awaits `timer::sleep_for`; on wasm it bridges a valtron `sleep`
  task. It needs no executor on native, so a retry wait never hangs the store.
- **The mock's `fail_with` answers every request; a script queue answers one
  request each.** The retry tests push a `VecDeque<ScriptedReply>`: the mock
  pops one reply per request, then answers normally, so a 429 followed by
  success is easy to stage.
- **`Retry-After` is whole seconds, so the honours test sleeps one second.**
  The assertion compares the two mock arrivals (a monotonic `Instant` on each
  `Seen`), not a wall-clock guess: the gap must be at least 900 ms.
- **Full jitter over `[0, capped]` avoids float casts.** The u32 entropy draw
  is scaled by `(draw as u128 * nanos) >> 32`, so clippy's
  `cast_precision_loss`/`cast_possible_truncation` never fire.

## E7

- **The reranker already took `texts: &[&str]`; the missing piece was a way to
  fetch the texts.** A new `TextLookup` trait (async, `Send + Sync`,
  `text(&self, id) -> Option<String>`) supplies them. `hybrid_search` collects
  the owned strings into a `Vec`, then borrows them for the rerank call.
- **A candidate with no text reads as an empty string** (`unwrap_or_default`).
  That keeps the text slice aligned with the candidate list, so a reranker can
  index the two together.
- **The recording reranker stores `Mutex<Vec<Vec<String>>>` and returns the
  candidates unchanged.** The test then asserts the recorded text order equals
  the fused order, not just that the call happened.

## E8

- **The scan-cost warning is captured with a `Layer`, not a global subscriber.**
  A `tracing::subscriber::with_default` closure drives the store with
  `futures_lite::future::block_on`, so the warn event lands on the test thread
  and the layer records it. `#[valtron_test]` would poll the store on a pool
  worker whose dispatcher was pinned before the test body, so `with_default`
  would miss it.
- **The warning is once per namespace, not once per search.** A
  `Mutex<HashSet<String>>` on the store records which namespaces already
  warned; a later scan of the same namespace stays quiet.
- **`TursoStorage` works under `futures_lite::future::block_on`.** The local
  turso backend needs no valtron pool, so a plain `#[test]` can drive it.

## E6

- **A target-specific dependency table closes the `[dependencies]` table in
  TOML.** The first fix put `[target.'cfg(..)'.dependencies.foundation_db]`
  between two plain dependencies, so every dependency after it was silently
  outside `[dependencies]` and `dep:futures-lite` stopped resolving. The target
  tables must come after all plain dependencies.
- **`foundation_db` without default features still provides `QueryStore` and
  `DataValue`.** Those types live in the always-compiled `core` module and are
  re-exported at the top level, so the wasm `sql-store` build needs no backend
  feature.
- **The wasm failure came from `foundation_db`'s default features
  (`turso`/`d1`/`r2`), not from `sql_store.rs`.** Each default feature turns on
  `foundation_core/multi`, which is native-only. Turning the default features
  off on `target_family = "wasm"` is enough.

## Dead ends

- **`f32::from(i32)` does not exist.** `f32` implements `From` for small
  integers (`u8`, `i8`, `u16`, `i16`) and `f64`, not `i32` or `u32`. Use a
  float counter instead of casting the loop index.
- **`as f32` casts of `i32` trip `clippy::cast_precision_loss`.** A float
  counter (`x += 1.0`) avoids the cast and the warning.

## Root causes

- fjall has only a sync API, so the store could only wait for the disk by
  blocking the caller. The offload pool gives it a place to wait without
  holding a valtron worker.
