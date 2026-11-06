---
feature: "A dropped stream facade cancels its reader and writer tasks"
status: "pending"
lane: "priority follow-up (found by feature 124)"
model: "sonnet"
branch: "spec70/f131-stream-drop-cancel"
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

- [ ] A dropped facade closes its socket; the peer observes the close.
- [ ] Splitting a bidi stream does not cancel it under the surviving half.
- [ ] A timeout stops the reader and writer tasks, not only the caller.
- [ ] The backlog §1 entry is deleted.
- [ ] CI green.

## Result

2026-11-06. Plan only. Not started. Feature 124 recorded the leak while it bounded the
facade waits.
