# Livewire — Deep Analysis

## Overview

Livewire is a **full-stack reactive component framework** for Laravel. Components are written in PHP (Blade templates), and Livewire handles the client-server synchronization: user interactions trigger server roundtrips, the server re-renders the component, and Livewire morphs the DOM to match.

## Architecture: Two-Layer State Model

### Canonical vs Ephemeral vs Reactive Data

Livewire maintains **three copies** of component state on the client:

```js
this.canonical = extractData(snapshot.data)   // Last known SERVER state
this.ephemeral = extractData(snapshot.data)   // Most current state (user-manipulable)
this.reactive = Alpine.reactive(this.ephemeral) // Reactive proxy of ephemeral
```

- **Canonical**: The source of truth — what the server last sent. Used for diffing.
- **Ephemeral**: The live state — user mutations happen here. Modified by x-model, etc.
- **Reactive**: Alpine reactive proxy of ephemeral — drives DOM updates.

### Request/Response Cycle

```
1. User interacts (click, input, etc.)
2. Livewire collects diffs: diff(this.canonical, this.ephemeral)
   → Only sends CHANGED properties to server
3. Server processes action, returns:
   - New snapshot (full state)
   - New HTML (re-rendered Blade)
   - Effects (listeners, redirects, dispatches)
4. Client merges new snapshot:
   - Applies changes surgically to preserve client-side ephemeral state
   - Dirty = diff(oldCanonical, newCanonical)
5. Alpine.morph(oldHTML, newHTML) → DOM update
6. Effects processed (dispatch events, navigate, etc.)
```

## Component System

### Component Class (Client-Side)

```js
class Component {
  constructor(el) {
    this.el = el                           // Root DOM element
    this.id = el.getAttribute('wire:id')   // Unique component ID
    this.key = el.getAttribute('wire:key') // Stable key for morphing
    this.snapshot = JSON.parse(el.getAttribute('wire:snapshot'))
    this.effects = JSON.parse(el.getAttribute('wire:effects'))
    this.canonical = extractData(snapshot.data)
    this.ephemeral = extractData(snapshot.data)
    this.reactive = Alpine.reactive(this.ephemeral)
    this.$wire = generateWireObject(this, this.reactive)
    el.$wire = this.$wire                  // $wire on root element
  }
}
```

Components are stored on the DOM: `el.__livewire = component`. Nested components are discovered via `[wire:id]` selectors.

### Children/Parent Hierarchy

```js
get children() {
  this.el.querySelectorAll('[wire\\:id]').forEach(el => {
    let parentComponentEl = el.parentElement.closest('[wire\\:id]')
    if (parentComponentEl === this.el) children.push(el.__livewire)
  })
  return children
}
```

Components form a tree — each component knows its direct children by DOM proximity.

## $wire Object

The `$wire` object is a **reactive proxy** to the component's state. It exposes:

- Property access: `$wire.count` → reads/writes `this.reactive.count`
- Method calls: `$wire.increment()` → triggers server action
- Special methods: `$wire.$refresh()`, `$wire.$set()`, `$wire.$toggle()`, etc.

## Morphing — The Core DOM Update Mechanism

Livewire uses **Alpine's morph** (not Idiomorph) for DOM updates. This is a critical choice because Alpine's morph preserves Alpine.js state.

### Morph Configuration

```js
function getMorphConfig(component) {
  return {
    key: (el) => el.hasAttribute('wire:id') ? el.getAttribute('wire:id')
      : el.hasAttribute('wire:key') ? el.getAttribute('wire:key')
      : el.id,
    updating: (el, toEl, childrenOnly, skip, skipChildren, skipUntil) => {
      // Skip fragment markers
      if (isStartFragmentMarker(el) && isStartFragmentMarker(toEl)) { ... }

      // Bypass DOM diffing for children
      if (el.__livewire_replace === true) { el.innerHTML = toEl.innerHTML; }
      // Completely bypass DOM diffing
      if (el.__livewire_replace_self === true) { el.outerHTML = toEl.outerHTML; return skip(); }
      // Skip this element entirely
      if (el.__livewire_ignore === true) return skip()
      // Skip only attributes (keep children)
      if (el.__livewire_ignore_self === true) childrenOnly()
      // Skip children (update this element only)
      if (el.__livewire_ignore_children === true) return skipChildren()
      // Different component? Skip
      if (isComponentRootEl(el) && el.getAttribute('wire:id') !== component.id) return skip()
    },
    lookahead: false,
  }
}
```

### Key-Based Matching

Livewire's morph uses a **3-tier key system**:

1. `wire:id` — component identity (highest priority)
2. `wire:key` — element identity (for lists/conditionals)
3. `id` — standard HTML ID (fallback)

### Child Component Preservation

Before morphing, Livewire **clones existing child components**:

```js
// Find all child components in the OLD DOM
el.querySelectorAll('[wire\\:id]').forEach(component => {
  existingComponentsMap[component.getAttribute('wire:id')] = component
})

// In the NEW DOM, replace placeholders with cloned components
to.querySelectorAll('[wire\\:id]').forEach(child => {
  if (child.hasAttribute('wire:snapshot')) return // New component, don't clone
  let existingComponent = existingComponentsMap[child.getAttribute('wire:id')]
  if (existingComponent) {
    child.replaceWith(existingComponent.cloneNode(true))
  }
})
```

This ensures child components **don't lose state** even if a parent re-render doesn't include `wire:key` attributes.

### Fragment Morphing

For conditional blocks (like `@if` in Blade), Livewire uses comment markers:

```html
<!--livewire:if-abc123-->
<div>conditional content</div>
<!--/livewire:if-abc123-->
```

`morphBetween()` morphs only the content between the markers, allowing conditionals to appear/disappear without re-rendering the entire component.

### Transition Integration

Morph operations are wrapped in `transitionDomMutation()` which checks for `[wire:transition]` elements and applies view transitions:

```js
await transitionDomMutation(fromEl, toEl, () => {
  Alpine.morph(el, to, config)
}, transitionOptions)
```

## Dirty Detection — What Gets Sent to Server

```js
getUpdates() {
  let propertiesDiff = diffAndConsolidate(this.canonical, this.ephemeral)
  return this.mergeQueuedUpdates(propertiesDiff)
}
```

The diff algorithm compares canonical (server state) against ephemeral (current state) and only sends **changed properties**. This is a deep diff that handles nested objects and arrays.

### Queued Updates

For programmatic state changes:

```js
queueUpdate(propertyName, value) {
  this.queuedUpdates[propertyName] = value
}
```

Queued updates take priority over ephemeral diffs — they're sent to the server first.

### Snapshot Merging

When the server responds:

```js
mergeNewSnapshot(snapshotEncoded, effects, updates = {}) {
  let oldCanonical = deepClone(this.canonical)
  let updatedOldCanonical = this.applyUpdates(oldCanonical, updates)
  let newCanonical = extractData(snapshot.data)
  let dirty = diff(updatedOldCanonical, newCanonical)

  this.canonical = extractData(snapshot.data)

  // Apply changes surgically to preserve client-side ephemeral state
  changes.forEach(key => {
    dataSet(this.reactive, key, dataGet(newData, key))
  })

  // Apply removals in reverse order (array indices stay valid)
  removals.sort((a, b) => bNum - aNum).forEach(key => {
    dataDelete(this.reactive, key)
  })

  return dirty
}
```

## Islands Architecture

Livewire v4 introduced **islands** — independently updateable regions within a component:

```js
get islands() {
  return this.snapshot.memo.islands
}
```

Islands use `morphFragment()` to update only a portion of the DOM (between comment markers) without re-rendering the entire component.

## Effects System

After every server response, Livewire processes effects:

```js
processEffects(effects) {
  trigger('effects', this, effects)
  trigger('effect', { component, effects, cleanup, request })
}
```

Effects include:
- Event dispatches (`$dispatch`)
- Navigation redirects
- Script execution
- Listener registration
- Flash messages

## Directives

Livewire extends Alpine with custom directives:

| Directive | Purpose |
|-----------|---------|
| `wire:model` | Two-way binding + debounced server sync |
| `wire:click` | Server action on click |
| `wire:submit` | Server action on form submit |
| `wire:poll` | Polling at interval |
| `wire:loading` | Show/hide during request |
| `wire:offline` | Detect offline state |
| `wire:transition` | Animate appearance/disappearance |
| `wire:key` | Stable identity for morphing |
| `wire:id` | Component identity |
| `wire:ignore` | Skip morphing for this element |
| `wire:ignore.self` / `wire:ignore.children` | Partial skip |
| `wire:replace` / `wire:replace.self` | Bypass morph, direct replacement |

## Relevance to Spec-39

### What We Learn

1. **Three-layer state model** — canonical/ephemeral/reactive is a clean pattern for server-synced state. The diff between canonical and ephemeral determines what to send to the server.

2. **Child component preservation** — cloning child components before morphing prevents state loss. Our framework should do the same for nested WASM components.

3. **Morph with framework state** — Alpine's morph copies `_x_dataStack`. Our morph should copy signal subscriptions and controller instances.

4. **Dirty detection via diff** — diffing old vs new state to determine what to send is efficient. Our Arrow batch format should only include changed values.

5. **Fragment morphing** — morphBetween for conditional blocks is useful for partial updates. Our framework should support targeted region updates.

6. **wire:ignore pattern** — the ability to skip morphing for specific elements is important for third-party libraries (charts, maps, etc.).

7. **Effects as side effects** — separating DOM updates from side effects (navigation, events, etc.) is clean. Our SSE stream actions should follow this pattern.

8. **Key-based morph matching** — `wire:id` > `wire:key` > `id` is a sensible priority system.

### What Livewire Doesn't Do

- No fine-grained DOM updates — always re-renders full HTML from server and morphs
- No client-side routing (relies on Laravel routing)
- No compiled templates — Blade templates are PHP, compiled server-side
- No WASM — everything is PHP + Alpine.js

### What Our Spec-39 Should Incorporate

- The canonical/ephemeral state model for server-synced reactivity
- Component preservation during morph (clone before morph)
- Fragment/island-style partial updates
- The ignore/replace directives for morph control
- Effects separation from DOM updates
- Key-based morph matching with priorities
