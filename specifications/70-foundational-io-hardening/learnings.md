# Spec 70 learnings

_What an agent learned that the next agent needs. Newest first._

Feature 01 (valtron parking) is on 
ewe-studios/ewe_platform#163, but it is not ready to merge. I sent it back to the agent with two blocking fixes.

What the agent fixed:

The smol/tokio hang. It happened only with default features, not with multi. A change made a stream stop driving the engine after its first empty read, so a task woken from another thread was never run again. The fix is a small change in DrivenStreamIterator::wake, and I accept it.
The parking. ThreadedIterFuture now parks while its future waits or its queue is full, instead of spinning. The first empty read no longer blocks.
Tests and checks. Four new tests fail on the old code and pass now. Each test passed 20 runs in a row in both feature sets, and ci.py run foundation_core passes.
Two tests pass on the old code as well. The old code could not reach those failures; the tests are kept to protect the new paths.
Problems I found in review:

Blocking: TaskStatus::from_stream adds a new callback on every poll. The callback list clears only when an item arrives, so memory grows with each poll while a stream waits.
Blocking: a producer waiting on a full queue adds a new callback every 10 ms until the consumer takes an item. During a long stall, they pile up.
Minor: every pop now takes a lock, even when no producer is waiting.
For each problem, the agent must add a test that fails on the current branch, fix the code, and run the 20-run check again. CI on 
ewe-studios/ewe_platform#163 was still running when I checked; the agent will report its result with the fixes.

The agent also recorded one older limitation in the backlog: a task that stays parked for a long time can still leave an async consumer with nothing to wake it. The old code had the same problem.

- 2026-10-28 (feature 01): run valtron tests in BOTH feature sets: `--features multi` and the
  default (no `multi`). The `smol/tokio_drives_parked_future_task` hang only shows without
  `multi`. Cause: the single-thread `DrivenStreamIterator` (`executors/non_sendables.rs`) drives the
  engine inside `next`. Priming it (registering a wake before the first pull) switched the inner
  `NotifyQueueStreamIterator` to non-blocking mode, so an empty pull answered `Wait`; the consumer
  then parked on a queue wake. But a parked task is made runnable by a foreign thread flipping a
  readiness signal, which pushes nothing and fires no queue wake, and nobody called `next` again.
  Evidence: a trace showed two engine drives within 30 us, then silence while the 30 ms thread
  fired. Fix: that `wake` registers on the queue only and leaves the inner blocking. The deeper
  fix (readiness sources that can register a wake) is in `docs/bug-backlog.md` section 4.
- 2026-10-28 (feature 01): `ThreadedIterFuture` cannot have a full queue when the future's `Err` is
  pushed (nothing is pushed before it; the smallest queue holds 2), so the old `let _ = push(Err)`
  could not lose the error in practice; the error test pins delivery only.

- 2026-10-28 (from the PR #160 work): `H2Connection::flush_write` used `write_all`, so a
  `WouldBlock` part-way re-sent the already-written prefix and corrupted the HTTP/2 frame
  stream. Any non-blocking writer must track how much it wrote.
- 2026-10-28: the shared reactor's drain thread must request an executor resume after it latches
  events, or parked tasks wait for the idle back-off (up to 100 ms).

