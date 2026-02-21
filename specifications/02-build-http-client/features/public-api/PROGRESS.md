# PROGRESS — Public API: Redirect-capable connection loop
path: specifications/02-build-http-client/features/public-api/PROGRESS.md
status: in-progress
last_updated: 2026-02-21
owner: Implementation Agent
priority: high
estimate: 2-4 days (iterative, test-driven)

---

## Current Implementation State (2026-02-21)

- The redirect-capable connection loop is now implemented in `GetHttpRequestRedirectTask`.
- The state machine uses `Init`, `Trying`, and `WriteBody` states, with transitions based on response intros, headers, and redirect detection.
- Tracing logs (`tracing::debug`, `tracing::info`, `tracing::error`) are added throughout for observability.
- Error handling is robust: connection, write, flush, and redirect resolution errors are surfaced via `HttpRequestRedirectResponse`.
- The `FlushFailed` variant is included to signal flush errors with connection and error details.
- Redirect logic:
  - Handles 3xx status and `Location` header.
  - Uses `redirects.rs` helpers for location resolution and follow-up descriptor creation.
  - Decrements `remaining_redirects` and enforces redirect limits.
  - Fallbacks to `WriteBody` state if intro or headers are missing.
- Body streaming uses `Http11::request_body(...).http_render_to_writer(...)` for correctness.
- All timeouts are managed via `ReadTimeoutOperations` and restored after reads.
- The state machine is fully flattened for clarity and maintainability.

---


## Implementation History (2026-02-21)
[Section moved above for historical context. See "Current Implementation State" for summary.]

---

## Remaining Tasks & Next Steps

### Test Placement (Explicit, crate-specific)

- **Unit tests for `simple_http`:**
  - Place all unit tests for this feature in `tests/backends/foundation_core/units/simple_http/`
  - Example: `http_redirect_task.rs` for state machine/unit tests
  - For state machine transitions, error handling, header stripping, POST→GET, flush failures.

- **Integration tests for `simple_http`:**
  - Place all integration tests for this feature in `tests/backends/foundation_core/integrations/simple_http/`
  - Example: `http_redirect_flow.rs` for end-to-end redirect scenarios
  - For multi-server redirect chains, failure cases, body handling edge cases, end-to-end flows.

- **Test module registration:**
  - Update `tests/backends/foundation_core/units/simple_http/mod.rs` and `tests/backends/foundation_core/integrations/simple_http/mod.rs` to include new test modules.
  - Ensure `Cargo.toml` references the test directory for discovery.

- **Documentation:**
  - Document test scenarios in `VERIFICATION.md` or feature-specific test plans as needed.
  - Use selective test execution for current work; let verification handle full suite runs.


- [ ] Wire redirect task into `HttpRequestTask` (if not already done).
- [ ] Add comprehensive unit tests and integration tests:

### Unit Tests
- Test state transitions for `GetHttpRequestRedirectTask`:
    - Init → Trying → WriteBody → Done for normal flow.
    - Trying → Trying (redirect loop) for multiple redirects.
    - Trying → WriteBody for missing intro/headers.
    - WriteBody → Done for successful body write.
    - WriteBody → FlushFailed for flush error.
- Test error handling:
    - Connection failure, write failure, invalid location, too many redirects.
    - Ensure all error variants are surfaced and handled.

### Integration Tests (in `tests/backends/foundation_core`)
- **Multi-server redirect scenarios:**
    1. Test with two servers: first redirects to second, second responds successfully.
    2. Test with three servers: first redirects to second, second redirects to third, third responds successfully.
    3. Test with four servers: chain redirects through all, fourth responds successfully.
    - These tests verify handling of multiple hosts and chained redirects.
- **Redirection failure cases:**
    - Test where one of the redirections fails (e.g., third server does not exist or is unreachable).
    - Ensure proper error is returned and connection is cleaned up.
- **Body handling edge cases:**
    - Test where the second server never redirects but responds (without body sent).
    - Verify if client handles response correctly, whether it processes the body or returns an error.
    - Test POST→GET follow-up semantics and header stripping when host changes.
- **Flush failure scenarios:**
    - Simulate flush errors and ensure `FlushFailed` variant is returned with connection and error.

**Instructions:**
- Place all integration tests in `tests/backends/foundation_core` as per `feature.md`.
- Update `Cargo.toml` and `mod.rs` to include new test files.
- Document test scenarios in `VERIFICATION.md` or feature-specific test plans.
- Use selective test execution for current work; let verification handle full suite runs.

- [ ] Finalize error mapping and polish:
    - Ensure all error variants are surfaced and documented.
    - Confirm `TooManyRedirects` and other edge cases are handled.
    - Ensure headers are stripped using `strip_sensitive_headers_for_redirect`.
- [ ] Documentation:
    - Update `feature.md` and `PROGRESS.md` as implementation evolves.
    - Add notes for any policy changes or architectural decisions.
- [ ] Verification and cleanup:
    - Run `cargo fmt` and `cargo clippy`.
    - Remove any remaining `TODO (public-api)` comments from `task.rs`.
    - Ensure all acceptance criteria are met.

---


## Proposed API/behavior decisions (confirmed)
- Follow `redirects.rs` policy for Phase 1:
  - Follow-up requests default to `GET` with no body (safe default).
  - Strip `Authorization` when host changes.
- Respect `max_redirects` exactly; when exceeded return a clear `HttpClientError` variant.
- Phase 1: blocking behaviour is acceptable; non-blocking will be future improvement.
- Redirect follow-ups will always use `PreparedRequest` builders and the `HttpConnectionPool` for connections.
- The redirect task will perform the probe and only return a final connection for non-3xx responses — this keeps `HttpRequestTask` simpler.

---

## Acceptance checklist (what must be true for this ticket to be complete)
- [ ] Redirect-capable connection loop implemented in `GetHttpRequestRedirectTask` and integrated into `HttpRequestTask`.
- [ ] Tracing logs present for all major state transitions and errors.
- [ ] Flush failures surfaced via `FlushFailed` variant.
- [ ] Unit tests added for:
    - Single redirect then success,
    - Multiple redirects up to `max_redirects` failure,
    - Invalid Location header handling,
    - Header stripping when host changes,
    - Redirect from POST -> GET semantics,
    - Flush failure scenarios.
- [ ] `HttpClientError` maps redirect errors clearly (invalid location, too many redirects).
- [ ] `cargo fmt` and `cargo clippy` are clean (no new warnings/errors).
- [ ] Documentation updated (`feature.md` notes and `PROGRESS.md` reflects final status).

---

## Risks & blockers
- Interaction between `GetRequestIntroTask` and redirect probing must be designed carefully to avoid double-reading the stream.
- Reusing connection pooling when switching hosts requires returning or discarding pooled connection correctly.
- Tests need a deterministic, controllable server or a mockable stream to simulate 3xx sequences.

---

## Immediate next action (what I'll do next)
1. Implement `GetHttpRequestRedirectTask::next()` with the connect/send/probe/redirect loop using `redirects.rs` helpers and rendering logic from `task.rs`.
2. Replace the plain `GetHttpRequestStreamTask` spawn with the redirect-capable task (or construct redirect task from `GetHttpRequestStreamInner`) so the pool and remaining redirects are available.
3. Add unit tests for the simple redirect chain case and iterate until green.

If you prefer, I can start by producing the minimal code patch for `GetHttpRequestRedirectTask` (small PR) showing:
- the `Trying` branch implementation (connect/send/probe),
- the state-reset-to-`Trying` behavior on redirect, and
- the updated `new()` signature if we add `pool` to task construction.

We follow test-first TDD approach
