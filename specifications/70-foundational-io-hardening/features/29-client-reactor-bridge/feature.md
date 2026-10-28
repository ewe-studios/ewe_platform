---
feature: "Client body reader reaches a Waker-capable readiness source"
status: "pending"
priority: "critical"
depends_on: ["02-h2-h3-server-readiness"]
branch: "fix/io-01-valtron-parking"
crates: ["foundation_reactor", "foundation_netio", "foundation_nativeapis", "foundation_iogate"]
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

## 3. Decision

Option A, in a new crate **`foundation_reactor`** (chosen by the main session). The whole
readiness layer moves below `foundation_netio`, so netio can use `FdReadiness` directly and no
cycle forms.

The moved code is self-contained. It imports only `foundation_core` (valtron
`EventReadiness`, `request_executor_resume`), `libc`, and `tracing` — never `foundation_db`,
`foundation_arrow`, or `foundation_compact`. The `poll` module is 5 286 lines (epoll, kqueue,
io_uring readiness and completion selectors).

## 4. What moves into `foundation_reactor`

| Source (today) | Target (foundation_reactor) |
|---|---|
| `foundation_nativeapis/src/native/poll/` (whole tree) | `src/poll/` |
| `Ready` in `foundation_nativeapis/src/native/fd/mod.rs` (lines 80–183) | `src/ready.rs` |
| `foundation_nativeapis/src/native/fd/reactor.rs` (`Reactor`, `SharedReadiness`, `AlreadyInitialised`, `drain_loop`, `Entry`) | `src/reactor.rs` |
| `foundation_iogate/src/native/fd_readiness.rs` (`FdReadiness`) | `src/fd_readiness.rs` |
| `next_token` in `foundation_iogate/src/native/accept.rs` (lines 31–52) | `src/token.rs` |

`foundation_reactor` depends on `foundation_core`, `libc`, `tracing` only.

## 5. What stays and re-exports

- `foundation_nativeapis` gains a `foundation_reactor` dependency. `native/mod.rs` changes
  `pub mod poll;` to `pub use foundation_reactor::poll;`. `native/fd/mod.rs` re-exports
  `Ready` and `reactor::{Reactor, SharedReadiness, AlreadyInitialised}` from the new crate, so
  every existing `foundation_nativeapis::{Poll, Token, Interest, Events, Registry}` and
  `native::fd::{Reactor, Ready, SharedReadiness}` path keeps working.
- The io_uring completion layer (`fd/completion.rs`, `fd/send_pool.rs`, `fd/mod.rs`
  `FdRegistration`/`RegisteredFd`) stays in `foundation_nativeapis`. It references the moved
  `poll` types through the re-export.
- `foundation_iogate` re-exports `FdReadiness` from `foundation_reactor` (its public path stays
  `foundation_iogate::native::FdReadiness`). Its `accept` module uses `foundation_reactor`'s
  token counter.
- `foundation_netio` gains a `foundation_reactor` dependency and uses `FdReadiness`.

## 6. The new Waker capability

`foundation_reactor`'s `FdReadiness` (or `Reactor`) gains:

```rust
pub fn register_waker(&self, waker: &std::task::Waker) -> bool;
```

It stores the waker with the token and the drain thread calls `waker.wake_by_ref()` after it
latches readiness for that token. Register-before-check applies: the caller registers the
waker, then reads once more, then parks. Clearing latched readiness before the next read
removes the waker's token so a stale edge does not re-wake forever.

## 7. Tasks

- [ ] T1. Create the `foundation_reactor` crate. Move the §4 code. Fix `crate::` paths.
- [ ] T2. Add the `foundation_reactor` dependency and §5 re-export shims to
      `foundation_nativeapis` and `foundation_iogate`. Build until `cargo check --workspace`
      is green.
- [ ] T3. Add `register_waker` (§6) with the register-then-check-then-clear pattern.
- [ ] T4. Wire it into feature 03: build the `ReadWakeSource` from `FdReadiness` in
      `foundation_netio` and attach it at every body construction site.
- [ ] T5. Finish feature 03 T5–T6 (regression tests, delete the two §2 backlog entries).
- [ ] T6. Update `progress.md`, `learnings.md`. Delete the §2 reactor-bridge backlog entry.

## 8. Verification (run exactly)

```bash
export CARGO_PROFILE_DEV_DEBUG=0 CARGO_INCREMENTAL=0
cargo check --workspace --all-targets --profile dev
for c in foundation_reactor foundation_netio foundation_nativeapis foundation_iogate foundation_db foundation_connectrpc; do
  python3 scripts/ci/ci.py run $c --profile dev || exit 1
done
```

Then the feature 03 verification (`async_body_waits_without_spinning` poll count under 10, CPU
under 50 ms).

## 9. Done when

- [ ] A plain-TCP async body read parks on readiness, not on self-wake.
- [ ] No crate cycle forms; `cargo check --workspace` is green.
- [ ] Feature 03 T3–T6 pass.
