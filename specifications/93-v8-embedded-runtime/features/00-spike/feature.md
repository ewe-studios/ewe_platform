---
feature: "Spike: isolate, module, op and timer on valtron"
status: "pending"
priority: "high"
model: "sonnet"
phase: "0"
depends_on: []
crates: ["foundation_testbed"]
---

# Feature 00: Spike — isolate, module, op and timer on valtron

_Written in ASD-STE100 Simplified Technical English (STE)._

## 1. Goal

Prove four primitives before the full move:

1. One V8 isolate runs JavaScript.
2. One ESM file loads from disk with a relative import and a correct `import.meta.url`.
3. One JavaScript call reaches one Rust op.
4. One `setTimeout` resolves on a valtron event loop, with no tokio.

If the spike fails, the owner stops at decision D1 with a measured answer. The spike is about
one hour of work (issue #139, step 1).

## 2. Tasks

| Task | Work |
|---|---|
| T1 | In the root `Cargo.toml` `[workspace.dependencies]`, add `v8 = "149.4"`. The lock file already holds `v8` 149.4.0 (`Cargo.lock:17664-17666`). In `backends/foundation_testbed/Cargo.toml`, add `v8 = { workspace = true, optional = true }` and a feature `wasm-embedded-v8 = ["wasm", "dep:v8", "dep:foundation_compact"]`. `foundation_core` and `foundation_context` are already non-optional dependencies (`Cargo.toml:84-85`). |
| T2 | Add `backends/foundation_testbed/src/wasm/v8_runtime/mod.rs` and `isolate.rs`. In `isolate.rs`, add `Runtime::new()`: `v8::Isolate::new(v8::CreateParams::default())`, one context, a handle scope and a `TryCatch`. |
| T3 | In `v8_runtime/loader.rs`, add `load_entry(path)`: resolve the file URL, read the bytes with `std::fs::read`, compile with `v8::ScriptCompiler::CompileModule`, instantiate, evaluate. Set the script origin resource name so `import.meta.url` is the file URL. Support one relative import. |
| T4 | In `v8_runtime/ops.rs`, add `op_fwt_report(json)` as a `v8::FunctionTemplate`, and expose it at `globalThis.Deno.core.ops.op_fwt_report`. The name matches the runner's feature test (`runner.js:19`, `:42`). |
| T5 | In `v8_runtime/event_loop.rs`, add `async fn drive(runtime)`: run the microtask checkpoint, arm one timer with `Ctx::timer().sleep_until(ctx, deadline)` (`foundation_context/src/env/timer.rs:17-23`), fire it, and return when idle. The deadline comes from `Ctx::clock().now()` (`foundation_context/src/lib.rs:474`). Expose a minimal `setTimeout` global that adds a timer to that queue. Feature 02 replaces it with the full timer API. |
| T6 | Drive the future with `foundation_core::valtron::executors::non_sendables::block_on_future` (`:461`) in the spike test. Do not add tokio. |
| T7 | Add `backends/foundation_testbed/tests/v8_runtime_tests.rs` with the test below. |
| T8 | Measure the line count and the wall time of the spike. Report both to the owner, with a note on `import.meta.url` and dynamic `import()`. |

## 3. The test that proves the stage

`spike_runs_module_op_and_timer` in `tests/v8_runtime_tests.rs`:

1. Write `util.mjs` with one exported function and `entry.mjs` with
   `import { f } from "./util.mjs"`.
2. `entry.mjs` calls `Deno.core.ops.op_fwt_report("spike")` and then
   `await new Promise((resolve) => setTimeout(resolve, 1))`.
3. Run the entry through `Runtime::new()` and `drive`.
4. Assert that the report sink holds `"spike"` and that the timer resolved.

The test proves the isolate, the loader, the ops bridge and the valtron loop in one run. It
does not use the Deno runtime, tokio or a subprocess. `Deno.core.ops` is only the global path
that the runner expects (`runner.js:19`).

## 4. Verify

```bash
cargo test -p foundation_testbed --no-default-features --features wasm-embedded-v8 \
  --profile uat --test v8_runtime_tests spike
cargo tree -p foundation_testbed --no-default-features --features wasm-embedded-v8 \
  --edges normal | rg tokio
```

The second command prints no line. A `tokio` line means the spike still pulls it through a
dependency and the gate fails.

## 5. Done when

- `spike_runs_module_op_and_timer` is green.
- The feature tree prints no `tokio` line.
- The report gives the line count, the wall time, and the result for `import.meta.url` and
  dynamic `import()`.
- The owner has the result for decision D1.
