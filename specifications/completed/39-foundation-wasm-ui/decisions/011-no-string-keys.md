# 011 — Signal identity: value-based, no string keys

**Date:** 2026-06-08
**Status:** Resolved

### Decision

**No string keys on signals.** The signal IS the variable:

```rust
// Don't do this:
let users = ctx.signal::<Vec<User>>("users");  // redundant string key

// Do this:
let users = ctx.signal::<Vec<User>>(vec![]);   // signal IS the variable
```

### How it works

- `ctx.signal(initial_value)` creates and returns a `Signal<T>` — the `Arc` is the identity
- No name collision possible — two `ctx.signal()` calls return different `Arc` references
- Computed signals track dependencies via `Arc::ptr_eq`, not string lookup
- Cross-context: pass `&Signal<T>` reference, the graph links by pointer identity

### Why this matters

- **No collision** — `"users"` used twice = same signal (bug). Two `ctx.signal()` calls = different signals
- **No typos** — no string to misspell
- **Type-safe** — compiler enforces uniqueness via `let` binding scope
- **Cleaner API** — `signal(initial_value)` not `signal::<T>("name", initial_value)`
