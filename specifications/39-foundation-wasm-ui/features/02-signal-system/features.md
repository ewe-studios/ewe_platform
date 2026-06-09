# Feature 02: Signal System (foundation_signals)

## Description

Create `foundation_signals` — a standalone reactive signal crate with no DOM coupling. Pure Rust, works anywhere (native, WASM, server). Uses R3-style architecture: height-based topological ordering, bucket queue, version-based stale detection, diamond dependency safety, explicit disposal, batch coalescing via `stabilize()`. Signals return `(getter, setter)` tuples for ergonomic function-style components.

Signal identity is the `Arc` reference — no string keys (decision 011). Bindings are effects in the signal graph, not a separate subscription system (decision 004). Effects run immediately on creation to track dependencies and queue their initial operation (decision 008).

**Decisions:** 002, 003, 004, 008, 011, 029

## Crate

`crates/foundation_signals/` (depends on `foundation_ui_traits`)

**G47 resolved — `foundation_signals` crate location:** New crate at `crates/foundation_signals/`.
Not a rename — it's a fresh crate. The existing signal-related code in `foundation_wasm` (if any)
will be migrated into this crate during the refactor (F00).

---

## Types & Structs

### Runtime

Single unified reactive graph. Created once, never replaced. Not a thread-local singleton — passed explicitly.

```rust
pub struct Runtime {
    dirty_heap: Vec<Vec<NodeId>>,                         // bucket queue indexed by height
    version: u64,                                          // global counter, incremented each stabilize
    active_effect: Option<NodeId>,                         // currently evaluating node (for dep tracking)
    pending_removals: Vec<NodeId>,                         // deferred removal during stabilize
    nodes: SlotMap<NodeId, Node>,                          // primary node storage, O(1) lookup
    callbacks: BTreeMap<u64, Box<dyn FnMut(EventData)>>,   // JS interop callback registry — EventData from F08
    next_callback_id: u64,                                 // monotonically increasing, never reset
    notification_managers: Vec<Box<dyn NotificationManager>>,     // fired after stabilize completes
}
```

**Invariants:** `stabilize()` is the only entry point for flushing dirty nodes — `set()` never immediately propagates. `dirty_heap` grows dynamically. Callback IDs are never reused; stale lookup returns None.

### Node

Three node types stored in the same SlotMap.

```rust
pub enum Node { Signal(SignalNode), Computed(ComputedNode), Effect(EffectNode) }

pub struct SignalNode {
    value: Box<dyn Any>,         // type-erased, downcast at read time
    observers: Vec<NodeId>,      // nodes depending on this signal
    version: u64,                // version at which last written
}

pub struct ComputedNode {
    compute_fn: Box<dyn FnMut() -> Box<dyn Any>>,
    height: u32,                 // max(dep_heights) + 1
    deps: Vec<NodeId>,           // current dependencies (updated by tracking)
    cached: Box<dyn Any>,        // last evaluation result
    state: ThreeState,
    observers: Vec<NodeId>,
    version: u64,                // version at which cached value last changed
}

pub struct EffectNode {
    effect_fn: Box<dyn FnMut()>,
    height: u32,                 // max(dep_heights) + 1
    deps: Vec<NodeId>,
    state: ThreeState,
    cleanup: Option<Box<dyn FnOnce()>>,  // runs before re-eval and on disposal
}
```

### ThreeState

```rust
#[derive(Clone, Copy, PartialEq, Eq)]
pub enum ThreeState { Clean, Check, Dirty }
```

**Transition rules:**

1. **Signal.set()** with new value: mark all observers `Dirty`, insert into `dirty_heap[observer.height]`.
2. **Computed value changes** after re-evaluation: mark Computed observers `Check` (transitive — might not change), mark Effect observers `Dirty`, insert all into dirty_heap.
3. **Processing Check**: re-check whether direct deps actually changed. If any did, promote to Dirty and re-evaluate. If none did, demote to Clean (short-circuit — skip re-evaluation).
4. **Processing Dirty**: re-evaluate, compare new vs cached, set to Clean. If value changed, propagate to observers per rule 2.

### Context

Logical ownership group within the unified graph. Provides scoped creation and cascading disposal.

```rust
pub struct Context {
    runtime: Arc<Runtime>,        // shared reference to the single runtime
    parent: Option<ContextId>,    // for disposal cascading
    owned_nodes: Vec<NodeId>,     // disposed when context drops
    children: Vec<ContextId>,     // dropped recursively on disposal
}

impl Context {
    pub fn new(runtime: Arc<Runtime>) -> Self;
    pub fn child(&self) -> Context;
    pub fn signal<T: Any + Clone + PartialEq + 'static>(
        &self, initial: T
    ) -> (SignalGetter<T>, SignalSetter<T>);
    pub fn computed<T: Any + Clone + PartialEq + 'static>(
        &self, f: impl FnMut() -> T + 'static
    ) -> ComputedGetter<T>;
    pub fn effect(&self, f: impl FnMut() + 'static);  // runs immediately (decision 008)
    pub fn on_cleanup(&self, f: impl FnOnce() + 'static);
}
```

**How effects produce side-effects (DOM ops):** Effects are closures — they capture whatever they need from their environment. The signal system doesn't know or care what an effect does. A `DomSignalBinding` helper in `foundation_wasm_ui` creates the effect and captures a reference to `InstructionReceiver`:

```rust
// In foundation_wasm_ui — no trait needed, closure captures receiver directly
impl DomSignalBinding {
    pub fn bind<T: Clone + 'static, F: Fn(&T) -> String + 'static>(
        ctx: &Context, getter: &SignalGetter<T>, receiver: &InstructionReceiver,
        node_id: u32, transform: F,
    ) -> Self {
        let getter = getter.clone();
        let receiver = receiver.clone();  // InstructionReceiver is Clone (Arc-based)
        ctx.effect(move || {
            let value = getter.get();                          // dependency tracked
            receiver.queue(DomOp::SetText { node_id, text: transform(&value) });  // direct call
        });
        Self { /* effect_node_id */ }
    }
}
```

The closure captures `receiver` alongside `getter`. No bridge trait, no `Box<dyn Any>`, no downcast. The signal system runs the closure; the closure does what it wants.

**Disposal:** dropping a Context runs on_cleanup for all owned effects, adds owned nodes to pending_removals, drops children recursively. Double-dispose is a no-op (SlotMap removal of nonexistent key does nothing).

### SignalGetter\<T\>

Read handle. Cloneable (Arc). Reading inside an effect/computed subscribes the caller as a dependency.

```rust
pub struct SignalGetter<T> {
    storage: Arc<SignalStorage<T>>,
    node_id: NodeId,
}

struct SignalStorage<T> {
    runtime: Arc<Runtime>,
    value: RefCell<T>,
}

impl<T: Clone + 'static> SignalGetter<T> {
    pub fn get(&self) -> T;
    // If runtime.active_effect is Some(caller_id):
    //   add caller_id to this signal's observers
    //   add self.node_id to caller's deps
    // Return value.clone()
}
```

### SignalSetter\<T\>

Write handle. Carries callback_id for JS interop.

```rust
pub struct SignalSetter<T> {
    storage: Arc<SignalStorage<T>>,  // same Arc as getter
    node_id: NodeId,
    callback_id: u64,               // monotonic, assigned at creation
}

impl<T: Any + Clone + PartialEq + 'static> SignalSetter<T> {
    pub fn set(&self, value: T);       // marks observers Dirty if value differs (PartialEq)
    pub fn update<F: FnOnce(&mut T)>(&self, f: F);
    pub fn callback_id(&self) -> u64;  // for html! macro codegen
}
```

### ComputedGetter\<T\>

Read handle for computed. Same subscription behavior as SignalGetter on read. Between stabilize() calls, returns last cached value.

```rust
pub struct ComputedGetter<T> {
    storage: Arc<ComputedStorage<T>>,
    node_id: NodeId,
}

impl<T: Clone + 'static> ComputedGetter<T> {
    pub fn get(&self) -> T;
}
```

---

## Algorithms

### stabilize()

Single flush entry point. Processes all dirty nodes across all contexts in topological order.

```
fn stabilize(runtime: &mut Runtime):
    runtime.version += 1

    for height in 0..runtime.dirty_heap.len():
        while let Some(node_id) = runtime.dirty_heap[height].pop():
            let node = runtime.nodes.get_mut(node_id)

            match node:
                Computed, state == Check:
                    // Check resolution: verify deps actually changed
                    if no dep version increased since last seen:
                        node.state = Clean; continue   // short-circuit
                    node.state = Dirty                 // promote, fall through

                Computed, state == Dirty:
                    startTracking(runtime, node_id)
                    let new_value = (node.compute_fn)()
                    endTracking(runtime, node_id)
                    if new_value != node.cached:
                        node.cached = new_value
                        node.version = runtime.version
                        // mark observers: Computed→Check, Effect→Dirty
                        // insert into dirty_heap[obs.height]
                    node.state = Clean

                Effect, state == Dirty:
                    if let Some(cleanup) = node.cleanup.take(): cleanup()
                    startTracking(runtime, node_id)
                    (node.effect_fn)()
                    endTracking(runtime, node_id)
                    node.state = Clean

                _: continue  // already Clean

    for node_id in runtime.pending_removals.drain(..):
        unlink_and_remove(runtime, node_id)

    for manager in &mut runtime.notification_managers:
        manager.on_stabilize_complete()
```

**Complexity:** O(D) where D = number of dirty nodes. Bucket queue gives O(1) insertion, level-by-level iteration without sorting.

### startTracking / endTracking

Dynamic dependency discovery. No static analysis — deps are discovered by intercepting get() calls.

**startTracking(runtime, node_id):** set `active_effect = Some(node_id)`, save current deps as prev_deps, clear deps.

**During evaluation:** any `signal.get()` or `computed.get()` checks `active_effect`. If set, the read node adds the active node to its observers and the active node records the read node in its deps.

**endTracking(runtime, node_id):** set `active_effect = None`. Compare new deps vs prev_deps. Unlink stale deps (in old but not new) — remove node_id from their observer lists. Link new deps (in new but not old) — add node_id to their observer lists. Recompute height if deps changed.

### Height Computation

Height determines processing order. Correct height assignment provides diamond dependency safety.

- **Signals**: height 0 (sources, never re-evaluated)
- **Computeds/Effects**: `height = max(dep_heights) + 1`, recomputed by endTracking

**Diamond example:** Signal A (h=0) -> Computed B (h=1), Computed C (h=1) -> Computed D (h=2). When A changes: B and C process at height 1, both mark D. D processes at height 2, reads fresh B and C. One evaluation, no duplicate, no special detection needed.

### onCleanup

Registered via `ctx.on_cleanup(fn)`. Runs (1) before each re-evaluation, and (2) on disposal when owning Context drops. Stores closure in EffectNode.cleanup — calling on_cleanup again replaces the previous one.

```rust
ctx.effect(|| {
    let id = set_interval(|| log("tick"), 1000);
    ctx.on_cleanup(move || clear_interval(id));
});
```

---

## Effect System

### Effect Trait

Defined in `foundation_signals`, implemented by concrete effect types. Effects are nodes in the signal graph — not a separate subscription system.

```rust
pub trait Effect {
    fn register(&mut self, runtime: &Arc<Runtime>);   // called on creation
    fn unregister(&self, runtime: &Arc<Runtime>);     // called on Drop
}
```

**Lifecycle:**
1. Creation: assigned height, linked into graph, `register()` called, runs immediately (decision 008)
2. Signal change: marked Dirty, inserted into dirty_heap
3. During stabilize: cleanup runs, effect_fn re-executes, deps re-tracked
4. On Drop: `unregister()` called, NodeId added to pending_removals
5. Deferred removal: after dirty loop finishes, nodes unlinked and freed — prevents heap corruption from mid-loop disposal

### DomSignalBinding

An effect bridging signal changes to DOM operations. Implemented in `foundation_wasm_ui`, not `foundation_signals`. Exactly the pattern from decision 004 — closure captures signal getter and receiver directly:

```rust
// In foundation_wasm_ui
impl DomSignalBinding {
    pub fn bind<T: Clone + 'static, F: Fn(&T) -> String + 'static>(
        ctx: &Context, getter: &SignalGetter<T>, receiver: &InstructionReceiver,
        node_id: u32, transform: F,
    ) -> Self {
        let getter = getter.clone();
        let receiver = receiver.clone();  // InstructionReceiver is Clone (Arc-based)
        ctx.effect(move || {
            let value = getter.get();                          // dependency tracked
            receiver.queue(DomOp::SetText { node_id, text: transform(&value) });
        });
        Self { /* effect_node_id */ }
    }
}
```

**Flow:** setter.set() -> observers marked Dirty -> stabilize() -> effect re-runs -> DomOp queued into InstructionReceiver -> flush() -> ProtocolEncoder -> single FFI call -> JS applies.

### NotificationManager

For external listeners (telemetry, JS interop) that want post-stabilize notification. Not used for DOM bindings.

```rust
pub trait NotificationManager {
    fn on_stabilize_complete(&mut self);
}
```

Registered on Runtime, fires once after stabilize in registration order.

---

## Getter/Setter API

### Tuple Returns

`ctx.signal()` returns `(SignalGetter<T>, SignalSetter<T>)`. Enables function-component pattern:

```rust
fn Counter(ctx: &Context) -> Html {
    let (count, set_count) = ctx.signal(0);
    html! {
        <button primal:onclick={move |_| set_count.update(|c| *c += 1)}>
            Count: {count.get()}
        </button>
    }
}
```

Both handles share `Arc<SignalStorage<T>>`. Getters for read-only access, setters for write access.

### callback_id and JS Interop

Each setter carries `callback_id: u64` assigned from the Runtime's monotonic counter. The ID bridges JS DOM events to Rust signal updates: JS serializes full event data into an arena slot, calls `invoke_callback(42, memory_id)` across the WASM boundary. Rust reads the `EventData` from the arena slot, looks up ID 42 in `runtime.callbacks`, dispatches to the setter closure. Stale IDs (disposed signals) return None — silently dropped. The arena slot is freed by Rust after reading (no JS-side dispose needed).

### Two-Way Binding Codegen

The `html!` macro (F03) detects `SignalSetter` in event-handler position and generates callback wiring:

1. Developer writes: `html! { <input primal:onchange={set_name} /> }`
2. Macro detects setter, reads `set_name.callback_id()` (e.g. 42)
3. Codegen registers callback: `runtime.register_callback(42, |event_data| { if let Some(v) = event_data.value { set_name.set(v) } })`
4. Rendered HTML attribute: `primal:setter(42)`
5. JS user interaction -> serializes `{ type: "change", value: "hello", ... }` into arena slot -> `invoke_callback(42, memory_id)` -> Rust deserializes EventData -> setter closure extracts `value` -> signal update -> stabilize -> effects

IDs are monotonically increasing per Runtime, assigned at signal creation time. The macro reads the ID; it does not assign it.

---

## Error Cases

**Stale callback invocation:** `invoke_callback(id, value)` with disposed signal — BTreeMap lookup returns None, silently dropped. Expected during component teardown races.

**Double dispose:** dropping an already-dropped Context — SlotMap removal of nonexistent key is a no-op. No error, no panic.

**Re-entrant set during stabilize:** effect calls `setter.set()` during its execution — newly dirtied observers are inserted into dirty_heap at their heights. If height >= current level, processed in this pass. If below, processed in a subsequent stabilize call. Never triggers recursive/inline propagation.

**Type mismatch on callback:** JS sends undeserializable value — `serde_json::from_value` fails, set is skipped, signal retains previous value. Logged if logging configured.

**G15 resolved — `RefCell<T>` thread safety:** `SignalStorage<T>` uses `RefCell<T>` because WASM is single-threaded.
For native (non-WASM) builds, `foundation_signals` uses a `single_threaded` feature flag (default).
When `single_threaded` is off, `RefCell<T>` is replaced with `Mutex<T>` via a cfg-based type alias:
```rust
#[cfg(feature = "single_threaded")]
type Cell<T> = RefCell<T>;
#[cfg(not(feature = "single_threaded"))]
type Cell<T> = Mutex<T>;
```
The default feature is `single_threaded` — WASM always uses this path.

**G16 resolved — `notification_managers` ordering:** NotificationManagers fire after the dirty loop
completes, in registration order. If a NotificationManager modifies signals during its callback,
the newly dirtied observers are NOT processed in the same stabilize() — they'll be processed on
the next `stabilize()` call. No infinite loop guard needed because stabilize() doesn't recurse;
it processes the dirty heap once and returns.

**G17 resolved — `callback_id` allocation timing:** Each setter gets its `callback_id` at signal
creation time (`ctx.signal()`), not lazily. The ID is assigned from `Runtime.next_callback_id`
when the `SignalSetter` is constructed. The callback is registered immediately in the BTreeMap:
`runtime.callbacks.insert(callback_id, Box::new(|event_data| { /* dispatches to setter */ }))`.

**G18 resolved — `ComputedNode.cached` downcast:** `ComputedNode.cached: Box<dyn Any>` stores
the last evaluation result. When `computed.get()` is called, it downcasts to `&T`. If types
changed between evaluations (e.g., the closure was modified at runtime — unlikely but possible
via dynamic code), the downcast fails and the computed re-evaluates. In practice, the closure
type is fixed at compile time, so this cannot happen.

---

## Integration Points

**F01 (IntoHtml):** `foundation_signals` implements `IntoHtml` for `SignalGetter<T>` where `T: IntoHtml + Clone`. Signals appear directly in html! output; get() inside an effect context auto-subscribes.

**F03 (Macro):** html! macro inspects expressions in event-handler positions. SignalSetter triggers callback registration codegen, emits `primal:setter(ID)` as DOM attribute.

**F04 (InstructionReceiver):** Effects queue `DomOp` values into the InstructionReceiver. The signal system has no knowledge of DOM — closures capture a receiver reference and queue ops as a side effect.

**F08 (Event Runtime):** Receives DOM events from JS, dispatches via `invoke_callback(id, value)`. The other half of two-way binding: JS events -> callback registry -> setter -> signal update -> stabilize.

---

## File Ownership

```
crates/foundation_signals/src/
    lib.rs           — re-exports, crate docs
    runtime.rs       — Runtime, stabilize(), startTracking/endTracking
    node.rs          — Node enum, SignalNode, ComputedNode, EffectNode, ThreeState
    context.rs       — Context, child(), on_cleanup(), disposal
    signal.rs        — SignalGetter<T>, SignalSetter<T>, SignalStorage<T>
    computed.rs      — ComputedGetter<T>, ComputedStorage<T>
    effect.rs        — Effect trait, effect creation
    callback.rs      — Callback registry, invoke_callback, ID assignment
    notification.rs  — NotificationManager trait
Cargo.toml           — depends on foundation_ui_traits, serde_json
```

---

## Refactoring Strategy

**Phase 1 — Core Graph:** Runtime, Node, ThreeState, stabilize(), startTracking/endTracking. Raw node manipulation, no type-safe handles.

**Phase 2 — Getter/Setter API:** SignalGetter, SignalSetter, ComputedGetter. Type-safe handles wrapping raw nodes. ctx.signal() tuple return.

**Phase 3 — Context and Disposal:** Context, child(), on_cleanup(). Cascading disposal and deferred removal.

**Phase 4 — Callback Registry:** callback_id(), BTreeMap registry, invoke_callback(). Stale ID handling.

**Phase 5 — Effect Trait and Notification:** Effect trait, NotificationManager. Extension points for downstream crates.

---

## Dependencies

- `foundation_ui_traits` — IntoHtml, DomOp, Html types
- `serde_json` — event data deserialization in callback invocations
- `slotmap` (or equivalent) — stable-ID node storage with O(1) lookup

---

## Testing

### Signal Basics
- Create signal(0), getter.get() returns 0. setter.set(5), getter returns 5. setter.update(|v| *v += 3), getter returns 8.
- Two signals from same context are independent — different NodeIds, different values.

### Effect Tracking
- Create signal(0), create effect reading it. Effect runs immediately with value 0 (decision 008).
- setter.set(5), stabilize() — effect ran exactly once with value 5.
- Conditional read (`if flag { sig.get() }`) — when false, signal is NOT a dependency.

### Computed
- Signals A, B; computed C = A + B. Set A, stabilize — C updates. Set both A and B, stabilize — C re-evaluates once.

### Diamond Dependency
- A -> B (A*2), A -> C (A*3), B+C -> D. Set A, stabilize — D computed exactly once, correct value. B and C both fresh when D evaluates.

### ThreeState Short-Circuit
- Signal A, Computed B = (A > 10), Computed C reads B. Set A from 1 to 2 — B re-evaluates (still false), C gets Check, resolves to Clean without re-evaluating.

### Context Disposal
- Create context with signal and effect, drop context. Cleanup runs, signal removed. Subsequent stabilize skips disposed nodes. Child disposal cascades.

### Callback Registry
- setter.callback_id() returns monotonic u64. Two setters have different IDs.
- invoke_callback(id, value) updates signal. invoke_callback(stale_id, value) silently dropped.
- After context drop, callback IDs return None on lookup.

### stabilize() Batching
- Set signal three times (1, 2, 3), stabilize once — effect runs once, sees value 3.
- Multiple signals set — all effects run in correct height order, each once.

### Re-entrant Set
- Effect reads A, writes B. Set A, stabilize. B's observers scheduled into dirty_heap, processed in same pass if height permits.

### Cleanup Ordering
- Effect registers on_cleanup. Signal changes, stabilize — cleanup runs BEFORE effect re-executes. On context drop, cleanup runs for all owned effects.

### Height Recomputation
- Computed initially reads signal A (height 1). After re-evaluation reads A and computed C — height recomputed to max(0, C.height) + 1.
