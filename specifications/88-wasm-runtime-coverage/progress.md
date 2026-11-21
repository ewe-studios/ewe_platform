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
| Implementation | feature 00 in progress (one agent, sonnet); features 01 to 03 wait for 00 |
| Pull request | not opened (PR #553 holds spec 85 and spec 88 work) |

Product code changes need the owner's approval of the plan. The owner approved feature 00 on
2026-11-16.

## The features

| Feature | State | Owner of the work | Blocked by |
|---|---|---|---|
| 00-executors | `in progress` | one agent, sonnet | nothing: approved 2026-11-16 |
| 01-valtron-runtime | `todo` | one agent, sonnet | 00 merged |
| 02-http-runtime | `todo` | one agent, sonnet | 00 merged |
| 03-connectrpc-runtime | `todo` | one agent, sonnet | 00 merged; 03 writes `server.rs` after 00 |

Features 01, 02 and 03 run in parallel after 00. They write different crates. 00 and 03 both
write `foundation_testbed/src/wasm/server.rs`, first 00 (the `/echo` route), then 03 (the
`/connect` route).

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
| workerd can fetch the host loopback | **not measured** | feature 03 section 6.5 measures it before the transport cases |

The last row is the one open question. Feature 03 measures it and records the answer; it does
not build a stub to avoid the question.

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
| #575 | medium | 01 T1 and T5 |
| #576 | low | 00 T8 records the check form; the issue stays open until `foundation_testing` builds for wasm32 |

#577 is recorded and not closed. This specification does not port `foundation_netio`'s wasm
fetch client to the owned ABI: it is a product change that adds no host coverage
(`requirements.md` section 11 and `LEARNINGS.md` entry 1).

## Next steps

1. Feature 00 runs now, one sonnet agent in its own git worktree, T1 to T8. It commits there;
   the Main Agent reviews and merges.
2. When 00 merges, features 01, 02 and 03 start in parallel, one agent each. Features 02 and
   03 carry the second test package from "Decisions after the first review".
3. Each feature reports to the Main Agent; the Main Agent reviews against "Done when" and
   commits.
4. One pull request for the specification (owner rule: one branch and one pull request per
   spec).

## Records that must exist before a feature is called done

- the case count per host;
- the R14 break: the change, the case, the red output per host;
- the new issues with a `severity:` label;
- the updates to this file and to `LEARNINGS.md`.

---

_Created: 2026-11-15_
