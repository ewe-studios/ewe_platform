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
