# Learnings — medium-severity defect fixes

_Written in ASD-STE100 Simplified Technical English (STE)._

_Newest first. Updated after each batch. See
`.agents/skills/specifications-management/skill.md`._

## Batch 2 (2026-11-09)

### A fix with no regression test lets the tracker go stale

- Seven of the nine issues in this batch were already fixed at `4ca007301`. The open
  tracker did not show that. Each fix had its commit, but only some had a test that names
  the defect.
- Before a fix is called done, run its regression test against the code without the fix.
  Record that result in the commit or the issue. A tracker entry then has a fact behind it.
- #291 had no direct test at first. The bridge test that showed the defect
  (`fast_producers_never_strand_a_consumer`) timed out at 60 s on the old code and takes
  under 2 s with the fix. That test is the regression test.

### A `Waker` built from a non-`Send` callback must stay on one thread

- `Waker` is `Send` and `Sync` by type, but `Waker::from_raw` expects thread-safe data.
  A non-`Send` callback inside the data breaks that promise if the waker crosses a thread.
- The non-`multi` build has no `Send` bound on `WakeCallback`. The single-threaded executor
  owns the waker, so the callback stays on that one thread. The type must not move.
- Use `#[cfg_attr(not(feature = "multi"), expect(clippy::arc_with_non_send_sync, ...))]`
  and state the single-thread rule in the doc comment.

### A no-progress requeue is not urgent work

- `TaskStatus::Ignore` maps to `State::Pending(None)`. The executor pushed that entry to the
  front of its queue, so a task that self-wakes and makes no progress ran again before any
  other runnable task. One such task could starve its worker.
- A requeue with no progress goes to the back of the queue, like other runnable work. Do not
  add a fairness counter until a real case needs one.

### Test a queue-order defect with a bounded number of ticks

- The no-progress task never ends, so `run_until(NoWork)` cannot drive the test.
- Schedule the no-progress task first and a finite sibling second. Run a fixed number of
  `run_once` ticks and assert that the sibling finished. The old order never runs the
  sibling. The new order gives it a turn after every no-progress poll.

## Batch 1 (2026-11-09)

### One reactor registration per fd, not one per token (issue #421)

- The reactor keys its selector registration by file descriptor (fd). `Reactor::register`
  overwrites the fd-to-token entry. `Reactor::deregister(fd, token)` removes the selector
  filters only when `token` is the live token, but it still drops the entry of the fd.
- The HTTP idle handler cached one `FdReadiness` for the life of a connection. The RPC
  connection I/O and the response writer also register the same fd, then drop their
  registration. Such a drop removes the fd from the selector. The handler then parked on an
  fd that nothing watched, and the next request never woke it.
- The fix registers the fd at each park and lets the registration drop when the task runs
  again. The live registration then always belongs to the parked task.
- The long-term shape is one `FdReadiness` per fd, owned by `RawStream` and shared by every
  user. That is a larger change; the current fix keeps the invariant with a short lifetime.
- The failure was invisible per branch. The `foundation_http` parking test passed alone, and
  the `foundation_connectrpc` keep-alive test passed against master's `foundation_http`.
  Only the merged tree failed. After each merge, run the tests of every crate that depends
  on the changed crate.

### A task must reuse one waker across polls (issue #329)

- `ReadinessNotifier` removes duplicate registrations by waker identity. A task that builds
  a fresh waker on every poll defeats that removal, so the registration list grows with the
  poll count.
- The fix gives each future and each state task one stable waker for all its polls. The
  split observers register a one-shot callback waker on their queue
  (`ReadinessWaker::from_callback`), so they park until the producer pushes a value.

### A non-blocking connect and a TLS handshake are state machines (issues #277, #279, #418)

- `connect(2)` and a TLS handshake can block a valtron worker. Start both on a non-blocking
  socket, then finish them from socket readiness. A valtron task parks with
  `TaskStatus::Depends`; a synchronous boundary uses `SocketReadiness::wait_blocking` with a
  clock-bounded deadline.
- Each TLS backend (rustls, openssl, native-tls) reports the direction its handshake waits
  on. One task drives all three.
- The openssl and native-tls backends did not compile before this change. A feature that CI
  never builds hides this class of defect. Add every backend to a CI run.

### The clippy gate must cover the feature that holds the code (issues #386, #387, #419)

- `foundation_netio` had 74 pedantic warnings in code that only compiles with `quic`. The
  default clippy run never saw them.
- A `clippy-deny-warnings = true` entry fails CI only when a `clippy-runs` entry compiles the
  affected code. Add both the feature run and the deny entry together.

### A target-only unused dependency is a stale dependency (foundation_http)

- `foundation_http` declared `getrandom` for wasm32 and enabled `getrandom/js`. `uuid` 1.20
  takes its wasm randomness from `crypto.getRandomValues` through `js-sys`, and its
  `getrandom` dependency is 0.3 and is target-gated off on wasm32-unknown-unknown. The
  declared 0.2 dependency was dead.
- A warning that appears on one target only can mean that the upstream crate moved its
  backend or changed its major version. Read the upstream features before you keep the
  dependency.

### Version lines conflict on every merge; keep the highest version

- Each branch changed a crate version and its consumer requirements. The merges conflicted
  on the same manifest and lock-file lines. Resolve with one rule: keep the highest version
  of every crate, line by line. Then run `cargo metadata --offline` for the workspace and
  each example lock file.

### Restore a path from `HEAD`, not from the index

- `git checkout <rev> -- <path>` writes the file into the index as well as the worktree.
  A later `git checkout -- <path>` restores from the index, not from `HEAD`, so the old
  tree comes back.
- Use `git restore --source=HEAD --staged --worktree -- <path>` to revert a file fully.

### The `shared/` tree is target-agnostic (skills change, `bac23cf3c`)

- Nothing in `shared/` is "wasm unsafe". A `cfg` that deletes a code path on wasm means the
  code is native-only and belongs in `native/`. Do not gate it.
- Issues #409, #411, #412 and #413 record the remaining places to move.
