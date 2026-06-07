# HTMX — Deep Analysis

## Overview

HTMX is a **single-file** (~5000 line) library that lets you trigger AJAX requests from HTML attributes and swap the response into the DOM. No JavaScript framework, no build step. The server returns HTML; HTMX swaps it in.

## Core Architecture

### Single Attribute-Driven Model

HTMX works by scanning the DOM for `hx-*` attributes and wiring up event listeners:

```html
<button hx-post="/click" hx-target="#result" hx-swap="innerHTML">Click</button>
```

The core verb attributes: `hx-get`, `hx-post`, `hx-put`, `hx-delete`, `hx-patch`.

These are discovered by scanning:
```js
const VERB_SELECTOR = '[hx-get], [data-hx-get], [hx-post], [data-hx-post], ...'
```

### Node Processing Pipeline

```
DOMContentLoaded → processNode(document.body)
  → initNode(elt)
    → getTriggerSpecs(elt)     // Parse hx-trigger or infer default
    → processVerbs(elt)        // Wire up hx-get/post/etc.
    → boostElement(elt)        // hx-boost: intercept links/forms
```

`processNode()` is called recursively on newly swapped content, making HTMX **self-initializing** — any new HTML dropped into the DOM just needs `htmx.process(newContent)` to activate.

### Trigger System

The `hx-trigger` attribute is a mini-DSL parsed by a custom tokenizer:

```
hx-trigger="click, changed, delay:500ms, from:closest form, target:button, throttle:1s, once, queue:last"
```

Special triggers:
- **`revealed`** — IntersectionObserver fires when element scrolls into view
- **`intersect`** — IntersectionObserver with custom threshold/root
- **`load`** — Fires immediately on page load
- **`every:30s`** — Polling at fixed intervals
- **`changed`** — Only fires if the element's `.value` changed since last trigger

### The AJAX Request Flow

```
1. Event fires → shouldCancel() checks if default should be prevented
2. getInputValues() → collects form data (FormData, enclosing form for non-GET)
3. issueAjaxRequest() → XMLHttpRequest
   - Headers: HX-Request, HX-Trigger, HX-Target, HX-Current-URL
   - Can include hx-headers, hx-vals, hx-include
4. Response handling:
   - HX-Trigger header → fire custom events from server
   - HX-Redirect → client-side redirect
   - HX-Location → AJAX navigation
   - HX-Push/Replace → history management
   - HX-Reswap → override swap style
5. swap() → applies HTML to DOM
6. processNode() → re-initialize htmx on new content
```

## DOM Swapping — The Core Mechanism

### Swap Styles

HTMX supports 8 swap styles:

| Style | Mechanism |
|-------|-----------|
| `innerHTML` (default) | Replace target's innerHTML |
| `outerHTML` | Replace the target element itself |
| `beforebegin` | Insert before target (adjacent sibling) |
| `afterbegin` | Insert as first child |
| `beforeend` | Insert as last child |
| `afterend` | Insert after target |
| `delete` | Remove target |
| Custom (via extensions) | morphdom, idiomorph, Alpine morph |

### The Swap Pipeline

```
swap(target, content, swapSpec)
  1. Preserve focus & selection (activeElement, selectionStart/End)
  2. Parse HTML → makeFragment()
  3. Process OOB swaps (hx-swap-oob) — elements with swap-oob are swapped to their targets by ID
  4. swapWithStyle() → do the actual DOM manipulation
  5. Restore focus & selection (look up by ID in new DOM)
  6. Settle phase:
     - Clone attributes (class, style, width, height) from old to new
     - Remove .htmx-added class
     - Trigger htmx:load on new elements
```

### Out-of-Band Swaps (OOB)

This is a **powerful pattern** — the server can update multiple elements in a single response:

```html
<!-- Main response targets #main-area -->
<div>Updated content</div>
<!-- But this element goes elsewhere by ID -->
<div id="sidebar" hx-swap-oob="true">Updated sidebar</div>
```

The `hx-swap-oob` attribute on any element in the response causes it to be swapped into the existing DOM by ID selector, regardless of the main swap target. Supports `hx-swap-oob="morph:css-selector"` for morph-based updates.

### Attribute Settling

After swap, HTMX does an **attribute reconciliation** phase:
```js
// Default: ['class', 'style', 'width', 'height']
config.attributesToSettle
```

1. Before swap: `handleAttributes()` — if old node has same ID as new node, preserve old attributes
2. After swap (settle): restore the original attributes

This prevents CSS transitions from being lost when class attributes change.

## History System

HTMX maintains a **localStorage-based** page cache:

```
localStorage['htmx-history-cache'] = [{url, content, title, scroll}, ...]
```

On `popstate`:
1. Check localStorage cache
2. If found: swap cached HTML, restore scroll position
3. If miss: fetch from server with `HX-History-Restore-Request` header

The history element defaults to `<body>` but can be customized with `[hx-history-elt]`.

`hx-history="false"` prevents a page from being cached (privacy).

## Event System

HTMX has a **rich event system** — every step of the request lifecycle fires a custom event:

```
htmx:confirm → htmx:configRequest → htmx:beforeRequest → htmx:beforeSend
→ htmx:xhr:* → htmx:beforeOnLoad → htmx:beforeSwap → htmx:afterSwap
→ htmx:afterSettle → htmx:load → htmx:afterRequest
```

Events are **cancelable** — `htmx:confirm` can prevent the request, `htmx:beforeSwap` can modify content.

**Server-triggered events** via `HX-Trigger` header:
```
HX-Trigger: {"showMessage": "Success!"}
```
The server can fire arbitrary custom events with payload data.

## Boost Mode

`hx-boost="true"` on a `<body>`, `<a>`, or `<form>` enables progressive enhancement:
- Intercept all clicks on links within the boosted element
- Intercept form submissions
- Fetch HTML via AJAX
- Swap into body (or target)
- Update URL via `pushState`

This turns a regular server-rendered app into an SPA without any code changes.

## Extensions

HTMX has a formal extension API:

```js
htmx.defineExtension('my-ext', {
  onEvent(name, evt) { ... },
  transformResponse(text, xhr, elt) { ... },
  isInlineSwap(swapStyle) { ... },
  handleSwap(swapStyle, target, fragment, settleInfo) { ... },
  encodeParameters(xhr, parameters, elt) { ... }
})
```

Notable extensions:
- **morphdom-swap** — uses morphdom for smarter DOM diffing
- **alpine-morph** — uses Alpine's morph (preserves Alpine state)
- **sse** — Server-Sent Events support
- **head-support** — merges `<head>` tags (scripts, styles)
- **path-deps** — path-based cache invalidation

## Input Value Collection

HTMX has sophisticated form handling:
- Collects values from the enclosing form (for non-GET requests)
- Tracks the last-clicked submit button (including `form` attribute)
- Supports `hx-include` to pull in values from arbitrary selectors
- Supports `hx-params` to filter which params are sent (`*`, `none`, `not:name`)
- `hx-encoding="multipart/form-data"` for file uploads
- Uses FormData for body encoding (actual FormData, not URL-encoded, when encoding is multipart)

## Key Design Patterns

### 1. Hypermedia-Driven

Everything is HTML over the wire. No JSON, no client-side state management. The server is the source of truth.

### 2. Attribute-Based Configuration

All configuration lives in HTML attributes — no JavaScript code needed. This makes HTMX declarative and server-friendly.

### 3. Swap-Based DOM Updates

HTMX doesn't morph by default — it does **brute-force replacement** (innerHTML, outerHTML). Morphing is available via extensions.

### 4. Self-Initializing

`processNode()` makes any new HTML htmx-aware. This is essential for AJAX responses that bring in new interactive elements.

### 5. Event-Driven Lifecycle

Every phase is an event. Extensions, custom logic, and debugging all hook into the event pipeline.

### 6. Request Queuing & Sync

`hx-sync` controls concurrent requests:
- `drop` — ignore new requests while one is in flight
- `abort` — cancel the current request, start a new one
- `replace` — abort current, start new (same as abort but triggers different events)
- `queue:first` / `queue:all` / `queue:last` — queue behavior

## Relevance to Spec-39

### What We Learn

1. **Swap-by-ID (OOB) is powerful** — HTMX's `hx-swap-oob` lets the server update any element by ID. Our Arrow batch format should support a similar "target by ID" operation.

2. **Brute-force replacement works well** — HTMX's default is innerHTML replacement, not morphing. This is simple and fast. Morphing is an optimization, not a requirement.

3. **hx-on: event handlers** — HTMX 2 supports inline event handlers via `hx-on:click="..."` (eval-based). Useful for quick interactions without a controller.

4. **Progressive enhancement** — `hx-boost` turns any server-rendered app into an SPA. Our framework should support a similar boost mode.

5. **Form handling is complex** — The input value collection code is ~200 lines. We should handle forms well.

### What HTMX Doesn't Do

- No fine-grained reactivity (no signals)
- No virtual DOM or morphing (by default)
- No component model
- No state management

### What Our Spec-39 Should Incorporate

- OOB-style targeted updates (target by ID in Arrow batches)
- A processNode-like initialization function for newly added content
- Event-driven lifecycle for DOM swaps
- The hx-trigger-like trigger DSL for declarative event binding
- Focus/selection preservation during swaps (HTMX does this well)
