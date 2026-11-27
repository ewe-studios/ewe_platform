---
feature: "V8 core: isolate, loader, ops and event loop"
status: "pending"
priority: "high"
model: "sonnet"
phase: "1"
depends_on: ["00-spike"]
crates: ["foundation_testbed"]
---

# Feature 01: V8 core — isolate, loader, ops and event loop

_Written in ASD-STE100 Simplified Technical English (STE)._

## 1. Goal

Build the whole runtime core behind a new feature, `wasm-embedded-v8`, beside
`wasm-embedded-js`. The core is the isolate, the module loader, the ops bridge and the event
loop, with the public API of the old module.

The stage is done when the core tests pass, the trap path keeps its report, and both feature
sets run in CI. The harness cases need the Web APIs (feature 02), so this stage does not claim
them.

## 2. Tasks

| Task | Work |
|---|---|
| T1 | Add `backends/foundation_testbed/src/wasm/v8_runtime/mod.rs`. Keep the public names `run_module`, `run_staged_harness` and `HarnessReport` (`embedded_js.rs:297`, `:315`, `:209-227`), so `fwt_runner.rs:198` and the tests change only the import path. |
| T2 | `v8_runtime/isolate.rs`: `Runtime::new()`, one context, one handle scope, `TryCatch` at each callback boundary, and one `RuntimeError` type that carries the JavaScript stack. |
| T3 | `v8_runtime/loader.rs`: `resolve`, `load`, `instantiate`, `evaluate`. Keep a map from `v8::ModuleRequest` to `v8::Global<v8::Module>`. Set the script origin resource name for `import.meta.url`. Send a dynamic `import()` through the same path. A missing file names the path in the error. |
| T4 | `v8_runtime/ops.rs`: the three ops with the same names and contracts as today: `op_fwt_read_file` (`embedded_js.rs:156-160`), `op_fwt_report` (`:166-169`) and `op_fwt_get_random_values` (`:175-178`). Keep the report sink as a shared `Rc<RefCell<Option<String>>>`. Expose `globalThis.Deno.core.ops` with the same shape. |
| T5 | `v8_runtime/event_loop.rs`: `async fn drive(runtime)`. Run the microtask checkpoint. Keep a timer queue keyed on `Ctx::clock().now()` and wait with `Ctx::timer().sleep_until(ctx, deadline)` (`foundation_context/src/env/timer.rs:17-23`). Expose the spike's minimal `setTimeout` in the core runtime. Keep a promise table for asynchronous ops. Declare idle only when there are no microtasks, no armed timers, no pending ops and no promise jobs. Add a run timeout as a configuration field. |
| T6 | `v8_runtime/mod.rs`: `run_staged_harness(stage_dir)`. Build the runtime, load `runner.js` from the stage dir (`fwt_runner.rs:114-136`), drive the loop, read the report, and return `HarnessReport`. Keep the `read_diagnostics` behaviour (`embedded_js.rs:367-402`), so a late trap keeps the verdicts and names the case (issue #587). |
| T7 | `src/wasm/fwt_runner.rs`: give `run_deno` (`:186-212`) three cfg forms. `#[cfg(feature = "wasm-embedded-v8")]` calls `v8_runtime::run_staged_harness`. `#[cfg(all(feature = "wasm-embedded-js", not(feature = "wasm-embedded-v8")))]` keeps the Deno call at `:198`. `#[cfg(not(any(feature = "wasm-embedded-v8", feature = "wasm-embedded-js")))]` keeps the `EmbeddedRuntimeDisabled` error (`:214-221`). V8 takes precedence when both features are on, so the two functions never collide. |
| T8 | `src/wasm/mod.rs`: declare `pub mod v8_runtime;` beside `embedded_js` (`:15`). |
| T9 | Port the three tests that need only the core (and, for the trap test, the spike's minimal `setTimeout`): `embedded_v8_evaluates_js` (`tests/embedded_js_tests.rs:21`), `embedded_v8_runs_webassembly` (`:27`) and `a_trap_in_a_host_callback_keeps_the_verdicts_and_names_the_case` (`:188`). Keep `loader_runs_relative_import_graph_with_web_globals` (`:38`) and `embedded_runs_sample_fixture_end_to_end` (`:145`) for feature 02, and the three network tests (`:64`, `:85`, `:110`) for feature 03. |
| T10 | Add `v8_core_runs_a_module_with_an_op_and_a_timer`: stage a pure-JS runner that imports a relative module, calls `op_fwt_read_file` and `op_fwt_report`, and awaits the minimal `setTimeout`. The ported trap test (T9) proves the `read_diagnostics` path (`embedded_js.rs:367-402`). The 105-case comparison moves to feature 02. |
| T11 | CI: add `wasm-embedded-v8` and `wasm-embedded-js` to `runs` in `ci/packages.toml:434` while the move is open (decision D8). This runs the unit tests, not the harness cases. |
| T12 | Update `backends/foundation_testbed/README.md:72-77` and the feature list with `wasm-embedded-v8`. |

## 3. The test that proves the stage

`v8_core_runs_a_module_with_an_op_and_a_timer` in
`backends/foundation_testbed/tests/v8_core_tests.rs`:

1. Write a runner that imports `./helper.mjs`, calls `op_fwt_read_file` on a staged file,
   reports through `op_fwt_report`, and awaits the minimal `setTimeout`.
2. Run it through `v8_runtime::run_staged_harness`.
3. Assert the report and the file bytes.
4. The ported trap test proves that a throw after `op_fwt_report` keeps the verdicts and names
   the case through `read_diagnostics`.

The test uses only the core and the spike's minimal timer. The loader with Web globals and the
5-case fixture (`integration/fixtures/fwt_sample.wasm`) move to feature 02.

## 4. Verify

```bash
cargo test -p foundation_testbed --no-default-features \
  --features "wasm-embedded-js wasm-embedded-v8" --profile uat --test v8_core_tests
cargo tree -p foundation_testbed --no-default-features --features wasm-embedded-v8 \
  --edges normal | rg tokio
```

The second command prints no line. The first builds both paths in one binary, which proves the
three cfg forms do not collide.

## 5. Done when

- The new core test passes on V8.
- The three ported core tests pass on V8.
- The event loop resolves the minimal `setTimeout` and reports no tokio edge.
- Both feature sets run in CI (`ci/packages.toml:434`).
- The README names the new feature.
