# Learnings — medium-severity defect fixes

_Written in ASD-STE100 Simplified Technical English (STE)._

_Newest first. Updated after each batch. See
`.agents/skills/specifications-management/skill.md`._

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
