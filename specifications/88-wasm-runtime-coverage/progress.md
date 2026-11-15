# Progress: Spec 88, Wasm Runtime Coverage

_Updated 2026-11-15. Written in ASD-STE100 Simplified Technical English (STE)._

## Where this specification is

| Stage | State |
|---|---|
| Survey of the current wasm test surface | done, measured on 2026-11-15 (requirements section 2) |
| Issues filed from the survey | done (#566-#576) |
| Architecture and rules | done (requirements sections 3-6) |
| Feature designs | done: four features, each with `feature.md` and `start.md` |
| Owner approval of the plan | **waiting** |
| Implementation | not started |
| Pull request | not opened (PR #553 holds spec 85 work) |

No product code changes until the owner approves the plan (owner rule: the plan update and
approval come before implementation).

## The features

| Feature | State | Owner of the work | Blocked by |
|---|---|---|---|
| 00-executors | `todo` | one agent, sonnet | nothing |
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

1. Owner reviews the four feature designs and the two narrowed decisions (requirements
   section 4: D2 and D3).
2. On approval, feature 00 starts alone.
3. When 00 merges, features 01, 02 and 03 start in parallel, one agent each.
4. Each feature reports to the Main Agent; the Main Agent reviews against "Done when" and
   commits.
5. One pull request for the specification (owner rule: one branch and one pull request per
   spec).

## Records that must exist before a feature is called done

- the case count per host;
- the R14 break: the change, the case, the red output per host;
- the new issues with a `severity:` label;
- the updates to this file and to `LEARNINGS.md`.

---

_Created: 2026-11-15_
