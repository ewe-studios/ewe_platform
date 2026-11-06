---
feature: "The macOS websocket upgrade relay test passes or is gated to its supported platform"
status: "pending (CI proof)"
lane: "priority follow-up (backlog §9)"
model: "sonnet"
branch: "spec70/f129-macos-websocket-relay"
crates: ["foundation_proxy"]
catalogue_rows: ["9"]
---

# Feature 129: the macOS websocket upgrade relay test

_Written in ASD-STE100 Simplified Technical English (STE). Code and identifiers stay as
they are. Follow [requirements.md](../../requirements.md) sections 3, 4 and 8._

Glossary:

- **upgrade**: the HTTP exchange that turns a request into a byte tunnel
  (`101 Switching Protocols`).
- **relay**: the proxy's two pumps that copy bytes between the client and the backend.
- **splice**: the kernel call that moves bytes between two sockets without a copy in user
  space.

## Problem (recorded 2026-11-05, backlog §9)

`foundation_proxy/tests/proxy_features_tests.rs::websocket_upgrade_relays_and_tunnels_bytes`
fails on macOS. The client receives the `101`, writes its frame, and then `read_exact` at
line 464 times out with `WouldBlock` waiting for the echo.

What is known:

- It fails the same way with the feature 63 writer changes reverted for the run.
- The proxy's upgrade path (`forward_upgrade` plus `aio::splice`) is unchanged by feature
  63, so the failure predates it.
- It passes in the Linux CI image.
- Retested on 2026-11-06 while features 126 and 127 ran the suite: it **failed** inside
  `cargo test -p foundation_proxy` twice and **passed** inside the same command once. The
  earlier note that it "fails on every macOS run" is wrong: the failure is intermittent,
  so a single green run does not close this feature.
- Feature 121 has replaced a blocking body read and a lost timer wake in nearby code. The
  failure survives that change.

## Design

1. Re-run the test alone and in the full suite on the current branch, ten times each, and
   record the result as a pass/fail count. Because the failure is intermittent, a single
   pass is not evidence; twenty green runs in a row are. If all pass, close this feature
   with that count and delete the backlog entry.
2. If it still fails, trace the two relays with `RUST_LOG=foundation_proxy=trace,
   foundation_netio=trace`. Answer these questions in the Result:
   - Does the backend receive the client's frame?
   - Does the backend send the echo?
   - Does the proxy's upstream→client pump read it and write it to the client?
   - On macOS, is `aio::splice` used or the buffered fallback? The syscall differs.
3. Fix the cause. If the cause is a macOS platform limitation of the splice path, gate the
   splice path (not the test) to the platforms that support it, and keep the test running
   everywhere. Gate the test itself only as a last resort, with a comment naming this
   feature and the syscall that does not work.

## Tests

The test itself is the proof. It must pass on macOS and in the Linux image. If it is
gated, the gate names the reason and the backlog entry stays until the cause is fixed.

## Verify

```bash
cargo fmt --all --check
cargo test --profile uat -p foundation_proxy --test proxy_features_tests
cargo test --profile uat -p foundation_proxy
```

## Done when

- [ ] The test passes on macOS, or its platform gate is recorded with a reason.
- [ ] The trace answers the four questions above in the Result.
- [ ] The backlog §9 entry is deleted when the cause is fixed.
- [ ] CI green.

## Result

2026-11-06. Fixed on macOS. The test runs everywhere; it is not gated.

### Reproduction before the fix

| Command | Runs | Pass | Fail |
|---|---|---|---|
| `cargo test --profile uat -p foundation_proxy --test proxy_features_tests websocket_upgrade_relays_and_tunnels_bytes -- --exact` | 10 | 0 | 10 |
| `cargo test --profile uat -p foundation_proxy` | 10 | 0 | 10 |

Every failure was `read echo: Os { code: 35, kind: WouldBlock }` at `tests/proxy_features_tests.rs:464`.

### Trace answers

Temporary `eprintln!` probes in `aio::pump`, the test echo backend and the kqueue selector
printed this order. The probes were removed after the run.

```text
[f129 kqueue] fd=8 data=77 last=None new_edge=true
[f129 pump] src_fd=8 dst_fd=7 n=77        # upstream -> client: the 101 head
[f129 kqueue] fd=7 data=15 last=None new_edge=true
[f129 pump] src_fd=7 dst_fd=8 n=15        # client -> upstream: the frame
[f129 backend] read n=15                  # the backend receives the frame
[f129 kqueue] fd=8 data=15 last=Some(77) new_edge=false   # the echo is suppressed
read echo: WouldBlock
[f129 pump] src_fd=8 dst_fd=7 n=15        # read only after the client gave up
```

- **Does the backend receive the client's frame?** Yes. `[f129 backend] read n=15`.
- **Does the backend send the echo?** Yes. The echo is queued on the upstream socket
  (fd 8) and raises a kqueue readable event with `data=15`.
- **Does the upstream→client pump read it and write it to the client?** No, not before the
  timeout. The reactor suppressed the echo event (`new_edge=false`), so the parked pump
  never woke. The pump read the 15 bytes only after the client socket closed.
- **On macOS, is `aio::splice` used or the buffered fallback?** The buffered fallback. In
  this code `crate::aio::splice` is a userspace two-way pump on every platform. It never
  calls `splice(2)`, and macOS has no `splice(2)`. The proxy uses `ServerIo::Std` on macOS;
  `io_uring` is Linux only.

### Cause

`RawStream::read_wake_source` armed the reactor waker without first clearing the latched
readiness (`backends/foundation_netio/src/native/raw_stream.rs:483`). The macOS kqueue
selector delivers a readable edge only when the socket's byte count grows
(`backends/foundation_reactor/src/poll/sys/unix/selector/kqueue.rs:458`). It re-arms a
drained fd only through `Reactor::clear` → `Poll::rearm_readable`. The pump saw the 77-byte
`101` head, the selector recorded 77 bytes, and the later 15-byte echo did not grow the
count. epoll on Linux needs no re-arm, so Linux passed.

### Fix

The `ReadWakeSource` closure now calls `readiness.clear()` before it registers the waker.
This follows the documented `FdReadiness` arm protocol: clear, register, then read once
more, then park. The reset also re-arms the kqueue read edge. Every caller of the source
gets the same order, including the response body readers.

### Verification after the fix

| Command | Exit | Result |
|---|---|---|
| `cargo fmt --all --check` | 0 | clean |
| `cargo clippy --profile uat -p foundation_proxy --all-targets --no-deps -- -D warnings` | 0 | no warnings |
| `cargo clippy --profile uat -p foundation_netio --all-targets --no-deps -- -D warnings` | 0 | no warnings |
| single test, 20 runs | 0 | 20 pass, 0 fail |
| `cargo test --profile uat -p foundation_proxy` | 0 | every test binary passes |
| `cargo test --profile uat -p foundation_netio` | 0 | 1218 tests pass in the main binary |

### Deviations

- The root cause is in `foundation_netio`, not `foundation_proxy`. The brief allows this
  edit when the cause is there. Only `raw_stream.rs` changed.
- No probe remains in the tree; `git status` shows only the fix, the backlog edit and this
  file.
- The splice path is not gated and the test is not gated.

### Found but not fixed

- The kqueue read-edge suppression also applies to sockets, not only the pipes named in
  the comment (`backends/foundation_reactor/src/poll/sys/unix/selector/kqueue.rs:458`).
  Any readiness user that does not call `FdReadiness::clear` can lose a later, smaller
  frame. This fix covers the `RawStream` source; the selector rule stays a latent risk.
- `RawStream::read_wake_source` registers a new `FdReadiness` token on each call for the
  same fd. Two live sources on one fd can replace each other's kqueue registration
  (`backends/foundation_netio/src/native/raw_stream.rs:480`). This test does not hit it.
- The backlog §9 entry for this test is deleted. The two findings above need their own
  entries and a scheduled feature.
