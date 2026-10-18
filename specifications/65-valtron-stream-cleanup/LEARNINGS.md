# LEARNINGS — 65-valtron-stream-cleanup

## Design decisions and rationale

- **Audit before deletion.** The cleanup scope was determined by a full
  cross-crate sweep of every `ConcurrentQueue<Stream<D, P>>` communicator, not by
  eyeballing the obvious dead types. This surfaced that the combinator layer also
  uses `ConcurrentQueue<Stream<D, P>>` but is alive and cross-crate, which kept it
  out of scope.
- **Two dead types, not three.** `RecvIterator` (`synca/mpp`) looked suspicious but
  is still the channel iterator behind `future_task.rs` (`ThreadedValue`) and the
  channel API — only its `StreamRecvIterator` wrapper is dead.
- **The stale "default iterator" claim.** `ConcurrentQueueStreamIterator`'s docs
  still called it "the default iterator used by valtron executors", but the
  execution path had already moved to `NotifyQueueStreamIterator`. Doc comments are
  the main place dead types hide after the code stops using them.

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

_Version: 1.0 — Created 2026-10-18_
