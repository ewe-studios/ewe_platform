---
feature: "Client body reads park on readiness"
status: "done"
priority: "critical"
depends_on: ["01-valtron-parking", "29-client-reactor-bridge"]
branch: "fix/io-01-valtron-parking"
crates: ["foundation_netio", "foundation_iogate", "foundation_connectrpc"]
backlog: "docs/bug-backlog.md §2 AsyncSendSafeBody polls sync body iterators inline; SendSafeBodyBytesIterator reports would-block inconsistently"
starting_point: "fix/foundational-readiness commit fcbf523fa: the foundation_netio body_reader.rs, http_exchange_task.rs and foundation_connectrpc dispatch.rs hunks, and tests/simple_http/body_bytes_wait_tests.rs"
---

# Feature 03: Client body reads park on readiness

_Written in ASD-STE100 Simplified Technical English (STE). Code and identifiers stay as they
are._

**Abbreviations in this file.** Hypertext Transfer Protocol (HTTP); file descriptor (fd);
central processing unit (CPU); pull request (PR); AeroSpace and Defence Industries Association
of Europe (ASD), the owner of the STE standard ASD-STE100.

## 1. Goal

Every async caller that reads a streamed HTTP response body (`collect_bytes_async`,
`collect_string_async`, the deployment provider clients, `TurboPufferVectorStore`, D1 and R2
stores, `foundation_ai` generators, connectrpc HTTP/3 bodies) waits for the socket without
holding a worker and without spinning.

Today, `AsyncSendSafeBody::poll_next` (`foundation_netio/src/shared/client/body_reader.rs`,
around line 1889) calls the inner iterator's `next()`:
- if the iterator blocks on the socket, `poll_next` blocks the worker;
- on `Data::Retry` (non-blocking `WouldBlock`) it calls `cx.waker().wake_by_ref()` and returns
  `Pending`: a busy loop.

`SendSafeBodyBytesIterator` also reports "not ready" two ways: `Ignore` for a sized body's
`Data::Retry`, and a `StreamError` for a chunked body's `WouldBlock`. Callers cannot tell
"not ready" from bookkeeping or failure.

## 2. Starting point

`fcbf523fa` (branch `fix/foundational-readiness`) already makes `SendSafeBodyBytesIterator`
report `Stream::Wait` for every body kind and adds `is_would_block` (body_reader.rs) plus
`tests/simple_http/body_bytes_wait_tests.rs`. Take those hunks:

```bash
git checkout fcbf523fa -- backends/foundation_netio/src/shared/client/body_reader.rs \
  backends/foundation_netio/src/http/tasks/http_exchange_task.rs \
  backends/foundation_netio/tests/simple_http/body_bytes_wait_tests.rs \
  backends/foundation_netio/tests/simple_http/mod.rs \
  backends/foundation_connectrpc/src/shared/router/dispatch.rs
```

Read each hunk before you keep it.

## 3. Design

### 3.1 A wake source on the body

`AsyncSendSafeBody` gets an optional **read-wake source**: something that, given a
`std::task::Waker`, arranges for the waker to be woken when the underlying connection becomes
readable, and returns whether it could.

```rust
/// Registers `waker` to be woken when the body's connection is readable.
/// Returns `false` when the transport cannot report readiness.
pub type ReadWakeSource = Arc<dyn Fn(&Waker) -> bool + Send + Sync>;

impl AsyncSendSafeBody {
    pub fn with_read_wake(mut self, source: ReadWakeSource) -> Self { … }
}
```

`poll_next` on a "not ready" item (`Data::Retry`, a `WouldBlock` error, the chunked or line
equivalent):
1. register `cx.waker()` with the source (**register first**);
2. pull the iterator **once more** (data may have arrived between the first pull and the
   registration);
3. if still not ready, return `Pending`.

With no source, or a source that returns `false`, keep today's self-wake, but log once at
`debug` ("body has no readiness source; polling"), so a busy loop is visible.

### 3.2 Where the source comes from

The response body is built where the client still has the connection (`HttpClientConnection`,
`foundation_netio/src/http/connection.rs`). The connection already has
`register_read_waker(ConnWaker) -> bool` (WireGuard overlay) and `readiness(IoInterest)` /
`park_readiness(..)` (the shared reactor, used by valtron tasks through `TaskStatus::Depends`).

The source must cover both:
- **Overlay transports:** wrap `register_read_waker` (check what `ConnWaker` is; adapt a
  `std::task::Waker` to it).
- **Reactor-backed sockets:** `FdReadiness` (`foundation_iogate/src/native/fd_readiness.rs`, from
  feature 02) registers an fd with the shared reactor. Add a way to register a
  `std::task::Waker` that the reactor wakes on readability (for example
  `FdReadiness::register_waker(&Waker)`), if feature 02 did not add one. The reactor drain
  thread already requests an executor resume after it latches events.
- **Blocking sockets** (non-blocking mode off): `WouldBlock` means the read timeout ran out.
  That is a failure, not a wait. Return the error.

Attach the source at every place that turns a connection into a `SendSafeBody` stream variant
(start at `foundation_netio/src/shared/http/impls.rs` around lines 453–470, and the response
readers in `http/tasks/`). List every place in the PR body.

### 3.3 Correctness rules

1. Register, then pull once more, then `Pending` (no lost wake).
2. Each `Pending` either has a registered wake or is the logged self-wake fallback.
3. A `WouldBlock` on a blocking socket is an error, never a wait.
4. Bookkeeping items (chunk-end marker, trailers, skipped line, SSE comment) are consumed in a
   loop inside one `poll_next`, not by a self-wake and a new poll.

If §3.2 does not fit the code (for example the connection is not reachable where the body is
built), stop and report with file and line. Do not invent a global registry.

## 4. Tasks

- [x] T1. Branch from the base after feature 01. Take the §2 hunks. Commit.
- [x] T2. Add `ReadWakeSource` and the `poll_next` logic of §3.1. Loop over bookkeeping items.
- [x] T3. Build the source from the connection (§3.2): overlay, reactor, blocking. Add a
      `std::task::Waker` registration to `FdReadiness` if needed.
- [x] T4. Attach the source at every body construction site. List them in the PR body.
- [x] T5. Regression tests in `backends/foundation_netio/tests/simple_http/` (register in `mod.rs`):
  - `async_body_waits_without_spinning`: a local server sends headers, waits 500 ms, then sends
    the body. An async consumer (`collect_bytes_async`) reads it. Assert the bytes, and that
    the poll count of `poll_next` during the wait is under 10 (wrap the body in a counting
    stream) and the process used under 50 ms of CPU. On the old code the count is in the
    thousands.
  - `async_body_chunked_waits_without_spinning`: the same with a chunked body and with an SSE
    body.
  - `blocking_socket_wouldblock_is_an_error`: a blocking socket with a short read timeout and a
    server that never sends; assert an error, not a hang.
  - Keep `body_bytes_wait_tests.rs` from the starting point; check it covers sized and chunked
    bodies reporting `Wait`.
- [x] T6. Delete the two §2 backlog entries. Update `progress.md`, `learnings.md`.

T3–T6 landed through feature 29 (the reactor bridge): the reactor moved below `foundation_netio`
so the body could reach `FdReadiness`. The `ReadWakeSource` is built in
`RawStream::read_wake_source` and attached in `SimpleResponse`/`into_async_parts`.

## 5. Verification (run exactly)

```bash
export CARGO_PROFILE_DEV_DEBUG=0 CARGO_INCREMENTAL=0
for c in foundation_netio foundation_iogate foundation_connectrpc foundation_deployment foundation_db foundation_ai; do
  python3 scripts/ci/ci.py run $c --profile dev || exit 1
done
cargo check --workspace --all-targets --profile dev
# 20-run flake check for each new test and for body_bytes_wait_tests.
rg -n 'wake_by_ref' backends/foundation_netio/src/shared/client/body_reader.rs   # each hit is the logged fallback only
```

## 6. Done when

- [x] All §5 commands pass; new tests passed 20 runs in a row.
- [x] The spinning tests fail on the base code (poll count / CPU) and pass on the new code.
- [x] Every `wake_by_ref` left in `body_reader.rs` is the logged fallback.
- [x] CI green. (The `quic` run still fails one pre-existing test,
      `idle_h3_connection_uses_no_cpu`, tracked in feature 02/28; it is out of scope here.)

## 7. Out of scope

Non-blocking connect and TLS (feature 04). Pushed request bodies (feature 05).

## 8. Review checklist (main session)

- [ ] Rule 1 in every branch of `poll_next`.
- [ ] The source is attached for TLS connections too (TLS readable ≠ socket readable: a TLS
      record may need more bytes; pulling once more after registration covers it).
- [ ] No caller lost the error on a blocking socket.
