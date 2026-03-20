# Learnings

This file captures learnings, discoveries, and design decisions made during implementation of Valtron Async Iterators.

## Pending Learnings

_No learnings recorded yet._

---

## Design Decisions

### Why TaskStatusIterator instead of modifying Iterator?

The standard `Iterator` trait cannot be modified and is fundamentally synchronous. By creating a new `TaskStatusIterator` trait, we:
- Keep `std::iter::Iterator` unchanged for synchronous cases
- Make async state (`Pending`, `Delayed`, `Ready`) explicit in the type system
- Enable Valtron executor integration without blocking

### Why collect_all() returns TaskStatus<Vec<T>> not Vec<T>?

Returning `TaskStatus<Vec<T>>` instead of `Vec<T>` allows the collection itself to be async-aware:
- Caller sees `Pending` while any sources are still pending
- Caller sees `Ready(vec)` only when all sources complete
- Enables further composition (map, filter) on the collected result

---

_Created: 2026-03-20_
