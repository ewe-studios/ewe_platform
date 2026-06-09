# Feature 08: Event Runtime

**Module:** `crates/foundation_wasm_ui/assets/foundation-wasm-ui.js` (event runtime section)
**Decisions:** 018 (event binding strategy)
**Constraint:** Direct binding always default. Delegation opt-in. MutationObserver never touches island subtrees.

Event binding system in JS: direct binding as default (`primal:onclick`), opt-in delegation (`primal:onclick:delegate`), MutationObserver for auto-cleanup of non-island content. Programmatic helpers via `runtime.on()`. WASM callback bridge for `primal:onclick="callback-7"` crossing the WASM boundary via `invoke_callback(N, allocation_id)`.

---

## 1. Types & Data Structures

```javascript
const elementListeners = new WeakMap();  // WeakMap<Element, Map<string, Function>>
let cleanupQueue = [];                   // Element[] — nodes awaiting microtask cleanup
let cleanupScheduled = false;
let documentObserver = null;             // MutationObserver|null
const CONVENIENCE_EVENTS = [
    'click', 'change', 'submit', 'keydown', 'keyup',
    'focus', 'blur', 'scroll', 'input', 'mousedown', 'mouseup'
];
```

`elementListeners`: WeakMap keyed by element — if the element is GC'd, the listener map is collected with it. Inner `Map<string, Function>` keyed by event type, storing the bound handler needed for `removeEventListener`. `cleanupQueue`: accumulates programmatically removed elements, drained via microtask.

---

## 2. scanAndWire Algorithm

```
scanAndWire(rootNode):
  1. let elements = [rootNode, ...rootNode.querySelectorAll('*')]
  2. for each element in elements:
     a. for each attr in element.attributes:
        i.   let name = attr.name                      // "primal:onclick:delegate"
        ii.  if !name.startsWith('primal:on') → skip
        iii. let handlerRef = attr.value                // "controller.delete" or "callback-7"
        iv.  let parts = name.split(':')                // ["primal","onclick","delegate"]
        v.   let eventKey = parts[1]                    // "onclick"
        vi.  if !eventKey.startsWith('on') → skip
        vii. let eventType = eventKey.slice(2)          // "click"
        viii.if parts[2] === 'delegate':
               wireDelegated(element, eventType, handlerRef)
             else:
               wireDirect(element, eventType, handlerRef)
```

Root itself is checked first. `querySelectorAll('*')` ensures nested attributed elements are scanned.

---

## 3. wireDirect — Direct Event Binding

```
wireDirect(element, eventType, handlerRef):
  1. let handler = resolveFunctionRef(handlerRef)
  2. if handler === null → console.warn, return
  3. let bound = handler.bind(element)              // clean this = target element
  4. let listeners = elementListeners.get(element) ?? new Map()
  5. elementListeners.set(element, listeners)
  6. if listeners.has(eventType):                   // prevent double-bind on re-scan
       element.removeEventListener(eventType, listeners.get(eventType))
  7. listeners.set(eventType, bound)
  8. element.addEventListener(eventType, bound)
```

Step 6 prevents double-binding when `scanAndWire` re-runs (e.g., morph re-scan). Old listener removed before new one attached.

---

## 4. resolveFunctionRef — Dot-Path Resolution

```
resolveFunctionRef(handlerRef):
  1. if handlerRef matches /^callback-(\d+)$/:
       return createWasmCallbackHandler(parseInt(RegExp.$1, 10))   // WASM bridge (§10)
  2. let parts = handlerRef.split('.')              // ["controller", "delete"]
  3. let current = window
  4. for each part: current = current[part]
       if current is undefined/null → return null   // resolution failed
  5. if typeof current !== 'function' → return null
  6. return current
```

Step 1 intercepts WASM callback refs before dot-path resolution. Step 4 walks the object graph segment-by-segment, failing early on missing properties.

---

## 5. resolveDelegateTarget

```
resolveDelegateTarget(element, selector):
  1. if selector starts with '#' → document.querySelector(selector)
  2. if selector === 'parent'   → element.parentElement
  3. if selector === 'body'     → document.body
  4. fallback                   → document.querySelector(selector)
```

Returns `null` if no element found. Callers guard against null.

---

## 6. wireDelegated — Opt-in Event Delegation

```
wireDelegated(element, eventType, selector):
  1. let target = resolveDelegateTarget(element, selector)
  2. if target === null → console.warn, return
  3. let delegatedHandler = function(event) {
       if element.contains(event.target) || element === event.target:
           event.delegateTarget = element
     }
  4. let listeners = elementListeners.get(target) ?? new Map()
  5. elementListeners.set(target, listeners)
  6. let key = eventType + ':delegate:' + (element.getAttribute('primal-id') || element.id)
  7. if listeners.has(key):
       target.removeEventListener(eventType, listeners.get(key))
  8. listeners.set(key, delegatedHandler)
  9. target.addEventListener(eventType, delegatedHandler)
```

`delegateKey` includes originating element's `primal-id` so multiple elements can delegate the same event type to the same container without colliding.

---

## 7. MutationObserver — Non-Island Content

Single observer on `document`, `{ subtree: true, childList: true }`. Skips island subtrees.

```javascript
function initMutationObserver() {
    documentObserver = new MutationObserver((mutations) => {
        for (const mutation of mutations) {
            if (mutation.type !== 'childList') continue;
            for (const node of mutation.addedNodes) {
                if (node.nodeType !== Node.ELEMENT_NODE) continue;
                if (node.closest('island')) continue;      // boundary rule
                scanAndWire(node);
            }
            for (const node of mutation.removedNodes) {
                if (node.nodeType !== Node.ELEMENT_NODE) continue;
                if (node.closest('island')) continue;      // boundary rule
                removeListeners(node);
            }
        }
    });
    documentObserver.observe(document, { subtree: true, childList: true });
}
```

**Boundary rule:** `node.closest('island')` returns nearest `<island>` ancestor (or self). If found, node is inside island subtree — MutationObserver skips entirely. Island lifecycle managed by `<island>` custom element (F06). Not responsible for: island content, delegated event cleanup, or WASM-managed node registries.

---

## 8. removeListeners Algorithm

```
removeListeners(rootNode):
  1. let elements = [rootNode, ...rootNode.querySelectorAll('*')]
  2. for each element in elements:
     a. let listeners = elementListeners.get(element)
     b. if listeners is undefined → skip
     c. for each [key, handler] in listeners:
        let baseType = key.split(':')[0]            // strip ":delegate:id" suffix
        element.removeEventListener(baseType, handler)
     d. listeners.clear()
     e. elementListeners.delete(element)
```

Handles both direct keys (`"click"`) and delegated keys (`"click:delegate:primal-42"`) by extracting the base event type. Explicit `delete` ensures immediate cleanup rather than waiting for WeakMap GC.

---

## 9. runtime.trackRemoved — Queue and Microtask Batching

```
runtime.trackRemoved(node):
  1. cleanupQueue.push(node)
  2. if cleanupScheduled → return
  3. cleanupScheduled = true
  4. queueMicrotask(() => {
       let batch = cleanupQueue.splice(0)
       cleanupScheduled = false
       for each node in batch: removeListeners(node)
     })
```

Removing 50 nodes synchronously produces a single cleanup pass. Microtask fires after the synchronous block but before paint. Still callable when MutationObserver IS available — framework code (e.g., morph diffing in F07) may call it directly for immediate scheduling.

---

## 10. Initial Scan — DOMContentLoaded Handler

```javascript
function initEventRuntime() {
    document.addEventListener('DOMContentLoaded', () => {
        scanAndWire(document.body);
        initMutationObserver();
    });
    // Guard: script loaded after parse (async/defer)
    if (document.readyState !== 'loading') {
        scanAndWire(document.body);
        initMutationObserver();
    }
}
```

Both paths are idempotent — re-scanning an already-wired element replaces the listener (section 3, step 6).

---

## 11. Programmatic API

```javascript
const runtime = {
    on(element, eventType, handlerRef, options = {}) {
        if (options.delegate) wireDelegated(element, eventType, options.delegate);
        else wireDirect(element, eventType, handlerRef);
    },
    off(element, eventType) {
        const listeners = elementListeners.get(element);
        if (!listeners) return;
        if (eventType) {
            for (const [key, handler] of listeners) {
                if (key === eventType || key.startsWith(eventType + ':')) {
                    element.removeEventListener(eventType, handler);
                    listeners.delete(key);
                }
            }
        } else {
            for (const [key, handler] of listeners) {
                element.removeEventListener(key.split(':')[0], handler);
            }
            listeners.clear(); elementListeners.delete(element);
        }
    },
    trackRemoved(node) { /* section 9 */ },
};
// Convenience methods generated via loop
for (const evt of CONVENIENCE_EVENTS) {
    runtime['on' + evt] = (element, handlerRef, options) =>
        runtime.on(element, evt, handlerRef, options);
}
// Produces: runtime.onclick, runtime.onchange, runtime.onsubmit, etc.
```

---

## 12. WASM Callback Bridge

When `resolveFunctionRef` matches `/^callback-(\d+)$/`, it returns a WASM bridge handler instead of resolving against window scope.

```
createWasmCallbackHandler(callbackId):
  return function(event) {
    1. let allocationId = event.target.getAttribute('primal-id')
    2. let numericId = parseInt(allocationId, 10)
    3. if isNaN(numericId): numericId = 0           // fallback: missing primal-id
    4. wasmExports.invoke_callback(callbackId, numericId)
  }
```

**Rust side** (foundation_wasm):
```rust
#[no_mangle]
pub extern "C" fn invoke_callback(callback_id: u64, allocation_id: u32) {
    RUNTIME.with(|rt| {
        if let Some(cb) = rt.borrow_mut().callbacks.get_mut(&callback_id) {
            cb(serde_json::Value::Number(allocation_id.into()));
        }
        // Not found → silently dropped (stale/disposed signal)
    });
}
```

**Callback ID lifecycle:** Monotonic `u64` from `Runtime.next_callback_id` (F02). Each `SignalSetter<T>` carries a `callback_id`. Signal disposal removes the `BTreeMap` entry. Stale `invoke_callback` finds no entry, silently returns.

**allocation_id:** Carries the `primal-id` of the event target, letting the Rust callback identify which DOM element triggered the event.

**html! macro codegen:**
```rust
// html! { <button primal:onclick={set_count}>Click</button> }  expands to:
Html { attributes: vec![
    ("primal:onclick".into(), format!("callback-{}", set_count.callback_id())),
    ("primal-id".into(), "42".into()),
], .. }
```

JS sees `primal:onclick="callback-7"`. `scanAndWire` calls `resolveFunctionRef("callback-7")` which returns the WASM bridge handler.

---

## 13. Error Cases

| Scenario | Behavior |
|----------|----------|
| Unresolvable dot-path handler | `resolveFunctionRef` returns null. `wireDirect` logs warning, skips. |
| Delegate target not found | `resolveDelegateTarget` returns null. `wireDelegated` logs warning, skips. |
| `callback-N` with disposed signal | `invoke_callback(N, _)` finds no BTreeMap entry. Silent no-op. |
| `primal-id` missing on callback target | `allocation_id` falls back to 0. |
| Double scanAndWire on same element | Old listener removed, new attached (idempotent). |
| removeListeners on unwired element | WeakMap returns undefined, skipped. |
| Non-bubbling event with delegation | focus/blur do not bubble. Use focusin/focusout or avoid delegation. |
| MutationObserver unavailable (jsdom) | Manual `trackRemoved` required. `initMutationObserver` no-ops. |
| Malformed attribute (`primal:foo`) | `startsWith('on')` check fails, skipped. |

## 14. Integration Points

| Feature | Connection |
|---------|-----------|
| F00 (foundation_wasm) | `invoke_callback` FFI extern. WASM memory for callback bridge. |
| F02 (Signal System) | `callback_id` from `SignalSetter<T>`. `BTreeMap<u64, ...>` callback registry. Stale ID disposal. |
| F03 (html! Macro) | Generates `primal:onclick="callback-N"` and `primal-id` attributes. |
| F05 (Arrow Encoding) | Ops 6-7 (Add/RemoveEventListener) trigger JS-side `scanAndWire`. |
| F06 (Web Components) | `<island>` boundary: MutationObserver skips island subtrees. |
| F07 (DOM Morphing) | Post-morph `scanAndWire` on patched subtree wires new `primal:on*` attrs. |

## 15. File Ownership

```
crates/foundation_wasm_ui/assets/
├── foundation-wasm-ui.js
│   ├── elementListeners, scanAndWire, wireDirect, wireDelegated
│   ├── resolveFunctionRef, resolveDelegateTarget, removeListeners
│   ├── initMutationObserver, initEventRuntime
│   ├── runtime.on / .off / .trackRemoved / convenience methods
│   └── createWasmCallbackHandler
└── foundation-wasm.js
    └── invoke_callback (FFI export)
```

## 16. Refactoring Strategy

**Phase 1 — Core:** `elementListeners`, `scanAndWire`, `wireDirect`, `resolveFunctionRef`, `removeListeners`. Test: attach `primal:onclick`, call scanAndWire, verify addEventListener.
**Phase 2 — Delegation:** `resolveDelegateTarget`, `wireDelegated`. Test: click child, delegated handler on container fires.
**Phase 3 — MutationObserver:** `initMutationObserver` with boundary rule. Test: append outside island (wired), inside island (skipped).
**Phase 4 — Cleanup:** `removeListeners`, `trackRemoved` microtask batching. Test: remove element, verify removeEventListener + WeakMap cleared.
**Phase 5 — WASM bridge:** `createWasmCallbackHandler`, `invoke_callback` FFI. Test: `primal:onclick="callback-7"`, simulate click, verify invoke_callback(7, primalId).
**Phase 6 — API:** `runtime.on`, `runtime.off`, convenience methods. Test: `runtime.onclick` wires; `runtime.off` removes.
**Phase 7 — Integration:** `initEventRuntime` + DOMContentLoaded. End-to-end: page loads, all wired, WASM callbacks functional, morph re-scan works.

## 17. Testing

### scanAndWire & wireDirect (tests 1-8)

| # | Scenario | Verify |
|---|----------|--------|
| 1 | `<button primal:onclick="ctrl.del">` | addEventListener('click', fn) called; fn is ctrl.del.bind(button) |
| 2 | `<input primal:onchange="handler.update">` | addEventListener('change', fn) on the input |
| 3 | Nested: parent onclick, child onkeydown | Both wired with correct event types |
| 4 | `primal:onfocus="handler.focus"` (non-bubbling) | addEventListener('focus', fn) direct, no delegation |
| 5 | Handler `this` context | Click fires, this === button element |
| 6 | Double scanAndWire same element | removeEventListener old, addEventListener new. Net: one listener |
| 7 | Unresolvable handler "nonexistent.fn" | console.warn, no listener, no error |
| 8 | Element with onclick + onsubmit | Two entries in Map, both wired |

### Delegation (tests 9-13)

| # | Scenario | Verify |
|---|----------|--------|
| 9 | `primal:onclick:delegate="#container"` | Listener on #container, not on button |
| 10 | `primal:onclick:delegate="parent"` | Listener on button.parentElement |
| 11 | `primal:onclick:delegate="body"` | Listener on document.body |
| 12 | Click on delegated child | Handler fires, event.delegateTarget set |
| 13 | Delegate target not found | console.warn, no listener |

### MutationObserver (tests 14-19)

| # | Scenario | Verify |
|---|----------|--------|
| 14 | Append div with primal:onclick to body | scanAndWire called, listener attached |
| 15 | Remove wired element from body | removeListeners called, removeEventListener fired |
| 16 | Append element inside `<island>` | scanAndWire NOT called |
| 17 | Remove element inside `<island>` | removeListeners NOT called |
| 18 | Append `<island>` itself to body | closest('island') returns self, skipped |
| 19 | Append subtree with 3 primal:on* elements | All 3 wired in single callback |

### removeListeners & trackRemoved (tests 20-26)

| # | Scenario | Verify |
|---|----------|--------|
| 20 | Element with click + change listeners | Both removeEventListener calls. WeakMap entry deleted |
| 21 | Parent with 3 wired descendants | All 4 elements cleaned |
| 22 | Unwired element | No error, no-op |
| 23 | Element with delegated listener key | removeEventListener uses base type from compound key |
| 24 | Track 5 nodes synchronously | Single microtask, all 5 cleaned |
| 25 | Track, await microtask, track again | Two separate batches |
| 26 | Track without MutationObserver active | Cleanup still runs via microtask |

### Programmatic API (tests 27-31)

| # | Scenario | Verify |
|---|----------|--------|
| 27 | runtime.on(el, 'click', 'ctrl.fn') | addEventListener with resolved handler |
| 28 | runtime.on(el, 'click', 'ctrl.fn', {delegate:'#box'}) | Listener on #box, not el |
| 29 | runtime.off(el, 'click') | removeEventListener called, entry removed |
| 30 | runtime.off(el) — no event type | All listeners removed, WeakMap entry deleted |
| 31 | runtime.onclick(el, 'ctrl.fn') | Convenience method wires correctly |

### WASM Callback Bridge (tests 32-38)

| # | Scenario | Verify |
|---|----------|--------|
| 32 | primal:onclick="callback-7" | resolveFunctionRef returns WASM bridge handler |
| 33 | Click on callback-bound element | invoke_callback(7, primalId) called |
| 34 | Element has primal-id="42" | allocation_id = 42 |
| 35 | Element missing primal-id | allocation_id = 0 |
| 36 | Stale callback (disposed signal) | invoke_callback returns, no panic |
| 37 | callback-0 (ID zero) | Valid, parsed and invoked |
| 38 | "callback-abc" (non-numeric) | Regex fails, falls to dot-path (graceful fail) |

### Integration (tests 39-42)

| # | Scenario | Verify |
|---|----------|--------|
| 39 | Page with 5 primal:on* elements loads | All 5 wired after DOMContentLoaded |
| 40 | Script loaded after DOMContentLoaded | readyState guard triggers immediate scan |
| 41 | Morph patches subtree with new primal:onclick | Post-morph scanAndWire wires new attribute |
| 42 | End-to-end: html! → WASM bridge → click → signal | Callback triggers, signal updates, stabilize, DOM reflects change |
