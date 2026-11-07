---
feature: "Verify an HTTP/1.1 RPC does not hold a valtron worker, then close #254"
status: "pending"
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
