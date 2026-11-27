# Fresh verification: spec 71 claims

_Date: 2026-11-27. Branch: `spec-durable-streams`. Head: `3f5377519`._

This report records a fresh check of the spec 71 claims. The verifier read the spec,
the feature files and the handoff, then ran the gates and read the code. No number here
is copied from another report. Every count below is from a command the verifier ran.

**Abbreviations.** Continuous Integration (CI); Durable Object (DO); Hypertext Transfer
Protocol (HTTP); JavaScript Object Notation (JSON); Server-Sent Events (SSE); time to
live (TTL); virtual file system (VFS); WebAssembly (wasm).

**Host note.** The host load average was 44 during the runs. The handoff section 4.4 and
report section 8 say the strict gate needs a quiet host. The verifier worked around the
load for the server suite (see D1).

## 1. Claim-by-claim table

| Claim | Verdict | Evidence |
|---|---|---|
| Server conformance: 338 passed (338), `subscriptions: true`, zero skip | Verified | Vitest serial run: `Tests  338 passed (338)`. See D1. |
| Client conformance: 267 passed, 0 failed, 2 skipped, both `Swift implementation broken` | Verified | `Total: 269 tests`, `Passed: 267`, `Failed: 0`, `Skipped: 2`; both skip lines carry `(skipped: Swift implementation broken)`. |
| Direction 1: TypeScript server suite against our server | Verified | Same 338-run above. |
| Direction 2: TypeScript client suite against our client | Verified | The client run above. |
| Direction 3: reference Rust client against our server (6 tests) | Verified | `cargo test --profile uat -p foundation_http --features multi --test durable_reference_cross_run` → `running 6 tests`, `test result: ok. 6 passed`. |
| Direction 4: our client against our server (5 tests) | Verified | `cargo test --profile uat -p foundation_http --features multi --test durable_cross_client` → `running 5 tests`, `test result: ok. 5 passed`. |
| Transport matrix: eight Given/When/Then scenarios, model oracle first, then cross-driver equality | Verified | `Scenario::all()` lists 8 (`integration/durable_transport_matrix/src/scenario.rs:59`). `assert_matrix` compares each driver with the model, then drivers with each other (`tests/transport_matrix.rs:20-82`). |
| Five drivers: embedded, HTTP, WebSocket, ConnectRPC, DO | Verified | Native run: `running 9 tests`, `9 passed` (8 scenarios + the list test). DO run: `cargo test --profile uat -p durable_transport_matrix --features do-driver --test do_driver` → `1 passed` (all 8 scenarios against the model and the embedded driver). |
| No-valtron build: `--no-default-features --features std,alloc` | Verified | `integration/durable_core_no_valtron/Cargo.toml` line 23. `cargo test --profile uat -p durable_core_no_valtron` → `running 11 tests`, `11 passed`. |
| Hosted object stores: live S3 over MinIO | Verified | Gate live section: `test result: ok. 53 passed; 0 failed; 0 ignored; ... finished in 4.55s`, `live S3: passed against http://127.0.0.1:9000`. Includes `live::live_s3_conformance`, `live::live_s3_two_writers_one_cas_wins`, `live::live_s3_restart_replays_the_spool`, `live::live_sql_vfs_flushes_to_s3`. |
| S3 block store | Verified | `blocks::sql_vfs_flushes_to_s3 ... ok`, `blocks::sql_vfs_reads_a_block_range_from_s3 ... ok` in the 53-test run. |
| Local R2 block store under `wrangler dev --local` | Verified | `cargo test --profile uat -p foundation_nativeapis --features r2-wrangler --test r2_wrangler` → `1 passed; ... finished in 6.07s`. |
| wasm32 build of the R2 path | Verified | `cargo check --profile uat --target wasm32-unknown-unknown -p foundation_nativeapis --no-default-features --features vfs-object-wasm` → `Finished`. |
| Derived timer: order Env, then `Clock::timer()`, then `ContextTimer` | Verified | `backends/foundation_context/src/lib.rs:555-572`. Tests `env_timer_overrides_the_default`, `clock_timer_wins_over_the_default`, `ctx_timer_defaults_to_the_clock` (`backends/foundation_context/tests/ctx_timer.rs:107,125,138`). |
| Timer thread moved to `foundation_context::timer` | Verified | `backends/foundation_context/src/timer.rs` (thread `valtron-timer`, `notify_clock_advanced`). Re-export: `backends/foundation_core/src/valtron/timer.rs:13-15`. |
| #1090 fixed at the call sites | Verified | `foundation_event_stream/src/shared/engine.rs:969` (`send_bounded` uses `self.ctx.timer()`); `foundation_agent_rpc/src/core/turn.rs:150`. |
| No stub on a live durable path | Verified | No `#[ignore]`, `todo!`, `unimplemented!` or `unreachable!` in the durable sources. The one `unimplemented!` is a cfg fallback when no HTTP backend feature is on (`foundation_netio/src/network_client.rs:460`). The two ignored tests are doc examples, not durable paths (`fd::completion`, `FileWatcherBuilder`). |
| Feature checkboxes match reality | Verified | Checked tasks in features 08, 09, 10, 11 and spec 92 have code and tests behind them (see section 3). |

## 2. Exact output lines the verdicts rely on

Server suite (serial vitest run, `--sequence.concurrent=false --testTimeout=30000`):

```text
Test Files  1 passed (1)
      Tests  338 passed (338)
```

Client suite (`@durable-streams/client-conformance-tests` 0.2.13):

```text
  ○ SSE base64 data integrity - multiline text (skipped: Swift implementation broken)
  ○ SSE handles 4KB base64 payload with potential line splits (skipped: Swift implementation broken)
Total: 269 tests
Passed: 267
Failed: 0
Skipped: 2
```

Live S3 (gate live section, MinIO `RELEASE.2025-10-15T17-29-55Z`):

```text
test result: ok. 53 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 4.55s
live S3: passed against http://127.0.0.1:9000
```

Cross-runs:

```text
running 5 tests
test result: ok. 5 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 1.07s
running 6 tests
test result: ok. 6 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.75s
```

Transport matrix, no-valtron, DO, R2:

```text
running 9 tests
test result: ok. 9 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 9.68s
running 11 tests
test result: ok. 11 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s
test result: ok. 1 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 12.76s   (DO driver)
test result: ok. 1 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 6.07s    (local R2)
```

Crate tests and clippy (all green): `foundation_context` (ctx_timer 4, timer_wasm 1, plus the
other binaries), `foundation_event_stream` (16 passed), `foundation_nativeapis` (all binaries
green; 2 ignored doc examples), `cargo clippy --profile uat -p foundation_event_stream
--all-targets --no-deps -- -D warnings` exit 0, `cargo clippy --profile uat -p foundation_core
--all-targets --no-deps -- -D warnings` exit 0.

## 3. Feature checkboxes

- Feature 08 T3 (cross-runs), T11 (BDD list) verified above. T1/T2 counts verified above.
- Feature 09 (S3/R2 VFS): the 53-test live run covers the checked conformance and coverage
  tests (`two_writers_one_cas_wins`, `cas_retry_preserves_both_segments`,
  `spool_survives_crash`, `one_put_per_batch`, `large_batch_uses_multipart`,
  `retention_drops_old_segments`, `r2_reports_conditional_write_support`, and so on).
- Feature 10 (subscriptions): the six subscription tests run under `subscriptions: true`
  (suite group `describe.runIf(options.subscriptions)('Reserved subscription APIs', ...)`,
  six tests). The 338-run has zero skip, so all six pass. `foundation_event_stream` unit
  tests: 16 passed.
- Feature 11 (hosted transports): verified above (transport, live S3, block stores, wasm,
  local R2). CI wiring present: `ci/packages.toml:871` `[gates.durable-conformance]`,
  `.github/workflows/check.yaml:763` job.
- Spec 92 (timer): the order, the moved thread and the call sites verified above.
  `SimulatedClock::advance` and `advance_wall` call `notify_clock_advanced`
  (`backends/foundation_core/src/valtron/ctx.rs:79,98`). Test
  `advancing_the_clock_wakes_a_ctx_timer_sleep` exists
  (`backends/foundation_core/tests/valtron/ctx_timer.rs:42`). T6 stays unchecked, as the
  wasm JavaScript default timer is recorded as issue #1093.

## 4. Discrepancies

| ID | Severity | Discrepancy | Issue |
|---|---|---|---|
| D1 | Low, environment | The default concurrent gate run on the loaded host (load 44) failed the server step: `Tests 3 failed | 335 passed (338)`. The three failures are TTL sliding-window tests (`should extend TTL on GET ?offset=now`, `should extend TTL on close-only POST`, `should extend TTL on producer close-only POST`). Each returned 404 because the 2-second TTL expired before the request reached the server under 338 concurrent tests (the suite sleeps 1500 ms, leaving a 500 ms margin). This is an assertion failure, not the timeout the handoff names. Serial reproduction of all three with curl passed (201/200/200, 201/204/200, 201/204/200), and the suite with `--sequence.concurrent=false` reports `338 passed (338)`. The code is correct; the failure is the documented host-load sensitivity (handoff 4.4, report section 8). | None. Not a code defect. |
| D2 | Low, doc | `features/08-conformance/report.md` says the cross-run `tokio` dev-dependency is "declared once in `[workspace.dependencies]`". It is declared once, but in `backends/foundation_http/Cargo.toml` dev-dependencies (lines 92-93), not the workspace root. Wording only. | None. |

No false claim and no real defect was found. No GitHub issue was opened.

## 5. What could not be checked

- The exact line numbers of the two skip entries in `read-sse-base64.yaml` (report section 6
  says lines 55 and 367 at the pinned commit). The verifier confirmed the two skips and their
  reason from the client suite output but did not clone the pinned reference commit to check
  the line numbers.
- A default concurrent gate run on a quiet host. The host was loaded. The verifier instead
  ran the server suite with vitest concurrency off and reproduced the three TTL behaviours
  serially with curl (see D1).
