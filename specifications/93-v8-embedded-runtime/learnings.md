# Spec 93 Learnings

## Nothing is implemented

This file records what the planning work found. No code changed.

## Why this spec exists (2026-11-27)

Issue #139 asks to replace `deno_core` with the `v8` crate on a valtron event loop. The
assessment on the issue recommends a one-group bump of the Deno set to `deno_core` 0.413 and
keeps the move as a contingency. The owner asked for the full plan so the choice (D1) has a
measured basis.

## Measured facts that changed the plan

1. **The direct Deno count is eight, not nine.** `Cargo.toml:126-142` lists `deno_core`,
   `deno_web`, `deno_webidl`, `deno_fetch`, `deno_net`, `deno_websocket`, `deno_permissions`
   and `deno_error`. The supporting crates are `tokio`, `sys_traits` and `rustls`.
2. **The test inventory is 106 cases in four files, not "about 20 files".** The four files
   hold 47, 40, 13 and 6 cases. The 47 in `foundation_core` are 34 `#[wasm_test]` plus 13
   `#[valtron_wasm_test]`; a count that reads only the first attribute finds 34. The sample
   template holds three more markers, but it is scaffolding.
3. **One case uses the network.** `echo_endpoint_answers` sends a POST to the testbed's local
   echo server. The other 105 cases are in memory. This is why the core stage cannot gate on
   the cases; the Web API stage (feature 02) does.
4. **The runtime unit tests do not run in CI today.** `ci/packages.toml:434` runs
   `foundation_testbed` with `["", "vms"]`, so `wasm-embedded-js` is off and the 8 tests in
   `tests/embedded_js_tests.rs` are compiled out. The wasm recipe runs the binary instead.
   Both feature sets must join the matrix during the move.
5. **The engine is already in the lock.** `v8` 149.4.0 comes with `deno_core` 0.404. A direct
   dependency at the same version reuses the prebuilt library, so the build does not get a
   second engine.
6. **The host ABI needs UTF-16LE.** `foundation-wasm.js` calls
   `new TextDecoder("utf-16le")` at `:344`, `:1042` and `:1583`. A UTF-8-only decoder fails
   the host ABI.
7. **The host ABI reads `response.ok`, `response.status` and `body.cancel()` before it
   streams a module** (`foundation-wasm.js:2327-2347`). The fetch stage must supply them.
8. **`WebAssembly.instantiateStreaming` with a hand-written `Response` is not proven.** The
   runner does not use it, but `loadURL` does. Phase 3 tests it and records the result.
9. **The harness cases need the Web APIs.** The runner and the host ABI read `setTimeout`,
   `TextDecoder`, `performance` and `console`. The core stage cannot run them. The 105-case
   gate belongs to the Web API stage (feature 02), not the core stage.
10. **`performance.timeOrigin` and `Date.now` are wall-clock values.** They need
    `Clock::wall_now()` (`foundation_context/src/lib.rs:90`), not the monotonic `now()`.

## Dead ends to avoid

- Do not copy Deno's JavaScript for the Web APIs. It depends on `Deno.core` internals and
  primordials (issue #139).
- Do not drive the V8 event loop with a synchronous loop that continues on `Wait`. It spins
  on a one-thread host (root `CLAUDE.md` rule 6). Drive a future on valtron, or park.
- Do not put V8 handles in a channel. An isolate is `!Send`. Send bytes, strings and JSON
  only.
- Do not delete the network tests to make a phase green. Keep them on the Deno feature until
  V8 passes them.

## Open

The owner has not settled D1 to D8 in `requirements.md`. The first useful measurement is the
spike (feature 00): line count, wall time, `import.meta.url`, and dynamic `import()`.
