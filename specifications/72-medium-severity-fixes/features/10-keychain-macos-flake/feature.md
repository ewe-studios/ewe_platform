---
feature: "The keychain streamed-body test reports timeouts and asserts progress"
status: "pending"
priority: "medium"
depends_on: []
crates: ["foundation_keychain"]
issues: [197]
model: "sonnet"
branch: "spec72/f10-keychain-macos-flake"
---

# Feature 10: Keychain macOS flake

_Written in ASD-STE100 Simplified Technical English (STE). Code and identifiers stay as they
are._

**Abbreviations in this file.** socket receive timeout (`SO_RCVTIMEO`); AeroSpace and Defence
Industries Association of Europe (ASD), the owner of the STE standard ASD-STE100.

## 1. Goal

Make `slow_request_body_does_not_hold_worker` stable on macOS and prove the property it
names. Close issue [#197](https://github.com/ewe-studios/ewe_platform/issues/197).

## 2. Problem

`tests/streamed_body.rs::slow_request_body_does_not_hold_worker` fails on macOS:
`read_response` panics with `WouldBlock` at `tests/streamed_body.rs:123`. macOS reports an
expired `SO_RCVTIMEO` as `WouldBlock`, so the helper cannot tell a timeout from a non-blocking
socket. It waits the full 10 s for a 2 s upload. The test's comment still says the Turso store
allows one use at a time, but feature 51 added the `ConnGate`.

## 3. Requirements

1. The helper reports a timeout as its own error, not as `WouldBlock`.
2. The test asserts progress: the third request answers while the uploads are still open. It
   does not assert an absolute 2 s wall time.
3. Remove the stale Turso comment.
4. Keep the test's intent: a slow request body does not hold a worker.
5. The test passes on Linux and macOS, and 20 runs in a row.

## 4. Design

- Change `read_response` to return a typed error enum: `Timeout`, `WouldBlock`, `Io`.
- On macOS, map an expired `SO_RCVTIMEO` (`WouldBlock` with an expired deadline) to
  `Timeout`.
- The test sends the third request before the uploads finish and asserts it answers while
  their connections are open. Use an explicit synchronization point, not a sleep.

## 5. Tasks

- [ ] Read `tests/streamed_body.rs` and the helper.
- [ ] Add the timeout error mapping.
- [ ] Rewrite the assertion to a progress check.
- [ ] Remove the stale comment.
- [ ] Run the verify commands. Record the results.
- [ ] Close #197 in the PR body.

## 6. Tests

The changed test is the test. Run it 20 times on macOS and on Linux. Report both counts.

## 7. Verify

```bash
export CARGO_PROFILE_DEV_DEBUG=0 CARGO_INCREMENTAL=0
cargo fmt -p foundation_keychain
cargo clippy --profile uat -p foundation_keychain --all-targets -- -D warnings
cargo test --profile uat -p foundation_keychain --test streamed_body
cargo test --profile uat -p foundation_keychain
```

## 8. Done when

- The test no longer panics on macOS.
- The test checks progress, not an absolute duration.
- 20 consecutive runs pass on macOS and on Linux.
- Issue #197 is closed by the PR.
- `progress.md` and `learnings.md` record the change.
