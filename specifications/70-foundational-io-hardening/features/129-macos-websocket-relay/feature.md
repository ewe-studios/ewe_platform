---
feature: "The macOS websocket upgrade relay test passes or is gated to its supported platform"
status: "pending"
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
- It fails on every macOS run and passes in the Linux CI image.
- Feature 121 has since replaced a blocking body read and a lost timer wake in nearby
  code, so the cause may already be gone; the test has not been re-run since.

## Design

1. Re-run the test alone and in the full suite on the current branch, ten times each, and
   record the result. If it now passes, close this feature with that evidence and delete
   the backlog entry.
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

2026-11-06. Plan only. Not started. Recorded while feature 121 measured the proxy stall.
