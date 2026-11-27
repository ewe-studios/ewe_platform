# Spec 93 Progress: the embedded V8 runtime

_Last updated: 2026-11-27._

## Status at a glance

| Feature | State |
|---|---|
| 00 Spike: isolate, module, op and timer | not started |
| 01 V8 core: isolate, loader, ops, event loop | not started |
| 02 Web APIs | not started |
| 03 Fetch and WebSocket | not started |
| 04 Cutover | not started |

**Nothing is implemented.** This is a plan for the owner to review.

## What is true today

- Spec 44 is implemented. The owned `#[wasm_test]` harness runs in process on `deno_core`
  0.404 plus seven other direct Deno crates, behind `wasm-embedded-js`
  (`backends/foundation_testbed/Cargo.toml:60-74`, `:126-142`).
- The runtime builds a current-thread tokio runtime for each run
  (`backends/foundation_testbed/src/wasm/embedded_js.rs:417-422`).
- The measured call sites are in `requirements.md` section 4: one module of 483 lines, 13
  `JsRuntime` references, 3 mise tasks and 1 CI recipe.
- The measured test inventory is 106 real cases in four files. The 47 in `foundation_core`
  are 34 `#[wasm_test]` plus 13 `#[valtron_wasm_test]`; both macros add a `__fwt_` export.
  105 cases do not use the network. The one network case is `echo_endpoint_answers`
  (`backends/foundation_testbed/integration/module/src/lib.rs:98-116`, fetch at `:72`).
- `v8` 149.4.0 is already in `Cargo.lock:17664-17666`, through `deno_core` 0.404
  (`Cargo.lock:3336-3339`). A direct `v8` dependency reuses the same prebuilt library.
- The package matrix does not enable `wasm-embedded-js` (`ci/packages.toml:434`). The 8
  runtime tests and the 2 gated runner tests do not run there. The wasm recipe runs the
  binary instead (`backends/foundation_ci/src/execution/recipes.rs:143`). Issue #1094 records
  this gap.
- The Deno set resolves as one group at `deno_core` 0.413 (issue #139, assessment comment).
  The owner has not chosen between that bump and this move (decision D1).

## Local checks done for this specification

- `grep` counted the `#[wasm_test]` attributes and the `JsRuntime` references.
- `cargo tree -p foundation_testbed --no-default-features --features wasm-embedded-js -i
  tokio --edges normal` shows the tokio edges. They come from the direct `tokio` dependency
  and from the Deno crates. No other enabled crate in this feature set pulls tokio.
- No code was changed. No build or test was run for V8, because the dependency is not in the
  tree.

## Next

1. The owner settles D1 to D8 in `requirements.md`.
2. If D1 is "start", feature 00 runs the spike and reports the line count, the wall time and
   the result for `import.meta.url` and dynamic `import()`.
3. If D1 is "bump the Deno set", close this spec as a contingency and record the bump in the
   issue #139 comment.
