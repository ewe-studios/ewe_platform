---
feature: "io_uring readiness selector (D14 F2)"
description: "Multishot-poll io_uring backend behind the existing Selector interface, with the uring↔epoll parity suite"
status: "complete"
priority: "medium"
phase: 2
depends_on: ["40-shared-reactor"]
estimated_effort: "large"
created: 2026-07-03
---
# Feature 41-uring-selector: io_uring readiness selector (D14 F2)

## Description

The real io_uring selector — readiness mode first, slotted behind the existing Poll/Registry API so no caller changes, gated by the parity suite.

## Normative sources (single source of truth — read before writing code)

- decisions/14-io-uring-reactor-backend.md — Scope #1 + Decided Details OQ#14.2 (normative)

## Scope

- poll/sys/unix/selector/uring.rs (IORING_OP_POLL_ADD multishot) behind cfg(linux, feature=uring)
- uring↔epoll behavioral parity test suite (same events for same stimuli, edge triggers, close-mid-poll, drain order)

## Out of scope

- Completion mode (feature 43)

## Acceptance criteria

- Parity suite green on a ≥5.13 kernel; identical task code across backends (D14 success criteria)

## Reopened 2026-07-10 — parity suite never existed; re-closed the same day

`uring.rs` was written and compiled under `--features uring`, but the parity suite this
feature's own scope and acceptance criteria require was never written. `grep -rln 'feature =
"uring"' tests/` matched nothing: there were **zero** uring-gated tests in the crate.

This matters beyond F41's own bookkeeping. D14 OQ#14.2 makes the suite the gate on F43:
"Ships with a uring↔epoll behavioral parity test suite (same events for same stimuli, edge
triggers, fd close mid-poll, drain ordering) — that suite later defines what F4's fallback
must preserve. **F4 does not start until it is green.**" F43 was about to be started against
a gate that had never been built.

### The selector had never been executed

Writing the first test that ran it surfaced a deadlock on the very first registration:

- `poll()` took `Mutex<IoUring>` and then called `submit_and_wait(1)` **while holding it**.
  The drain thread parked in `io_cqring_wait` holding the mutex; every `register_fd` blocked
  forever in `arm_poll` waiting for the same mutex. Confirmed from `/proc/<pid>/task/*/wchan`.
- `poll()` ignored its `timeout` argument entirely except for a zero check, so the drain
  thread could never observe its own shutdown flag.
- The poll mask omitted `POLLRDHUP`, which `epoll_events_from_interest` requests — peer
  shutdown would have been reported differently by the two backends.

### Fixes

- `IoUring` is `Send + Sync`; the ring is now held bare with separate `sq`/`cq` mutexes, and
  the wait holds **neither** — `Submitter::submit*` only issues `io_uring_enter`, which is
  safe to call concurrently with SQ pushes. Lock order where two are taken: `entries` → `sq`.
- Real timeout support via `submit_with_args` + `Timespec`; `ETIME`/`EINTR`/`EBUSY`/`EAGAIN`
  are benign "no completion this round", everything else propagates.
- `POLLRDHUP` requested; revents → epoll-bit translation written out explicitly.
- Multishot re-arm when the kernel clears `IORING_CQE_F_MORE`, else the fd goes deaf.
- `Event::from_parts` replaces a `transmute` of `libc::epoll_event`.

### Both backends now compile together

`sys/unix/mod.rs` compiled *either* epoll *or* uring. Both are now always compiled on Linux:
the parity suite needs to drive them side by side in one process, and F42's runtime ladder
cannot fall back to a backend that isn't in the binary. `Registry` construction moved from
`Selector::new_with_registry` (which hardcoded the chosen type) into `Poll::new`.

### Suite

`tests/uring_epoll_parity_tests.rs` — 9 scenarios, each driving one stimulus against both
selectors and comparing token → readiness-flag maps (order-insensitive within a drain; the
backends may batch differently, not disagree). Each scenario also asserts the *expected*
readiness, since agreement alone would be satisfied by two identically-wrong backends.

Covered: readable-after-write, idle silence, edge-trigger non-refire, re-arm after drain,
write-end close mid-poll (hangup), deregistration, multi-fd drain completeness, zero-timeout
non-blocking, timeout expiry. Verified to have teeth by mutation: dropping the `POLLHUP`
translation from the uring selector fails `parity_write_end_closed_mid_poll` and nothing else.
