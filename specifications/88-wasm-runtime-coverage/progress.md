# Progress: Spec 88, Wasm Runtime Coverage

_Updated 2026-11-16. Written in ASD-STE100 Simplified Technical English (STE)._

## Where this specification is

| Stage | State |
|---|---|
| Survey of the current wasm test surface | done, measured on 2026-11-15 (requirements section 2) |
| Issues filed from the survey | done (#566-#576) |
| Architecture and rules | done (requirements sections 3-6) |
| Feature designs | done: four features, each with `feature.md` and `start.md` |
| Owner approval of the plan | **approved 2026-11-16**: feature 00 is approved with four answers, recorded below |
| The owner's answers | D1: repair the browser host in this specification. D2: both harnesses run where a crate allows it. D3: all three hosts run on every pull request |
| Implementation | feature 00: T1 to T8 **done and reviewed 2026-11-16**; T9 (#586) **reviewed 2026-11-16**; the Linux-run fixes (#587, #588, #590) landed and passed the native `amd64` run 2026-11-16; #599 fixed 2026-11-16; features 01 to 03 next |
| Pull request | not opened (PR #553 holds spec 85 and spec 88 work) |

Product code changes need the owner's approval of the plan. The owner approved feature 00 on
2026-11-16.

## The features

| Feature | State | Owner of the work | Blocked by |
|---|---|---|---|
| 00-executors | `in progress`: T1 to T9 done and reviewed; the Linux-run fixes (#587, #588, #590) landed 2026-11-16 | one agent, sonnet | nothing |
| 01-valtron-runtime | `todo` | one agent, sonnet | 00 merged (T9 included) |
| 02-http-runtime | `todo` | one agent, sonnet | 00 merged |
| 03-connectrpc-runtime | `todo` | one agent, sonnet | 00 merged; 03 writes `server.rs` after 00 |

Features 01, 02 and 03 run in parallel after 00. They write different crates. 00 and 03 both
write `foundation_testbed/src/wasm/server.rs`, first 00 (the `/echo` route), then 03 (the
`/connect` route).

## Feature 00: what landed (2026-11-16)

Branch `spec/88-executors`, base `982b434ee`, pushed. The Main Agent reviewed it against the
"Done when" list of `features/00-executors/feature.md` section 9, and added one review commit
that records the result and closes #578.

| Commit | Task | What it does |
|---|---|---|
| `4c7ab99e3` | T2, part | checkpoint of the browser search |
| `90cf6b327` | T2 | the full browser search order, including `/Applications` |
| `f248cb3d2` | T3 | the browser check runs before the wasm build, and no Node, npm or npx is needed |
| `b48fd4ed7` | T4 | the 50 ms read deadline, the trace, and the verdict from the last `test result:` line |
| `d6a64b207` | T5 | `test bindgen-deno` and `test bindgen-web` fail when a case fails |
| `7785a77fc` | T6 | a local `/echo` endpoint and a base URL on each host |
| `bf3035a77` | T7 | `--filter` on `test bindgen-wrangler`, and two test artifacts fail with both names |
| `532b54488` | T8 | the five executors and the failing fixture in the `wasm-tests` CI job |
| `b143a07ca` | T8 | the testbed docs name the right runner and browser host |
| `835fc2ee4` | T9 | the web, deno and wrangler runners drop libtest's `main` from the glue |

Measured on macOS with Chrome for Testing 157.0.8103.51:

| Check | Result |
|---|---|
| `foundation_browser` tests `resolve_binary`, `smoke`, `connection_lost` | 10, 18 and 2 cases pass |
| `mise run test:wasm-testbed:browser` | 18 cases pass on the owned browser host |
| `MISE_ENV=wasm mise run test:wasm-testbed:bindgen-fails` | both bindgen runners reject `b_fails` |
| `test:wasm-bindgen:core` | 19 `dual!` cases pass on Deno |
| `test:wasm-bindgen:web` | 34 cases pass on Chromium: `foundation_core` (spec 88 T9) |
| workerd self-fetch | 200 from `/echo` inside the worker |

Playwright is gone: no task, dependency or install names it. The browser task downloads the
pinned `chrome-headless-shell` into `~/.cache/foundation/chromium`, and the workflow caches
that directory.

### T9: the Chromium start trap (#586), 2026-11-16

The wasm-bindgen CLI writes `wasm.__wbindgen_start();` into the web glue. In a test binary that
start is libtest's `main`, which ran the plain native-only `#[test]` cases. A case that spawns
threads panicked, and the module aborted before the first `#[wasm_bindgen_test]` case. The Deno
runner already removed the call; the web runner did not. `wasm::patch_bindgen_glue` now removes
it for web, deno and wrangler, and keeps the externref table setup.

| Check | Result |
|---|---|
| `test bindgen-web backends/foundation_core --features js-wasmbindgen,std --headless` | Discovered 34 tests; `test result: ok. 34 passed` |
| `test bindgen-deno backends/foundation_core --features js-wasmbindgen,std` | 34 passed |
| `MISE_ENV=wasm mise run test:wasm-bindgen:core` | 19 passed |
| `MISE_ENV=wasm mise run test:wasm-bindgen:web` | 34 passed |
| `MISE_ENV=wasm mise run test:wasm-testbed:bindgen-fails` | both runners reject `b_fails` |
| `cargo test -p foundation_testbed --test bindgen_glue_tests --profile uat` | 3 cases pass |

`test:wasm-bindgen:web` now runs `backends/foundation_core`, and the `bindgen-fails` step keeps
the failing fixture red. `LEARNINGS.md` entry 15 records the cause.

### The Linux run (2026-11-16)

`scripts/ci/linux.sh foundation_http` passed: fmt, build, clippy with `-D warnings`, the test
set, the wasm check and the new `.mjs` test.

`scripts/ci/linux.sh wasm-tests` failed four commands. The host was `arm64` and the image runs
`linux/amd64` under emulation.

| Command | Result | Issue |
|---|---|---|
| `build`: `cargo fmt --check` on `bindgen_failing_case` | the file was not formatted; fixed in `df0b096ec` | none |
| `test:wasm-testbed:deno` | aborts with `RuntimeError: unreachable` in `wake::any_readiness_parks`; no result line | #587 high |
| `test:wasm-bindgen:deno` | 12 pass, 3 timing cases fail (`wasm_wake_support.rs:756`, `:404`, `:604`) | #589 medium |
| `test:wasm-testbed:browser` | 17 pass, 1 fails; the harness does not name it | #588 high |

`scripts/ci/linux.sh` cannot run from a worktree without an extra read-only mount of the main
`.git` directory; issue #590 records it.

### The Linux-run fixes (2026-11-16)

Commit `25c34d5f8` on `spec/88-executors`. The measurements ran in the same image, still
emulated.

| Issue | Fix | Evidence |
|---|---|---|
| #587 high | The runner reports after each case, keeps its line buffer, and exposes `__FWT_DIAGNOSTICS__`. The embedded driver appends the error, the case in flight and the panic report. `TestReports.snapshot()` keeps a report a trap cut off during delivery. | The run names `any_readiness_resumes_when_a_child_is_ready` and prints `resumed 64ms after the child was ready (window 25ms)` |
| #588 high | `BrowserOutput.output` holds every page line. The runner prints it, and `CaseFailures` and `OwnedRunFailed` carry it. | `tests/web_failure_report_tests.rs` (2 cases); the browser run names `deadline_only_park_resumes_at_its_deadline` and its message (below) |
| #590 low | `scripts/ci/linux.sh` resolves the git common directory and mounts it read-only. | The script ran `foundation_nostd` from the worktree inside the container |

The owned browser suite in the same image runs 18 cases on Chromium: 17 pass, 1 fails, and it
now names the case and the message:

```
FAILED  deadline_only_park_resumes_at_its_deadline
        deadline_only_park_resumes_at_its_deadline: panicked at
          src/../../../tests/valtron/wasm_wake_support.rs:604:13:
        the host timer (10 ms) ran only after the deadline: the executor blocked
          the event loop while it waited
test result: FAILED. 17 passed; 1 failed; 0 ignored
```

The same case is one of the three that #589 records for the wasm-bindgen host. Both hosts fail
it on the emulated image, so the cause is the host, not one executor.

The #587 message shows that the wake works: `polls_at_fire` was 1, so the executor did not poll
the parked task. The 25 ms resume window failed on an emulated host. That is the family of
#589, not a lost wake.

### The native `amd64` measurement (2026-11-16)

Manual run 43753198986: `gh workflow run check.yaml --ref spec/88-executors -f packages=wasm-tests`.
The package job runs in the CI image on a native `amd64` GitHub runner. Every timing case
passes.

| Host | Result |
|---|---|
| owned harness, Deno (`test:wasm-testbed:deno`) | 18 passed, 0 failed |
| owned harness, Chromium (`test:wasm-testbed:browser`) | 18 passed, 0 failed |
| wasm-bindgen, Deno (`test:wasm-bindgen:deno`) | 15 passed, 0 failed |
| wasm-bindgen, Chromium (`test bindgen-web backends/foundation_core`) | 34 passed, 0 failed |
| `test:wasm-testbed:bindgen-fails` | both runners reject `b_fails`, as intended |

The macOS job `macOS 1/1` passes its two browser runs. The `macOS build` job failed for a
different reason: the runner had no `protoc`, and the build script of `foundation_agent_rpc`
needs it. Issue #599 (`high`) records it. The same change fixes it: the workflow installs the
pinned `protoc` 36.2 in the `macos-test`, `windows-test` and `macOS build` jobs.

The step downloads the release asset for the runner's operating system (OS) and processor,
checks its SHA-256 against the value in `ci/platform-images/mise/mise.lock`, unpacks it and
adds `bin` to the `PATH`. The step ran on this macOS `arm64` host: it printed
`libprotoc 36.2`, and `actionlint` accepts the workflow.

Issues #587, #588, #589 and #590 closed with these measurements.

New issues from the same work: #597 (`low`, a timing bound in `foundation_nostd`), #598
(`low`, 16 clippy warnings in `foundation_wasm` under a wasm feature tree that CI does not
lint) and #599 (`high`, the macOS build job needs `protoc`).

### Issues that feature 00 closed

#566, #567, #568, #569, #571, #572, #573, #574, #579, #580, #581, #582. The review closed
#578 with a one-line MIME fix and its test.

### Issues that feature 00 opened

| Issue | Severity | State | Home |
|---|---|---|---|
| #577 | low | open | out of scope (`requirements.md` section 11) |
| #578 | high | closed by the review | feature 02 records it in section 6.9 |
| #583 | low | open | the wasm-bindgen 0.3.76 context quirk; it waits |
| #584 | low | open | workerd's request URL has no port; the runner avoids it |
| #585 | low | open | `wrangler::run` ignores `config_path`; it waits |
| #586 | high | open | feature 00 T9 (lifted by the owner on 2026-11-16), fixed on `spec/88-executors` |
| #587 | high | closed | the owned Deno abort; fixed in `25c34d5f8`, native run 43753198986 passes |
| #588 | high | closed | the browser harness names a failed case; fixed in `25c34d5f8`, native run passes |
| #589 | medium | closed | native `amd64` run 43753198986 passes the three timing cases |
| #590 | low | closed | the CI script works in a worktree; fixed in `25c34d5f8` |
| #597 | low | open | a `foundation_nostd` timing bound; it waits |
| #598 | low | open | 16 clippy warnings in a wasm feature tree that CI does not lint; it waits |
| #599 | high | closed | the macOS and Windows jobs install the pinned `protoc` 36.2 (this change) |

## What the plan depends on

| Assumption | State | Where it is measured |
|---|---|---|
| The owned harness cannot start a module that imports wasm-bindgen glue | measured | requirements 2.7 (7 imports, all `__wbindgen_*`) |
| The `abi` runtime compiles a new JavaScript function at runtime and binds it to a handle | measured | `foundation-wasm.js:1202-1218`; the owned wasm test crate registers a clock (`foundation_core/integration/wasm_tests/src/lib.rs:28-34`) |
| `test bindgen-deno` and `test bindgen-web` discard the verdict | measured | requirements 2.6 (FAILED summary, exit code 0) |
| The 19 `dual!` cases pass on Deno but no step runs them | measured | requirements 2.3, issue #575 |
| A macOS Chrome exists at `/Applications` and the driver never looks there | measured | requirements 2.2 |
| The Linux CI image ships Chrome | read from the image build | requirements 2.2 |
| `foundation_connectrpc` builds for wasm32 with the `rpc` feature set | measured | feature 03 section 1 |
| `foundation_http`'s library builds for wasm32 with `wasm-bindgen-http` | measured | feature 02 section 6.7 |
| workerd can fetch the host loopback | measured: it can; the runner sends its origin in an `x-fwt-base-url` header | feature 00 T6, `LEARNINGS.md` entry 12 |

The last row was the open question. Feature 00 T6 measured it: a worker fetched its own
dev-server origin and received 200. Feature 03 still repeats the measurement for its own
transport case.

## The approval in detail

The owner answered three questions on 2026-11-15. The full record is section 4 of
`requirements.md`. The owner then reviewed the plan again, and approved feature 00 on
2026-11-16 with the four answers in "The approval of feature 00". The table keeps the three
earlier answers as a record.

| Decision | The owner's answer | What the plan does |
|---|---|---|
| D1: the browser executor | "Fix it here" | Feature 00 repairs the mise task, the preflight, the install path, the macOS Chrome case and the protocol defect. No change from the answer. |
| D2: the test form | "Sounds reasonable" (the first answer was: owned harness where a crate can run without wasm-bindgen; wasm-bindgen where it cannot; workerd is wasm-bindgen only) | Both harnesses run where a crate allows it. Features 02 and 03 add a crate feature that gates `foundation_netio`'s `wasm-fetch`, build a second test package without it, measure its imports, and run the pure-Rust cases on the owned harness on Deno and Chromium (requirements 2.7 and 4). A transport case stays on the wasm-bindgen harness on all three hosts |
| D3: the CI cadence | "Keep all three on every PR" | All three hosts run on every pull request, inside the Linux `wasm-tests` job. The Chromium install is cached, so the cost is paid once. The macOS job runs the browser suite on full runs. Rule 16 still holds |

The words "narrowing" and "amendment" are the Main Agent's names for the gap between the first
answer and the measured plan. They are not the owner's words.

## The approval of feature 00 (2026-11-16)

| Question | The owner's answer | What the plan does |
|---|---|---|
| Q1: approve feature 00 as written | "01 - approved" | One sonnet agent does T1 to T8 in its own git worktree. It commits there; the Main Agent reviews and merges its branch |
| Q2: no Chrome on the machine | "download, follow recommendation" | T1 downloads a pinned `chrome-headless-shell` into `~/.cache/foundation/chromium`. It never asks for `sudo` |
| Q3: the limit of T4 | "we trace as much as possible and identify the issue and present it if not an easy fix" | T4 adds the trace, finds the root cause and writes it in #569 and `LEARNINGS.md`. A fix that is not small stops the task with a report, not a rewrite |
| Q4: the macOS CI step | "of course it runs there" | The browser suite runs in the macOS `macos-test` job. That job runs on full runs only: a push to `master` or `platform/release`, a pull request into or from `platform/release`, the nightly schedule and a manual run (`.github/workflows/check.yaml:38-49`) |

A local check on a Mac runs the task directly. It needs no Docker container. The Linux
container checks Linux compatibility only.

## Decisions after the first review

| Date | Decision | What the plan does |
|---|---|---|
| 2026-11-16 | Remove Playwright. `foundation_browser` supplies the Chrome DevTools Protocol (CDP) client. | Feature 00 T1 deletes the `setup:playwright` task (`mise.toml:145-153`) and `backends/foundation_testbed/integration/package.json`. No task, dependency or install keeps Playwright. |
| 2026-11-16 | Test with both harnesses where a crate allows it. A case that touches the transport stays on the wasm-bindgen harness. A case that needs no host stays on the owned harness where the module allows it. | Settled. The approach has five steps: add a crate feature that turns on `foundation_netio/wasm-fetch` and keep it on for the default wasm build; build a second test package with the feature off; measure its imports; run the pure-Rust cases under the owned harness on Deno and Chromium; keep the transport cases under the wasm-bindgen harness on all three hosts. One more test module and one more wasm build per crate is the cost. It pays, because the result shows which cases work on both harnesses. This is not #577: #577 ports the fetch client to the `abi` contract; this change only builds a module without the client. |

## Issues this specification closes

| Issue | Severity | Closed by |
|---|---|---|
| #566 | high | 00 T1 |
| #567 | medium | 00 T3 |
| #568 | medium | 00 T1 and T2 |
| #569 | high | 00 T4 |
| #570 | medium | 02 T5 |
| #571 | low | 00 T7 |
| #572 | high | 00 T5 |
| #573 | low | 00 T4 |
| #574 | low | 00 T7 |
| #575 | medium | closed by 00 T8: the 19 `dual!` cases run on Deno in CI |
| #576 | low | 00 T8 records the check form; the issue stays open until `foundation_testing` builds for wasm32 |
| #578 | high | 00 review: `StaticFileHandler` answers `.mjs` with `text/javascript` |
| #579 | low | 00 T4 |
| #580 | low | 00 T4 |
| #581 | low | 00 T1 |
| #582 | low | 00 T8 |
| #586 | high | 00 T9: the web runner drops libtest's `main` from the wasm-bindgen glue |
| #587 | high | 00 Linux-run fix: the harness names the trapped case and keeps its output |
| #588 | high | 00 Linux-run fix: the browser harness prints every page line |
| #589 | medium | the native `amd64` measurement (run 43753198986); no bound changed |
| #590 | low | 00 Linux-run fix: `scripts/ci/linux.sh` runs from a worktree |

#577 is recorded and not closed. This specification does not port `foundation_netio`'s wasm
fetch client to the owned ABI: it is a product change that adds no host coverage
(`requirements.md` section 11 and `LEARNINGS.md` entry 1).

## Next steps

1. The next full run proves the #599 fix: the `macOS build` job must pass. The Main Agent then
   merges `spec/88-executors` into `spec/split-65`, which keeps one pull request for the
   specification (PR #553).
2. Features 01, 02 and 03 then start in parallel, one agent each. Features 02 and 03 carry the
   second test package from "Decisions after the first review".
3. Each feature reports to the Main Agent; the Main Agent reviews against "Done when" and
   commits on the branch of that feature.
4. The Linux job of PR #553 is the first full run of the repaired executors. A macOS and a
   Windows run need a push to `master` or `platform/release`, a pull request into or from
   `platform/release`, the nightly schedule or a manual run.

## Records that must exist before a feature is called done

- the case count per host;
- the R14 break: the change, the case, the red output per host;
- the new issues with a `severity:` label;
- the updates to this file and to `LEARNINGS.md`.

---

_Created: 2026-11-15_
