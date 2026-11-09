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
