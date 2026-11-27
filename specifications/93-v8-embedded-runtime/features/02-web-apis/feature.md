---
feature: "Web APIs: console, encoders, timers, URL, crypto and performance"
status: "pending"
priority: "high"
model: "sonnet"
phase: "2"
depends_on: ["01-v8-core"]
crates: ["foundation_testbed"]
---

# Feature 02: Web APIs — console, encoders, timers, URL, crypto and performance

_Written in ASD-STE100 Simplified Technical English (STE)._

## 1. Goal

Add the Web APIs that the host ABI and the runner read. After this feature, the unchanged
`foundation-wasm.js` (2,449 lines) loads in the V8 runtime, and `FoundationWasm` constructs.

This feature does not add `fetch` or `WebSocket`. Feature 03 adds them.

## 2. The measured surface

The table comes from `backends/foundation_wasm/runtime/foundation-wasm.js` and
`backends/foundation_testbed/src/wasm/templates/fwt/runner.js`.

| Global | Where it is used | Notes |
|---|---|---|
| `console` | `runner.js`, host ABI | `log`, `error`, `warn`, `debug` |
| `TextEncoder` | Host ABI | UTF-8 |
| `TextDecoder` | Host ABI `:339`, `:344`, `:1042`, `:1583` | Must accept `utf-16le` at `:344`, `:1042`, `:1583` |
| `setTimeout`, `clearTimeout` | Host ABI, tests | On the valtron clock |
| `setInterval`, `clearInterval` | Old bootstrap (`embedded_js.rs:103-106`) | On the valtron clock |
| `URL`, `URLSearchParams` | `runner.js:15` | Relative resolution against `import.meta.url` |
| `crypto.getRandomValues` | Host ABI `:2050`, old bootstrap (`embedded_js.rs:118-143`) | `foundation_compact::entropy::fill` |
| `performance.now` | Host ABI `:2036` | From `Ctx::clock().now()` |
| `performance.timeOrigin` | Host ABI `:2040` | From `Ctx::clock().wall_now()`, as epoch milliseconds |
| `Date.now` | Host ABI `:1668`, `:2038`, `:2043` | From `Ctx::clock().wall_now()`, as epoch milliseconds |
| `queueMicrotask` | Host ABI `:1476-1477` | `Isolate::EnqueueMicrotask` |
| `requestAnimationFrame` | Optional; the host ABI falls back to a timer (`:1660-1668`) | Do not add; the fallback is enough |

The old bootstrap also publishes `TextEncoderStream` and `TextDecoderStream`
(`embedded_js.rs:101-102`). No measured code uses them. Leave them out until a test needs
them.

## 3. Tasks

| Task | Work |
|---|---|
| T1 | Add `v8_runtime/web/mod.rs` with `install(context, ctx)`. Call it once after the context exists. |
| T2 | `v8_runtime/web/console.rs`: `console.log`, `info`, `warn`, `error`, `debug`. Write to standard output and standard error, or to `tracing` with the same text. |
| T3 | `v8_runtime/web/text.rs`: `TextEncoder` and `TextDecoder`. Support UTF-8 and UTF-16LE. Keep the Web standard contract: `encode` returns a `Uint8Array`; `decode` accepts an `ArrayBuffer` or a view and an options object. |
| T4 | `v8_runtime/web/timers.rs`: `setTimeout`, `setInterval`, `clearTimeout`, `clearInterval`. Put each timer in the event-loop queue keyed on `Ctx::clock().now()`, and wait with `Ctx::timer().sleep_until(ctx, deadline)` (`foundation_context/src/env/timer.rs:17-23`). Add a timer count limit as a configuration field (decision D6). |
| T5 | Add `url = { workspace = true }` to `backends/foundation_testbed/Cargo.toml`. Add `v8_runtime/web/url.rs`: `URL` and `URLSearchParams`, on the `url` crate (`Cargo.toml:166`). Resolve a relative URL against a base. |
| T6 | `v8_runtime/web/crypto.rs`: `crypto.getRandomValues`, with the same contract as the old shim: integer typed arrays only, at most 65,536 bytes per call, fill in place, return the view (`embedded_js.rs:118-143`). Use `foundation_compact::entropy::fill`. |
| T7 | `v8_runtime/web/performance.rs`: `performance.now()` from `Ctx::clock().now()`. `performance.timeOrigin` from `Ctx::clock().wall_now()` (`foundation_context/src/lib.rs:90`), as milliseconds since the Unix epoch. Replace `Date.now` with `Ctx::clock().wall_now()`, as epoch milliseconds; the host ABI reads it at `foundation-wasm.js:1668`, `:2038` and `:2043`. |
| T8 | `v8_runtime/web/microtask.rs`: `queueMicrotask` through `Isolate::EnqueueMicrotask`. Add it only if V8 does not install it. |
| T9 | `v8_runtime/event_loop.rs`: connect the timer queue and the microtask queue to the idle check from feature 01. |
| T10 | Add `backends/foundation_testbed/tests/v8_web_tests.rs` with the tests below. Port `loader_runs_relative_import_graph_with_web_globals` (`tests/embedded_js_tests.rs:38`) and `embedded_runs_sample_fixture_end_to_end` (`:145`). Add `v8_web_runs_every_non_network_case`: run the three integration crates and the filtered sample through both runtimes and compare `passed`, `failed`, `ignored` and the verdict lines. The total is 105 cases: 47 + 40 + 13 + 5. |
| T11 | Add a `test:wasm-testbed:v8` task to `mise.wasm.toml` with the same four runs as the three Deno tasks: the sample with `--filter passes` (`:23`), `foundation_core/integration/wasm_tests` (`:25`), `foundation_connectrpc/integration/wasm_owned` (`:99`) and `foundation_http/integration/wasm_owned` (`:148`), with `--features wasm-embedded-v8`. |

## 4. The tests that prove the stage

| Test | What it proves |
|---|---|
| `v8_web_globals_match_the_web_standard` | Each global exists, has the right shape, and the encoder and decoder round-trip UTF-8 and UTF-16LE. `performance.now()` increases. `crypto.getRandomValues` fills a view and rejects a `Float64Array` and a view over 65,536 bytes. `URL` resolves a relative path against a base. |
| `v8_host_abi_loads_and_constructs_foundation_wasm` | Stage `foundation-wasm.js` and an entry module. The entry imports `FoundationWasm` (`:1875`), `TestReports` (`:1682`) and `ParamType` (`:84`), constructs `new FoundationWasm()`, and asserts `typeof rt.web_abi === "object"` and `typeof TestReports.FAIL === "number"`. This proves the host ABI loads with the measured globals. |
| `loader_runs_relative_import_graph_with_web_globals` | Relative ESM, `TextEncoder`/`TextDecoder`, `setTimeout` and `console` work in process. |
| `embedded_runs_sample_fixture_end_to_end` | The 5-case `integration/fixtures/fwt_sample.wasm` fixture gives 3 passed, 1 failed and 1 ignored on V8. |
| `v8_web_runs_every_non_network_case` | The 105-case gate: 47 (`foundation_core`) + 40 (`foundation_http`) + 13 (`foundation_connectrpc`) + 5 (the sample without `echo_endpoint_answers`), with the same verdicts as Deno. |

## 5. Verify

```bash
cargo test -p foundation_testbed --no-default-features --features wasm-embedded-v8 \
  --profile uat --test v8_web_tests
cargo run -p foundation_testbed --no-default-features --features wasm-embedded-v8 \
  --profile uat --bin wasm-testbed -- deno backends/foundation_core/integration/wasm_tests
cargo run -p foundation_testbed --no-default-features --features wasm-embedded-v8 \
  --profile uat --bin wasm-testbed -- deno backends/foundation_http/integration/wasm_owned
cargo run -p foundation_testbed --no-default-features --features wasm-embedded-v8 \
  --profile uat --bin wasm-testbed -- deno backends/foundation_connectrpc/integration/wasm_owned
cargo run -p foundation_testbed --no-default-features --features wasm-embedded-v8 \
  --profile uat --bin wasm-testbed -- deno backends/foundation_testbed/integration/module --filter passes
MISE_ENV=wasm mise run test:wasm-testbed:v8
```

## 6. Done when

- The five tests are green.
- `foundation-wasm.js` loads unchanged and `FoundationWasm` constructs.
- `TextDecoder("utf-16le")` round-trips.
- `performance.now()` and `crypto.getRandomValues` read the injected clock and entropy, not a
  global.
- The 105-case gate stays green.
- The V8 mise task is green on the four runs.
- No `fetch` or `WebSocket` code is in this feature.
