# Alpine.js — Deep Analysis

## Overview

Alpine.js is a **lightweight reactive framework** that adds declarative reactivity to HTML via attributes. Think "Vue/Angular directives" but without a build step or component model. It's the UI reactivity layer in the Hotwire stack (used with Livewire).

## Architecture

### Plugin-Based Core

Alpine is designed as a **microkernel + plugins**:

```
packages/
  alpinejs/     → Core (directives, reactivity, lifecycle)
  morph/        → DOM morphing (diff-based)
  csp/          → CSP-safe evaluator (no eval/Function)
  navigate/     → SPA navigation (Turbo-like)
  persist/      → localStorage state persistence
  intersect/    → IntersectionObserver plugin
  focus/        → Focus trap plugin
  collapse/     → Height animation plugin
  mask/         → Input masking
  ui/           → Headless UI components (menu, dialog, tabs, etc.)
  sort/         → Drag-and-drop sorting
  anchor/       → Positioning anchor
  history/      → URL state
  resize/       → ResizeObserver plugin
```

### Core API

```js
Alpine.directive('model', ...)    // x-model
Alpine.directive('for', ...)      // x-for
Alpine.directive('show', ...)     // x-show
Alpine.directive('bind', ...)     // x-bind/:class
Alpine.directive('on', ...)       // x-on/@click
Alpine.directive('effect', ...)   // x-effect
Alpine.directive('if', ...)       // x-if
Alpine.directive('transition', ...) // x-transition
Alpine.magic('watch', ...)        // $watch
Alpine.magic('nextTick', ...)     // $nextTick
Alpine.magic('dispatch', ...)     // $dispatch
Alpine.store('name', data)        // global stores
```

## Reactivity System

### Pluggable Reactivity Engine

Alpine's reactivity is **pluggable** — by default it uses a lightweight custom reactive system, but can swap to Vue's reactivity engine:

```js
Alpine.setReactivityEngine({
  reactive,   // fn: object → reactive proxy
  effect,     // fn: callback → effect (auto-tracks deps)
  release,    // fn: effect → cleanup
  raw         // fn: reactive → plain object
})
```

### How the Default Reactive Works

The default reactive engine (Svelte 3-style) uses `Object.defineProperty` or Proxy to track reads and writes. When a reactive property is read inside an `effect()`, it becomes a tracked dependency. When written, all dependent effects are scheduled for re-execution.

### Scheduler — Microtask-Queued Effect Flushing

```js
// scheduler.js
function queueJob(job) {
  if (!queue.includes(job)) queue.push(job)
  queueFlush()
}

function queueFlush() {
  if (!flushing && !flushPending) {
    flushPending = true
    queueMicrotask(flushJobs)
  }
}
```

All effect updates are **batched into a single microtask**. Multiple state changes in the same tick only trigger one re-render cycle. This is the same approach as Vue 3 and Svelte 5.

### Transactions

```js
Alpine.transaction(async () => {
  startTransaction()    // Block effect scheduling
  data.a = 1
  data.b = 2            // Multiple mutations, no intermediate effects
  await Promise.resolve() // Yield for mutation cleanup
  commitTransaction()   // Flush all effects at once
})
```

This is Alpine's **atomic update** mechanism — during a transaction, effects are deferred until commit.

### Effect Lifecycle

```js
// Each directive (x-text, x-bind, etc.) creates an effect
effect(() => {
  el.textContent = data.message  // Reads data.message → tracked dependency
})
// When data.message changes → effect is scheduled → re-executes

// Effects are bound to DOM elements
elementBoundEffect(el) {
  let effectReference = effect(callback)
  el._x_effects.add(effectReference)
  // On element removal:
  cleanup = () => {
    el._x_effects.delete(effectReference)
    release(effectReference)  // Free the effect from the reactive engine
  }
}
```

### Watch

```js
Alpine.watch(
  () => data.user,           // getter
  (value, previousValue) => { ... }  // callback (queued via queueMicrotask)
)
```

Watches use `JSON.stringify` for deep comparison — a simple but effective approach for detecting changes in nested objects.

## Mutation Observer — DOM Initialization

### The Initialization Pattern

Alpine uses a **single MutationObserver** on `document` (subtree, childList, attributes, attributeOldValue):

```js
observer.observe(document, { subtree: true, childList: true, attributes: true, attributeOldValue: true })
```

### Mutation Batching

```js
function flushObserver() {
  let records = observer.takeRecords()
  queuedMutations.push(() => records.length > 0 && onMutate(records))

  // Key insight: process mutations at the END of the event loop
  queueMicrotask(() => {
    if (queuedMutations.length === queueLengthWhenTriggered) {
      // This is the LAST flush — process all accumulated mutations
      while (queuedMutations.length > 0) queuedMutations.shift()()
    }
  })
}
```

The microtask queue length check is clever — it ensures all mutations from a single "event" (e.g., a morph operation that adds/removes many nodes) are batched together and processed once, rather than triggering multiple re-initializations.

### Mutation Categories

```
onMutate(mutations):
  1. Removed nodes → fire onElRemoved callbacks → cleanup effects
  2. Added nodes (without _x_marker) → fire onElAdded callbacks → initTree()
  3. Attribute changes → cleanup old attribute bindings → apply new bindings
```

The `_x_marker` flag prevents re-initializing nodes that were "moved" (removed and re-added in the same batch).

### mutateDom() — Temporary Observer Suspension

When Alpine itself modifies the DOM, it temporarily suspends the observer to avoid self-triggering:

```js
function mutateDom(callback) {
  stopObservingMutations()
  let result = callback()
  startObservingMutations()
  return result
}
```

### Deferred Mutations

For bulk DOM operations (like SSR hydration), mutations can be collected and processed in a single batch:

```js
deferMutations()
// ... many DOM operations ...
flushAndStopDeferringMutations()  // Process all at once
```

## Directives — How They Work

### x-data — Component Root

```html
<div x-data="{ count: 0 }">
```

Creates a reactive scope. The expression is evaluated and the result is made reactive. The scope is stored on the element via `addScopeToNode()`, creating a **scope chain** that child elements can access.

### x-model — Two-Way Binding

```html
<input x-model="count" x-model.number>
```

The most complex directive:
1. Creates `evaluateGet` and `evaluateSet` functions (compiles expressions)
2. Listens for `input` (text), `change` (select/checkbox/radio) events
3. On event: reads DOM value → writes to reactive data
4. Creates an effect: reads reactive data → updates DOM value
5. Handles modifiers: `.number`, `.boolean`, `.trim`, `.lazy`, `.fill`
6. Special handling for:
   - Checkboxes with array values (toggle in array)
   - Radio buttons (shared name attribute)
   - Select multiple (array of selected options)
   - Form reset listener (re-sync on form reset)
7. `unintrusive` modifier: don't update DOM if element is focused
8. Form submit integration: pending model updates flushed before form submit

### x-for — Reactive Lists

```html
<template x-for="item in items" :key="item.id">
  <li x-text="item.name"></li>
</template>
```

The `x-for` directive is **sophisticated keyed diffing**:

1. Parse expression: `item in items` → `{items: 'items', item: 'item', index: 'index'}`
2. Evaluate items → normalize (Set/Map → Array, number → range)
3. Build **new lookup map** keyed by `:key` expression
4. For each key:
   - If key exists in old lookup → move existing element, refresh scope
   - If key is new → clone template, create reactive scope, initTree()
5. Remove orphaned elements from old lookup
6. **DOM reordering**: If element is out of order, move it with `replaceWith()`

The key insight: `x-for` creates a **separate reactive scope** for each iteration, so each item's bindings are independent. This avoids re-rendering the entire list when one item changes.

### x-bind — Attribute Binding

```html
<div :class="{ active: isActive }" :style="styleObj">
```

Supports:
- **Object syntax**: `{class: 'foo bar'}` → sets attribute
- **Boolean attributes**: `disabled` → add/remove attribute based on truthiness
- **Class binding**: merges with existing classes, removes old bound classes
- **Style binding**: merges with existing inline styles, removes old bound styles
- **Event listeners**: `@click` → addEventListener, cleaned up on element removal

### x-on — Event Handling

```html
<button @click="count++" @click.window="close()" @keyup.enter="submit()">
```

- Supports **modifiers**: `.once`, `.prevent`, `.stop`, `.self`, `.away`, `.window`, `.document`, `.passive`
- **Key filters**: `.enter`, `.tab`, `.esc`, `.space`, `.up`, `.down`, etc.
- **Debouncing/throttling**: `.debounce.500ms`, `.throttle.500ms`
- Event listeners are tracked and cleaned up on element removal

### x-show / x-if — Conditional Rendering

- **x-show**: `display: none` toggle (element stays in DOM)
- **x-if**: actual DOM add/remove (uses `<template>` tag)
- Both support `x-transition` for animations

## Alpine Morph

Alpine has its own morphing library (`packages/morph`), which is **designed to preserve Alpine state**:

### Key Differences from Idiomorph

1. **Key-based matching**: Uses `key` attribute for node identity (not ID set intersection)
2. **Conditional markers**: Supports `[if BLOCK]` / `[if ENDBLOCK]` comment markers for Livewire's conditional rendering
3. **Alpine state cloning**: When morphing, it calls `Alpine.cloneNode(from, to)` which copies `_x_dataStack` and other internal state
4. **Lookahead**: Optional lookahead — if current `from` node doesn't match, scan ahead for an `isEqualNode` match

### Algorithm

```
patch(from, to):
  1. If different tag/type/key → swapElements (replace)
  2. Copy Alpine data stack: to._x_dataStack = from._x_dataStack
  3. Clone Alpine state: Alpine.cloneNode(from, to)
  4. If transitioning, skip attribute patching
  5. Patch attributes
  6. Patch children:
     a. Build key→element map for `from` children
     b. Iterate `to` children:
        - If no `from` sibling: clone and append
        - If keys match: recurse patch
        - If keys differ: move/remove/clone as needed
     c. Remove remaining `from` children
```

### morphBetween (Fragment Morphing)

For morphing between comment markers (used by Livewire for conditional blocks):

```html
<!--morph-start-->
<div>conditional content</div>
<!--morph-end-->
```

The `Block` class wraps start/end comment markers and provides a child iteration interface.

## DOM Direct DOM Operations

### Alpine's Approach to DOM Updates

Alpine does **direct, targeted DOM manipulation** — there is no virtual DOM, no batched updates. Each effect runs its DOM mutations directly:

```js
// x-text effect
effect(() => { el.textContent = data.value })

// x-show effect
effect(() => {
  if (data.show) {
    el.style.display = ''
  } else {
    el.style.display = 'none'
  }
})
```

This is extremely simple but means every state change triggers immediate DOM mutation (batched only at the microtask level by the scheduler).

### Mutation Delegation

All DOM mutations go through `mutateDom()` which temporarily suspends the MutationObserver to prevent self-triggering:

```js
mutateDom(() => {
  el.setAttribute('class', 'active')  // Won't trigger onMutate
})
```

## Stores — Global State

```js
Alpine.store('sidebar', { open: false })
```

Global reactive stores accessible via `$store.sidebar`. Simple key-value registry with reactive values.

## Relevance to Spec-39

### What We Learn

1. **Microtask batching is sufficient** — Alpine's entire reactivity scheduling is a simple queue + queueMicrotask. We don't need a complex scheduler.

2. **Effect-per-directive** — Each binding creates its own effect that runs independently. This is simple and efficient.

3. **MutationObserver for initialization** — One observer for the entire document, batched at microtask boundaries. We should use a similar pattern.

4. **Keyed list diffing** — `x-for` uses key-based matching with element reuse. Our `x-for` equivalent should do the same.

5. **Alpine morph preserves framework state** — When morphing, Alpine copies `_x_dataStack` from old to new. Our morph module should similarly preserve signal subscriptions and controller instances.

6. **x-model is the gold standard** — Two-way binding with form handling, modifiers, reset support. Our input binding should replicate this.

7. **Scoped data chains** — Alpine's scope chain allows child elements to access parent data. Our component model should support a similar hierarchy.

8. **Transaction pattern** — Alpine's transaction defers effects for atomic updates. Our signal system should support this.

### Key Differences from Our Spec-39

- Alpine evaluates expressions at runtime (no compilation). We compile Rust code to WASM.
- Alpine has no component model beyond `x-data` scopes. We have proper Component traits.
- Alpine morph is key-based; Idiomorph (Turbo) is ID-set-based. Both work; keys are simpler but require explicit key attributes.
- Alpine has no server-side communication built in. Livewire provides that layer.
