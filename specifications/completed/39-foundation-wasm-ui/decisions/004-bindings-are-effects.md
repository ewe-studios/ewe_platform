# 004 — Bindings are Effects in the signal graph

**Date:** 2026-06-08
**Status:** Resolved

### Decision

**DOM (and other) bindings are not separate subscription systems — they are `Effect`s registered in the signal graph.**

```rust
impl DomSignalBinding {
    pub fn bind<T, F>(ctx: &Context, signal: &Signal<T>, node_id: u32, transform: F) -> Self
    where
        T: Clone,
        F: Fn(&T) -> String + 'static,
    {
        let signal = signal.clone();  // Arc, cheap
        let effect = ctx.effect(move || {
            let value = signal.get();            // reads signal → dependency auto-tracked
            let text = transform(&value);        // transform to DOM operation
            receiver.queue(DomOp::SetText(node_id, text));  // queues, doesn't apply yet
        });
        
        Self { effect }
    }
}
```

### How it works

1. **Creation**: `ctx.effect()` registers a callback in the signal graph
2. **Initial run**: Effect runs immediately → reads signals → dependencies auto-linked via `startTracking`/`endTracking`
3. **Signal change**: `stabilize()` runs effects in height order → effect re-runs → queues DOM op into `InstructionReceiver` (decision 030)
4. **Flush**: After `stabilize()` completes → `InstructionReceiver.flush()` → protocol encodes ops → single FFI call → JS applies

### Why this design

- **No manual subscribe/unsubscribe** — disposal happens when the `Effect` is dropped
- **Glitch-free** — effect runs after all computeds it depends on are fresh (height ordering)
- **Batched** — multiple bindings queue into the same pending batch, flushed once per `stabilize()`
- **Generic** — not just `DomSignalBinding`. `EventSignalBinding`, `WorkerBinding`, `LogBinding` — all just effects that queue into the right place

### NotificationManager (separate concern)

For **external listeners** (JS interop, telemetry, custom integrations) who want to know "a batch is ready, here's what changed":
- Registered on the `Runtime`, not on individual signals
- Fires once after `stabilize()` completes
- Carries: which effects ran, what values changed (optional)
- Not used for DOM bindings — those are effects directly

### Effect registration and disposal

Effects are **nodes in the signal graph** — registered the same way computeds are. No separate global registry.

**`Effect` trait** (defined in `foundation_signals`, implemented by `DomBindingEffect` and others):
```rust
trait Effect {
    fn register(&mut self, runtime: &Arc<Runtime>);   // called on creation
    fn unregister(&self, runtime: &Arc<Runtime>);     // called on Drop
}
```

**Lifecycle:**
1. On creation: effect gets assigned height, links into dependency graph, calls `register(runtime)` 
2. On signal change: effect marked Dirty, inserted into `dirtyHeap[height]`
3. During `stabilize()`: effect runs, queues DOM op into `InstructionReceiver`
4. On Drop: effect calls `unregister(runtime)` — Runtime adds its ID to `pending_removals`
5. **Deferred removal**: Runtime collects removals during stabilize, applies them **after** the loop finishes (prevents heap corruption from mid-loop unlink)

**Global Runtime access:**
- `foundation_wasm_ui` initializes the global Runtime via a dedicated module
- Effects access it via `Runtime::global()` — scoped to that module, not `pub static`
- Each component creates its own `DomBindingEffect` instances with their own heights

### InstructionReceiver ownership

The `InstructionReceiver` lives on the `Runtime` (decision 030), not in `foundation_signals` or a global:
- Effects receive a reference to the receiver during creation and queue DOM ops into it
- The Runtime owns both the signal graph and the instruction receiver — no cross-boundary coupling
- `stabilize()` triggers `receiver.flush()` after all dirty nodes are processed
- The configured protocol handles encoding, memory allocation, and FFI dispatch

### Multi-signal bindings

No special multi-signal API needed. Two equivalent approaches:

1. **Direct effect** — closure reads whatever signals it needs, auto-tracked:
```rust
ctx.effect(|| {
    let first = sig1.get();   // tracked
    let last = sig2.get();    // tracked
    receiver.queue(DomOp::SetText(node_id, format!("{} {}", first, last)));
});
```

2. **`bind()` convenience** — accepts `Vec<&Signal<T>>` for single-type signal sets:
```rust
DomSignalBinding::bind(&ctx, vec![&sig1, &sig2], node_id, |values| {
    format!("{} {}", values[0], values[1])
});
```
- Height = `max(dep heights) + 1`
- Any dependent signal changes → effect re-runs → one DOM op queued
- Short-circuit in the closure (early return) → unread signals not tracked as dependencies for that invocation
