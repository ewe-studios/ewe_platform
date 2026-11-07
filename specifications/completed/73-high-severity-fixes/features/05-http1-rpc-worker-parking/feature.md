---
feature: "Verify an HTTP/1.1 RPC does not hold a valtron worker, then close #254"
status: "completed"
priority: "high"
depends_on: []
crates: ["foundation_http"]
issues: [254]
model: "sonnet"
branch: "spec73/f05-http1-rpc-worker-parking"
---

# Feature 05: HTTP/1.1 RPC worker parking — verification

_Written in ASD-STE100 Simplified Technical English (STE). Code and identifiers stay as they
are._

**Abbreviations in this file.** HTTP/1.1 (HTTP/1.1); Remote Procedure Call (RPC); AeroSpace and
Defence Industries Association of Europe (ASD), the owner of the STE standard ASD-STE100.

## 1. Goal

Close [#254](https://github.com/ewe-studios/ewe_platform/issues/254) with evidence. The
defect is already fixed on master.

## 2. Measured state (2026-11-07)

`foundation_http/src/shared/serve/mod.rs:74` `ConnectionResult` has an `Async(ServeFuture)`
variant (line 84): "The handler continues asynchronously. The connection task parks on the
future and applies its result (Keep / Take / Close) when it completes."

`ConnectRpcServe::serve` (`foundation_connectrpc/src/native/server.rs:147-226`) returns
`ConnectionResult::Async(Box::pin(fut))` at once. The module doc says "`serve` returns
`ConnectionResult::Async` at once". The connection loop stores it in
`HandlerState::Awaiting` and parks (`foundation_http/src/native/server/connection.rs`
`handle_awaiting`). The static file and static asset handlers use the same shape.

So an HTTP/1.1 RPC does not hold a worker while it waits. The fix landed before this spec.

## 3. Requirements for the pinning test

1. The test proves the property on the current code: a long RPC does not block a short
   request on a one-worker pool.
2. The test uses the public API only and lives in `backends/foundation_http/tests/`,
   registered in the crate's test module.
3. The test has a hard upper bound, so a regression fails instead of hanging.

## 4. Tasks

- [ ] Run the code checks in section 2 and record the lines.
- [ ] Add `a_long_rpc_does_not_block_a_short_request`: a one-worker pool, a handler that
      returns `ConnectionResult::Async` with a future that completes only after a signal, and
      one short request. Assert the short request answers before the long one ends. If an
      in-repo test already proves this, name it instead and do not add a duplicate.
- [ ] Run the verify commands.
- [ ] Close #254 in the PR body with the code evidence and the test name.

## 5. Verify

```bash
export CARGO_PROFILE_DEV_DEBUG=0 CARGO_INCREMENTAL=0
cargo fmt -p foundation_http
cargo clippy --profile uat -p foundation_http --no-deps --all-targets -- -D warnings
cargo test --profile uat -p foundation_http
```

## 6. Done when

- The parking path is confirmed against the code, with line numbers in the PR.
- A test pins the property, or an existing test is named as the proof.
- #254 is closed with the evidence.

## 7. Result (2026-11-07)

Branch `spec73/f05-http1-rpc-worker-parking`, commit `003d11bae`, merged into
`spec73/high-severity-fixes` as `b425b17c2`.

1. **Measured path.** `ConnectionResult::Async` is at `shared/serve/mod.rs:84`. The dispatch
   stores it in `HandlerState::Awaiting` (`connection.rs:891`) and returns `Pending`.
   `handle_awaiting` polls the future and parks with
   `TaskStatus::Depends(AsyncReadiness { .. })` at `connection.rs:945`.
   `ConnectRpcServe::serve` returns `Async` at once (`foundation_connectrpc/src/native/server.rs:226`).
   The defect is already fixed.
2. **Test.** No existing test proved the property. The new test is
   `a_long_rpc_does_not_block_a_short_request` at `tests/async_serve_tests.rs:489`. It uses
   the public API only. Two hard bounds make a regression fail instead of hang.
3. **Sensitivity proof.** The fix predates the spec, so the test passes on the base tree. To
   show the test detects a regression, the parking arm was replaced with `TaskStatus::Ignore`
   for one run. The test failed in 5.05 s with "only 2 of 4 long RPCs started; a parked RPC
   must not hold a worker". The mutation was reverted.
4. **Deviation, pool size.** The public pool API clamps the thread count to at least 3 and
   gives at least 2 task workers. The test asks for the smallest pool and parks four RPCs to
   saturate it. Filed #385 for the macro doc.
5. **Deviation, feature gate.** The file `async_serve_tests.rs` is `#![cfg(feature = "multi")]`.
   The literal verify command compiles it out (77 tests). The counts are: default 77,
   `--features multi` 156, `--features "multi quic"` 159. All pass.
6. **Existing test fixed.** `tests/integration_tests.rs:1097` asserted the keep-alive idle
   default as 120 s. Feature 04 sets it to 75 s, so the suite failed on the base branch. The
   value is now 75 s.
7. **New defects.** Filed #384 (low: the async writer emits `Connection: close` but the
   connection stays open) and #385 (low: `valtron_test` thread count floors at 3).
