# Learnings — medium-severity defect fixes

_Written in ASD-STE100 Simplified Technical English (STE)._

_Newest first. Updated after each batch. See
`.agents/skills/specifications-management/skill.md`._

## Batch 2 (2026-11-09)

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
