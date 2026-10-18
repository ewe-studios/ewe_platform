# LEARNINGS — 65-valtron-stream-cleanup

## Design decisions and rationale

- **Audit before deletion.** The cleanup scope was determined by a full
  cross-crate sweep of every `ConcurrentQueue<Stream<D, P>>` communicator, not by
  eyeballing the obvious dead types. This surfaced that the combinator layer also
  uses `ConcurrentQueue<Stream<D, P>>` but is alive and cross-crate, which kept it
  out of scope.
- **Two dead types, plus one to migrate.** `RecvIterator` (`synca/mpp`) looked
  suspicious but is alive — its sole non-test caller is `ThreadedIterFuture`
  (`future_task.rs`, `multi`), the `!Send`-iterator sync bridge. Only its
  `StreamRecvIterator` wrapper is dead. Feature 01 migrates that caller to
  `NotifyQueue`, so `RecvIterator` loses its last user while the `mpp` module
  itself stays (`Broadcaster`/`Receiver`/`recv_timeout` are used cross-crate).
- **`Pipe` is waker-based, not thread-blocking.** `Pipe<T>` parks by stashing a
  `Waker` and exposes no blocking receive / `Iterator`, so it cannot block the raw
  `std::thread` that consumes `ThreadedIterFuture`'s iterator. `NotifyQueue` is the
  condvar (thread-blocking) member of the family and is the correct replacement.
- **The stale "default iterator" claim.** `ConcurrentQueueStreamIterator`'s docs
  still called it "the default iterator used by valtron executors", but the
  execution path had already moved to `NotifyQueueStreamIterator`. Doc comments are
  the main place dead types hide after the code stops using them.
- **`Waiting` becomes a ~1 s heartbeat on multi.** `NotifyRecvIterator` maps
  `NotificationItem::None → ThreadedValue::Waiting`, and native
  `DEFAULT_NOTIFY_QUEUE_MAX_SPINS = 100` × the 10 ms wait timeout means that yield
  fires only after ~1 s of empty queue — a cooperative heartbeat, not a per-item
  yield. `close()` doesn't `notify_one`, so a parked consumer observes close on the
  next timeout expiry (≤ 10 ms), faster than the old 100 ms `park_timeout` floor.
- **The migration breaks three tests by contract, not by determinism.** Three
  `multi` tests (`future_error`, `empty_iterator`, `backpressure`) assert
  `Waiting => unreachable!`. They usually still pass because `Waiting` needs ~1 s
  of empty queue, but the assertion is now invalid and flaky under scheduling
  pressure — convert to tolerant and add a slow-iterator case that provably emits
  `Waiting`.

## Challenges

- Distinguishing "no callers" from "only test callers" vs "only doc-comment
  references" requires grepping each reference class separately, not a single
  name search.

## Patterns to reuse

- For a dead-code sweep: grep the type name three ways — definitions, non-comment
  non-test references, and cross-crate imports — then decide.
- A removal feature should record the audit as the task list, so the person doing
  the work can trust the "safe to delete" claim without re-deriving it.

---

_Version: 1.2 — Created 2026-10-18_
