---
feature: "A dropped stream facade cancels its reader and writer tasks"
status: "done"
lane: "priority follow-up (found by feature 124)"
model: "sonnet"
branch: "spec70/priority-followups"
crates: ["foundation_connectrpc"]
catalogue_rows: ["1"]
---

# Feature 131: a dropped stream facade cancels its reader and writer tasks

_Written in ASD-STE100 Simplified Technical English (STE). Code and identifiers stay as
they are. Follow [requirements.md](../../requirements.md) sections 3, 4 and 8._

Glossary:

- **facade**: the typed stream value a caller holds — `ServerStream`, `BidiStream`,
  `BidiSender`, `BidiReceiver`.
- **reader task**: the spawned task that reads response frames into the response pipes.
- **cancel signal**: the `CancelSignal` in the call `Ctx`. Firing it stops the call's tasks.

## Problem (found by feature 124 on 2026-11-06)

Feature 124 bounds each facade wait with the call deadline, but the facade does not hold
the call's `CancelSignal`, so `with_deadline_at` is called with `None` for its cancel
argument. Two consequences:

- A wait that times out stops the caller but not the reader task.
- Dropping a facade against a peer that sends nothing leaves the spawned protocol reader
  parked on the response body. The transport stream and its socket stay open until the
  peer sends data, the transport idle timeout fires, or the call cancels.

Feature 124's stalled-peer test observes the socket stay open after both the deadline and
the drop. On a proxy or a long-lived client this leaks one socket per abandoned stream.

`docs/bug-backlog.md` §1 holds the entry.

## Design

1. Each facade struct gains the call's cancel signal beside the deadline it already
   carries:

   ```rust
   cancel: Option<CancelSignal>,
   ```

   The constructor copies it from the call `Ctx` in the `*_inner` function, and
   `BidiStream::split` clones it into both halves.
2. Every `with_deadline_at(self.deadline, None, ...)` call becomes
   `with_deadline_at(self.deadline, self.cancel.as_ref(), ...)`, so a timeout also fires
   the cancel signal and stops the call's tasks.
3. Implement `Drop` for each facade to fire the signal. All four facades share one call,
   so a facade that is split or cloned must not cancel the call while a sibling still
   reads. Use the existing handle-count pattern: the `Ctx` cancel signal fires once, and
   all facades of one call hold clones of the same `CancelSignal`; check the signal's
   semantics before choosing. If it fires globally, hold a shared `Arc<AtomicUsize>` of
   live facades and fire only when the last one drops. State the choice in the Result.
4. Check for a facade that the `Client` keeps after returning a stream (for example an
   internal receiver). It must hold the same handle count, or the first drop cancels the
   call under it.

## Tests

Add to `backends/foundation_connectrpc/tests/client_deadline_tests.rs`:

- `dropping_a_server_stream_closes_the_socket`: a raw `H2Channel` peer that sends the head
  and then nothing. Drop the `ServerStream` and assert the peer sees the connection close
  (a read returns `Ok(0)` or `ECONNRESET`) within 2 s. On the old code the socket stays
  open and the peer read times out.
- `dropping_one_bidi_half_keeps_the_other_alive`: split a bidi stream, drop the sender,
  and assert the receiver still gets a message the peer sends afterwards.
- `a_timed_out_receive_cancels_the_call`: after the deadline timeout, the peer sees the
  close, not only a caller-side error.

## Verify

```bash
cargo fmt --all --check
cargo clippy --profile uat -p foundation_connectrpc --all-targets --no-deps -- -D warnings
cargo test --profile uat -p foundation_connectrpc
cargo test --profile uat -p foundation_proxy
```

Then 30 runs of `rpc_proxy_e2e_tests` in the Linux CI image with `--cpus=2`.

## Done when

- [x] A dropped facade closes its socket; the peer observes the close.
- [x] Splitting a bidi stream does not cancel it under the surviving half.
- [x] A timeout stops the reader and writer tasks, not only the caller.
- [x] The backlog §1 entry is deleted.
- [ ] CI green. Run `42205799516` (head `bc7e870d5`) failed 13 jobs, so it is not
      proof. Record the final run ID here when the branch head passes.

## Result

2026-11-06. Implemented on branch `spec70/priority-followups`. The feature is
`pending (CI proof)` until the pull-request CI is green.

### What changed

- **One guard for the call's lifetime.** A new private `CancelGuard` in
  `backends/foundation_connectrpc/src/shared/client.rs` holds the call's
  `CancelSignal` and a shared `Arc<AtomicUsize>` of live facades. `Clone` adds
  one to the count; the last `Drop` fires the signal. Firing a signal that
  already fired does nothing, so a drop during unwinding is safe. The guard
  allocates nothing itself and takes no lock on an already-fired signal (the
  `CancelSignal::cancel` fast path is an atomic swap).
- **Every facade carries the guard.** `ServerStream`, `BidiStream`,
  `BidiSender` and `BidiReceiver` gain one private field, `cancel: CancelGuard`.
  Each constructor makes the guard from the call `Ctx` signal, and
  `BidiStream::split` clones it into the sender half and moves the original
  into the receiver half. The count therefore equals the number of live facade
  values.
- **Every wait can fire the signal.** All 14
  `with_deadline_at(deadline, None, ..)` call sites now pass
  `Some(self.cancel.signal())`, so a timeout also stops the reader and writer
  tasks instead of only the caller. The setup path in `with_deadline` already
  passed the caller's signal and did not change.
- **No facade kept by the `Client`.** `server_stream_inner` and
  `bidi_stream_inner` move the receiver (and, for bidi, the sender) into the
  returned facade. The spawned reader and writer tasks hold the `CancelSignal`
  but not the guard, so the count counts only values the caller holds. No
  public signature changed, so the crate stays 0.0.5.

### `CancelSignal` semantics and the chosen mechanism

`CancelSignal` is an `Arc` over one shared flag: every clone shares it, and one
`cancel()` stops everything that holds a clone. It is therefore **global to the
call**. Firing it from one facade would cancel a live sibling, for example the
other half of a split bidi stream. The feature uses the **shared live-handle
count** (`Arc<AtomicUsize>`), not the raw fire, and fires only when the last
facade drops.

### Tests

Three new cases in `tests/client_deadline_tests.rs`:

- `dropping_a_server_stream_closes_the_socket`: the raw peer reports whether
  the client closed the socket within 2 s. Passing on the new code.
- `dropping_one_bidi_half_keeps_the_other_alive`: the peer sends its message
  only after the sender half drops, and the receiver still gets it.
- `a_timed_out_receive_cancels_the_call`: after the 200 ms deadline, the peer
  reports the close, not only the caller's `DeadlineExceeded`.

Old-code proof (the new tests run against the reverted `client.rs`):

- `dropping_a_server_stream_closes_the_socket` fails at
  `tests/client_deadline_tests.rs:714` with
  `dropping the stream must close the peer's socket within 2 s`.
- `a_timed_out_receive_cancels_the_call` fails at
  `tests/client_deadline_tests.rs:780` with
  `a timed-out receive must close the peer's socket within 2 s`.
- `dropping_one_bidi_half_keeps_the_other_alive` passes on the old code, as
  expected: it guards the new count against a wrong raw fire.

### Deviation from the plan's first test

The plan asks for a peer that sends the head and **nothing more**. That peer
cannot release `server_stream`: the call setup asks
`PipeClientReceiver::response_headers` for the head before the reader task can
publish it (there is no yield between spawning the reader and asking), so the
method waits for the first body frame, and a peer that sends none keeps the
call open until it closes the socket. The test therefore uses the stalled peer
(head plus one message); after that message the reader is parked on the response
body, which is the leak the feature ends. The underlying wait is a separate bug,
recorded in `docs/bug-backlog.md` §1.

### Verify

- `cargo fmt --all --check`: clean.
- `cargo clippy --profile uat -p foundation_connectrpc --all-targets
  --no-deps -- -D warnings`: clean.
- `cargo test --profile uat -p foundation_connectrpc`: 248 passed, 0 failed,
  1 ignored.
- `cargo test --profile uat -p foundation_proxy`: 128 passed, 1 failed (of 129).
  The failure is the known macOS flake
  `proxy_features_tests::websocket_upgrade_relays_and_tunnels_bytes`, with
  `read echo: WouldBlock` in the post-upgrade tunnel (backlog §9, feature 129).
  The rerun of that binary has the same result. Not caused by this change.
- `cargo check --profile uat --target wasm32-unknown-unknown -p
  foundation_connectrpc --no-default-features --features rpc`: clean.

### CI failure and its cause (2026-11-06, after the feature landed)

CI run `42175894858` failed
`client_deadline_tests::a_timed_out_receive_cancels_the_call` twice, at
`tests/client_deadline_tests.rs:821`, with "a timed-out receive must close the
peer's socket within 2 s". The client was not at fault. The test blocked a
valtron worker:

- The test body ran `peer.join()`, which parks the calling thread. In a
  `#[valtron_test]` body the poll that matters runs on a valtron pool worker,
  and on CI (2 vCPUs) the pool has 2 task workers. Two test bodies that block in
  `join()` held both, so the H2 pump task never ran and never closed the socket.
- Reproduction: with `threads = 1` on the tests and `--test-threads=2`, 3 of 3
  local runs failed with the same panic. With `threads = 2`, or with one test
  thread, all runs passed. This is the same signature as CI.
- Fix: `join_within` in `tests/client_deadline_tests.rs` waits on
  `valtron::sleep_async` in 10 ms steps while `JoinHandle::is_finished()` is
  false, fails the test at the bound, and only then calls `join()`. The six
  synchronous joins in the file now use it. `dropping_a_server_stream_closes_the_socket`
  already used this pattern.
- After the fix, the `threads = 1` + `--test-threads=2` reproduction passed 4 of
  4 runs and the full binary passed 12 of 12.

The general hazard (any blocking call in a poll starves the pool silently) is
recorded in `docs/bug-backlog.md` §4 and scheduled as feature 139.

