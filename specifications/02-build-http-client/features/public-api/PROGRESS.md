# PROGRESS — Public API: Redirect-capable connection loop
path: specifications/02-build-http-client/features/public-api/PROGRESS.md
status: in-progress
last_updated: 2026-02-21
owner: Implementation Agent
priority: high
estimate: 2-4 days (iterative, test-driven)

## Clarified understanding (key point)
The redirect "loop" is implemented by updating the `GetHttpRequestRedirectTask` state so that the task resets itself to the `HttpRequestRedirectState::Trying(...)` variant with the *same* `SimpleIncomingRequest` and `OpTimeout` but an updated `RequestDescriptor`. Concretely:

- Each iteration only modifies the `RequestDescriptor` (target URL, method, headers) produced for the next connection attempt.
- The `SimpleIncomingRequest` (rendering template) and `OpTimeout` remain unchanged across attempts.
- When a redirect is detected during the `Trying` state, the task should:
  1. Resolve the new location and build a follow-up `RequestDescriptor`.
  2. Decrement `remaining_redirects`.
  3. Replace the current state with `HttpRequestRedirectState::Trying(Some(Box::new((data, timeout, new_descriptor))))`.
  4. Return `Some(TaskStatus::Pending(HttpOperationState::Connecting))` so the executor will call `next()` again and re-run the connect/send/probe step using the new descriptor.
- This approach makes the redirect behavior explicit, deterministic, and simple: the task re-enters the same `Trying` branch on the next invocation to attempt the new connection.

This is the intended semantics: the state machine loop is driven by re-setting the `Trying` state with the updated descriptor, not by spinning an internal synchronous loop inside a single `next()` invocation.

---

## Goal (short)
Implement a redirect-capable connection loop for the public API feature that:
- Uses the `RequestDescriptor` and `HttpConnectionPool` to attempt a connection,
- Sends the request, probes the response intro (status + headers),
- Detects 3xx redirect responses, resolves the `Location` header, and when appropriate:
  - Builds a follow-up request (switch to `GET`/no-body as Phase 1 policy) and strips sensitive headers when host changes,
  - Re-connects to the new target and repeats the loop by resetting the task state to `Trying` with the updated descriptor,
- Observes and respects `max_redirects`, maps failures to `HttpClientError`, and exposes this behavior in `ClientRequest`/`SimpleHttpClient` per `feature.md`.

This work is intended to implement the connection/redirect loop referenced around line ~150 of
`backends/foundation_core/src/wire/simple_http/client/task.rs` and wire it into the `HttpRequestTask`
state machine so verification can pass.

---

## Current state (what I inspected)
Files relevant to the change:
- `backends/foundation_core/src/wire/simple_http/client/task.rs`
  - Contains `GetHttpRequestStreamTask` and `HttpRequestTask` state machine with `TODO (public-api)` about redirect handling.
  - Has `GetHttpRequestRedirectTask` skeleton (partial) that currently enters `Connecting` but does not implement the connect/send/probe/redirect loop.
- `backends/foundation_core/src/wire/simple_http/client/redirects.rs`
  - Provides `resolve_location`, `build_followup_request_from_request_descriptor`, `build_followup_request_from`, and `strip_sensitive_headers_for_redirect` helpers. These should be reused.
- `backends/foundation_core/src/wire/simple_http/client/request.rs`
  - Provides `PreparedRequest` and builder. `PreparedRequest::into_simple_incoming_request()` converts to `SimpleIncomingRequest` for rendering.
- `backends/foundation_core/src/wire/simple_http/client/pool.rs`
  - Connection pool exists (used by tasks) — confirm pool API surface for `create_http_connection`.
- `backends/foundation_core/src/wire/simple_http/client/intro.rs` and `GetRequestIntroTask`
  - Intro reading logic exists; currently `GetRequestIntroTask` reads intro and headers and returns them.

Observations:
- `GetHttpRequestStreamTask` currently performs a blocking connect/send and then returns the connection (no redirect handling).
- `GetRequestIntroTask` reads status and headers, but redirect detection is not implemented in `HttpRequestTask::Reading` (TODO exists).
- `GetHttpRequestRedirectTask` exists but is incomplete: its current `next()` only sets up and signals `Connecting` without performing the connect/send/probe/redirect loop.
- `redirects.rs` already handles URL resolving and follow-up request creation (switch-to-GET behavior), so redirect logic should call those helpers.

---

## Required code changes (high level, clarified)

The area of focus is in `GetHttpRequestRedirectTask::next()` at:

- [`task.rs` lines 149–157](file:///home/darkvoid/Boxxed/@dev/ewe_platform/backends/foundation_core/src/wire/simple_http/client/task.rs#L149:157)

The logic to implement is:

1. **Create a new connection** using the current `RequestDescriptor` (from state).
2. **Render the HTTP message** using `Http11::request_descriptor`.
3. **Read the response intro** (status line and headers) from the stream, within the timeout.
4. **If no redirect is indicated** (i.e., not a 3xx with Location), write the body (if any) and return `HttpRequestRedirectResponse::Done`.
5. **If a redirect is indicated** (3xx with Location header):
    - Parse the new location from the response.
    - Build a new `RequestDescriptor` for the follow-up request (using the helpers in `redirects.rs`).
    - Decrement the max redirection count.
    - Set the state back to `HttpRequestRedirectState::Trying` with the new descriptor, same `SimpleIncomingRequest`, same `OpTimeout`, and updated remaining redirects.
    - On the next `next()` call, the process repeats with the new target.
6. **If the max redirection count is exceeded**, return an error.

**Note:**  
- The `max_redirects` (remaining count) should be added to both `Init` and `Trying` variants of `HttpRequestRedirectState` to track and enforce the redirect limit.

---

## How to get the connection and read the response intro (clarified)

**Getting the connection:**
- Use the `HttpConnectionPool`'s `create_http_connection` method with the current `RequestDescriptor`'s URL.
- Example:  
  `let mut connection = pool.create_http_connection(&descriptor.request_url, None)?;`
- On error, return an appropriate error and end the task.

**Rendering and sending the request:**
- Render the HTTP message using `Http11::request_descriptor`.
- Example:  
  `let request_string = Http11::request_descriptor(&descriptor).http_render_string()?;`
- Write the request string to the connection's stream and flush.

**Reading the response intro and headers (with timeout):**
- Use the `ReadTimeoutInto` trait on the stream to read the intro and headers with a timeout.
- Example:  
  ```rust
  let mut buf = [0u8; 4096];
  let n = connection.stream_mut().read_timeout_into(&mut buf, timeout)?;
  // Parse the intro and headers from buf[..n]
  ```
- With the new `ReadTimeoutOperations` trait, you can set the read timeout directly on any stream or connection using `set_read_timeout_as(timeout)`.
- If using `HttpResponseReader`, simply call `set_read_timeout_as(timeout)` on the connection before reading. The reader will automatically honor the timeout for all reads.
- No need to wrap or modify `HttpResponseReader` itself.

**Redirect detection and handling:**
- After reading intro and headers, check if the status code is 3xx and a `Location` header is present.
- If so, resolve the new location, build a new `RequestDescriptor`, decrement redirects, and set state back to `Trying` for the next run.
- If not, write the body (if any) and return `Done`.

**Pseudocode:**
```rust
// 1. Get connection
let mut connection = pool.create_http_connection(&descriptor.request_url, None)?;

// 2. Render and send request
let request_string = Http11::request_descriptor(&descriptor).http_render_string()?;
connection.stream_mut().write_all(request_string.as_bytes())?;
connection.stream_mut().flush()?;

// 3. Read response intro and headers with timeout
let mut buf = [0u8; 4096];
let n = connection.stream_mut().read_timeout_into(&mut buf, timeout)?;
let (intro, headers) = parse_intro_and_headers(&buf[..n]); // parse as needed

// 4. Check for redirect
if is_redirect(&intro, &headers) {
    // handle redirect: resolve location, build new descriptor, decrement redirects, set state to Trying
} else {
    // write body if needed, return Done
}
```

## ReadTimeoutOperations integration plan

- All response intro/header reads in the redirect-capable task should use `ReadTimeoutOperations` with the timeout from `OpTimeout`.
- Before reading the response intro and headers, call `set_read_timeout_as(timeout)` on the connection or stream.
- All subsequent reads (including those by `HttpResponseReader`) will respect this timeout.
- When reading directly, use `read_timeout_into` for explicit timeout-gated reads.
- This prevents indefinite blocking and ensures robust timeout handling for all HTTP response reads.

**Example:**
```rust
connection.set_read_timeout_as(timeout)?;
// All reads via HttpResponseReader or direct reads will respect this timeout.
let mut reader = HttpResponseReader::<SimpleHttpBody, RawStream>::new(connection.clone_stream(), SimpleHttpBody);
let intro = reader.next()?; // Will timeout if server stalls
```

**Direct read example:**
```rust
let mut buf = [0u8; 4096];
let n = connection.read_timeout_into(&mut buf, timeout)?;
```

## Concrete next steps (break into small PR-friendly tasks)

1. Implement redirect loop core (no tests yet)
   - Implement the connect/send/probe/redirect loop inside `GetHttpRequestRedirectTask::next()`:
     - On `Trying` state, perform connection via `pool.create_http_connection(&descriptor.request_url, None)`.
     - Render and send the request bytes (reuse `Http11::request(...).http_render_string()`).
     - Probe the intro using `HttpResponseReader` to get status + headers (short read).
     - If redirect -> resolve + build follow-up `PreparedRequest` via helpers and update `descriptor`, decrement `remaining_redirects`.
     - Set the task state back to `Trying` with updated descriptor and return `Pending(HttpOperationState::Connecting)`.
     - If final -> return `HttpRequestRedirectResponse::Done(connection)`.
   - Keep behavior synchronous / blocking for Phase 1 (consistent with existing tasks).
   - Add tracing logs for each step.

2. Wire redirect task into `HttpRequestTask`
   - Replace spawning of plain `GetHttpRequestStreamTask` with the redirect-capable task (or construct redirect task from `GetHttpRequestStreamInner` so pool and remaining redirects are available).
   - Ensure produced `TaskStatus` states follow existing execution patterns: spawn the redirect task and handle `HttpRequestRedirectResponse::Done` similar to current `HttpStreamReady::Done`.

3. Tests:
   - Add tests for:
     - Single redirect followed by success (3xx -> Location -> 200).
     - Multiple redirects chain up to `max_redirects` -> error.
     - Invalid `Location` header -> `InvalidLocation` error.
     - Header stripping when host changes.
     - POST->GET follow-up semantics.
   - Use a mock `RawStream` or a small test harness to simulate server responses deterministically.

4. Error handling and API polish:
   - Add or map `HttpClientError::TooManyRedirects` or reuse an existing variant with clear message.
   - Ensure headers are stripped using `strip_sensitive_headers_for_redirect`.
   - Document behavior in `feature.md` if any policy changes occur.

5. Verification and cleanup:
   - Run `cargo fmt` and `cargo clippy`.
   - Ensure no `TODO (public-api)` comments remain in `task.rs` to pass verification agent's incomplete implementation scan.

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
- [ ] Redirect-capable connection loop implemented in `GetHttpRequestRedirectTask` (or equivalent) and integrated into `HttpRequestTask`.
- [ ] `TODO (public-api)` comment removed from `backends/foundation_core/src/wire/simple_http/client/task.rs`.
- [ ] Unit tests added for:
  - Single redirect then success,
  - Multiple redirects up to `max_redirects` failure,
  - Invalid Location header handling,
  - Header stripping when host changes,
  - Redirect from POST -> GET semantics.
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

Which do you prefer: code patch now, or test-first TDD approach?