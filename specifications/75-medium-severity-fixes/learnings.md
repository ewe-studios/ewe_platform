# Learnings — medium-severity defect fixes

_Written in ASD-STE100 Simplified Technical English (STE)._

_Newest first. Updated after each batch. See
`.agents/skills/specifications-management/skill.md`._

## Batch 2 (2026-11-09)

### A `macro_rules!` cannot emit a match arm; a cfg alias can carry the rule

- A macro call in match-arm position is parsed as a pattern. The compiler reports "expected
  pattern, found `#`" and "this macro call doesn't expand to a pattern". One macro cannot emit
  `pattern => body` into an enclosing `match`.
- A named cfg alias keeps one rule behind many arms. Add a `build.rs` that prints
  `cargo::rustc-check-cfg=cfg(<name>)` and, when the rule holds, `cargo::rustc-cfg=<name>`.
  This repository already does this for `cranelift_backend` in `foundation_buildtools`.
- Build scripts see `CARGO_CFG_TARGET_FAMILY` and `CARGO_CFG_TARGET_OS`. `wasm32-wasip1` and
  `wasm32-wasip2` both have `target_family = "wasm"` and `target_os = "wasi"`. WASI preopens
  give them `std::fs`.
- Check the build script's choice by reading
  `target/<target>/<profile>/build/<crate>/<hash>/run/stdout`. It shows the cfg that the target
  got.
- `wasm32-wasip2` has `target_env = "p2"`. `foundation_compact`'s `wasi_p2_3` entropy backend
  imports an undeclared `wasip2` crate. So a `foundation_db` wasip2 check stops in that
  dependency. Do not read a wasip1 pass as a wasip2 pass.

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
- The type cannot show the rule. `ReadinessWaker` is still `Send` and `Sync`, so safe code
  can move it. Issue #423 records this risk and its likely fix.

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

### A CPU bound in a test must include its own bookkeeping (issue #425)

- The idle-CPU test measured every `valtron-worker` thread and required under 2 ms of CPU
  in one second. The Linux container measured 2.3 to 2.7 ms. The workers made 8 to 9
  wake-ups, which is the documented idle rate, so they did not spin.
- One wake costs about 0.25 ms of CPU in the unoptimised `uat` build. The 2 ms bound had no
  room for the wake cost and the container scheduler.
- Raise the bound to 10 ms. This is 2.7 times below the old defect (about 27 ms) and 3.5
  times above the measured steady state. Keep the wake-up bound at 30, because the defect
  made about 900 wake-ups.
- Measure a bound against the defect it must catch, not against a best-case run.

### Read the status before you read the body (issue #410)

- The unary client read the whole response body, then checked the HTTP status. A 256 KiB
  error body then delayed the error and used memory for data that becomes one message.
- The fix reads the response head, then branches on the status. For a non-OK status it reads
  only `error_body_max_bytes` (default 8 KiB) and drops the stream. The error returns at
  once, and the body closes early.
- The regression test writes 64 chunks of 4 KiB through a pipe. It asserts that the message
  length is the cap and that the sender did not finish every chunk. A timer is not needed,
  because the dropped stream is the proof.
- The cap is a field of `SizeLimits` with a builder method. The default follows nginx's
  error-page buffer and `foundation_db`'s error-body default.

### A hard-coded socket timeout is configuration (issue #420)

- `Connection::connect_unix` set a read timeout of 5 seconds and ignored the result of
  `set_read_timeout`. A caller could not change the timeout, and a failed set was silent.
- The timeout is now a `ClientConfig` field with a builder method. `connect_unix` takes it
  as an argument and returns the error when the platform cannot set it.
- The test reads the timeout back from the kernel socket option. It proves that the
  configured value reaches the socket without a sleep.

### An ignored `must_use` result can be a lost wake (issues #390, #424)

- `PipeSender::close` returns a bool that only reports who closed the pipe. A dropped result
  is correct there, and `let _ =` records that choice.
- `FdReadiness::register_waker` is different: `false` means the reactor no longer holds the
  fd, so the waker will never fire. Ignoring it parks the task forever. The fix returns
  `BrokenPipe` so the poll fails instead.
- The per-crate clippy command shows residual warnings from dependency crates. CI uses
  `--no-deps -- -D warnings`, so only the crate's own warnings fail the run.

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

### Identifiers come from `foundation_compact::uuid`, not from a `uuid` backend

- `foundation_compact` already owns the version-4 UUID path: `uuid::new_v4()` fills 16 bytes
  from `entropy::fill` and sets the version and variant bits with `uuid`'s builder. The
  entropy backend is selected for the target (the operating system source on native, the
  `abi` host module on wasm32-unknown-unknown). This is the spec 70 rule 11 path.
- `foundation_http`'s request-ID middleware called `uuid::new_v4()` directly and enabled
  `uuid`'s `v4` and `js` features. It now calls `foundation_compact::uuid::new_v4()` and
  warns if the entropy source fails. The `uuid` dependency is gone from the crate.
- `uuid` 1.20 refuses to compile on wasm32-unknown-unknown unless one of `js`,
  `rng-getrandom` or `rng-rand` is on, even when the crate only uses the value API. The old
  `uuid`/`js` feature hid this for the whole graph. With it gone, no crate in the
  `foundation_http` wasm graph enables `uuid`'s `v4` feature, so `uuid` compiles without its
  `rng` module. Keep `foundation_compact`'s `uuid` dependency at
  `default-features = false`; do not add a randomness feature to silence a compile error.
- `#[must_use]` on `FdReadiness::register_waker` and `wait_for_events` exposed two ignored
  return values in `foundation_http`. The writer now returns `NotConnected` when the reactor
  no longer holds its registration (parking would hang). The accept loop names the ignored
  generation with a comment, because the next iteration takes a fresh snapshot.

### A crate version bump must update the standalone example lockfiles

- The plan job runs `cargo metadata --locked` for `examples/devbinary`,
  `examples/web/wello` and `examples/platform_android/src-tauri`. These manifests are outside
  the workspace and take the crates by path, so a crate version bump makes their lockfiles
  stale and the job fails.
- The `foundation_wasmtime` bump to 0.2.0 updated the workspace lockfile but missed
  `examples/platform_android/src-tauri/Cargo.lock`, which still named 0.1.1. Regenerate each
  one with `cargo metadata --offline` and check it with `cargo metadata --locked`.

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
