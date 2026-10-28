---
feature: "Client body reader reaches a Waker-capable readiness source"
status: "pending"
priority: "critical"
depends_on: ["02-h2-h3-server-readiness"]
branch: "fix/io-01-valtron-parking"
crates: ["foundation_netio", "foundation_iogate", "foundation_nativeapis", "foundation_core"]
backlog: "docs/bug-backlog.md §2 The client body reader cannot reach the reactor"
starting_point: "feature 03 §3.2, blocked by the crate cycle netio ← iogate"
---

# Feature 29: Client body reader reaches a Waker-capable readiness source

_Written in ASD-STE100 Simplified Technical English (STE). Code and identifiers stay as they
are._

**Abbreviations in this file.** File descriptor (fd); central processing unit (CPU); pull
request (PR); Hypertext Transfer Protocol (HTTP); Transport Layer Security (TLS).

## 1. Goal

`AsyncSendSafeBody::poll_next` (`foundation_netio/src/shared/client/body_reader.rs`) can park a
plain TCP or Unix socket without holding a worker and without spinning. It must get a
`std::task::Waker`-capable readiness source inside `foundation_netio`.

Feature 03 §3.2 already designed the source. This feature removes the one blocker: the reactor
that can wake a `std::task::Waker` is not reachable from `foundation_netio`.

## 2. The blocker (evidence)

The client body is built inside `foundation_netio`:

- `foundation_netio/src/shared/http/impls.rs`, `HttpReadState::Body` (around line 4651) turns
  `self.bodies` into a `SendSafeBody`.
- `foundation_netio/src/http/api.rs`, `send_async` (around line 499) returns the connection and
  the body together.

The connection's readiness for a kernel socket is poll-based, not reactor-backed:

- `foundation_netio/src/native/connection.rs`, `readiness` (around line 1170) returns
  `SocketReadiness` for `Tcp` and `Unix`. Its doc says: "For kernel sockets this is a
  zero-timeout `poll(2)` on the fd." `SocketReadiness`
  (`foundation_netio/src/native/readiness.rs`, line 60) is an `EventReadiness`, not a
  `std::task::Waker` source. It cannot wake a `std::task::Waker`.

The reactor that could wake a `std::task::Waker` is `FdReadiness`
(`foundation_iogate/src/native/fd_readiness.rs`, from feature 02). It is not reachable from
`foundation_netio` because of the crate graph:

- `foundation_nativeapis` → `foundation_db` → `foundation_netio`
- `foundation_iogate` → `{ foundation_netio, foundation_nativeapis }`

So `foundation_netio` cannot depend on `foundation_iogate` or `foundation_nativeapis` without
closing a cycle. `foundation_iogate` sits above `foundation_netio` for exactly this reason (see
its `lib.rs` header).

## 3. Design options

Pick one. Each keeps the crate graph acyclic and does not invent a global registry.

### Option A: Move the reactor below netio (recommended)

Move the shared reactor (and `SharedReadiness`, `Interest`) from `foundation_nativeapis` into a
crate that `foundation_netio` may depend on: either `foundation_core` or a new
`foundation_reactor`. Then `FdReadiness` (or a netio copy of it) can live in `foundation_netio`
or the new crate, and the client body reader can register a `std::task::Waker`.

- Pros: one reactor for the whole tree; matches feature 02 and 28, which already use
  `FdReadiness`; no duplicate machinery.
- Cons: a wide refactor. Every crate that imports the reactor path
  (`foundation_nativeapis::native::fd::{Reactor, Ready, SharedReadiness}`) must move to the new
  path. `foundation_iogate` and `foundation_nativeapis` both change.

### Option B: A small poll reactor inside `foundation_netio`

Add a netio-side readiness source that registers fds and wakes `std::task::Waker`s on
readability, using the same "register, then check, then clear" pattern as
`FdReadiness`.

- Pros: localized to netio; no dependency-graph change.
- Cons: a second reactor beside the nativeapis one (rule 11 discourages a parallel system);
  netio grows a background thread.

### Option C: Drive the async body through valtron `Depends`

Stop making the async body a `futures_core::Stream` that returns `Pending`. Instead expose the
body as a valtron task that returns `TaskStatus::Depends(SocketReadiness)` on `WouldBlock`, and
have async callers `execute(..).into_ready_future().await`. `SocketReadiness` is already an
`EventReadiness`.

- Pros: reuses feature 01 and 02 machinery; no new reactor; no `std::task::Waker` needed.
- Cons: changes the consumer API for every async body read (`collect_bytes_async`,
  `collect_string_async`, deployment providers, `foundation_db`, `foundation_ai`, connectrpc);
  feature 03's `ReadWakeSource` design is dropped.

## 4. Tasks

- [ ] T1. Pick an option with the main session. Record the choice here.
- [ ] T2. Implement the readiness source in the chosen crate.
- [ ] T3. Add `FdReadiness`-style `register_waker(&std::task::Waker)` (or the chosen equivalent)
      with the register-then-check-then-clear pattern.
- [ ] T4. Update feature 03 §3.2 to point at the new source, then finish feature 03 T3–T6.
- [ ] T5. Update `progress.md`, `learnings.md`. Delete the §2 backlog entry.

## 5. Verification (run exactly)

```bash
export CARGO_PROFILE_DEV_DEBUG=0 CARGO_INCREMENTAL=0
cargo check --workspace --all-targets --profile dev
for c in foundation_core foundation_netio foundation_iogate foundation_nativeapis foundation_db foundation_connectrpc; do
  python3 scripts/ci/ci.py run $c --profile dev || exit 1
done
```

Then the feature 03 verification (`async_body_waits_without_spinning` poll count under 10, CPU
under 50 ms).

## 6. Done when

- [ ] A plain-TCP async body read parks on readiness, not on self-wake.
- [ ] No crate cycle forms; `cargo check --workspace` is green.
- [ ] Feature 03 T3–T6 pass.
