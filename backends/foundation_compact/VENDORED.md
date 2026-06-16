# Vendored Code in foundation_compact

This crate vendors code from the following upstream projects. All vendored code
is licensed under MIT OR Apache-2.0.

## getrandom v0.4.2

- **Source:** <https://github.com/rust-random/getrandom>
- **License:** MIT OR Apache-2.0
- **Location:** `src/entropy/`
- **Scope:** Full source — all platform backends, error types, utility modules.
- **Adaptations:**
  - Module paths: `crate::` → `crate::entropy::` (submodule layout)
  - wasm32-unknown-unknown: always provides `wasm_js` backend (upstream gates it
    behind `feature = "wasm_js"` and emits `compile_error!` when absent)
  - Error messages: "entropy:" prefix instead of "getrandom:"

## rand (glue only) — rand 0.10.1 / rand_core 0.10 / rand_chacha 0.10

- **Source:** <https://github.com/rust-random/rand>
- **License:** MIT OR Apache-2.0
- **Location:** `src/rng/`
- **Scope:** Vendored glue only — `SysRng`, `ThreadRng`, `StdRng`, `SmallRng`,
  `Xoshiro128PlusPlus`, `Xoshiro256PlusPlus`. The `rand_core` and `rand_chacha`
  crates are used from crates.io (pure computation, wasm-safe).
- **Adaptations:**
  - `SysRng` bridges `crate::entropy::fill()` → `rand_core::TryRng`
  - `ThreadRng` reseeds from `SysRng` (vendored entropy, not external getrandom)
  - Block counter tracked in `ReseedingCore` (rand_chacha's `ChaCha12Core`
    doesn't expose `get_block_pos()`)

## scru128 (via foundation_rng)

- **Source:** <https://github.com/scru128/rust>
- **License:** MIT OR Apache-2.0
- **Location:** `src/ids/`
- **Scope:** Full scru128 — `Id` type, `Generator`, `RandSource`/`TimeSource`
  traits, global generator, serde support.
- **Adaptations:**
  - `TimeSource` (`StdSystemTime`): uses `crate::SystemTime` (cross-platform
    polyfill) instead of `std::time::SystemTime`
  - `RandSource` adapter: bridges `rand_core::Rng` → `RandSource`
  - `new_scru128()`: thread-local generator backed by our `ThreadRng` + `SystemTime`
  - `global_gen`: replaces `reseeding_rng::StdReseedingRng` with our own
    `StdRng` + `SysRng` reseeding (removes external dep)
  - Dropped `with_rand09.rs` (deprecated rand 0.9 compat)
