---
feature: "Valtron Runtime"
this_file: "specifications/88-wasm-runtime-coverage/features/01-valtron-runtime/start.md"
spec: "specifications/88-wasm-runtime-coverage/requirements.md"
model: "sonnet"
branch: "spec/split-65"
created: 2026-11-15
---

# Start: Valtron Runtime

You own feature 01 of spec 88. Feature 00 must be merged before you start: you run its
runners and you call its `--filter`.

## Mission

Prove the valtron runtime on embedded Deno, headless Chromium and workerd. The wasm-bindgen
suite exists and no step runs it. The owned suite runs 18 shallow cases. You make the first
run on all hosts and you deepen both suites where parking, waking, backpressure, cancellation,
streams and `#[task]` are thin.

You do not change `foundation_http` or `foundation_connectrpc`. You may change `foundation_core`
when a case proves a defect (feature section 6.7).

## Required reading, in this order

1. `requirements.md` sections 2.3, 2.7, 4 (D2), 5, 6 and 9.
2. `features/01-valtron-runtime/feature.md` in full.
3. `.agents/skills/rust-valtron-usage/skill.md`. Its test section is older than the code: on
   wasm the cases are `#[wasm_test]`, `#[valtron_wasm_test]`, `#[wasm_bindgen_test]` and
   `#[valtron_bindgen]`; `#[valtron_test]` is native only.
4. These files, before you change them:
   - `backends/foundation_core/integration/wasm_tests/src/lib.rs`
   - `backends/foundation_core/tests/valtron/wasm_wake_support.rs`
   - `backends/foundation_core/tests/valtron/wasm_js_yield_integration.rs`
   - `backends/foundation_core/tests/valtron/mod.rs`
   - `backends/foundation_core/tests/valtron/state_task.rs`
   - `backends/foundation_core/tests/valtron/async_first_api.rs`

## Owned paths

- `backends/foundation_core/integration/wasm_tests/` (the standalone owned crate)
- `backends/foundation_core/tests/valtron/`
- `backends/foundation_core/src/valtron/` — only for a defect that a case proves
- `mise.toml`, `mise.wasm.toml`, `scripts/ci/ci.py` — only the tasks this feature adds

You do not write `foundation_http`, `foundation_connectrpc`, `foundation_testbed` or
`foundation_browser`. Report a needed change there.

## Task order

| Task | One line | Run before you continue |
|---|---|---|
| T1 | run the `mod` target on Deno, Chromium and workerd; triage every red case | `MISE_ENV=wasm mise run test:wasm-bindgen:core` |
| T2 | add six shared scenarios and wire each into both suites | both hosts run the scenario |
| T3 | add eight owned-only cases | `mise run test:wasm-testbed:deno` |
| T4 | prove `#[task]` on a JS-hosted wasm build | `test bindgen-deno` with the filter |
| T5 | run the 19 `dual!` cases and record the count | `test:wasm-bindgen:web` |
| T6 | record the R14 break per host, then revert it | each host red, then green |

T1 comes first. A target that never compiles hides every error behind it. T4 may become a
`foundation_core` fix; that is in scope.

## Rules that decide your choices

| Rule | What it means for you |
|---|---|
| R1 | no fake host; the real `setTimeout`, `Date.now` and `Promise` are what you test |
| R2 | every case names its oracle: an example, a bound, a model, a fault |
| R3 | a failure names the host, the case and the first unexpected value |
| R4 | every park has a bound; a case that spins panics with its name |
| R5 | no case needs the network |
| R9, R10 | one test target per crate; the owned cases sit in the lib for the cdylib |
| R14 | one deliberate break per suite, recorded per host |
| R15 | the owned module imports only `abi` |
| root 7 | no inline `#[cfg(test)] mod tests` |
| root 10 | update the spec's `progress.md` and `LEARNINGS.md` |
| root 12 | write in ASD-STE100 Simplified Technical English (STE) |
| root 13 | no AI attribution anywhere |

## Environment facts

- `MISE_ENV=wasm` selects `mise.wasm.toml`.
- The pinned Deno is 2.9.7 and the pinned wasm-bindgen CLI is 0.2.126.
- `wrangler` runs with `dev --local`; its local bindings are `DB`, `BUCKET` and `KV`.
- `git status` fails on the `whisper.cpp` symlink. Scope it by path.
- Stop a process by its saved PID, never with `pkill -f`.

## Do not

- do not commit or push.
- do not mark a case `#[ignore]`.
- do not gate a module off wasm without an issue number and a reason.
- do not change a scenario to match a host. If the two hosts disagree, one of them is wrong;
  find which, and record it.

## Report

1. Per task: files changed, the command run, its result (one line each).
2. The case count of the `mod` target on each host, before and after.
3. Every red case you triaged: the reason class (harness gap, layer defect, missing tool), the
   issue number, and what you did.
4. Every `foundation_core` change, with the failing case that proves it.
5. The R14 record: the break, the case, the red output per host.
6. `git diff --stat` for your owned paths.

---

_Created: 2026-11-15_
