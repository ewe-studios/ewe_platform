# Feature 08: Event Runtime

## Description

Event binding system: direct binding as default (`primal:onclick`), opt-in delegation (`primal:onclick:delegate`), MutationObserver for auto-cleanup of non-island content. Programmatic helpers via `primal.on()`.

**Decision:** 018

## Module

`crates/foundation_wasm_ui/assets/foundation-wasm-ui.js` (event runtime)

## Direct binding (default)

```html
<button primal:onclick="controller.delete">Click</button>
<input primal:onchange="handler.update" />
```

`element.addEventListener(eventType, handler)` — clean `this` context, works for bubbling AND non-bubbling events (focus, blur, scroll).

## Opt-in delegation

```html
<button primal:onclick:delegate="#container">Click</button>
<button primal:onclick:delegate="parent">Click</button>
```

## MutationObserver — non-island only

```javascript
const observer = new MutationObserver((mutations) => {
    for (const mutation of mutations) {
        if (mutation.type === 'childList') {
            for (const node of mutation.addedNodes) {
                if (node.nodeType === Node.ELEMENT_NODE && !node.closest('island')) {
                    scanAndWire(node);
                }
            }
            for (const node of mutation.removedNodes) {
                if (node.nodeType === Node.ELEMENT_NODE && !node.closest('island')) {
                    removeListeners(node);
                }
            }
        }
    }
});
observer.observe(document, { subtree: true, childList: true });
```

**Boundary rule:** MutationObserver explicitly skips any subtree rooted at `<island>`. WASM-injected content with `<island>` tags is fully delegated to the browser's custom element lifecycle.

## Programmatic event binding

```javascript
runtime.on(element, "click", "controller.delete");
runtime.on(element, "click", "controller.delete", { delegate: "#container" });
runtime.off(element, "click");
runtime.onclick(element, "controller.delete");
```

## Dependencies

- Feature 00 (JS runtime core)

## Testing

- Direct binding: click → handler called with correct `this`
- Delegation: click on child → handler on parent called
- MutationObserver: added element → wired, removed element → cleaned
- MutationObserver skips island subtree
- Programmatic: runtime.on / runtime.off work correctly
- Initial scan: existing attributes wired on DOMContentLoaded
- Non-bubbling events (focus, blur) → direct binding works