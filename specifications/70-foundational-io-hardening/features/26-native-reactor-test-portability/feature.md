---
feature: "foundation_nativeapis shared-reactor tests compile and run on macOS"
status: "pending"
priority: "high"
depends_on: "—"
branch: "fix/io-26-native-reactor-test-portability"
crates: ["foundation_nativeapis"]
backlog: "docs/bug-backlog.md §13 (1 entry)"
---

# Feature 26: Native reactor tests on macOS

_Written in ASD-STE100 Simplified Technical English (STE). Code and identifiers stay as they
are._

**Abbreviations in this file.** pull request (PR); AeroSpace and Defence Industries
Association of Europe (ASD), the owner of the STE standard ASD-STE100.

Use the shared protocol in [`requirements.md`](../../requirements.md) sections 3 and 4.

## 1. Goal

`cargo check --workspace --all-targets` must pass on macOS. Today it fails on one test file,
`foundation_nativeapis/tests/reactor_shared_selector_tests.rs`. That file uses two Linux-only
things: `libc::pipe2` and `/proc/self/fd`. The `Reactor` under test already has a kqueue
backend for macOS. Only the test is not portable.

## 2. Fix

- **E1.** `nonblocking_pipe()` calls `libc::pipe2(fds, O_CLOEXEC | O_NONBLOCK)`. Replace it
  with a portable sequence: `libc::pipe(fds)`, then two `fcntl` calls. Set `O_NONBLOCK` with
  `F_SETFL`. Set `O_CLOEXEC` with `F_SETFD`. This works on Linux and macOS.
- **E2.** `registering_many_fds_adds_no_selector_fds` reads `/proc/self/fd`. That path is
  Linux-only. Gate the test and its `open_fd_count` helper with
  `#[cfg(target_os = "linux")]`. The other three tests use only the portable pipe, so they run
  on both platforms.

## 3. Tests

No new test. The fix makes the existing four tests compile on macOS. Three run on macOS. One
(the fd count) is Linux-only and still runs in CI.

## Verification (run exactly)

```bash
export CARGO_PROFILE_DEV_DEBUG=0 CARGO_INCREMENTAL=0
# On macOS:
cargo check -p foundation_nativeapis --all-targets --profile dev
cargo test -p foundation_nativeapis --profile dev --test reactor_shared_selector_tests
cargo check --workspace --all-targets --profile dev
# CI (Linux) also builds and runs the Linux-only test.
```

## Done when

- [ ] `cargo check --workspace --all-targets` passes on macOS.
- [ ] The three portable reactor tests pass on macOS. The Linux-only one passes on Linux.
- [ ] The backlog entry is deleted.
- [ ] CI green.
