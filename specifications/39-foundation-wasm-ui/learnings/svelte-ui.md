# Svelte (Rune Mode / Svelte 5) — Deep Analysis of Signals & DOM Efficiency

## Overview

Svelte 5 replaced the old compiler-driven reactive statements (`$:`) with a **runtime signal system** (`$state`, `$derived`, `$effect`). This document focuses on the **signal runtime** and how it makes DOM change handling efficient — not on the compiler itself.

## Signal Types

Svelte's runtime has three core reactive primitives:

### Source (Writable Signal)

```js
// Created via $state() or internal source()
const count = $state(0)
```

A Source holds a value (`signal.v`) and tracks which reactions depend on it (`signal.reactions`). When set, it marks all dependent reactions as dirty.

### Derived (Computed Signal)

```js
// Created via $derived()
const doubled = $derived(count * 2)
```

A Derived has no direct value — it computes its value by running a function. It tracks its own dependencies and can be **disconnected** (removed from the graph) when no one reads it, then **reconnected** when read again.

### Effect (Side-Effect Signal)

```js
// Created via $effect()
$effect(() => {
  document.body.textContent = `Count: ${count}`
})
```

An Effect runs a function that performs side effects (DOM mutations). It tracks dependencies automatically — every signal read inside the effect becomes a dependency.

## Core Runtime — How It Works

### The `get()` Function — Dependency Tracking

The `get()` function is the heart of the system. Every time a signal's value is read, `get()` is called:

```js
export function get(signal) {
  // 1. Register dependency on the active reaction
  if (active_reaction !== null && !untracking) {
    if (current_sources === null || !includes.call(current_sources, signal)) {
      var deps = active_reaction.deps

      // Fast path: if deps haven't changed, increment skipped_deps (no GC!)
      if (new_deps === null && deps !== null && deps[skipped_deps] === signal) {
        skipped_deps++
      } else if (new_deps === null) {
        new_deps = [signal]
      } else {
        new_deps.push(signal)
      }

      // Add this reaction to the signal's subscribers
      (signal.reactions ??= []).push(active_reaction)
    }
  }

  // 2. For deriveds, compute if dirty
  if (is_derived) {
    if (is_dirty(derived)) {
      update_derived(derived)
    }
  }

  // 3. Return current value
  return signal.v
}
```

**Key optimization**: If the reaction reads its dependencies in the **same order** as the previous run, no new dependency array is allocated — `skipped_deps` is simply incremented. This avoids GC pressure for stable dependency graphs.

### The `set()` Function — Marking Dirty

```js
export function set(signal, value) {
  if (value !== signal.v) {
    signal.v = value
    signal.wv = increment_write_version()  // Write version increments

    // Capture value in current batch
    current_batch?.capture(signal, old_value)

    // Mark all dependent reactions dirty
    mark_reactions(signal, DIRTY)
  }
}
```

When a signal is set:
1. The value is updated
2. **Write version** increments (used for dirty detection)
3. The value is captured in the current batch (for undo/redo support)
4. All dependent reactions are marked dirty

### Dirty Detection — Lazy Evaluation

Svelte uses a **lazy evaluation** model with three statuses:

```
CLEAN       → Not dirty, no need to re-execute
MAYBE_DIRTY → Dependencies may have changed, need to check
DIRTY       → Definitely dirty, must re-execute
```

The `is_dirty()` function walks the dependency tree recursively:

```js
export function is_dirty(reaction) {
  if (flags & DIRTY) return true

  if (flags & MAYBE_DIRTY) {
    for (const dependency of dependencies) {
      if (is_dirty(dependency)) {
        update_derived(dependency)  // Recompute derived
      }
      if (dependency.wv > reaction.wv) {
        return true  // Dependency was written after this reaction ran
      }
    }
    // No dependencies changed → mark clean
    set_signal_status(reaction, CLEAN)
  }
  return false
}
```

**Key insight**: `MAYBE_DIRTY` reactions only check their dependencies' **write versions** — if no dependency was written since the reaction last ran, it's still clean. This avoids unnecessary re-execution.

### Version Numbers — The Core of Efficiency

Two version counters drive the system:

- **`write_version`** — increments every time any signal is set
- **`read_version`** — increments every time a reaction runs

Each signal has:
- **`wv`** (write version) — the `write_version` when the signal was last set
- **`rv`** (read version) — the `read_version` when the signal was last read by this reaction

A reaction is dirty if **any dependency's `wv` > reaction's `wv`**. This is a simple integer comparison — no callbacks, no queues, no equality checks.

## Batch System — Efficient Flush

### The Batch Class

Svelte 5 introduces a **Batch system** for grouping state changes:

```js
class Batch {
  current = new Map()   // New values for signals changed in this batch
  previous = new Map()  // Old values (for undo/rollback)
  #roots = []           // Root effects to flush
  #commit_callbacks = new Set()  // DOM commit callbacks
}
```

When multiple signals are set in the same tick, they're all captured in the current batch:

```js
capture(source, value) {
  if (!this.previous.has(source)) {
    this.previous.set(source, value)  // Save old value
  }
  this.current.set(source, source.v)   // Save new value
}
```

### Effect Traversal

The batch traverses the effect tree once, collecting effects into two buckets:

```js
#traverse(root, effects, render_effects) {
  while (effect !== null) {
    if (is_branch && is_skippable_branch) { skip; continue }

    if (!skip && effect.fn !== null) {
      if (is_branch) {
        effect.f ^= CLEAN  // Toggle branch status
      } else if ((flags & EFFECT) !== 0) {
        effects.push(effect)  // Queue for flush
      } else if (is_dirty(effect)) {
        update_effect(effect)  // Execute immediately
      }

      // Descend into children
      if (child !== null) { effect = child; continue }
    }
    // Move to next sibling or parent
  }
}
```

**Key optimization**: CLEAN branch effects are skipped entirely — no traversal needed. Only dirty branches are visited.

### Flush Cycle

```
set() → mark_reactions() → schedule_effect() → Batch.ensure() → queue_micro_task(flush)
                                                                    ↓
flush() → #process() → #traverse() → collect effects → flush_queued_effects()
```

1. `set()` marks reactions dirty
2. `Batch.ensure()` creates/activates the current batch
3. `queueMicrotask()` schedules flush
4. `flush()` → `#process()` → `#traverse()` collects dirty effects
5. `flush_queued_effects()` executes them in order (ancestor → descendant)

### Commit Callbacks

After effects are flushed, commit callbacks are executed:

```js
for (const fn of this.#commit_callbacks) fn(this)
```

This is where **DOM mutations happen** — effects append/remove elements, and commit callbacks finalize the changes.

## Effect Hierarchy

Svelte organizes effects into a **tree structure**:

```
RootEffect
  └── BlockEffect (if/each/await blocks)
        └── RenderEffect (DOM bindings, text updates)
        └── Effect ($effect())
        └── BranchEffect (conditional branches)
```

- **RootEffect**: Top-level effect for a component
- **BlockEffect**: Controls block visibility (if/each/await)
- **RenderEffect**: Direct DOM mutations (text updates, attribute changes)
- **Effect**: User-defined side effects
- **BranchEffect**: Conditional rendering branches

When a block effect becomes CLEAN, all its children are skipped — this is the primary efficiency gain. Entire subtrees of the DOM are untouched if their state didn't change.

## DOM Update Efficiency

### Text Node Updates

```js
export function set_text(text, value) {
  var str = value == null ? '' : typeof value === 'object' ? `${value}` : value
  if (str !== (text.__t ??= text.nodeValue)) {
    text.__t = str
    text.nodeValue = `${str}`
  }
}
```

The `__t` cache prevents unnecessary DOM writes — if the string hasn't changed, `nodeValue` isn't touched. This avoids layout thrashing.

### Event Delegation

Svelte uses **global event delegation** — not per-element listeners:

```js
// Single listener per event type on the mount target + document
for (const node of [target, document]) {
  node.addEventListener(event_name, handle_event_propagation, { passive })
}
```

When an event fires, `handle_event_propagation` walks up the DOM tree looking for registered handlers. This means:
- **O(1)** listeners regardless of how many elements have event handlers
- No listener management when elements are added/removed
- Events from dynamically moved elements (manual portals) are still caught

### Branch Skipping

When an `{#if}` block's condition changes:

```js
// Block effect becomes CLEAN → children are skipped during traversal
if (is_branch && (flags & CLEAN) !== 0) {
  skip = true
  continue
}
```

The entire subtree of effects under a CLEAN branch is **never traversed**. This is how Svelte achieves O(1) updates for stable UI — if nothing in a branch changed, the branch isn't touched.

### Each Block (List Rendering)

Svelte's each block uses **key-based matching** with an efficient lookup:

1. Old items are stored in a key→effect map
2. New items are matched by key
3. Matched items: refresh scope, check if effect is dirty
4. New items: create effect, mount DOM
5. Removed items: destroy effect, remove DOM

The key insight: **the effect tree IS the list state**. Each list item is a BlockEffect. Reordering list items means reordering effects in the tree, not re-rendering.

## Forks — Speculative State Changes

Svelte 5 introduced **forks** for speculative state changes:

```js
const fork = fork(() => {
  state.count = state.count + 1  // Speculative change
})

// Later, if confirmed:
await fork.commit()  // Apply to DOM

// Or discard:
fork.discard()  // Undo changes, don't update DOM
```

This enables features like prefetching data on hover — the state changes are evaluated but not applied to the DOM until confirmed.

### How Forks Work

1. Changes are captured in a separate `Batch`
2. The batch's `current` map stores new values
3. The batch's `previous` map stores old values (for rollback)
4. Effects are scheduled in the fork's batch, not the main batch
5. On commit: values are applied, effects are flushed
6. On discard: values are reverted, batch is deleted

This is **time-travel** — multiple batches can coexist, and effects see the values from their own batch via `batch_values` map override.

## Dependency Graph — Connection/Disconnection

Svelte's derived signals can **disconnect** from the graph when no one reads them:

```js
// In remove_reaction():
if (reactions === null && (dependency.f & DERIVED) !== 0) {
  // No one reads this derived → disconnect it
  derived.f ^= CONNECTED
  derived.f &= ~WAS_MARKED

  // Disconnect its own dependencies
  remove_reactions(derived, 0)

  // Freeze any effects inside this derived
  freeze_derived_effects(derived)
}
```

When the derived is read again, it **reconnects**:

```js
// In get() for derived:
if ((derived.f & CONNECTED) === 0 && active_reaction !== null) {
  unfreeze_derived_effects(derived)
  reconnect(derived)
}
```

This is **lazy evaluation** at the graph level — unused computations don't run, and disconnected deriveds don't receive notifications.

## Relevance to Spec-39

### What We Learn

1. **Write version + dirty flags** — The combination of `wv` (write version) and `DIRTY/MAYBE_DIRTY/CLEAN` flags is an incredibly efficient dirty detection system. Our signal system should use version numbers, not callback queues.

2. **Dependency ordering optimization** — If dependencies are read in the same order, no array allocation is needed (`skipped_deps++`). This is a subtle but important optimization for GC.

3. **Batch-based flushing** — All state changes in a tick are batched, effects are collected in a single tree traversal, then flushed. Our JS runtime should do the same.

4. **Branch skipping** — CLEAN branch effects skip entire subtrees. This is the primary efficiency mechanism. Our component tree should support this.

5. **Event delegation** — One listener per event type, not per element. Our JS runtime should delegate events at the root.

6. **Text value caching** — Cache the last string value on the DOM node (`__t`) to avoid unnecessary `nodeValue` writes.

7. **Fork system** — Speculative state changes are powerful for prefetching. Our signal system could support this pattern.

8. **Effect teardown tracking** — Effects track their teardown functions, which are called before re-execution. This prevents resource leaks.

9. **Lazy derived evaluation** — Deriveds only compute when read and dirty. They disconnect when unreferenced. This prevents wasted computation.

### Key Differences from Other Frameworks

- **No virtual DOM** — Svelte compiles templates to direct DOM operations inside effects. No diffing at all.
- **Compile-time knowledge** — The compiler knows the exact structure of the component, so it generates precise effect trees. We're compiling Rust to WASM, so we have the same advantage.
- **Version-based dirty checking** — Unlike Vue's scheduler or Alpine's microtask queue, Svelte uses integer version comparisons.

### What Our Spec-39 Should Incorporate

- Version-based dirty checking (write_version + read_version)
- Effect tree with branch effects for O(1) subtree skipping
- Batch system for atomic state updates
- Event delegation at the root level
- Text value caching to avoid unnecessary DOM writes
- Fork system for speculative updates
- Lazy derived evaluation with connect/disconnect
- Key-based list item matching with effect reuse
