---
feature: "Cutover: V8 is the default and Deno is gone"
status: "pending"
priority: "high"
model: "sonnet"
phase: "4"
depends_on: ["03-fetch-and-websocket"]
crates: ["foundation_testbed", "foundation_ci"]
---

# Feature 04: Cutover — V8 is the default and Deno is gone

_Written in ASD-STE100 Simplified Technical English (STE)._

## 1. Goal

Make the V8 runtime the only embedded runtime. Delete the Deno module and its crates. Remove
tokio from this test path. Update the docs. Close issue #139.

After this feature, `wasm-testbed deno` runs the owned harness on V8, in process, with no
Deno crate and no tokio in the feature tree. The command name stays, because contributors and
CI use it (`mise.wasm.toml:18`, `recipes.rs:143`).

The separate `src/wasm/deno.rs` shells out to the external `deno` binary for wasm-bindgen
interop. It is out of scope (issue #139). Keep it.

## 2. Tasks

| Task | Work |
|---|---|
| T1 | `backends/foundation_testbed/Cargo.toml`: rename the feature to `wasm-embedded-v8` (or make it the default for `wasm`; decision D2). Remove the eight direct Deno crates (`:126-142`), `tokio` (`:142`), `sys_traits` (`:139`) and `rustls` (`:140`). Keep `foundation_compact` for entropy. |
| T2 | Delete `backends/foundation_testbed/src/wasm/embedded_js.rs`. In `src/wasm/mod.rs:15`, replace `pub mod embedded_js;` with `pub mod v8_runtime;`. Delete `tests/embedded_js_tests.rs`; its tests now live in `v8_core_tests.rs` and `v8_web_tests.rs`, and its import at `:15` names the deleted module. In `tests/fwt_runner_tests.rs`, change the two gates at `:35` and `:65` from `wasm-embedded-js` to `wasm-embedded-v8`. |
| T3 | `src/wasm/fwt_runner.rs`: remove the Deno and disabled branches. Keep one `run_deno` that calls `v8_runtime::run_staged_harness` (`:198`). Keep `server::start_serving_with_echo` (`:196`) and `write_harness_json` (`:197`). |
| T4 | `mise.wasm.toml`: rename `test:wasm-testbed:deno` (`:18`), `test:wasm-testbed:connectrpc:deno` (`:95`) and `test:wasm-testbed:http:deno` (`:144`) to the V8 feature. Keep the task aliases so the documented commands still work. Remove the extra V8 task that feature 02 added, or keep it as the only task. |
| T5 | `backends/foundation_ci/src/execution/recipes.rs:143`: replace `"wasm-embedded-js"` with `"wasm-embedded-v8"`. |
| T6 | `ci/packages.toml:426-434`: set `runs` to `["", "vms", "wasm-embedded-v8"]` and remove the old feature. The `windows-tests = "none"` opt-out with reason #531 stays (root rule 15). |
| T7 | Docs: rewrite `backends/foundation_testbed/docs/embedded-js.md`, `embedded-js-internals.md`, `extending-the-runtime.md` and `running-js-and-wasm-tests.md` for the V8 runtime. Update the feature row in `backends/foundation_testbed/README.md:72-77` and the note in `ci/ewe-platform-image/README.md:60`. Keep the file names, or rename them and update every link; do not leave a dead link. |
| T8 | Add a dated note to `specifications/completed/44-embedded-deno-runtime/spec.md`: spec 93 replaces this runtime. Update the embedded-runtime row of the runner table in `specifications/completed/88-wasm-runtime-coverage/requirements.md:80-90` to name V8. |
| T9 | Add the dependency guard to the CI wasm recipe: `cargo tree -p foundation_testbed --no-default-features --features wasm-embedded-v8 --edges normal | rg 'deno_|tokio'` must print nothing. The step fails on a line. |
| T10 | Close issue #139 in this change. The closing comment gives the new line count, the test commands and the result. |

## 3. The test that proves the stage

`cutover_runs_the_full_owned_matrix_on_v8` in
`backends/foundation_testbed/tests/v8_web_tests.rs` (the feature 02 test, now without the
Deno comparison):

1. Run the four measured case files on the default V8 build: `foundation_core` (47),
   `foundation_http` (40), `foundation_connectrpc` (13) and the sample (6).
2. Assert 106 of 106 cases and the same verdicts as the recorded Deno run.
3. Assert that `tests/v8_core_tests.rs`, `tests/v8_web_tests.rs` and
   `tests/fwt_runner_tests.rs` run with the new feature in the package matrix.

The CI dependency guard (T9) is the second proof. A `deno_*` or `tokio` line fails the job.

## 4. Verify

```bash
cargo test -p foundation_testbed --no-default-features --features wasm-embedded-v8 \
  --profile uat
MISE_ENV=wasm mise run test:wasm-testbed:deno
cargo tree -p foundation_testbed --no-default-features --features wasm-embedded-v8 \
  --edges normal | rg 'deno_|tokio'
rg -n 'deno_(core|web|webidl|fetch|net|websocket|permissions|error)|dep:tokio' \
  backends/foundation_testbed/Cargo.toml
```

The third command prints no line. The fourth prints no line.

## 5. Done when

- `embedded_js.rs` is deleted and `v8_runtime` is the only embedded runtime.
- Eight Deno crates, `tokio`, `sys_traits` and `rustls` are gone from
  `foundation_testbed/Cargo.toml`.
- `cargo tree` prints no `deno_*` and no `tokio` line for the feature.
- All 106 cases pass on V8, and the ported runtime tests plus the 2 runner tests run in CI.
- The mise tasks and the CI recipe use `wasm-embedded-v8`.
- The docs name V8 and have no dead link.
- Issue #139 is closed with the line count and the result.
