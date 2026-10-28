---
feature: "Pushed request body and the accept loop park on readiness"
status: "pending"
priority: "high"
depends_on: ["02-h2-h3-server-readiness", "04-nonblocking-connect-tls"]
branch: "fix/io-05-push-body-and-accept"
crates: ["foundation_netio", "foundation_http"]
backlog: "docs/bug-backlog.md §2 a pushed request body should wait on pipe readiness (task #24); §3 The accept loop polls a non-blocking listener"
---

# Feature 05: Pushed request body and the accept loop park on readiness

_Written in ASD-STE100 Simplified Technical English (STE). Code and identifiers stay as they
are._

**Abbreviations in this file.** remote procedure call (RPC); Hypertext Transfer Protocol
version 1.1 (HTTP/1.1); file descriptor (fd); too many open files (EMFILE); pull request (PR);
AeroSpace and Defence Industries Association of Europe (ASD), the owner of the STE standard
ASD-STE100.

## 1. Goal

1. **Pushed request body.** A client-streaming or bidirectional RPC pushes request bytes into a
   `PushableRequestBody` (`foundation_netio/src/shared/http/pushable_body.rs`) over a valtron
   `Pipe`. When the producer has nothing yet, the request writer task
   (`foundation_netio/src/http/tasks/request_redirect.rs`, `WritingBody`, around lines 920–940)
   retries at once, then backs off 1, 2, 4 … ms (`MAX_PRODUCER_BACKOFF_MS`). It must instead
   park until the producer pushes or closes.
2. **Accept loop.** `foundation_http/src/native/server/mod.rs` `serve_with_listener` (around
   lines 671–765) runs on its own thread. On `WouldBlock` it `thread::sleep(would_block_sleep)`
   and tries again. It must wait for the listener to be readable, and wake at once on shutdown.

## 2. Design

### 2.1 Pushed body

- Feature 02 added `PipeReceiver::register_waker` / pipe wake registration. The writer task
  parks with `TaskStatus::Depends(readiness)` where the readiness becomes ready on a push or a
  close of the pipe. Use the pipe's existing wake registration; if only a `std::task::Waker`
  form exists, add an `EventReadiness` adapter next to it in `foundation_core` (one small type,
  with a test).
- Register first, then check the pipe once more, then park (no lost wake).
- Remove `producer_stalls` back-off and `MAX_PRODUCER_BACKOFF_MS` if nothing else uses them.
- A closed pipe ends the body (zero-length chunk for chunked encoding). A dropped producer
  without close is an error for the request, as today. Check today's behaviour first and keep it.

### 2.2 Accept loop

This loop is a dedicated thread, a real sync boundary. A blocking wait here is correct; a sleep
loop is not.

- Wait for the listener fd to be readable **or** for shutdown, with no fixed sleep. Options, in
  order of preference:
  1. The shared reactor: an `FdReadiness` on the listener plus the shutdown signal, waited on
     with `is_ready(Some(timeout))` where `timeout` is only a safety upper bound from config.
  2. `poll(2)` on the listener fd and a wake fd (eventfd or a pipe) that shutdown writes to.
  Choose one; write why in the PR body. Windows: if neither fits, keep the sleep there behind
  `cfg(windows)` with a backlog entry.
- On an accept error (for example `EMFILE`), keep a back-off, but take it from config
  (`accept_error_sleep`), as nginx does with its accept back-off. That is not a readiness
  problem.
- Shutdown must stop the loop within one wake, not within one sleep.

## 3. Tasks

- [ ] T1. Branch from the base after features 02 and 04.
- [ ] T2. Pushed body parks on the pipe (§2.1). Delete the back-off code.
- [ ] T3. Accept loop waits on readiness or shutdown (§2.2). Keep `would_block_sleep` only if
      some path still needs it; otherwise deprecate the config field with a doc note (do not
      break callers).
- [ ] T4. Regression tests:
  - `foundation_netio/tests/…` `pushed_body_waits_without_polling`: a client-streaming request
    whose producer pushes one chunk every 300 ms, three times, then closes. Count the writer
    task's polls; assert under 20 (the old back-off polls many times), the server receives the
    bytes in order, and the request completes.
  - `foundation_netio/tests/…` `pushed_body_producer_drop_fails_the_request` (keeps today's
    behaviour; adjust to what the code does today and say so).
  - `foundation_http/tests/…` `accept_loop_idle_uses_no_cpu`: start a server, stay idle 1 s,
    assert under 20 ms CPU for the accept thread or the process.
  - `foundation_http/tests/…` `accept_loop_stops_at_once_on_shutdown`: signal shutdown; assert
    the loop exits in under 50 ms (the old loop may wait up to one sleep).
  - `foundation_http/tests/…` `accept_loop_accepts_after_idle`: after 1 s idle, a connection is
    accepted in under 20 ms.
- [ ] T5. Delete the two backlog entries. Update `progress.md`, `learnings.md`.

## 4. Verification (run exactly)

```bash
export CARGO_PROFILE_DEV_DEBUG=0 CARGO_INCREMENTAL=0
for c in foundation_netio foundation_http foundation_connectrpc foundation_core; do
  python3 scripts/ci/ci.py run $c --profile dev || exit 1
done
cargo check --workspace --all-targets --profile dev
# 20-run flake check for each new test and for foundation_connectrpc conformance_tests.
rg -n 'thread::sleep' backends/foundation_http/src/native/server/mod.rs   # only the error back-off, or a cfg(windows) path
```

## 5. Done when

- [ ] All §4 commands pass; new tests passed 20 runs in a row.
- [ ] Each new test fails on the base code (where it can) and passes now.
- [ ] Client-streaming and bidirectional conformance tests still pass.
- [ ] CI green.

## 6. Review checklist (main session)

- [ ] No lost wake between "pipe empty" and the park.
- [ ] Shutdown wakes the accept wait.
- [ ] The accept back-off on errors still exists and is configuration.
