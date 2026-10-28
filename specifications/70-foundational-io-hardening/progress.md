# Spec 70 progress

_Newest first. One entry per feature PR: date, feature, PR, result._

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
