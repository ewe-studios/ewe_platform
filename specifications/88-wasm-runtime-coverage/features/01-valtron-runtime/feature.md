---
feature: "Valtron Runtime"
description: "Deep valtron cases on the embedded Deno host, the headless Chromium host and workerd: the existing wasm-bindgen suite runs on all three, the owned suite grows to cover parking, the wake registry, backpressure, cancellation, streams and #[task] transitions."
status: "todo"
priority: "high"
depends_on: ["00-executors"]
estimated_effort: "large"
created: 2026-11-15
last_updated: 2026-11-16
author: "Main Agent"
model: "sonnet"
tasks:
  completed: 0
  uncompleted: 6
  total: 6
  completion_percentage: 0%
---

# Valtron Runtime

_Status: **todo**, feature 01 of spec 88. Written in ASD-STE100 Simplified Technical English
(STE). Code and identifiers stay as they are._

**Abbreviations.** JavaScript (JS); WebAssembly (wasm); Simplified Technical English (STE);
continuous integration (CI).

## 1. Goal

Prove the valtron runtime on the three hosts:

| Host | Harness | Suite |
|---|---|---|
| embedded Deno | owned `#[wasm_test]` | `foundation_core/integration/wasm_tests` |
| headless Chromium | owned `#[wasm_test]` | the same crate |
| embedded Deno and workerd | wasm-bindgen | `foundation_core/tests/` (the `mod` target) |
| headless Chromium | wasm-bindgen | the same `mod` target |

The work is not only new cases. The `mod` target holds about forty wasm-eligible modules and
no CI step builds or runs it on any host (#575). This feature makes it run, then deepens both
suites where they are thin.

## 2. Story

| ID | Actor | Trigger | Result |
|---|---|---|---|
| S1 | CI | a pull request touches `foundation_core` | the `mod` target runs on Deno, Chromium and workerd |
| S2 | a contributor | runs one case by name | `--filter` selects it on every runner |
| S3 | a case | parks on a `ResumeSignal` that a host timer sets | it resumes, and the loop was not blocked |
| S4 | a case | cancels a parked task | the task ends, its park is released and the registry drops the waker |
| S5 | a case | fills a bounded queue while the consumer is parked | the producer parks, then resumes; no item is lost |
| S6 | a case | drops the only waiter of a registry entry, then wakes it | no panic, and the count drops to zero |
| S7 | a case | runs a `#[task]` transition on a JS-hosted wasm build | the Ready and Pending states are observed in order |
| S8 | a maintainer | breaks `WakeRegistry::wake_all` | one named case turns red on each host (R14) |
| S9 | a reader | reads a failure | the message names the host, the case and the first unexpected value (R3) |

## 3. Bounded context

### 3.1 Inside

| Entity | Meaning | Owner (one writer) |
|---|---|---|
| `integration/wasm_tests/src/lib.rs` | the owned suite: 18 cases today | this feature |
| `tests/valtron/wasm_wake_support.rs` | the scenarios both suites share | this feature |
| `tests/valtron/wasm_js_yield_integration.rs` | the bindgen twin: 15 cases today | this feature |
| `tests/valtron/async_first_api.rs` | 19 `dual!` async-API cases | this feature |
| `tests/valtron/state_task.rs` | `#[task]` transitions, native only | this feature (a wasm-eligible part) |
| `tests/valtron/mod.rs` | the module list and its target gates | this feature |
| the mise task `test:wasm-bindgen:core` | runs the `mod` target on a host | this feature |

### 3.2 Outside

| Thing | Owner | This feature touches it? |
|---|---|---|
| the runners and their flags | feature 00 | no; it calls them |
| the `/echo` endpoint | feature 00 | no; no valtron case needs the network (R5) |
| `foundation_http`, `foundation_connectrpc` | features 02 and 03 | no |
| the valtron implementation itself | `foundation_core/src/valtron` | only when a case proves a defect (see 6.7) |

### 3.3 Verbs

| Verb | Kind | Effect |
|---|---|---|
| `run` | story verb | runs one suite on one host and reports a verdict |
| `park` | story verb | suspends a task until a wake, a deadline or both |
| `wake` | entity verb | marks one waiter ready |
| `cancel` | entity verb | ends a task and releases its park |
| `transition` | entity verb | moves a `#[task]` from one state to the next |
| `verify` | entity verb | checks the scenario's counts and the recorded order |

## 4. Rules

### 4.1 Law worksheet

```
Entity: a parked task
Conserved value:    wakes delivered == wakes recorded by the waiter (no lost, no extra)
Bound:              every park ends by its wake or its deadline; the case fails past 3 s
Inverse pairs:      park / wake; cancel / release
Idempotent verbs:   wake a task that is already ready (a second wake does not double-run it)
Ordering pairs:     register before the action that wakes; wake before the deadline
Monotone values:    the wake count of a waiter
Freeze points:      the scenario's recorded order does not change after it ends
Links to other entities: the suite's verdict, the executor's counters and the host clock

Entity: a wasm test suite
Conserved value:    discovered cases == passed + failed + ignored
Bound:              the fleet runs every case it discovered
Inverse pairs:      none
Idempotent verbs:   a re-run starts from a fresh module instance
Ordering pairs:     build before run; stage before run
Monotone values:    none
Freeze points:      the verdict does not change after the last case
Links to other entities: the mise task, the CI step and the printed summary agree
```

### 4.2 Invariants

- **I1.** A case that parks resumes; no case hangs past its stated bound.
- **I2.** The same scenario yields the same recorded order on both harnesses.
- **I3.** A dropped waiter never panics a wake, and the registry's count returns to zero.
- **I4.** A bounded queue never loses or reorders an item under backpressure.
- **I5.** The owned module imports only the `abi` module (R15).
- **I6.** Every wasm-eligible module in `tests/valtron/mod.rs` runs on at least one host, or
  carries a target gate with the reason and the issue number.

### 4.3 Must have

- the `mod` target green on Deno, Chromium and workerd, or a named issue for each case that
  cannot pass;
- at least six new shared scenarios, each run by both suites;
- at least eight new owned cases;
- at least two `#[task]` cases that run on a JS-hosted wasm build;
- one use of `#[valtron_wasm_test]` (it has no user today);
- the R14 break recorded with its red output per host.

### 4.4 Must do

- put each new shared scenario in `wasm_wake_support.rs` so both suites run it;
- name the host in every assertion message (R3);
- bound every wait and count every wake (R4);
- keep one test target per crate (R10);
- update `progress.md` and `LEARNINGS.md` before you stop (root rule 10).

### 4.5 Must not

- must not add a `#[cfg(test)] mod tests` to a source file (root rule 7);
- must not add a case that needs the network (R5);
- must not replace a host function with a stub (R1);
- must not gate a module out of wasm without an issue number and a reason (I6);
- must not weaken a case to make a host green. A host that fails for a real reason gets an
  issue, and the case says so.

## 5. Acceptance scenarios

```gherkin
Scenario: The wasm-bindgen suite runs on Deno
  Given a clean checkout
  When I run "mise run test:wasm-bindgen:core"
  Then the Deno runner prints "test result: ok"
  And the number of cases is at least the number the target discovers natively

Scenario: The wasm-bindgen suite runs on workerd
  Given a clean checkout
  When I run "test bindgen-wrangler backends/foundation_core"
  Then the worker prints a pass count and reports passed: true

Scenario: A cancelled park is released
  Given a task parked on a ResumeSignal
  When the task is cancelled before the wake
  Then the task ends within one executor re-entry
  And the registry's waiter count is zero

Scenario: Two waiters on one registry entry
  Given two tasks registered on one WakeRegistry entry
  When the host wakes the entry
  Then both tasks resume
  And neither resumes twice

Scenario: A dropped waiter does not break a wake
  Given one task registered and then dropped
  When the host wakes the entry
  Then the wake returns without a panic
  And the waiter count is zero

Scenario: A bounded queue parks its producer
  Given an EventQueue of capacity two and a consumer parked on a host timer
  When the producer pushes five items
  Then the producer parks after the second push
  And the five items arrive in order after the consumer resumes

Scenario: A #[task] transitions on wasm
  Given a #[task] struct built with its input
  When the executor polls it through Ready, Pending and Ready
  Then the transitions are observed in that order
  And the final state is Done

Scenario: The suite proves it can fail
  Given WakeRegistry::wake_all is changed to wake nothing
  When each host runs the suite
  Then "two_waiters_both_resume" fails on that host
  And the message names the host and the case
```

## 6. Technical breakdown

### 6.1 Tasks

| Task | What | Files | Done when |
|---|---|---|---|
| T1 | Run the `mod` target on Deno, Chromium and workerd; triage every failure | `tests/valtron/mod.rs`, `mise.toml`, `mise.wasm.toml`, `scripts/ci/ci.py` | the task `test:wasm-bindgen:core` is green on Deno and Chromium; workerd is green or every red case has an issue |
| T2 | Add the shared scenarios and wire them into both suites | `tests/valtron/wasm_wake_support.rs`, `tests/valtron/wasm_js_yield_integration.rs`, `integration/wasm_tests/src/lib.rs` | each new scenario passes on Chromium (owned) and Deno (bindgen) |
| T3 | Add the owned-only cases for the owned yielder and ABI | `integration/wasm_tests/src/lib.rs` | eight new cases pass on the `deno` and `browser` runners |
| T4 | Prove `#[task]` on a JS-hosted wasm build | `tests/valtron/state_task*.rs`, `tests/valtron/mod.rs` | two cases pass on deno and wrangler |
| T5 | Run the `dual!` async-API cases on Chromium (#575; feature 00 closed the Deno half in T8) | `mise.toml`, `tests/valtron/async_first_api.rs` | 19 cases pass on Chromium and Deno |
| T6 | Record the R14 break for each host | the feature's notes, `LEARNINGS.md`, issue #575 | the red output is in the record, and the break is reverted |

Order: T1 first, because an unrun target hides compile errors. T2 and T3 are one design;
T4 and T5 are independent. T6 last.

### 6.2 T1: one target, three hosts

`foundation_core/tests/mod.rs` includes `mod valtron;` and `tests/valtron/mod.rs` gates modules
by target and feature. Today on wasm the target keeps about forty modules, including
`local_executor`, `wake_registry`, `stream_iterators`, `task_iterators`, `units`,
`flatten_combinators`, `map_circuit`, `clock_driven_tasks`, `cancellable_future`,
`wasm_js_yield_integration`, `wasm_wake_support`, `wrappers`, `drain`, `builder_tests`,
`async_first_api` and `futures_in_valtron`. No CI step runs them (#575).

Add the task:

```toml
[tasks."test:wasm-bindgen:core"]
description = "foundation_core's wasm-bindgen valtron cases, on Deno"
run = """
cargo run -p foundation_testbed --no-default-features --features wasm --profile uat \
  --bin wasm-testbed -- test bindgen-deno backends/foundation_core
"""
depends = ["setup:wasm-bindgen"]
```

T5 adds the Chromium form of the same run, and the wrangler form is T1's second command.
Triage rules for a red case:

1. **A harness gap** (the runner cannot do what the case needs): file an issue with a severity
   and gate the case with `cfg` and the issue number.
2. **A layer defect** (valtron misbehaves on the host): file it `severity: high`, fix it in this
   feature, and keep the case.
3. **A missing tool** (no `Deno`, no `wrangler`): a runner problem, not a case problem. Report
   it to feature 00.

R10 holds: this is one target. `cargo test --tests --target wasm32-unknown-unknown -p
foundation_core --no-default-features --features std,js-wasmbindgen` is the compile check the
CI build step already runs (`ci.py:751-753`); T1 keeps it and adds the run.

### 6.3 T2: the shared scenarios

`wasm_wake_support.rs` is included twice, by `#[path]`:

- the owned crate: `integration/wasm_tests/src/lib.rs:428`;
- the bindgen twin: `tests/valtron/wasm_js_yield_integration.rs` (the case block at 564-612).

Each scenario is `fn name(after: After) -> Scenario`, where `After` schedules a host callback
and `Scenario` is a future with `name()` and `verify()`. Add these:

| Scenario | What it proves | Oracle |
|---|---|---|
| `cancelled_park_releases` | a cancelled task ends and its waker leaves the registry | expected outcome + counts |
| `two_waiters_both_resume` | one wake reaches two registered waiters, each once | model (wake count per waiter) |
| `dropped_waiter_wake_is_safe` | a dropped waiter never breaks a wake; the count returns to zero | expected outcome |
| `bounded_queue_backpressure` | a producer parks on a full queue and resumes in order | model (order + count) |
| `stream_resumes_from_a_host_timer` | `into_future_stream` yields to the loop and resumes | expected outcome + bound |
| `cancelled_child_does_not_wake_parent` | `AnyReadiness` ignores a cancelled child | expected outcome |

Each scenario records the order it observed and `verify()` asserts it. Do not assert on wall
time except as an upper bound; the owned host and the bindgen host have different clocks
(`Date.now` over the ABI versus `foundation_compact::Instant`).

Wire each scenario into both files as two cases: one `#[wasm_test]` (owned) and one
`#[wasm_bindgen_test]` (bindgen), with the same case name so the two hosts are comparable by
name (I2).

### 6.4 T3: the owned-only cases

These prove what only the owned harness can: the `js-foundation-wasm` yielder, the
`schedule_timeout` re-entry and the host ABI. Add them to
`integration/wasm_tests/src/lib.rs` next to the case they test:

| Case | What it proves | Oracle |
|---|---|---|
| `yield_for_reports_waiting_for_a_long_delay` | a 50 ms yield returns `WaitStatus::Waiting`, not `Ready` | expected outcome |
| `two_parked_tasks_resume_in_deadline_order` | deadlines 10 ms and 30 ms resume in order | example (recorded order) |
| `sixty_four_tasks_share_the_loop` | 64 tasks of 4 yields each all end, none starves | bound (all end, ≤ 3 s) |
| `timer_callback_runs_after_an_executor_re_entry` | the callback runs after the executor yields, not inside `yield_for` | expected outcome |
| `host_clock_matches_the_timer_deadline` | the ABI clock and the timer agree within 50 ms | bound |
| `waitgroup_timeout_after_all_done_returns_early` | an idle group's `wait_timeout` returns early | expected outcome |
| `valtron_wasm_test_initializes_the_pool` | `#[valtron_wasm_test]` brings up the pool (it has no user today) | expected outcome |
| `nested_wait_inside_a_wasm_case_returns` | a nested wait returns instead of spinning | expected outcome |

Every case keeps the existing `WaitUntil` bound (`max_polls = 500`) or its own stated bound.
No case may spin: a `Pending` round trip yields, and a case that polls 500 times without
progress panics with its name.

### 6.5 T4: `#[task]` on wasm

`tests/valtron/state_task.rs` is excluded from wasm by the gate at `tests/valtron/mod.rs`:

```
#[cfg(not(all(
    target_family = "wasm",
    any(feature = "js-wasmbindgen", feature = "js-foundation-wasm")
)))]
mod state_task;
```

The reason text says "blocking sync-boundary helpers do not exist on JS-hosted wasm". Check
whether the whole file needs those helpers or only some cases do. Split it:

1. move the cases that use only `TaskStatus`, the `#[task]` struct and the executor into a new
   `tests/valtron/state_task_wasm.rs` with no blocking helper;
2. include it on wasm with `mod state_task_wasm;` and on native too;
3. keep the existing file for the cases that need `sync_one`.

Add two cases there:

- `task_transition_ready_pending_ready_runs_on_the_executor` — the three states in order;
- `task_cancelled_before_its_first_poll_ends_without_running`.

If the macro or the executor cannot do this on wasm, that is a layer defect: file it
`severity: high` and fix it in `foundation_core`. `#[task]` is part of the surface the owner
asked to prove; "it does not work on wasm" is not an acceptable end state for this feature.

### 6.6 T5: the `dual!` cases

`tests/valtron/async_first_api.rs` registers 19 cases twice: `#[valtron_test]` on native and
`#[valtron_bindgen]` on wasm. The Deno task filters the target by
`wasm_js_yield_integration` (`mise.wasm.toml`), so these 19 never run (#575).

T1 creates `test:wasm-bindgen:core` with a filter and `test:wasm-bindgen:web` without one:

```toml
[tasks."test:wasm-bindgen:web"]
description = "foundation_core's wasm-bindgen valtron cases, in Chromium"
run = """
cargo run -p foundation_testbed --no-default-features --features wasm --profile uat \
  --bin wasm-testbed -- test bindgen-web backends/foundation_core
"""
depends = ["setup:wasm-bindgen", "test:browsers:chromium"]
```

This task removes the filter from the Deno task, records the case count of the whole target,
and asserts that the count is at least the 19 of `async_first_api` plus the 15 of
`wasm_js_yield_integration`. Keep the count in the feature notes; a later change that drops a
module then shows as a count drop. If a case
needs `multi` or a native-only helper it must carry a gate with the reason; today it does not,
because the module is written for both targets.

### 6.7 If a case finds a valtron defect

The owner's direction is "enhancing the crates and foundations as needed". A defect that this
feature proves gets fixed here, with a test that fails on the old code:

1. write the case that fails;
2. capture its output;
3. fix the code;
4. show the case green;
5. add the case to the regression set;
6. file the issue with `severity: high` if the fix is not trivial, and close it in this change.

Do not mark a case `#[ignore]`. An ignored case is a false green with extra steps.

### 6.8 Fault table

| Step that fails | What the runner does | What the contributor sees | What is left behind |
|---|---|---|---|
| the target does not compile for wasm | the build step fails before staging | the compiler error | nothing |
| a case panics | the rest run, the suite fails | the case name and the panic | the module instance is dropped |
| a case spins | `WaitUntil` panics past 500 polls | the case name and the poll count | the module instance is dropped |
| the pool does not start | `#[valtron_bindgen]` panics | the case name and "pool" | the module instance is dropped |
| a scenario's `verify` fails | the case fails | the host, the case and the first mismatch (R3) | the scenario is dropped |

## 7. Tests

| Law or scenario | Test | Oracle |
|---|---|---|
| I1, S3 | every shared scenario | expected outcome and its own bound |
| I2 | the same scenario on both harnesses, same name | example (recorded order and counts match) |
| I3, S6 | `dropped_waiter_wake_is_safe` | expected outcome |
| I4, S5 | `bounded_queue_backpressure` | model |
| I5, R15 | the import check from spec 88 section 2.7, re-run on the owned module | example (imports == `abi`) |
| I6 | a check of `tests/valtron/mod.rs` against the running case list | example |
| R14, S8 | one deliberate break per suite | expected outcome (non-zero exit, one named case) |

The R14 record names: the break, the file and line, the case that went red on each host, and
the red output. Put it in the feature notes and in `LEARNINGS.md`.

## 8. Verify

```bash
# T1, T2, T3, T5 — owned hosts
MISE_ENV=wasm mise run test:wasm-testbed:deno
mise run test:wasm-testbed:browser --filter bounded_queue_backpressure

# T1, T4, T5 — bindgen hosts
MISE_ENV=wasm mise run test:wasm-bindgen:core
MISE_ENV=wasm mise run test:wasm-bindgen:web
cargo run -p foundation_testbed --no-default-features --features wasm --profile uat \
  --bin wasm-testbed -- test bindgen-wrangler backends/foundation_core

# the compile check the CI build step runs
cargo check --tests --target wasm32-unknown-unknown -p foundation_core \
  --no-default-features --features std,js-wasmbindgen

# native, so a wasm-only change does not regress the native suite
cargo test --profile uat -p foundation_core --features multi --test mod

# hygiene
cargo fmt --check -p foundation_core
cargo clippy -p foundation_core --all-targets -- -D warnings
```

Run one case by name before every run: `--filter <case>` on the owned runners
(`fwt_runner.rs:73-75`) and on the bindgen runners after feature 00 T7.

## 9. Done when

- [ ] `mise run test:wasm-bindgen:core` is green on Deno, and the Chromium form is green.
- [ ] The `mod` target runs on workerd, or every red case names its issue.
- [ ] Six new shared scenarios pass on the owned and the bindgen host.
- [ ] Eight new owned cases pass on `deno` and `browser`.
- [ ] Two `#[task]` cases pass on a JS-hosted wasm build, or the fixing change is in this
      feature.
- [ ] The 19 `dual!` cases pass on Chromium and Deno, and CI runs them (#575 closed).
- [ ] `#[valtron_wasm_test]` has at least one user.
- [ ] The R14 break is recorded per host and reverted.
- [ ] No wasm-eligible module is skipped without a reason and an issue (I6).
- [ ] The native `mod` target still passes.
- [ ] `progress.md` and `LEARNINGS.md` name what landed and what is next.

---

_Created: 2026-11-15_
