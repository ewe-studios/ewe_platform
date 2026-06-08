# 019 — Scoped Script & Style Tags: compile-time transformation

**Date:** 2026-06-08
**Status:** Resolved

### Decision

Scoped `<script>` and `<style>` tags inside `html!` are transformed at compile-time by the macro, not handled at runtime.

### `:parent` pseudo-class

A compile-time custom pseudo-class that gets replaced with the parent element's id or class:

```html
<div id="menu-tabs">
  <style scoped primal:style>
    :parent { background: white; }
    :parent:hover { opacity: 0.8; }
    :parent .title { font-size: 20px; }
  </style>
</div>
<!-- Transformed: -->
<style>
  #menu-tabs { background: white; }
  #menu-tabs:hover { opacity: 0.8; }
  #menu-tabs .title { font-size: 20px; }
</style>
```

`:parent` is parsed by `lightningcss` as a valid pseudo-class (unknown but accepted), then replaced at compile-time. If the parent has no id, uses its first class.

### Smart prefixing (not blind)

The `lightningcss` AST allows us to inspect each selector and decide:

1. **`:parent`** → replaced with parent id/class
2. **Selectors matching parent's id/classes** → left as-is (already scoped)
3. **Nested CSS (`&`)** → inherits scoping from parent selector, no work needed
4. **Other selectors** → prepended with parent id/class
5. **Custom properties (`--primary`)** → left as-is, not selectors

### Scoped Script Tags

```html
<div id="menu-tabs">
  <script scoped primal:script>
    function(scope) {
      let targets = scope.targets();
      primal.on(targets, "click", () => { ... });
    }
  </script>
</div>
```

**Script format**: Must expose a `function(scope){...}` body. The JS runtime extracts `script.textContent` and hydrates it via `new Function(...)` — not an IIFE in the source.

**`scope` object** — created by the runtime:

| Method | Returns |
|--------|---------|
| `scope.targets()` | Array of target elements (parent, or elements matching `scoped="selector"`) |
| `scope.parent()` | The actual parent DOM node containing the script |
| `scope.querySelector(sel)` | Runs `querySelector` scoped to `scope.parent()` |
| `scope.addEvent(target, eventType, handler)` | Attaches event listener to target with auto-cleanup |

**`primal` global** — helper methods available in scoped scripts:
- `primal.on(target, eventType, handler)` — wires up event listener with auto-cleanup
- `primal.onclick(target, handler)` — shortcut for click events
- `primal.onchange(target, handler)` — shortcut for change events
- (More convenience methods for common events)

### Compile-time combining

Multiple scoped `<style>` tags pointing to the same parent are combined into one at compile-time:

```html
<div id="menu-tabs">
  <style scoped primal:style>.title { font-size: 20px; }</style>
  <style scoped primal:style>.subtitle { color: gray; }</style>
</div>
<!-- Combined: -->
<style>#menu-tabs .title { font-size: 20px; } #menu-tabs .subtitle { color: gray; }</style>
```

### SSR parity

The same compile-time processing runs whether building a binary or WASM. Server and client share the code — DOM interactions are always at the boundaries.

### MutationObserver handling

The MutationObserver **only** handles event binding and cleanup for dynamically added elements outside `<island>` components (see decision 018). It does NOT process scoped scripts or styles — those are exclusively handled by `<island>`'s `connectedCallback`. When WASM or JS injects content outside an island, the MutationObserver:
- Wires up `primal:on*` attributes on new elements
- Cleans up listeners on removed elements (batched via `queueMicrotask()`)

Scoped `<script>` and `<style>` tags inside dynamically-injected `<island>` elements are handled entirely by the island's own lifecycle.

### Why this design

- **Compile-time CSS transformation** — zero runtime cost, selectors are pre-scoped
- **No shadow DOM needed** — standard CSS prefixing achieves scoping
- **Automatic** — users just add `scoped primal:style`, macro handles the rest
- **Runtime fallback** — MutationObserver handles dynamic content
- **`primal` global** — helper methods like `primal.addEvent()` available in scoped scripts
