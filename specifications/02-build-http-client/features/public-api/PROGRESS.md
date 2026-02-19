---
feature: public-api
status: in-progress
started: 2026-02-18
last_updated: 2026-02-20
author: Implementation Agent
progress:
  completed_tasks: 0
  total_tasks: 22
  percent_complete: 0
---

# PROGRESS — Public API (SimpleHttpClient / ClientRequest)

Summary
- Goal: Expose a simple, ergonomic public HTTP client API (`SimpleHttpClient`, `ClientRequest`) that hides TaskIterator internals and supports optional connection pooling and configurable redirect following.
- Current state: Core public APIs (`ClientRequest`, `SimpleHttpClient`) already exist. A minimal `ConnectionPool` was implemented. Verification flagged remaining TODOs in redirect and interim-response handling. After a code review we revised the plan to inline redirect handling in the request stream acquisition and to always send `Expect: 100-continue` in the request head.

Key facts / recent changes
- Implemented: minimal, thread-safe `ConnectionPool` to replace prior stub.
  - File: `backends/foundation_core/src/wire/simple_http/client/pool.rs`
- Doc/lint fixes applied to unblock verification iteration:
  - `backends/foundation_nostd/src/primitives/wait_duration.rs`
  - `crates/config/src/lib.rs`
  - `crates/watchers/src/handlers.rs`
- Remaining blocking items and findings from code review (see below) are all concentrated in:
  - `backends/foundation_core/src/wire/simple_http/client/task.rs`
  - `backends/foundation_core/src/wire/simple_http/client/request.rs`
  - `backends/foundation_core/src/wire/simple_http/impls.rs` (Http response parsing/reader)

Decision (finalized)
- Introduce a dedicated `GetHttpStreamState::RedirectHandling` state and localize the probe/redirect loop there. Overall flow:
  - The `Connecting` state remains responsible for preparing connection-related properties (host, port), obtaining or creating a connection (pool checkout or connect), and converting the `PreparedRequest` into a `SimpleIncomingRequest` (or otherwise preparing a request descriptor) but it will not serialize or send the request body immediately.
  - `Connecting` will render and send only the request descriptor/head (including the `Expect: 100-continue` header) and then transition to `RedirectHandling(Some(simple_request, metadata))`, passing the preserved `SimpleIncomingRequest` (or minimal metadata from `PreparedRequest`) into the new state.
  - `RedirectHandling` owns the `SimpleIncomingRequest` (or the minimal preserved metadata) and the active `SharedByteBufferStream` while it performs the redirection/probe loop:
    - Probe the stream with a short-lived `HttpResponseReader` for an intro within a bounded, configurable timeout.
    - If a 3xx redirect with `Location` is observed:
      - Resolve the `Location` (support absolute and relative forms relative to the original request URL).
      - Construct a follow-up `PreparedRequest` (follow-ups default to GET/no-body unless policy indicates otherwise), drop or strip sensitive headers on cross-host redirects, decrement `remaining_redirects`, and repeat the loop by establishing a connection to the new target (pool checkout or new connect).
    - If `100 Continue` is observed: write the body using `Http11::request_body` for the preserved `SimpleIncomingRequest` and continue normal response reading.
    - If a final non-redirect response is observed (e.g., 417, >=400) before body is written: do not write the body; propagate the response to the caller.
    - If probe times out with no intro: assume the server expects the body, write the body using the preserved request, and continue.
  - When `RedirectHandling` finishes (either by following to a final non-redirect response, writing the body and continuing, or exhausting redirects), it returns `(Option<ResponseIntro>, SharedByteBufferStream<RawStream>)` via the expanded `HttpStreamReady::Done` so the higher layers receive the intro if present and retain the stream for continued reading.
- Rationale: Localizing the probe and redirect loop to a dedicated `RedirectHandling` state keeps `Connecting` simple, makes the redirect flow easier to unit test, and still avoids unnecessary cloning of request bodies because we send only the descriptor first and preserve the `SimpleIncomingRequest` until we decide to write the body (or build a follow-up GET/no-body).

Code review findings (what the codebase currently shows)
+- `backends/foundation_core/src/wire/simple_http/client/task.rs`
+  - `GetHttpRequestStreamTask` can render only the request descriptor via `Http11::request_descriptor(...)` (which emits the request head) and send that to the stream while preserving ownership of the underlying `SimpleIncomingRequest`/`PreparedRequest` metadata. In practice this allows sending only the head (with `Expect: 100-continue`) and then probing the stream for an early response intro without needing to clone the full `PreparedRequest`. Therefore, cloning the entire `PreparedRequest` is not required for the inline probe/redirect loop.
+  - We'll expand `HttpStreamReady::Done` to return a tuple-like value of `(Option<ResponseIntro>, SharedByteBufferStream<RawStream>)` (or an explicit struct) so the caller can receive an immediate intro when available while retaining the stream for subsequent body writes. This enables inline redirect detection and handling without handing off or losing the stream.
+  - `GetRequestIntroTask` already exists and provides a small state machine to read intro+headers via `HttpResponseReader`; we'll reuse it as a short-lived response probe with timeouts.
- `backends/foundation_core/src/wire/simple_http/client/request.rs`
  - `PreparedRequest::into_simple_incoming_request` converts prepared requests into renderable `SimpleIncomingRequest`. We must use `PreparedRequest` metadata to construct follow-up `PreparedRequest` instances for redirects without cloning large body payloads. Currently the `PreparedRequest` owns the body; follow-up defaults must avoid reusing the original body by design.
- `backends/foundation_core/src/wire/simple_http/impls.rs`
  - `HttpResponseReader` provides the reader abstraction we need to parse intro + headers and to hold the stream while we write body later. It supports getting a mutable stream via `stream_mut()` which is essential for the approach.
- Tests: There are existing test stubs and test helpers for end-to-end HTTP server emulation (`foundation_testing::http::TestHttpServer` and helpers). Some redirect-related tests are present but ignored or marked for future enabling; they will be used as the basis for the TDD plan.

Edge cases and tricky behaviors that must be covered
1. Location resolution:
   - Absolute URL in `Location`.
   - Relative path in `Location` (relative to request URL).
   - `Location` containing only path and query.
   - Invalid `Location` header value (should be treated as client error).
2. Redirect semantics:
   - 301/302 default semantics for non-GET methods (historical behavior vs safe default). For Phase 1 we MUST default follow-ups to GET/no body to avoid accidental re-sending of bodies (documented behavior).
   - 303 MUST convert to GET for subsequent request per RFC.
   - Preserve/strip sensitive headers on cross-host redirects (e.g., `Authorization`, `Cookie`) — initial implementation: strip `Authorization` when host changes (document and test).
3. 100-continue and other 1xx responses:
   - Server may respond `100 Continue` to prompt body upload; our probe must treat `100` as informational and, if observed, proceed to write the body.
   - Server may respond a final non-1xx response immediately (e.g., redirect or 200) — must handle by not sending body unless the server expects it; when a final non-redirect response is seen, we will proceed to write body (keeping `Expect: 100-continue` in place) then process response.
   - Server may send other 1xx responses; treat them as informational and continue probing until final or timeout.
4. Timeouts:
   - If the probe doesn't yield an intro within configured timeout → write body and continue.
   - Configurable read/write/connect timeouts must be honored.
   - Tests must avoid flakiness by controlling the TestHttpServer timing.
5. Connection reuse and pooling:
   - If a follow-up redirects to the same host:port and pooling enabled, reuse connection where possible.
   - If we give up a stream due to redirect, ensure not to return the stream to pool incorrectly.
   - On final success, if pooling enabled, return the stream correctly.
6. Transfer encodings and body types:
   - Content-Length bodies vs chunked vs streaming body iterators. Follow-ups default to GET/no body to avoid resending non-repeatable bodies.
   - If original request is idempotent and the body is repeatable (e.g., bytes/text), follow-up could theoretically resend; initial implementation will avoid this complexity.
7. Security:
   - Do not forward `Authorization` across hosts.
   - Do not inadvertently log request bodies.
8. Relative vs scheme change redirects:
   - Redirect from http -> https (different scheme): initial implementation should not follow scheme-change redirects automatically unless configuration allows; document behavior.
9. Redirect loops and `max_redirects`:
   - Enforce `max_redirects` and produce a clear `HttpClientError::TooManyRedirects` or equivalent.
10. Header canonicalization and case insensitivity:
    - Ensure header name matching (e.g., `Location`) is case-insensitive when parsing.

Missing pieces to implement (high-level)
- Add new state to task internals:
  - `GetHttpStreamState::RedirectHandling(Option<(SimpleIncomingRequest, /* optional metadata from PreparedRequest */)>)` — this state will own the preserved request descriptor/body metadata and the active stream while performing the probe/redirect loop.
- Expand the `HttpStreamReady::Done` variant (or equivalent) to include:
  - `Option<ResponseIntro>` (status/proto/reason when probe observed a response intro before body send)
  - `SharedByteBufferStream<RawStream>`
- Update `GetHttpRequestStreamTask`:
-   `Connecting` responsibilities:
-    - Validate/prep the `PreparedRequest`, fill host/port fields in task inner state, and preserve the `PreparedRequest` and convert to `SimpleIncomingRequest` and hold it for later body write which will be set in the `RedirectHandling` state. Do NOT perform connection checkout or call `connect` here; connection establishment and pool checkout are handled inside `RedirectHandling` for each loop iteration.
- Do NOT render or send the request descriptor in `Connecting`; `Connecting` only validates and preserves the `PreparedRequest` / `SimpleIncomingRequest` metadata and fills host/port in the task state. The connection establishment and descriptor send (with `Expect: 100-continue`) are performed inside `RedirectHandling` for each loop iteration.
-    - Store the preserved `SimpleIncomingRequest` (and minimal metadata) and the active stream in the task inner structure and transition to `RedirectHandling(Some(...))`, handing ownership to that state.
-  `RedirectHandling` responsibilities:
-    - Perform the probe/redirect loop on the stream using a short-lived `HttpResponseReader` with configurable timeout and decide the next action.
-    - When preparing a descriptor to send for a loop iteration (initial or follow-up), the `RedirectHandling` loop will insert `Expect: 100-continue` into the `RequestDescriptor` headers immediately before serializing and sending that descriptor for probing.
-    - On 3xx+Location: resolve the `Location` and derive a modified `RequestDescriptor` (do not reconstruct a full `PreparedRequest` unless explicitly required). Mutate the descriptor's method as required by redirect semantics (e.g., rewrite to GET/HEAD by policy), strip sensitive headers if host changed, decrement `remaining_redirects`, close or abandon the current stream if necessary, and connect to the new target to send the modified descriptor and continue probing.
    - On 3xx+Location: resolve and build a follow-up `PreparedRequest` (defaults to GET/no-body), drop/strip sensitive headers for cross-host redirects, decrement `remaining_redirects`, and perform a new connection + descriptor send to the new endpoint. Continue looping in `RedirectHandling`.
    - On 100 Continue: write the body via `Http11::request_body()` using the preserved `SimpleIncomingRequest` and yield the stream/intro as appropriate.
    - On final 4xx/5xx or 417 before body: do not write the body; return the response intro and stream.
    - On probe timeout: write the body and continue.
- Helper utilities:
  - `resolve_location(base: &ParsedUrl, location_header: &str) -> Result<ParsedUrl, HttpClientError>`
  - `strip_sensitive_headers_for_redirect(headers: &mut SimpleHeaders, original_host: &str, new_host: &str)`
- Config:
  - Expose redirect policy flags later if needed (`follow_on_same_scheme_only`, `follow_cross_scheme`, `strip_auth_on_host_change`).
- Error handling:
  - Add new variants (`InvalidLocation`, `TooManyRedirects`, `RedirectDisallowed`) in `client/errors.rs`.
- Tests (see TDD checklist below).

TDD-style tests to add (unit + integration; prioritized)
- Unit tests (fast, isolated)
  1. `resolve_location_absolute` — absolute Location → returns correct ParsedUrl.
  2. `resolve_location_relative` — relative Location → resolves against base URL.
  3. `strip_auth_on_host_change` — ensure Authorization header is removed when host changes.
  4. `follow_up_request_builder_defaults_to_get` — building follow-up request from PreparedRequest produces GET/no body.
  5. `probe_reader_returns_intro` — `GetRequestIntroTask` returns Intro+Headers when stream contains a response.
  6. `probe_reader_timeouts` — probe respects the timeout and returns no intro (simulate via custom reader).
  7. `max_redirects_enforced` — attempt more redirects than allowed yields TooManyRedirects error.

- Integration tests (use `TestHttpServer` helpers; deterministic):
  1. `test_single_redirect_followed` — server: /redirect → 302 Location /target; /target → 200 OK. Client should end up with 200 and body from /target.
  2. `test_chain_of_redirects_within_max` — series of redirects of length <= max_redirects → final 200 expected.
  3. `test_redirect_loop_exceeds_max` — server that redirects in a loop; client returns TooManyRedirects.
  4. `test_303_changes_to_get` — POST to endpoint that replies 303 Location → follow-up must use GET and get the target response; request body should not be resent.
  5. `test_location_invalid_header` — server returns 302 with invalid Location; client returns InvalidLocation error.
  6. `test_no_interim_response_timeout_writes_body` — server does not respond after head; after probe timeout client writes body and server receives it.
  7. `test_server_sends_100_continue` — server sends `HTTP/1.1 100 Continue` promptly; client then writes body and proceeds; final response processed.
  8. `test_strip_authorization_on_cross_host_redirect` — server redirects to different host; client follow-up removes Authorization header.
  9. `test_connection_pool_reuse_after_redirect_same_host` — when pooling enabled and redirect stays on same host, ensure reuse or correct checkin behavior.
  10. `test_chunked_body_and_probe` — sending chunked/stream body should still be possible after probe timeout (verify body is sent correctly).
  11. `test_redirect_to_https_behaviour` — redirect from http to https should be either disallowed by default or follow depending on config (test default behaviour is conservative: do not follow).

- Fuzz / property tests (future, not required for initial PR)
  - Random Location header forms (encoded, relative, authority-only) to ensure resolve_location is robust.

Verification approach before implementing
- Add unit tests first for the helpers (resolve_location, header-stripping logic, probe-reader behavior).
- Implement the minimal changes: expand Ready variant, adapt GetHttpRequestStreamTask to use probe loop, wire behavior into HttpRequestTask.
- Run unit tests, then the integration tests using `TestHttpServer`. Use small probe timeout in tests (server side controls timing) to make tests deterministic and fast.
- Run the full verification pipeline (CHECK #1 → fmt → clippy → tests → build → docs → audit).

Minimal next steps (concrete, small PR-sized tasks)
1. Add new error variants to `client/errors.rs` for redirect errors (`InvalidLocation`, `TooManyRedirects`, `RedirectDisallowed`).
2. Add `resolve_location` helper and unit tests.
3. Add the new state variant `GetHttpStreamState::RedirectHandling(Option<...>)` and add a field to `GetHttpRequestStreamInner` to hold the preserved `SimpleIncomingRequest` or required metadata (e.g., `pending_simple_request: Option<SimpleIncomingRequest>`).
4. Expand `HttpStreamReady::Done` (or equivalent Ready) to carry `Option<ResponseIntro>` and `SharedByteBufferStream`.
5. Update `GetHttpRequestStreamTask` splitting logic:
   - Keep `Connecting` focused on preparing host/port and obtaining a connection (pool checkout or connect), convert `PreparedRequest` into `SimpleIncomingRequest` but do not render/write the body.
   - Render/send only the request descriptor/head (insert `Expect: 100-continue`) while preserving the `SimpleIncomingRequest` in the inner state, then transition to `RedirectHandling(Some(...))`.
   - In `RedirectHandling`, perform the probe/redirect loop:
     - Probe stream via short-lived `HttpResponseReader` with timeout.
     - On 3xx+Location: resolve location, build follow-up `PreparedRequest` (GET/no-body), connect to follow-up target (pool or connect), send descriptor and continue loop.
     - On 100 Continue: write body using preserved `SimpleIncomingRequest` (`Http11::request_body`) and return stream/intro as needed.
     - On 417 or >=400: do not write body; return intro/stream.
     - On timeout: write body and return stream.
   - Ensure pool/stream ownership semantics are correct when abandoning or reusing streams.
6. Add unit + integration tests (TDD order from the list above), starting with descriptor-only/Expect tests and probe outcomes.
7. Run verification (fmt, clippy, tests), iterate on issues.

Verification artifacts
- Verification report (partial) and raw outputs are saved in the feature folder:
  - `specifications/02-build-http-client/features/public-api/VERIFICATION.md`
  - `specifications/02-build-http-client/features/public-api/verification_outputs/`

Notes / constraints and design decisions to document in code
- Follow-ups default to GET/no body to avoid accidental re-sending of non-repeatable bodies.
- `Expect: 100-continue` is always sent so servers know a body may follow; this simplifies probing semantics.
- Probe timeout should be a small, configurable value (e.g., 200–1000ms depending on environment). Tests should explicitly control timing.
- Strip `Authorization` on host change; preserve when redirect target host equals original host.
- By default do not follow scheme-changing redirects (http -> https) without explicit config.
- Honor `max_redirects`. Produce clear error when exceeded.
- Keep changes minimal and localized to a couple of files so verification is straightforward.

Do not start coding redirect handling until you confirm. After confirmation I'll implement the changes in small, verifiable commits, add tests, and run the verification.
