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
  <script type="text/javascript" scoped="div#menu-tabs" primal:script></script>
```

OR:

```
  <script type="text/javascript" scoped="div.tabs" primal:script></script>
```

The `scoped` is a standard selector you would pass to querySelectorAll that returns 1 or more objects (we enforce 1 or more and throw/raise error if zero  cause it should apply to something).

This way users get a clean api that applies to 1 or more elements and our nice helper methods like addEvents/etc make this all super easy to do.

I am thinking we should not have free methods but instead have a central `primal` scoped variable on window or the global context (this) so users can do `primal.addEvent()` etc.



## 1.2 Styling Runtime — Separate from Everything

In my mind, styling should not be anything unique, we should let the web do what the web do, we could support tailwind style specific css attributes but its optional and users can just specify css as they like.

But the web always had issues with scoped css where this css should only apply to just this eleemnt and its children, whilst shadow dom fixes this, its not a silver bullet and has its own gotchas and support matrix.

So i would like to like script support normal `style` dom nodes and scoped ones like the script tag above in 1.1

```
  <style type="text/css" scoped primal:style></style>
  <style type="text/css" scoped="div#menu-tabs" primal:style></style>
```

Where if a style tag has `primal:style` in then its applied to the parent dom node where it appears under (the parent), and if `scoped` has a value then its applied to the dom nodes matching that selector which is 1 or more.

We need a way to ensure the styles once we pull the content via the style.text to:

1. Extract the css and hydrate them into a structure that is performant representation of css, this allows us during the html parsing clearly represent them properly and then use this to create specific apply rules that apply to specific objects matching the selectors, scoped already provides the target, so we can in the simplest sense just prefix the selectors with the tag and id or tag and class for that parent and it should properly scope it. 
2. I think if we create a actual repreentation of css then we can just wrap them in a parent css that represent the scoped tags, and when the scoped produces many dom nodes, we just ensure to create scoped css code for each and do a simple <style> append that can be pushed first to the head or body before appending the element, though we might need to be careful here, if html does not care where it appears then its good to style put in the parent of the target so its easy to also remove later, another approach is to have what i call the central atomic StyleManager.

The idea is: We create a StyleManager that will takes all the css styles, break them down into their atomic units e.g margin-large, paddng, ...etc like tailwind does, then returns the list of atomic css rules for a giving dom node, this way we can centralize all them, creating unique custom css rules for very specific things, then these can be added to added to a central style tag which will own all of them, we ensure to deduplicate them and ensure they are very specific to the property they are applying.

I reason there will be cases where some rules are just so specific e.g animations, in such situations, we can use the matching node to ensure the css rules is specific to it and not anything else.

Lets think deeply, and design a system that works extensively for this type of behaviour for css.

But the core idea is: when a style has scoped and `primal:style` it is treated differently and in the simplest situation we just add a prefix for each dom node and clone all the rules to only apply to that specific dom node so the rules stay scoped and not break other things.

In a more engineered and performant system, we use the css structure to deduplicate similar rules, move them into a shared style tag (they will never get duplicated since we deduplicate by a central style manager) and anything specific is scoped to the dom nodes they affected based on `scoped` selector matching.


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
    <mount-stream api="/v2/dashboard/stream" method="GET" target="div#dashboard" />
</div>
```

### Transport-Agnostic Design

The `<mount-stream />` element is an **abstraction** — the transport is an implementation detail. The same HTML element can use different streaming protocols based on a `transport` attribute:

**TODO**: there should always be the central manager which gets the actions they are needed to be performed, it can then either respect their need for a custom transport or use the already selected transport being used, this lets it use SSE, websocket or whatever has been set.
This also lets it own the request bundling via queueMicroTask which lets it schedule all these to the wasm or server.

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
    // TODO: this is interesting, but the idea is when the 
    // project is built, and we generate the js, we should be bundling all this into the single js file we generate, users should not need to care about this.
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

**TODO**: I like the Context, its a nice abstraction and we can use it to own the full signal chain, this way there is a control and scoping of the context, pass it around and even have multiple contexts for different parts of the UI.
But it needs more elaboration, how does it work, where is it set up, who manages it, how does the rust component get it, alot of things is vague.

```rust
#[component]
fn UserList(ctx: &Context) -> impl Html {
    let users = ctx.signal::<Vec<User>>("users");

`   **TODO**: i hate this @for directive, this is now custom 
    stuff not needed, we are using macros here, nothing stops us from creating writing a rust for loop and yields a Vec of html elements that gets append into this.
    
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

---

## 11. Summary of Lessons from the learnings docs.

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


## Big TODOs

1.0 Lets review all the todos across all features and this requirement file. For now i have moved all the old features in ./specifications/39-foundation-wasm-ui/old_features and then rewrite them following a more refined thoughtful designed process.
1.1 
2. I want to lean more into the core ideas of primal-ui in specifications/39-foundation-wasm-ui/learnings/primal-ui.md (but we need to make it clear what exactly we are bring in and what is out)
3. Its now clear, i would like to expand our codegen tooling (foundation_codegen) with capability javascript and typescript generation capabilities after seeing how web-rs does this (see specifications/39-foundation-wasm-ui/learnings/web-gen-ts-binding-generation.md)
4. Also its clear i want to refactor and make communication protocol aware, so that interactions always start with protocol and version starters in the messages sent back and forth to support multiple protocol and versions e.g our current custom binary protocol and arrow messages.
5. Its seems reasonable to also have some central fetch wrapper that knows how to batch API requests to reduce the thundering heard problem and use this everywhere so that we can control and better manage outgoing requests and it wrapping the fetch allows us to be smart in how this works, how long it waits to batch or if it batches based on how many times it gets triggered in the shortest amount of time, we need to think about this, research and see what others do or if this is even a good idea.
6.Its clear we want to be smart with how we define our WebComponent setup and not go crazy creating many different types but instead a specific set of types which understand how to interact in some specific way e.g mount-api, mount-stream, mount-data, we need to clearly define this, how they work and create a generic web component where these build on and doing it well will allow them just automatically work since we move e.g http communication to a service worker when its available and transparently owns the communication and responds and properly proxies to the server, but for this like web-workers, we might want to maybe add a mount-from-worker (to indicate this is coming from a webworker? I am unsure if this is a good idea) or if there is something we can do to indicate via mount-api, mount-stream, mount-data if its going to a web worker which might be better, I think i like this better, users can probably add a `worker=name-of-worker` and a central system that knows the web-workers (probably our webworkers) add them selves to some list and then the name just cleaning map and uses the worker communication proxy to deliver the messages to it and workers send back their response to them - we figure the right way to identify whoes response hook will get the reply.
7. Its clear we want to support: direct invocation, web-worker execution, service workers (when possible, which will allow isomorphic http endpoints that get intercepted before they go to the server or remote endpoint) and so need to think more about how this should work.
  a. I was thinking just like we do with the #[wasm_bin] proc macro, we can mark functions further that specific use #[wasm_bin], new proc macros that indicate how its going to be executed:
    - `#[wasm_bin]` — regular WASM function, executed in the main thread
      - #[wasm_bin(js=single-file, encoded=b64|uint8array)] - generates also a js wrapper which will encoded the generate wasm beside it as a single js file and by default add it as a Uint8Array else base64 encoded data with the needed logic to decode and initialize it.
    - `#[wasm_worker]` — executed in a web worker and also will generate a js wrapper for it and could have a marker js=single-file to indicate when present to not just generate a wasm but then create a js file which will base64 encode the wasm into the js file and setup the necessary logic to have it running which can be served like a regular file and if not then it automatically assumes where ever its (the js) is served, it will just ask the server for the wasm file in the web-worker.
    - `#[wasm_worker(js=single-file, encoded=b64|uint8array)]` — executed in a web worker and also will generate a js wrapper file will base64 encode the wasm into the js file and setup the necessary logic to have it running which can be served like a regular file and if not then it automatically assumes where ever its (the js) is served, it will just ask the server for the wasm file in the web-worker. When the js property is present then we look for encoded which by default is `uint8array` where we just store the raw bytes in a Uint8Array (see specifications/39-foundation-wasm-ui/learnings/wasm-delivery.md) and letting the server compress it. 
    - `#[wasm_service]` — executed in a service worker - which will let users present a fetch endpoint (yes we are stealing from cloudflare) which lets us present a http endpoint to fetch content and a route() method that returns the routes the service worker should scope for going to the wasm else passing them along to the server.
    - `#[wasm_service(js=single-file, encoded=b64|uint8array)]` — executed in a service worker and following the same semantics as #[wasm_worker] to support how its encoded into the single file when we generate it.
8. I am super interesting in data star signal communication to the server, how does it work, how does it first set it up on the server and communicate to the client? Lets dig in and update the learnings (specifications/39-foundation-wasm-ui/learnings) with a more detailed exploration of it, check the data store code  location (see /home/darkvoid/Boxxed/@formulas/src.UIFrameworks/src.starfederation/datastar/ and /home/darkvoid/Boxxed/@formulas/src.UIFrameworks/src.starfederation/datastar-go/ and https://data-star.dev/guide/backend_requests). Also datastar as a very interesting html, signal merging logic that we can definitely learn from, adapt for our needs, make reusable for both sides (rust and js - js for the actual dom, rust can use json or arrow to merge changes into a map structure? - lets think on it).
9. I am very interested in setting up a proxy for {} which allows us patch but listen for changes on js object, javascript has the proxy object that you see alpine, datastar use, lets learn as much in how each sets those up, use them, interact with them, update them and come up a clear idea of the good parts we can adapt for our approach. Also datastar sets up a way to ensure all signals are updated as a batch instead of one at a time, by adding them to a process/flush queue and the queue just contains functions that get executed all at once, allowing us to keep things closely linked to get triggered and updated together. This is really good. I would really like to see how datastar and r3 really compare, their difference, whats great in each side and how we can take each of those, create a specification for signals for both js and rust side that adapt these good parts to create a more resilient and cool signal framework (see /home/darkvoid/Boxxed/@dev/ewe_platform/specifications/39-foundation-wasm-ui/learnings/r3-vs-datastar-signals.md which already does this, especially the automatic depth creation for signals). I added notes we should adapt in our features to guide our thinking.
10. I leant alot today about how livewire does request bundling, instead of using a timeout to batch requests, it instead setups a manager which it registers to the queueMicroTask (letting the browser call it when its ready) and within that period multiple elements (dom nodes, processes) can register their requests to the manager which will then bundle all of those together into a single request object or batch deliver them, letting the server also respond in kind but with an explicit id to identify the response from the others, in my mind, i can bundle all the requests  content together and let the server stream the response back via SSE or Websocket which resolves a need to let the slowest block all response but we need to ensure we create a concrete structure for the structure that represent what the request about, this way SSE or Websocket can send back another well structured response that has the id to indicate this is the response for this - it does require a different way of thinking about how the body is structured.
11. Another cool thing for datastar is - it uses fetch instead of the EventSource API so it can send other HTTP methods to the backend, then it can just listen to the request response continously listening and reading the SSE responses from it, and for GET request, the body is actually base64 encoded and included as a query parameter (normal stuff) which the server will know and handle.
12. Also, it has this idea that all signals (data signals) should always be sent to the server, like livewire, letting the server, decide what should change and send SSE data to update them which lets the server decide how the client changes and not the other way around. Datastar and livewire share this, i was always worried about state and the server needing to be stateful but always immagined we needed some new stateful setup or thinking to get this to work but if the client always present back its state then its not something the server needs to keep and the server can then build specific logic for the differnt usecase on how and what state it sends to client, the state it gets back, how that state should change e.g logged in or not, etc, this creates very interesting senergy. Our primal-ui lets client stay client, and get updates from server, we can also add the fact to them that they can also communicate some data to the server when they fetch that request endpoint which the server can respond back just like regular http handler that get queries or request body and use that to define how they respond either as a normal single request response lifecycle (HTTP, RPC e.g ConnectRPC) or they get the request and deliver the update via SSE/Websocket
13. Lets create a feature focused on dom updates and form preservation taking key ideas from datastar which combines morphdom & idiomorph to create a consistent dom morphing system that works for our primal ui usecase, see specifications/39-foundation-wasm-ui/learnings/domupdates/datastar-morph.md
