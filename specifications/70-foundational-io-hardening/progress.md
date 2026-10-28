# Spec 70 progress

_Newest first. One entry per feature PR: date, feature, PR, result._

- 2026-10-28: feature 29 added (client body reader reaches a Waker-capable readiness source).
  Feature 03 T1–T2 landed and pass (netio 1206 tests, connectrpc, iogate), but T3–T6 are
  blocked: the client body is built in `foundation_netio`, whose kernel-socket readiness is
  poll-based `SocketReadiness` (no `std::task::Waker`), while the reactor that could wake a
  `Waker` (`FdReadiness`) lives in `foundation_iogate`, which depends on netio. Feature 03 §3.2
  says stop and report rather than invent a global registry. Feature 29 records the bridge
  (move the reactor below netio, add a netio poll reactor, or drive the body through valtron
  `Depends`). Pending, size M, depends on 02; blocks 03 → 04 → 05 → 06.

- 2026-10-28: feature 28 added (QUIC driver parks on its UDP socket). It closes the gap that
  feature 02 left open: `serve_h3` builds the `QuicDriver` without `with_fd_readiness`, so the
  driver still polls at 1 ms (`IDLE_POLL`), and `EventReadiness` has no `clear` for the
  edge-triggered socket. Pending, size M, depends on 02.

- 2026-10-28: feature 02 (HTTP/2 and HTTP/3 servers park on readiness) implemented on
  `fix/io-02-h2-h3-readiness`. The h2 connection parks on its socket through `FdReadiness`
  (readable-only; writable is opt-in via `set_writable` so an idle socket does not spin the
  drain thread). The reactor gained `reregister` and kqueue `reregister_fd` deletes a dropped
  filter before re-adding. The H3 accept task parks on its waker/shutdown; H3 request futures
  park on the QUIC connection. Three backlog entries deleted. The CPU test exposed a
  pre-existing executor busy-spin: a worker hosting only a readiness-parked task returned
  `CanProgress` every loop and `block_on` did not yield on it. Fixed by sleeping the readiness
  quantum in `block_on`'s `CanProgress` arm when nothing is runnable. PR number: see the PR.

- 2026-10-28: two new features added from bugs found during the feature 01 review. Feature 26
  (native reactor tests on macOS, `libc::pipe2`/`/proc/self/fd` portability) and feature 27
  (`SleepIterator` zero-duration deadline). Both are pending, size S, no dependency.

- 2026-10-28: feature 01 review fixes landed on `fix/io-01-valtron-parking`. The two blocking
  findings (per-poll callback growth in `TaskStatus::from_stream`/`prime`; full-queue producer
  callback pile-up) and the minor finding (lock on every pop) are fixed via `WakeableStream::announce`,
  `WakeRegistry::replace`, and a `has_waiters()` fast path in `NotifyQueue::pop`. Three new tests in
  `tests/valtron/wakeable_stream.rs` pin each fix. All valtron tests pass in both feature sets.

- 2026-10-28: feature 01 (valtron parking) done on `fix/io-01-valtron-parking`. `ThreadedIterFuture`
  parks on `Pending` and on a full queue; async stream lenses prime before the first pull; the
  `smol/tokio_drives_parked_future_task` hang (without `multi`) is fixed in
  `DrivenStreamIterator::wake`. PR number: see the PR.

- 2026-10-28: features 12–25 added: every other backlog section (6–21) and the four bugs
  recorded only on the branches of PRs #157 and #158 now have a feature. Spec total: 25.
- 2026-10-28: spec written by the main session. Baseline: PR #160 (backlog sections 1–5
  verified; correctness fixes; CI green). Readiness work in progress is on branch
  `fix/foundational-readiness` (commits `31467ebd4`, `ed13119db`, and a work-in-progress commit
  `fcbf523fa` with one hanging test).
