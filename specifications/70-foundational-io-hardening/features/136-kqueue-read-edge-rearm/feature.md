---
feature: "Registering a read waker re-arms the fd's read edge, so a lost edge needs no caller discipline"
status: "pending"
lane: "priority follow-up (found under feature 129)"
model: "sonnet"
branch: "spec70/priority-followups"
crates: ["foundation_reactor"]
catalogue_rows: ["5b"]
---

# Feature 136: registering a read waker re-arms the read edge

_Written in ASD-STE100 Simplified Technical English (STE). Code and identifiers stay as
they are. Follow [requirements.md](../../requirements.md) sections 3, 4 and 8._

Glossary:

- **edge**: a readiness event that the selector delivers once.
- **re-arm**: to forget the last delivered event, so the next arrival produces a new edge.
- **park**: to stop polling a task until a readiness signal wakes it.
- **latched readiness**: the bits the reactor keeps for an fd until the reader clears them.

## Problem (found under feature 129 on 2026-11-06, backlog §5b)

macOS reports `EVFILT_READ` level-triggered for pipes despite `EV_CLEAR`. The kqueue
selector therefore suppresses repeats itself: it stores each delivered fd's byte count and
delivers a readable event again only when the count grows
(`backends/foundation_reactor/src/poll/sys/unix/selector/kqueue.rs:458-489`). It forgets an
fd only through `Reactor::clear` → `Poll::rearm_readable`.

A socket that drains N bytes and then receives M < N bytes produces no new edge. A task
that registers its waker without clearing the latched readiness never wakes. That is
exactly what happened in `RawStream::read_wake_source`
(`backends/foundation_netio/src/native/raw_stream.rs:489`): the websocket upgrade relay lost
its 15-byte echo after a 77-byte head, and the client timed out. Feature 129 added the
missing `clear()` at that one call site.

The remaining hazard is structural: every readiness user must know the clear-before-register
protocol. One forgotten call is a lost wake on macOS only, so Linux CI stays green. The
protocol is documented but not enforced, and no test covers the selector rule.

## Design

```mermaid
sequenceDiagram
    participant Task as reader task
    participant Readiness as FdReadiness
    participant Reactor as Reactor
    participant Selector as kqueue selector
    Task->>Readiness: register_waker(waker)
    Readiness->>Reactor: register_waker(token, waker)
    Reactor->>Selector: rearm_readable(fd) (new)
    Reactor->>Reactor: store the waker
    Task->>Task: read once more, then park
    Selector->>Reactor: new edge on any later arrival, even if smaller
    Reactor->>Task: wake
```

_One diagram: the re-arm moves into registration, so a caller cannot forget it._

### 1. Re-arm where the waker is registered

`backends/foundation_reactor/src/reactor.rs`:

- `Reactor::register_waker(token, waker)` and `Reactor::register_task_waker(token, waker)`
  (whichever are the real entry points; `SharedReadiness::register` delegates to one of
  them) look up the entry, call `self.poll.rearm_readable(entry.fd)`, then store the waker.
  The existing `Poll::rearm_readable` is a no-op on epoll and io_uring, so the change costs
  nothing off macOS.
- `Reactor::clear` keeps its re-arm for `READABLE`: it is still correct, and harmless to
  repeat.
- Update the protocol docs on `FdReadiness::clear`, `FdReadiness::register_waker` and
  `EventReadiness::register`: clearing is now required only for the **latched flags** (so
  `is_ready` does not spin), not for the edge. State what the reactor does.

Why this is safe: `rearm_readable` forgets the last delivered byte count. The next poll
delivers a readable event for the fd if data is present. A caller that registers, reads,
and drains sees at most one extra poll on a socket that still held data, which is a real
read, not a spin. A caller that registers and parks without reading gets one wake per
arrival, which is the level-triggered behaviour it would have had without the suppression.

### 2. Test the selector rule directly

New `backends/foundation_reactor/tests/read_edge_rearm.rs`, registered in
`tests/mod.rs`. Use a loopback TCP socket pair (the pipe case in the selector comment did
not reproduce with a socket in feature 129's trace, which is the point):

1. `a_smaller_frame_after_a_drain_wakes_a_registered_task`: connect a pair, register a
   `FdReadiness` on the reader fd, read 77 bytes with `recv`, send 15 bytes back from the
   peer, then call `register_waker` followed by one `recv` (would-block) and park. Assert
   the waker fires within 2 s. On the old code the selector suppresses the 15-byte event
   and the waker never fires on macOS.
2. `a_wake_after_a_drain_does_not_repeat_forever`: after the wake and a full drain, assert
   no further wake arrives within 200 ms. This guards the spin risk of the re-arm.
3. `clear_still_disarms_the_latched_readiness`: `clear()` then `ready()` has no `READABLE`
   bit. This guards the second half of the protocol.

Run the test on macOS (where it fails on the old code) and in the Linux image (where it
must pass). Record both results.

## Verify

```bash
cargo fmt --all --check
cargo clippy --profile uat -p foundation_reactor --all-targets --no-deps -- -D warnings
cargo test --profile uat -p foundation_reactor
cargo test --profile uat -p foundation_netio
cargo test --profile uat -p foundation_proxy
scripts/ci/linux.sh foundation_reactor
```

The clippy command reports the crate's pre-existing pedantic debt (backlog §5b, feature
122). Record the count before and after; do not grow it. If the count blocks a clean run,
run clippy without `-D warnings` and state that.

## Done when

- [ ] Registering a read waker re-arms the fd's read edge in the reactor.
- [ ] The docs say clearing is needed for the latched flags, not for the edge.
- [ ] The three tests exist; the first fails on the old code on macOS.
- [ ] `foundation_reactor` is bumped one patch step if the change touches public API, and
      every consumer requirement follows with `Cargo.lock`.
- [ ] The backlog §5b selector entry is deleted.
- [ ] CI green on macOS and in the Linux image.

## Result

2026-11-06. Plan only. Not started. Written after feature 129 found the missing `clear()`
in foundation_netio.
