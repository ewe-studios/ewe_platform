---
description: "Extend foundation_core's existing valtron test tree (native tests/valtron/) and its wasm integration crate (integration/wasm_tests/) with runtime checks — run headless via the wasm-testbed harness (embedded Deno) and in a browser (headless Chromium + Playwright) — so the parking bridge, WakeableStream/WakeRegistry, and the run_future → block_on_future rename plus run_future_iter wasm progress are proven at runtime on wasm32-unknown-unknown, not just compile-checked."
status: "proposed"
priority: "medium"
created: 2026-10-20
author: "Main Agent"
metadata:
  version: "1.0"
  last_updated: 2026-10-20
  estimated_effort: "medium"
  tags:
    - valtron
    - wasm
    - wasm32
    - browser
    - wasm-testbed
    - parking-bridge
    - wakeable-stream
    - wake-registry
    - lazy-streaming
    - js-event-loop
  skills:
    - rust-valtron-usage
  tools: []
has_features: false
has_fundamentals: false
builds_on: ""
related_specs:
  - "specifications/65-agent-rpc-serving-system"
  - "specifications/completed/31-wasm-testbed"
  - "specifications/completed/30-valtron-js-eventloop-yield"
  - "specifications/completed/64-valtron-stream-task"
tasks:
  completed: 0
  uncompleted: 4
  total: 4
  completion_percentage: 0
---

# Overview

Spec 65's `00-foundation` made `foundation_core`/`foundation_ai` *compile* on
`wasm32-unknown-unknown`, and its feature 06 (`wasm-async-streaming`) fixed the
T6 runtime deadlock by removing the `run_future`/`collect()` anti-pattern and
making the future-stream bridge **park**. But every one of those fixes is
verified today by a compile gate (`cargo check --target wasm32-unknown-unknown`)
plus, at most, one miniflare smoke test of a single streaming path.

**Nothing exercises valtron itself — the executor, the
`Stream`/`StreamIterator`/`WakeableStream` machinery, and the parking bridge —
at runtime on wasm or inside a browser.** That is the gap this spec closes, and
it is a real one, because the behaviours spec 65 depends on are exactly the ones
that only manifest at runtime on the single-threaded wasm executor:

- `into_future_stream` / `into_next_stream` must return `Poll::Pending` on
  `Stream::Wait`/`Pending`/`Init`/`Delayed` (yield to the JS event loop) and
  resume on wake; `into_pure_future_stream` stays a raw lens.
- `WakeableStream::wake` + `WakeRegistry` must fire exactly on readiness, with
  the self-wake fallback when a stream reports `NotSupported`.
- `run_future` → `block_on_future` must be absent from the wasm path (it is the
  sync-`collect()` trap); `run_future_iter` must make progress on wasm.

### Honest correction: the harness is `wasm-testbed`, not `foundation_ui_components`

The original feature-07 draft assumed `foundation_ui_components` was the
wasm/browser test harness to copy. **That is wrong.** `foundation_ui_components`
has no wired wasm/browser tests — only native Rust tests plus an orphaned
`node:test` file. The repo's real wasm/browser test harness is **`wasm-testbed`**
— the bespoke CLI at `backends/foundation_testbed/src/bin/wasm-testbed.rs` — and
the model crate is `backends/foundation_core/integration/wasm_tests/`. This spec
therefore **extends `foundation_core`'s existing test infrastructure in place**
rather than creating a new crate.

# Goals

- Add **native** parking/wake tests to `foundation_core/tests/valtron/` that pin
  the parking bridge, `WakeableStream`/`WakeRegistry`, and the
  `run_future` → `block_on_future` rename (overlapping spec 65 feature 06 T1).
- Add the **same behaviours** to `foundation_core`'s wasm integration crate
  (`integration/wasm_tests`) as owned `#[wasm_test]`/`#[valtron_wasm_test]`
  cases, and run them **headless** (embedded Deno) and **in a browser**
  (headless Chromium + Playwright) via `wasm-testbed`.
- Prove the parking-bridge contract (`Poll::Pending` on control states,
  resume-on-wake; raw lens forwards as-is) at runtime on wasm, not just native.
- Prove `WakeableStream`/`WakeRegistry` fire exactly on readiness, plus the
  `NotSupported` self-wake fallback, on wasm.
- Prove `block_on_future` is native-only (absent from the wasm build) and
  `run_future_iter` makes progress on wasm.
- Wire the wasm crate into the `test:wasm-testbed:deno` **and**
  `test:wasm-testbed:web` mise tasks, and record the CI gap (wasm runtime tests
  are local mise tasks only; CI stays compile-gate only).

# Requirements

1. **Extend, do not create.** All changes land in `foundation_core`'s existing
   test surface. No new crate, no new root-`Cargo.toml` `exclude` entry, no new
   package name.
2. **Two-tier split.** Tier A (native) lives in `tests/valtron/`; Tier B
   (wasm/web) lives in `integration/wasm_tests/`. The native tier is the
   authoritative, fast, hand-rolled-waker contract; the wasm tier re-proves the
   same contract at runtime on the single-threaded executor.
3. **Use the owned `#[wasm_test]` harness — zero wasm-bindgen.** The wasm cases
   are `#[wasm_test]`/`#[valtron_wasm_test]` fns driven by the `host_report` ABI
   (`foundation_wasm`) + `foundation_macros`, run by `wasm-testbed deno` /
   `wasm-testbed web`. `#[valtron_wasm_test]` is `#[wasm_test]` with valtron pool
   auto-init (foundation_macros F52) — use it for any case that needs a pool;
   use plain `#[wasm_test]` for manual-poll cases that need no pool. Never call
   `initialize_pool` manually in the new cases.
4. **The specific behaviours under test** (the spec-65 critical path):
   - `into_future_stream` / `into_next_stream` / `into_collect_future` /
     `into_ready_future` park (`Poll::Pending`) on
     `Wait`/`Pending`/`Init`/`Delayed` and resume on wake.
   - `into_pure_future_stream` forwards raw `Stream<D, P>` items as-is (never
     parks).
   - `WakeableStream::wake` + `WakeRegistry` fire exactly on readiness;
     `NotSupported` fallback self-wakes.
   - `block_on_future` (renamed `run_future`) is native-only and absent from the
     wasm build; `run_future_iter`/`ThreadedIterFuture` makes progress on wasm.
5. **Run on both runtimes.** The same `integration/wasm_tests` crate runs
   headless (embedded Deno) *and* in a browser (headless Chromium via
   Playwright). No separate JS driver file is needed — the `wasm-testbed`
   harness supplies the JS host for both paths.
6. **No production-code changes.** This is test-only; it must not modify
   `foundation_core`'s public API except where a test reaches something that
   legitimately needs to become public (rule 7: make the right thing public
   rather than inlining a test).

# Architecture

## Two-tier split

| Tier | Where | Target | Purpose |
|------|-------|--------|---------|
| A — native | `backends/foundation_core/tests/valtron/` | native host | Fast, deterministic parking/wake contract with a hand-rolled counting waker (already has `bridge_park.rs`, `wakeable_stream.rs`, `wake_registry.rs`, `cross_runtime.rs`) |
| B — wasm/web | `backends/foundation_core/integration/wasm_tests/` | `wasm32-unknown-unknown`, run headless (Deno) and in-browser (Chromium) | The same contract at runtime on the single-threaded `JSThreadYielder` executor |

There is also a third, related surface to keep in mind:
`backends/foundation_core/tests/valtron/wasm_js_yield_integration.rs` — the
`#[wasm_bindgen_test]` originals over the `js-wasmbindgen` backend that the
owned `#[wasm_test]` crate ports. It is native-compiled and unchanged by this
spec unless the ported cases require a matching tweak.

## The harness (`wasm-testbed`)

`backends/foundation_testbed/src/bin/wasm-testbed.rs` has two relevant paths:

1. **Owned `#[wasm_test]`** (zero wasm-bindgen, via `foundation_wasm`'s
   `host_report` ABI + `foundation_macros::wasm_test`), run headless via embedded
   Deno (`--features wasm-embedded-js`) or in a browser via headless Chromium +
   Playwright (`--features wasm`).
2. A `bindgen` wasm-bindgen browser path (`--features wasm,wasm-bindgen-test`).

This spec uses path 1 only. The `bindgen` path is out of scope (see Out of
scope).

## The model crate (`integration/wasm_tests`)

`backends/foundation_core/integration/wasm_tests/` (package
`foundation_core_wasm_tests`) is the existing standalone crate to extend. Its
shape (already in place):

```toml
[lib]
crate-type = ["cdylib"]

[dependencies]
foundation_core = { path = "../..", features = ["js-foundation-wasm"] }
foundation_wasm = { path = "../../../foundation_wasm", features = ["web"] }
foundation_macros = { path = "../../../foundation_macros" }

[workspace]                      # standalone: own empty workspace, excluded from root

[profile.uat]
inherits = "release"
codegen-backend = "llvm"         # wasm needs LLVM; root dev profile is Cranelift
```

It currently holds only the ported JS-yield cases
(`js_yield_returns_immediately`, `js_yield_callback_reschedules_executor`,
`concurrent_tasks_make_progress`, `no_deadlock_with_js_yield`). The parking
bridge, wake, and `block_on_future`/`run_future_iter` behaviours are **not yet
covered** there — that is what this spec adds.

## The parking-bridge test template

`backends/foundation_core/tests/valtron/bridge_park.rs` is the native template to
port. It uses:

- `CountingWaker` (implements `std::task::Wake`) to prove park-vs-spin by
  counting every wake.
- `NotifyQueueStreamIterator` with `set_max_spins(1)`, so an empty `next()`
  yields `Stream::Wait` after one short CondVar wait — single-threaded, fast,
  deterministic (the test itself pushes/closes between polls, no producer
  thread).
- The assertion shape: `Poll::Pending` with wake-count 0, then `push`/`close`
  fires exactly one wake, and the re-poll resolves; a `NotSupported` iterator
  self-wakes (wake count 1).

This single-threaded design maps directly onto the single-threaded wasm
executor: on wasm the "producer" that wakes the parked bridge is the JS event
loop (a timer/fetch/promise), but the park-then-wake contract is identical.

# Implementation Location

All in `backends/foundation_core/`, extended in place:

- `backends/foundation_core/tests/valtron/` — native parking/wake tests (Tier A).
  Extend `bridge_park.rs` (and, where the 06 rename requires, `stream_future.rs`,
  `wakeable_stream.rs`, `wake_registry.rs`) with any missing contract coverage;
  register new modules in `tests/valtron/mod.rs`.
- `backends/foundation_core/integration/wasm_tests/src/lib.rs` — wasm/web cases
  (Tier B). Add `#[wasm_test]`/`#[valtron_wasm_test]` fns porting the parking
  bridge, wake, and `run_future_iter` behaviours.
- `mise.toml` — wire `integration/wasm_tests` into the browser mise task (it is
  already in the Deno task; see T3).
- No change to the root `Cargo.toml` `exclude` list (the crate is already
  excluded as `backends/foundation_core/integration/wasm_tests`).

# Tasks

- [ ] **T1 — native parking/wake tests (`tests/valtron/`).** Extend the native
      tier with any parking-bridge and wake contract not yet pinned after the 06
      rename (`into_future_stream` parks / `into_pure_future_stream` stays raw,
      `block_on_future` is documented native-only). Overlaps spec 65 feature 06
      T1. Gate: `cargo test -p foundation_core` green.
- [ ] **T2 — wasm/web cases (`integration/wasm_tests`).** Port the parking-bridge
      contract, the `WakeableStream`/`WakeRegistry` + `NotSupported` self-wake
      fallback, and the `run_future_iter` wasm progress into
      `src/lib.rs` as `#[wasm_test]`/`#[valtron_wasm_test]` cases, using the
      `CountingWaker` + `set_max_spins(1)` template. `block_on_future` absence is
      enforced by it not existing in the wasm build (gated native-only), covered
      by the wasm compile gate rather than a runtime test. Gate: `wasm-testbed
      deno` green.
- [ ] **T3 — mise wiring.** `integration/wasm_tests` is already in
      `test:wasm-testbed:deno` (`mise.toml`); add it to `test:wasm-testbed:web`
      (currently that task runs only `foundation_testbed/integration/module`).
      Gate: `mise run test:wasm-testbed:deno` and `mise run test:wasm-testbed:web`
      green locally.
- [ ] **T4 — CI gap note.** CI is compile-gate only (`ci/packages.toml` +
      `scripts/ci/ci.py --step wasm` → `cargo check --target
      wasm32-unknown-unknown`), and `integration/wasm_tests` is a standalone
      crate outside the root workspace, so the wasm runtime tests are local mise
      tasks only. Record this gap (and any compile-gate blind spot for the
      standalone crate) in the spec/backlog rather than attempting to run the
      harness in CI. Gate: CI stays green; the gap is documented.

# Testing

## Test cases

1. **Parking on the wasm target.** `into_future_stream`/`into_next_stream`/
   `into_collect_future`/`into_ready_future` return `Poll::Pending` on
   `Wait`/`Pending`/`Init`/`Delayed` and resume on wake (hand-rolled
   `CountingWaker`).
2. **Raw lens unchanged.** `into_pure_future_stream` forwards `Wait`/`Pending` as
   items.
3. **Wake contract.** A `WakeableStream` fires its `WakeRegistry` exactly on
   readiness; a `NotSupported` stream falls back to self-wake and still makes
   progress.
4. **No sync-collect on wasm.** `block_on_future` does not exist in the wasm
   build (native-only gate, verified by the wasm compile gate); `run_future_iter`
   drives a future to completion with repeated polls on wasm.
5. **Both runtimes.** The same `#[wasm_test]` cases pass headless (embedded
   Deno) and in-browser (headless Chromium + Playwright), proving the executor
   yields and resumes in a real JS environment.

## Local run commands

```bash
# native parking/wake tests (Tier A)
cargo test -p foundation_core valtron

# wasm runtime, headless embedded Deno (Tier B)
cargo run -p foundation_testbed --no-default-features --features wasm-embedded-js \
  --profile uat --bin wasm-testbed -- deno backends/foundation_core/integration/wasm_tests

# wasm runtime, browser (headless Chromium + Playwright) — needs the Playwright setup first
mise run setup:playwright
cargo run -p foundation_testbed --no-default-features --features wasm \
  --bin wasm-testbed -- web backends/foundation_core/integration/wasm_tests --headless

# CI compile gate (unchanged; wasm runtime tests are NOT in CI)
python scripts/ci/ci.py --step wasm
```

Notes:

- `--profile uat` is required for any wasm build: the root `[profile.dev]` is
  Cranelift, which cannot target `wasm32-unknown-unknown`; `uat` uses LLVM. The
  existing `test:wasm-testbed:web` mise task omits `--profile uat` — when T3
  touches that task, add `--profile uat` to its command and record the omission
  as a bug backlog item if it is confirmed to matter.
- The Deno run command above already matches the existing
  `test:wasm-testbed:deno` task (which already runs `integration/wasm_tests`).

# Success criteria

- [ ] Native parking/wake tests in `tests/valtron/` pin the parking bridge,
      `WakeableStream`/`WakeRegistry`, and the `block_on_future` rename
      (`into_future_stream` parks; `into_pure_future_stream` stays raw).
- [ ] `integration/wasm_tests` gains `#[wasm_test]`/`#[valtron_wasm_test]` cases
      asserting the same parking/wake contract at runtime on wasm, and they pass
      via `wasm-testbed deno`.
- [ ] The same cases pass via `wasm-testbed web --headless` (real browser/JS
      environment), proving the single-threaded executor yields and resumes.
- [ ] `run_future_iter` progress is covered on wasm; `block_on_future` is
      confirmed native-only via the wasm compile gate.
- [ ] `test:wasm-testbed:deno` and `test:wasm-testbed:web` mise tasks both run
      `integration/wasm_tests`; the CI gap (runtime tests local-only) is
      documented.
- [ ] No new crate, no production-code change to `foundation_core` except making
      genuinely-needed test surfaces public (rule 7); no inline tests in source
      files.

# Out of scope

- **A new crate.** The work extends `foundation_core`'s existing `tests/valtron/`
  and `integration/wasm_tests/`; there is no `foundation_valtron_wasm_stream_tests`
  crate and no new root-`Cargo.toml` `exclude` entry.
- **The `bindgen` wasm-bindgen-test browser path** (`--features
  wasm,wasm-bindgen-test`). The owned `#[wasm_test]` path covers the same runtime
  in both Deno and a real browser; the bindgen path is only revisited if a
  wasm-bindgen-specific surface must be exercised.
- **`foundation_netio`'s `WasmSseIterator` / lazy `send_sse`.** Those are
  exercised by `foundation_netio`'s own tests and the `cf-cloud-provider`
  example; dragging `foundation_netio` into the `integration/wasm_tests` crate
  is out of scope here (tracked as a possible follow-on if a browser-side SSE
  bridge test is wanted).
- **Running the wasm/browser runtime tests in CI.** CI remains compile-gate only;
  the runtime tests are local mise tasks. Closing that gap is a separate,
  larger decision (a headless-browser CI stage), not part of this spec.

---

_Created: 2026-10-20_
_Structure: Single requirements.md (has_features: false)_
