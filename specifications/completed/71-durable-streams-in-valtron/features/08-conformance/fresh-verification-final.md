# Fresh verification (final): spec 71 claims after the 2026-11-28 gap closure

_Written in ASD-STE100 Simplified Technical English (STE). Code and identifiers stay as they
are._

_Date: 2026-11-28. Branch: `spec-durable-streams`. Head: `0de3b6662`._

**Abbreviations in this file.** Continuous Integration (CI); Durable Object (DO); Hypertext
Transfer Protocol (HTTP); JavaScript Object Notation (JSON); Server-Sent Events (SSE); virtual
file system (VFS); Amazon Simple Storage Service (S3).

This report checks the claims added after the previous verification
(`features/08-conformance/fresh-verification.md`, head `3f5377519`). The verifier read the
handoff, `requirements.md`, the feature files 01 to 11, the spec 94 feature file and the
previous reports, then ran the commands and read the code. No number here is copied from
another report. Every count below is from a command the verifier ran.

**Host note.** The runs below happened on the quiet worktree host (macOS, Darwin 27.2.0). The
gate passed on this host with no load-related failure.

## 1. Claim-by-claim table

| Claim | Verdict | Evidence |
|---|---|---|
| Feature 01/05: the full core suite is 1,046 passed with default features, 0 failed, 0 ignored | Verified | `cargo test --profile uat -p foundation_core`. Sum of the 25 target lines: 1,046 passed, 0 failed, 0 ignored. The `mod` target reports 914 passed and the doc-tests report 71 passed. |
| Feature 01/05: the full core suite is 1,134 passed with `test-clock,multi`, 0 failed, 0 ignored | Verified | `cargo test --profile uat -p foundation_core --features test-clock,multi`. Sum of the target lines: 1,134 passed, 0 failed, 0 ignored. The `mod` target reports 1,001 passed and the doc-tests report 74 passed. |
| Feature 06: `security_headers_present` and `injected_error_is_typed` pass | Verified | `cargo test --profile uat -p foundation_http --features multi --test durable_adapter -- security_headers_present injected_error_is_typed` reports `2 passed; 0 failed; 0 ignored`. Tests: `backends/foundation_http/tests/durable_adapter.rs:792` and `:877`. |
| Feature 06: #858 is fixed by the Cranelift cfg in `foundation_connectrpc/build.rs` | Verified | `build.rs` has `emit_cranelift_cfg()` which emits `cargo::rustc-cfg=cranelift_backend`. The test `panic_in_handler_becomes_internal_via_recover` is gated with `#[cfg_attr(cranelift_backend, ignore = "...")]` (`backends/foundation_connectrpc/tests/interceptor_tests.rs:249-252`). `cargo test --profile uat -p foundation_connectrpc --test interceptor_tests panic_in_handler_becomes_internal_via_recover` reports `1 passed; 0 ignored` on this macOS host. Issue #858 is CLOSED. |
| Feature 06: T11 is open for the missing golden oracle (#1167) | Verified | `gh issue view 1167`: OPEN, `severity: low`, title `foundation_http: durable feature 06 T11 has no golden oracle`. |
| Feature 07: the client core owns retry, producer, dynamic headers and the injected clock (#1135); the conformance adapter uses them | Verified | The core holds `ctx: Option<Ctx>` and `with_ctx` (`backends/foundation_core/src/durable/shared/client/core.rs:33,238`) and exports `RetryPolicy`, `Producer`, `DynamicHeaders` (`client/mod.rs:20-28`; files `retry.rs`, `producer.rs`, `dynamic.rs`). The adapter imports `foundation_core::durable::client::{DurableClient, Producer, DynamicHeaders, ClientError, ...}` (`backends/foundation_netio/examples/durable_client_adapter.rs:80,145,212`). Issue #1135 is CLOSED. |
| Feature 08: the `durable-valtron` gate runs reader, writer, wait and wake as separate groups | Verified | `ci/packages.toml:968` `[gates.durable-valtron]`; `.github/workflows/check.yaml:884` job with `matrix: group: [reader, writer, wait, wake]` (line 902) and `cargo run ... run durable-valtron --step test --group ${{ matrix.group }}` (line 939); `backends/foundation_ci/src/execution/recipes.rs:265` `VALTRON_GROUPS` has the four groups. All four groups ran and passed: reader 4 tests, writer 1, wait 1, wake 1. |
| Feature 08: the fault-injection and malformed-input fuzz oracles pass; T6 and T17 are ticked | Verified | `cargo test --profile uat -p foundation_core --features test-clock,multi --test mod -- durable::fault_injection::injected_error_is_typed durable::fuzz_harness::malformed_input_never_panics` reports `2 passed`. `features/08-conformance/feature.md` line 126 T6 is `[x]` and line 212 T17 is `[x]`. |
| #1134: `DurableStream` keeps a forked `Ctx`, so the clock stays in the context and the cycle is gone | Verified | `DurableStream { vfs, ctx: Ctx }`; `new` stores `ctx.fork()` (`backends/foundation_core/src/durable/shared/mod.rs`). `cargo test --profile uat -p foundation_core --test mod -- durable::leak` reports `1 passed`; the nine TTL tests advance the simulated clock after construction and pass, which proves the stream reads the context clock. Issue #1134 is CLOSED. |
| Netio clippy is clean and `RedirectConfig` carries the documented allow (#1024/#1155) | Verified | `cargo clippy --profile uat -p foundation_netio --all-targets --no-deps -- -D warnings` finishes with exit 0. The allow is `#[allow(clippy::struct_excessive_bools)]` with the comment "Tracked as issue #1024" (`backends/foundation_netio/src/shared/client/config.rs:21-24`). Issues #1024 and #1155 are CLOSED. |
| The strict gate still holds: server 338 of 338; client 267 passed, 2 upstream-forced skips, our base64 coverage; DO and local R2 | Verified | `bash ci/durable_conformance.sh` exits 0. Output: `Tests  338 passed (338)`; `client conformance: 267 passed, 0 failed, 2 skipped`; base64 SSE coverage 3 passed; DO driver `1 passed` (40.50 s); local R2 `1 passed` (30.68 s). The live S3 section printed `live S3: skipped (EWE_S3_ENDPOINT is not set ...)`. |
| Cross-runs 5 + 6 still pass | Verified | `cargo test --profile uat -p foundation_http --features multi --test durable_cross_client --test durable_reference_cross_run` reports `5 passed` and `6 passed`. |
| Five-driver matrix holds (embedded, HTTP, WebSocket, ConnectRPC native; DO in the gate) | Verified | `cargo test --profile uat -p durable_transport_matrix` reports `9 passed` (8 scenarios and the list test). The DO driver runs in the gate (above). |
| Spec 94: `Ctx::timer` order and the moved thread | Verified | Order in `try_timer`: Env timer, then `Clock::timer()`, then the cached `ContextTimer` default (`backends/foundation_context/src/lib.rs:555-572`). The thread moved to `backends/foundation_context/src/timer.rs`; `foundation_core::valtron::timer` re-exports it (`backends/foundation_core/src/valtron/timer.rs`). Tests `env_timer_overrides_the_default`, `clock_timer_wins_over_the_default`, `ctx_timer_defaults_to_the_clock` (`backends/foundation_context/tests/ctx_timer.rs:107,125,138`). |
| CI plumbing: `foundation-ci plan` lists both gates; the workflow lints | Verified | `cargo run --profile uat -p foundation_ci -- plan` prints `gates: durable-conformance (...), durable-valtron (...)`. `actionlint .github/workflows/check.yaml` exits 0. |

## 2. Discrepancies

| ID | Severity | Discrepancy | Issue |
|---|---|---|---|
| D1 | Low, documentation | `features/06-http-rpc-surface/feature.md` is stale. T11 (lines 110-114) still says "No `injected_error_is_typed` and no `security_headers_present` test exists for the HTTP adapter"; both exist and pass (`backends/foundation_http/tests/durable_adapter.rs:792,877`). Open items (lines 133-138) still call #858 "the known macOS failure"; #858 is closed by the Cranelift cfg. The newest `progress.md` entry is correct; the feature file was not updated to match. | #1170 (`severity: low`) |

No false claim was found among the claims added after the previous verification. One issue was
opened for the stale feature record (D1).

## 3. What could not be checked

- The live S3 section of the gate. `EWE_S3_ENDPOINT` is not set on this host, so the gate printed
  `live S3: skipped`. The claim under review accepts that skip.
- The GitHub-hosted Linux CI run itself (MinIO service container, `ubuntu-latest` jobs). This
  report ran the local equivalents: `actionlint`, `foundation-ci plan`, and the gate and matrix
  commands on macOS. The CI MinIO live S3 step needs the S3 variables and Docker and was not run.
- Spec 94 issue #1093 (the wasm JavaScript default timer). Another session owns it, per the
  verification brief. It was not checked.
