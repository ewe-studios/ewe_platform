# Feature 13 — Status: COMPLETE (2026-06-11)

| Criterion | Evidence |
|---|---|
| `#[wasm_test]` → discoverable `__fwt_` export; panics become failures | foundation_macros::wasm_test: `__fwt_<name>() -> u32` + `name\|flags` manifest line (`__fwt_manifest` custom section via link_section statics). wasm32 panics ABORT, so the macro installs a user-crate panic hook that ships the failure text over `host_report` BEFORE the trap; runners catch the trap and re-instantiate. |
| Testbed enumerates without running | `foundation_wasm_testbed::fwt::discover_cases` — export scan + manifest flags via OUR wasmbin port (foundation_codegen::wasm). Deviation from spec text ("walrus"): F15 was built with this use case as a success criterion; decision 031 prefers the owned reader. The `__wbgt_` walrus scan remains for the F14 opt-in. |
| Pass/fail/panic over the owned ABI + runner summary/exit code | `host_report(status, ptr, len)` import (0/1/2 + UTF-8 message) → JS `TestReports` (promise-based `next()` for async). The F12 runner prints per-case verdicts + a `test result:` summary and exits non-zero on failure. |
| Async via the owned executor | `testing::run_async`: `Waker::noop()` poll; `Pending` re-polls via `abi::web::register_schedule(0)` — the owned JS-yield loop. Validated by a Pending-once future resolving through real node timers, and by the four ported valtron JS-yield integration tests. |
| Example green through this path | `foundation_wasm_testbed/integration/module` (5 cases covering every flag) — 5 JS protocol tests + 2 Rust discovery tests green; the runner e2e is F12's tests. Example lives crate-local per the integration-location rule (spec text predates it). |

`should_panic` semantics: a manifest flag the RUNNER inverts on — under abort
semantics the module cannot observe its own expected panic.
