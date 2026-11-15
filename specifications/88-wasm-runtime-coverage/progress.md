# Progress: Spec 88, Wasm Runtime Coverage

_Updated 2026-11-15. Written in ASD-STE100 Simplified Technical English (STE)._

## Where this specification is

| Stage | State |
|---|---|
| Survey of the current wasm test surface | done, measured on 2026-11-15 (requirements section 2) |
| Issues filed from the survey | done (#566-#576) |
| Architecture and rules | done (requirements sections 3-6) |
| Feature designs | done: four features, each with `feature.md` and `start.md` |
| Owner approval of the plan | **approved 2026-11-16**: three answers, given in full in "The approval in detail" below |
| What the approval settles | D1: repair the browser host in this specification. D2: `foundation_http` and `foundation_connectrpc` use the wasm-bindgen harness on all three hosts. D3: all three hosts run on every pull request |
| Implementation | feature 00 in progress (one agent, sonnet); features 01 to 03 wait for the owner's review |
| Pull request | not opened (PR #553 holds spec 85 and spec 88 work) |

No product code changes until the owner approves the plan (owner rule: the plan update and
approval come before implementation).

## The features

| Feature | State | Owner of the work | Blocked by |
|---|---|---|---|
| 00-executors | `in progress` | one agent, sonnet | nothing |
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

The owner answered three questions. The plan follows the answers. The full record is section 4
of `requirements.md`.

| Decision | The owner's answer | What the plan does |
|---|---|---|
| D1: the browser executor | "Fix it here" | Feature 00 repairs the mise task, the preflight, the install path, the macOS Chrome case and the protocol defect. No change from the answer. |
| D2: the test form | "Accept the narrowing" (the first answer was: owned harness where a crate can run without wasm-bindgen; wasm-bindgen where it cannot; workerd is wasm-bindgen only) | `foundation_core` uses the owned harness on Deno and the browser. `foundation_http` and `foundation_connectrpc` use the wasm-bindgen harness on all three hosts, because their wasm modules import wasm-bindgen glue through `foundation_netio`'s `wasm-fetch` feature (requirements 2.7). A port of the netio fetch client to the owned ABI would remove the limit; it costs a second transport implementation and gives no host coverage |
| D3: the CI cadence | "Keep all three on every PR" | All three hosts run on every pull request, inside the Linux `wasm-tests` job. The Chromium install is cached, so the cost is paid once. The macOS job runs the browser suite on full runs. Rule 16 still holds |

The words "narrowing" and "amendment" are the Main Agent's names for the gap between the first
answer and the measured plan. They are not the owner's words.

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

## Next steps

1. Feature 00 starts alone (approved 2026-11-16).
2. When 00 merges, features 01, 02 and 03 start in parallel, one agent each.
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
