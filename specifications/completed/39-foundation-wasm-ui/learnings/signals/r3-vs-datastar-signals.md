# R3 vs Datastar Signals — Comparison & Learnings

## Sources

- **R3**: `/home/darkvoid/Boxxed/@dev/repo-expolorations/r3/` — hybrid push-pull system with height-based topological ordering, bucket queue scheduling, three-color marking
- **Datastar**: `/home/darkvoid/Boxxed/@dev/repo-expolorations/src.datastar/datastar/markdown/02-reactive-signals.md` — push-propagation system with Pending/Dirty flags, version-based stale detection, linked-list stack DFS

---

## 1. Architecture Philosophy

| | R3 | Datastar |
|---|---|---|
| **Ordering** | Height-based (topological) — each computed has `height = max(dep heights) + 1` | Flag-based (Pending/Dirty) — no explicit height, uses bit-flag state machine |
| **Scheduling** | Bucket queue — `dirtyHeap[height]` = linked list of computeds at that height | Array queue — `queuedEffects[]` populated by walking subscriber graph |
| **Propagation** | Push during stabilize — process height 0 → max, recompute in order | Push during set — walk subscriber tree iteratively with DFS stack, mark flags |
| **Lazy evaluation** | Pull during read — `updateIfNecessary()` traverses deps recursively | Pull during read — `checkDirty()` traverses deps recursively with stack |
| **Stale detection** | `version_` field on each link — compare against global `version` | Same — `version_` field on each link, global `version` incremented per `startTracking()` |
| **Link structure** | Bidirectional Link in two doubly-linked lists | Same — one Link object in dep's subs AND sub's deps |

**Key difference**: R3 processes computeds in **height order** (level by level). Datastar propagates **depth-first** through the subscriber tree. Both are glitch-free — R3 via height ordering, Datastar via Pending/Dirty flag progression.

---

## 2. Core Data Structures

### ReactiveNode — Identical Concept, Different Naming

| Field | R3 | Datastar |
|-------|----|----------|
| Dependencies head | `deps` | `deps_` |
| Dependencies tail | `depsTail` | `depsTail_` |
| Subscribers head | `subs` | `subs_` |
| Subscribers tail | `subsTail` | `subsTail_` |
| State flags | `flags: ReactiveFlags` | `flags_: ReactiveFlags` |
| Value | `value` | `value_` |

Both use the same bidirectional Link pattern — one Link object serves as a node in two doubly-linked lists simultaneously. Both use tail pointers for O(1) append.

### Flags

| R3 Flag | Datastar Flag | Meaning |
|---------|---------------|---------|
| `None = 0` | `None = 0` | Clean |
| `Check = 1` | `Pending = 32` | May need recomputation |
| `Dirty = 2` | `Dirty = 16` | Must recompute |
| `RecomputingDeps = 4` | `RecursedCheck = 4` | Currently tracking deps |
| `InHeap = 8` | `Watching = 2` | Is an effect |
| — | `Recursed = 8` | Detected diamond |
| — | `Queued = 64` | Effect in queue |
| `Mutable` (implicit — has value) | `Mutable = 1` | Can be written to |

**R3 uses Check/Dirty** — Check means "something upstream changed, verify". **Datastar uses Pending/Dirty** — same concept, same purpose. Both enable short-circuit evaluation.

Datastar adds `Recursed`/`RecursedCheck` for diamond detection during `propagate()`, while R3 detects diamonds implicitly through the height-ordered bucket queue.

### Specialized Nodes

| Type | R3 | Datastar |
|------|----|----------|
| Signal | `Signal<T>` — has `value`, `subs` | `AlienSignal<T>` — has `value_`, `previousValue` |
| Computed | `Computed<T>` — has `fn`, `deps`, `subs` | `AlienComputed<T>` — has `getter`, `value_`, `deps`, `subs` |
| Effect | Implicit (subscriber with no `fn`) | `AlienEffect` — has `fn_`, `deps`, no `subs` |
| Disposal | `disposal: Vec<F>` | Via `unlink()` cascade — computed loses subs → unlinks from deps |

**Key difference**: R3 has explicit disposal callbacks (`onCleanup()`). Datastar relies on the `unlink()` cascade — when a node loses its last subscriber, it recursively unlinks from all dependencies, which naturally releases resources.

---

## 3. The Propagation Algorithms

### R3: Height-Ordered Push

```
setSignal(signal, value):
  if value unchanged: return
  signal.value = value
  for sub in signal.subs:
    insertIntoHeap(sub)  // at sub.height

stabilize():
  for height from 0 to maxDirty:
    for computed in dirtyHeap[height]:
      recompute(computed)

recompute(computed):
  run_disposal()
  new_value = computed.fn()    // tracks new deps via link()
  cleanup_unused_deps()
  if new_value != old_value:
    computed.value = new_value
    for sub in computed.subs:
      insertIntoHeap(sub)
```

**Guarantee**: Computeds are always processed before their dependents because of height ordering. No glitches possible.

### Datastar: Flag-Based DFS

```
signalOper(value):  // SET path
  if value unchanged: return false
  s.value_ = value
  s.flags_ = Mutable | Dirty
  if s.subs_:
    propagate(s.subs_)
  if !batchDepth:
    flush()

propagate(link):
  // Iterative DFS with Stack<Link>
  // Walks subscriber tree, marking Pending/Dirty
  // Handles diamonds via RecursedCheck/Recursed flags
  // Queues effects via notify()

flush():
  while notifyIndex < queuedEffectsLength:
    run(queuedEffects[notifyIndex++])

run(effect):
  if Dirty or (Pending and checkDirty()):
    startTracking()
    effect.fn_()       // tracks new deps via link()
    endTracking()      // removes stale deps
```

**Guarantee**: Effects are only queued when they're Dirty (or Pending confirmed by checkDirty). Diamond dependencies are handled by the `Recursed` flag — when a node is reached through a second path, propagation skips redundant processing.

### Complexity Comparison

| Operation | R3 | Datastar |
|-----------|----|----------|
| `set()` | O(subs) to insert into heap | O(subs) to propagate + DFS |
| `stabilize()` | O(affected) — each computed once | O(affected) — each node visited once per flag state |
| `read()` computed | O(1) if clean, O(getter) if dirty | Same |
| `checkDirty()` | Not needed (height ordering handles it) | O(deps × depth) worst case |
| Memory | O(n) for dirtyHeap + O(links) | O(n) for queuedEffects + Stack + O(links) |
| GC pressure | Low — bucket queue is pre-allocated | Low — Stack is linked list, queue is reused array |

---

## 4. Lazy Evaluation

### R3: `updateIfNecessary()` — Pull-Based

```
updateIfNecessary(computed):
  if flags has Check:
    for dep in computed.deps:
      if dep is computed: updateIfNecessary(dep)  // recursive pull
      if flags has Dirty: break  // early exit
  if flags has Dirty:
    recompute(computed)
  flags = None
```

**Key**: Only called during `read()` — computed values are never proactively updated. If a computed is never read, it never re-evaluates even if its dependencies changed.

### Datastar: `checkDirty()` — Lazy Walk

```
checkDirty(link, sub):
  // Iterative DFS with Stack<Link>
  // Walks dependency tree to confirm if any source changed
  // Returns true if any dependency actually changed value
```

**Key**: Only called when a computed is Pending but not Dirty. If Dirty, re-evaluation happens immediately without the walk. If clean (neither), the cached value is returned instantly.

### Short-Circuit Behavior

Both systems support short-circuit evaluation:

```
// R3:
const c = computed(() => {
  if (!read(a)) return "early exit";  // a changed but result same
  read(b);  // only evaluated if a is truthy
});

// Datastar:
computed(() => {
  if (read(cond)) return read(a);  // b not read, not a dependency
  return read(b);
});
```

When `a` changes but `c`'s early exit produces the same result, downstream subscribers aren't notified. When `cond` changes to take the `b` branch, `a` is unlinked and `b` is linked automatically.

---

## 5. Dynamic Dependencies

Both systems handle dynamic dependencies identically through the version-based stale detection:

1. `startTracking()` increments global `version`
2. During re-evaluation, each `read()` creates/updates a link with the current `version`
3. `endTracking()` walks from `depsTail_.nextDep_` — any link with an older `version` is stale and gets `unlink()`ed

The result: dependencies that are no longer read during execution are automatically removed. Dependencies that are newly read are automatically added.

**This is identical in both systems.** The same `Link` structure, the same `version` field, the same `startTracking/endTracking` pattern.

---

## 6. Diamond Dependencies

### R3: Implicit via Height Ordering

```
     s (h=0)
    / \
   a   b (h=1)
    \ /
     c (h=2)
```

When `s` changes:
1. `insertIntoHeap(a)` → bucket[0], `insertIntoHeap(b)` → bucket[0]
2. `stabilize()`: process height 0 → recompute `a` and `b` (order: insertion order)
3. `a` recomputes → value changed → `insertIntoHeap(c)` → bucket[1]
4. `b` recomputes → value changed → `insertIntoHeap(c)` → already in heap, skip
5. `stabilize()`: process height 1 → recompute `c` (once, with both `a` and `b` fresh)

**No special diamond detection needed.** Height ordering guarantees `c` runs after both `a` and `b`, and the `InHeap` flag prevents double-insertion.

### Datastar: Explicit via Recursed Flags

```
     s → a → c → effect
       → b →/
```

When `s` changes:
1. `propagate()` walks: `a` marked Pending, then `c` marked Pending via `a`'s path
2. Continue walking: `b` marked Pending, then `c` reached again
3. `c` has `RecursedCheck` (from first path) — now both `RecursedCheck` AND `Recursed` are set
4. `isValidLink()` confirms the link is still valid → skip redundant processing
5. `notify()` queues the effect (only once, thanks to `Queued` flag)

**Diamond detection is explicit.** The `RecursedCheck`/`Recursed` flag combination marks that a node has been reached through multiple paths.

---

## 7. Batching

| | R3 | Datastar |
|---|----|----------|
| Mechanism | `stabilize()` processes all dirty computeds | `beginBatch()` / `endBatch()` with `batchDepth` counter |
| Multiple sets | Each `setSignal` inserts into heap, `stabilize` runs once | Each `set` propagates, `flush` deferred until batch ends |
| Coalescing | Heap deduplicates (InHeap flag) | Queue deduplicates (Queued flag) |
| DOM updates | Flush after stabilize | Dispatch DOM event after endBatch |

Both batch identically: multiple signal changes in one batch cause only one round of effect execution.

---

## 8. Memory Management

### R3: Explicit Disposal

```rust
onCleanup(|| {
  // cleanup code
});
// Runs before recompute() and when computed is unwatched
```

R3 has explicit disposal callbacks. When a computed is unwatched (no subscribers), disposal runs. This is important for side effects like timers, subscriptions, and DOM event listeners.

### Datastar: Cascade Unlink

```
unlink(link) when computed loses last subscriber:
  if computed has 'getter':
    computed.flags_ = Mutable | Dirty
    for dep in computed.deps:
      unlink(dep)  // recursive cascade
  if dep has 'fn_' but no getter (effect):
    effectOper(dep)  // dispose the effect
```

Datastar relies on recursive unlinking. When a computed loses its last subscriber, it unlinks from all its dependencies, which may cascade further up the graph. Effects are disposed via `effectOper()`, which unlinks all dependencies and zeroes flags.

**Tradeoff**: R3's explicit disposal is more predictable — you know exactly what will run. Datastar's cascade is more automatic — resources release when no one needs them, but the cascade can be deep and expensive.

---

## 9. Deep Reactivity

### R3: No Built-in Deep Reactivity

R3's `Signal<T>` holds a single value of type `T`. For nested objects, you'd create separate signals for each property. There's no automatic deep tracking.

### Datastar: Proxy-Based Deep Reactivity

```typescript
const root = deep({})  // Proxy-wrapped object

root.user.name = "Alice"  // Auto-creates signals for user, user.name
root.items.push("new")    // Keys signal increments, reactive
```

Datastar's `deep()` wraps objects/arrays in Proxies that:
- Auto-create signals for new properties
- Track array operations via a `keys` signal
- Deep-merge objects on set
- Dispatch patches on every mutation

**Tradeoff**: Proxy-based deep reactivity is convenient but adds overhead. For WASM-UI, we'd need a different approach (path-based store, since Rust doesn't have Proxies).

---

## 10. Comparison with Other Systems

| System | Ordering | Marking | Scheduling | Lazy Eval | Dynamic Deps | Deep Reactivity |
|--------|----------|---------|------------|-----------|--------------|-----------------|
| **R3** | Height buckets | Check/Dirty | Bucket queue | `updateIfNecessary()` | Version-based stale links | No |
| **Datastar** | Flag DFS | Pending/Dirty + Recursed | Array queue + Stack | `checkDirty()` | Version-based stale links | Yes (Proxy) |
| **Svelte 5** | Effect tree | DIRTY/MAYBE_DIRTY/CLEAN | Batch traversal | `is_dirty()` | Write version comparison | Yes (Proxy) |
| **SolidJS** | Queue + generations | Sync/Dirty | Queue | Lazy memo read | Version comparison | No |
| **Vue 3** | Queue | Dirty | Queue | Lazy computed | Dependency tracking | Yes (Proxy) |

---

## 11. Design Decisions for Our WASM-UI

### 11.1 Use Height-Based Ordering (R3 approach)

**Why**: Height-based ordering is simpler than Datastar's flag state machine for diamond detection. The bucket queue is a single loop: process height 0, then 1, then 2... No recursive DFS, no Recursed flags, no `isValidLink` checks.

**For Rust**: The bucket queue maps naturally to `Vec<Vec<*mut Computed>>`. Each height is a linked list within its bucket.

### 11.2 Use Check/Dirty Flags (both systems agree)

Both R3 and Datastar use a three-state system: Clean, "might be dirty", and "definitely dirty". This is the right abstraction — it enables short-circuit evaluation and lazy updates.

### 11.3 Use Version-Based Stale Detection (both systems agree)

Both systems use a global `version` counter incremented per evaluation, with `version_` on each link. This is the simplest way to detect stale dependencies. No full rebuild of the dependency graph needed.

### 11.4 Explicit Disposal (R3 approach)

For WASM-UI, explicit disposal is critical. DOM bindings (signal → DOM update) need to be cleaned up when elements are removed. R3's `onCleanup()` pattern is cleaner than Datastar's cascade unlink for this purpose.

### 11.5 No Proxy-Based Deep Reactivity

WASM doesn't have JavaScript Proxies. For nested state, use a path-based store:

```rust
store.set("user.name", "Alice");
store.get::<String>("user.name");
```

This maps naturally to our Arrow batch format — paths are strings, values are typed.

But we should do this on the javascript side, so we can create the proxies on the javascript side for direct, specific signal updates communicated by the wasm side and from the js to the wasm side which could just create a hashmap/dict of existing properties with the newly changed one and the rust wasm side updates.

But i also believe updates should go 1 way, actions should trigger the wasm side or server side and then wasm or server communicates the signal updates and not two way reducing complexity.

### 11.6 Signal-to-DOM Bridge

The critical difference: R3 has no built-in DOM integration, and Datastar binds via attribute plugins (`data-text`, `data-bind`). Our WASM-UI needs a direct bridge:

```rust
// In Rust:
let binding = signal.bind_dom(NodeId(42), DomUpdateType::TextContent);

// When signal changes, during stabilize():
// → enqueue DOM update → Arrow batch → JS applies to real DOM
```

This is a new layer that neither R3 nor Datastar has — the DOM binding is managed by the WASM signal system, not by a client-side plugin.

**NOTE FROM USER**: If wasm and server only ever returns signals (patches) either as dom updates, json dom updates then js just applies the dom updates and never really cares about the signals since what it gets is already computed changes it needs to merge/morph to the DOM. We could use the js side signals for client only things that the js benefits from.

### 11.7 Batching is Essential

Both systems agree: batch multiple signal changes into one effect flush cycle. For WASM-UI, this means:
- Multiple signal sets → one `stabilize()` → one Arrow batch → one JS `host_batch_apply()` call
- This minimizes WASM ↔ JS crossing overhead

### 11.8 What We DON'T Need from Either

- **Datastar's expression compiler** — We compile Rust to WASM, not evaluate string expressions at runtime
- **R3's firewall signals** — Component ownership can be handled via separate signal scopes, not a special signal type
- **Datastar's `deep()` Proxy** — Path-based store is simpler and WASM-compatible
- **R3's `markHeap()`** — This is only needed for pull-based updates in `read()`. With height ordering, stabilization happens automatically.

**NOTE**: The firewall signal is interesting - is there anything stopping us from adding it for completeness and users can use it or not.
---

## 12. Summary

R3 and Datastar are two implementations of the same fundamental ideas:

| Shared Concept | R3 Name | Datastar Name |
|---------------|---------|---------------|
| Dependency graph nodes | Signal, Computed | AlienSignal, AlienComputed |
| Bidirectional edges | Link | Link |
| Dirty detection | Check vs Dirty | Pending vs Dirty |
| Stale link detection | `version_` vs global `version` | Same |
| Dep discovery during evaluation | `context` global | `activeSub` global |
| Stale dep cleanup | `cleanupUnusedDeps()` | `endTracking()` |
| Multiple changes → one flush | Heap + `InHeap` flag | Queue + `Queued` flag |
| Nested batch support | Not explicitly modeled | `beginBatch()` / `endBatch()` |

**Where they diverge**:
- R3 uses **height ordering** (simpler, no diamond detection needed)
- Datastar uses **DFS with flag state machine** (more complex, but handles more edge cases)
- R3 has **explicit disposal** (cleaner for side effects)
- Datastar has **cascade unlink** (more automatic, but deeper call chains)
- Datastar has **Proxy-based deep reactivity** (convenient, but not WASM-compatible)

For our WASM-UI, the **R3 approach is the better fit**: height-based ordering, explicit disposal, no Proxy, and the version-based stale detection that both systems share.

References: 
1. specifications/39-foundation-wasm-ui/learnings/signals/signal.md
2. specifications/39-foundation-wasm-ui/learnings/signals/datastar-signals-sse.md
3. specifications/39-foundation-wasm-ui/learnings/signals/svelte-ui.md
