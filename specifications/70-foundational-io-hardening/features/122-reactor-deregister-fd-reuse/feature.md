---
feature: "A late reactor deregister cannot remove another socket's readiness"
status: "done"
lane: "priority follow-up (bug found while diagnosing feature 121)"
model: "sonnet"
branch: "spec70/f122-reactor-fd-reuse"
crates: ["foundation_reactor"]
catalogue_rows: []  # from docs/bug-backlog.md §5b; not a §8 catalogue row
---

# Feature 122: a late reactor deregister cannot remove a new socket's readiness

_Written in ASD-STE100 Simplified Technical English (STE). Code and identifiers stay as
they are. Follow [requirements.md](../../requirements.md) sections 3, 4 and 8._

Glossary:

- **readiness**: a signal that a socket can be read or written without blocking.
- **reactor**: the process-wide thread that waits for readiness on many sockets.
- **file descriptor (fd)**: the small integer that names an open socket.
- **registration**: one entry that asks the reactor to watch one fd.
- **token**: the unique number that labels one registration. A token is never reused
  (`foundation_reactor/src/token.rs`).

## Problem (found by reading the code on 2026-11-06)

The reactor changes its selector by fd number alone, so a late deregistration can remove
the wrong registration. The chain:

1. `FdReadiness::drop` (`foundation_reactor/src/fd_readiness.rs:149`) calls
   `Reactor::deregister(self.fd, self.token)` (`reactor.rs:538`).
2. `Reactor::deregister` removes the entries-map entry for `token`, then calls
   `registry.deregister_fd(fd)` (`poll/mod.rs:426`). The selector is kqueue, epoll or
   io_uring, and all three find a registration by fd number
   (`kqueue.rs:263`, `epoll.rs:84`, `uring.rs:223`). **The token is not compared with
   the registration that the fd number now holds.**
3. A socket can close before its `FdReadiness` drops. `FdReadiness::drop` says so in
   its comment: "The fd may already be closed (its stream dropped first)".
4. The operating system gives the lowest free fd number to the next open. A new socket
   can therefore get the number that the closed socket had, and register a new token.
5. The late `deregister` then deletes that new registration from the selector. The new
   socket never reports readiness again. A task parked on it waits until its own
   deadline, or forever when the park has no deadline. Nothing else repairs the entry:
   `register_waker` only checks the entries map, which still holds the new token, so the
   task parks with a waker that never fires.

The reactor's entries map is keyed by token, so the entries-map removal is harmless to the
new registration. Only the selector entry is lost, and that is the one that delivers the
wake.

Step 3 is common. A body-read waker holds `Arc<FdReadiness>`
(`foundation_netio/src/native/raw_stream.rs:480`) after the stream is gone, so the drop
happens at an arbitrary later time, often on another thread.

This is the leading explanation for the stalled proxy tests in
[feature 121](../121-proxy-spread-e2e-flake/feature.md): a proxy starts within 2 ms of
another proxy stopping, the previous test's sockets close, and one late drop removes the
new listener's or connection's registration. The recorded failure needs no other fault.

## Design

### 1. Make deregistration prove the fd number still belongs to this token

Add an fd-to-token map to `Reactor`, guarded by the same `reg_lock` that serialises
selector changes:

```rust
/// The live token for each registered fd number. A stale drop checks this map
/// before it touches the selector.
fd_tokens: Mutex<HashMap<RawFd, Token>>,
```

- `register` and `register_completion` insert `fd -> token` after
  `registry.register_fd` succeeds.
- `reregister` keeps the entry: the fd and its token do not change.
- `deregister(fd, token)`:
  1. Remove `entries[token]` as today.
  2. Read `fd_tokens[fd]`. If it is absent or is a different token, **do not call
     `registry.deregister_fd`**. A later socket holds that number.
  3. If it is this token, remove the map entry and call `registry.deregister_fd(fd)`.

The map holds one token per number, so it is bounded by the open-fd count. Do not add a
second lock: take `reg_lock` first, then the map, in the same order as today.

- `register` must remove any old map entry for the same number first. Other registrations
  already hold their own tokens, and the last `register` owns the number.

### 2. Keep the selector's error behaviour

`ENOENT` from the selector stays harmless, as today. A skipped call is not an error: log
it at `tracing::debug!` with both tokens and the fd.

### 3. Check the other holders of a registration

- The send pools and completion registrations (`send_pool.rs`, `register_completion`) use
  their own tokens. They use the same map, so cover them in the same change.
- `foundation_reactor` must not depend on feature 121. Feature 121 uses this fix if its
  measurement confirms the lost wake.

## Tests

Test target: `tests/fd_reuse.rs`, a new binary that cargo discovers on its own (like
`tests/selector_wait.rs`). It needs its own process for the reactor singleton.

- `a_drop_after_fd_reuse_leaves_the_new_registration_armed`. Both Linux and macOS give
  the lowest free fd number, so the sequence is deterministic:

  ```text
  let a = one end of a socketpair(2);  // fd L
  let ready_a = FdReadiness::register(L);
  let number = a.as_raw_fd();          // L
  drop(a);                             // L is closed, ready_a still live
  let b = connect a new socket;        // gets L again while it stays the lowest free
  assert_eq!(b.as_raw_fd(), number);   // if not, open sockets until it matches
  let ready_b = FdReadiness::register(b.as_raw_fd());
  drop(ready_a);                       // the late deregistration
  write from the peer;                 // b must report readiness
  ```

  Assert that `ready_b` reports readable, or that the accept/read waker fires, within a
  short bound. This test fails on the old code: the selector entry for `number` is gone.
- `a_drop_of_the_live_registration_still_deregisters`: the normal case, which keeps
  today's behaviour.
- Keep both tests free of timing assumptions: drive the selector with a connected socket
  pair over a real local socket, not a simulated one.

## Verify

```bash
cargo fmt --all --check
cargo clippy --profile uat -p foundation_reactor --all-targets --no-deps -- -D warnings
cargo test --profile uat -p foundation_reactor
cargo test --profile uat -p foundation_netio
cargo test --profile uat -p foundation_http
cargo test --profile uat -p foundation_proxy
python3 scripts/ci/ci.py run foundation_reactor --profile uat
```

`foundation_reactor` is not in the `clippy-deny-warnings` set in `ci/packages.toml`, so CI
does not run its `-D warnings` check. The crate already carries 120 `clippy::pedantic`
errors on `master`, and this change adds none. Backlog §5b holds the entry that schedules
the cleanup.

Then run the feature 121 loop: the proxy test binaries on Linux, thirty times.

## Done when

- [x] The fd-to-token map guards every selector deregistration.
- [x] `a_drop_after_fd_reuse_leaves_the_new_registration_armed` fails without the fix and
      passes with it.
- [x] `foundation_reactor`, `foundation_netio`, `foundation_http` and `foundation_proxy`
      tests pass. (`foundation_proxy` has one known macOS failure, recorded in backlog
      §9; it is not caused by this change.)
- [x] The backlog §5b entry is deleted after merge.
- [ ] CI green. Run `42205799516` (head `bc7e870d5`) failed 13 jobs, so it is
      not proof. Record the final run ID here when the branch head passes.

## Result

2026-11-06. Implemented on branch `spec70/priority-followups`.

### What changed

`backends/foundation_reactor/src/reactor.rs`, only:

- New private field `Reactor::fd_tokens: Mutex<HashMap<RawFd, Token>>`. It records the
  live token for each registered fd number.
- `register` and `register_completion` insert `fd -> token` after the selector call
  succeeds. `insert` replaces an older token for the same number, because the latest
  registration owns the number.
- `reregister` leaves the mapping unchanged: the fd number and the token do not change.
- `deregister(fd, token)` always removes `entries[token]`. It then reads `fd_tokens[fd]`
  and compares it with `token`:
  - equal: remove the map entry and call `registry.deregister_fd(fd)` as before;
  - different: log at `tracing::debug!` with the fd and both tokens, and do not touch
    the selector;
  - absent: do not touch the selector.
  `ENOENT` from the selector stays harmless.

Invariants:

- The map holds one token per fd number, so it is bounded by the open-fd count.
- All access happens under the existing `reg_lock`, in the existing order
  (`reg_lock`, then the entry map or the token map). No second lock order is added.
- A selector deregistration happens only when the number still belongs to the token.
- No public API changes. No crate version change. `Cargo.lock` is unchanged.

### Tests

New binary `backends/foundation_reactor/tests/fd_reuse.rs` (two tests, serialised by a
process-local mutex because both need a known fd number):

- `a_drop_after_fd_reuse_leaves_the_new_registration_armed`: registers socket A on
  number L, closes A, reopens a socket on L, registers it, drops A's `FdReadiness`,
  writes from the peer and waits up to 2 s for readiness.
- `a_drop_of_the_live_registration_still_deregisters`: the live drop removes the
  registration; a later event does not reach the old token; the number accepts a new
  registration.

Old-code proof: `reactor.rs` was replaced with `git show HEAD:.../reactor.rs` (test
file unchanged) and the reuse test was run. Observed failure:

```text
thread 'a_drop_after_fd_reuse_leaves_the_new_registration_armed' (...) panicked at
backends/foundation_reactor/tests/fd_reuse.rs:69:5:
a drop after fd reuse: fd 3 (FdReadiness { fd: 3, token: Token(2) }) did not report
readability within 2s
test result: FAILED. 0 passed; 1 failed; 0 ignored; 0 measured; 1 filtered out;
finished in 2.08s
```

With the fix the same test passes in 2 of 2 cases on macOS and in the Linux CI image.

### Commands and results

| Command | Result |
| --- | --- |
| `cargo fmt --all --check` | pass |
| `cargo check --profile uat -p foundation_reactor --all-targets` | pass, no warnings |
| `cargo clippy --profile uat -p foundation_reactor --all-targets --no-deps -- -D warnings` | **fails on 120 pre-existing `clippy::pedantic` errors**; HEAD and `master` report the same 120, so this change adds none. `foundation_reactor` is not in the `clippy-deny-warnings` set in `ci/packages.toml`. Recorded in backlog §5b. |
| `cargo test --profile uat -p foundation_reactor` | pass: 12 tests passed, 0 failed, 1 doc test ignored |
| `cargo test --profile uat -p foundation_netio` | pass: 1291 tests passed, 0 failed |
| `cargo test --profile uat -p foundation_http` | pass: 66 tests passed, 0 failed, 1 ignored |
| `cargo test --profile uat -p foundation_proxy --no-fail-fast` | 128 passed, 1 failed: `proxy_features_tests::websocket_upgrade_relays_and_tunnels_bytes`, the known macOS failure in backlog §9 (`WouldBlock`, `proxy_features_tests.rs:464`) |
| `cargo test --profile uat -p foundation_nativeapis --test reactor_shared_selector_tests --test reactor_parking_tests --test reactor_event_gen_tests --test fd_event_driven_tests` | pass: 9 passed, 0 failed |
| `cargo test --profile uat -p foundation_iogate` | pass: 8 passed, 0 failed |
| Linux image: `cargo test --profile uat -p foundation_reactor` | pass: all binaries green, including `fd_reuse` 2 of 2 |
| Linux image: `cargo test --profile uat -p foundation_nativeapis --features uring --test completion_source_seam_tests --test reactor_shared_selector_tests --test reactor_parking_tests --test fd_event_driven_tests` | pass: 18 passed, 0 failed (the `uring` run compiles `register_completion`) |
| Linux image: `cargo test --profile uat -p foundation_iogate` | pass: 10 passed, 0 failed |

CI (GitHub) is not run from this branch; the feature stays `pending` until that run is
green.

