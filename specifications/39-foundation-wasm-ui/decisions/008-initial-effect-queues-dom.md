# 008 — Initial effect execution queues DOM op

**Date:** 2026-06-08
**Status:** Resolved

### Decision

When an effect is created and runs immediately to track dependencies, it **also queues its DOM op** into `InstructionReceiver`.

```rust
ctx.effect(move || {
    let value = signal.get();  // tracked as dependency
    receiver.queue(DomOp::SetText(node_id, value.to_string()));  // queues initial render too
});
```

### Why

- **Consistent** — the effect's closure is the same code, runs the same way every time
- **User intent** — if they put it in an effect, they want it rendered
- **No special case** — no "first run vs re-run" branching logic
- `InstructionReceiver` deduplicates — even if queued multiple times in the same flush, only the last write wins
