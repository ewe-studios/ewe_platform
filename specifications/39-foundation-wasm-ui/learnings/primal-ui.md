# Primal UI — Original Vision & Lessons Learned

## Source

Original Notion design notes: "HTML As DataCarrier V2" — early conceptual exploration of a UI framework built on plain HTML with minimal runtime enhancement, island architecture, and server/client dual rendering.

---

## 1. Core Philosophy

**Return to primal HTML** — enhance it with a minimal runtime (JS or WASM) that transforms presentation markup into real HTML. The page should work as plain HTML, then become interactive when the runtime loads.

### Three Deployment Modes

| Mode | How It Works |
|------|-------------|
| **Client SPA** | Import core runtime (WASM or JS), render directly in browser. Open an HTML file from filesystem and it works. |
| **Static Build** | Compiler parses the markup and generates complete static HTML. No runtime needed in browser. |
| **Server Rendering** | Runtime moves to server. Browser is a shell that sends presentation templates to server, server transforms to HTML, browser merges into page. |

### Key Insight

The same runtime core works across all three modes — WASM for client, compiled Rust for static build, embedded HTTP server or WASM module for server rendering. This enables **single-file websites** (like Deno's "whole website in a single JS file").

---

## 2. HTML As DataCarrier

### The Original Idea (Now Discarded)

Early exploration used `{{dotted.notation}}` templates and custom elements like `<mount-data>`, `<for-data>`, `<index>`:

```html
<mount-data api=/v2/users>
  <h1>{{user.name.first}} <span>{{user.name.last}}</span></h1>
  <for-data context="user.schools" as="school">
    <div><label>Name:</label><span>{{school.name}}</span></div>
  </for-data>
</mount-data>
```

### The Refined Idea (What Actually Matters)

**Discard the template syntax entirely.** Use a Rust `html!` macro that compiles to plain HTML. Normal Rust structs for data, normal Rust loops for iteration. No special syntax in the HTML.

```rust
html! {
  <div id="users">
    <h1>{ user.name.first } <span>{ user.name.last }</span></h1>
    @for school in &user.schools {
      <div><label>Name:</label><span>{ school.name }</span></div>
    }
  }
}
```

The output is **plain HTML** — no `{{}}`, no `<for-data>` tags. The browser sees normal HTML. The Rust code that generated it uses normal language constructs.

### `<mount-ui />` — The Materialization Tag

```html
<mount-ui api="/v2/users" />
```

This is the one custom element that matters. It tells the runtime:
- **At load time**: Make an HTTP request to `/v2/users` and materialize the response into this spot
- **Server-rendered mode**: The server resolves it and returns full HTML (matching Accept header)
- **Client-rendered mode**: WASM resolves it and renders directly

The response can itself contain `<mount-ui />` tags for nested lazy loading.

### `<mount-data />` — The Input Tag

```html
<mount-data api="/v2/users" method="POST" data="{...}" />
```

Natural HTTP requests (POST/PUT/DELETE, default POST) to an endpoint that returns **HTML or JSON change definitions** (like Datastar's `patch-elements` / `patch-signals`).

This is the interaction model — users supply input, the server returns what needs to change, the runtime applies it. No special syntax, just HTTP.

---

## 3. Script-Level Functions & Event Binding

### The Original Idea

```html
<script>
    const showCaseController = {
         showPage: (ctx: Context, buttonNode: PrimalNode) => {},
    }
</script>

<button id="show-case-button" primal:onclick="showCaseController.showPage">Click me</button>
```

Or even simpler:

```html
<button controller="showPage" />
```

Where:
- **For events** (`primal:onclick`): The function is called when the event occurs
- **For controllers** (`controller="..."`): The function is called immediately on page load, so it can set up the DOM element (e.g., bind signals)

### How This Maps to What We've Learned

This is **exactly Stimulus's model**, simplified:

| Primal (Original) | Stimulus | Our WASM-UI |
|---|---|---|
| `primal:onclick="controller.method"` | `data-action="click->controller#method"` | Signal binding or JS event delegation |
| `controller="setupMethod"` | `data-controller="controller"` (connects on load) | Component trait `connect()` lifecycle |
| `showPage(ctx, node)` | `connect() { this.element }` | Rust Component trait with element reference |
| Script-level object on `window` | ES module registered with Application | WASM function registry |

### What Works

1. **MutationObserver for discovery** — Use Stimulus's pattern: one `MutationObserver` on the document, scan for `primal:onclick` (or whatever attribute), wire up event listeners.

2. **No expression parsing** — The attribute value is just a dotted path (`showCaseController.showPage`). No template language, no eval, no complex syntax. Just resolve the function and call it with `(event, element)`.

3. **Auto-registration** — A `<script>` tag that defines `window.showCaseController` is all that's needed. The MutationObserver picks up elements with `primal:onclick`, resolves the function, wires it up.

4. **Immediate invocation for controllers** — When an element has `controller="..."`, call the function immediately. This is the Stimulus `connect()` lifecycle — the controller can set up signals, bind event listeners, initialize state.

### What to Simplify

The original idea had `PrimalNode` with `dom`, `vdom`, `value` — a VNode abstraction. **This is unnecessary.** The function receives the real DOM `Element` and an event object. No VNode layer needed.

---

## 4. Island Controllers vs Element Controllers

### The Split

The original design broke components into two pieces:

**Island Controllers** — Stateful, self-contained portions of reactivity:
```rust
struct UserDetailIsland {
    // Business logic for a section of the page
}
```

**Element Controllers** — Stateless enhancements for specific elements:
```html
<x-button>
    <title>show user</title>
    <on-click>
        <mount-api endpoint="/user/1" as="user">
            <popup-modal>...</popup-modal>
        </mount-api>
    </on-click>
</x-button>
```

### What This Maps To

| Original Concept | Stimulus Equivalent | Our WASM-UI |
|---|---|---|
| Island Controller | Stimulus Controller with state | Component trait + Signal<T> |
| Element Controller | Custom element / web component | Headless UI component (Feature 08) |
| `<x-button>` as element controller | Stimulus controller on `<button>` | `html!` macro → `<button>` with bindings |
| Portal / island scoping | `data-controller` scope | Component root element with shadow DOM |

### The Key Insight

**Don't over-engineer custom elements.** `<x-button>` is just `<button>` enhanced with bindings. The `html!` macro in Rust produces real `<button>` HTML. The WASM runtime wires up the behavior. No custom element registration needed for most cases.

Custom elements (Feature 05) are only needed for the **web component boundary** — shadow DOM encapsulation, lifecycle callbacks. Most UI elements don't need shadow DOM.

---

## 5. Portals — Scoped Activation

### The Original Idea

```html
<portal id="component-title-section">
    <section>
        <h3>Title</h3>
        <x-button>...</x-button>
    </section>
    <script src="x-button.js" />
    <data>{}</data>
</portal>
```

Portals scope component activation to only things within the `<portal>` tag. They can contain:
- **`<portal-data>`** — Initial data for hydration
- **`<script>`** — Component definitions loaded on demand
- **Lazy loading** — Scripts only activate when portal scrolls into view

### What This Maps To

This is the **islands architecture** (Astro, Qwik). Each portal is an island:
- Self-contained section of the page
- Has its own data and scripts
- Can be lazily hydrated (IntersectionObserver triggers loading)

**In our WASM-UI**: A Component (Feature 01/05) is a portal. It has:
- A root element (the portal boundary)
- Signal state (the `<portal-data>`)
- WASM logic (the `<script>`)
- Lifecycle hooks (connect/disconnect, like Stimulus)

### Lazy Loading

The idea of loading scripts only when scrolled into view maps to:
- Stimulus's `AppearanceObserver` (used by Turbo Frames for `loading="lazy"`)
- Our web component's `connectedCallback` + IntersectionObserver
- WASM module lazy loading via dynamic import

---

## 6. Event Runtime — Separate from Everything

### The Original Idea

> "The activation of behaviours for interaction, execution or activation must be separate and on the JS side"

A central event runtime that:
1. Manages triggering events for DOM elements based on user activation
2. Elements indicate by attribute what should be done (`primal:onclick`, `primal:onpress`)
3. Delegates to a central runtime that knows what to call
4. No complex parsing syntax — just dotted path resolution

### What This Maps To

This is **Stimulus's Dispatcher** + **HTMX's event-driven model**:

- Stimulus: One `EventListener` per (target, event) combo, multiple bindings share it
- HTMX: `hx-on:click="..."` inline event handlers, resolved at runtime

**In our WASM-UI**: The JS runtime (`foundation-wasm-ui.js`) has an event dispatcher that:
1. Listens for events at the root (delegation)
2. When an event fires, looks up the handler by attribute value
3. Calls the WASM function with `(event, element)`
4. WASM handles the logic, returns DOM updates via Arrow batch

---

## 7. JIT Compilation vs Build Step

### The Original Idea

Two modes for preparing the presentation markup:
1. **JIT in browser** — First page load compiles the markup into an internal representation, checksums it, re-compiles if it changes
2. **Build step** — Pre-compile everything, ready the page for speed without browser overhead

### What This Maps To

In our WASM-UI:
- **JIT**: The Rust `html!` macro compiles at build time (no runtime JIT needed). The WASM binary is the pre-compiled representation.
- **Build step**: Static site generation — run the WASM at build time, produce static HTML files.

The original concern about "cost of transformation every time" is solved by **compiling to WASM** — the transformation happens once at compile time, not at runtime.

---

## 8. Dual Rendering Modes

### Client-Rendered

```
Browser → WASM renders → DOM updates via Arrow batches → morphing
```

The client owns all state. Server returns data, WASM renders it.

### Server-Rendered

```
Browser → sends interaction → Server renders HTML → browser morphs DOM
```

The browser is a forwarding proxy. Portal controller identifies a route, delivers triggers to server, server decides how to handle updates.

### What We've Learned

Datastar, Livewire, and HTMX all prove the server-rendered model works well:
- **HTMX**: `hx-get="/api/data"` → server returns HTML → swap into DOM
- **Livewire**: User interaction → server re-renders → Alpine morphs DOM
- **Datastar**: SSE stream → `patch-elements` → morph DOM

Our WASM-UI supports **both modes**:
- **Client mode**: WASM handles rendering, server returns data (JSON/Arrow)
- **Server mode**: Server returns HTML, WASM morphs DOM (using the morph module from Feature 02/learnings)

---

## 9. What to Keep, What to Discard

### Keep (These Ideas Are Good)

| Idea | Why | Maps To |
|------|-----|---------|
| **Plain HTML on the page** | Works without JS, progressive enhancement | `html!` macro output |
| **`<mount-ui />` materialization tag** | Clean lazy loading boundary | Web component base (Feature 05) |
| **`<mount-data />` for POST/PUT/DELETE** | Natural HTTP interaction | SSE stream actions + Arrow batches |
| **Script-level functions** | Simple, no framework needed | WASM function registry |
| **`primal:onclick="path.to.func"`** | No expression parsing needed | Event delegation in JS runtime |
| **Controller auto-connect on load** | Stimulus pattern, works well | Component `connect()` lifecycle |
| **Island scoping** | Lazy loading, self-contained | Component root + shadow DOM |
| **Server returns HTML or JSON changes** | Flexibility, like Datastar | Arrow batches (JSON alternative) |
| **Event runtime separate from everything** | Clean separation | JS event delegation + WASM handlers |
| **No VNode layer** | Real DOM, no virtual tree | Direct DOM updates via Arrow batches |

### Discard (These Ideas Are Over-Engineered)

| Idea | Why Discard | Simpler Alternative |
|------|------------|-------------------|
| **`{{dotted.notation}}` templates** | Custom syntax, parser needed | Rust `html!` macro with `{expr}` |
| **`<for-data>` custom elements** | Custom element overhead | Rust `for` loops in `html!` macro |
| **`<mount-api>` / `<mount-data>` as nested elements** | Complex nesting | HTTP endpoint returns HTML with `<mount-ui>` tags |
| **`PrimalNode` with VNode/PNode abstraction** | Unnecessary indirection | Real DOM `Element` reference |
| **`<portal-data>` JSON blocks in HTML** | Inline data in HTML is fragile | Server sends data, WASM manages state |
| **`<on-click>` as child element of button** | Non-standard HTML structure | `primal:onclick` attribute |
| **JIT compilation in browser** | WASM is already compiled | Build-time WASM compilation |
| **Component checksums for change detection** | Over-engineered | Standard WASM module versioning |
| **`<popup-onclick>` custom elements** | Every interaction as custom element | `primal:onclick` + function |
| **ElementController as functions returning HTML** | Functions returning HTML = templates | `html!` macro already does this |

---

## 10. `<mount-stream />` — Streaming Server Updates

### The Idea

`<mount-stream />` is like `<mount-data />` but for **streaming responses**. Instead of a one-shot POST/GET that returns a single HTML fragment or JSON patch, it opens a stream (SSE or chunked HTTP) and applies a sequence of changes as they arrive.

```html
<!-- Stream into self (default) -->
<mount-stream api="/v2/live-feed" method="GET" />

<!-- Stream into a sibling element -->
<mount-stream api="/v2/notifications" method="GET" target="next" />

<!-- Stream into a parent container -->
<mount-stream api="/v2/comments" method="POST" data="{ postId: 42 }" target="parent" />

<!-- Stream into a specific element by selector -->
<mount-stream api="/v2/dashboard" method="GET" target="#live-panel" />
```

### Target Modes

| Target | Behavior | Use Case |
|--------|----------|----------|
| `self` (default) | Materialize changes into the element itself | Live feed, chat messages |
| `next` | Append to the next sibling element | Notification badges next to a trigger |
| `prev` | Prepend to the previous sibling | New items before existing list |
| `parent` | Apply changes to the parent container | Form submission updating a list |
| `#selector` | Target any element by CSS selector | Dashboard panels, remote sections |

### Server Response Format

The server streams **SSE events** (like Datastar) with typed payloads:

```
event: patch-elements
data: selector #user-count
data: mode inner
data: elements <span>42</span>

event: patch-signals
data: signals {"unreadCount": 42, "lastUpdate": "2026-06-06T08:00:00Z"}

event: patch-elements
data: selector #notification-list
data: mode append
data: elements <div class="notification">New message</div>

```

The server can mix element patches and signal updates in a single stream. Each event is applied immediately as it arrives.

### How It Works

```
1. Page loads, runtime encounters <mount-stream api="/v2/live-feed" method="GET" />
2. Runtime opens SSE connection (or chunked HTTP with SSE parser)
3. Server streams events as they occur
4. For each event:
   a. Parse the event type (patch-elements, patch-signals, etc.)
   b. Resolve the target (self, sibling, parent, selector)
   c. Apply the change:
      - patch-elements → morph or direct DOM update
      - patch-signals → update WASM signal store, trigger effects
   d. Continue listening for next event
```

### Comparison with `<mount-data />`

| Feature | `<mount-data />` | `<mount-stream />` |
|---------|-----------------|-------------------|
| Method | POST/PUT/DELETE (default POST) | POST/PUT/DELETE/GET |
| Response | Single HTML fragment or JSON patch | Stream of SSE events |
| Lifetime | One request → one response | Open connection → many events |
| Use Case | Form submission, action triggers | Live feeds, notifications, real-time dashboards |
| Target | Same as stream (self, sibling, parent, selector) | Same as data |

### Why This Is Clean

1. **No WebSocket complexity** — SSE is HTTP, works through proxies/CDNs, auto-reconnects
2. **No client-side state management** — Server pushes what to change, browser applies it
3. **Progressive enhancement** — Without JS, the element is just a placeholder. With JS, it becomes a live feed.
4. **Composable** — Multiple `<mount-stream />` elements on one page, each with its own target and endpoint
5. **Familiar** — Same pattern as Datastar, but expressed as a simple HTML element instead of JavaScript API

### Real-World Examples

```html
<!-- Live notification count -->
<span id="notif-count">0</span>
<mount-stream api="/v2/notifications/stream" method="GET" target="#notif-count" />

<!-- Chat messages streaming into a container -->
<div id="chat-messages"></div>
<mount-stream api="/v2/chat/stream" method="GET" target="#chat-messages" />

<!-- Form submission that streams progress updates -->
<form>
    <input name="file" type="file" />
    <button type="submit">Upload</button>
    <div id="upload-progress"></div>
    <mount-stream
        api="/v2/upload"
        method="POST"
        data-form="this"
        target="#upload-progress"
    />
</form>

<!-- Dashboard panel that streams real-time metrics -->
<div id="dashboard">
    <mount-stream api="/v2/dashboard/stream" method="GET" target="self" />
</div>
```

### Transport-Agnostic Design

The `<mount-stream />` element is an **abstraction** — the transport is an implementation detail. The same HTML element can use different streaming protocols based on a `transport` attribute:

```html
<!-- SSE (default) — best for server-to-client streams -->
<mount-stream api="/v2/notifications" transport="sse" />

<!-- WebSocket — best for bidirectional real-time -->
<mount-stream api="ws://localhost/v2/chat" transport="ws" />

<!-- Chunked HTTP — best for large single-response streams -->
<mount-stream api="/v2/export" method="POST" transport="chunked" />

<!-- Long-polling — fallback for restrictive proxies -->
<mount-stream api="/v2/updates" transport="poll" interval="2000" />

<!-- Omit transport attribute — runtime picks best available -->
<mount-stream api="/v2/live-feed" />
```

| Transport | Protocol | Bidirectional | Use Case |
|-----------|----------|---------------|----------|
| `sse` (default) | HTTP `text/event-stream` | Server → Client | Live feeds, notifications, dashboards |
| `ws` / `wss` | WebSocket | Both ways | Chat, collaborative editing, games |
| `chunked` | HTTP `Transfer-Encoding: chunked` | Server → Client | Large data exports, progress streams |
| `poll` | HTTP repeated GET/POST | Client → Server (via request body) | Fallback for restrictive proxies/CDNs |
| `auto` | Runtime negotiates | Depends on transport | Let the runtime pick the best option |

### Unified Event Format

Regardless of transport, the **event format is the same**. The transport is just the delivery mechanism:

```
event: patch-elements
data: selector #notif-list
data: mode append
data: elements <div class="notif">New notification</div>

event: patch-signals
data: signals {"unreadCount": 5}

```

- **SSE**: Native `event:` and `data:` fields
- **WebSocket**: JSON envelope `{ "event": "patch-elements", "data": { "selector": "#notif-list", ... } }`
- **Chunked HTTP**: Same SSE format, parsed line-by-line from the chunk stream
- **Long-polling**: Same SSE format in each poll response

The runtime normalizes all transports into the same internal event stream. The server can use one format (SSE is simplest) and the runtime handles the protocol conversion.

### Transport Negotiation

When `transport="auto"` (or omitted), the runtime can negotiate:

```
1. Try SSE first (most servers support it, works through most proxies)
2. If SSE fails or server indicates WebSocket support → upgrade to WS
3. If both fail → fall back to long-polling
```

The server can advertise supported transports via response headers:

```
HTTP/1.1 200 OK
X-Supported-Transports: sse, ws, chunked, poll
```

### How This Differs from Datastar

**In Datastar** — you write JavaScript/attributes:

```html
<!-- The trigger -->
<button data-on:click="@get('/v2/notifications/stream')">
    Subscribe
</button>

<!-- Or use a watcher plugin -->
<div data-watch:notifications="@get('/v2/notifications/stream')"></div>
```

The `@get()` is a **compiled expression** — Datastar's expression compiler transforms it into a `fetchEventSource()` call with an SSE parser. The runtime has to:

1. Parse the attribute value
2. Compile the expression via `genRx()`
3. Execute the compiled function
4. Open the SSE connection
5. Parse incoming events
6. Dispatch to the appropriate watcher plugin

All of this requires understanding Datastar's attribute syntax (`@get`, `data-watch:`, etc.) and how the expression compiler works.

**With `<mount-stream />`** — you just write HTML:

```html
<mount-stream api="/v2/notifications/stream" method="GET" target="#notif-list" />
```

That's it. No expression compilation, no attribute parsing, no `@get` syntax. The runtime:

1. Scans the DOM for `<mount-stream>` elements (via MutationObserver, like Stimulus scans for `data-controller`)
2. Reads the attributes — `api`, `method`, `target` — plain HTML attributes, no special syntax
3. Opens the connection directly
4. Parses events and applies them to the resolved target

The server sends the **exact same SSE format** in both cases:

```
event: patch-elements
data: selector #notif-list
data: mode append
data: elements <div class="notif">New notification</div>

event: patch-signals
data: signals {"unreadCount": 5}

```

The difference is entirely on the **client setup** side:

| Aspect | Datastar Approach | `<mount-stream />` Approach |
|--------|------------------|----------------------------|
| Client code | `@get('/api/stream')` expression in attribute | Plain HTML element with attributes |
| Compilation | Expression compiler (`genRx()`) transforms `$signal` references and `@actions` | None — attributes are read directly |
| Discovery | MutationObserver finds `data-watch:` attributes | MutationObserver finds `<mount-stream>` elements |
| Configuration | Embedded in expression syntax | Plain attributes (`api`, `method`, `target`) |
| Server response | SSE events (`datastar-patch-elements`, `datastar-patch-signals`) | SSE events (`patch-elements`, `patch-signals`) — identical format |

### The Key Insight

**The server doesn't change at all.** It streams the same SSE format regardless of whether the client is Datastar or our `<mount-stream />` element. The only difference is how the client initiates the connection:

- **Datastar**: Compile an expression, execute it, open SSE
- **`<mount-stream />`**: Read HTML attributes, open SSE

This is the same philosophy as `<mount-data />` and `<mount-ui />` — declarative HTML elements that the runtime discovers and acts on, no expression language needed.

### The `<mount-data />` Connection

`<mount-data />` is just `<mount-stream />` with `transport="single"` — one request, one response, close. They share the same target resolution, event parsing, and DOM application logic:

| Feature | `<mount-data />` | `<mount-stream />` |
|---------|-----------------|-------------------|
| Transport | Single HTTP request/response | Persistent stream (SSE/WS/chunked/poll) |
| Events | One batch | Many batches over time |
| Lifecycle | Request → Response → Done | Connect → Stream → (Reconnect) → ... |
| Target | Same resolution | Same resolution |
| Event format | Same | Same |

---

## 11. The Refined Vision for WASM-UI

Combining the best of the original Primal ideas with everything we've learned:

### The Page

```html
<!DOCTYPE html>
<html>
<head>
    <script src="foundation-wasm.js"></script>
    <script src="foundation-wasm-ui.js"></script>
</head>
<body>
    <h1>Users</h1>

    <!-- Server-rendered initial content -->
    <div id="user-list">
        <div class="user" id="user-1">
            <span>Alice</span>
            <button primal:onclick="userController.delete" data-user-id="1">Delete</button>
        </div>
    </div>

    <!-- Lazy-loaded section -->
    <mount-ui api="/v2/analytics" lazy />

    <script>
        const userController = {
            delete: (event, element) => {
                // WASM handles the delete via Arrow batch
                // Returns: remove #user-1 from DOM
            }
        };
    </script>
</body>
</html>
```

### The Rust Component

```rust
#[component]
fn UserList(ctx: &Context) -> impl Html {
    let users = ctx.signal::<Vec<User>>("users");

    html! {
        <div id="user-list">
            @for user in users.iter() {
                <div class="user" id={format!("user-{}", user.id)}>
                    <span>{ user.name }</span>
                    <button
                        primal:onclick="userController.delete"
                        data-user-id={user.id.to_string()}
                    >Delete</button>
                </div>
            }
        </div>
    }
}
```

### The Interaction Flow

```
1. User clicks "Delete" button
2. JS event dispatcher catches event
3. Resolves "userController.delete" → calls WASM function
4. WASM sends Arrow batch to server (POST /v2/users/1/delete)
5. Server processes, returns HTML fragment or Arrow DOM commands
6. JS runtime applies changes to DOM (morph or direct update)
```

### The Core Principles

1. **Plain HTML** — Everything is valid HTML, works without JS
2. **Attributes for behavior** — `primal:onclick="path.to.func"` — no expression parsing
3. **HTTP for everything** — POST/PUT/DELETE to endpoints, server returns HTML or change definitions
4. **Rust macros for generation** — `html!` macro produces plain HTML, normal Rust for logic
5. **Signals for state** — Fine-grained reactivity (R3-inspired signal system)
6. **Arrow for batching** — Efficient WASM→JS DOM updates
7. **Morphing for updates** — Server returns HTML, morph applies changes preserving state
8. **Islands for scoping** — `<mount-ui />` tags define lazy-loaded boundaries
9. **Event delegation** — One listener per event type at root, dispatches to registered handlers
10. **No VNode** — Real DOM, direct updates, morphing for complex changes

---

## 11. Summary of Lessons from the Original Vision

The original Primal exploration was searching for the right abstraction level — not too low (raw DOM manipulation), not too high (full component framework). The answer, validated by all the frameworks we've studied, is:

- **HTMX** proved that plain HTML + attributes + HTTP is powerful enough for most interactions
- **Stimulus** proved that MutationObserver + attribute-based controller discovery is the right pattern for client-side behavior
- **Datastar** proved that SSE streaming + morphing + signals is a clean server-driven model
- **Alpine** proved that effects + two-way binding + minimal directives covers the reactive cases
- **Livewire** proved that server-side state + client-side morphing preserves form state and feels snappy
- **Svelte** proved that compile-time optimization + fine-grained signals eliminates the need for VDOM

The original Primal ideas of `{{}}` templates, `<for-data>` elements, and `PrimalNode` abstractions were all trying to solve problems that **don't need solving** — Rust's `html!` macro, normal language constructs, and real DOM references are simpler and more powerful.

What **does** matter from the original vision:
- `<mount-ui />` as the materialization boundary
- `<mount-data />` for one-shot POST/PUT/DELETE interactions
- `<mount-stream />` for streaming SSE updates with target resolution
- `primal:onclick` as the event binding mechanism
- Script-level functions as the controller model
- Island scoping as the lazy loading strategy
- HTTP as the interaction protocol
- HTML as the response format (with Arrow as an optimization)
