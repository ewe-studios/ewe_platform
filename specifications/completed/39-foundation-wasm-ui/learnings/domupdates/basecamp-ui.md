# Basecamp/Hotwire UI Framework — Deep Analysis

## Overview

The Basecamp ecosystem consists of four layered components that together form a **server-driven, progressive-enhancement UI framework**:

| Component | Purpose | Core Mechanism |
|-----------|---------|----------------|
| **Stimulus** | Controller/action wiring on existing DOM | MutationObserver-driven DOM scanning, event delegation |
| **Turbo** | Navigation, frame updates, streaming | Fetch + morph (Idiomorph) + SSE stream actions |
| **Strada-web** | Web-to-native bridge | Message bus with component registration |
| **Hotwire Spark** | Dev-time live reload | WebSocket + morph/stimulus hot-reload |

**Design philosophy:** HTML over the wire, no build step required, progressive enhancement, no virtual DOM. The server renders HTML; the browser is a thin host that wires up interactivity via data attributes.

---

## 1. Stimulus — The Controller System

### Architecture

Stimulus is **NOT a framework** — it's a wiring layer. The DOM is the source of truth; controllers attach to elements via `data-controller` attributes.

```
Application
  ├── Dispatcher        (event listener registry, delegates to EventListener)
  ├── Router            (scope lifecycle, module loading)
  │   └── ScopeObserver (watches data-controller attribute via MutationObserver)
  │       └── ValueListObserver
  │           └── TokenListObserver  (parses space-separated controller names)
  │               └── AttributeObserver
  │                   └── ElementObserver  (single MutationObserver instance)
  └── ErrorHandler
```

### Key Insight: Single MutationObserver for Everything

All DOM watching flows through **one** `ElementObserver` per root element, configured with:
```js
{ attributes: true, childList: true, subtree: true }
```

The observer processes mutations in batch (one callback per microtask), then delegates to:
1. **AttributeObserver** — watches for a specific attribute (`[data-controller]`)
2. **TokenListObserver** — parses space-separated values, tracks diffs efficiently
3. **ScopeObserver** — creates `Scope` objects for each controller identifier

The **diff algorithm** in `TokenListObserver.refreshTokensForElement()` is clever:
```js
// Compare old vs new tokens, find first differing index
// Only unmatch tokens from that index forward, match new ones from that index forward
const firstDifferingIndex = zip(previousTokens, currentTokens).findIndex(...)
return [previousTokens.slice(firstDifferingIndex), currentTokens.slice(firstDifferingIndex)]
```

This means changing `data-controller="a b c"` to `data-controller="a b d"` only disconnects controller `c` and connects controller `d` — controllers `a` and `b` are untouched.

### Controller Lifecycle

1. **Registration**: `application.register(identifier, ControllerClass)` → `Router.loadDefinition()` → creates `Module`
2. **Connection**: `ScopeObserver` detects `data-controller` attribute → `scopeConnected()` → `Module.connectContextForScope()` → `Controller.connect()`
3. **Disconnection**: Attribute removed or element removed from DOM → `scopeDisconnected()` → `Module.disconnectContextForScope()` → `Controller.disconnect()`

**Reference counting**: The same controller identifier on multiple elements shares a single `Scope` — the `scopeReferenceCounts` WeakMap tracks how many elements reference each scope. Connection/disconnection only fires on the first connect and last disconnect.

### Actions — Event Delegation

Actions are declared as `data-action="click->my-controller#doSomething"`. The `Dispatcher` doesn't add a listener per binding. Instead, it **delegates**:

```js
// Dispatcher.fetchEventListener() creates ONE listener per (eventTarget, eventName, options) combo
// Multiple bindings to the same event share the same native listener
// The EventListener iterates its bindings and calls handleEvent on each
```

When an event fires:
1. `Binding.handleEvent(event)` checks `willBeInvokedByEvent()` — is the event target inside the controller's scope?
2. If yes, applies event modifiers (keyboard filters, mouse filters) via `applyEventModifiers()`
3. Calls `this.method.call(this.controller, event)` — the controller method

**Default event names**: Stimulus infers events from element type (`<a>` → click, `<form>` → submit, `<input>` → input, `<details>` → toggle).

### Values — Type-Coerced Data Attributes

Values bridge HTML attributes to typed JavaScript properties:
```js
static values = { loading: Boolean, count: Number, items: Array }
// Generates: this.loadingValue, this.hasLoadingValue, this.loadingValueDefault
```

The getter/setter reads/writes `data-[identifier]-[name]-value` attributes. Type coercion happens via readers/writers:
- `Boolean`: `"0"` and `"false"` → false, everything else → true
- `Number`: `Number(value.replace(/_/g, ""))` — supports `1_000` syntax
- `Array`/`Object`: JSON.parse / JSON.stringify
- `String`: identity

### Targets & Outlets

**Targets**: `data-[identifier]-target="name"` — scoped element lookup within the controller's element. Uses `SelectorObserver` (watches `MutationObserver` output for matching selectors).

**Outlets**: Reference to OTHER controllers by identifier. Allows cross-controller communication. `OutletSet` maintains a set of outlet elements; `OutletObserver` watches for them appearing/disappearing.

### Classes — CSS Class Mapping

`static classes = ["active"]` generates `this.activeClass` → reads from `data-[identifier]-active-class`. This avoids hardcoding CSS class names in JS.

---

## 2. Turbo — Navigation + Morphing + Streaming

### Two Rendering Modes

Turbo supports **replace** and **morph** rendering:

| Mode | Mechanism | Preserves |
|------|-----------|-----------|
| **Replace** | `document.body.replaceWith(newBody)` | Only `data-turbo-permanent` elements (via Bardo) |
| **Morph** | `Idiomorph.morph(currentBody, newBody)` | ALL existing elements that match by id/structure |

### The Bardo System (Permanent Elements)

**Bardo** = "between states". It preserves `data-turbo-permanent` elements during replace rendering:

```
enter():
  For each permanent element pair (old, new):
    1. Replace new permanent element with <meta name="turbo-permanent-placeholder" content="id">
    2. (Render happens — the old permanent element is still in the DOM)

leave():
  For each permanent element:
    1. Clone old permanent element, replace with clone
    2. Replace placeholder with original old permanent element
    3. Delegate gets enteringBardo/leavingBardo hooks for focus management
```

The focus management is critical: if the active element is inside a permanent element, Bardo tracks it and restores focus after the swap.

### Idiomorph — The Morphing Engine

This is the **most important piece** for understanding DOM morphing. Turbo vendors a copy of Idiomorph (bundled in hotwire_spark.js). Here's the algorithm:

#### Core Algorithm

```
morph(oldNode, newContent):
  1. Parse newContent into a DocumentFragment
  2. Create idMap (bottom-up: every element maps to set of all descendant IDs)
  3. morphNormalizedContent(oldNode, normalizedNewContent)
```

#### morphOldNodeTo — The Decision Tree

```
if newContent == null:
  → remove oldNode
else if !isSoftMatch(oldNode, newContent):  // different tag name or type
  → replace oldNode with newContent
else:
  → syncNodeFrom(newContent, oldNode)       // copy attributes
  → morphChildren(newContent, oldNode)      // recurse
```

**isSoftMatch**: Same node type and tag name. That's it. No class comparison, no attribute comparison.

#### morphChildren — The Matching Algorithm

This is where the intelligence lives. For each new child, scan the old children:

```
for each newChild in newContent:
  1. If insertionPoint is null → append newChild
  2. If isIdSetMatch(newChild, insertionPoint) → morph
  3. Search for findIdSetMatch(newChild, remaining old children) → morph if found
  4. Search for findSoftMatch(newChild, remaining old children) → morph if found
  5. Otherwise → insert newChild before insertionPoint
```

**Id Set Matching** is the key innovation:
- Build a bottom-up `idMap`: each element maps to a `Set<string>` of all IDs in its subtree
- Two elements are an "id set match" if they share the same tag AND (same id OR their id sets intersect)
- This means `<div><span id="foo">` matches `<div><span id="foo">` even if the divs themselves have no id

**findIdSetMatch** (forward scan with pruning):
```js
let newChildPotentialIdCount = getIdIntersectionCount(newChild, oldParent)
// Only search if there's a possibility of a match
for each potentialMatch in oldChildren from insertionPoint:
  if isIdSetMatch(newChild, potentialMatch) → return it
  otherMatchCount += getIdIntersectionCount(potentialMatch, newContent)
  if otherMatchCount > newChildPotentialIdCount → return null  // too many competing matches
```

**findSoftMatch** (tag-name only, also with lookahead):
```js
for each potentialSoftMatch in oldChildren:
  if getIdIntersectionCount(potentialSoftMatch, newContent) > 0 → return null
    // don't soft-match something that has id children needed by other new nodes
  if isSoftMatch(newChild, potentialSoftMatch) → return it
  if isSoftMatch(nextNewSibling, potentialSoftMatch) → siblingSoftMatchCount++
  if siblingSoftMatchCount >= 2 → return null
    // don't consume a node that the next two siblings need
```

#### Attribute Syncing

`syncNodeFrom(from, to)` copies attributes:
1. Iterate `from.attributes` → set each on `to`
2. Iterate `to.attributes` backward → remove any not in `from`
3. **Special handling for form inputs**: `checked`, `disabled`, `selected`, `value`
4. **Active element protection**: if `to` is the currently focused input and `ignoreActiveValue` is true, skip value updates

#### Head Element Handling

The `<head>` is treated specially — merged by comparing `outerHTML` strings:
- Elements with same `outerHTML` are kept (no reload)
- New elements are appended with load-tracking (await stylesheet/script loads)
- Removed elements are deleted AFTER new ones are appended (avoids flash of unstyled content)
- `im-preserve` attribute: keeps element across morphs
- `im-re-append` attribute: forces re-append (for re-executing scripts)

### Turbo Streams — Server-Driven DOM Updates

Server sends `text/vnd.turbo-stream.html` content-type responses containing `<turbo-stream>` elements:

```html
<turbo-stream action="replace" target="my_element" method="morph">
  <template>
    <div id="my_element">...</div>
  </template>
</turbo-stream>
```

**8 stream actions**:

| Action | Effect |
|--------|--------|
| `after` | Insert template content after target |
| `append` | Append template content to target |
| `before` | Insert template content before target |
| `prepend` | Prepend template content to target |
| `remove` | Remove target element |
| `replace` | Replace target outer HTML (supports `method="morph"`) |
| `update` | Replace target inner HTML (supports `method="morph"`) |
| `refresh` | Trigger page refresh via session |

**Morph support**: `replace` and `update` actions accept `method="morph"` which uses Idiomorph instead of simple DOM replacement.

**Duplicate ID protection**: `append` and `prepend` call `removeDuplicateTargetChildren()` — if the new content contains elements with IDs that already exist in the target, the old ones are removed first.

### Turbo Frames — Scoped Navigation

`<turbo-frame>` elements create isolated navigation scopes:
- Clicking a link inside a frame, or submitting a form inside a frame, updates ONLY that frame
- `loading="lazy"`: defer loading until frame appears in viewport (uses `IntersectionObserver` via `AppearanceObserver`)
- `refresh="morph"`: when the frame reloads, use Idiomorph morph instead of replace
- Frame controller manages: fetch lifecycle, form submission, link interception, history updates

**Frame reloading with morph**:
```js
if (src && refresh === "morph") {
  this.#shouldMorphFrame = true
  // ... fetch response ...
  const rendererClass = this.#shouldMorphFrame ? MorphingFrameRenderer : FrameRenderer
  MorphingFrameRenderer.renderElement(currentElement, newElement) {
    morphChildren(currentElement, newElement)
  }
}
```

### Turbo Drive — Full Page Navigation

1. Intercept link clicks and form submissions
2. Fetch HTML via `fetch()` (not full page navigation)
3. Parse response into `PageSnapshot`
4. Render via `PageRenderer`:
   - Merge `<head>` (track stylesheets/scripts, wait for loads)
   - Replace `<body>` (with Bardo for permanent elements)
   - Focus first autofocusable element
5. Update History API (`pushState`)

**Snapshot caching**: Previous pages are cached in `SessionStorage` (or memory). Visiting a cached page shows it instantly as a "preview" while the fresh version loads.

**Tracked element signature**: Page navigation is rejected if `<head>` tracked elements (via `data-turbo-track`) differ between current and new page — forces a full reload instead.

---

## 3. Strada-web — Web-to-Native Bridge

### Architecture

Strada is the bridge between web pages and native app shells (iOS/Android WebView):

```
Native App (iOS/Android)
  └── WebView → loads HTML page
      └── Strada-web (JS) ←→ Bridge → Native Adapter
```

**BridgeComponent** extends Stimulus Controller:
```js
class BridgeComponent extends Controller {
  static component = "form"  // component name
  
  // Only loads when running inside a native app WebView
  static get shouldLoad() { return isStradaNativeApp }
  
  send(event, data, callback) {
    this.bridge.send({ component: this.component, event, data, callback })
  }
}
```

**Message flow**:
1. Web page: BridgeComponent sends `{component, event, data, callback}` to Bridge
2. Bridge generates a message ID, stores callback in `pendingCallbacks` Map
3. Native adapter receives message, processes it, sends response back
4. Bridge routes response to stored callback by message ID

**Pending message queue**: Messages sent before the native adapter is ready are queued and replayed once the adapter connects.

**Platform opt-out**: Elements can declare `data-controller-optout-ios="form"` to disable native bridging on specific platforms.

**BridgeElement** helper: Wraps DOM elements to read/set `data-bridge-*` attributes. Used to convey element metadata (title, disabled state) to native apps.

---

## 4. Hotwire Spark — Dev-Time Live Reload

### How It Works

Spark runs a Rails middleware that watches for file changes and pushes updates via **ActionCable WebSocket**:

```
File watcher (server) → ActionCable channel → WebSocket → Browser
  → HotwireSpark channel handler → dispatch action
```

**Three reload types**:

| Change Type | Importmap Mode | Bundler Mode |
|-------------|---------------|--------------|
| HTML | Fetch + Idiomorph morph + Stimulus hot-reload | Turbo visit (full page replace) |
| CSS | Fetch new stylesheet, replace href | Same |
| Stimulus controller | Dynamic `import()` + application.unload/register | Turbo visit (full page replace) |

### HTML Morph Reload (Importmap mode)

```js
async reloadHtml() {
  const reloadedDocument = await fetchAndParseHtml();  // cache-busted fetch
  Idiomorph.morph(document.body, reloadedDocument.body);
  await StimulusReloader.reloadAll();
}
```

This is remarkably smooth — the DOM is morphed in place, preserving focus, scroll, and any DOM state that Idiomorph's matching algorithm can preserve.

### Stimulus Hot-Reload

```js
async #reloadStimulusController(moduleName) {
  const path = cacheBustedUrl(pathForModule);  // bust browser cache
  const module = await import(path);           // dynamic import
  this.#registerController(controllerName, module);  // unload + register
}
```

The controller is unloaded and re-registered, which triggers disconnect/connect for all active instances. Scroll position and DOM state are preserved because the morph happens first.

### Connection Monitoring

Spark uses ActionCable's `ConnectionMonitor` with:
- 6-second stale threshold
- Exponential backoff reconnection (15% backoff rate)
- Visibility change detection (reopen stale connections when tab becomes visible)

---

## Key Design Patterns & Insights

### 1. DOM as Source of Truth

Neither Stimulus nor Turbo maintains a virtual representation of the DOM. The DOM IS the state. Controllers read from and write to real DOM elements. Turbo compares real DOM trees. This eliminates the entire vDOM diff/patch layer and the memory overhead of maintaining parallel tree structures.

### 2. MutationObserver as the Engine

Stimulus runs on a single `MutationObserver` per root. Everything — controller discovery, target detection, outlet tracking, value watching — flows from processing the same mutation records. This is efficient because the browser batches mutations into single callbacks.

### 3. Id Set Matching for Smart Morphing

Idiomorph's ID set intersection algorithm is the key to effective morphing. By building a bottom-up map of which IDs live in which subtrees, it can match nodes that don't have IDs themselves but contain (or are contained by) elements that do. This is far more robust than simple position-based or tag-based matching.

### 4. Bardo for State Preservation

The Bardo pattern (placeholder swap) is how Turbo preserves DOM state during full body replacement. By temporarily replacing permanent elements with `<meta>` placeholders, the rendering algorithm can proceed as if those elements don't exist, then restore them afterward. Focus management hooks allow restoring focus to the correct element.

### 5. Stream Actions as DOM Commands

Turbo Streams are essentially a command language for the DOM. The server sends declarative operations (`append`, `replace`, `remove`) that the client executes. With `method="morph"`, even `replace` and `update` become intelligent rather than brute-force.

### 6. Progressive Enhancement Layers

```
Layer 1: Server-rendered HTML (works without JS)
Layer 2: Turbo Drive (intercepts navigation, fetches HTML)
Layer 3: Turbo Frames (scoped updates within page)
Layer 4: Turbo Streams (server-pushed DOM updates via SSE)
Layer 5: Stimulus (client-side interactivity via data attributes)
Layer 6: Strada (native app bridge)
```

Each layer is optional and independent. You can use Turbo without Stimulus, or Stimulus without Turbo. The framework degrades gracefully.

### 7. Event-Driven, Not Render-Driven

Stimulus controllers respond to events. There is no `render()` method, no `setState()` that triggers a re-render. Controllers mutate the DOM directly in response to user actions or DOM events. This makes the mental model simple but shifts complexity to managing DOM state manually.

### 8. Attribute-Based Configuration

Everything is configured via `data-*` attributes:
- `data-controller` — wire up controllers
- `data-action` — bind events to methods
- `data-target` — reference elements
- `data-[name]-value` — typed values
- `data-turbo-frame` — navigation scoping
- `data-turbo-permanent` — preserve across renders
- `data-bridge-*` — native app metadata

This makes the HTML self-describing and avoids JavaScript configuration files.

---

## Relevance to Our WASM-UI Framework (Spec 39)

### What We Can Learn

1. **Morphing is hard** — Idiomorph's algorithm is 1300+ lines. The id set matching, soft match lookahead, and attribute syncing are carefully tuned. If we want morphing, we should consider using Idiomorph directly or implementing a similar algorithm in Rust.

2. **MutationObserver is efficient** — Stimulus uses one observer for everything. Our WASM-UI could use a similar approach: one observer that drives controller discovery, target tracking, and attribute watching.

3. **Stream actions are a good SSE pattern** — Server sends `<turbo-stream>` commands. Our WASM-UI could have a similar command format but with Arrow batches for efficiency (which we already plan in Feature 04).

4. **Bardo is the right pattern for permanent elements** — Placeholder swap during rendering is clean. Our spec's `data-turbo-permanent` equivalent should use a similar pattern.

5. **Event delegation reduces listener count** — Stimulus's Dispatcher shares one native listener per (target, event) combo. Our JS runtime should do the same rather than attaching listeners per binding.

6. **Reference counting for shared scopes** — Stimulus's `scopeReferenceCounts` WeakMap prevents redundant connect/disconnect. Important if multiple elements share the same controller identifier.

7. **No virtual DOM means manual state management** — The tradeoff is real. Stimulus controllers must manually manage DOM state. Our signal system (Feature 02) should abstract this away.

8. **Controller lifecycle matters** — `initialize()`, `connect()`, `disconnect()` hooks are essential for cleanup. Our Component trait (Feature 01) should follow this pattern.

### What We Already Cover (and Should Verify)

- **Signal-based reactivity** (Feature 02) — Addresses Stimulus's manual DOM manipulation
- **Arrow batch format** (Feature 04) — More efficient than Turbo's individual stream actions
- **Web component base** (Feature 05) — Similar to Stimulus controllers but with shadow DOM
- **SSE client** (Feature 06 JS runtime) — Similar to Turbo Streams but with Arrow batches
- **Controller pattern** — Explicitly inspired by Stimulus in our requirements

### Gaps to Consider

- **Morphing strategy**: Our spec mentions a `morph/` module but doesn't detail the algorithm. Idiomorph is proven; we should either adopt it or document a clear alternative.
- **Permanent element preservation**: Not explicitly covered in our feature specs. The Bardo pattern should be added.
- **Event delegation in JS runtime**: Our feature specs describe per-binding event listeners. We should consolidate to delegated listeners per (target, event) pair.
- **Head element handling**: Idiomorph has special `<head>` merge logic. Our framework should handle this if we're doing full-page morphing.
- **Form input value syncing**: Idiomorph has special handling for `<input>`, `<textarea>`, `<select>` — syncing `value`, `checked`, `selected` while protecting the active element. Our morph module should replicate this.
