# 003 — Context-based signal scoping (single unified Runtime, sub-graphs for ownership)

**Date:** 2026-06-08
**Status:** Resolved

### Decision

**One global `Runtime`** — created once, never replaced. Owns the single unified reactive graph. `Runtime` maintains "what's currently active" for dependency linking.

**Contexts are sub-graphs** — logical groupings within that unified graph for ownership and disposal:

```rust
fn App(runtime: &Runtime) {
    let ctx = runtime.context();
    let users = ctx.signal::<Vec<User>>(vec![]);

    let child = ctx.child();
    let count = child.computed(|| users.get().len());

    // When child drops, count and its deps are cleaned up
    // The unified graph stabilizes as one
}
```

### How it works

- `Runtime` owns the global graph: bucket queue, version counter, active tracking
- `ctx.child()` creates a child context — tracks parent reference for disposal cascade
- All signals/computeds know which Context owns them
- `stabilize()` is unified — processes all dirty nodes across all contexts
- Drop a Context → its tagged region of the graph cascades unlink

### Why this design

- **No global/static magic** — `Runtime` is explicit, not a thread-local singleton like Leptos
- **Ownership** — component signals are cleaned up when their Context drops
- **Unified graph** — no cross-scope notification problem, no fragmentation cost
- **No naming collisions** — signal identity is the `Arc` reference, not a string
- **Disposal** — `onCleanup()` cascades when Context drops
