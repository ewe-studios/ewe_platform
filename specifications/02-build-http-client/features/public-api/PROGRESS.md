---
feature: public-api
status: in-progress
started: 2026-02-18
last_updated: 2026-02-18
author: Implementation Agent
progress:
  completed_tasks: 0
  total_tasks: 17
  percent_complete: 0
---

# PROGRESS — Public API (SimpleHttpClient / ClientRequest)

Summary
- Goal: Expose a simple, ergonomic public HTTP client API (`SimpleHttpClient`, `ClientRequest`) that hides TaskIterator internals and supports optional connection pooling and configurable redirect following.
- Current state: Core public APIs (`ClientRequest`, `SimpleHttpClient`) already exist. A minimal `ConnectionPool` was implemented. Verification failed due to a remaining TODO for redirect handling in the client task state machine.

Key facts / recent changes
- Implemented: minimal, thread-safe `ConnectionPool` to replace prior stub.
  - File: `backends/foundation_core/src/wire/simple_http/client/pool.rs`
- Doc/lint fixes applied to unblock verification iteration:
  - `backends/foundation_nostd/src/primitives/wait_duration.rs`
  - `crates/config/src/lib.rs`
  - `crates/watchers/src/handlers.rs`
- Remaining blocking item: redirect handling TODO in:
  - `backends/foundation_core/src/wire/simple_http/client/task.rs` (CHECK #1 failure)

Decision (recorded)
- Chosen approach for redirects: inline redirect handling inside `HttpRequestTask` so the same TaskIterator yields the final response (simpler to deliver final response to `ClientRequest`).
- Implementation will only begin after explicit approval.

Minimal next steps (once approved)
1. Refactor `GetHttpRequestStreamInner` to preserve request metadata sufficient to create follow-up `PreparedRequest` for redirect targets (follow-ups will default to GET and no body to avoid heavy cloning).
2. Detect 3xx statuses and `Location` header during the `Reading` stage; resolve `Location` and construct a new `PreparedRequest`.
3. Loop inline inside `HttpRequestTask`, decrementing `remaining_redirects`, and perform follow-up requests until final non-3xx response or `max_redirects` exhausted.
4. Add unit tests:
   - single redirect → final response
   - redirect loop exceeding max → error
   - malformed/absent `Location` on 3xx → error
5. Re-run full verification (CHECK #1 → fmt → clippy → tests → build → docs → audit).

Verification artifacts
- Verification report (partial) and raw outputs are saved in the feature folder:
  - `specifications/02-build-http-client/features/public-api/VERIFICATION.md`
  - `specifications/02-build-http-client/features/public-api/verification_outputs/`

Notes
- Do not start coding redirect handling until you confirm. After confirmation I'll implement the minimal inline redirect flow, add tests, and run the full verification sequence.