# Signal System — Learnings from R3 & Cross-Framework Analysis

## Source

Deep exploration of the **r3** reactive system (`/home/darkvoid/Boxxed/@dev/repo-expolorations/r3/`) — a ~420-line TypeScript implementation of a hybrid push-pull reactive system with height-based topological ordering, bucket queue scheduling, and three-color marking.

---

## 1. Core Concepts

### The Fundamental Tradeoff

| Approach | Pros | Cons |
|----------|------|------|
| **Push** (notify everyone on change) | Fast propagation | Wastes work on unused branches, needs batching |
| **Pull** (compute on demand) | Simple, no batching needed | Re-traverses graph constantly |
| **Push-Pull Hybrid** | Best of both | Complex implementation |

R3 implements a **height-ordered push with lazy pull** — the most efficient known approach for fine-grained reactivity.

### The Three Pillars

1. **Height-based topological ordering** — every computed node has a `height = max(dep heights) + 1`. Changes flow from low to high, preventing glitches.
2. **Dirty heap (bucket queue)** — a bucket queue organized by height. `dirtyHeap[height] = [computed nodes at that height]`.
3. **Three-color marking** — `None` (clean), `Check` (may need recomputation), `Dirty` (definitely needs recomputation).

---

## 2. Data Structures (Rust Design)

### Signal<T> — The Source

```
Signal<T> {
    value: T,
    subs: *mut Link,      // Head of subscription list
    subs_tail: *mut Link, // Tail for O(1) append
}
```

A Signal is the root of truth — values are set externally, never computed.

### Computed<T> — The Derived Worker

```
Computed<T> {
    // Inherited from Signal (a computed IS a signal)
    value: T,
    subs: *mut Link,
    subs_tail: *mut Link,

    // Dependency tracking
    deps: *mut Link,      // What I depend on
    deps_tail: *mut Link, // O(1) append

    // Scheduling
    flags: ReactiveFlags, // bitfield: None/Check/Dirty/RecomputingDeps/InHeap
    height: u32,          // topological level
    next_heap: *mut Computed, // linked list in dirtyHeap bucket
    prev_heap: *mut Computed,

    // Execution
    fn: fn() -> T,        // the computation function
    disposal: Vec<Box<dyn FnOnce()>>, // cleanup callbacks

    // Firewall children (component ownership)
    child_firewall: *mut FirewallSignal,
}
```

### Link — The Bidirectional Edge

```
Link {
    dep: SignalOrComputed,   // The dependency (signal or computed)
    sub: *mut Computed,      // The subscriber (always computed)
    next_dep: *mut Link,     // Next dep in subscriber's list
    prev_sub: *mut Link,     // Prev sub in dependency's list
    next_sub: *mut Link,     // Next sub in dependency's list
}
```

**Why bidirectional**: Dep → Sub for "notify children when I change"; Sub → Dep for "check if I need to update".

### ReactiveFlags (Bitfield)

```rust
bitflags! {
    struct ReactiveFlags: u8 {
        const NONE            = 0;
        const CHECK           = 1 << 0;  // May need recomputation
        const DIRTY           = 1 << 1;  // Must recompute
        const RECOMPUTING_DEPS = 1 << 2; // Currently tracking deps
        const IN_HEAP         = 1 << 3;  // Currently in dirtyHeap
    }
}
```

### Global Scheduler State

```rust
struct Scheduler {
    dirty_heap: Vec<Option<*mut Computed>>, // bucket queue, size ~2000
    min_dirty: u32,
    max_dirty: u32,
    context: Option<*mut Computed>,         // currently executing computed
    marked_heap: bool,                       // has heap been marked this batch?
}
```

---

## 3. The Algorithm — Step by Step

### 3.1 read(signal) — Dependency Tracking

```
read(signal):
    if context is set (inside a computed):
        link(signal, context)  // Subscribe context to signal

        if signal is a computed:
            // Height adjustment: maintain topological order
            if signal.height >= context.height:
                context.height = signal.height + 1

            // Pull-based update: ensure dependency is fresh
            if signal.height >= min_dirty || signal.flags has CHECK/DIRTY:
                mark_heap()           // Mark all in heap
                update_if_necessary(signal)

    return signal.value
```

**Key insight**: Reading creates the edge in the dependency graph. The `link()` function has fast paths — if the same dep was just read, it's O(1) no-op.

### 3.2 link(dep, sub) — Fast-Path Subscription

```
link(dep, sub):
    prevDep = sub.deps_tail

    // Fast path #1: same dep as last read (most common case)
    if prevDep.dep === dep: return

    // Fast path #2: dep is next in line (recomputing, same order)
    if recomputing and prevDep.nextDep.dep === dep:
        sub.deps_tail = prevDep.nextDep
        return

    // Fast path #3: already subscribed from dep's side
    if dep.subs_tail.sub === sub and is_valid_link(dep.subs_tail, sub):
        return

    // Full path: create new link
    newLink = { dep, sub, nextDep: prevDep.nextDep, prevSub: dep.subs_tail }
    sub.deps_tail = newLink
    dep.subs_tail = newLink
    // Wire into both linked lists
```

### 3.3 setSignal(signal, value) — Change Propagation

```
setSignal(signal, value):
    if signal.value === value: return  // Early exit — critical optimization
    signal.value = value

    for link in signal.subs:  // Iterate all direct subscribers
        insert_into_heap(link.sub)  // Insert into dirtyHeap at their height
```

**Only marks direct subscribers** — not recursive descendants. Height ordering handles the rest during stabilize.

### 3.4 insertIntoHeap(computed) — O(1) Bucket Insert

```
insertIntoHeap(computed):
    if computed.flags has IN_HEAP: return  // Don't duplicate

    computed.flags |= IN_HEAP
    height = computed.height
    bucket = dirtyHeap[height]

    if bucket is empty:
        dirtyHeap[height] = computed
    else:
        // Append to end of linked list (O(1) with prevHeap)
        tail = bucket.prevHeap
        tail.nextHeap = computed
        computed.prevHeap = tail
        bucket.prevHeap = computed

    if height > max_dirty:
        max_dirty = height
```

### 3.5 stabilize() — Topological Flush

```
stabilize():
    for height from min_dirty to max_dirty:
        el = dirtyHeap[height]
        dirtyHeap[height] = null  // Clear bucket

        while el is not null:
            next = el.nextHeap
            recompute(el, del=false)
            el = next
```

**Why this works**: Processing from lowest height to highest guarantees we never compute a node before its dependencies are updated. This prevents **glitches** — inconsistent states where some nodes have updated and others haven't.

### 3.6 recompute(computed) — Execute & Propagate

```
recompute(computed):
    // Phase 1: Preparation
    run_disposal(computed)      // Run cleanup callbacks
    old_context = context
    context = computed
    computed.deps_tail = null   // Reset deps tracking
    computed.flags = RECOMPUTING_DEPS

    // Phase 2: Execute
    new_value = computed.fn()   // During execution, read() calls link()

    // Phase 3: Cleanup
    computed.flags = NONE
    context = old_context

    // Remove stale dependencies (deps not re-subscribed)
    remove_unused_deps(computed)

    // Phase 4: Propagation
    if new_value !== computed.value:
        computed.value = new_value
        for link in computed.subs:
            if link.sub.flags has CHECK:
                link.sub.flags |= DIRTY  // Upgrade Check to Dirty
            insert_into_heap(link.sub)
```

### 3.7 updateIfNecessary(computed) — Lazy Pull

```
updateIfNecessary(computed):
    // If marked Check, verify by checking deps
    if computed.flags has CHECK:
        for dep in computed.deps:
            if dep is a computed:
                updateIfNecessary(dep)  // Recursive pull
            if computed.flags has DIRTY:
                break  // Early exit — we know we're dirty

    // If Dirty, recompute
    if computed.flags has DIRTY:
        recompute(computed, del=true)

    computed.flags = NONE
```

---

## 4. Edge Cases — How R3 Handles Them

### Diamond Dependencies

```
     s (h=0)
    /│\
   a b c  (h=1)
    \│/
     d    (h=2)
```

Without height ordering, `d` might run twice. With height ordering: `stabilize()` processes height 1 (a, b, c) before height 2 (d). `d` runs exactly once with consistent state.

### Dynamic Dependencies

```rust
let c = computed(|| {
    if read(cond) { read(a) } else { read(b) }
});
```

When `cond` changes:
1. `c` is marked Dirty
2. `recompute(c)` runs
3. `c.fn()` only reads the active branch
4. `unlinkSubs()` removes the stale dependency automatically

### Disappearing Dependencies

```rust
let c = computed(|| {
    if done { return 0; }
    let v = read(s);
    if v > 2 { done = true; }
    v
});
```

Once `done` becomes true, `c` stops reading `s`. `unlinkSubs()` removes `c` from `s.subs`. Future changes to `s` never trigger `c` again.

### Check vs Dirty — Short-Circuit Evaluation

```
Check = "Something changed upstream, verify if it affects me"
Dirty = "I know my inputs changed, I must recompute"
```

A computed marked `Check` can exit early:
```rust
let c = computed(|| {
    if !read(a) { return "early exit"; }  // a changed but result same
    read(b) + read(c)
});
```

When `a` changes, `c` is marked `Check` (not `Dirty`). During `updateIfNecessary`, if `c`'s early exit means its value didn't change, its subscribers aren't notified. This is **lazy evaluation** — work is only done if actually needed.

---

## 5. Firewall Signals — Component Ownership

Firewall signals allow **component-scoped reactivity**:

```rust
let selector = computed(|| { ... });
let a = signal(true, &selector);  // FirewallSignal owned by selector
let b = signal(false, &selector);
```

Structure:
```
Computed (selector)
    └─ child → FirewallSignal A
                ├─ owner: selector
                └─ nextChild → FirewallSignal B
```

When `selector` recomputes:
1. All its firewall children's subscribers are marked `Check`
2. This propagates invalidation through component boundaries

**For WASM-UI**: Each WASM component gets its own computed selector. Signals created within that component are firewall signals, ensuring the component's reactivity is properly scoped.

---

## 6. Bridge to the Web Side (JS Runtime)

This is where R3 meets our spec-39 JS runtime. The signal system in Rust/WASM needs to notify the JS side of changes.

### Notification Flow

```
Rust Signal Change
    │
    ▼
setSignal(signal, value)
    │
    ▼
recompute() detects value change
    │
    ▼
For each subscriber:
    if subscriber is a DOM binding:
        enqueue_dom_update(subscriber.dom_target)
    if subscriber is a computed:
        insert_into_heap(subscriber)
    │
    ▼
After stabilize():
    flush_dom_updates()
```

### DOM Update Queue

DOM updates should be **batched** — collected during `stabilize()`, then flushed in a single microtask:

```rust
struct DomUpdateQueue {
    updates: Vec<(NodeId, DomUpdate)>,
}

impl DomUpdateQueue {
    fn enqueue(&mut self, node_id: u32, update: DomUpdate) {
        self.updates.push((node_id, update));
    }

    fn flush(&mut self) {
        if self.updates.is_empty() { return; }

        // Send all updates to JS in one Arrow batch
        let batch = build_arrow_batch(&self.updates);
        send_to_js(&batch);
        self.updates.clear();
    }
}
```

### Arrow Batch Format for Signal-Driven Updates

Each DOM update is an Arrow row:

```
op_id | node_id | operation  | attribute  | value      | text_val
u32   | u32     | u8 enum    | string     | string     | string
```

Operations:
1. `SetText` — update text content
2. `SetAttribute` — set attribute
3. `RemoveAttribute` — remove attribute
4. `AddClass` — add CSS class
5. `RemoveClass` — remove CSS class
6. `SetStyle` — set inline style
7. `RemoveStyle` — remove inline style
8. `Show` — set display to ''
9. `Hide` — set display to 'none'
10. `InsertBefore` — insert DOM node
11. `AppendChild` — append DOM node
12. `RemoveChild` — remove DOM node
13. `ReplaceNode` — replace DOM node
14. `SetValue` — set input value
15. `SetChecked` — set checkbox/radio checked state

### WASM → JS Communication

```rust
// In the WASM signal system:
fn flush_dom_updates() {
    let batch = build_arrow_batch(&self.pending_updates);
    // Host invoke: host_batch_apply(batch_ptr, batch_len)
    unsafe {
        host_batch_apply(batch.as_ptr() as *const u8, batch.len());
    }
}
```

The JS runtime (`foundation-wasm-ui.js`) receives the Arrow batch and applies it to the real DOM:

```js
class ArrowDomApplicator {
  apply(buffer) {
    const columns = parseArrowColumns(buffer);
    for (let i = 0; i < columns.op_id.length; i++) {
      applyOp(columns.op_id[i], columns.node_id[i], columns.operation[i], ...);
    }
  }
}
```

---

## 7. Rust Implementation Architecture

### Crate Structure

```
foundation_wasm_ui/src/shared/signal.rs
```

### Key Types

```rust
// Opaque handle types for JS-side IDs
pub struct SignalId(u32);
pub struct ComputedId(u32);
pub struct NodeId(u32);  // JS DOM node reference

// Signal is generic but must be Send + Clone
pub struct Signal<T: Clone> {
    pub value: T,
    subs: *mut Link,
    subs_tail: *mut Link,
}

// Computed is parameterized by a function type
pub struct Computed<T: Clone> {
    value: T,
    subs: *mut Link,
    subs_tail: *mut Link,
    deps: *mut Link,
    deps_tail: *mut Link,
    flags: ReactiveFlags,
    height: u32,
    next_heap: *mut Computed<T>,
    prev_heap: *mut Computed<T>,
    computation: Box<dyn FnMut() -> T>,
    disposal: Vec<Box<dyn FnOnce()>>,
}

// Links use raw pointers for performance
struct Link {
    dep: DepTarget,
    sub: *mut ComputedAny,
    next_dep: *mut Link,
    prev_sub: *mut Link,
    next_sub: *mut Link,
}

enum DepTarget {
    Signal(*mut SignalAny),
    Computed(*mut ComputedAny),
}
```

### Global Scheduler

```rust
static mut SCHEDULER: Scheduler = Scheduler::new();

struct Scheduler {
    dirty_heap: Vec<Option<*mut ComputedAny>>,
    min_dirty: u32,
    max_dirty: u32,
    context: Option<*mut ComputedAny>,
    dom_queue: DomUpdateQueue,
}
```

### Public API

```rust
/// Create a new signal
pub fn signal<T: Clone>(value: T) -> Signal<T>;

/// Create a new signal owned by a computed (firewall signal)
pub fn signal_owned<T: Clone>(value: T, owner: &Computed<T>) -> Signal<T>;

/// Set signal value, propagate changes
pub fn set_signal<T: Clone>(signal: &mut Signal<T>, value: T);

/// Create a computed value
pub fn computed<T: Clone, F: FnMut() -> T + 'static>(fn: F) -> Computed<T>;

/// Stabilize — process all pending updates
pub fn stabilize();

/// Flush DOM updates to JS
pub fn flush_dom_updates();

/// Read a signal (call this inside computed functions)
pub fn read<T: Clone>(signal: &Signal<T>) -> T;

/// Register a DOM binding subscriber
pub fn bind_dom<T: Clone>(signal: &Signal<T>, node_id: NodeId, update_type: DomUpdateType);

/// Cleanup registration
pub fn on_cleanup(fn: impl FnOnce() + 'static);
```

---

## 8. DOM Binding Signals

This is how the Rust signal system connects to the JS DOM side.

### DomSignalBinding

```rust
struct DomSignalBinding {
    signal: *mut SignalAny,
    node_id: NodeId,
    update_type: DomUpdateType,
    next: *mut DomSignalBinding,
}

enum DomUpdateType {
    TextContent,        // Update text node
    Attribute(String),  // Set attribute
    Class(String),      // Add/remove class
    Style(String),      // Set inline style
    InputValue,         // Set input value
    Checked,            // Set checkbox checked
    Show,               // Show element
    Hide,               // Hide element
}
```

### How Bindings Work

1. When a Rust component creates a DOM binding:
   ```rust
   // In the html! macro expansion:
   let binding = DomSignalBinding {
       signal: &mut count_signal,
       node_id: NodeId(42),
       update_type: DomUpdateType::TextContent,
       next: null,
   };
   binding.subscribe();  // Adds to signal.subs
   ```

2. When the signal changes:
   ```rust
   set_signal(&mut count, 5);
   ```

3. During `recompute()`:
   - The binding's subscriber is inserted into `dirtyHeap`
   - When processed, the binding enqueues a DOM update

4. During `flush_dom_updates()`:
   - All pending updates are packed into an Arrow batch
   - Sent to JS via `host_batch_apply()`
   - JS applies to real DOM in one shot

### Avoiding Double-Subscription

A critical issue: a DOM binding is also a subscriber in the signal's `subs` list. When a computed that contains a DOM binding recomputes, the binding might get subscribed twice (once from the signal directly, once through the computed).

**Solution**: DOM bindings subscribe to signals via `bind_dom()`, which creates a special link. During `recompute()`, old bindings are cleaned up via `unlinkSubs()`, and new bindings are created during the fn() execution. This ensures each binding is subscribed exactly once.

---

## 9. Cross-Framework Signal Comparison

| System | Ordering | Marking | Dynamic Deps | GC | DOM Updates |
|--------|----------|---------|--------------|-----|-------------|
| **R3** | Height buckets | Tri-color (Check/Dirty/None) | Yes | Unwatched → unlink | Via subscriber |
| **Svelte 5** | Effect tree + batch | DIRTY/MAYBE_DIRTY/CLEAN + write_version | Yes | Destroy effect children | Direct in effects |
| **Alpine** | Microtask queue | Scheduler flag | Yes | Element removal → cleanup | Direct in effect |
| **SolidJS** | Queue + generations | Sync/Dirty | Yes | Dispose → cleanup | Direct in effects |
| **Vue Reactivity** | Queue | Dirty flag | Yes | Stop effect | Via watcher → render |

### Key Takeaway

**R3's height-based bucket queue** is the most efficient for complex dependency graphs with many nodes. **Svelte 5's write_version** system is the simplest for version-based dirty checking. For our WASM-UI, we should combine:
- R3's height-based ordering (guarantees glitch-free updates)
- Svelte 5's write_version (efficient dirty detection without recursive checks)
- Svelte 5's effect tree (branch skipping for O(1) subtree stability)

---

## 10. Design Decisions for Our Implementation

### 10.1 Height-Based Ordering — YES

**Why**: Diamond dependencies are common in UI (multiple signals feeding into one component). Height ordering guarantees glitch-free updates.

**Implementation**: Track height on each computed. Adjust during `read()`. Use bucket queue for scheduling.

### 10.2 Three-Color Marking — YES

**Why**: The Check/Dirty distinction enables short-circuit evaluation. If a signal changes but the computed's output is the same (early return), downstream subscribers aren't notified.

**Implementation**: Use bitflags for flags. Check bit = "verify", Dirty bit = "must recompute".

### 10.3 Bidirectional Links — YES

**Why**: O(1) subscription and unsubscription. Essential for dynamic dependency tracking.

**Implementation**: Use raw pointers (or indices into an arena) for Link nodes. Tail pointers for O(1) append.

### 10.4 Arena Allocation — YES

**Why**: Signals and computeds are created frequently and need stable addresses. Arena allocation avoids fragmentation and enables O(1) deallocation.

**Implementation**:
```rust
struct SignalArena {
    signals: Vec<SignalAny>,
    free_list: Vec<u32>,
}

struct ComputedArena {
    computeds: Vec<ComputedAny>,
    free_list: Vec<u32>,
}

struct LinkArena {
    links: Vec<Link>,
    free_list: Vec<u32>,
}
```

### 10.5 DOM Update Batching — YES

**Why**: Multiple signal changes in one tick should produce ONE Arrow batch, not many individual messages.

**Implementation**: Collect DOM updates during `stabilize()`, flush after.

### 10.6 Forks (Speculative Updates) — FUTURE

R3 doesn't have forks, but Svelte 5 does. For our WASM-UI, speculative updates are useful for:
- Prefetching data on hover
- Undo/redo
- Optimistic UI updates

**Implementation**: Save snapshot of dirty_heap, dirty flags, and signal values. Restore on discard.

### 10.7 Untracked Reads — YES

```rust
pub fn untrack<T, F: FnOnce() -> T>(f: F) -> T {
    let old_untracking = UNSUBTRACKING;
    UNSUBTRACKING = true;
    let result = f();
    UNSUBTRACKING = old_untracking;
    result
}
```

Essential for reading signals without creating dependencies (e.g., logging, analytics).

### 10.8 Effects (Side-Effect Reactions)

Beyond computed values, we need **effects** — reactions that perform side effects (DOM updates, network calls):

```rust
pub struct Effect {
    deps: *mut Link,
    deps_tail: *mut Link,
    fn: Box<dyn FnMut()>,
    teardown: Option<Box<dyn FnOnce()>>,
}
```

Effects are like computed but:
- Don't produce a value
- Always run when dirty (no short-circuit)
- Are leaf nodes (no subscribers)

### 10.9 JS → WASM Signal Notification

When JS detects a DOM change (input, click), it needs to notify the WASM signal system:

```js
// foundation-wasm-ui.js
class SignalBridge {
  bindInput(nodeId, signalId) {
    // When input fires:
    element.addEventListener('input', () => {
      const value = element.value;
      wasm.signal_set_string(signalId, value);
    });
  }
}
```

The WASM side receives the signal set, runs `stabilize()`, and flushes DOM updates back to JS.

---

## 11. Memory Management

### Garbage Collection Strategy

Unlike JS, Rust doesn't have a GC. We need explicit cleanup:

1. **Computed disposal**: When a computed has no subscribers (`unwatched()`), it unlinks from all deps and runs disposal callbacks.

2. **DOM binding cleanup**: When a DOM node is removed, its bindings must be unsubscribed:
   ```rust
   pub fn cleanup_bindings(node_id: NodeId) {
       // Find all DomSignalBinding for this node
       // Unsubscribe from their signals via unlinkSubs()
       // Free the binding memory
   }
   ```

3. **Arena compaction**: Periodically compact arenas to reclaim freed slots.

### Cross-Boundary Memory

The JS runtime holds references to WASM memory (via `ExternalPointer`/`InternalPointer`). When WASM disposes a signal, the JS side must be notified to drop its reference.

---

## 12. Summary: The Mental Model

Think of the signal system as a **waterfall with buckets**:

1. **Signals** are the source of water at the top (height 0)
2. **Computed nodes** are buckets at different heights
3. **Effects/DOM bindings** are the buckets at the bottom that actually do work
4. When a signal changes, it **opens valves** (inserts computeds into dirtyHeap)
5. Water flows **one height at a time** (stabilize loop)
6. Each bucket only fills if water **actually reaches it** (value change check)
7. Buckets can **change which pipes feed them** (dynamic deps via unlinkSubs)
8. After all water settles, **DOM updates are flushed** in one Arrow batch

The beauty is in the **lazy evaluation**:
- Don't mark everything dirty upfront
- Only process what's actually affected
- Use height ordering to prevent glitches
- Batch all DOM updates into a single message

---

## 13. Implementation Priority

| Phase | Feature | Dependencies |
|-------|---------|--------------|
| **1** | Signal<T>, read(), setSignal() | None |
| **1** | Link struct, link(), unlinkSubs() | Signal |
| **1** | Computed<T>, recompute() | Link |
| **1** | Dirty heap, insertIntoHeap(), deleteFromHeap() | Computed |
| **1** | stabilize() | Dirty heap |
| **2** | Three-color marking (Check/Dirty) | stabilize |
| **2** | Height-based ordering | read() |
| **2** | Dynamic dependency cleanup | recompute() |
| **2** | DomSignalBinding, DOM update queue | Signal |
| **2** | flush_dom_updates() → Arrow batch | DomSignalBinding |
| **3** | Firewall signals | Computed |
| **3** | Effects (side-effect reactions) | Computed |
| **3** | onCleanup() / disposal | Computed |
| **3** | untrack() | Global state |
| **4** | Arena allocation | All core types |
| **4** | Forks (speculative updates) | stabilize |
| **4** | JS ↔ WASM signal bridge | All above |
