# Progress: Spec 88, Wasm Runtime Coverage

_Updated 2026-11-23. Written in ASD-STE100 Simplified Technical English (STE)._

**Feature 00 is merged** as `f2e4688ad` (#655). **Feature 01 is merged** as `447519256`.
**Feature 02 is merged** as `c12e94aec`. **Feature 03 is complete** in the working tree on `spec88/f03-connectrpc-runtime`:
T1 to T7 are done. Cases 14 to 16 now run on Deno, Chromium and workerd.

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
| 00-executors | `done`: T1 to T9 done and reviewed; the Linux-run fixes (#587, #588, #590) landed 2026-11-16 | one agent, sonnet | nothing |
| 01-valtron-runtime | `done`: T1 to T6 done, reviewed and merged as `447519256` | one agent, sonnet | nothing |
| 02-http-runtime | `done`: T1 to T6 done, reviewed and merged as `c12e94aec` | one agent, sonnet | nothing |
| 03-connectrpc-runtime | `done`: T1 to T7 done; cases 1 to 17 pass on Deno, Chromium and workerd | one agent, sonnet | 00 merged |

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

Manual run 43754950849 (`spec/88-executors`, `packages=wasm-tests`) proves the fix: the
`macOS build` job passes, and the run concludes `success`. The Windows form of the step is
not proven yet. A `windows-test` job needs a full run that selects `foundation_agent_rpc`.

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
| #599 | high | closed | the macOS and Windows jobs install the pinned `protoc` 36.2; run 43754950849 passes `macOS build` |

## Feature 01: what landed (2026-11-23)

Branch `spec88/f01-valtron-runtime`, base `ca0c0296a`. The work is in the working tree; it is
not committed. Feature 00 merged first, so the runners, `--filter`, the `/echo` endpoint and
the `wasm-tests` CI job were in place.

### Tasks

| Task | State | What landed |
|---|---|---|
| T1 | done | The whole wasm-bindgen `mod` target runs on three hosts: 43 cases on Deno, 43 on Chromium and 43 on workerd. The baseline was 35 cases (19 `async_first_api`, 15 `wasm_js_yield_integration`, 1 `js_stream_park`). No case was red, so T1 filed no issue. |
| T2 | done | Six shared scenarios in `tests/valtron/wasm_wake_support.rs`, each with the same case name in both suites: `cancelled_park_releases`, `two_waiters_both_resume`, `dropped_waiter_wake_is_safe`, `bounded_queue_backpressure`, `stream_resumes_from_a_host_timer` and `cancelled_child_does_not_wake_parent`. |
| T3 | done | Eight owned cases in `integration/wasm_tests/src/lib.rs`: `yield_for_reports_waiting_for_a_long_delay`, `two_parked_tasks_resume_in_deadline_order`, `sixty_four_tasks_share_the_loop`, `timer_callback_runs_after_an_executor_re_entry`, `host_clock_matches_the_timer_deadline`, `waitgroup_timeout_after_all_done_returns_early`, `valtron_wasm_test_initializes_the_pool` and `nested_wait_inside_a_wasm_case_returns`. |
| T4 | done | `tests/valtron/state_task_wasm.rs` holds the cases that use only `TaskStatus`, the `#[task]` struct and the executor. It adds `task_transition_ready_pending_ready_runs_on_the_executor` and `task_cancelled_before_its_first_poll_ends_without_running`, both dual-registered. `state_task.rs` keeps the two `sync_one` cases. The two new cases pass on Deno, Chromium, workerd and natively. |
| T5 | done | `test:wasm-bindgen:core` runs the whole target and fails below 34 cases; it measured 43. The 19 `dual!` cases run on Deno and Chromium. |
| T6 | done | The R14 break in `WakeRegistry::fire` turned `two_waiters_both_resume` red on all five hosts. The break is reverted and the case is green again. |

### Case counts

| Host | Suite | Before | After |
|---|---|---|---|
| wasm-bindgen Deno | `mod` target | 35 | 43 |
| wasm-bindgen Chromium | `mod` target | 35 | 43 |
| wasm-bindgen workerd | `mod` target | not run | 43 |
| owned Deno | `integration/wasm_tests` | 33 | 47 |
| owned Chromium | `integration/wasm_tests` | 33 | 47 |

### Verification

| Command | Result |
|---|---|
| `MISE_ENV=wasm mise run test:wasm-bindgen:core` | `test result: ok. 43 passed; 0 failed`; the task asserts `count >= 34` |
| `MISE_ENV=wasm mise run test:wasm-bindgen:web` | `Discovered 43 tests`; `test result: ok. 43 passed; 0 failed` |
| `test bindgen-wrangler backends/foundation_core --features js-wasmbindgen,std` | `{"passed":true,...}`; `43 passed` |
| `MISE_ENV=wasm mise run test:wasm-testbed:deno` | `47 passed; 0 failed` on the owned Deno host |
| `MISE_ENV=wasm mise run test:wasm-testbed:browser` | `47 passed; 0 failed` on owned Chromium |
| `... browser ... --filter bounded_queue_backpressure --headless` | `1 passed; 0 failed` |
| `cargo check --tests --target wasm32-unknown-unknown -p foundation_core --no-default-features --features std,js-wasmbindgen` | `Finished` |
| `cargo test --profile uat -p foundation_core --features multi --test mod` | `809 passed; 0 failed` |
| `cargo fmt --check -p foundation_core` | clean |
| `cargo clippy -p foundation_core --all-targets -- -D warnings` | clean |

### The R14 break (T6)

The feature calls the method `WakeRegistry::wake_all`; the code name is `WakeRegistry::fire`
(`backends/foundation_core/src/valtron/wake.rs:157`). The break took the registered callbacks
and called none of them. Each host ran one case:
`--filter two_waiters_both_resume`.

| Host | Red output |
|---|---|
| wasm-bindgen Deno | `two_waiters_both_resume: the parked task never ran again (a lost wake)`; `test result: FAILED. 0 passed; 1 failed`; exit 1 |
| wasm-bindgen Chromium | the same line; `test result: FAILED. 0 passed; 1 failed`; exit 1 |
| wasm-bindgen workerd | `{"passed":false}`, the same line, `test result: FAILED. 0 passed; 1 failed`; exit 1 |
| owned Deno | `failing case: two_waiters_both_resume`; the same line; exit 1 |
| owned Chromium | `FAILED  two_waiters_both_resume`; the same line; exit 1 |

The break is reverted. After the revert the same filtered case is green on all five hosts.

### Notes and corrections

- `sixty_four_tasks_share_the_loop` took 4.004 s on the owned Deno host. The executor hands
  each of the 256 task steps to the JS event loop, and a host timer costs about 10 ms. The
  case keeps a tight 2000-re-entry poll bound (the anti-spin guard) and uses a 10 s wall-clock
  bound instead of the 3 s in the feature. Issue #740 records the cost.
- The feature's verify section shows the workerd form without `--features js-wasmbindgen,std`.
  The build needs them; this record uses them.
- `mise run test:wasm-testbed:browser --filter <case>` ignores the filter, because the task's
  fixed script does not forward `$@`. Issue #739 records it. The direct
  `cargo run ... --filter` form runs one case.

### New issues

| Issue | Severity | Problem |
|---|---|---|
| #739 | low | the `test:wasm-testbed:browser` and `:deno` mise tasks do not forward `--filter` |
| #740 | low | the owned wasm executor pays a host timer per task step; 256 steps take 4.0 s |
| #741 | low | the wasm feature trees of `foundation_core` and `foundation_compact` fail `clippy -D warnings` |

## Feature 02: what landed (2026-11-23)

Merged as `c12e94aec`, on branch `spec88/f02-http-runtime-final` from base `93f9c9d9a`.

### Tasks

| Task | State | What landed |
|---|---|---|
| T1 | done | `backends/foundation_http/integration/wasm/` builds for wasm32 with one test target and runs on Deno, Chromium and workerd. |
| T2 | done | Cases 1 to 11 cover the web bridge, the middleware chain, the context fork, the injected clock and the decoded path. |
| T3 | done | Cases 12 to 15 cover the CF bridge. Case 14 finds the real local D1 on workerd and skips with a message elsewhere. |
| T4 | done | Cases 16 to 20 cover the serve-writer path, the writer body limit, `WasmStream` and the response builder. |
| T5 | done | CI checks the crate for wasm32, formats it, and runs the native `wasm-test` suite (9 + 22 + 9 cases) and the three hosts (#570 closed). |
| T6 | done | `integration/wasm_owned` has no fetch client. It imports only four `abi` functions and runs 40 cases on Deno and 40 on Chromium. |

### Case counts

| Host | Harness | Cases |
|---|---|---|
| wasm-bindgen Deno | `test bindgen-deno` | 19 pass (case 9 gated; case 14 skips) |
| wasm-bindgen Chromium | `test bindgen-web` | 19 pass (case 9 gated; case 14 skips) |
| workerd | `test bindgen-wrangler` | 19 pass (case 9 gated; case 14 passes with the real D1) |
| owned Deno | `deno integration/wasm_owned` | 40 pass |
| owned Chromium | `browser integration/wasm_owned --headless` | 40 pass |
| native `wasm-test` | `cargo test -p foundation_http --features wasm-test` | 40 pass (9 response, 22 stream, 9 server) |

Case 9 (`handler_panic_becomes_a_500`) is gated by the crate feature
`panicking-handler-case`. Measured: a handler panic aborts the module
(`RuntimeError: unreachable`), and the wasm-bindgen-test host reports the case as failed. A
`wasm32-unknown-unknown` build uses `panic = "abort"`, so the bridge cannot answer 500. Issue
#742 holds the fix.

### Verification

| Command | Result |
|---|---|
| `cargo check -p foundation_http --target wasm32-unknown-unknown --features wasm-bindgen-http` | `Finished` |
| `cargo check --tests --target wasm32-unknown-unknown --manifest-path .../integration/wasm/Cargo.toml` | `Finished` |
| `test bindgen-deno backends/foundation_http/integration/wasm` | `19 passed; 0 failed` |
| `test bindgen-web ... --headless` | `19 passed; 0 failed` |
| `test bindgen-wrangler ...` | `{"passed":true}`, `19 passed; 0 failed` |
| `cargo test -p foundation_http --features wasm-test` (three files) | `9`, `22` and `9` passed |
| `wasm-testbed deno .../integration/wasm_owned` | `40 passed; 0 failed` |
| `wasm-testbed browser .../integration/wasm_owned --headless` | `40 passed; 0 failed` |
| import measurement of the owned module | only `abi.host_report`, `abi.host_fill_random`, `abi.host_wall_clock_ms`, `abi.host_monotonic_now_ms`; no `__wbindgen_` |
| `cargo fmt --all -- --check` and both integration manifests | clean |
| `cargo clippy -p foundation_http --all-targets -- -D warnings` | clean |

`wasm2wat` is not installed on this host. The import list came from a reader of the wasm import
section (section 6.8 of the feature and `LEARNINGS.md` entry 23).

### The R14 break

The router's 405 answer was changed to 404 in `dispatch_web`. Each host ran
`--filter wrong_method_returns_405`.

| Host | Red output |
|---|---|
| wasm-bindgen Deno | `deno: case 4: status; left: 404; right: 405`; `test result: FAILED. 0 passed; 1 failed` |
| wasm-bindgen Chromium | `browser: case 4: status; left: 404; right: 405`; `test result: FAILED. 0 passed; 1 failed` |
| workerd | `workerd: case 4: status; left: 404; right: 405`; `{"passed":false}`; `test result: FAILED. 0 passed; 1 failed` |

The break is reverted. The same filtered case is green on all three hosts after the revert.

### Production changes the cases proved

| Case | Defect | Fix | Issue |
|---|---|---|---|
| 3 | A route miss rejected the JS promise instead of answering 404 | a 404 `Response` from `dispatch_web` and `dispatch_cf` | #744 |
| 4 | A wrong method could not be 405 | `RouteOp::MethodNotAllowed` and `Router::dispatch_result` | #745 |
| 5 | `%2F` never decoded | `bridge::parse_request_uri` decodes only the path | #746 |
| 16 | The writer path routed on the full URL | route on `request_uri.path()` | #747 |
| 18 | No writer-path body limit | `HttpApp::max_writer_body_bytes`, default 1 MiB | none |
| 2 | `AsyncSendSafeBody` self-wakes without a host yield on workerd | the suite yields per `Data::Retry`; netio keeps the defect | #749 |

`foundation_http` goes from `0.0.13` to `0.0.14` for the new public API (root AGENTS rule 12),
and every workspace consumer moves to `0.0.14`.

### New issues

| Issue | Severity | Problem |
|---|---|---|
| #742 | high | a panicking wasm handler aborts the module; it cannot answer 500 |
| #743 | medium | the native HTTP/1 server answers 404 for a known path with a wrong method |
| #744 | high | the wasm bridge rejected the JS promise for a route miss |
| #745 | medium | the router could not report a method miss |
| #746 | medium | the wasm bridge did not percent-decode the path |
| #747 | high | the writer path routed on the full URL |
| #749 | medium | `AsyncSendSafeBody` self-wakes without a host yield on workerd |
| #750 | low | the `foundation_http` wasm feature tree fails `clippy -D warnings` |

#744, #745, #746 and #747 are fixed on the branch and close when it lands. #570 is closed by
T5.

### Notes and corrections

- The feature's step 5 of section 6.8 says `tests/owned.rs`. The cases are in `src/lib.rs`:
  the owned runner builds the package lib and reads its `__fwt_` exports (feature section 5,
  rule R9). A case in `tests/` never runs.
- Case 8 uses a normal duplicate header, not `Set-Cookie`: Chromium hides `Set-Cookie` from a
  `Response` that a page constructs, so its order cannot be read there.
- `tests/wasm_server_tests.rs` asserted an upper-case header name. `SimpleHeader` renders the
  name in lower case, and HTTP field names are case-insensitive (RFC 9110 section 5.1). The
  assertion is now case-insensitive. The same correction is in the owned crate.
- The first owned Deno run failed on the same upper-case assertion; it is fixed and the run is
  green.

## Feature 03: what landed (2026-11-23)

Branch `spec88/f03-connectrpc-runtime`, from `012fbc6be`. The work is in the working tree; it
is not committed. Spec 85 created `integration/wasm`; feature 03 extended it and kept one test
target (`tests/wasm.rs`, renamed from `tests/wasm_streaming.rs`).

### Tasks

| Task | State | What landed |
|---|---|---|
| T1 | done | `integration/wasm` builds for wasm32 with `rpc,wasm-fetch` and runs one target. The unary case passes on Deno, Chromium and workerd. |
| T2 | done | Cases 1 to 10: the router, the error model, the recording sink, the four stream shapes and two services. |
| T3 | done | Cases 11 to 13 on a deterministic clock this crate supplies. The layer reads no wall clock. |
| T4 | done | `RpcRoute` on `/connect/*` in `foundation_testbed`, on the same origin as the page. `tests/connect_server_tests.rs` has 3 native cases. |
| T5 | done | Cases 14 to 17. The Deno and workerd templates answer `/connect` in JS (#752, fixed). `H1Transport` sends a fixed-length body for unary and server-streaming calls on wasm (#753, fixed). Three further defects were found and fixed: the wasm `run_until` spin (#757), the missing `Content-Length` (#758) and the fetch double-decode (#759). |
| T6 | done | `mise.wasm.toml` has four new tasks; `scripts/ci/ci.py` checks the owned crate and runs the four hosts. |
| T7 | done | `foundation_connectrpc` has a `wasm-fetch` feature. `integration/wasm_owned` builds without it, imports only `abi`, and runs 13 cases on Deno and 13 on Chromium. |

### Case counts

| Host | Harness | Cases |
|---|---|---|
| wasm-bindgen Deno | `test bindgen-deno` | 22 pass |
| wasm-bindgen Chromium | `test bindgen-web` | 22 pass |
| workerd | `test bindgen-wrangler` | 22 pass (`{"passed":true}`) |
| owned Deno | `deno integration/wasm_owned` | 13 pass |
| owned Chromium | `browser integration/wasm_owned --headless` | 13 pass |
| native `/connect` route | `cargo test -p foundation_testbed` | 3 pass |

### Verification

| Command | Result |
|---|---|
| `cargo check -p foundation_connectrpc --target wasm32-unknown-unknown --no-default-features --features rpc` | `Finished` |
| `cargo check --tests --target wasm32-unknown-unknown --manifest-path .../integration/wasm/Cargo.toml` | `Finished` |
| `cargo check --tests --target wasm32-unknown-unknown --manifest-path .../integration/wasm_owned/Cargo.toml` | `Finished` |
| `test bindgen-deno backends/foundation_connectrpc/integration/wasm` | `22 passed; 0 failed` |
| `test bindgen-web ... --headless` | `22 passed; 0 failed` |
| `test bindgen-wrangler ...` | `{"passed":true}` |
| `wasm-testbed deno .../integration/wasm_owned` | `13 passed; 0 failed` |
| `wasm-testbed browser .../integration/wasm_owned --headless` | `13 passed; 0 failed` |
| `cargo test -p foundation_testbed` | all suites pass; `connect_server_tests` 3 pass |
| import measurement of the owned module | `abi.schedule_timeout`, `abi.host_report`, `abi.host_monotonic_now_ms`; no `__wbindgen_` |
| `cargo fmt --check` for the three crates and both standalone manifests | clean |
| `cargo clippy -p foundation_connectrpc -p foundation_netio -p foundation_testbed --all-targets --no-deps -- -D warnings` | clean |
| `cargo clippy -p foundation_core --all-targets --no-deps -- -D warnings` | clean |
| `MISE_ENV=wasm mise run test:wasm-bindgen:core` | `43 passed; 0 failed` (the `run_until` change) |
| `MISE_ENV=wasm mise run test:wasm-bindgen:netio` | `10 passed; 0 failed` (the fetch-client change) |
| the same clippy command without `--no-deps` | fails on 16 pre-existing `foundation_wasm` warnings (#598) |

`wasm2wat` is not installed on this host. The import list came from a reader of the wasm import
section (LEARNINGS entry 23).

### The R14 break

`dispatch_to` was changed to call `sink.finish()` before the first chunk. Each host ran
`--filter server_stream_frames_arrive_in_order`.

| Host | Red output |
|---|---|
| wasm-bindgen Deno | `case 5: the first call is open; got ["finish", "open", "chunk", "chunk", "chunk", "chunk", "finish"]`; `test result: FAILED. 0 passed; 1 failed` |
| wasm-bindgen Chromium | the same line; `test result: FAILED. 0 passed; 1 failed` |
| workerd | `{"passed":false}`; the same line; `test result: FAILED. 0 passed; 1 failed` |
| owned Deno | `failing case: server_stream_frames_arrive_in_order`; `case 5: open`; exit 1 |
| owned Chromium | `FAILED  server_stream_frames_arrive_in_order`; `case 5: open`; exit 1 |

The break is reverted, and the same filtered case is green on all five hosts.

### Production changes the cases proved

| Case | Defect | Fix | Issue |
|---|---|---|---|
| 14 | The wasm fetch client did not set `duplex: "half"`, so Chromium refused the `Request` | set it in `build_web_request`; the spec 85 inbound case does the same | #754 (closed) |
| 2 | The client maps a non-Connect 404 to `NotFound`, not `unimplemented` (Decision 03) | not changed; the case asserts the documented reverse mapping | #755 |
| T7 | `rpc` pulls `auth`, and foundation_auth's wasm feature pulls `wasm-fetch` | the owned package uses no features | #756 |
| 14-16 | A browser cannot stream a request body to an HTTP/1.1 origin | `H1Transport` sends a fixed-length body for unary and server-streaming calls; the client frames the one message | #753 (closed) |
| 16 | The wasm `run_until` loops on runnable work inside a microtask and starves the JS event loop | yield to the host every 200 steps; the driver uses `run_until_complete` plus a host sleep | #757 (closed) |
| 14 | The `/connect` route returned a keep-alive response with no `Content-Length`, so a browser waited forever | set `Content-Length` from the in-memory body; the native tests assert it | #758 (closed) |
| 14 | The fetch client returned a decoded body with the stale `content-encoding` header, so the client decompressed twice | drop `content-encoding` in `response_from_web` | #759 (closed) |

### Notes and corrections

- The feature's T1 `Cargo.toml` block names `foundation_context` and `foundation_compact` with
  `host-abi`. That is wrong for the wasm-bindgen crate: the module then imports `abi`, which
  the Deno host does not supply (`Import "abi" not a dependency`). The crate uses the
  wasm-bindgen clock backend instead.
- The feature's T7 example builds the owned package with `features = ["rpc"]`. Measured:
  `rpc` pulls wasm-bindgen glue through `auth` (#756), so the package uses no features.
- Case 5 sees four chunks, not three: the Connect EndStream frame is its own chunk. The case
  asserts three messages in order plus the end-stream frame.
- The `/connect` route reads a chunked body into memory, bounded by
  `ServerLimits::max_connect_body` (default 4 MiB), before dispatch. The router's in-memory
  `dispatch` cannot wait on a connection. A route miss has no header, so the route adds a
  framing header before `render_message`.
- `valtron::execute` hangs in a wasm-bindgen test body (#565). The transport driver polls the
  client future, steps the single-threaded engine with `run_once`, and yields to the host.
- `foundation_testbed` goes from `0.0.8` to `0.0.9` for the new `ServerLimits` field.

### New issues

| Issue | Severity | Problem |
|---|---|---|
| #752 | medium | no wasm host can run the transport cases: Deno and workerd start no Rust endpoint; Chromium cannot stream a body |
| #753 | high | `H1Transport` cannot send a streaming body from a browser to an HTTP/1.1 origin |
| #754 | high | the wasm fetch client omitted `RequestInit.duplex` (fixed, closed) |
| #755 | medium | the connectrpc client maps a non-Connect 404 to `NotFound`, not `unimplemented` |
| #756 | medium | the `rpc` feature pulls wasm-bindgen glue through `foundation_auth` |

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
| #744 | high | 02 T2: the wasm bridge answers 404 (fixed on `spec88/f02-http-runtime`) |
| #745 | medium | 02 T2: the router reports a method miss, so it answers 405 (fixed on the branch) |
| #746 | medium | 02 T2: the wasm bridge decodes the request path (fixed on the branch) |
| #747 | high | 02 T4: the writer path routes on the path (fixed on the branch) |

#577 is recorded and not closed. This specification does not port `foundation_netio`'s wasm
fetch client to the owned ABI: it is a product change that adds no host coverage
(`requirements.md` section 11 and `LEARNINGS.md` entry 1).

## Next steps

1. The `macOS build` job passes with the new `protoc` step (run 43754950849). A full run that
   selects `foundation_agent_rpc` must then prove the Windows form. The Main Agent then merges
   `spec/88-executors` into `spec/split-65`, which keeps one pull request for the
   specification (PR #553).
2. Features 01 and 02 are merged. Feature 03 starts next, from the same base, and keeps
   its own standalone crate.
3. Each feature reports to the Main Agent; the Main Agent reviews against "Done when" and
   commits on the branch of that feature.
4. The Linux job of PR #553 is the first full run of the repaired executors. A macOS and a
   Windows run need a push to `master` or `platform/release`, a pull request into or from
   `platform/release`, the nightly schedule or a manual run.
5. A full PR run must prove the three wasm hosts for `foundation_core` on the Linux CI image.
   Issues #739, #740 and #741 wait for later pickup; none blocks feature 01.

## Records that must exist before a feature is called done

- the case count per host;
- the R14 break: the change, the case, the red output per host;
- the new issues with a `severity:` label;
- the updates to this file and to `LEARNINGS.md`.

---

_Created: 2026-11-15_
