---
feature: "foundation_context crate"
status: "completed"
priority: "high"
model: "sonnet"
depends_on: []
crates: ["foundation_context", "foundation_core"]
---

# Feature 01: The `foundation_context` crate

_Written in ASD-STE100 Simplified Technical English (STE). Code and identifiers stay as they
are._

**Abbreviations in this file.** random number generator (RNG); continuous integration (CI);
deterministic simulation testing (DST); AeroSpace and Defence Industries Association of Europe
(ASD), the owner of the STE standard ASD-STE100.

## 1. Goal

Move `Clock`, `MonotonicClock`, `Config`, `Ctx` and `MinimumElement` out of
`backends/foundation_core/src/valtron/ctx.rs` into a new crate,
`backends/foundation_context`. `foundation_core::valtron` re-exports them, so no caller
changes in this feature.

## 2. Measured facts

- `ctx.rs` uses `crate::compati::Mutex`. On wasm, that is `foundation_nostd::primitives::Mutex`.
  On native, it is `std::sync::Mutex`. The new crate needs the same switch.
  `foundation_nostd::primitives::RwLock` also exists; `compati` uses it on wasm.
- `ctx.rs` uses `foundation_compact::{Duration, Instant, SystemTime}` and
  `foundation_compact::rng::{ChaCha8Rng, SeedableRng}`.
- `SimulatedClock` (behind the `test-clock` feature) calls
  `crate::valtron::executors::request_executor_resume()`, which is `pub(crate)`. It must stay
  in `foundation_core` (requirements section 2.1).
- `foundation_macros` checks the type name `Ctx` only, and emits `foundation_core::valtron`
  paths. A re-export keeps it working.
- The workspace includes `backends/*`, so a new directory there joins it with no edit to the
  root `Cargo.toml` members list.

## 3. Work

1. Create `backends/foundation_context/` with `Cargo.toml`, `README.md`, `src/lib.rs` and
   `tests/`.
   - `Cargo.toml`: version `0.0.1`, the workspace `edition`, `rust-version`, `license`,
     `authors` and `repository`, `[lints] workspace = true`, and a description.
   - Dependencies: `foundation_compact = { path = "../foundation_compact", version = "0.2.4",
     default-features = false }` with the features `ctx.rs` needs today (check how
     `foundation_core` enables them, and copy that). Add `foundation_nostd` on wasm only, for
     `Mutex` and `RwLock`.
   - Forward the wasm time-backend features the way `foundation_core` does
     (`wasm-bindgen`, `host-abi`), so a wasm build picks one `foundation_compact` backend.
     Read `foundation_core/Cargo.toml` features `js-wasmbindgen` and `js-foundation-wasm`,
     and mirror only the `foundation_compact/...` part.
2. Move the code. `src/lib.rs` holds a small `sync` module (the `Mutex`/`RwLock` switch), and
   `Clock`, `MonotonicClock`, `Config`, `MinimumElement` and `Ctx` with all of their current
   methods and doc comments. Keep `CtxInner` private (`pub(crate)`). Keep `incomplete_for_test`
   as `#[doc(hidden)] pub`.
3. In `foundation_core`:
   - Add the dependency `foundation_context = { path = "../foundation_context", version = "0.0.1" }`.
     Forward its wasm features from `js-wasmbindgen` and `js-foundation-wasm`.
   - Replace the moved items in `valtron/ctx.rs` with
     `pub use foundation_context::{Clock, Config, Ctx, MinimumElement, MonotonicClock};`.
     Keep `SimulatedClock` in that file.
   - `SimulatedClock` used private `Ctx` internals? Check. If it does, use public API only.
4. Move the tests that test only the moved items (`backends/foundation_core/tests/valtron/ctx.rs`,
   and the `Ctx` parts of `clock_injection.rs` and `clock_wall.rs` that do not use
   `SimulatedClock` or an executor) to `backends/foundation_context/tests/`, with a
   `tests/mod.rs`-style registration the way other crates do it. Leave tests that use an
   executor or `SimulatedClock` in `foundation_core`.
5. Register the crate in `ci/packages.toml` (copy the `foundation_compact` entry shape:
   `wasm = "enforce"` and the wasm runs that pick each backend). Add a README with a
   "Publishing" note: this crate must be published before `foundation_core`. release-plz
   orders that by itself.
6. Update `docs/` or `.agents` text that names `foundation_core/src/valtron/ctx.rs` as the
   home of `Ctx` (`grep -rn "valtron/ctx.rs" .agents docs specifications/CLAUDE.md`).

## 4. Tests

- The moved tests pass in `foundation_context`.
- A new test in `foundation_context/tests/` proves that `Ctx` is `Send + Sync` and that
  `Ctx::clone` shares the map while `Ctx::fork` copies it.
- A new test in `foundation_core/tests/valtron/` proves that the re-export is the same type:
  a `foundation_context::Ctx` passes to a function that takes `foundation_core::valtron::Ctx`.
  This test does not compile on the old code, because `foundation_context` does not exist there.

## 5. Verify

```bash
cargo fmt --all --check
cargo clippy -p foundation_context --all-targets --profile uat -- -D warnings
cargo test -p foundation_context --profile uat
cargo test -p foundation_core --profile uat --features test-clock
cargo check -p foundation_context --target wasm32-unknown-unknown --profile uat
cargo check -p foundation_core --target wasm32-unknown-unknown --no-default-features --features std,js-foundation-wasm --profile uat
python3 scripts/ci/package_workspace.py   # the new crate packages
```

## 6. Done when

- `foundation_context` exists, builds on native and wasm, and owns the moved items.
- `foundation_core::valtron::{Ctx, Clock, MonotonicClock, Config, MinimumElement}` are
  re-exports, and every other workspace crate compiles with no edit
  (`cargo check --workspace --profile uat`).
- `SimulatedClock` stays in `foundation_core`, and its wake tests pass.
