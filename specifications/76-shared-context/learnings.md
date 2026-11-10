# Spec 76 learnings

_Written in ASD-STE100 Simplified Technical English (STE). Code and identifiers stay as they
are._

**Abbreviations in this file.** random number generator (RNG); Hypertext Transfer Protocol
(HTTP); AeroSpace and Defence Industries Association of Europe (ASD), the owner of the STE
standard ASD-STE100.

- valtron `Ctx` and `foundation_http::ContextBag` are the same idea: a type map, a clock and a
  shared `ChaCha8Rng`. Only `Ctx` has a config, and only the bag uses an `RwLock`. The bag
  also has an `unsafe impl Send/Sync` that the compiler does not need.
- `SimulatedClock` cannot move to `foundation_context`. Its `advance` must wake the parked
  executors (#305), and that wake is crate-private in `foundation_core`. A listener hook would
  let a caller build a simulated clock that does not wake them.
- `foundation_connectrpc::Ctx` is emitted by two generators (`foundation_macros` and
  `foundation_connectrpc_codegen`). The rename to `RpcCtx` must change both and regenerate.
- About 600 `dyn Clock` sites exist outside tests (2026-11-09). `foundation_ai` (124),
  `foundation_deployment_platform` (81), `foundation_auth` (80) and `foundation_nativeapis`
  (73) are the largest.
- The master review on 2026-11-09 found more parallel time traits: `TimeSource` in
  `foundation_nostd` (`primitives/wait_duration.rs`), `foundation_wasm` (`wasi_host.rs`) and
  `foundation_compact` (`ids/generator.rs`, with `RandSource`), `DeadlineTimer` in connectrpc
  and `ClockSource` in foundation_browser. The crates below `foundation_context` cannot take a
  `Ctx`, so this spec does not remove them. Issue #459 tracks the `std::time::Instant` use in
  two of them.
- `Ctx::host()` seeds its RNG with `0` (#448). Feature 02 fixes it, because it changes the
  `Ctx` constructors.
- About 27 direct randomness reads bypass `Ctx` (#453). That is a separate change: this
  spec moves the clock, and `ctx.rng()` is already the place those reads must use.
- Feature 01: `SimulatedClock::new` reads `Instant::now()` for its base. That read stayed in
  `foundation_core/src/valtron/ctx.rs`, so the time-source allowlist needs an entry for that
  file, next to the new entry for `foundation_context/src/lib.rs`.
- Feature 01: `cargo fmt --all` rewrites files in other crates (`foundation_connectrpc_codegen`
  `generator.rs`, `foundation_openapi` `unified/emit.rs`). Those files were already not
  formatted. Revert them, or format only the crate you changed.
- Feature 01: `foundation_context` has no default features. Its wasm build compiles with no
  time backend chosen, so each consumer must forward `wasm-bindgen` or `host-abi`.
- Feature 02: a text replace of `Ctx` is not safe in the connectrpc, proxy and docker crates.
  They name both the RPC `Ctx` and the valtron `Ctx` (for example `use
  foundation_core::valtron::{self, Ctx, ..}` in `transport/h2.rs` and `h3.rs`). Rename the RPC
  type first, then check each valtron import by hand.
- Feature 02: `cargo fix` removes `use std::sync::Arc;` when no code uses it in the checked
  feature set. `foundation_http/src/native/serve.rs` needs `Arc` only with the `quic` feature, so
  that import now has a `cfg`. Check each crate with `--all-features` after `cargo fix`.
- Feature 02: `cargo clippy -p <crate> -- -D warnings` also lints workspace dependencies. Today
  `foundation_reactor` fails it, so read only the warnings that name the changed crate.
- Feature 02: `Ctx::host()` now reads entropy. It panics if the entropy source fails. A host that
  must not panic calls `Ctx::try_host()`. `PlatformSession` does this and keeps its warning.
- Feature 02: `Ctx` has no pointer-equality method. A test that proved "the app stores the
  caller's context" with `Arc::ptr_eq` now stores a value through one handle and reads it from the
  other, because a `Ctx` clone shares the map.
