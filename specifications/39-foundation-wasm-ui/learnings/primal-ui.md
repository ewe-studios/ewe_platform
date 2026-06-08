# Primal UI — Original Vision & Lessons Learned

## Source

Original Notion design notes: "HTML As DataCarrier V2" — early conceptual exploration of a UI framework built on plain HTML with minimal runtime enhancement, island architecture, and server/client dual rendering.

---

## 1.0 Core Ideas: Dom Updates and Rendering

1. Island focused architecture where the wasm or server owns the actual rendering and also is the defactor owner of state updates, the browser / client is simply the presentation layer.

2. Signals are the defacto means of reactivity, and signals dont just live on the client but also come from the server/wasm, state changes are communicated as actions (request either query or body contains: the current state the client was giving, could be a subset for the specific component and the action they want performed), which allows the server receive, process and respond with a response telling the client what to do:
  a. This could be pure json body that has nothing to do about dom updates, signals and the component has code on the client side to process that it process any how it wants via fetch.
  b. The server responds with a specific content type indicating its a signal update e.g application/primal+json (for the json updates - the content indicates if its signal or DOM updates), application/primal+event-stream event stream of primal data (signals, doms, state update), this ensures we can clearly indicate when its a primal ui style response and not your regular json or event stream. 
  

Everything else is the server or wasm sending html content (optimized via arrow batches) to the browser for zero deserialization and applied updates.


This means it can be from the wasm initialized in the page, in a web worker, in a service worker or from the server.

Actions go to the wasm, server, web worker, service worker then gets back html to apply and morph with.

## 1.1 Event Runtime — Separate from Everything

We utilize mutation observers, event bubbling and basic javascript attached to dom nodes that is owned by the html elements in that scope. This is why i want to introduce a special web component.


### Events can be attached based on basic html `primal:{event}` attributes

Simple attribute based event attachement, letting users define script tags containing code that gets attached to the window context (or whatever users want).

And tell the component to attach the event to he function referenced in the value, no special process needed, its literally a javascript  reference string that if we use will get the function to call and use it.

```html
<script>
    const showCaseController = {
         showPage: (ctx: Context, buttonNode: PrimalNode) => {},
    }
    window.showCaseController;
</script>

<button id="show-case-button" primal:onclick="showCaseController.showPage">Click me</button>
```

Or even simpler:

```html
<script>
    const showPage = (ctx: Context, buttonNode: PrimalNode) => {},
    window.showPage = showPage;
</script>
<button primal::on-click="window.showPage" />
```

We can use mutation observers and as well querySelectorAll to find these and get them wired up.

### Events and wiring up are just script tags in dom elements with scoped attributes

I want users to be able to express themselves with html, css and javascript like normal without complexity, so in my mind user should be able to do:

```
<div id="menu-tabs">
  <script type="text/javascript">
   // normal javascript stuff that is not scoped and just is a tag like others and we do what we need to do
  </script>
  <script type="text/javascript" scoped primal:script>
    // a specific javascript tag scoped to the containing div, 
    // ensuring to only export a annoymouse function, the framework will use text to take the content, parse and materialize it within a function e.g (function(scope){})(scope) where scope has all the needed parameters and configuration to focus on the html node that owns the script with these attributes.
    function(scope, ...) {
      // scope represents the scoped target the focuses operation
      // around the node this script is defined in, this allows 
      // operations to be very specific to this node.
      let parents = scope.targets();
      
      // users want to handle teardown themselves
      parent.addEventListener(...)

      or 

      addEvent(parent, "click", () -> {})

      Do something, addEvent wires up the needed event, adds the needed registration that this element has events, and the needed tear down to ensure if this ever gets removed from the dom then its gets properly turned down, it could use event bubbling and wire the event to the body tag and instead when event bubble, it sees if the dom node is the target and react (like what jquery does) to reduce tear down issues.
    }
  </script>

</div>

```

We can apply this same idea the same idea around, we could also allow variants that lets user specify the scope to specific dom node/nodes like:

```
  <script type="text/javascript" scoped="div#menu-tabs" primal:script>
```

OR:

```
  <script type="text/javascript" scoped="div.tabs" primal:script>
```

The `scoped` is a standard selector you would pass to querySelectorAll that returns 1 or more objects (we enforce 1 or more and throw/raise error if zero  cause it should apply to something).

This way users get a clean api that applies to 1 or more elements and our nice helper methods like addEvents/etc make this all super easy to do.

I am thinking we should not have free methods but instead have a central `primal` scoped variable on window or the global context (this) so users can do `primal.addEvent()` etc.


## 2. Components
. 
### Scroll or Appearance Observers: react to Scroll

Like Stimulus's `AppearanceObserver` (used by Turbo Frames for `loading="lazy"`), we can allow components to only load themselves when they know they are close to be viewed by the scroll wheel, letting things stay dormant and they load themselves and cache content then materialize it when the scroll will is close to them, further reducing page load cost and requests.


### No templates but just plain rust macros creating html

**Discard the templating language syntax entirely.** Use a Rust `html!` macro that compiles to plain HTML. Normal Rust structs for data, normal Rust loops for iteration. No special syntax in the HTML.

```rust
html! {
  <div id="users">
    <!-- Rust struct fields just get placed in -->
    <h1>{ self.user.name.first } <span>{ self.user.name.last }</span></h1>
  }
}
```

See more in specifications/39-foundation-wasm-ui/old_features/03-html-templates/feature.md

## Components of Islands - Interactive or Non-Interactive.

These are defined components of island which are dynamic content that pull and expect primal content from the server, and they expect the response sent by the server to be primal style responses:

- Html
- JSON (signal updates, state update)

They dont care much if its using the primal content type headers, but will treat it as signal, and dom updates based on the structure of the data, this works whether its RPC, HTTP response, SSE event streams.

We utilize mutation observers, event bubbling and basic javascript attached to dom nodes that is owned by the html elements in that scope. This is why i want to introduce a special web component.

```html
<island>
    <div>...</div>
    <style></style>
    <script>
        // the island injects a scope() function that will focus down all operations to just its own children. This allows users still refer to the dom, but use scope() specifically to focus on just whats in the content the island contains.
        let div = scope().querySelector("div");

        // helper functions that makes event attachement easier and simple.
        addEvent(div, "onClick", () => {...})
    </script>
</island
```

Where the island will have a created web  component representing the island and it will take care of scoping the csss and script  to anything within its children. This can be shadow dom or basic html parents.

You will notice its all standard html, nothing special, no templating process, wasm or server can return this and the js already just has the custom element / web component defined to handle this.

It can contain all other components that get perform other operations with the server/wasm or in the page, can be web components themselves.


### Decision 1: Dynamic Content Templating is a  NOGO

The output is **plain HTML** — no `{{}}`, no `<for-data>` tags. The browser sees normal HTML. The Rust code that generated it uses normal language constructs.

### Component 1: `<mount-ui />` — The Materialization Tag

```html
<mount-ui api="/v2/users" />
```

This is the one custom element that matters. It tells the runtime:
- **At load time**: Make an HTTP request to `/v2/users` and materialize the response into this spot
- **Server-rendered mode**: The server resolves it and returns full HTML (matching Accept header)
- **Client-rendered mode**: WASM resolves it and renders directly

The response can itself contain `<mount-ui />` tags for nested lazy loading.

### Component 2:  `<mount-data />` — The Input Tag

```html
<mount-data api="/v2/users" method="POST" data="{...}" />
```

Natural HTTP requests (POST/PUT/DELETE, default POST) to an endpoint that returns **HTML or JSON change definitions** (like Datastar's `patch-elements` / `patch-signals`).

This is the interaction model — users supply input, the server returns what needs to change, the runtime applies it. No special syntax, just HTTP.


### Component 3:  `<mount-stream />` — The Stream and optional input Tag

We add a new `<mount-stream />` or `<mount-stream state={} />` — Streaming Server Updates

### The Idea

`<mount-stream />` is like `<mount-data />` but for **streaming responses**. Instead of a one-shot POST/GET that returns a single HTML fragment or JSON patch, it opens a stream (SSE or chunked HTTP) and applies a sequence of changes as they arrive, optionally allowing us to pass data/state to the server for the requests sent. Like datastar lets use fetch instead and process the event stream, i think we can pass it to wasm (if from a http server) to process or if its the wasm responding then it uses arrow for fast zero deserialization speed.

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

The content type indicates to us if its: html, json or arrow.

*TODO*: This needs update, the source can be a wasm instant instantianted for the page (webworker, in main thread, service worker) or a http endpoint. Also, the transport is just mechanism, it does not matter how we get this: SSE, websocket, plain http response.

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
