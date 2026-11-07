---
feature: "The streamed-body test helper does not sleep before its first request"
status: "pending"
priority: "low"
depends_on: []
crates: ["foundation_keychain"]
issues: [358]
model: "haiku"
branch: "spec72/f14-keychain-start-server-sync"
---

# Feature 14: Streamed-body start synchronization

_Written in ASD-STE100 Simplified Technical English (STE). Code and identifiers stay as they
are._

**Abbreviations in this file.** Transmission Control Protocol (TCP); AeroSpace and Defence
Industries Association of Europe (ASD), the owner of the STE standard ASD-STE100.

## 1. Goal

Remove the fixed 150 ms sleep from `start_server` in
`backends/foundation_keychain/tests/streamed_body.rs`. Close issue
[#358](https://github.com/ewe-studios/ewe_platform/issues/358).

This feature was added to the spec on 2026-11-07 by the owner. It is the last feature of the
spec. It is `severity: low`, and it was found while fixing feature 10.

## 2. Problem

`start_server` binds the listener, starts the serve loop on a thread, and then sleeps 150 ms
before the test sends its first request.

The socket is bound before the thread starts. A connect succeeds into the accept backlog, so
the sleep is not needed. It adds a fixed 150 ms to each test. It also gives no guarantee that
the serve loop has started on a loaded machine: a slow machine can start the loop after the
sleep, and a fast machine wastes the wait.

## 3. Requirements

1. No fixed sleep in `start_server`. No sleep anywhere on the path to the first request.
2. The first request waits for the listener, not for a timer. Connecting is the signal: the
   kernel accepts into the backlog until the loop runs.
3. If the test needs to know that the loop is running, use a readiness channel from the serve
   thread, not a duration.
4. The test keeps its intent and stays stable. It passes 20 runs in a row on macOS and on
   Linux.

## 4. Design

- Delete the sleep.
- Keep the listener bind before the thread starts, so a connect cannot fail.
- If a later assertion needs the loop to have started, send one message on a channel from the
  thread after its first accept, and select on it in the test.

## 5. Tasks

- [ ] Read `start_server` and the tests that call it.
- [ ] Remove the sleep and add a readiness signal only if an assertion needs one.
- [ ] Run the tests 20 times in a row.
- [ ] Close #358 in the PR body.

## 6. Tests

The changed helper is its own test. Run `cargo test --profile uat -p foundation_keychain
--test streamed_body` 20 times and report the counts for macOS and Linux.

## 7. Verify

```bash
export CARGO_PROFILE_DEV_DEBUG=0 CARGO_INCREMENTAL=0
cargo fmt -p foundation_keychain
cargo clippy --profile uat -p foundation_keychain --all-targets --no-deps -- -D warnings
cargo test --profile uat -p foundation_keychain --test streamed_body
```

## 8. Done when

- No sleep remains in `start_server`.
- The test passes 20 runs in a row on macOS, and on Linux where the image is reachable.
- Issue #358 is closed by the PR.
- `progress.md` and `learnings.md` record the change.
